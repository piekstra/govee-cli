use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not authenticated. Run 'govee login' first.")]
    NotAuthenticated,

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("API error: {message}")]
    Api {
        message: String,
        error_code: Option<i32>,
    },

    #[error("Rate limit exceeded: {0}")]
    RateLimited(String),

    #[error("Keychain error: {0}")]
    Keychain(String),

    #[error("Device does not support this operation: {0}")]
    UnsupportedOperation(String),

    #[error("{0}")]
    InvalidInput(String),

    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            AppError::NotAuthenticated => 2,
            AppError::DeviceNotFound(_) => 3,
            AppError::RateLimited(_) => 4,
            _ => 1,
        }
    }

    pub fn error_type(&self) -> &'static str {
        match self {
            AppError::NotAuthenticated => "not_authenticated",
            AppError::DeviceNotFound(_) => "device_not_found",
            AppError::Api { .. } => "api",
            AppError::RateLimited(_) => "rate_limited",
            AppError::Keychain(_) => "keychain",
            AppError::UnsupportedOperation(_) => "unsupported_operation",
            AppError::InvalidInput(_) => "invalid_input",
            AppError::Http(_) => "http",
            AppError::Json(_) => "json",
            AppError::Io(_) => "io",
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut obj = json!({
            "error": self.error_type(),
            "message": self.to_string(),
        });
        if let AppError::Api {
            error_code: Some(code),
            ..
        } = self
        {
            obj["error_code"] = json!(code);
        }
        obj
    }
}
