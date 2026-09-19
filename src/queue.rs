use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use thiserror::Error;
use url::{Host, Url};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Job {
    pub id: Uuid,
    pub user_id: String,
    pub action: String,
    pub location_claim: String,
    pub attempts: u32,
}

impl Job {
    pub fn new(
        user_id: impl Into<String>,
        action: impl Into<String>,
        location_claim: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id: user_id.into(),
            action: action.into(),
            location_claim: location_claim.into(),
            attempts: 0,
        }
    }

    fn validate(&self) -> Result<(), QueueError> {
        if self.user_id.trim().is_empty() || self.action.trim().is_empty() {
            return Err(QueueError::InvalidJob(
                "user_id and action must not be empty".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct QueuePolicy {
    pub lease_duration: Duration,
    pub max_attempts: u32,
    pub base_retry_delay: Duration,
    pub max_retry_delay: Duration,
}

impl Default for QueuePolicy {
    fn default() -> Self {
        Self {
            lease_duration: Duration::seconds(30),
            max_attempts: 5,
            base_retry_delay: Duration::seconds(1),
            max_retry_delay: Duration::minutes(5),
        }
    }
}

impl QueuePolicy {
    fn validate(&self) -> Result<(), QueueError> {
        if self.lease_duration <= Duration::zero()
            || self.max_attempts == 0
            || self.base_retry_delay < Duration::zero()
            || self.max_retry_delay < self.base_retry_delay
        {
            return Err(QueueError::InvalidPolicy);
        }
        Ok(())
    }

    fn retry_delay(&self, attempt: u32) -> Duration {
        let exponent = attempt.saturating_sub(1).min(20);
        let multiplier = 1_i32.checked_shl(exponent).unwrap_or(i32::MAX);
        self.base_retry_delay
            .checked_mul(multiplier)
            .unwrap_or(self.max_retry_delay)
            .min(self.max_retry_delay)
    }
}

#[derive(Clone, Debug)]
pub struct ClaimedJob {
    pub job: Job,
    pub lease_token: Uuid,
    pub lease_expires_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetryOutcome {
    Requeued,
    FailedPermanently,
}

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("job serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("invalid job: {0}")]
    InvalidJob(String),
    #[error("queue policy requires positive lease/max-attempt values and ordered retry delays")]
    InvalidPolicy,
    #[error("lease is missing, recovered, or owned by another worker")]
    LostLease,
    #[error("queued payload {0} is malformed")]
    MalformedPayload(Uuid),
}

/// PostgreSQL is the durable boundary; secrets and private keys are deliberately absent.
#[derive(Clone)]
pub struct PgQueue {
    pool: PgPool,
    policy: QueuePolicy,
}

impl PgQueue {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            policy: QueuePolicy::default(),
        }
    }

    pub fn with_policy(pool: PgPool, policy: QueuePolicy) -> Result<Self, QueueError> {
        policy.validate()?;
        Ok(Self { pool, policy })
    }

    pub async fn initialize(&self) -> Result<(), QueueError> {
        sqlx::raw_sql(schema()).execute(&self.pool).await?;
        Ok(())
    }

    /// Idempotently persists a job. `false` means the same job id already existed.
    pub async fn enqueue(&self, job: &Job) -> Result<bool, QueueError> {
        job.validate()?;
        let result = sqlx::query(
            "INSERT INTO work_items \
             (id, payload, status, available_at, attempts, max_attempts) \
             VALUES ($1, $2, 'queued', now(), 0, $3) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(job.id)
        .bind(serde_json::to_value(job)?)
        .bind(i32::try_from(self.policy.max_attempts).map_err(|_| QueueError::InvalidPolicy)?)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Claims one ready row without blocking other workers that use the same query.
    pub async fn claim_next(&self) -> Result<Option<ClaimedJob>, QueueError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT id, payload, attempts \
             FROM work_items \
             WHERE status = 'queued' AND available_at <= now() AND attempts < max_attempts \
             ORDER BY available_at, id \
             FOR UPDATE SKIP LOCKED \
             LIMIT 1",
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };

        let id: Uuid = row.try_get("id")?;
        let value: serde_json::Value = row.try_get("payload")?;
        let previous_attempts: i32 = row.try_get("attempts")?;
        let mut job: Job = match serde_json::from_value(value) {
            Ok(job) => job,
            Err(_) => {
                sqlx::query(
                    "UPDATE work_items \
                     SET status = 'failed', last_error = 'malformed queue payload' \
                     WHERE id = $1",
                )
                .bind(id)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
                return Err(QueueError::MalformedPayload(id));
            }
        };
        if job.id != id {
            sqlx::query(
                "UPDATE work_items \
                 SET status = 'failed', last_error = 'payload id does not match row id' \
                 WHERE id = $1",
            )
            .bind(id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Err(QueueError::MalformedPayload(id));
        }
        if let Err(error) = job.validate() {
            sqlx::query("UPDATE work_items SET status = 'failed', last_error = $2 WHERE id = $1")
                .bind(id)
                .bind(error.to_string())
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            return Err(error);
        }

        let attempts = previous_attempts.saturating_add(1);
        job.attempts = u32::try_from(attempts).unwrap_or(u32::MAX);
        let lease_token = Uuid::new_v4();
        let lease_expires_at = Utc::now() + self.policy.lease_duration;
        sqlx::query(
            "UPDATE work_items \
             SET status = 'running', attempts = $2, claimed_at = now(), \
                 lease_token = $3, lease_expires_at = $4, last_error = NULL \
             WHERE id = $1",
        )
        .bind(id)
        .bind(attempts)
        .bind(lease_token)
        .bind(lease_expires_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Some(ClaimedJob {
            job,
            lease_token,
            lease_expires_at,
        }))
    }

    pub async fn renew_lease(&self, claim: &ClaimedJob) -> Result<DateTime<Utc>, QueueError> {
        let lease_expires_at = Utc::now() + self.policy.lease_duration;
        let result = sqlx::query(
            "UPDATE work_items SET lease_expires_at = $3 \
             WHERE id = $1 AND status = 'running' AND lease_token = $2",
        )
        .bind(claim.job.id)
        .bind(claim.lease_token)
        .bind(lease_expires_at)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            return Err(QueueError::LostLease);
        }
        Ok(lease_expires_at)
    }

    pub async fn complete(&self, claim: &ClaimedJob) -> Result<(), QueueError> {
        let result = sqlx::query(
            "UPDATE work_items \
             SET status = 'complete', lease_token = NULL, lease_expires_at = NULL \
             WHERE id = $1 AND status = 'running' AND lease_token = $2",
        )
        .bind(claim.job.id)
        .bind(claim.lease_token)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            return Err(QueueError::LostLease);
        }
        Ok(())
    }

    pub async fn retry(&self, claim: &ClaimedJob, error: &str) -> Result<RetryOutcome, QueueError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT attempts, max_attempts FROM work_items \
             WHERE id = $1 AND status = 'running' AND lease_token = $2 \
             FOR UPDATE",
        )
        .bind(claim.job.id)
        .bind(claim.lease_token)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(QueueError::LostLease)?;
        let attempts: i32 = row.try_get("attempts")?;
        let max_attempts: i32 = row.try_get("max_attempts")?;
        let outcome = if attempts >= max_attempts {
            sqlx::query(
                "UPDATE work_items \
                 SET status = 'failed', last_error = $2, lease_token = NULL, lease_expires_at = NULL \
                 WHERE id = $1",
            )
            .bind(claim.job.id)
            .bind(error)
            .execute(&mut *tx)
            .await?;
            RetryOutcome::FailedPermanently
        } else {
            let delay = self
                .policy
                .retry_delay(u32::try_from(attempts).unwrap_or(u32::MAX));
            sqlx::query(
                "UPDATE work_items \
                 SET status = 'queued', available_at = $2, last_error = $3, \
                     claimed_at = NULL, lease_token = NULL, lease_expires_at = NULL \
                 WHERE id = $1",
            )
            .bind(claim.job.id)
            .bind(Utc::now() + delay)
            .bind(error)
            .execute(&mut *tx)
            .await?;
            RetryOutcome::Requeued
        };
        tx.commit().await?;
        Ok(outcome)
    }

    /// Makes work from crashed workers available again, or dead-letters exhausted rows.
    pub async fn recover_expired_leases(&self) -> Result<u64, QueueError> {
        let result = sqlx::query(
            "UPDATE work_items \
             SET status = CASE WHEN attempts >= max_attempts THEN 'failed' ELSE 'queued' END, \
                 available_at = CASE WHEN attempts >= max_attempts THEN available_at ELSE now() END, \
                 last_error = CASE WHEN attempts >= max_attempts \
                                   THEN 'maximum attempts reached after lease expiry' \
                                   ELSE 'worker lease expired' END, \
                 claimed_at = NULL, lease_token = NULL, lease_expires_at = NULL \
             WHERE status = 'running' AND lease_expires_at <= now()",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn recoverable_count(&self) -> Result<i64, QueueError> {
        Ok(sqlx::query_scalar(
            "SELECT count(*) FROM work_items WHERE status IN ('queued','running')",
        )
        .fetch_one(&self.pool)
        .await?)
    }
}

pub fn schema() -> &'static str {
    r#"
CREATE TABLE IF NOT EXISTS work_items (
  id uuid PRIMARY KEY,
  payload jsonb NOT NULL,
  status text NOT NULL CHECK (status IN ('queued', 'running', 'complete', 'failed')),
  available_at timestamptz NOT NULL,
  attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
  max_attempts integer NOT NULL DEFAULT 5 CHECK (max_attempts > 0),
  last_error text,
  claimed_at timestamptz,
  lease_token uuid,
  lease_expires_at timestamptz,
  CONSTRAINT running_lease_consistency CHECK (
    (status = 'running' AND lease_token IS NOT NULL AND lease_expires_at IS NOT NULL)
    OR (status <> 'running' AND lease_token IS NULL AND lease_expires_at IS NULL)
  )
);
ALTER TABLE work_items ADD COLUMN IF NOT EXISTS max_attempts integer NOT NULL DEFAULT 5;
ALTER TABLE work_items ADD COLUMN IF NOT EXISTS lease_token uuid;
ALTER TABLE work_items ADD COLUMN IF NOT EXISTS lease_expires_at timestamptz;
UPDATE work_items
  SET status = 'queued', claimed_at = NULL
  WHERE status = 'running' AND (lease_token IS NULL OR lease_expires_at IS NULL);
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conrelid = 'work_items'::regclass AND conname = 'running_lease_consistency'
  ) THEN
    ALTER TABLE work_items ADD CONSTRAINT running_lease_consistency CHECK (
      (status = 'running' AND lease_token IS NOT NULL AND lease_expires_at IS NOT NULL)
      OR (status <> 'running' AND lease_token IS NULL AND lease_expires_at IS NULL)
    );
  END IF;
END $$;
CREATE INDEX IF NOT EXISTS work_items_ready_idx
  ON work_items (available_at, id) WHERE status = 'queued';
CREATE INDEX IF NOT EXISTS work_items_expired_lease_idx
  ON work_items (lease_expires_at) WHERE status = 'running';
"#
}

