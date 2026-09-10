//! Offline surface tests: flags, exit codes, and the JSON output contract.
//! No network, and nothing here reads the OS keychain: every command under
//! test either needs no credential or fails validation before looking for
//! one (SPEC §1.5), using a throwaway config with nothing configured and no
//! `$GOVEE_API_KEY` in the environment.

use assert_cmd::Command;
use predicates::prelude::*;

fn govee() -> Command {
    let mut c = Command::cargo_bin("govee").unwrap();
    // A config path that does not exist: no account, no "key stored" marker,
    // so any credentialed command stops with exit 3 before the keychain.
    c.env("GOVEE_CONFIG", "/nonexistent/govee-test-config.json");
    c.env_remove("GOVEE_API_KEY");
    c.env_remove("GOVEE_EMAIL");
    c.env_remove("GOVEE_PASSWORD");
    c.env_remove("NO_COLOR");
    c
}

fn json_stdout(out: &assert_cmd::assert::Assert) -> serde_json::Value {
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {stdout}"))
}

#[test]
fn help_lists_the_standard_and_domain_surface() {
    govee()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("auth"))
        .stdout(predicate::str::contains("config"))
        .stdout(predicate::str::contains("devices"))
        .stdout(predicate::str::contains("power"))
        .stdout(predicate::str::contains("light"))
        .stdout(predicate::str::contains("scene"))
        .stdout(predicate::str::contains("rooms"))
        .stdout(predicate::str::contains("api"))
        .stdout(predicate::str::contains("self-update"))
        .stdout(predicate::str::contains("completions"))
        .stdout(predicate::str::contains("info"))
        .stdout(predicate::str::contains("--json"))
        .stdout(predicate::str::contains("--quiet"))
        .stdout(predicate::str::contains("--no-color"))
        // The 0.1 flag is accepted but no longer advertised.
        .stdout(predicate::str::contains("--table").not());
}

#[test]
fn every_subcommand_help_renders() {
    // Catches clap runtime panics (e.g. a subcommand flag colliding with a
    // global like -q) that only surface when the subtree is built.
    for args in [
        vec!["auth", "--help"],
        vec!["auth", "login", "--help"],
        vec!["auth", "status", "--help"],
        vec!["auth", "logout", "--help"],
        vec!["auth", "set-credential", "--help"],
        vec!["auth", "login-account", "--help"],
        vec!["auth", "logout-account", "--help"],
        vec!["config", "--help"],
        vec!["config", "set", "--help"],
        vec!["devices", "--help"],
        vec!["devices", "list", "--help"],
        vec!["devices", "get", "--help"],
        vec!["devices", "search", "--help"],
        vec!["devices", "caps", "--help"],
        vec!["power", "on", "--help"],
        vec!["power", "off", "--help"],
        vec!["power", "toggle", "--help"],
        vec!["power", "status", "--help"],
        vec!["light", "brightness", "--help"],
        vec!["light", "color", "--help"],
        vec!["light", "temp", "--help"],
        vec!["light", "color-temp", "--help"],
        vec!["light", "state", "--help"],
        vec!["scene", "list", "--help"],
        vec!["scene", "list-diy", "--help"],
        vec!["scene", "list-snapshots", "--help"],
        vec!["scene", "activate", "--help"],
        vec!["scene", "activate-snapshot", "--help"],
        vec!["toggle", "gradient", "--help"],
        vec!["toggle", "dreamview", "--help"],
        vec!["toggle", "list", "--help"],
        vec!["segment", "color", "--help"],
        vec!["segment", "brightness", "--help"],
        vec!["segment", "info", "--help"],
        vec!["music", "list", "--help"],
        vec!["music", "set", "--help"],
        vec!["rooms", "list", "--help"],
        vec!["rooms", "devices", "--help"],
        vec!["rooms", "move", "--help"],
        vec!["rooms", "create", "--help"],
        vec!["rooms", "rename", "--help"],
        vec!["rooms", "delete", "--help"],
        vec!["api", "--help"],
        vec!["self-update", "--help"],
        vec!["completions", "--help"],
    ] {
        govee().args(&args).assert().success();
    }
}

#[test]
fn info_emits_cli_info_v1() {
    let out = govee().arg("info").assert().success();
    let v = json_stdout(&out);
    assert_eq!(v["schema"], "cli-info/v1");
    assert_eq!(v["name"], "govee");
    assert_eq!(v["spec"], "piekstra-cli/1");
    assert_eq!(v["repo"], "https://github.com/piekstra/govee-cli");
    assert_eq!(v["auth"]["required"], true);
    assert_eq!(v["auth"]["method"], "password");
    assert_eq!(v["auth"]["login_hint"], "govee auth login");
    let caps: Vec<&str> = v["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    for cap in [
        "devices", "power", "light", "scene", "toggle", "segment", "music", "rooms", "api",
    ] {
        assert!(caps.contains(&cap), "missing capability {cap}");
    }
}

