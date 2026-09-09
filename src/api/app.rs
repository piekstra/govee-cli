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
        self.request(reqwest::Method::POST, path, body, token).await
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Value,
        token: Option<&str>,
    ) -> Result<Value, AppError> {
        let url = format!("{BASE}{path}");
        if self.verbose {
            eprintln!("{method} {url}");
        }
        let resp = self
            .client
            .request(method, &url)
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
            message: format!(
                "HTTP {}: {}",
                status.as_u16(),
                text.chars().take(200).collect::<String>()
            ),
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
                    v.get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("no message")
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
        let v = self
            .post("/account/rest/account/v2/login", body, None)
            .await?;
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
                    v.get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("no message")
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
                    v.get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("no message")
                ),
                error_code: other.map(|n| n as i32),
            }),
        }
    }
}

/// How a device can be reached. `Wifi` devices are cloud-reachable (and so
/// visible to the Platform API and Google Home); `Bluetooth` devices are
/// controllable only from a phone next to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Connectivity {
    Wifi,
    Bluetooth,
}

/// One row of the app's device list, reduced to what the CLI needs.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AppDevice {
    pub sku: String,
    pub device: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room: Option<String>,
    pub connectivity: Connectivity,
}

/// A device row as `devices list` prints it: the Platform view joined with
/// the app view, plus the app's Bluetooth-only devices.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceRow {
    pub name: String,
    pub device: String,
    pub sku: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub category: String,
    pub connectivity: Connectivity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room: Option<String>,
}

/// A Platform-listed device, as much of it as the merge needs.
#[derive(Debug, Clone)]
pub struct PlatformDevice {
    pub device: String,
    pub sku: String,
    pub name: String,
    pub kind: String,
    pub category: String,
}

/// Join the Platform list with the app view. Platform devices are Wi-Fi by
/// definition (the Platform API only lists cloud devices) and gain their
/// room from the app; app devices the Platform never listed are appended
/// with the connectivity the app reports (Bluetooth-only in practice; a
/// Wi-Fi device missing from the Platform list is Platform-side lag and is
/// shown rather than dropped). With no app view, rows come out room-less.
pub fn merge_app_view(platform: &[PlatformDevice], app: Option<&[AppDevice]>) -> Vec<DeviceRow> {
    let mut rows: Vec<DeviceRow> = platform
        .iter()
        .map(|p| DeviceRow {
            name: p.name.clone(),
            device: p.device.clone(),
            sku: p.sku.clone(),
            kind: p.kind.clone(),
            category: p.category.clone(),
            connectivity: Connectivity::Wifi,
            room: app.and_then(|a| {
                a.iter()
                    .find(|x| x.device == p.device)
                    .and_then(|x| x.room.clone())
            }),
        })
        .collect();
    if let Some(app) = app {
        for a in app.iter() {
            if !platform.iter().any(|p| p.device == a.device) {
                rows.push(DeviceRow {
                    name: a.name.clone(),
                    device: a.device.clone(),
                    sku: a.sku.clone(),
                    kind: match a.connectivity {
                        Connectivity::Bluetooth => "bluetooth-only".into(),
                        Connectivity::Wifi => "app-only".into(),
                    },
                    category: "app-only".into(),
                    connectivity: a.connectivity,
                    room: a.room.clone(),
                });
            }
        }
    }
    rows
}

/// Parse the app device list into rows, resolving groups to room names and
/// deciding connectivity from the device's Wi-Fi settings.
pub fn app_devices(list: &Value) -> Vec<AppDevice> {
    let groups: Vec<(i64, String)> = list
        .get("groups")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|g| {
                    Some((
                        g.get("groupId")?.as_i64()?,
                        g.get("groupName")?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    list.get("devices")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|d| {
                    let settings = d
                        .pointer("/deviceExt/deviceSettings")
                        .and_then(Value::as_str)
                        .and_then(|s| serde_json::from_str::<Value>(s).ok())
                        .unwrap_or(Value::Null);
                    let wifi = settings
                        .get("wifiName")
                        .and_then(Value::as_str)
                        .is_some_and(|w| !w.is_empty())
                        || settings.get("wifiSoftVersion").is_some();
                    Some(AppDevice {
                        sku: d.get("sku")?.as_str()?.to_string(),
                        device: d.get("device")?.as_str()?.to_string(),
                        name: d
                            .get("deviceName")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        room: d.get("groupId").and_then(Value::as_i64).and_then(|gid| {
                            groups
                                .iter()
                                .find(|(id, _)| *id == gid)
                                .map(|(_, n)| n.clone())
                        }),
                        connectivity: if wifi {
                            Connectivity::Wifi
                        } else {
                            Connectivity::Bluetooth
                        },
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}
