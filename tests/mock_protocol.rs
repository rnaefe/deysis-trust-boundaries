use async_trait::async_trait;
use chrono::Duration;
use deysis_trust_boundaries::crypto::{DeviceCredential, DeviceIdentity};
use deysis_trust_boundaries::{
    action_signature_payload,
    crypto::KeyStore,
    mock_provider::MockAttendanceProvider,
    provider::{ActionRequest, AttendanceProvider, ProviderError},
    queue::Job,
    worker,
};
use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use uuid::Uuid;

#[tokio::test]
async fn valid_flow_accepts_and_replay_is_rejected() {
    let provider = MockAttendanceProvider::default();
    let keys = KeyStore::new([1; 32]);
    let (credential, session, challenge, location) =
        issue(&provider, &keys, "user-a", "check-in").await;
    let request = signed_request(
        &credential,
        "user-a",
        session,
        "check-in",
        location,
        challenge,
    );
    assert!(provider.submit(request.clone()).await.is_ok());
    assert!(matches!(
        provider.submit(request).await,
        Err(ProviderError::Challenge(message)) if message == "challenge already consumed"
    ));
}

#[tokio::test]
async fn expired_challenge_is_rejected() {
    let provider = MockAttendanceProvider::new(Duration::milliseconds(1));
    let keys = KeyStore::new([2; 32]);
    let (credential, session, challenge, location) =
        issue(&provider, &keys, "user-a", "check-in").await;
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let request = signed_request(
        &credential,
        "user-a",
        session,
        "check-in",
        location,
        challenge,
    );
    assert!(matches!(
        provider.submit(request).await,
        Err(ProviderError::Challenge(message)) if message == "challenge expired"
    ));
}

#[tokio::test]
async fn wrong_user_is_rejected_without_consuming_challenge() {
    assert_binding_mutation_rejected(|request| request.user_id = "user-b".into()).await;
}

#[tokio::test]
async fn wrong_session_is_rejected_without_consuming_challenge() {
    assert_binding_mutation_rejected(|request| request.session_id = "other-session".into()).await;
}

#[tokio::test]
async fn wrong_device_is_rejected_without_consuming_challenge() {
    assert_binding_mutation_rejected(|request| request.device_id = Uuid::new_v4()).await;
}

#[tokio::test]
async fn wrong_action_is_rejected_without_consuming_challenge() {
    assert_binding_mutation_rejected(|request| request.action = "check-out".into()).await;
}

#[tokio::test]
async fn modified_location_and_malformed_signature_do_not_consume_challenge() {
    for mutation in ["location", "signature"] {
        let provider = MockAttendanceProvider::default();
        let keys = KeyStore::new([3; 32]);
        let (credential, session, challenge, location) =
            issue(&provider, &keys, "user-a", "check-in").await;
        let valid = signed_request(
            &credential,
            "user-a",
            session,
            "check-in",
            location,
            challenge,
        );
        let mut invalid = valid.clone();
        if mutation == "location" {
            invalid.location_claim = "region:changed".into();
        } else {
            invalid.signature = "not-a-signature".into();
        }
        assert!(matches!(
            provider.submit(invalid).await,
            Err(ProviderError::Request(message)) if message == "invalid signature"
        ));
        assert!(provider.submit(valid).await.is_ok());
    }
}

