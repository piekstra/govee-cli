use serde_json::json;

// -- Scene matching tests --

#[test]
fn normalize_regular_space() {
    assert_eq!(
        govee::cli::scene::normalize_for_match("Milky Way"),
        "milky way"
    );
}

#[test]
fn normalize_nbsp() {
    // \u{00a0} is the non-breaking space Govee uses in some scene names
    assert_eq!(
        govee::cli::scene::normalize_for_match("Milky\u{00a0}Way"),
        "milky way"
    );
}

#[test]
fn normalize_mixed_whitespace() {
    assert_eq!(
        govee::cli::scene::normalize_for_match("Sunset\t\u{00a0}Glow"),
        "sunset  glow"
    );
}

#[test]
fn normalize_already_lowercase() {
    assert_eq!(govee::cli::scene::normalize_for_match("aurora"), "aurora");
}

#[test]
fn normalize_empty_string() {
    assert_eq!(govee::cli::scene::normalize_for_match(""), "");
}

#[test]
fn normalize_nbsp_matches_regular_space_input() {
    let scene_name = "Milky\u{00a0}Way";
    let user_input = "Milky Way";
    assert_eq!(
        govee::cli::scene::normalize_for_match(scene_name),
        govee::cli::scene::normalize_for_match(user_input)
    );
}

// -- Scene extraction tests --

#[test]
fn extract_scenes_from_api_response() {
    let data = json!({
        "capabilities": [{
            "type": "devices.capabilities.dynamic_scene",
            "instance": "lightScene",
            "parameters": {
                "dataType": "ENUM",
                "options": [
                    {"name": "Aurora", "value": {"paramId": 1, "id": 100}},
                    {"name": "Sunset Glow", "value": {"paramId": 2, "id": 101}},
                    {"name": "Milky\u{00a0}Way", "value": {"paramId": 3, "id": 102}}
                ]
            }
        }]
    });
    let scenes = govee::cli::scene::extract_scene_names(&data);
    assert_eq!(scenes.len(), 3);
    assert_eq!(scenes[0]["name"], "Aurora");
    assert_eq!(scenes[1]["name"], "Sunset Glow");
    assert_eq!(scenes[2]["name"], "Milky\u{00a0}Way");
}

#[test]
fn extract_scenes_empty_options() {
    let data = json!({
        "capabilities": [{
            "type": "devices.capabilities.dynamic_scene",
            "instance": "lightScene",
            "parameters": {
                "dataType": "ENUM",
                "options": []
            }
        }]
    });
    let scenes = govee::cli::scene::extract_scene_names(&data);
    assert_eq!(scenes.len(), 0);
}

#[test]
fn extract_scenes_no_capabilities() {
    let data = json!({});
    let scenes = govee::cli::scene::extract_scene_names(&data);
    assert_eq!(scenes.len(), 0);
}

#[test]
fn extract_scenes_multiple_capability_groups() {
    let data = json!({
        "capabilities": [
            {
                "type": "devices.capabilities.dynamic_scene",
                "instance": "lightScene",
                "parameters": {
                    "dataType": "ENUM",
                    "options": [
                        {"name": "Aurora", "value": {"paramId": 1, "id": 100}}
                    ]
                }
            },
            {
                "type": "devices.capabilities.dynamic_scene",
                "instance": "diyScene",
                "parameters": {
                    "dataType": "ENUM",
                    "options": [
                        {"name": "My Scene", "value": {"paramId": 10, "id": 200}}
                    ]
                }
            }
        ]
    });
    let scenes = govee::cli::scene::extract_scene_names(&data);
    assert_eq!(scenes.len(), 2);
}

#[test]
fn extract_scene_preserves_value() {
    let data = json!({
        "capabilities": [{
            "parameters": {
                "options": [
                    {"name": "Test", "value": {"paramId": 42, "id": 99}}
                ]
            }
        }]
    });
    let scenes = govee::cli::scene::extract_scene_names(&data);
    assert_eq!(scenes[0]["value"]["paramId"], 42);
    assert_eq!(scenes[0]["value"]["id"], 99);
}

// -- Hex color parsing tests --

