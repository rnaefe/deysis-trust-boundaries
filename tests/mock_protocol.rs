use chrono::Duration;
use deysis_trust_boundaries::{
    crypto::KeyStore,
    mock_provider::MockAttendanceProvider,
    provider::{ActionRequest, AttendanceProvider},
    queue::Job,
    worker,
};
use std::sync::Arc;

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
