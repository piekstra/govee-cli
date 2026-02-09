use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::error::AppError;

const BASE_URL: &str = "https://openapi.api.govee.com/router/api/v1";

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
    data: Option<serde_json::Value>,
    #[serde(default)]
    payload: Option<serde_json::Value>,
}

impl ApiResponse {
    fn into_result(self) -> Result<serde_json::Value, AppError> {
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
                401 => AppError::NotAuthenticated,
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
    pub fn new(api_key: String, verbose: bool) -> Result<Self, AppError> {
        let client = reqwest::Client::builder()
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
                eprintln!("[verbose] Per-minute rate limit remaining: {}", v.to_str().unwrap_or("?"));
            }
            if let Some(v) = response.headers().get("X-RateLimit-Remaining") {
                eprintln!("[verbose] Daily rate limit remaining: {}", v.to_str().unwrap_or("?"));
            }
        }
    }

    /// GET /user/devices - list all devices
    pub async fn get_devices(&self) -> Result<serde_json::Value, AppError> {
        let url = format!("{}/user/devices", BASE_URL);
        if self.verbose {
            eprintln!("[verbose] GET {}", url);
        }
        let response = self
            .client
            .get(&url)
            .headers(self.headers())
            .send()
            .await?;
        self.log_rate_limits(&response);
        let api_response: ApiResponse = response.json().await?;
        api_response.into_result()
    }

    /// POST /device/control - control a device
    pub async fn control_device(
        &self,
        sku: &str,
        device: &str,
        cap_type: &str,
        instance: &str,
        value: serde_json::Value,
    ) -> Result<(), AppError> {
        let url = format!("{}/device/control", BASE_URL);
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
            eprintln!("[verbose] POST {}", url);
            eprintln!(
                "[verbose] Body: {}",
                serde_json::to_string_pretty(&body).unwrap_or_default()
            );
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
        api_response.into_result()?;
        Ok(())
    }

    /// POST /device/state - query device state
    pub async fn get_device_state(
        &self,
        sku: &str,
        device: &str,
    ) -> Result<serde_json::Value, AppError> {
        let url = format!("{}/device/state", BASE_URL);
        let body = json!({
            "requestId": Self::request_id(),
            "payload": { "sku": sku, "device": device }
        });
        if self.verbose {
            eprintln!("[verbose] POST {}", url);
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

    /// POST /device/scenes - list available scenes
    pub async fn get_device_scenes(
        &self,
        sku: &str,
        device: &str,
    ) -> Result<serde_json::Value, AppError> {
        let url = format!("{}/device/scenes", BASE_URL);
        let body = json!({
            "requestId": Self::request_id(),
            "payload": { "sku": sku, "device": device }
        });
        if self.verbose {
            eprintln!("[verbose] POST {}", url);
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

    /// POST /device/diy-scenes - list DIY scenes
    pub async fn get_device_diy_scenes(
        &self,
        sku: &str,
        device: &str,
    ) -> Result<serde_json::Value, AppError> {
        let url = format!("{}/device/diy-scenes", BASE_URL);
        let body = json!({
            "requestId": Self::request_id(),
            "payload": { "sku": sku, "device": device }
        });
        if self.verbose {
            eprintln!("[verbose] POST {}", url);
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
}
