use crate::{
    crypto::KeyStore,
    provider::{
        ActionRequest, AttendanceProvider, ProviderError, ProviderResult, action_signature_payload,
    },
    queue::{Job, PgQueue, QueueError, RetryOutcome},
};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Semaphore;

pub const DEFAULT_CONCURRENCY: usize = 4;

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("worker concurrency must be greater than zero")]
    ZeroConcurrency,
    #[error("worker task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error(transparent)]
    Queue(#[from] QueueError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueRunOutcome {
    Idle,
    Completed,
    Requeued,
    FailedPermanently,
}

pub async fn execute<P: AttendanceProvider>(
    provider: &P,
    keys: &KeyStore,
    job: Job,
    identity_secret: &str,
) -> Result<ProviderResult, ProviderError> {
    let (generated, _) = keys
        .generate()
        .map_err(|_| ProviderError::Request("credential setup failed".into()))?;
    let identity = generated.identity.clone();
    drop(generated);
    let session = provider.authenticate(&job.user_id, identity_secret).await?;
    provider.register_device(&session, identity.clone()).await?;
    let challenge = provider
        .request_challenge(&session, identity.id, &job.action)
        .await?;
    let mut request = ActionRequest {
        user_id: job.user_id,
        device_id: identity.id,
        session_id: session,
        action: job.action,
        location_claim: job.location_claim,
        challenge,
        signature: String::new(),
    };
    let credential = keys
        .load(identity.id)
        .map_err(|_| ProviderError::Request("credential lookup failed".into()))?;
    request.signature = credential.sign(&action_signature_payload(&request));
    provider.submit(request).await
}

pub async fn run_bounded<P: AttendanceProvider + 'static>(
    provider: Arc<P>,
    keys: KeyStore,
    jobs: Vec<Job>,
    concurrency: usize,
) -> Result<Vec<Result<ProviderResult, ProviderError>>, WorkerError> {
    if concurrency == 0 {
        return Err(WorkerError::ZeroConcurrency);
    }
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
        out.push(h.await?);
    }
    Ok(out)
}

/// Claims and settles one durable queue item. Provider failures follow the queue's
/// retry/dead-letter policy, while a successful provider action is completed only
/// while this worker still owns the lease.
pub async fn run_queue_once<P: AttendanceProvider>(
    provider: &P,
    keys: &KeyStore,
    queue: &PgQueue,
    identity_secret: &str,
) -> Result<QueueRunOutcome, WorkerError> {
    let Some(claim) = queue.claim_next().await? else {
        return Ok(QueueRunOutcome::Idle);
    };
    match execute(provider, keys, claim.job.clone(), identity_secret).await {
        Ok(_) => {
            queue.complete(&claim).await?;
            Ok(QueueRunOutcome::Completed)
        }
        Err(error) => match queue.retry(&claim, &error.to_string()).await? {
            RetryOutcome::Requeued => Ok(QueueRunOutcome::Requeued),
            RetryOutcome::FailedPermanently => Ok(QueueRunOutcome::FailedPermanently),
        },
    }
}
