//! Device resolution: the Platform device list, and the family reference
//! ladder (`pk_cli_core::resolve::pick`) that turns a user-supplied name, id
//! or unique partial name into one device.

use pk_cli_core::{resolve::pick, CliError};

use crate::api::client::GoveeApi;
use crate::error::AppError;
use crate::models::device::Device;
use crate::models::device_info::DeviceInfo;
use crate::models::device_type::DeviceType;

fn parse_devices(data: &serde_json::Value) -> Vec<DeviceInfo> {
    match data.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| serde_json::from_value::<DeviceInfo>(v.clone()).ok())
            .collect(),
        None => vec![],
    }
}

/// Fetch all devices from the Platform API.
pub async fn fetch_all_devices(api: &GoveeApi) -> Result<Vec<(DeviceInfo, DeviceType)>, AppError> {
    let data = api.get_devices().await?;
    Ok(parse_devices(&data)
        .into_iter()
        .map(|info| {
            let dtype = DeviceType::from_sku(&info.sku);
            (info, dtype)
        })
        .collect())
}

/// Resolve a device by name or device id: exact name, exact id
/// (case-insensitive), case-insensitive name, then a unique partial name.
/// A tie at any tier is exit 4 naming the candidates.
pub async fn resolve_device(api: &GoveeApi, name_or_id: &str) -> Result<Device, CliError> {
    let data = api.get_devices().await?;
    let all_devices = parse_devices(&data);
    let info = pick(
        &all_devices,
        name_or_id,
        |d| vec![d.id().to_string()],
        |d| d.name(),
        "device",
    )?;
    let dtype = DeviceType::from_sku(&info.sku);
    Ok(Device::new(api.clone(), info.clone(), dtype))
}