#[test]
fn usage_error_exits_2_with_json_error_dto() {
    let out = govee()
        .args(["--json", "config", "set", "bogus_key", "x"])
        .assert()
        .code(2);
    let v = json_stdout(&out);
    assert_eq!(v["error"]["code"], "usage");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unknown config key"));
}

#[test]
fn config_round_trips_through_the_override_path() {
    let dir = std::env::temp_dir().join(format!("govee-cli-test-{}", std::process::id()));
    let path = dir.join("config.json");
    let path_s = path.to_str().unwrap();
    let mut cmd = govee();
    cmd.env("GOVEE_CONFIG", path_s)
        .args(["config", "path"])
        .assert()
        .success()
        .stdout(predicate::str::contains(path_s));
    govee()
        .env("GOVEE_CONFIG", path_s)
        .args(["config", "set", "username", "you@example.com"])
        .assert()
        .success();
    let out = govee()
        .env("GOVEE_CONFIG", path_s)
        .args(["--json", "config", "show"])
        .assert()
        .success();
    assert_eq!(json_stdout(&out)["username"], "you@example.com");
    // The keychain marker is not settable by hand.
    govee()
        .env("GOVEE_CONFIG", path_s)
        .args(["config", "set", "api_key_in_keychain", "true"])
        .assert()
        .code(2);
    govee()
        .env("GOVEE_CONFIG", path_s)
        .args(["config", "unset", "username"])
        .assert()
        .success();
    let out = govee()
        .env("GOVEE_CONFIG", path_s)
        .args(["--json", "config", "show"])
        .assert()
        .success();
    assert!(json_stdout(&out).get("username").is_none());
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn auth_status_works_logged_out() {
    let out = govee()
        .args(["--json", "auth", "status"])
        .assert()
        .success();
    let v = json_stdout(&out);
    assert_eq!(v["schema"], "auth-status/v1");
    assert_eq!(v["required"], true);
    assert_eq!(v["authenticated"], false);
    assert_eq!(v["method"], "password");
    assert_eq!(v["credential_in_keychain"], false);
    assert_eq!(v["account_session"]["configured"], false);
    assert_eq!(v["account_session"]["signed_in"], false);
    assert!(v.get("key_source").is_none());
    // Text mode renders a block, not JSON.
    govee()
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Authenticated: no"))
        .stdout(predicate::str::contains("App account:"));
}

#[test]
fn auth_status_reports_an_env_key_without_the_keychain() {
    // `$GOVEE_API_KEY` is a usable credential on its own (env > keychain);
    // reporting it needs no keychain read because the config has no marker.
    let out = govee()
        .env("GOVEE_API_KEY", "not-a-real-key")
        .args(["--json", "auth", "status"])
        .assert()
        .success();
    let v = json_stdout(&out);
    assert_eq!(v["authenticated"], true);
    assert_eq!(v["key_source"], "env");
    assert_eq!(v["credential_in_keychain"], false);
}

#[test]
fn credentialed_reads_exit_3_before_the_keychain_when_nothing_is_configured() {
    for args in [
        vec!["devices", "list"],
        vec!["devices", "get", "Office Lamp"],
        vec!["devices", "search", "lamp"],
        vec!["devices", "caps", "Office Lamp"],
        vec!["power", "status", "Office Lamp"],
        vec!["power", "on", "Office Lamp"],
        vec!["light", "state", "Office Lamp"],
        vec!["light", "brightness", "Office Lamp", "50"],
        vec!["light", "color", "Office Lamp", "--hex", "#FF0000"],
        vec!["light", "temp", "Office Lamp", "4000"],
        vec!["scene", "list", "Office Lamp"],
        vec!["scene", "activate", "Office Lamp", "Sunrise"],
        vec!["toggle", "gradient", "Office Lamp", "on"],
        vec!["segment", "info", "Office Lamp"],
        vec!["music", "list", "Office Lamp"],
        vec!["api", "GET", "/user/devices"],
    ] {
        let mut full = vec!["--json"];
        full.extend(args.iter());
        let out = govee().args(&full).assert().code(3);
        let v = json_stdout(&out);
        assert_eq!(v["error"]["code"], "auth", "args {args:?}");
        assert!(
            v["error"]["message"]
                .as_str()
                .unwrap()
                .contains("auth login"),
            "args {args:?}"
        );
    }
}

#[test]
fn room_reads_exit_3_pointing_at_the_account_login() {
    for args in [vec!["rooms", "list"], vec!["rooms", "devices"]] {
        let mut full = vec!["--json"];
        full.extend(args.iter());
        let out = govee().args(&full).assert().code(3);
        let v = json_stdout(&out);
        assert_eq!(v["error"]["code"], "auth", "args {args:?}");
        assert!(v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("auth login-account"));
    }
}

#[test]
fn env_key_alone_gets_past_auth_to_the_network() {
    // With a key in the environment the credential gate passes without any
    // keychain read; the next stop is the vendor, which the sandbox can't
    // reach, so the result is upstream (5) or, if a network is present, a
    // rejection of the bogus key (3) — never a keychain error (1) and never
    // a prompt.
    let out = govee()
        .env("GOVEE_API_KEY", "not-a-real-key")
        .args(["--json", "devices", "list"])
        .timeout(std::time::Duration::from_secs(30))
        .assert();
    let code = out.get_output().status.code().unwrap_or(-1);
    assert!(code == 5 || code == 3, "unexpected exit code {code}");
}

#[test]
fn room_writes_exit_6_when_non_interactive_without_force() {
    // Checked before any credential or network access, so it is exit 6 even
    // with no account configured — a driver never hangs on a prompt.
    for args in [
        vec!["rooms", "move", "Office Lamp", "--room", "Office"],
        vec!["rooms", "create", "Loft"],
        vec!["rooms", "rename", "Loft", "Attic"],
        vec!["rooms", "delete", "Loft"],
    ] {
        let mut full = vec!["--json"];
        full.extend(args.iter());
        let out = govee().args(&full).assert().code(6);
        assert_eq!(
            json_stdout(&out)["error"]["code"],
            "confirmation_required",
            "args {args:?}"
        );
    }
    // With --force the gate passes and the next stop is the account (3).
    let out = govee()
        .args(["--json", "rooms", "create", "Loft", "--force"])
        .assert()
        .code(3);
    assert!(json_stdout(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("auth login-account"));
    // A bad room name is a usage error before the confirmation gate.
    let long = "x".repeat(23);
    govee()
        .args(["--json", "rooms", "create", &long])
        .assert()
        .code(2);
    govee()
        .args(["--json", "rooms", "rename", "Loft", "   "])
        .assert()
        .code(2);
}

#[test]
fn control_validates_its_arguments_before_any_credential() {
    for args in [
        vec!["light", "brightness", "Lamp", "0"],
        vec!["light", "brightness", "Lamp", "150"],
        vec!["light", "temp", "Lamp", "1000"],
        vec!["light", "color", "Lamp"],
        vec!["light", "color", "Lamp", "--red", "1"],
        vec!["light", "color", "Lamp", "--hex", "GGHHII"],
        vec!["light", "color", "Lamp", "--hex", "#FFF"],
        vec!["toggle", "gradient", "Lamp", "maybe"],
        vec!["toggle", "dreamview", "Lamp", "2"],
        vec!["segment", "color", "Lamp", "{not json"],
        vec!["segment", "brightness", "Lamp", "[1,"],
        vec!["music", "set", "Lamp", "Rhythm", "--sensitivity", "101"],
        vec!["api", "PATCH", "/user/devices"],
        vec!["api", "POST", "/device/state", "--data", "{not json"],
    ] {
        let mut full = vec!["--json"];
        full.extend(args.iter());
        let out = govee().args(&full).assert().code(2);
        assert_eq!(json_stdout(&out)["error"]["code"], "usage", "args {args:?}");
    }
    // --hex and --red/--green/--blue are mutually exclusive (clap, exit 2).
    govee()
        .args(["light", "color", "Lamp", "--hex", "FF0000", "--red", "1"])
        .assert()
        .code(2);
}

#[test]
fn login_never_takes_the_key_on_argv() {
    // The secret enters only via --stdin / --from-env / prompt; a positional
    // key must be a clap usage error.
    govee()
        .args(["auth", "login", "not-a-key-on-argv"])
        .assert()
        .code(2);
    // --non-interactive without a source stops before any prompt or keychain.
    let out = govee()
        .args(["--json", "auth", "login", "--non-interactive"])
        .assert()
        .code(2);
    assert!(json_stdout(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--stdin"));
    // Two sources at once is a usage error too.
    govee()
        .args(["--json", "auth", "login", "--stdin", "--from-env", "X"])
        .write_stdin("k\n")
        .assert()
        .code(2);
    // set-credential requires an explicit source.
    govee()
        .args(["--json", "auth", "set-credential"])
        .assert()
        .code(2);
    // An unset --from-env variable is reported, not prompted for.
    govee()
        .env_remove("GOVEE_TEST_UNSET_KEY")
        .args([
            "--json",
            "auth",
            "set-credential",
            "--from-env",
            "GOVEE_TEST_UNSET_KEY",
        ])
        .assert()
        .code(2);
}

#[test]
fn account_login_needs_a_password_source_when_headless() {
    // Non-TTY, no --stdin, no $GOVEE_PASSWORD: usage error before the
    // keychain is consulted for a remembered session.
    let out = govee()
        .args([
            "--json",
            "auth",
            "login-account",
            "--email",
            "you@example.com",
        ])
        .assert()
        .code(2);
    assert!(json_stdout(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--stdin"));
}

#[test]
fn logout_account_is_a_no_op_with_nothing_configured() {
    govee()
        .args(["auth", "logout-account"])
        .assert()
        .success()
        .stderr(predicate::str::contains("nothing to clear"));
    govee()
        .args(["--json", "auth", "logout-account"])
        .assert()
        .success()
        .stdout("");
}

#[test]
fn table_flag_is_accepted_as_a_no_op() {
    // 0.1's `-t/--table` still parses (it is hidden), and the command then
    // proceeds normally — here to the credential gate.
    govee()
        .args(["--table", "devices", "list"])
        .assert()
        .code(3);
    govee().args(["-t", "devices", "list"]).assert().code(3);
    // And it does not turn `info` into anything but JSON.
    let out = govee().args(["--table", "info"]).assert().success();
    assert_eq!(json_stdout(&out)["schema"], "cli-info/v1");
}

#[test]
fn error_output_is_one_json_document_in_json_mode_and_stderr_only_otherwise() {
    let out = govee().args(["--json", "devices", "list"]).assert().code(3);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert_eq!(stdout.matches("\"error\"").count(), 1);
    let out = govee().args(["devices", "list"]).assert().code(3);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.is_empty(), "text mode puts errors on stderr only");
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(stderr.starts_with("error: "));
}

#[test]
fn completions_render_for_zsh_and_bash() {
    govee()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("#compdef govee"));
    govee()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("govee"));
}

#[test]
fn self_update_check_needs_no_credential() {
    // Reaches GitHub (or fails to, offline): never the keychain, never a
    // Govee credential. Exit 0 (checked) or 5 (no network) are both fine;
    // a Govee auth error (3) or keychain error (1) would be a bug.
    let out = govee()
        .args(["--json", "self-update", "--check"])
        .timeout(std::time::Duration::from_secs(30))
        .assert();
    let code = out.get_output().status.code().unwrap_or(-1);
    assert!(
        code == 0 || code == 5 || code == 4,
        "unexpected exit code {code}"
    );
}

/// The repo must carry no personal data (SPEC §1.7). Scan tracked files for
/// shapes — real-looking emails, API keys (UUIDs), and bearer tokens — never
/// a denylist of real values. Runtime output may carry them; git may not.
#[test]
fn tracked_files_carry_no_personal_data() {
    let root = env!("CARGO_MANIFEST_DIR");
    let out = std::process::Command::new("git")
        .args(["-C", root, "ls-files"])
        .output()
        .expect("git ls-files");
    if !out.status.success() {
        eprintln!("not a git checkout; skipping");
        return;
    }
    let files = String::from_utf8_lossy(&out.stdout);
    for rel in files.lines().filter(|f| !f.ends_with(".lock")) {
        let path = format!("{root}/{rel}");
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (n, line) in text.lines().enumerate() {
            let where_ = format!("{rel}:{}", n + 1);
            for tok in line.split(|c: char| c.is_whitespace() || "\"'`<>()[]{},;|".contains(c)) {
                if tok.contains('@') && tok.contains('.') && !tok.contains("example.com") {
                    let local = tok.split('@').next().unwrap_or("");
                    let domain = tok.rsplit('@').next().unwrap_or("");
                    let is_email = !local.is_empty()
                        && local
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || "._+-".contains(c))
                        && domain.contains('.')
                        && domain
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || ".-".contains(c));
                    assert!(
                        !is_email,
                        "{where_}: `{tok}` looks like a real email address"
                    );
                }
                let parts: Vec<&str> = tok.split('-').collect();
                let is_uuid = parts.len() == 5
                    && parts.iter().map(|p| p.len()).collect::<Vec<_>>() == [8, 4, 4, 4, 12]
                    && tok.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
                    && tok.chars().any(|c| c.is_ascii_digit())
                    && tok.chars().any(|c| c.is_ascii_alphabetic());
                assert!(
                    !is_uuid,
                    "{where_}: `{tok}` looks like a Govee API key (UUID)"
                );
                assert!(
                    !(tok.starts_with("eyJ") && tok.len() > 40),
                    "{where_}: `{tok}` looks like a bearer token"
                );
            }
        }
    }
}
