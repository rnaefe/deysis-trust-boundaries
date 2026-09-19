use async_trait::async_trait;
use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ActionRequest {
    pub user_id: String,
    pub device_id: Uuid,
    pub session_id: String,
    pub action: String,
    pub location_claim: String,
    pub challenge: String,
    pub signature: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderResult {
    pub receipt: String,
    pub accepted_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("authentication failed")]
    Authentication,
    #[error("challenge rejected: {0}")]
    Challenge(String),
    #[error("device rejected")]
    Device,
    #[error("request rejected: {0}")]
    Request(String),
    #[error("provider unavailable")]
    Unavailable,
}

#[async_trait]
pub trait AttendanceProvider: Send + Sync {
    async fn authenticate(&self, user_id: &str, secret: &str) -> Result<String, ProviderError>;
    async fn register_device(
        &self,
        session: &str,
        identity: crate::crypto::DeviceIdentity,
    ) -> Result<(), ProviderError>;
    async fn request_challenge(
        &self,
        session: &str,
        device_id: Uuid,
        action: &str,
    ) -> Result<String, ProviderError>;
    async fn submit(&self, request: ActionRequest) -> Result<ProviderResult, ProviderError>;
}
