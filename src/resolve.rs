//! Device resolution: the Platform device list, and the name/id ladder every
//! command uses to turn a user-supplied reference into one device.

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

/// Resolve a device by name or device ID.
///
/// Resolution priority:
/// 1. Exact device name match
/// 2. Exact device ID match
/// 3. Case-insensitive name match
/// 4. Partial name match (only if exactly one result)
pub async fn resolve_device(api: &GoveeApi, name_or_id: &str) -> Result<Device, AppError> {
    let data = api.get_devices().await?;
    let all_devices = parse_devices(&data);

    if all_devices.is_empty() {
        return Err(AppError::DeviceNotFound(format!(
            "no devices on the account; is `{name_or_id}` correct?"
        )));
    }

    let name_lower = name_or_id.to_lowercase();

    // 1. Exact name match
    if let Some(info) = all_devices.iter().find(|d| d.name() == name_or_id) {
        return Ok(build_device(api, info.clone()));
    }

    // 2. Exact device ID match
    if let Some(info) = all_devices.iter().find(|d| d.id() == name_or_id) {
        return Ok(build_device(api, info.clone()));
    }

    // 3. Case-insensitive name match
    if let Some(info) = all_devices
        .iter()
        .find(|d| d.name().to_lowercase() == name_lower)
    {
        return Ok(build_device(api, info.clone()));
    }

    // 4. Partial match (unambiguous only)
    let partial: Vec<_> = all_devices
        .iter()
        .filter(|d| d.name().to_lowercase().contains(&name_lower))
        .collect();

    match partial.len() {
        1 => Ok(build_device(api, partial[0].clone())),
        0 => Err(AppError::DeviceNotFound(format!(
            "no device matching `{name_or_id}`"
        ))),
        _ => {
            let names: Vec<String> = partial.iter().map(|d| d.name().to_string()).collect();
            Err(AppError::DeviceNotFound(format!(
                "multiple devices match `{name_or_id}`: {}",
                names.join(", ")
            )))
        }
    }
}

fn build_device(api: &GoveeApi, info: DeviceInfo) -> Device {
    let dtype = DeviceType::from_sku(&info.sku);
    Device::new(api.clone(), info, dtype)
}

/// Resolve a user-supplied reference the way [`resolve_device`] does, in
/// this order: exact name, exact id, case-insensitive name, then a unique
/// partial name. More than one hit at any tier (ids included) is a
/// `DeviceNotFound` naming the candidates, the same class `resolve_device`
/// uses for ambiguity; a silent first-pick never happens.
pub fn pick<'a, T>(
    items: &'a [T],
    query: &str,
    ids_of: impl Fn(&T) -> Vec<String>,
    name_of: impl Fn(&T) -> &str,
    what: &str,
) -> Result<&'a T, AppError> {
    let q = query.trim();
    let ambiguous = |hits: &[&T]| {
        AppError::DeviceNotFound(format!(
            "multiple {what}s match `{q}`: {}",
            hits.iter()
                .map(|x| format!("{} ({})", name_of(x), ids_of(x).join("/")))
                .collect::<Vec<_>>()
                .join("; ")
        ))
    };
    let one = |hits: Vec<&'a T>| -> Option<Result<&'a T, AppError>> {
        match hits.len() {
            0 => None,
            1 => Some(Ok(hits[0])),
            _ => Some(Err(ambiguous(&hits))),
        }
    };
    if let Some(r) = one(items.iter().filter(|x| name_of(x) == q).collect()) {
        return r;
    }
    let ql = q.to_lowercase();
    if let Some(r) = one(items
        .iter()
        .filter(|x| ids_of(x).iter().any(|i| i.to_lowercase() == ql))
        .collect())
    {
        return r;
    }
    if let Some(r) = one(items
        .iter()
        .filter(|x| name_of(x).to_lowercase() == ql)
        .collect())
    {
        return r;
    }
    match one(items
        .iter()
        .filter(|x| name_of(x).to_lowercase().contains(&ql))
        .collect())
    {
        Some(r) => r,
        None => Err(AppError::DeviceNotFound(format!(
            "no {what} matching `{q}`"
        ))),
    }
}
