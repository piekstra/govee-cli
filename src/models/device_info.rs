use serde::{Deserialize, Serialize};

use super::capability::Capability;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub sku: String,
    pub device: String,
    pub device_name: String,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
}

impl DeviceInfo {
    pub fn name(&self) -> &str {
        &self.device_name
    }

    pub fn id(&self) -> &str {
        &self.device
    }

    pub fn model(&self) -> &str {
        &self.sku
    }

    pub fn has_capability(&self, cap_type: &str, instance: &str) -> bool {
        self.capabilities
            .iter()
            .any(|c| c.capability_type == cap_type && c.instance == instance)
    }

    pub fn has_power(&self) -> bool {
        self.has_capability("devices.capabilities.on_off", "powerSwitch")
    }

    pub fn has_brightness(&self) -> bool {
        self.has_capability("devices.capabilities.range", "brightness")
    }

    pub fn has_color_rgb(&self) -> bool {
        self.has_capability("devices.capabilities.color_setting", "colorRgb")
    }

    pub fn has_color_temp(&self) -> bool {
        self.has_capability("devices.capabilities.color_setting", "colorTemperatureK")
    }

    pub fn has_scenes(&self) -> bool {
        self.has_capability("devices.capabilities.dynamic_scene", "lightScene")
    }

    pub fn has_diy_scenes(&self) -> bool {
        self.has_capability("devices.capabilities.dynamic_scene", "diyScene")
    }

    pub fn has_snapshots(&self) -> bool {
        self.has_capability("devices.capabilities.dynamic_scene", "snapshot")
    }

    pub fn has_gradient_toggle(&self) -> bool {
        self.has_capability("devices.capabilities.toggle", "gradientToggle")
    }

    pub fn has_dreamview_toggle(&self) -> bool {
        self.has_capability("devices.capabilities.toggle", "dreamViewToggle")
    }

    pub fn has_segment_color(&self) -> bool {
        self.has_capability(
            "devices.capabilities.segment_color_setting",
            "segmentedColorRgb",
        )
    }

    pub fn has_music_mode(&self) -> bool {
        self.has_capability("devices.capabilities.music_setting", "musicMode")
    }
}
