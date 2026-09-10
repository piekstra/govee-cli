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

/// Method and body are checked before any credential is read.
pub fn validate(args: &ApiCommand) -> Result<(reqwest::Method, Option<Value>), CliError> {
    Ok((args.api.parsed_method()?, args.api.parsed_body()?))
}

pub async fn run(
    ctx: &Ctx,
    args: &ApiCommand,
    method: reqwest::Method,
    body: Option<Value>,
) -> Result<(), CliError> {
    let api = ctx.api()?;
    let url = args.api.url(BASE_URL);
    let payload = api.raw(method, &url, body).await?;
    output::emit(ctx.json, "api-response", payload, output::render);
    Ok(())
}
