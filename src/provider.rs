use async_trait::async_trait;
use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

const SIGNATURE_DOMAIN: &[u8] = b"deysis-trust-boundaries/action/v1\0";

/// Builds the only byte representation accepted by the mock signature protocol.
///
/// Every field is length-prefixed, so separators inside attacker-controlled values
/// cannot produce an ambiguous message. The domain prefix makes the format both
/// versioned and unsuitable for reuse in another signing context.
pub fn action_signature_payload(request: &ActionRequest) -> Vec<u8> {
    let device_id = request.device_id.as_bytes();
    let fields: [&[u8]; 6] = [
        request.challenge.as_bytes(),
        request.user_id.as_bytes(),
        device_id,
        request.session_id.as_bytes(),
        request.action.as_bytes(),
        request.location_claim.as_bytes(),
    ];
    let mut payload = Vec::with_capacity(
        SIGNATURE_DOMAIN.len() + fields.iter().map(|field| 8 + field.len()).sum::<usize>(),
    );
    payload.extend_from_slice(SIGNATURE_DOMAIN);
    for field in fields {
        payload.extend_from_slice(&(field.len() as u64).to_be_bytes());
        payload.extend_from_slice(field);
    }
    payload
}

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

#[cfg(test)]
mod tests {
    use super::{ActionRequest, action_signature_payload};
    use uuid::Uuid;

    fn request(action: &str, location: &str) -> ActionRequest {
        ActionRequest {
            user_id: "user-a".into(),
            device_id: Uuid::nil(),
            session_id: "session-a".into(),
            action: action.into(),
            location_claim: location.into(),
            challenge: "challenge-a".into(),
            signature: String::new(),
        }
    }

    #[test]
    fn length_prefixes_prevent_separator_ambiguity() {
        assert_ne!(
            action_signature_payload(&request("a|b", "c")),
            action_signature_payload(&request("a", "b|c"))
        );
    }

    #[test]
    fn every_request_context_field_changes_the_payload() {
        let base = request("check-in", "region:demo");
        let expected = action_signature_payload(&base);
        let mutations = [
            ActionRequest {
                challenge: "other".into(),
                ..base.clone()
            },
            ActionRequest {
                user_id: "other".into(),
                ..base.clone()
            },
            ActionRequest {
                device_id: Uuid::new_v4(),
                ..base.clone()
            },
            ActionRequest {
                session_id: "other".into(),
                ..base.clone()
            },
            ActionRequest {
                action: "other".into(),
                ..base.clone()
            },
            ActionRequest {
                location_claim: "other".into(),
                ..base
            },
        ];
        assert!(
            mutations
                .iter()
                .all(|request| action_signature_payload(request) != expected)
        );
    }
}
