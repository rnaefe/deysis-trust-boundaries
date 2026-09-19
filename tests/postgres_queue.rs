use chrono::Duration;
use deysis_trust_boundaries::{
    MockAttendanceProvider,
    crypto::KeyStore,
    queue::{ClaimedJob, Job, PgQueue, QueueError, QueuePolicy, RetryOutcome, is_local_database},
    worker::{self, QueueRunOutcome},
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn local_pool() -> Option<PgPool> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    assert!(
        is_local_database(&url),
        "integration tests refuse non-loopback PostgreSQL hosts"
    );
    Some(
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .expect("connect to TEST_DATABASE_URL"),
    )
}

fn policy(max_attempts: u32) -> QueuePolicy {
    QueuePolicy {
        lease_duration: Duration::milliseconds(100),
        max_attempts,
        base_retry_delay: Duration::zero(),
        max_retry_delay: Duration::zero(),
    }
}

fn forged_claim(claim: &ClaimedJob) -> ClaimedJob {
    ClaimedJob {
        job: claim.job.clone(),
        lease_token: Uuid::new_v4(),
        lease_expires_at: claim.lease_expires_at,
    }
}

#[tokio::test]
async fn postgres_queue_covers_claim_retry_recovery_and_idempotency() {
    let _guard = DB_LOCK.lock().await;
    let Some(pool) = local_pool().await else {
        eprintln!("skipping: TEST_DATABASE_URL is not set");
        return;
    };
    sqlx::query("DROP TABLE IF EXISTS work_items")
        .execute(&pool)
        .await
        .unwrap();
    let queue = PgQueue::with_policy(pool.clone(), policy(2)).unwrap();
    queue.initialize().await.unwrap();

    let job = Job::new("user-a", "check-in", "region:demo");
    assert!(queue.enqueue(&job).await.unwrap());
    assert!(!queue.enqueue(&job).await.unwrap());
    let first = queue.claim_next().await.unwrap().unwrap();
    assert_eq!(first.job.attempts, 1);
    assert!(matches!(
        queue.complete(&forged_claim(&first)).await,
        Err(QueueError::LostLease)
    ));
    assert_eq!(
        queue.retry(&first, "transient").await.unwrap(),
        RetryOutcome::Requeued
    );
    let second = queue.claim_next().await.unwrap().unwrap();
    assert_eq!(second.job.id, job.id);
    assert_eq!(second.job.attempts, 2);
    assert_eq!(
        queue.retry(&second, "still failing").await.unwrap(),
        RetryOutcome::FailedPermanently
    );

    let recoverable = Job::new("user-b", "check-in", "region:demo");
    let recovery_queue = PgQueue::with_policy(pool.clone(), policy(3)).unwrap();
    recovery_queue.enqueue(&recoverable).await.unwrap();
    let abandoned = recovery_queue.claim_next().await.unwrap().unwrap();
    assert_eq!(abandoned.job.id, recoverable.id);
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    assert_eq!(recovery_queue.recover_expired_leases().await.unwrap(), 1);
    let recovered = recovery_queue.claim_next().await.unwrap().unwrap();
    assert_eq!(recovered.job.id, recoverable.id);
    assert_eq!(recovered.job.attempts, 2);
    recovery_queue.complete(&recovered).await.unwrap();

    let first_parallel = Job::new("user-c", "check-in", "region:demo");
    let second_parallel = Job::new("user-d", "check-in", "region:demo");
    queue.enqueue(&first_parallel).await.unwrap();
    queue.enqueue(&second_parallel).await.unwrap();
    let (left, right) = tokio::join!(queue.claim_next(), queue.claim_next());
    let left = left.unwrap().unwrap();
    let right = right.unwrap().unwrap();
    assert_ne!(left.job.id, right.job.id);
    queue.complete(&left).await.unwrap();
    queue.complete(&right).await.unwrap();

    let malformed_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO work_items \
         (id, payload, status, available_at, attempts, max_attempts) \
         VALUES ($1, '{}'::jsonb, 'queued', now(), 0, 2)",
    )
    .bind(malformed_id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        queue.claim_next().await,
        Err(QueueError::MalformedPayload(id)) if id == malformed_id
    ));
    let status: String = sqlx::query_scalar("SELECT status FROM work_items WHERE id = $1")
        .bind(malformed_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "failed");
    assert_eq!(queue.recoverable_count().await.unwrap(), 0);
}

#[tokio::test]
async fn durable_worker_claims_executes_and_settles_a_job() {
    let _guard = DB_LOCK.lock().await;
    let Some(pool) = local_pool().await else {
        eprintln!("skipping: TEST_DATABASE_URL is not set");
        return;
    };
    sqlx::query("DROP TABLE IF EXISTS work_items")
        .execute(&pool)
        .await
        .unwrap();
    let queue = PgQueue::with_policy(pool, policy(2)).unwrap();
    queue.initialize().await.unwrap();
    queue
        .enqueue(&Job::new("user-a", "check-in", "region:demo"))
        .await
        .unwrap();

    assert_eq!(
        worker::run_queue_once(
            &MockAttendanceProvider::default(),
            &KeyStore::new([11; 32]),
            &queue,
            "secret",
        )
        .await
        .unwrap(),
        QueueRunOutcome::Completed
    );
    assert_eq!(queue.recoverable_count().await.unwrap(), 0);
}
