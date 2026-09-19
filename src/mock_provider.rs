use crate::{
    crypto::{DeviceCredential, DeviceIdentity},
    provider::{ActionRequest, AttendanceProvider, ProviderError, ProviderResult},
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
    devices: HashMap<Uuid, DeviceIdentity>,
    challenges: HashMap<String, Challenge>,
    consumed: HashSet<String>,
}
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
        if !self.state.lock().unwrap().sessions.contains_key(session) {
            return Err(ProviderError::Authentication);
        }
        self.state
            .lock()
            .unwrap()
            .devices
            .insert(identity.id, identity);
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
        if !state.devices.contains_key(&device_id) {
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
            .remove(&request.challenge)
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
        let identity = state
            .devices
            .get(&request.device_id)
            .ok_or(ProviderError::Device)?;
        let message = format!(
            "{}|{}|{}|{}",
            request.challenge, request.session_id, request.action, request.location_claim
        );
        DeviceCredential::verify(identity, message.as_bytes(), &request.signature)
            .map_err(|_| ProviderError::Request("invalid signature".into()))?;
        state.consumed.insert(request.challenge);
        Ok(ProviderResult {
            receipt: format!("mock-receipt-{}", Uuid::new_v4()),
            accepted_at: Utc::now(),
        })
    }
}
