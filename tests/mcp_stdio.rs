//! `govee mcp` end to end: the real binary over stdio, as Claude Code and Claude Desktop
//! launch it, against a stub of Govee's Platform API on 127.0.0.1 that answers from the
//! scrubbed fixtures and remembers what it was told. The key comes from `$GOVEE_API_KEY`, so
//! nothing here touches a keychain, and the live path runs for real: config, key, client, the
//! server's blocking pool. No request leaves this machine and no device exists: a debug binary
//! takes `GOVEE_TEST_BASE_URL`, and a release binary does not.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

const TOOLS: [&str; 5] = [
    "govee_devices",
    "govee_device_state",
    "govee_scenes",
    "govee_power",
    "govee_light",
];
const KEY: &str = "dummy-govee-key-for-the-stub";
const LAMP: &str = "Office Floor Lamp";
const HEATER: &str = "Bedroom Heater";

/// A scratch home and config, so a test never reads the developer's real settings.
fn govee(test: &str, base_url: Option<&str>) -> Command {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("mcp-stdio-{test}"));
    std::fs::create_dir_all(&root).unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_govee"));
    match base_url {
        Some(url) => c.env("GOVEE_TEST_BASE_URL", url),
        None => c.env_remove("GOVEE_TEST_BASE_URL"),
    };
    c.env_remove("GOVEE_API_KEY")
        .env("GOVEE_CONFIG", root.join("config.json"))
        .env("XDG_CONFIG_HOME", &root)
        .env("HOME", &root)
        .env("USERPROFILE", &root);
    c
}

fn fixture(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// One request the stub received.
#[derive(Debug, Clone)]
struct Seen {
    method: String,
    path: String,
    body: Value,
    has_key: bool,
}

/// The light's state as the stub holds it, and every request it saw.
#[derive(Default)]
struct Stub {
    seen: Vec<Seen>,
    state: Option<Value>,
    /// Answer every request with this envelope instead.
    refuse_with: Option<Value>,
    /// Apply a command, and answer it with something that is not JSON.
    garble_control: bool,
}

type Shared = Arc<Mutex<Stub>>;

fn stub_api(refuse_with: Option<Value>) -> (String, Shared) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let shared: Shared = Arc::new(Mutex::new(Stub {
        state: Some(fixture("platform-state.json")),
        refuse_with,
        ..Stub::default()
    }));
    let stub = shared.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let mut parts = line.split(' ');
            let method = parts.next().unwrap_or("").to_string();
            let path = parts.next().unwrap_or("").to_string();
            let (mut has_key, mut length) = (false, 0usize);
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header.trim().is_empty() {
                    break;
                }
                let lower = header.to_ascii_lowercase();
                has_key |= lower.starts_with("govee-api-key:") && header.contains(KEY);
                if let Some(n) = lower.strip_prefix("content-length:") {
                    length = n.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            let answer = answer(&stub, &method, &path, &body, has_key).to_string();
            let garbled = stub.lock().unwrap().garble_control && path == "/device/control";
            let answer = if garbled {
                "<html>bad gateway</html>".to_string()
            } else {
                answer
            };
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{answer}",
                answer.len()
            );
        }
    });
    (url, shared)
}

fn answer(stub: &Shared, method: &str, path: &str, body: &Value, has_key: bool) -> Value {
    let mut stub = stub.lock().unwrap();
    stub.seen.push(Seen {
        method: method.to_string(),
        path: path.to_string(),
        body: body.clone(),
        has_key,
    });
    if let Some(refusal) = &stub.refuse_with {
        return refusal.clone();
    }
    let payload = |p: Value| json!({"code": 200, "msg": "success", "payload": p});
    match (method, path) {
        ("GET", "/user/devices") => {
            let mut devices = fixture("platform-devices.json");
            let other = fixture("platform-devices-other.json");
            devices
                .as_array_mut()
                .unwrap()
                .extend(other.as_array().unwrap().clone());
            json!({"code": 200, "message": "success", "data": devices})
        }
        ("POST", "/device/state") => payload(stub.state.clone().unwrap()),
        ("POST", "/device/scenes") => payload(fixture("platform-scenes.json")),
        ("POST", "/device/control") => {
            let capability = &body["payload"]["capability"];
            let state = stub.state.as_mut().unwrap();
            for cap in state["capabilities"].as_array_mut().unwrap() {
                if cap["instance"] == capability["instance"] {
                    cap["state"]["value"] = capability["value"].clone();
                }
            }
            json!({"code": 200, "message": "success", "capability": capability})
        }
        _ => json!({"code": 404, "message": "no such route in the stub"}),
    }
}

struct Server {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
}

