//! Contract tests over the wire shapes in `tests/fixtures/`: the fields the
//! parsers read must be where they are expected, and every fixture must be
//! scrubbed per `tests/fixtures/README.md`.

use govee::api::app::{app_devices, merge_app_view, Connectivity, PlatformDevice};
use govee::cli::rooms::{find_device, find_room, members_of, rooms_of};
use govee::models::device_info::DeviceInfo;
use govee::models::device_type::DeviceType;
use serde_json::Value;

fn fixture(rel: &str) -> Value {
    let path = format!("{}/tests/fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parsing {path}: {e}"))
}

#[test]
fn platform_devices_parse_with_their_capabilities() {
    let v = fixture("platform-devices.json");
    let devices: Vec<DeviceInfo> = serde_json::from_value(v).expect("device list parses");
    assert_eq!(devices.len(), 2);
    let lamp = &devices[0];
    assert_eq!(lamp.name(), "Office Floor Lamp");
    assert_eq!(DeviceType::from_sku(lamp.model()), DeviceType::H60B0);
    assert!(lamp.has_power() && lamp.has_brightness() && lamp.has_color_rgb());
    assert!(lamp.has_color_temp() && lamp.has_scenes() && lamp.has_snapshots());
    assert!(lamp.has_gradient_toggle() && lamp.has_segment_color() && lamp.has_music_mode());
    let bulb = &devices[1];
    assert!(bulb.has_power() && bulb.has_brightness());
    assert!(!bulb.has_scenes() && !bulb.has_music_mode());
}

#[test]
fn scene_and_music_options_come_out_of_the_capability_parameters() {
    let v = fixture("platform-devices.json");
    let devices: Vec<DeviceInfo> = serde_json::from_value(v).unwrap();
    let lamp = &devices[0];
    // `/device/scenes` answers with the same capability shape the device
    // list carries, so the extractor can be exercised on the fixture.
    let scenes = govee::cli::scene::extract_scene_names(&serde_json::json!({
        "capabilities": lamp.capabilities
    }));
    let names: Vec<&str> = scenes.iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"Sunrise") && names.contains(&"Evening"));
    let modes = govee::cli::music::extract_music_modes(&lamp.capabilities);
    assert_eq!(modes.len(), 2);
    assert_eq!(modes[0]["name"], "Energic");
    assert_eq!(modes[0]["value"], 1);
}

#[test]
fn app_list_yields_rooms_connectivity_and_membership() {
    let list = fixture("app-device-list.json");
    let rooms = rooms_of(&list);
    assert_eq!(rooms.len(), 2);
    assert_eq!(find_room(&rooms, "office").unwrap().id, 1001);
    let devices = app_devices(&list);
    assert_eq!(devices.len(), 4);
    let by = |id: &str| devices.iter().find(|d| d.device == id).unwrap();
    assert_eq!(
        by("AA:BB:CC:DD:EE:FF:00:11").connectivity,
        Connectivity::Wifi
    );
    assert_eq!(
        by("AA:BB:CC:DD:EE:FF:00:11").room.as_deref(),
        Some("Office")
    );
    assert_eq!(
        by("11:22:33:44:55:66:77:88").connectivity,
        Connectivity::Wifi
    );
    assert_eq!(
        by("22:33:44:55:66:77:88:99").connectivity,
        Connectivity::Bluetooth
    );
    assert!(
        by("33:44:55:66:77:88:99:AA").room.is_none(),
        "groupId 0 is no room"
    );
    assert_eq!(members_of(&devices, 1001).len(), 2);
    assert_eq!(
        find_device(&devices, "H617A_22:33:44:55:66:77:88:99")
            .unwrap()
            .name,
        "Desk Strip"
    );
}

#[test]
fn platform_and_app_views_merge_into_one_device_table() {
    let platform: Vec<DeviceInfo> =
        serde_json::from_value(fixture("platform-devices.json")).unwrap();
    let platform: Vec<PlatformDevice> = platform
        .iter()
        .map(|d| {
            let t = DeviceType::from_sku(d.model());
            PlatformDevice {
                device: d.id().to_string(),
                sku: d.model().to_string(),
                name: d.name().to_string(),
                kind: t.display_name().to_string(),
                category: t.category().to_string(),
            }
        })
        .collect();
    let app = app_devices(&fixture("app-device-list.json"));
    let rows = merge_app_view(&platform, Some(&app));
    // Two Platform devices gain rooms; the two app-only devices are appended.
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0].room.as_deref(), Some("Office"));
    assert_eq!(rows[1].room.as_deref(), Some("Hallway"));
    assert_eq!(rows[2].kind, "bluetooth-only");
    assert_eq!(rows[3].kind, "app-only");
    assert_eq!(rows[3].connectivity, Connectivity::Wifi);
}

#[test]
fn fixtures_carry_only_dummy_identities() {
    for name in ["platform-devices.json", "app-device-list.json"] {
        let mut strings = Vec::new();
        collect_strings(&fixture(name), &mut strings);
        for s in strings {
            if s.contains('@') {
                assert!(
                    s.ends_with("@example.com"),
                    "{name}: email `{s}` is not a dummy"
                );
            }
            assert!(
                !looks_like_uuid(&s),
                "{name}: `{s}` looks like an API key / UUID"
            );
            assert!(
                !s.starts_with("eyJ"),
                "{name}: `{s}` looks like a bearer token"
            );
            // Device settings are a JSON string; check inside it too.
            if let Ok(inner) = serde_json::from_str::<Value>(&s) {
                let mut nested = Vec::new();
                collect_strings(&inner, &mut nested);
                for n in nested {
                    assert!(
                        !n.contains("wifi")
                            || n == "ExampleNet"
                            || n.chars().all(|c| c.is_ascii_digit() || c == '.'),
                        "{name}: `{n}` is not the dummy Wi-Fi name"
                    );
                }
            }
        }
    }
}

fn collect_strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Array(a) => a.iter().for_each(|x| collect_strings(x, out)),
        Value::Object(m) => m.values().for_each(|x| collect_strings(x, out)),
        _ => {}
    }
}

fn looks_like_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts.iter().map(|p| p.len()).collect::<Vec<_>>() == [8, 4, 4, 4, 12]
        && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}
