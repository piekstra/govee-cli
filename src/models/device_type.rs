use serde::Serialize;

/// Known Govee device types, mapped from SKU prefix.
/// Govee has hundreds of SKUs; this captures well-known ones for display purposes.
/// Unknown devices still work via dynamic capability detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DeviceType {
    // Floor Lamps
    H60B0,
    // LED Strips
    H6601,
    H618E,
    // Light Bars
    H6057,
    // Catch-all
    Unknown,
}

const SKU_MAP: &[(&str, DeviceType)] = &[
    ("H60B0", DeviceType::H60B0),
    ("H6601", DeviceType::H6601),
    ("H618E", DeviceType::H618E),
    ("H6057", DeviceType::H6057),
];

impl DeviceType {
    pub fn from_sku(sku: &str) -> Self {
        for (prefix, device_type) in SKU_MAP {
            if sku.starts_with(prefix) {
                return *device_type;
            }
        }
        DeviceType::Unknown
    }

    pub fn category(&self) -> &'static str {
        match self {
            DeviceType::H60B0 => "floor_lamp",
            DeviceType::H6601 | DeviceType::H618E => "led_strip",
            DeviceType::H6057 => "light_bar",
            DeviceType::Unknown => "unknown",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            DeviceType::H60B0 => "Uplighter Floor Lamp",
            DeviceType::H6601 => "RGBIC LED Strip",
            DeviceType::H618E => "RGBICWW LED Strip",
            DeviceType::H6057 => "RGB Light Bar",
            DeviceType::Unknown => "Unknown Device",
        }
    }
}