#[test]
fn parse_hex_with_hash() {
    let (r, g, b) = govee::cli::light::parse_hex_color("#FF0000").unwrap();
    assert_eq!((r, g, b), (255, 0, 0));
}

#[test]
fn parse_hex_without_hash() {
    let (r, g, b) = govee::cli::light::parse_hex_color("00FF00").unwrap();
    assert_eq!((r, g, b), (0, 255, 0));
}

#[test]
fn parse_hex_blue() {
    let (r, g, b) = govee::cli::light::parse_hex_color("#0000FF").unwrap();
    assert_eq!((r, g, b), (0, 0, 255));
}

#[test]
fn parse_hex_mixed() {
    let (r, g, b) = govee::cli::light::parse_hex_color("#FF8040").unwrap();
    assert_eq!((r, g, b), (255, 128, 64));
}

#[test]
fn parse_hex_lowercase() {
    let (r, g, b) = govee::cli::light::parse_hex_color("ff8040").unwrap();
    assert_eq!((r, g, b), (255, 128, 64));
}

#[test]
fn parse_hex_black() {
    let (r, g, b) = govee::cli::light::parse_hex_color("000000").unwrap();
    assert_eq!((r, g, b), (0, 0, 0));
}

#[test]
fn parse_hex_white() {
    let (r, g, b) = govee::cli::light::parse_hex_color("FFFFFF").unwrap();
    assert_eq!((r, g, b), (255, 255, 255));
}

#[test]
fn parse_hex_too_short() {
    assert!(govee::cli::light::parse_hex_color("FFF").is_err());
}

#[test]
fn parse_hex_too_long() {
    assert!(govee::cli::light::parse_hex_color("#FF00FF00").is_err());
}

#[test]
fn parse_hex_invalid_chars() {
    assert!(govee::cli::light::parse_hex_color("GGHHII").is_err());
}

#[test]
fn parse_hex_empty() {
    assert!(govee::cli::light::parse_hex_color("").is_err());
}

// -- Device type tests --

#[test]
fn device_type_from_known_sku() {
    use govee::models::device_type::DeviceType;
    assert_eq!(DeviceType::from_sku("H60B0"), DeviceType::H60B0);
    assert_eq!(DeviceType::from_sku("H6601"), DeviceType::H6601);
    assert_eq!(DeviceType::from_sku("H618E"), DeviceType::H618E);
    assert_eq!(DeviceType::from_sku("H6057"), DeviceType::H6057);
}

#[test]
fn device_type_from_sku_with_suffix() {
    use govee::models::device_type::DeviceType;
    assert_eq!(DeviceType::from_sku("H60B0111"), DeviceType::H60B0);
    assert_eq!(DeviceType::from_sku("H60B0A11"), DeviceType::H60B0);
}

#[test]
fn device_type_unknown_sku() {
    use govee::models::device_type::DeviceType;
    assert_eq!(DeviceType::from_sku("H9999"), DeviceType::Unknown);
}

#[test]
fn device_type_new_skus() {
    use govee::models::device_type::DeviceType;
    assert_eq!(DeviceType::from_sku("H6004"), DeviceType::H6004);
    assert_eq!(DeviceType::from_sku("H6008"), DeviceType::H6008);
    assert_eq!(DeviceType::from_sku("H6022"), DeviceType::H6022);
    assert_eq!(DeviceType::from_sku("H6046"), DeviceType::H6046);
    assert_eq!(DeviceType::from_sku("H6056"), DeviceType::H6056);
    assert_eq!(DeviceType::from_sku("H6076"), DeviceType::H6076);
    assert_eq!(DeviceType::from_sku("H6099"), DeviceType::H6099);
    assert_eq!(DeviceType::from_sku("H61A8"), DeviceType::H61A8);
}

#[test]
fn device_type_group_skus() {
    use govee::models::device_type::DeviceType;
    assert_eq!(DeviceType::from_sku("BaseGroup"), DeviceType::BaseGroup);
    assert_eq!(
        DeviceType::from_sku("SameModeGroup"),
        DeviceType::SameModeGroup
    );
}

