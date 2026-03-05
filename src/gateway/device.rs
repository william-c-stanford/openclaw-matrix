use base64::Engine;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct StoredIdentity {
    device_id: String,
    secret_key: String,
}

/// Ed25519 device identity for gateway authentication
pub struct DeviceIdentity {
    pub device_id: String,
    signing_key: SigningKey,
}

impl DeviceIdentity {
    /// Load existing identity or generate a new one
    pub fn load_or_create() -> std::io::Result<Self> {
        let path = identity_path();

        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let stored: StoredIdentity = serde_json::from_str(&content)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            let key_bytes = base64::engine::general_purpose::STANDARD
                .decode(&stored.secret_key)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            let key_array: [u8; 32] = key_bytes
                .try_into()
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "bad key length"))?;

            let signing_key = SigningKey::from_bytes(&key_array);

            Ok(Self {
                device_id: stored.device_id,
                signing_key,
            })
        } else {
            Self::generate()
        }
    }

    fn generate() -> std::io::Result<Self> {
        let secret_bytes: [u8; 32] = rand::random();
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key: VerifyingKey = signing_key.verifying_key();

        let device_id = format!(
            "matrix-{}",
            &hex::encode(verifying_key.as_bytes())[..16]
        );

        let stored = StoredIdentity {
            device_id: device_id.clone(),
            secret_key: base64::engine::general_purpose::STANDARD.encode(secret_bytes),
        };

        let path = identity_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(&stored).unwrap())?;

        Ok(Self {
            device_id,
            signing_key,
        })
    }

    /// Sign a challenge string, return base64 signature
    pub fn sign(&self, challenge: &str) -> String {
        let signature = self.signing_key.sign(challenge.as_bytes());
        base64::engine::general_purpose::STANDARD.encode(signature.to_bytes())
    }
}

fn identity_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openclaw")
        .join("identity")
        .join("device-matrix.json")
}
