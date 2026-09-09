//! The Govee Home app's private API (`app2.govee.com`). The public Platform
//! API has no notion of rooms; the app API does (device `groupId` +
//! `groups[]`). Login is email + password and, since mid-2026, an emailed
//! verification code (status 454 → request a code → log in again with it).
//! Header set mirrors the iOS app; older `appVersion`s are rejected.

use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::{json, Value};

use crate::error::AppError;

const BASE: &str = "https://app2.govee.com";
const APP_VERSION: &str = "7.4.10";
const USER_AGENT: &str =
    "GoveeHome/7.4.10 (com.ihoment.GoVeeSensor; build:8; iOS 26.5.0) Alamofire/5.11.0";

pub struct GoveeApp {
    client: reqwest::Client,
    client_id: String,
    verbose: bool,
}

pub enum LoginOutcome {
    /// Logged in: the bearer token and account id.
    Token { token: String, account_id: String },
    /// Govee wants the code it just emailed (or will email on request).
    NeedsCode,
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

impl GoveeApp {
    /// `client_id` identifies this installation to Govee; keep it stable
    /// across runs (it is stored alongside the token).
    pub fn new(client_id: String, verbose: bool) -> Result<Self, AppError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()?;
        Ok(Self {
            client,
            client_id,
            verbose,
        })
    }

    pub fn new_client_id() -> String {
        uuid::Uuid::new_v4().simple().to_string()
    }

    fn headers(&self, token: Option<&str>) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("appVersion", HeaderValue::from_static(APP_VERSION));
        h.insert("clientId", HeaderValue::from_str(&self.client_id).unwrap());
        h.insert("clientType", HeaderValue::from_static("1"));
        h.insert("iotVersion", HeaderValue::from_static("0"));
        h.insert(
            "timestamp",
            HeaderValue::from_str(&now_ms().to_string()).unwrap(),
        );
        h.insert("User-Agent", HeaderValue::from_static(USER_AGENT));
        h.insert("Content-Type", HeaderValue::from_static("application/json"));
        if let Some(t) = token {
            if let Ok(v) = HeaderValue::from_str(&format!("Bearer {t}")) {
                h.insert("Authorization", v);
            }
        }
        h
    }

    async fn post(&self, path: &str, body: Value, token: Option<&str>) -> Result<Value, AppError> {
        let url = format!("{BASE}{path}");
        if self.verbose {
            eprintln!("POST {url}");
        }
        let resp = self
            .client
            .post(&url)
            .headers(self.headers(token))
            .json(&body)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        if self.verbose {
            eprintln!("HTTP {} ({} bytes)", status.as_u16(), text.len());
        }
        serde_json::from_str(&text).map_err(|_| AppError::Api {
            message: format!("HTTP {}: {}", status.as_u16(), text.chars().take(200).collect::<String>()),
            error_code: Some(status.as_u16() as i32),
        })
    }

    /// Ask Govee to email the verification code for `email`.
    pub async fn request_code(&self, email: &str) -> Result<(), AppError> {
        let v = self
            .post(
                "/account/rest/account/v1/verification",
                json!({"type": 8, "email": email}),
                None,
            )
            .await?;
        match v.get("status").and_then(Value::as_i64) {
            Some(200) => Ok(()),
            other => Err(AppError::Api {
                message: format!(
                    "verification request failed: {}",
                    v.get("message").and_then(Value::as_str).unwrap_or("no message")
                ),
                error_code: other.map(|n| n as i32),
            }),
        }
    }

    pub async fn login(
        &self,
        email: &str,
        password: &str,
        code: Option<&str>,
    ) -> Result<LoginOutcome, AppError> {
        let mut body = json!({"email": email, "password": password, "client": self.client_id});
        if let Some(c) = code {
            body["code"] = json!(c);
        }
        let v = self.post("/account/rest/account/v2/login", body, None).await?;
        match v.get("status").and_then(Value::as_i64) {
            Some(200) => {
                let client = v.get("client").cloned().unwrap_or(Value::Null);
                let token = client
                    .get("token")
                    .and_then(Value::as_str)
                    .ok_or_else(|| AppError::Api {
                        message: "login succeeded but no token in response".into(),
                        error_code: None,
                    })?
                    .to_string();
                let account_id = client
                    .get("accountId")
                    .map(|a| a.to_string().trim_matches('"').to_string())
                    .unwrap_or_default();
                Ok(LoginOutcome::Token { token, account_id })
            }
            Some(454) => Ok(LoginOutcome::NeedsCode),
            Some(455) => Err(AppError::Api {
                message: "verification code wrong or expired".into(),
                error_code: Some(455),
            }),
            other => Err(AppError::Api {
                message: format!(
                    "login failed: {}",
                    v.get("message").and_then(Value::as_str).unwrap_or("no message")
                ),
                error_code: other.map(|n| n as i32),
            }),
        }
    }

    /// The app's device list: devices with `groupId`, plus the `groups`
    /// table (the app's rooms).
    pub async fn device_list(&self, token: &str) -> Result<Value, AppError> {
        let v = self
            .post("/device/rest/devices/v1/list", json!({}), Some(token))
            .await?;
        match v.get("status").and_then(Value::as_i64) {
            Some(200) => Ok(v),
            Some(401) | Some(403) => Err(AppError::NotAuthenticated),
            other => Err(AppError::Api {
                message: format!(
                    "device list failed: {}",
                    v.get("message").and_then(Value::as_str).unwrap_or("no message")
                ),
                error_code: other.map(|n| n as i32),
            }),
        }
    }
}