#[tokio::test]
async fn device_registration_enforces_owner_id_and_key_uniqueness() {
    let provider = MockAttendanceProvider::default();
    let keys = KeyStore::new([4; 32]);
    let (first, _) = keys.generate().unwrap();
    let (second, _) = keys.generate().unwrap();
    let session_a = provider.authenticate("user-a", "secret").await.unwrap();
    let session_b = provider.authenticate("user-b", "secret").await.unwrap();

    provider
        .register_device(&session_a, first.identity.clone())
        .await
        .unwrap();
    // Exact retries are idempotent; neither id nor key can be rebound.
    provider
        .register_device(&session_a, first.identity.clone())
        .await
        .unwrap();
    assert!(
        provider
            .register_device(&session_b, first.identity.clone())
            .await
            .is_err()
    );
    assert!(
        provider
            .register_device(
                &session_a,
                DeviceIdentity {
                    id: first.identity.id,
                    public_key: second.identity.public_key.clone(),
                },
            )
            .await
            .is_err()
    );
    assert!(
        provider
            .register_device(
                &session_a,
                DeviceIdentity {
                    id: Uuid::new_v4(),
                    public_key: first.identity.public_key.clone(),
                },
            )
            .await
            .is_err()
    );
    assert!(
        provider
            .request_challenge(&session_b, first.identity.id, "check-in")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn malformed_public_key_is_rejected_at_enrollment() {
    let provider = MockAttendanceProvider::default();
    let session = provider.authenticate("user-a", "secret").await.unwrap();
    assert!(
        provider
            .register_device(
                &session,
                DeviceIdentity {
                    id: Uuid::new_v4(),
                    public_key: vec![1, 2, 3],
                },
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn unregistered_device_cannot_request_a_challenge() {
    let provider = MockAttendanceProvider::default();
    let session = provider.authenticate("user-a", "secret").await.unwrap();
    assert!(matches!(
        provider
            .request_challenge(&session, Uuid::new_v4(), "check-in")
            .await,
        Err(ProviderError::Device)
    ));
}

#[tokio::test]
async fn simultaneous_submissions_have_exactly_one_winner() {
    let provider = MockAttendanceProvider::default();
    let keys = KeyStore::new([5; 32]);
    let (credential, session, challenge, location) =
        issue(&provider, &keys, "user-a", "check-in").await;
    let request = signed_request(
        &credential,
        "user-a",
        session,
        "check-in",
        location,
        challenge,
    );
    let (left, right) = tokio::join!(provider.submit(request.clone()), provider.submit(request));
    assert_eq!(
        [left.is_ok(), right.is_ok()]
            .into_iter()
            .filter(|ok| *ok)
            .count(),
        1
    );
}

#[tokio::test]
async fn bounded_runner_completes_all_jobs() {
    let provider = Arc::new(MockAttendanceProvider::default());
    let results = worker::run_bounded(
        provider,
        KeyStore::new([6; 32]),
        (0..10)
            .map(|n| Job::new(format!("u{n}"), "check-in", "region:demo"))
            .collect(),
        2,
    )
    .await
    .unwrap();
    assert_eq!(results.len(), 10);
    assert!(results.iter().all(Result::is_ok));
}

#[tokio::test]
async fn bounded_runner_rejects_zero_concurrency() {
    let result = worker::run_bounded(
        Arc::new(MockAttendanceProvider::default()),
        KeyStore::new([7; 32]),
        vec![Job::new("user-a", "check-in", "region:demo")],
        0,
    )
    .await;
    assert!(matches!(result, Err(worker::WorkerError::ZeroConcurrency)));
}

#[tokio::test]
async fn bounded_runner_never_exceeds_configured_concurrency() {
    let provider = Arc::new(InstrumentedProvider::new());
    let results = worker::run_bounded(
        provider.clone(),
        KeyStore::new([8; 32]),
        (0..20)
            .map(|n| Job::new(format!("u{n}"), "check-in", "region:demo"))
            .collect(),
        3,
    )
    .await
    .unwrap();
    assert!(results.iter().all(Result::is_ok));
    assert!(provider.max.load(Ordering::SeqCst) <= 3);
    assert_eq!(provider.active.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn provider_failure_is_returned_as_failure() {
    let result = worker::execute(
        &MockAttendanceProvider::default(),
        &KeyStore::new([9; 32]),
        Job::new("user-a", "check-in", "region:demo"),
        "",
    )
    .await;
    assert!(matches!(result, Err(ProviderError::Authentication)));
}

async fn assert_binding_mutation_rejected(mutate: impl FnOnce(&mut ActionRequest)) {
    let provider = MockAttendanceProvider::default();
    let keys = KeyStore::new([10; 32]);
    let (credential, session, challenge, location) =
        issue(&provider, &keys, "user-a", "check-in").await;
    let valid = signed_request(
        &credential,
        "user-a",
        session,
        "check-in",
        location,
        challenge,
    );
    let mut invalid = valid.clone();
    mutate(&mut invalid);
    assert!(matches!(
        provider.submit(invalid).await,
        Err(ProviderError::Challenge(message)) if message == "binding mismatch"
    ));
    assert!(provider.submit(valid).await.is_ok());
}

fn signed_request(
    credential: &DeviceCredential,
    user_id: &str,
    session_id: String,
    action: &str,
    location_claim: String,
    challenge: String,
) -> ActionRequest {
    let mut request = ActionRequest {
        user_id: user_id.into(),
        device_id: credential.identity.id,
        session_id,
        action: action.into(),
        location_claim,
        challenge,
        signature: String::new(),
    };
    request.signature = credential.sign(&action_signature_payload(&request));
    request
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
    sessions: Arc<Mutex<HashSet<String>>>,
}

impl InstrumentedProvider {
    fn new() -> Self {
        Self {
            inner: MockAttendanceProvider::default(),
            active: Arc::new(AtomicUsize::new(0)),
            max: Arc::new(AtomicUsize::new(0)),
            sessions: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    fn finish(&self, session: &str) {
        if self.sessions.lock().unwrap().remove(session) {
            self.active.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

#[async_trait]
impl AttendanceProvider for InstrumentedProvider {
    async fn authenticate(&self, user_id: &str, secret: &str) -> Result<String, ProviderError> {
        let current = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max.fetch_max(current, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let result = self.inner.authenticate(user_id, secret).await;
        match &result {
            Ok(session) => {
                self.sessions.lock().unwrap().insert(session.clone());
            }
            Err(_) => {
                self.active.fetch_sub(1, Ordering::SeqCst);
            }
        }
        result
    }

    async fn register_device(
        &self,
        session: &str,
        identity: DeviceIdentity,
    ) -> Result<(), ProviderError> {
        let result = self.inner.register_device(session, identity).await;
        if result.is_err() {
            self.finish(session);
        }
        result
    }

    async fn request_challenge(
        &self,
        session: &str,
        device_id: Uuid,
        action: &str,
    ) -> Result<String, ProviderError> {
        let result = self
            .inner
            .request_challenge(session, device_id, action)
            .await;
        if result.is_err() {
            self.finish(session);
        }
        result
    }

    async fn submit(
        &self,
        request: ActionRequest,
    ) -> Result<deysis_trust_boundaries::ProviderResult, ProviderError> {
        let session = request.session_id.clone();
        let result = self.inner.submit(request).await;
        self.finish(&session);
        result
    }
}
