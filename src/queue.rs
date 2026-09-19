use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
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
}

/// PostgreSQL is the durable boundary; secrets and private keys are deliberately absent.
#[derive(Clone)]
pub struct PgQueue {
    pool: PgPool,
}
impl PgQueue {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub async fn enqueue(&self, job: &Job) -> Result<()> {
        sqlx::query("INSERT INTO work_items (id, payload, status, available_at) VALUES ($1, $2, 'queued', now())").bind(job.id).bind(serde_json::to_value(job)?).execute(&self.pool).await?;
        Ok(())
    }
    pub async fn recoverable_count(&self) -> Result<i64> {
        Ok(sqlx::query_scalar(
            "SELECT count(*) FROM work_items WHERE status IN ('queued','running')",
        )
        .fetch_one(&self.pool)
        .await?)
    }
}

pub fn schema() -> &'static str {
    "CREATE TABLE IF NOT EXISTS work_items (id uuid PRIMARY KEY, payload jsonb NOT NULL, status text NOT NULL, available_at timestamptz NOT NULL, attempts integer NOT NULL DEFAULT 0, last_error text, claimed_at timestamptz)"
}
pub fn is_local_database(url: &str) -> bool {
    url.split("@")
        .last()
        .and_then(|v| v.split('/').next())
        .map(|host| {
            host.starts_with("localhost")
                || host.starts_with("127.0.0.1")
                || host.starts_with("[::1]")
        })
        .unwrap_or(false)
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
    fn database_guard_allows_only_loopback() {
        assert!(is_local_database("postgres://u:p@localhost:5432/db"));
        assert!(is_local_database("postgres://u:p@127.0.0.1:5432/db"));
        assert!(!is_local_database("postgres://u:p@db.example/db"));
    }
}
