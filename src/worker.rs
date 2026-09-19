use crate::{
    crypto::KeyStore,
    provider::{ActionRequest, AttendanceProvider, ProviderError, ProviderResult},
    queue::Job,
};
use std::sync::Arc;
use tokio::sync::Semaphore;

pub const DEFAULT_CONCURRENCY: usize = 4;

pub async fn execute<P: AttendanceProvider>(
    provider: &P,
    keys: &KeyStore,
    job: Job,
    identity_secret: &str,
) -> Result<ProviderResult, ProviderError> {
    let (credential, _) = keys
        .generate()
        .map_err(|_| ProviderError::Request("credential setup failed".into()))?;
    let session = provider.authenticate(&job.user_id, identity_secret).await?;
    provider
        .register_device(&session, credential.identity.clone())
        .await?;
    let challenge = provider
        .request_challenge(&session, credential.identity.id, &job.action)
        .await?;
    let message = format!(
        "{}|{}|{}|{}",
        challenge, session, job.action, job.location_claim
    );
    provider
        .submit(ActionRequest {
            user_id: job.user_id,
            device_id: credential.identity.id,
            session_id: session,
            action: job.action,
            location_claim: job.location_claim,
            challenge,
            signature: credential.sign(message.as_bytes()),
        })
        .await
}

pub async fn run_bounded<P: AttendanceProvider + 'static>(
    provider: Arc<P>,
    keys: KeyStore,
    jobs: Vec<Job>,
    concurrency: usize,
) -> Vec<Result<ProviderResult, ProviderError>> {
    let permits = Arc::new(Semaphore::new(concurrency));
    let mut handles = Vec::new();
    for job in jobs {
        let permit = permits.clone().acquire_owned().await.unwrap();
        let p = provider.clone();
        let k = keys.clone();
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            execute(p.as_ref(), &k, job, "demo-secret").await
        }));
    }
    let mut out = Vec::new();
    for h in handles {
        out.push(h.await.expect("worker task panicked"));
    }
    out
}
