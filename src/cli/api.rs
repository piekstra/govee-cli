//! `govee api <METHOD> <PATH> [--data JSON]` — raw Platform API passthrough
//! with the `Govee-API-Key` header attached. The response envelope is
//! printed as-is (`api-response/v1`).

use clap::Args;
use pk_cli_core::{output, CliError};
use pk_cli_http::ApiArgs;
use serde_json::Value;

use super::Ctx;
use crate::api::client::BASE_URL;

#[derive(Args, Debug, Clone)]
pub struct ApiCommand {
    #[command(flatten)]
    pub api: ApiArgs,
}

/// A checked request: its method, its body, and a URL the key may travel to.
pub struct Checked {
    pub method: reqwest::Method,
    pub body: Option<Value>,
    pub url: String,
}

/// Checked before any credential is read. A full URL may name any `https` host, since the user
/// chose it, but the key goes with the request, so never plain `http` off this machine.
pub fn validate(args: &ApiCommand) -> Result<Checked, CliError> {
    Ok(Checked {
        method: args.api.parsed_method()?,
        body: args.api.parsed_body()?,
        url: args.api.secure_url(BASE_URL)?,
    })
}

pub async fn run(ctx: &Ctx, request: Checked) -> Result<(), CliError> {
    let api = ctx.api()?;
    let payload = api.raw(request.method, &request.url, request.body).await?;
    output::emit(ctx.json, "api-response", payload, output::render);
    Ok(())
}
