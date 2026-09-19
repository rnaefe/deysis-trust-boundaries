use crate::{
    crypto::{DeviceCredential, DeviceIdentity},
    provider::{
        ActionRequest, AttendanceProvider, ProviderError, ProviderResult, action_signature_payload,
    },
};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct MockAttendanceProvider {
    state: Arc<Mutex<State>>,
    challenge_ttl: Duration,
}
struct State {
    sessions: HashMap<String, String>,
    devices: HashMap<Uuid, RegisteredDevice>,
    device_ids_by_public_key: HashMap<Vec<u8>, Uuid>,
    challenges: HashMap<String, Challenge>,
    consumed: HashSet<String>,
}
struct RegisteredDevice {
    owner_user_id: String,
    identity: DeviceIdentity,
}
#[derive(Clone)]
struct Challenge {
    user_id: String,
    device_id: Uuid,
    session_id: String,
    action: String,
    expires_at: chrono::DateTime<Utc>,
}

impl Default for MockAttendanceProvider {
    fn default() -> Self {
        Self::new(Duration::seconds(30))
    }
}
impl MockAttendanceProvider {
    pub fn new(challenge_ttl: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                sessions: HashMap::new(),
                devices: HashMap::new(),
                device_ids_by_public_key: HashMap::new(),
                challenges: HashMap::new(),
                consumed: HashSet::new(),
            })),
            challenge_ttl,
        }
    }
    pub fn expire_all_challenges(&self) {
        self.state
            .lock()
            .unwrap()
            .challenges
            .values_mut()
            .for_each(|c| c.expires_at = Utc::now() - Duration::seconds(1));
    }
}

#[async_trait]
impl AttendanceProvider for MockAttendanceProvider {
    async fn authenticate(&self, user_id: &str, secret: &str) -> Result<String, ProviderError> {
        if secret.is_empty() {
            return Err(ProviderError::Authentication);
        }
        let session = format!("mock-session-{}", Uuid::new_v4());
        self.state
            .lock()
            .unwrap()
            .sessions
            .insert(session.clone(), user_id.to_owned());
        Ok(session)
    }
    async fn register_device(
        &self,
        session: &str,
        identity: DeviceIdentity,
    ) -> Result<(), ProviderError> {
        DeviceCredential::validate_identity(&identity).map_err(|_| ProviderError::Device)?;
        let mut state = self.state.lock().unwrap();
        let user_id = state
            .sessions
            .get(session)
            .cloned()
            .ok_or(ProviderError::Authentication)?;

        if let Some(existing) = state.devices.get(&identity.id) {
            return if existing.owner_user_id == user_id
                && existing.identity.public_key == identity.public_key
            {
                Ok(())
            } else {
                Err(ProviderError::Device)
            };
        }
        if state
            .device_ids_by_public_key
            .get(&identity.public_key)
            .is_some_and(|existing_id| *existing_id != identity.id)
        {
            return Err(ProviderError::Device);
        }
        state
            .device_ids_by_public_key
            .insert(identity.public_key.clone(), identity.id);
        state.devices.insert(
            identity.id,
            RegisteredDevice {
                owner_user_id: user_id,
                identity,
            },
        );
        Ok(())
    }
    async fn request_challenge(
        &self,
        session: &str,
        device_id: Uuid,
        action: &str,
    ) -> Result<String, ProviderError> {
        let mut state = self.state.lock().unwrap();
        let user_id = state
            .sessions
            .get(session)
            .cloned()
            .ok_or(ProviderError::Authentication)?;
        let device = state.devices.get(&device_id).ok_or(ProviderError::Device)?;
        if device.owner_user_id != user_id {
            return Err(ProviderError::Device);
        }
        let id = Uuid::new_v4().to_string();
        state.challenges.insert(
            id.clone(),
            Challenge {
                user_id,
                device_id,
                session_id: session.to_owned(),
                action: action.to_owned(),
                expires_at: Utc::now() + self.challenge_ttl,
            },
        );
        Ok(id)
    }
    async fn submit(&self, request: ActionRequest) -> Result<ProviderResult, ProviderError> {
        let mut state = self.state.lock().unwrap();
        if state.consumed.contains(&request.challenge) {
            return Err(ProviderError::Challenge(
                "challenge already consumed".into(),
            ));
        }
        let c = state
            .challenges
            .get(&request.challenge)
            .cloned()
            .ok_or_else(|| ProviderError::Challenge("unknown challenge".into()))?;
        if c.expires_at <= Utc::now() {
            return Err(ProviderError::Challenge("challenge expired".into()));
        }
        if c.user_id != request.user_id
            || c.device_id != request.device_id
            || c.session_id != request.session_id
            || c.action != request.action
        {
            return Err(ProviderError::Challenge("binding mismatch".into()));
        }
        if state.sessions.get(&request.session_id) != Some(&request.user_id) {
            return Err(ProviderError::Authentication);
        }
        let device = state
            .devices
            .get(&request.device_id)
            .ok_or(ProviderError::Device)?;
        if device.owner_user_id != request.user_id {
            return Err(ProviderError::Device);
        }
        DeviceCredential::verify(
            &device.identity,
            &action_signature_payload(&request),
            &request.signature,
        )
        .map_err(|_| ProviderError::Request("invalid signature".into()))?;
        // The mutex makes validation and consumption one atomic state transition.
        // Invalid attempts leave the challenge available to its legitimate holder.
        state.challenges.remove(&request.challenge);
        state.consumed.insert(request.challenge);
        Ok(ProviderResult {
            receipt: format!("mock-receipt-{}", Uuid::new_v4()),
            accepted_at: Utc::now(),
        })
    }
}
