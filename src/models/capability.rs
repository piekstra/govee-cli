use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CapabilityType {
    OnOff,
    Toggle,
    Range,
    ColorSetting,
    SegmentColorSetting,
    DynamicScene,
    Mode,
    WorkMode,
    TemperatureSetting,
    Online,
    Unknown(String),
}

impl CapabilityType {
    pub fn from_api_type(s: &str) -> Self {
        match s {
            "devices.capabilities.on_off" => Self::OnOff,
            "devices.capabilities.toggle" => Self::Toggle,
            "devices.capabilities.range" => Self::Range,
            "devices.capabilities.color_setting" => Self::ColorSetting,
            "devices.capabilities.segment_color_setting" => Self::SegmentColorSetting,
            "devices.capabilities.dynamic_scene" => Self::DynamicScene,
            "devices.capabilities.mode" => Self::Mode,
            "devices.capabilities.work_mode" => Self::WorkMode,
            "devices.capabilities.temperature_setting" => Self::TemperatureSetting,
            "devices.capabilities.online" => Self::Online,
            other => Self::Unknown(other.to_string()),
        }
    }

    pub fn api_type(&self) -> &str {
        match self {
            Self::OnOff => "devices.capabilities.on_off",
            Self::Toggle => "devices.capabilities.toggle",
            Self::Range => "devices.capabilities.range",
            Self::ColorSetting => "devices.capabilities.color_setting",
            Self::SegmentColorSetting => "devices.capabilities.segment_color_setting",
            Self::DynamicScene => "devices.capabilities.dynamic_scene",
            Self::Mode => "devices.capabilities.mode",
            Self::WorkMode => "devices.capabilities.work_mode",
            Self::TemperatureSetting => "devices.capabilities.temperature_setting",
            Self::Online => "devices.capabilities.online",
            Self::Unknown(s) => s,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    #[serde(rename = "type")]
    pub capability_type: String,
    pub instance: String,
    pub parameters: serde_json::Value,
}

impl Capability {
    pub fn parsed_type(&self) -> CapabilityType {
        CapabilityType::from_api_type(&self.capability_type)
    }
}
