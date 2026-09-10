//! Vendor-layer errors, mapped onto the family exit-code contract
//! (`pk_cli_core::CliError`, SPEC v1 §1.5) at the command boundary via
//! `From`. The API clients and models speak `AppError`; command handlers
//! return `CliError`, and `?` does the translation.

use pk_cli_core::CliError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// The Platform API key is missing or was rejected.
    #[error("no valid Govee API key; run `govee auth login`")]
    NotAuthenticated,

    /// The Govee Home account session is missing or was rejected.
    #[error("no valid Govee account session; run `govee auth login-account`")]
    AccountNotAuthenticated,

    #[error("{0}")]
    DeviceNotFound(String),

    #[error("{message}")]
    Api {
        message: String,
        error_code: Option<i32>,
    },

    #[error("rate limit exceeded: {0}")]
    RateLimited(String),

    #[error("device does not support this operation: {0}")]
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

impl From<AppError> for CliError {
    fn from(e: AppError) -> Self {
        match e {
            AppError::NotAuthenticated | AppError::AccountNotAuthenticated => {
                CliError::Auth(e.to_string())
            }
            AppError::DeviceNotFound(m) => CliError::NotFound(m),
            AppError::Api {
                message,
                error_code: Some(code),
            } => CliError::Upstream(format!("{message} (Govee status {code})")),
            AppError::Api {
                message,
                error_code: None,
            } => CliError::Upstream(message),
            AppError::RateLimited(_) | AppError::Http(_) => CliError::Upstream(e.to_string()),
            AppError::UnsupportedOperation(_) | AppError::InvalidInput(_) => {
                CliError::Usage(e.to_string())
            }
            AppError::Json(_) | AppError::Io(_) => CliError::Other(e.to_string()),
        }
    }
}
