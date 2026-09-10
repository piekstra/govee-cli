//! The Govee Platform API (the public developer API). Auth is the
//! `Govee-API-Key` header; every response is an envelope
//! `{code, message|msg, data|payload}` where `code` is the real status.

use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::AppError;

pub const BASE_URL: &str = "https://openapi.api.govee.com/router/api/v1";

#[derive(Clone)]
pub struct GoveeApi {
    client: reqwest::Client,
    api_key: String,
    verbose: bool,
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    code: i32,
    #[serde(alias = "msg")]
    message: Option<String>,
    #[serde(default)]
    data: Option<Value>,
    #[serde(default)]
    payload: Option<Value>,
}

impl ApiResponse {
    fn into_result(self) -> Result<Value, AppError> {
        if self.code == 200 {
            if let Some(data) = self.data {
                return Ok(data);
            }
            if let Some(payload) = self.payload {
                return Ok(payload);
            }
            Ok(json!(null))
        } else {
            let message = self
                .message
                .unwrap_or_else(|| format!("API error code {}", self.code));
            Err(match self.code {
                401 | 403 => AppError::NotAuthenticated,
                429 => AppError::RateLimited(message),
                _ => AppError::Api {
                    message,
                    error_code: Some(self.code),
                },
            })
        }
    }
}

impl GoveeApi {
    /// `api_key` is the raw key; it is only ever placed in the request
    /// header and never logged.
    pub fn new(api_key: String, verbose: bool) -> Result<Self, AppError> {
        let client = reqwest::Client::builder()
            .user_agent(format!("{}/{}", crate::BIN, env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(15))
            .build()?;
        Ok(Self {
            client,
            api_key,
            verbose,
        })
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Ok(val) = HeaderValue::from_str(&self.api_key) {
            headers.insert("Govee-API-Key", val);
        }
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        headers
    }

    fn request_id() -> String {
        Uuid::new_v4().to_string()
    }

    fn log_rate_limits(&self, response: &reqwest::Response) {
        if self.verbose {
            if let Some(v) = response.headers().get("API-RateLimit-Remaining") {
                eprintln!(
                    "[verbose] per-minute rate limit remaining: {}",
                    v.to_str().unwrap_or("?")
                );
            }
            if let Some(v) = response.headers().get("X-RateLimit-Remaining") {
                eprintln!(
                    "[verbose] daily rate limit remaining: {}",
                    v.to_str().unwrap_or("?")
                );
            }
        }
    }

    async fn post(&self, path: &str, body: Value) -> Result<Value, AppError> {
        let url = format!("{BASE_URL}{path}");
        if self.verbose {
            eprintln!("[verbose] POST {url}");
        }
        let response = self
            .client
            .post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;
        self.log_rate_limits(&response);
        let api_response: ApiResponse = response.json().await?;
        api_response.into_result()
    }

    /// GET /user/devices — every device on the account with its capabilities.
    pub async fn get_devices(&self) -> Result<Value, AppError> {
        let url = format!("{BASE_URL}/user/devices");
        if self.verbose {
            eprintln!("[verbose] GET {url}");
        }
        let response = self.client.get(&url).headers(self.headers()).send().await?;
        self.log_rate_limits(&response);
        let api_response: ApiResponse = response.json().await?;
        api_response.into_result()
    }

    /// POST /device/control — set one capability instance.
    pub async fn control_device(
        &self,
        sku: &str,
        device: &str,
        cap_type: &str,
        instance: &str,
        value: Value,
    ) -> Result<(), AppError> {
        let body = json!({
            "requestId": Self::request_id(),
            "payload": {
                "sku": sku,
                "device": device,
                "capability": {
                    "type": cap_type,
                    "instance": instance,
                    "value": value,
                }
            }
        });
        if self.verbose {
            eprintln!(
                "[verbose] body: {}",
                serde_json::to_string(&body).unwrap_or_default()
            );
        }
        self.post("/device/control", body).await?;
        Ok(())
    }

    /// POST /device/state — current capability values.
    pub async fn get_device_state(&self, sku: &str, device: &str) -> Result<Value, AppError> {
        self.post("/device/state", Self::device_body(sku, device))
            .await
    }

    /// POST /device/scenes — the dynamic (and snapshot) scenes a device offers.
    pub async fn get_device_scenes(&self, sku: &str, device: &str) -> Result<Value, AppError> {
        self.post("/device/scenes", Self::device_body(sku, device))
            .await
    }

    /// POST /device/diy-scenes — user-created DIY scenes.
    pub async fn get_device_diy_scenes(&self, sku: &str, device: &str) -> Result<Value, AppError> {
        self.post("/device/diy-scenes", Self::device_body(sku, device))
            .await
    }

    fn device_body(sku: &str, device: &str) -> Value {
        json!({
            "requestId": Self::request_id(),
            "payload": { "sku": sku, "device": device }
        })
    }

    /// Raw passthrough for `govee api`: the response body as-is (envelope
    /// included), with the HTTP status mapped onto the exit-code contract.
    pub async fn raw(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<Value>,
    ) -> Result<Value, AppError> {
        if self.verbose {
            eprintln!("[verbose] {method} {url}");
        }
        let mut req = self.client.request(method, url).headers(self.headers());
        if let Some(b) = body {
            req = req.json(&b);
        }
        let response = req.send().await?;
        self.log_rate_limits(&response);
        let status = response.status();
        let text = response.text().await?;
        if self.verbose {
            eprintln!("[verbose] HTTP {} ({} bytes)", status.as_u16(), text.len());
        }
        match status.as_u16() {
            401 | 403 => return Err(AppError::NotAuthenticated),
            404 => return Err(AppError::DeviceNotFound(format!("HTTP 404 for {url}"))),
            429 => return Err(AppError::RateLimited(snippet(&text))),
            s if !status.is_success() => {
                return Err(AppError::Api {
                    message: format!("HTTP {s}: {}", snippet(&text)),
                    error_code: Some(s as i32),
                })
            }
            _ => {}
        }
        serde_json::from_str(&text).map_err(|_| AppError::Api {
            message: format!("non-JSON response: {}", snippet(&text)),
            error_code: None,
        })
    }
}

fn snippet(text: &str) -> String {
    text.chars().take(200).collect()
}