#[test]
fn device_type_category() {
    use govee::models::device_type::DeviceType;
    assert_eq!(DeviceType::H60B0.category(), "floor_lamp");
    assert_eq!(DeviceType::H6601.category(), "led_strip");
    assert_eq!(DeviceType::H6057.category(), "light_bar");
    assert_eq!(DeviceType::H6004.category(), "bulb");
    assert_eq!(DeviceType::H6008.category(), "bulb");
    assert_eq!(DeviceType::H6022.category(), "table_lamp");
    assert_eq!(DeviceType::H6046.category(), "light_bar");
    assert_eq!(DeviceType::H6076.category(), "panel");
    assert_eq!(DeviceType::H6099.category(), "tv_backlight");
    assert_eq!(DeviceType::H61A8.category(), "neon_light");
    assert_eq!(DeviceType::BaseGroup.category(), "group");
    assert_eq!(DeviceType::SameModeGroup.category(), "group");
    assert_eq!(DeviceType::Unknown.category(), "unknown");
}

#[test]
fn device_type_display_name() {
    use govee::models::device_type::DeviceType;
    assert_eq!(DeviceType::H60B0.display_name(), "Uplighter Floor Lamp");
    assert_eq!(DeviceType::H6004.display_name(), "Smart Bulb");
    assert_eq!(DeviceType::H6022.display_name(), "Table Lamp");
    assert_eq!(DeviceType::H6046.display_name(), "RGBIC Light Bar");
    assert_eq!(DeviceType::H6076.display_name(), "RGBIC Panel");
    assert_eq!(DeviceType::H6099.display_name(), "TV Backlight");
    assert_eq!(DeviceType::H61A8.display_name(), "Neon Rope Light");
    assert_eq!(DeviceType::BaseGroup.display_name(), "Device Group");
    assert_eq!(DeviceType::SameModeGroup.display_name(), "Sync Group");
    assert_eq!(DeviceType::Unknown.display_name(), "Unknown Device");
}

// -- DeviceInfo capability tests --

#[test]
fn device_info_has_capability() {
    let info: govee::models::device_info::DeviceInfo = serde_json::from_value(json!({
        "sku": "H60B0",
        "device": "AA:BB:CC:DD",
        "deviceName": "Test Lamp",
        "capabilities": [
            {"type": "devices.capabilities.on_off", "instance": "powerSwitch", "parameters": {}},
            {"type": "devices.capabilities.range", "instance": "brightness", "parameters": {}},
            {"type": "devices.capabilities.color_setting", "instance": "colorRgb", "parameters": {}},
            {"type": "devices.capabilities.color_setting", "instance": "colorTemperatureK", "parameters": {}},
            {"type": "devices.capabilities.dynamic_scene", "instance": "lightScene", "parameters": {}},
            {"type": "devices.capabilities.dynamic_scene", "instance": "diyScene", "parameters": {}},
            {"type": "devices.capabilities.dynamic_scene", "instance": "snapshot", "parameters": {}},
            {"type": "devices.capabilities.toggle", "instance": "gradientToggle", "parameters": {}},
            {"type": "devices.capabilities.toggle", "instance": "dreamViewToggle", "parameters": {}},
            {"type": "devices.capabilities.segment_color_setting", "instance": "segmentedColorRgb", "parameters": {}},
            {"type": "devices.capabilities.music_setting", "instance": "musicMode", "parameters": {}}
        ]
    }))
    .unwrap();

    assert!(info.has_power());
    assert!(info.has_brightness());
    assert!(info.has_color_rgb());
    assert!(info.has_color_temp());
    assert!(info.has_scenes());
    assert!(info.has_diy_scenes());
    assert!(info.has_snapshots());
    assert!(info.has_gradient_toggle());
    assert!(info.has_dreamview_toggle());
    assert!(info.has_segment_color());
    assert!(info.has_music_mode());
}

