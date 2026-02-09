use crate::api::client::GoveeApi;
use crate::auth::api_key::get_api_key;
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

/// Fetch all devices from the Govee API.
pub async fn fetch_all_devices(verbose: bool) -> Result<Vec<(DeviceInfo, DeviceType)>, AppError> {
    let api_key = get_api_key()?;
    let api = GoveeApi::new(api_key, verbose)?;
    let data = api.get_devices().await?;
    let devices = parse_devices(&data);
    Ok(devices
        .into_iter()
        .map(|info| {
            let dtype = DeviceType::from_sku(&info.sku);
            (info, dtype)
        })
        .collect())
}

/// Resolve a device by name or device ID.
///
/// Resolution priority:
/// 1. Exact device name match
/// 2. Exact device ID match
/// 3. Case-insensitive name match
/// 4. Partial name match (only if exactly one result)
pub async fn resolve_device(name_or_id: &str, verbose: bool) -> Result<Device, AppError> {
    let api_key = get_api_key()?;
    let api = GoveeApi::new(api_key.clone(), verbose)?;
    let data = api.get_devices().await?;
    let all_devices = parse_devices(&data);

    if all_devices.is_empty() {
        return Err(AppError::DeviceNotFound(format!(
            "No devices found. Is '{}' correct?",
            name_or_id
        )));
    }

    let name_lower = name_or_id.to_lowercase();

    // 1. Exact name match
    if let Some(info) = all_devices.iter().find(|d| d.name() == name_or_id) {
        return build_device(info.clone(), api_key, verbose);
    }

    // 2. Exact device ID match
    if let Some(info) = all_devices.iter().find(|d| d.id() == name_or_id) {
        return build_device(info.clone(), api_key, verbose);
    }

    // 3. Case-insensitive name match
    if let Some(info) = all_devices
        .iter()
        .find(|d| d.name().to_lowercase() == name_lower)
    {
        return build_device(info.clone(), api_key, verbose);
    }

    // 4. Partial match (unambiguous only)
    let partial: Vec<_> = all_devices
        .iter()
        .filter(|d| d.name().to_lowercase().contains(&name_lower))
        .collect();

    match partial.len() {
        1 => build_device(partial[0].clone(), api_key, verbose),
        0 => Err(AppError::DeviceNotFound(name_or_id.to_string())),
        _ => {
            let names: Vec<String> = partial.iter().map(|d| d.name().to_string()).collect();
            Err(AppError::DeviceNotFound(format!(
                "Multiple devices match '{}': {}",
                name_or_id,
                names.join(", ")
            )))
        }
    }
}

fn build_device(info: DeviceInfo, api_key: String, verbose: bool) -> Result<Device, AppError> {
    let dtype = DeviceType::from_sku(&info.sku);
    let api = GoveeApi::new(api_key, verbose)?;
    Ok(Device::new(api, info, dtype))
}
