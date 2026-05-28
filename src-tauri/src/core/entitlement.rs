use crate::error::{AppError, AppResult};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use reqwest::StatusCode;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const SERVICE_NAME: &str = "YtbDownGUI";
const DEVICE_ID_KEY: &str = "device_id";
const INSTALLATION_ID_KEY: &str = "installation_id";
const TOKEN_ISSUER: &str = "ytbdown-license-server";
const TOKEN_AUDIENCE: &str = "ytbdown-client";
const EMERGENCY_GRACE_SECONDS: u64 = 24 * 60 * 60;

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
    pub token_validation_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum ActivateProResult {
    #[serde(rename = "activated")]
    Activated { status: EntitlementStatus },
    #[serde(rename = "transfer_code_required")]
    TransferCodeRequired {
        email_hint: String,
        active_device_count: u32,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferCodeStatus {
    pub sent: bool,
    pub email_hint: String,
    pub expires_at: String,
    pub dev_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSession {
    pub checkout_session_id: String,
    pub checkout_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResendLicenseResponse {
    pub accepted: bool,
    pub email_hint: String,
    #[serde(default)]
    pub dev_license_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportContact {
    pub support_email: String,
    pub privacy_url: String,
    pub terms_url: String,
    pub support_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeQuotaStatus {
    pub installation_id: String,
    pub quota_limit: u32,
    pub used_count: u32,
    pub reserved_count: u32,
    pub remaining_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeQuotaReservation {
    pub installation_id: String,
    pub quota_limit: u32,
    pub used_count: u32,
    pub reserved_count: u32,
    pub remaining_count: u32,
    pub reservation_id: Option<String>,
    pub reservation_status: Option<String>,
    pub reservation_count: Option<u32>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct EntitlementClaims {
    license_id: String,
    device_id: String,
    plan: String,
    exp: u64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
enum ServerActivateResponse {
    #[serde(rename = "activated")]
    Activated { status: ServerLicenseStatus },
    #[serde(rename = "transfer_code_required")]
    TransferCodeRequired {
        email_hint: String,
        active_device_count: u32,
    },
}

#[derive(Debug, Deserialize)]
struct ServerLicenseStatus {
    token: String,
    token_expires_at: String,
    license_email: String,
    license_key_last4: String,
}

#[derive(Debug, Serialize)]
struct ActivateRequest<'a> {
    license_key: &'a str,
    device_id: &'a str,
    device_name: &'a str,
    platform: &'a str,
    app_version: &'a str,
}

#[derive(Debug, Serialize)]
struct RefreshRequest<'a> {
    token: &'a str,
    device_id: &'a str,
    app_version: &'a str,
}

#[derive(Debug, Serialize)]
struct DeactivateRequest<'a> {
    token: &'a str,
    device_id: &'a str,
}

#[derive(Debug, Serialize)]
struct TransferCodeRequest<'a> {
    license_key: &'a str,
    device_id: &'a str,
    device_name: &'a str,
    platform: &'a str,
    app_version: &'a str,
}

#[derive(Debug, Serialize)]
struct ActivateWithTransferCodeRequest<'a> {
    license_key: &'a str,
    transfer_code: &'a str,
    device_id: &'a str,
    device_name: &'a str,
    platform: &'a str,
    app_version: &'a str,
}

#[derive(Debug, Serialize)]
struct PurchaseEmailRequest<'a> {
    purchase_email: &'a str,
}

#[derive(Debug, Serialize)]
struct FreeQuotaStatusRequest<'a> {
    installation_id: &'a str,
}

#[derive(Debug, Serialize)]
struct FreeQuotaReserveRequest<'a> {
    installation_id: &'a str,
    count: u32,
}

#[derive(Debug, Serialize)]
struct FreeQuotaReservationRequest<'a> {
    reservation_id: &'a str,
}

#[derive(Debug, Deserialize)]
struct ServerTransferCodeResponse {
    sent: bool,
    email_hint: String,
    expires_at: String,
    dev_code: Option<String>,
}