pub fn is_local_database(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    if !matches!(parsed.scheme(), "postgres" | "postgresql") {
        return false;
    }
    match parsed.host() {
        Some(Host::Domain(host)) => {
            host == "localhost"
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|address| address.is_loopback())
        }
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jobs_do_not_contain_secrets() {
        let value = serde_json::to_value(Job::new("user-a", "check-in", "coarse-region")).unwrap();
        let object = value.as_object().unwrap();
        let keys = object
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            keys,
            ["action", "attempts", "id", "location_claim", "user_id"]
                .into_iter()
                .collect()
        );
        let json = value.to_string();
        assert!(!json.contains("password"));
        assert!(!json.contains("private_key"));
        assert!(!json.contains("bearer"));
    }

    #[test]
    fn database_guard_allows_only_exact_loopback_hosts() {
        assert!(is_local_database("postgres://u:p@localhost:5432/db"));
        assert!(is_local_database("postgresql://u:p@127.0.0.1:5432/db"));
        assert!(is_local_database("postgres://u:p@[::1]:5432/db"));
        assert!(!is_local_database(
            "postgres://u:p@localhost.attacker.example/db"
        ));
        assert!(!is_local_database(
            "postgres://u:p@127.0.0.1.attacker.example/db"
        ));
        assert!(!is_local_database("https://localhost/db"));
        assert!(!is_local_database("not-a-url"));
    }

    #[test]
    fn retry_backoff_is_exponential_and_capped() {
        let policy = QueuePolicy::default();
        assert_eq!(policy.retry_delay(1), Duration::seconds(1));
        assert_eq!(policy.retry_delay(3), Duration::seconds(4));
        assert_eq!(policy.retry_delay(30), Duration::minutes(5));
    }
}
