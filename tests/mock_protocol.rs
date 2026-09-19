use async_trait::async_trait;
use chrono::Duration;
use deysis_trust_boundaries::crypto::{DeviceCredential, DeviceIdentity};
use deysis_trust_boundaries::{
    crypto::KeyStore,
    mock_provider::MockAttendanceProvider,
    provider::{ActionRequest, AttendanceProvider},
    queue::Job,
    worker,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[tokio::test]
async fn valid_flow_accepts_and_replay_is_rejected() {
    let provider = MockAttendanceProvider::default();
    let keys = KeyStore::new([1; 32]);
    let (credential, _) = keys.generate().unwrap();
    let session = provider.authenticate("user-a", "secret").await.unwrap();
    provider
        .register_device(&session, credential.identity.clone())
        .await
        .unwrap();
    let challenge = provider
        .request_challenge(&session, credential.identity.id, "check-in")
        .await
        .unwrap();
    let message = format!("{challenge}|{session}|check-in|region:demo");
    let request = ActionRequest {
        user_id: "user-a".into(),
        device_id: credential.identity.id,
        session_id: session.clone(),
        action: "check-in".into(),
        location_claim: "region:demo".into(),
        challenge: challenge.clone(),
        signature: credential.sign(message.as_bytes()),
    };
    assert!(provider.submit(request.clone()).await.is_ok());
    assert!(provider.submit(request).await.is_err());
}

#[tokio::test]
async fn expired_challenge_and_binding_mismatch_fail() {
    let provider = MockAttendanceProvider::new(Duration::milliseconds(1));
    let keys = KeyStore::new([2; 32]);
    let (credential, _) = keys.generate().unwrap();
    let session = provider.authenticate("user-a", "secret").await.unwrap();
    provider
        .register_device(&session, credential.identity.clone())
        .await
        .unwrap();
    let challenge = provider
        .request_challenge(&session, credential.identity.id, "check-in")
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let message = format!("{challenge}|{session}|check-in|region:demo");
    let request = ActionRequest {
        user_id: "user-b".into(),
        device_id: credential.identity.id,
        session_id: session,
        action: "check-in".into(),
        location_claim: "region:demo".into(),
        challenge,
        signature: credential.sign(message.as_bytes()),
    };
    assert!(provider.submit(request).await.is_err());
}

#[tokio::test]
async fn bounded_runner_completes_all_jobs() {
    let provider = Arc::new(MockAttendanceProvider::default());
    let results = worker::run_bounded(
        provider,
        KeyStore::new([3; 32]),
        (0..10)
            .map(|n| Job::new(format!("u{n}"), "intent", "check-in", "region:demo"))
            .collect(),
        2,
    )
    .await;
    assert_eq!(results.len(), 10);
    assert!(results.iter().all(Result::is_ok));
}

#[tokio::test]
async fn bounded_runner_never_exceeds_configured_concurrency() {
    let provider = Arc::new(InstrumentedProvider::new());
    let results = worker::run_bounded(
        provider.clone(),
        KeyStore::new([4; 32]),
        (0..20)
            .map(|n| Job::new(format!("u{n}"), "intent", "check-in", "region:demo"))
            .collect(),
        3,
    )
    .await;
    assert!(results.iter().all(Result::is_ok));
    assert!(provider.max.load(Ordering::SeqCst) <= 3);
}

#[tokio::test]
async fn signature_and_context_mutations_are_rejected() {
    let provider = MockAttendanceProvider::default();
    let keys = KeyStore::new([5; 32]);
    let (credential, session, challenge, location) =
        issue(&provider, &keys, "user-a", "check-in").await;
    let message = format!("{challenge}|{session}|check-in|{location}");
    let mut request = ActionRequest {
        user_id: "user-a".into(),
        device_id: credential.identity.id,
        session_id: session.clone(),
        action: "check-in".into(),
        location_claim: location.clone(),
        challenge: challenge.clone(),
        signature: credential.sign(message.as_bytes()),
    };
    request.location_claim = "region:changed".into();
    assert!(provider.submit(request).await.is_err());

    let (credential, session, challenge, location) =
        issue(&provider, &keys, "user-a", "check-in").await;
    let message = format!("{challenge}|{session}|check-in|{location}");
    let mut request = ActionRequest {
        user_id: "user-a".into(),
        device_id: credential.identity.id,
        session_id: session,
        action: "check-in".into(),
        location_claim: location,
        challenge,
        signature: credential.sign(message.as_bytes()),
    };
    request.signature = "not-a-signature".into();
    assert!(provider.submit(request).await.is_err());
}

#[tokio::test]
async fn wrong_device_session_action_and_unregistered_device_fail() {
    let provider = MockAttendanceProvider::default();
    let keys = KeyStore::new([6; 32]);
    let (_credential, session, challenge, location) =
        issue(&provider, &keys, "user-a", "check-in").await;
    let (other, _, _, _) = issue(&provider, &keys, "user-a", "check-in").await;
    let message = format!("{challenge}|{session}|check-in|{location}");
    let request = ActionRequest {
        user_id: "user-a".into(),
        device_id: other.identity.id,
        session_id: session.clone(),
        action: "check-in".into(),
        location_claim: location.clone(),
        challenge,
        signature: other.sign(message.as_bytes()),
    };
    assert!(provider.submit(request).await.is_err());

    let (credential, _session, challenge, location) =
        issue(&provider, &keys, "user-a", "check-in").await;
    let other_session = provider.authenticate("user-a", "secret").await.unwrap();
    let message = format!("{challenge}|{other_session}|check-in|{location}");
    let request = ActionRequest {
        user_id: "user-a".into(),
        device_id: credential.identity.id,
        session_id: other_session,
        action: "check-in".into(),
        location_claim: location,
        challenge,
        signature: credential.sign(message.as_bytes()),
    };
    assert!(provider.submit(request).await.is_err());

    let (credential, session, challenge, location) =
        issue(&provider, &keys, "user-a", "check-in").await;
    let message = format!("{challenge}|{session}|other-action|{location}");
    let request = ActionRequest {
        user_id: "user-a".into(),
        device_id: credential.identity.id,
        session_id: session,
        action: "other-action".into(),
        location_claim: location,
        challenge,
        signature: credential.sign(message.as_bytes()),
    };
    assert!(provider.submit(request).await.is_err());

    let (credential, session, challenge, location) =
        issue(&provider, &keys, "user-a", "check-in").await;
    let unregistered = DeviceIdentity {
        id: uuid::Uuid::new_v4(),
        public_key: credential.identity.public_key.clone(),
    };
    let message = format!("{challenge}|{session}|check-in|{location}");
    let request = ActionRequest {
        user_id: "user-a".into(),
        device_id: unregistered.id,
        session_id: session,
        action: "check-in".into(),
        location_claim: location,
        challenge,
        signature: credential.sign(message.as_bytes()),
    };
    assert!(provider.submit(request).await.is_err());
}

async fn issue(
    provider: &MockAttendanceProvider,
    keys: &KeyStore,
    user: &str,
    action: &str,
) -> (DeviceCredential, String, String, String) {
    let (credential, _) = keys.generate().unwrap();
    let session = provider.authenticate(user, "secret").await.unwrap();
    provider
        .register_device(&session, credential.identity.clone())
        .await
        .unwrap();
    let challenge = provider
        .request_challenge(&session, credential.identity.id, action)
        .await
        .unwrap();
    (credential, session, challenge, "region:demo".into())
}

#[derive(Clone)]
struct InstrumentedProvider {
    inner: MockAttendanceProvider,
    active: Arc<AtomicUsize>,
    max: Arc<AtomicUsize>,
}
impl InstrumentedProvider {
    fn new() -> Self {
        Self {
            inner: MockAttendanceProvider::default(),
            active: Arc::new(AtomicUsize::new(0)),
            max: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl AttendanceProvider for InstrumentedProvider {
    async fn authenticate(
        &self,
        user_id: &str,
        secret: &str,
    ) -> Result<String, deysis_trust_boundaries::provider::ProviderError> {
        let current = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max.fetch_max(current, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        self.inner.authenticate(user_id, secret).await
    }
    async fn register_device(
        &self,
        session: &str,
        identity: DeviceIdentity,
    ) -> Result<(), deysis_trust_boundaries::provider::ProviderError> {
        self.inner.register_device(session, identity).await
    }
    async fn request_challenge(
        &self,
        session: &str,
        device_id: uuid::Uuid,
        action: &str,
    ) -> Result<String, deysis_trust_boundaries::provider::ProviderError> {
        self.inner
            .request_challenge(session, device_id, action)
            .await
    }
    async fn submit(
        &self,
        request: ActionRequest,
    ) -> Result<
        deysis_trust_boundaries::provider::ProviderResult,
        deysis_trust_boundaries::provider::ProviderError,
    > {
        let result = self.inner.submit(request).await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        result
    }
}
