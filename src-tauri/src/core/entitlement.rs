use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

const SERVICE_NAME: &str = "YtbDownGUI";
const DEVICE_ID_KEY: &str = "device_id";
const INSTALLATION_ID_KEY: &str = "installation_id";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitlementFile {
    #[serde(default)]
    pub device_id_fallback: Option<String>,
    #[serde(default)]
    pub installation_id_fallback: Option<String>,
    #[serde(default)]
    pub license_id: Option<String>,
    #[serde(default)]
    pub license_email: Option<String>,
    #[serde(default)]
    pub license_key_last4: Option<String>,
    #[serde(default)]
    pub signed_token: Option<String>,
    #[serde(default)]
    pub token_expires_at: Option<String>,
    #[serde(default)]
    pub trial_used_count_cache: Option<u32>,
    #[serde(default)]
    pub trial_remaining_count_cache: Option<u32>,
    #[serde(default)]
    pub emergency_grace_used_for_token: Option<String>,
}

impl Default for EntitlementFile {
    fn default() -> Self {
        Self {
            device_id_fallback: None,
            installation_id_fallback: None,
            license_id: None,
            license_email: None,
            license_key_last4: None,
            signed_token: None,
            token_expires_at: None,
            trial_used_count_cache: None,
            trial_remaining_count_cache: None,
            emergency_grace_used_for_token: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EntitlementStatus {
    pub plan: String,
    pub pro_active: bool,
    pub device_id: String,
    pub installation_id: String,
    pub secure_storage_available: bool,
    pub license_id: Option<String>,
    pub license_email: Option<String>,
    pub license_key_last4: Option<String>,
    pub signed_token: Option<String>,
    pub token_expires_at: Option<String>,
    pub trial_used_count_cache: Option<u32>,
    pub trial_remaining_count_cache: Option<u32>,
    pub emergency_grace_used_for_token: Option<String>,
}

pub struct EntitlementStore {
    path: PathBuf,
    state: Mutex<EntitlementFile>,
}

impl EntitlementStore {
    pub fn load(app_data_dir: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(app_data_dir)?;
        let path = app_data_dir.join("entitlement.json");
        let state = if path.exists() {
            let bytes = std::fs::read(&path)?;
            serde_json::from_slice::<EntitlementFile>(&bytes).unwrap_or_else(|e| {
                eprintln!("[entitlement] parse error: {e}, using defaults");
                EntitlementFile::default()
            })
        } else {
            EntitlementFile::default()
        };
        Ok(Self {
            path,
            state: Mutex::new(state),
        })
    }

    pub fn get_status(&self) -> AppResult<EntitlementStatus> {
        let mut state = self.state.lock().unwrap();
        let device = self.ensure_identity(&mut state, DEVICE_ID_KEY)?;
        let installation = self.ensure_identity(&mut state, INSTALLATION_ID_KEY)?;
        let pro_active = state.signed_token.is_some() && state.token_expires_at.is_some();
        let status = EntitlementStatus {
            plan: if pro_active { "pro".into() } else { "free".into() },
            pro_active,
            device_id: device.id,
            installation_id: installation.id,
            secure_storage_available: device.secure && installation.secure,
            license_id: state.license_id.clone(),
            license_email: state.license_email.clone(),
            license_key_last4: state.license_key_last4.clone(),
            signed_token: state.signed_token.clone(),
            token_expires_at: state.token_expires_at.clone(),
            trial_used_count_cache: state.trial_used_count_cache,
            trial_remaining_count_cache: state.trial_remaining_count_cache,
            emergency_grace_used_for_token: state.emergency_grace_used_for_token.clone(),
        };
        self.persist(&state)?;
        Ok(status)
    }

    fn ensure_identity(
        &self,
        state: &mut EntitlementFile,
        key: &str,
    ) -> AppResult<IdentityValue> {
        if let Ok(Some(id)) = read_keyring(key) {
            match key {
                DEVICE_ID_KEY => state.device_id_fallback = Some(id.clone()),
                INSTALLATION_ID_KEY => state.installation_id_fallback = Some(id.clone()),
                _ => {}
            }
            return Ok(IdentityValue { id, secure: true });
        }

        let fallback = match key {
            DEVICE_ID_KEY => &mut state.device_id_fallback,
            INSTALLATION_ID_KEY => &mut state.installation_id_fallback,
            _ => return Err(AppError::Other(format!("unknown entitlement identity key: {key}"))),
        };

        let id = fallback
            .clone()
            .unwrap_or_else(|| format!("ytb_{}", Uuid::new_v4()));
        *fallback = Some(id.clone());
        let secure = write_keyring(key, &id).is_ok();
        Ok(IdentityValue { id, secure })
    }

    fn persist(&self, state: &EntitlementFile) -> AppResult<()> {
        let bytes = serde_json::to_vec_pretty(state)?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

struct IdentityValue {
    id: String,
    secure: bool,
}

fn read_keyring(key: &str) -> Result<Option<String>, keyring::Error> {
    let entry = keyring::Entry::new(SERVICE_NAME, key)?;
    match entry.get_password() {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) => Ok(None),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error),
    }
}

fn write_keyring(key: &str, value: &str) -> Result<(), keyring::Error> {
    let entry = keyring::Entry::new(SERVICE_NAME, key)?;
    entry.set_password(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_fallback_ids_and_persists_entitlement_file() {
        let temp = tempfile::tempdir().unwrap();
        let store = EntitlementStore::load(temp.path()).unwrap();
        let status = store.get_status().unwrap();

        assert!(status.device_id.starts_with("ytb_"));
        assert!(status.installation_id.starts_with("ytb_"));
        assert_eq!(status.plan, "free");
        assert!(!status.pro_active);
        assert!(temp.path().join("entitlement.json").exists());
    }

    #[test]
    fn reloads_fallback_ids_from_entitlement_file() {
        let temp = tempfile::tempdir().unwrap();
        let first = EntitlementStore::load(temp.path())
            .unwrap()
            .get_status()
            .unwrap();
        let second = EntitlementStore::load(temp.path())
            .unwrap()
            .get_status()
            .unwrap();

        assert_eq!(first.device_id, second.device_id);
        assert_eq!(first.installation_id, second.installation_id);
    }
}