#[test]
fn device_info_missing_capability() {
    let info: govee::models::device_info::DeviceInfo = serde_json::from_value(json!({
        "sku": "H6008",
        "device": "AA:BB:CC:DD",
        "deviceName": "Basic Bulb",
        "capabilities": [
            {"type": "devices.capabilities.on_off", "instance": "powerSwitch", "parameters": {}}
        ]
    }))
    .unwrap();

    assert!(info.has_power());
    assert!(!info.has_brightness());
    assert!(!info.has_color_rgb());
    assert!(!info.has_color_temp());
    assert!(!info.has_scenes());
    assert!(!info.has_diy_scenes());
    assert!(!info.has_snapshots());
    assert!(!info.has_gradient_toggle());
    assert!(!info.has_dreamview_toggle());
    assert!(!info.has_segment_color());
    assert!(!info.has_music_mode());
}

#[test]
fn device_info_no_capabilities() {
    let info: govee::models::device_info::DeviceInfo = serde_json::from_value(json!({
        "sku": "BaseGroup",
        "device": "12345",
        "deviceName": "My Group"
    }))
    .unwrap();

    assert!(!info.has_power());
    assert!(info.capabilities.is_empty());
}

#[test]
fn device_info_deserialization() {
    let info: govee::models::device_info::DeviceInfo = serde_json::from_value(json!({
        "sku": "H60B0",
        "device": "14:DF:DD:99:83:06:19:44",
        "deviceName": "Living Room Light 1",
        "capabilities": []
    }))
    .unwrap();

    assert_eq!(info.name(), "Living Room Light 1");
    assert_eq!(info.id(), "14:DF:DD:99:83:06:19:44");
    assert_eq!(info.model(), "H60B0");
}

// -- Capability type tests --

#[test]
fn capability_type_round_trip() {
    use govee::models::capability::CapabilityType;
    let types = vec![
        ("devices.capabilities.on_off", CapabilityType::OnOff),
        ("devices.capabilities.range", CapabilityType::Range),
        (
            "devices.capabilities.color_setting",
            CapabilityType::ColorSetting,
        ),
        (
            "devices.capabilities.dynamic_scene",
            CapabilityType::DynamicScene,
        ),
        ("devices.capabilities.toggle", CapabilityType::Toggle),
        ("devices.capabilities.mode", CapabilityType::Mode),
        ("devices.capabilities.work_mode", CapabilityType::WorkMode),
        ("devices.capabilities.online", CapabilityType::Online),
        (
            "devices.capabilities.music_setting",
            CapabilityType::MusicSetting,
        ),
        (
            "devices.capabilities.segment_color_setting",
            CapabilityType::SegmentColorSetting,
        ),
    ];
    for (api_str, expected) in types {
        let parsed = CapabilityType::from_api_type(api_str);
        assert_eq!(parsed, expected);
        assert_eq!(parsed.api_type(), api_str);
    }
}

#[test]
fn capability_type_unknown_preserved() {
    use govee::models::capability::CapabilityType;
    let parsed = CapabilityType::from_api_type("devices.capabilities.future_thing");
    assert_eq!(
        parsed,
        CapabilityType::Unknown("devices.capabilities.future_thing".to_string())
    );
    assert_eq!(parsed.api_type(), "devices.capabilities.future_thing");
}

// -- Error type tests --

#[test]
fn error_exit_codes() {
    use govee::error::AppError;
    assert_eq!(AppError::NotAuthenticated.exit_code(), 2);
    assert_eq!(AppError::DeviceNotFound("x".into()).exit_code(), 3);
    assert_eq!(AppError::RateLimited("x".into()).exit_code(), 4);
    assert_eq!(AppError::InvalidInput("x".into()).exit_code(), 1);
}

#[test]
fn error_json_format() {
    use govee::error::AppError;
    let err = AppError::DeviceNotFound("My Lamp".into());
    let j = err.to_json();
    assert_eq!(j["error"], "device_not_found");
    assert!(j["message"].as_str().unwrap().contains("My Lamp"));
}

#[test]
fn error_api_includes_error_code() {
    use govee::error::AppError;
    let err = AppError::Api {
        message: "Bad request".into(),
        error_code: Some(400),
    };
    let j = err.to_json();
    assert_eq!(j["error"], "api");
    assert_eq!(j["error_code"], 400);
}