pub struct EntitlementStore {
    path: PathBuf,
    state: Mutex<EntitlementFile>,
    public_key: String,
    license_server_url: String,
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
            public_key: normalize_public_key(env!("LICENSE_PUBLIC_KEY")),
            license_server_url: env!("LICENSE_SERVER_URL").trim_end_matches('/').to_string(),
        })
    }

    #[cfg(test)]
    fn load_with_config(
        app_data_dir: &Path,
        public_key: &str,
        license_server_url: &str,
    ) -> AppResult<Self> {
        let mut store = Self::load(app_data_dir)?;
        store.public_key = normalize_public_key(public_key);
        store.license_server_url = license_server_url.trim_end_matches('/').to_string();
        Ok(store)
    }

    pub fn get_status(&self) -> AppResult<EntitlementStatus> {
        let mut state = self.state.lock().unwrap();
        let device = self.ensure_identity(&mut state, DEVICE_ID_KEY)?;
        let installation = self.ensure_identity(&mut state, INSTALLATION_ID_KEY)?;
        let evaluation = evaluate_token_for_device(
            state.signed_token.as_deref(),
            &device.id,
            &self.public_key,
            state.emergency_grace_used_for_token.as_deref(),
        );
        let pro_active = evaluation
            .claims
            .as_ref()
            .map(|claims| claims.plan == "pro")
            .unwrap_or(false);
        let status = EntitlementStatus {
            plan: if pro_active {
                "pro".into()
            } else {
                "free".into()
            },
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
            token_validation_error: evaluation.error,
        };
        self.persist(&state)?;
        Ok(status)
    }

    pub async fn activate_pro(&self, license_key: String) -> AppResult<ActivateProResult> {
        let (device_id, device_name, platform, app_version) = self.device_context()?;
        let request = ActivateRequest {
            license_key: license_key.trim(),
            device_id: &device_id,
            device_name: &device_name,
            platform: &platform,
            app_version: &app_version,
        };
        let response = self
            .client()?
            .post(self.endpoint("/v1/licenses/activate"))
            .json(&request)
            .send()
            .await
            .map_err(http_error)?
            .error_for_status()
            .map_err(status_error)?
            .json::<ServerActivateResponse>()
            .await
            .map_err(http_error)?;

        match response {
            ServerActivateResponse::Activated { status } => {
                self.apply_server_status(status, &device_id)?;
                Ok(ActivateProResult::Activated {
                    status: self.get_status()?,
                })
            }
            ServerActivateResponse::TransferCodeRequired {
                email_hint,
                active_device_count,
            } => Ok(ActivateProResult::TransferCodeRequired {
                email_hint,
                active_device_count,
            }),
        }
    }

    pub async fn refresh_pro(&self) -> AppResult<EntitlementStatus> {
        let (device_id, _, _, app_version) = self.device_context()?;
        let token = {
            let state = self.state.lock().unwrap();
            state
                .signed_token
                .clone()
                .ok_or_else(|| AppError::Other("No Pro token is stored".into()))?
        };
        let request = RefreshRequest {
            token: &token,
            device_id: &device_id,
            app_version: &app_version,
        };
        let refresh_result = match self
            .client()?
            .post(self.endpoint("/v1/licenses/refresh"))
            .json(&request)
            .send()
            .await
        {
            Ok(response) => response
                .error_for_status()
                .map_err(status_error)?
                .json::<ServerLicenseStatus>()
                .await
                .map_err(http_error),
            Err(error) => Err(http_error(error)),
        };
        let status = match refresh_result {
            Ok(status) => status,
            Err(error) => {
                self.try_emergency_grace(&device_id, &token)?;
                if self.get_status()?.pro_active {
                    return self.get_status();
                }
                return Err(error);
            }
        };
        self.apply_server_status(status, &device_id)?;
        self.get_status()
    }

    pub async fn deactivate_pro(&self) -> AppResult<EntitlementStatus> {
        let (device_id, _, _, _) = self.device_context()?;
        let token = {
            let state = self.state.lock().unwrap();
            state.signed_token.clone()
        };

        if let Some(token) = token {
            let request = DeactivateRequest {
                token: &token,
                device_id: &device_id,
            };
            let response = self
                .client()?
                .post(self.endpoint("/v1/licenses/deactivate"))
                .json(&request)
                .send()
                .await
                .map_err(http_error)?;
            if response.status() != StatusCode::NOT_FOUND {
                response.error_for_status().map_err(status_error)?;
            }
        }

        {
            let mut state = self.state.lock().unwrap();
            clear_pro_state(&mut state);
            self.persist(&state)?;
        }
        self.get_status()
    }

    pub async fn send_transfer_code(&self, license_key: String) -> AppResult<TransferCodeStatus> {
        let (device_id, device_name, platform, app_version) = self.device_context()?;
        let request = TransferCodeRequest {
            license_key: license_key.trim(),
            device_id: &device_id,
            device_name: &device_name,
            platform: &platform,
            app_version: &app_version,
        };
        let response = self
            .client()?
            .post(self.endpoint("/v1/licenses/send-transfer-code"))
            .json(&request)
            .send()
            .await
            .map_err(http_error)?
            .error_for_status()
            .map_err(status_error)?
            .json::<ServerTransferCodeResponse>()
            .await
            .map_err(http_error)?;
        Ok(TransferCodeStatus {
            sent: response.sent,
            email_hint: response.email_hint,
            expires_at: response.expires_at,
            dev_code: response.dev_code,
        })
    }

    pub async fn activate_with_transfer_code(
        &self,
        license_key: String,
        transfer_code: String,
    ) -> AppResult<EntitlementStatus> {
        let (device_id, device_name, platform, app_version) = self.device_context()?;
        let request = ActivateWithTransferCodeRequest {
            license_key: license_key.trim(),
            transfer_code: transfer_code.trim(),
            device_id: &device_id,
            device_name: &device_name,
            platform: &platform,
            app_version: &app_version,
        };
        let response = self
            .client()?
            .post(self.endpoint("/v1/licenses/activate-with-transfer-code"))
            .json(&request)
            .send()
            .await
            .map_err(http_error)?
            .error_for_status()
            .map_err(status_error)?
            .json::<ServerActivateResponse>()
            .await
            .map_err(http_error)?;

        match response {
            ServerActivateResponse::Activated { status } => {
                self.apply_server_status(status, &device_id)?;
                self.get_status()
            }
            ServerActivateResponse::TransferCodeRequired { .. } => {
                Err(AppError::Other("Transfer code is still required".into()))
            }
        }
    }

    pub async fn create_checkout_session(
        &self,
        purchase_email: String,
    ) -> AppResult<CheckoutSession> {
        let request = PurchaseEmailRequest {
            purchase_email: purchase_email.trim(),
        };
        let response = self
            .client()?
            .post(self.endpoint("/v1/billing/create-checkout-session"))
            .json(&request)
            .send()
            .await
            .map_err(http_error)?;
        json_or_status::<CheckoutSession>(response).await
    }

    pub async fn resend_license(&self, purchase_email: String) -> AppResult<ResendLicenseResponse> {
        let request = PurchaseEmailRequest {
            purchase_email: purchase_email.trim(),
        };
        let response = self
            .client()?
            .post(self.endpoint("/v1/licenses/resend"))
            .json(&request)
            .send()
            .await
            .map_err(http_error)?;
        json_or_status::<ResendLicenseResponse>(response).await
    }

    pub async fn support_contact(&self) -> AppResult<SupportContact> {
        let response = self
            .client()?
            .get(self.endpoint("/v1/support/contact"))
            .send()
            .await
            .map_err(http_error)?;
        json_or_status::<SupportContact>(response).await
    }

    pub async fn sync_free_quota_status(&self) -> AppResult<FreeQuotaStatus> {
        let status = self.get_status()?;
        let request = FreeQuotaStatusRequest {
            installation_id: &status.installation_id,
        };
        let quota = self
            .client()?
            .post(self.endpoint("/v1/free-quota/status"))
            .json(&request)
            .send()
            .await
            .map_err(http_error)?;
        let quota = json_or_status::<FreeQuotaStatus>(quota).await?;
        self.apply_free_quota_cache(&quota)?;
        Ok(quota)
    }

    pub async fn reserve_free_quota(&self, count: u32) -> AppResult<FreeQuotaReservation> {
        let status = self.get_status()?;
        let request = FreeQuotaReserveRequest {
            installation_id: &status.installation_id,
            count,
        };
        let quota = self
            .client()?
            .post(self.endpoint("/v1/free-quota/reserve"))
            .json(&request)
            .send()
            .await
            .map_err(http_error)?;
        let quota = json_or_status::<FreeQuotaReservation>(quota).await?;
        self.apply_free_quota_reservation_cache(&quota)?;
        Ok(quota)
    }

    pub async fn confirm_free_quota(
        &self,
        reservation_id: String,
    ) -> AppResult<FreeQuotaReservation> {
        self.complete_free_quota_reservation("/v1/free-quota/confirm", reservation_id)
            .await
    }

    pub async fn release_free_quota(
        &self,
        reservation_id: String,
    ) -> AppResult<FreeQuotaReservation> {
        self.complete_free_quota_reservation("/v1/free-quota/release", reservation_id)
            .await
    }

    fn ensure_identity(&self, state: &mut EntitlementFile, key: &str) -> AppResult<IdentityValue> {
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
            _ => {
                return Err(AppError::Other(format!(
                    "unknown entitlement identity key: {key}"
                )))
            }
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

    fn apply_server_status(&self, status: ServerLicenseStatus, device_id: &str) -> AppResult<()> {
        let claims = validate_token_for_device(&status.token, device_id, &self.public_key, true)
            .map_err(AppError::Other)?;
        if claims.license_id.trim().is_empty() {
            return Err(AppError::Other("Token license id is missing".into()));
        }
        let mut state = self.state.lock().unwrap();
        state.license_id = Some(claims.license_id);
        state.license_email = Some(status.license_email);
        state.license_key_last4 = Some(status.license_key_last4);
        state.signed_token = Some(status.token);
        state.token_expires_at = Some(status.token_expires_at);
        state.emergency_grace_used_for_token = None;
        self.persist(&state)
    }

    fn apply_free_quota_cache(&self, quota: &FreeQuotaStatus) -> AppResult<()> {
        let mut state = self.state.lock().unwrap();
        state.trial_used_count_cache = Some(quota.used_count);
        state.trial_remaining_count_cache = Some(quota.remaining_count);
        self.persist(&state)
    }

    fn apply_free_quota_reservation_cache(&self, quota: &FreeQuotaReservation) -> AppResult<()> {
        let mut state = self.state.lock().unwrap();
        state.trial_used_count_cache = Some(quota.used_count);
        state.trial_remaining_count_cache = Some(quota.remaining_count);
        self.persist(&state)
    }

    fn try_emergency_grace(&self, device_id: &str, token: &str) -> AppResult<()> {
        let claims = validate_token_for_device(token, device_id, &self.public_key, false)
            .map_err(AppError::Other)?;
        if claims.exp > now_seconds() || claims.exp + EMERGENCY_GRACE_SECONDS <= now_seconds() {
            return Ok(());
        }
        let mut state = self.state.lock().unwrap();
        if state.emergency_grace_used_for_token.as_deref() != Some(token) {
            state.emergency_grace_used_for_token = Some(token.to_string());
            self.persist(&state)?;
        }
        Ok(())
    }

    fn device_context(&self) -> AppResult<(String, String, String, String)> {
        let status = self.get_status()?;
        Ok((
            status.device_id,
            device_name(),
            std::env::consts::OS.to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        ))
    }

    fn client(&self) -> AppResult<reqwest::Client> {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|e| AppError::Other(format!("reqwest build: {e}")))
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.license_server_url, path)
    }

    async fn complete_free_quota_reservation(
        &self,
        path: &str,
        reservation_id: String,
    ) -> AppResult<FreeQuotaReservation> {
        let request = FreeQuotaReservationRequest {
            reservation_id: reservation_id.trim(),
        };
        let quota = self
            .client()?
            .post(self.endpoint(path))
            .json(&request)
            .send()
            .await
            .map_err(http_error)?;
        let quota = json_or_status::<FreeQuotaReservation>(quota).await?;
        self.apply_free_quota_reservation_cache(&quota)?;
        Ok(quota)
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

struct TokenEvaluation {
    claims: Option<EntitlementClaims>,
    error: Option<String>,
}

fn evaluate_token_for_device(
    token: Option<&str>,
    device_id: &str,
    public_key: &str,
    emergency_grace_used_for_token: Option<&str>,
) -> TokenEvaluation {
    let Some(token) = token else {
        return TokenEvaluation {
            claims: None,
            error: Some("token_missing".into()),
        };
    };
    if emergency_grace_used_for_token == Some(token) {
        if let Ok(claims) = validate_token_for_device(token, device_id, public_key, false) {
            if claims.exp <= now_seconds() && claims.exp + EMERGENCY_GRACE_SECONDS > now_seconds() {
                return TokenEvaluation {
                    claims: Some(claims),
                    error: Some("emergency_grace_active".into()),
                };
            }
        }
    }
    match validate_token_for_device(token, device_id, public_key, true) {
        Ok(claims) => TokenEvaluation {
            claims: Some(claims),
            error: None,
        },
        Err(error) => TokenEvaluation {
            claims: None,
            error: Some(error),
        },
    }
}

fn validate_token_for_device(
    token: &str,
    device_id: &str,
    public_key: &str,
    validate_exp: bool,
) -> Result<EntitlementClaims, String> {
    if public_key.trim().is_empty() {
        return Err("public_key_missing".into());
    }
    let key = DecodingKey::from_ed_pem(public_key.as_bytes())
        .map_err(|e| format!("public_key_invalid: {e}"))?;
    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.set_issuer(&[TOKEN_ISSUER]);
    validation.set_audience(&[TOKEN_AUDIENCE]);
    validation.validate_exp = validate_exp;
    let data = decode::<EntitlementClaims>(token, &key, &validation)
        .map_err(|e| format!("token_invalid: {e}"))?;
    if data.claims.device_id != device_id {
        return Err("token_device_mismatch".into());
    }
    if validate_exp && data.claims.exp <= now_seconds() {
        return Err("token_expired".into());
    }
    Ok(data.claims)
}

fn clear_pro_state(state: &mut EntitlementFile) {
    state.license_id = None;
    state.license_email = None;
    state.license_key_last4 = None;
    state.signed_token = None;
    state.token_expires_at = None;
    state.emergency_grace_used_for_token = None;
}

fn normalize_public_key(value: &str) -> String {
    value.replace("\\n", "\n").trim().to_string()
}

fn http_error(error: reqwest::Error) -> AppError {
    AppError::Other(format!("license server request failed: {error}"))
}

fn status_error(error: reqwest::Error) -> AppError {
    if let Some(status) = error.status() {
        AppError::Other(format!("license server returned {status}"))
    } else {
        http_error(error)
    }
}

async fn json_or_status<T: DeserializeOwned>(response: reqwest::Response) -> AppResult<T> {
    let status = response.status();
    if status.is_success() {
        return response.json::<T>().await.map_err(http_error);
    }

    let body = response
        .text()
        .await
        .unwrap_or_else(|_| String::from("<unreadable response body>"));
    Err(AppError::Other(format!(
        "license server returned {status}: {body}"
    )))
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "This device".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    use serde::Serialize;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    const TEST_PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIA7zL6eM/tNB2W5iXqP5UwBeNNnlFinprAJIyH01gko7\n-----END PRIVATE KEY-----";
    const TEST_PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAPrX03lyKn6qKDyxzRGqDOeEztXtIYMzR0rRkOGdC+DQ=\n-----END PUBLIC KEY-----";

    #[derive(Serialize)]
    struct TestClaims<'a> {
        license_id: &'a str,
        device_id: &'a str,
        plan: &'a str,
        activation_limit: u32,
        iss: &'a str,
        aud: &'a str,
        jti: &'a str,
        iat: u64,
        exp: u64,
    }

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

    #[test]
    fn valid_signed_token_returns_pro_status() {
        let temp = tempfile::tempdir().unwrap();
        let store =
            EntitlementStore::load_with_config(temp.path(), TEST_PUBLIC_KEY, "http://127.0.0.1:9")
                .unwrap();
        let device_id = store.get_status().unwrap().device_id;
        let token = test_token(&device_id, now_seconds() + 3600);

        store
            .apply_server_status(
                ServerLicenseStatus {
                    token,
                    token_expires_at: "2099-01-01T00:00:00.000Z".into(),
                    license_email: "buyer@example.com".into(),
                    license_key_last4: "ABCD".into(),
                },
                &device_id,
            )
            .unwrap();

        let status = store.get_status().unwrap();
        assert!(status.pro_active);
        assert_eq!(status.plan, "pro");
        assert_eq!(status.license_id.as_deref(), Some("lic_test"));
        assert_eq!(status.token_validation_error, None);
    }

    #[test]
    fn expired_signed_token_returns_free_status() {
        let temp = tempfile::tempdir().unwrap();
        let store =
            EntitlementStore::load_with_config(temp.path(), TEST_PUBLIC_KEY, "http://127.0.0.1:9")
                .unwrap();
        let mut state = store.state.lock().unwrap();
        let device_id = "ytb_test_device";
        state.device_id_fallback = Some(device_id.into());
        state.signed_token = Some(test_token(device_id, now_seconds() - 60));
        state.token_expires_at = Some("2000-01-01T00:00:00.000Z".into());
        drop(state);

        let status = store.get_status().unwrap();
        assert!(!status.pro_active);
        assert_eq!(status.plan, "free");
        assert!(matches!(
            status.token_validation_error.as_deref(),
            Some(error) if error.contains("ExpiredSignature") || error == "token_expired"
        ));
    }

    #[test]
    fn deactivate_clears_pro_state_but_keeps_identity() {
        let mut state = EntitlementFile {
            device_id_fallback: Some("ytb_device".into()),
            installation_id_fallback: Some("ytb_installation".into()),
            license_id: Some("lic_test".into()),
            license_email: Some("buyer@example.com".into()),
            license_key_last4: Some("ABCD".into()),
            signed_token: Some("token".into()),
            token_expires_at: Some("2099-01-01T00:00:00.000Z".into()),
            trial_used_count_cache: Some(1),
            trial_remaining_count_cache: Some(9),
            emergency_grace_used_for_token: Some("token".into()),
        };

        clear_pro_state(&mut state);

        assert_eq!(state.device_id_fallback.as_deref(), Some("ytb_device"));
        assert_eq!(
            state.installation_id_fallback.as_deref(),
            Some("ytb_installation")
        );
        assert!(state.license_id.is_none());
        assert!(state.signed_token.is_none());
        assert_eq!(state.trial_remaining_count_cache, Some(9));
    }

    #[tokio::test]
    async fn activate_pro_saves_server_token() {
        let temp = tempfile::tempdir().unwrap();
        let initial_store =
            EntitlementStore::load_with_config(temp.path(), TEST_PUBLIC_KEY, "http://127.0.0.1:9")
                .unwrap();
        let device_id = initial_store.get_status().unwrap().device_id;
        drop(initial_store);

        let token = test_token(&device_id, now_seconds() + 3600);
        let server = mock_json_server(format!(
            r#"{{
              "kind":"activated",
              "status":{{
                "status":"active",
                "plan":"pro",
                "activation_limit":3,
                "active_device_count":1,
                "token":"{token}",
                "token_expires_at":"2099-01-01T00:00:00.000Z",
                "license_email":"buyer@example.com",
                "license_key_last4":"ABCD"
              }}
            }}"#
        ));
        let store =
            EntitlementStore::load_with_config(temp.path(), TEST_PUBLIC_KEY, &server.url).unwrap();

        let result = store
            .activate_pro("YTB-AAAA-BBBB-CCCC-DDDD".into())
            .await
            .unwrap();

        match result {
            ActivateProResult::Activated { status } => {
                assert!(status.pro_active);
                assert_eq!(status.license_email.as_deref(), Some("buyer@example.com"));
            }
            ActivateProResult::TransferCodeRequired { .. } => panic!("expected activation"),
        }
    }

    #[tokio::test]
    async fn activate_pro_returns_transfer_code_required() {
        let temp = tempfile::tempdir().unwrap();
        let server = mock_json_server(
            r#"{
              "kind":"transfer_code_required",
              "email_hint":"bu***@example.com",
              "active_device_count":3
            }"#
            .into(),
        );
        let store =
            EntitlementStore::load_with_config(temp.path(), TEST_PUBLIC_KEY, &server.url).unwrap();

        let result = store
            .activate_pro("YTB-AAAA-BBBB-CCCC-DDDD".into())
            .await
            .unwrap();

        match result {
            ActivateProResult::TransferCodeRequired {
                email_hint,
                active_device_count,
            } => {
                assert_eq!(email_hint, "bu***@example.com");
                assert_eq!(active_device_count, 3);
            }
            ActivateProResult::Activated { .. } => panic!("expected transfer_code_required"),
        }
    }

    #[tokio::test]
    async fn refresh_pro_updates_stored_token() {
        let temp = tempfile::tempdir().unwrap();
        let mut store =
            EntitlementStore::load_with_config(temp.path(), TEST_PUBLIC_KEY, "http://127.0.0.1:9")
                .unwrap();
        let device_id = store.get_status().unwrap().device_id;
        store
            .apply_server_status(
                ServerLicenseStatus {
                    token: test_token(&device_id, now_seconds() + 3600),
                    token_expires_at: "2099-01-01T00:00:00.000Z".into(),
                    license_email: "buyer@example.com".into(),
                    license_key_last4: "ABCD".into(),
                },
                &device_id,
            )
            .unwrap();

        let refreshed = test_token(&device_id, now_seconds() + 7200);
        let server = mock_json_server(format!(
            r#"{{
              "status":"active",
              "plan":"pro",
              "activation_limit":3,
              "active_device_count":1,
              "token":"{refreshed}",
              "token_expires_at":"2099-01-02T00:00:00.000Z",
              "license_email":"buyer@example.com",
              "license_key_last4":"WXYZ"
            }}"#
        ));
        store.license_server_url = server.url;

        let status = store.refresh_pro().await.unwrap();

        assert!(status.pro_active);
        assert_eq!(status.license_key_last4.as_deref(), Some("WXYZ"));
        assert_eq!(status.signed_token.as_deref(), Some(refreshed.as_str()));
    }

    #[tokio::test]
    async fn refresh_failure_enables_one_day_emergency_grace_for_expired_token() {
        let temp = tempfile::tempdir().unwrap();
        let store =
            EntitlementStore::load_with_config(temp.path(), TEST_PUBLIC_KEY, "http://127.0.0.1:9")
                .unwrap();
        let device_id = store.get_status().unwrap().device_id;
        let expired = test_token(&device_id, now_seconds() - 60);
        {
            let mut state = store.state.lock().unwrap();
            state.license_id = Some("lic_test".into());
            state.license_email = Some("buyer@example.com".into());
            state.license_key_last4 = Some("ABCD".into());
            state.signed_token = Some(expired.clone());
            state.token_expires_at = Some("2000-01-01T00:00:00.000Z".into());
        }

        let status = store.refresh_pro().await.unwrap();

        assert!(status.pro_active);
        assert_eq!(
            status.token_validation_error.as_deref(),
            Some("emergency_grace_active")
        );
        assert_eq!(
            status.emergency_grace_used_for_token.as_deref(),
            Some(expired.as_str())
        );
    }

    #[tokio::test]
    async fn send_transfer_code_returns_email_hint() {
        let temp = tempfile::tempdir().unwrap();
        let server = mock_json_server(
            r#"{
              "sent":true,
              "email_hint":"bu***@example.com",
              "expires_at":"2099-01-01T00:00:00.000Z",
              "dev_code":"123456"
            }"#
            .into(),
        );
        let store =
            EntitlementStore::load_with_config(temp.path(), TEST_PUBLIC_KEY, &server.url).unwrap();

        let result = store
            .send_transfer_code("YTB-AAAA-BBBB-CCCC-DDDD".into())
            .await
            .unwrap();

        assert!(result.sent);
        assert_eq!(result.email_hint, "bu***@example.com");
        assert_eq!(result.dev_code.as_deref(), Some("123456"));
    }

    #[tokio::test]
    async fn activate_with_transfer_code_saves_server_token() {
        let temp = tempfile::tempdir().unwrap();
        let initial_store =
            EntitlementStore::load_with_config(temp.path(), TEST_PUBLIC_KEY, "http://127.0.0.1:9")
                .unwrap();
        let device_id = initial_store.get_status().unwrap().device_id;
        drop(initial_store);
        let token = test_token(&device_id, now_seconds() + 3600);
        let server = mock_json_server(format!(
            r#"{{
              "kind":"activated",
              "status":{{
                "status":"active",
                "plan":"pro",
                "activation_limit":3,
                "active_device_count":1,
                "token":"{token}",
                "token_expires_at":"2099-01-01T00:00:00.000Z",
                "license_email":"buyer@example.com",
                "license_key_last4":"ABCD"
              }}
            }}"#
        ));
        let store =
            EntitlementStore::load_with_config(temp.path(), TEST_PUBLIC_KEY, &server.url).unwrap();

        let status = store
            .activate_with_transfer_code("YTB-AAAA-BBBB-CCCC-DDDD".into(), "123456".into())
            .await
            .unwrap();

        assert!(status.pro_active);
        assert_eq!(status.license_email.as_deref(), Some("buyer@example.com"));
    }

    #[tokio::test]
    async fn sync_free_quota_status_updates_local_cache() {
        let temp = tempfile::tempdir().unwrap();
        let server = mock_json_server(
            r#"{
              "installation_id":"ytb_installation",
              "quota_limit":10,
              "used_count":2,
              "reserved_count":0,
              "remaining_count":8
            }"#
            .into(),
        );
        let store =
            EntitlementStore::load_with_config(temp.path(), TEST_PUBLIC_KEY, &server.url).unwrap();

        let quota = store.sync_free_quota_status().await.unwrap();
        let status = store.get_status().unwrap();

        assert_eq!(quota.remaining_count, 8);
        assert_eq!(status.trial_used_count_cache, Some(2));
        assert_eq!(status.trial_remaining_count_cache, Some(8));
    }

    #[tokio::test]
    async fn reserve_free_quota_updates_local_cache() {
        let temp = tempfile::tempdir().unwrap();
        let server = mock_json_server(
            r#"{
              "installation_id":"ytb_installation",
              "quota_limit":10,
              "used_count":2,
              "reserved_count":1,
              "remaining_count":7,
              "reservation_id":"reservation_test",
              "reservation_status":"reserved",
              "reservation_count":1,
              "expires_at":"2099-01-01T00:00:00.000Z"
            }"#
            .into(),
        );
        let store =
            EntitlementStore::load_with_config(temp.path(), TEST_PUBLIC_KEY, &server.url).unwrap();

        let quota = store.reserve_free_quota(1).await.unwrap();
        let status = store.get_status().unwrap();

        assert_eq!(quota.reservation_id.as_deref(), Some("reservation_test"));
        assert_eq!(status.trial_remaining_count_cache, Some(7));
    }

    #[tokio::test]
    async fn reserve_free_quota_error_preserves_server_body() {
        let temp = tempfile::tempdir().unwrap();
        let server = mock_json_server_with_status(
            402,
            r#"{"code":"quota_exceeded","message":"免费版最多可下载 10 个视频。"}"#.into(),
        );
        let store =
            EntitlementStore::load_with_config(temp.path(), TEST_PUBLIC_KEY, &server.url).unwrap();

        let error = store.reserve_free_quota(1).await.unwrap_err().to_string();

        assert!(error.contains("402"));
        assert!(error.contains("quota_exceeded"));
        assert!(error.contains("免费版最多可下载 10 个视频"));
    }

    #[tokio::test]
    async fn confirm_free_quota_updates_local_cache() {
        let temp = tempfile::tempdir().unwrap();
        let server = mock_json_server(
            r#"{
              "installation_id":"ytb_installation",
              "quota_limit":10,
              "used_count":3,
              "reserved_count":0,
              "remaining_count":7,
              "reservation_id":"reservation_test",
              "reservation_status":"confirmed",
              "reservation_count":1,
              "expires_at":"2099-01-01T00:00:00.000Z"
            }"#
            .into(),
        );
        let store =
            EntitlementStore::load_with_config(temp.path(), TEST_PUBLIC_KEY, &server.url).unwrap();

        let quota = store
            .confirm_free_quota("reservation_test".into())
            .await
            .unwrap();
        let status = store.get_status().unwrap();

        assert_eq!(quota.reservation_status.as_deref(), Some("confirmed"));
        assert_eq!(status.trial_used_count_cache, Some(3));
        assert_eq!(status.trial_remaining_count_cache, Some(7));
    }

    fn test_token(device_id: &str, exp: u64) -> String {
        encode(
            &Header::new(Algorithm::EdDSA),
            &TestClaims {
                license_id: "lic_test",
                device_id,
                plan: "pro",
                activation_limit: 3,
                iss: TOKEN_ISSUER,
                aud: TOKEN_AUDIENCE,
                jti: "token_test",
                iat: now_seconds(),
                exp,
            },
            &EncodingKey::from_ed_pem(TEST_PRIVATE_KEY.as_bytes()).unwrap(),
        )
        .unwrap()
    }

    struct MockServer {
        url: String,
    }

    fn mock_json_server(body: String) -> MockServer {
        mock_json_server_with_status(200, body)
    }

    fn mock_json_server_with_status(status: u16, body: String) -> MockServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request);
            let response = format!(
                "HTTP/1.1 {status} OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        MockServer {
            url: format!("http://{addr}"),
        }
    }
}
