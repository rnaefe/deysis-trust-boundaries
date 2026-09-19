use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, OsRng, rand_core::RngCore},
};
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use p256::ecdsa::{
    Signature, SigningKey, VerifyingKey,
    signature::{Signer, Verifier},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceIdentity {
    pub id: Uuid,
    pub public_key: Vec<u8>,
}

pub struct DeviceCredential {
    pub identity: DeviceIdentity,
    signing_key: SigningKey,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedDeviceKey {
    pub identity: DeviceIdentity,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
}

#[derive(Clone)]
pub struct KeyStore {
    cipher: Aes256Gcm,
}

impl KeyStore {
    pub fn new(key: [u8; 32]) -> Self {
        Self {
            cipher: Aes256Gcm::new_from_slice(&key).expect("valid AES key"),
        }
    }
    pub fn generate(&self) -> Result<(DeviceCredential, EncryptedDeviceKey)> {
        let signing_key = SigningKey::random(&mut OsRng);
        let identity = DeviceIdentity {
            id: Uuid::new_v4(),
            public_key: VerifyingKey::from(&signing_key)
                .to_encoded_point(false)
                .as_bytes()
                .to_vec(),
        };
        let credential = DeviceCredential {
            identity: identity.clone(),
            signing_key,
        };
        Ok((credential.clone_for_test(), self.encrypt(&credential)?))
    }
    #[allow(deprecated)]
    pub fn encrypt(&self, credential: &DeviceCredential) -> Result<EncryptedDeviceKey> {
        let mut nonce = [0_u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let private_key = credential.signing_key.to_bytes().to_vec();
        let ciphertext = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), private_key.as_slice())
            .map_err(|_| anyhow::anyhow!("key encryption failed"))?;
        Ok(EncryptedDeviceKey {
            identity: credential.identity.clone(),
            ciphertext,
            nonce: nonce.to_vec(),
        })
    }
    #[allow(deprecated)]
    pub fn decrypt(&self, stored: &EncryptedDeviceKey) -> Result<DeviceCredential> {
        if stored.nonce.len() != 12 {
            bail!("invalid AES-GCM nonce length")
        }
        let bytes = self
            .cipher
            .decrypt(
                Nonce::from_slice(&stored.nonce),
                stored.ciphertext.as_slice(),
            )
            .map_err(|_| anyhow::anyhow!("key decryption failed"))?;
        let signing_key = SigningKey::from_slice(&bytes).context("invalid P-256 private key")?;
        let actual = VerifyingKey::from(&signing_key)
            .to_encoded_point(false)
            .as_bytes()
            .to_vec();
        if actual != stored.identity.public_key {
            bail!("stored public key does not match private key")
        }
        Ok(DeviceCredential {
            identity: stored.identity.clone(),
            signing_key,
        })
    }
}

impl DeviceCredential {
    pub fn sign(&self, message: &[u8]) -> String {
        let signature: Signature = self.signing_key.sign(message);
        URL_SAFE_NO_PAD.encode(signature.to_bytes())
    }
    pub fn verify(identity: &DeviceIdentity, message: &[u8], encoded: &str) -> Result<()> {
        let key =
            VerifyingKey::from_sec1_bytes(&identity.public_key).context("invalid public key")?;
        let sig = Signature::from_slice(
            &URL_SAFE_NO_PAD
                .decode(encoded)
                .context("invalid signature encoding")?,
        )
        .context("invalid signature")?;
        key.verify(message, &sig)
            .context("signature verification failed")
    }
    fn clone_for_test(&self) -> Self {
        Self {
            identity: self.identity.clone(),
            signing_key: self.signing_key.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::KeyStore;

    #[test]
    fn encrypted_private_key_round_trips_and_tampering_fails() {
        let store = KeyStore::new([9; 32]);
        let (credential, encrypted) = store.generate().unwrap();
        let restored = store.decrypt(&encrypted).unwrap();
        assert_eq!(credential.identity.id, restored.identity.id);
        assert_eq!(
            credential.sign(b"independent-message"),
            restored.sign(b"independent-message")
        );
        let mut tampered = encrypted.clone();
        tampered.ciphertext[0] ^= 1;
        assert!(store.decrypt(&tampered).is_err());
    }
}