impl Server {
    fn start(mut command: Command) -> Self {
        let mut child = command
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("govee starts");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut server = Self {
            child,
            stdin,
            lines,
        };
        server.request(
            0,
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "0"}
            }),
        );
        server
    }

    fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(self.stdin, "{msg}").unwrap();
        self.stdin.flush().unwrap();
        loop {
            let line = self
                .lines
                .recv_timeout(Duration::from_secs(30))
                .expect("a response within 30 s");
            let msg: Value = serde_json::from_str(&line)
                .unwrap_or_else(|e| panic!("non-JSON on stdout ({e}): {line}"));
            if msg["id"] == json!(id) {
                return msg;
            }
        }
    }

    fn call(&mut self, id: u64, tool: &str, arguments: Value) -> Value {
        self.request(
            id,
            "tools/call",
            json!({"name": tool, "arguments": arguments}),
        )
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn structured(call: &Value) -> &Value {
    assert!(call["result"]["isError"] != true, "{call}");
    &call["result"]["structuredContent"]
}

fn error_text(call: &Value) -> &str {
    assert_eq!(call["result"]["isError"], true, "{call}");
    call["result"]["content"][0]["text"].as_str().unwrap()
}

fn tool_names(list: &Value) -> Vec<&str> {
    list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect()
}

#[test]
fn tools_are_listed_with_no_key_no_config_and_no_network() {
    let mut server = Server::start(govee("list", None));
    let list = server.request(1, "tools/list", json!({}));
    assert_eq!(tool_names(&list), TOOLS);
}

#[test]
fn a_call_with_no_key_asks_for_a_sign_in_and_does_not_prompt() {
    let mut server = Server::start(govee("no-key", None));
    let call = server.call(1, "govee_devices", json!({}));
    let text = error_text(&call);
    assert!(text.starts_with("auth:"), "{text}");
    assert!(text.contains("`govee auth login`"), "{text}");
}

#[test]
fn the_list_flag_prints_the_tools_and_the_key_is_not_needed() {
    let out = govee("list-flag", None)
        .args(["mcp", "--list-tools"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let got: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(got["tools"].as_array().unwrap().len(), TOOLS.len());
}

/// Every tool, through the real client: what reaches Govee is the four routes and six
/// capabilities a tool may use, the key is only ever in its header, and a write returns the
/// state read back from Govee.
#[test]
fn the_tools_reach_govee_only_through_the_routes_they_need() {
    let (url, stub) = stub_api(None);
    let mut command = govee("all-tools", Some(&url));
    command.env("GOVEE_API_KEY", KEY);
    let mut server = Server::start(command);

    let devices = server.call(1, "govee_devices", json!({}));
    let devices = structured(&devices);
    assert_eq!(devices["items"][0]["name"], LAMP);
    assert!(devices["items"][0].get("device_id").is_none(), "{devices}");

    let state = server.call(2, "govee_device_state", json!({"device": "office"}));
    assert_eq!(structured(&state)["power"], "on");

    let scenes = server.call(3, "govee_scenes", json!({"device": LAMP}));
    assert_eq!(structured(&scenes)["items"][0]["name"], "Sunrise");

    let off = server.call(4, "govee_power", json!({"device": LAMP, "power": "off"}));
    let off = structured(&off);
    assert_eq!(off["confirmed"], true, "{off}");
    assert_eq!(off["device"]["power"], "off");

    let dim = server.call(5, "govee_light", json!({"device": LAMP, "brightness": 30}));
    assert_eq!(structured(&dim)["device"]["brightness"], 30);
    let warm = server.call(
        6,
        "govee_light",
        json!({"device": LAMP, "color_temp_k": 2700}),
    );
    assert_eq!(structured(&warm)["device"]["color_temp_k"], 2700);
    let red = server.call(
        7,
        "govee_light",
        json!({"device": LAMP, "color": "#FF0000"}),
    );
    assert_eq!(structured(&red)["device"]["color"], "#FF0000");
    let scene = server.call(8, "govee_light", json!({"device": LAMP, "scene": "Forest"}));
    assert_eq!(structured(&scene)["confirmed"], false);
    let snapshot = server.call(
        9,
        "govee_light",
        json!({"device": LAMP, "scene": "Movie Night"}),
    );
    assert_eq!(structured(&snapshot)["requested"], "snapshot Movie Night");

    for result in [&off, &devices] {
        assert!(!result.to_string().contains(KEY));
    }

    let seen = stub.lock().unwrap().seen.clone();
    let mut sent = Vec::new();
    for request in &seen {
        assert!(request.has_key, "{request:?}");
        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/user/devices") | ("POST", "/device/state") | ("POST", "/device/scenes") => {}
            ("POST", "/device/control") => {
                let payload = &request.body["payload"];
                assert_eq!(payload["device"], "AA:BB:CC:DD:EE:FF:00:11", "{request:?}");
                let capability = &payload["capability"];
                sent.push((
                    capability["type"].as_str().unwrap().to_string(),
                    capability["instance"].as_str().unwrap().to_string(),
                ));
            }
            other => panic!("a tool reached {other:?}"),
        }
    }
    let expected: Vec<(String, String)> = [
        ("on_off", "powerSwitch"),
        ("range", "brightness"),
        ("color_setting", "colorTemperatureK"),
        ("color_setting", "colorRgb"),
        ("dynamic_scene", "lightScene"),
        ("dynamic_scene", "snapshot"),
    ]
    .iter()
    .map(|(t, i)| (format!("devices.capabilities.{t}"), i.to_string()))
    .collect();
    assert_eq!(sent, expected);
}

#[test]
fn a_refused_write_sends_no_command() {
    let (url, stub) = stub_api(None);
    let mut command = govee("refused", Some(&url));
    command.env("GOVEE_API_KEY", KEY);
    let mut server = Server::start(command);

    for (id, args) in [
        (1, json!({"device": "all", "power": "off"})),
        (2, json!({"device": "hall", "power": "off"})),
        (3, json!({"device": HEATER, "power": "on"})),
        (4, json!({"device": LAMP, "power": "off", "force": true})),
    ] {
        let call = server.call(id, "govee_power", args);
        assert_eq!(call["result"]["isError"], true, "{call}");
    }
    let seen = stub.lock().unwrap().seen.clone();
    assert!(seen.iter().all(|r| r.path != "/device/control"), "{seen:?}");
}

/// The command took effect and the answer was lost: a real client's decode error, through to
/// a result that reports the state and does not ask for the command again.
#[test]
fn a_garbled_answer_to_a_command_is_settled_by_the_state() {
    let (url, stub) = stub_api(None);
    stub.lock().unwrap().garble_control = true;
    let mut command = govee("garbled", Some(&url));
    command.env("GOVEE_API_KEY", KEY);
    let mut server = Server::start(command);

    let call = server.call(1, "govee_power", json!({"device": LAMP, "power": "off"}));
    let got = structured(&call);
    assert_eq!(got["confirmed"], true, "{got}");
    assert_eq!(got["device"]["power"], "off");

    // A scene cannot be read back, so this one stays unknown, and says so with no URL.
    let call = server.call(2, "govee_light", json!({"device": LAMP, "scene": "Forest"}));
    let got = structured(&call);
    assert_eq!(got["confirmed"], false, "{got}");
    let note = got["note"].as_str().unwrap();
    assert!(note.contains("may or may not have taken effect"), "{note}");
    assert!(!note.contains("127.0.0.1"), "{note}");
}

/// `auth login` would write the keychain, which the variable wins over.
#[test]
fn a_rejected_key_points_at_where_it_was_set() {
    let (url, _stub) = stub_api(Some(json!({"code": 401, "message": "Unauthorized"})));
    let mut command = govee("rejected", Some(&url));
    command.env("GOVEE_API_KEY", KEY);
    let mut server = Server::start(command);
    let call = server.call(1, "govee_devices", json!({}));
    let text = error_text(&call);
    assert!(text.starts_with("auth:"), "{text}");
    assert!(text.contains("GOVEE_API_KEY"), "{text}");
    assert!(!text.contains("auth login"), "{text}");
    assert!(!text.contains(KEY), "{text}");
}

#[test]
fn a_limited_or_failing_api_is_upstream_and_names_no_url() {
    for (name, refusal) in [
        ("limited", json!({"code": 429, "message": "too many"})),
        ("failing", json!({"code": 500, "message": "internal"})),
    ] {
        let (url, _stub) = stub_api(Some(refusal));
        let mut command = govee(name, Some(&url));
        command.env("GOVEE_API_KEY", KEY);
        let mut server = Server::start(command);
        let call = server.call(1, "govee_devices", json!({}));
        let text = error_text(&call);
        assert!(text.starts_with("upstream:"), "{text}");
        assert!(!text.contains("127.0.0.1"), "{text}");
    }
}

#[test]
fn nothing_listening_is_upstream_and_names_no_url() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let mut command = govee("closed", Some(&url));
    command.env("GOVEE_API_KEY", KEY);
    let mut server = Server::start(command);
    let call = server.call(1, "govee_devices", json!({}));
    let text = error_text(&call);
    assert!(text.starts_with("upstream:"), "{text}");
    assert!(!text.contains("127.0.0.1"), "{text}");
}
