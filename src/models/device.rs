use serde_json::json;

use crate::api::client::GoveeApi;
use crate::error::AppError;
use crate::models::device_info::DeviceInfo;
use crate::models::device_type::DeviceType;

/// A resolved device ready for control operations.
pub struct Device {
    api: GoveeApi,
    pub info: DeviceInfo,
    pub device_type: DeviceType,
}

impl Device {
    pub fn new(api: GoveeApi, info: DeviceInfo, device_type: DeviceType) -> Self {
        Self {
            api,
            info,
            device_type,
        }
    }

    pub fn name(&self) -> &str {
        self.info.name()
    }

    pub fn sku(&self) -> &str {
        self.info.model()
    }

    pub fn device_id(&self) -> &str {
        self.info.id()
    }

    // -- Power --

    pub async fn power_on(&self) -> Result<(), AppError> {
        self.require_capability("devices.capabilities.on_off", "powerSwitch")?;
        self.api
            .control_device(
                self.sku(),
                self.device_id(),
                "devices.capabilities.on_off",
                "powerSwitch",
                json!(1),
            )
            .await
    }

    pub async fn power_off(&self) -> Result<(), AppError> {
        self.require_capability("devices.capabilities.on_off", "powerSwitch")?;
        self.api
            .control_device(
                self.sku(),
                self.device_id(),
                "devices.capabilities.on_off",
                "powerSwitch",
                json!(0),
            )
            .await
    }

    // -- Brightness --

    pub async fn set_brightness(&self, level: u8) -> Result<(), AppError> {
        if level == 0 || level > 100 {
            return Err(AppError::InvalidInput(
                "Brightness must be between 1 and 100".to_string(),
            ));
        }
        self.require_capability("devices.capabilities.range", "brightness")?;
        self.api
            .control_device(
                self.sku(),
                self.device_id(),
                "devices.capabilities.range",
                "brightness",
                json!(level),
            )
            .await
    }

    // -- Color --

    pub async fn set_color_rgb(&self, r: u8, g: u8, b: u8) -> Result<(), AppError> {
        self.require_capability("devices.capabilities.color_setting", "colorRgb")?;
        let packed: u32 = (r as u32) * 65536 + (g as u32) * 256 + (b as u32);
        self.api
            .control_device(
                self.sku(),
                self.device_id(),
                "devices.capabilities.color_setting",
                "colorRgb",
                json!(packed),
            )
            .await
    }

    pub async fn set_color_temp(&self, kelvin: u16) -> Result<(), AppError> {
        if kelvin < 2000 || kelvin > 9000 {
            return Err(AppError::InvalidInput(
                "Color temperature must be between 2000 and 9000 Kelvin".to_string(),
            ));
        }
        self.require_capability("devices.capabilities.color_setting", "colorTemperatureK")?;
        self.api
            .control_device(
                self.sku(),
                self.device_id(),
                "devices.capabilities.color_setting",
                "colorTemperatureK",
                json!(kelvin),
            )
            .await
    }

    // -- Scenes --

    pub async fn activate_scene(
        &self,
        param_id: serde_json::Value,
        id: serde_json::Value,
    ) -> Result<(), AppError> {
        self.require_capability("devices.capabilities.dynamic_scene", "lightScene")?;
        self.api
            .control_device(
                self.sku(),
                self.device_id(),
                "devices.capabilities.dynamic_scene",
                "lightScene",
                json!({"paramId": param_id, "id": id}),
            )
            .await
    }

    pub async fn get_scenes(&self) -> Result<serde_json::Value, AppError> {
        self.api
            .get_device_scenes(self.sku(), self.device_id())
            .await
    }

    pub async fn get_diy_scenes(&self) -> Result<serde_json::Value, AppError> {
        self.api
            .get_device_diy_scenes(self.sku(), self.device_id())
            .await
    }

    // -- State --

    pub async fn get_state(&self) -> Result<serde_json::Value, AppError> {
        self.api
            .get_device_state(self.sku(), self.device_id())
            .await
    }

    // -- Capability check --

    fn require_capability(&self, cap_type: &str, instance: &str) -> Result<(), AppError> {
        if !self.info.has_capability(cap_type, instance) {
            return Err(AppError::UnsupportedOperation(format!(
                "{} ({}) does not support {}/{}",
                self.name(),
                self.sku(),
                cap_type,
                instance
            )));
        }
        Ok(())
    }
}
