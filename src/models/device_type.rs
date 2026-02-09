use serde::Serialize;

/// Known Govee device types, mapped from SKU prefix.
/// Govee has hundreds of SKUs; this captures well-known ones for display purposes.
/// Unknown devices still work via dynamic capability detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DeviceType {
    // Bulbs
    H6004,
    H6008,
    // Table Lamps
    H6022,
    // Light Bars
    H6046,
    H6056,
    H6057,
    // Floor Lamps
    H60B0,
    // Panels / RGBIC Lights
    H6076,
    // TV Backlights
    H6099,
    // LED Strips
    H6601,
    H618E,
    // Neon Lights
    H61A8,
    // Device Groups
    BaseGroup,
    SameModeGroup,
    // Catch-all
    Unknown,
}

const SKU_MAP: &[(&str, DeviceType)] = &[
    ("H6004", DeviceType::H6004),
    ("H6008", DeviceType::H6008),
    ("H6022", DeviceType::H6022),
    ("H6046", DeviceType::H6046),
    ("H6056", DeviceType::H6056),
    ("H6057", DeviceType::H6057),
    ("H6076", DeviceType::H6076),
    ("H6099", DeviceType::H6099),
    ("H60B0", DeviceType::H60B0),
    ("H6601", DeviceType::H6601),
    ("H618E", DeviceType::H618E),
    ("H61A8", DeviceType::H61A8),
];

impl DeviceType {
    pub fn from_sku(sku: &str) -> Self {
        // Check for group types first (exact match)
        match sku {
            "BaseGroup" => return DeviceType::BaseGroup,
            "SameModeGroup" => return DeviceType::SameModeGroup,
            _ => {}
        }
        // Prefix-based lookup for hardware devices
        for (prefix, device_type) in SKU_MAP {
            if sku.starts_with(prefix) {
                return *device_type;
            }
        }
        DeviceType::Unknown
    }

    pub fn category(&self) -> &'static str {
        match self {
            DeviceType::H6004 | DeviceType::H6008 => "bulb",
            DeviceType::H6022 => "table_lamp",
            DeviceType::H6046 | DeviceType::H6056 | DeviceType::H6057 => "light_bar",
            DeviceType::H60B0 => "floor_lamp",
            DeviceType::H6076 => "panel",
            DeviceType::H6099 => "tv_backlight",
            DeviceType::H6601 | DeviceType::H618E => "led_strip",
            DeviceType::H61A8 => "neon_light",
            DeviceType::BaseGroup | DeviceType::SameModeGroup => "group",
            DeviceType::Unknown => "unknown",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            DeviceType::H6004 => "Smart Bulb",
            DeviceType::H6008 => "Smart Bulb",
            DeviceType::H6022 => "Table Lamp",
            DeviceType::H6046 => "RGBIC Light Bar",
            DeviceType::H6056 => "RGBIC Light Bar",
            DeviceType::H6057 => "RGB Light Bar",
            DeviceType::H60B0 => "Uplighter Floor Lamp",
            DeviceType::H6076 => "RGBIC Panel",
            DeviceType::H6099 => "TV Backlight",
            DeviceType::H6601 => "RGBIC LED Strip",
            DeviceType::H618E => "RGBICWW LED Strip",
            DeviceType::H61A8 => "Neon Rope Light",
            DeviceType::BaseGroup => "Device Group",
            DeviceType::SameModeGroup => "Sync Group",
            DeviceType::Unknown => "Unknown Device",
        }
    }
}
