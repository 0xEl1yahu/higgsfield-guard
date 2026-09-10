//! Process-level contract for robot mode (docs/superpowers/specs/2026-09-10-robot-mode-cli-design.md,
//! SPEC.md §6.1). Every test spawns the real binary with piped stdio - a non-terminal on
//! stdin and stdout, exactly what an AI agent's shell looks like - so the default output
//! format under test is JSON and `--text` has to be asked for.
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    fn json(&self) -> Value {
        serde_json::from_str(&self.stdout).unwrap_or_else(|e| {
            panic!(
                "stdout is not one JSON document: {e}\nstdout={:?}\nstderr={:?}",
                self.stdout, self.stderr
            )
        })
    }
    fn error_code(&self) -> String {
        self.json()["error"]["code"]
            .as_str()
            .expect("error.code")
            .to_string()
    }
    fn suggestions(&self) -> Vec<String> {
        self.json()["error"]["suggestions"]
            .as_array()
            .expect("error.suggestions")
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect()
    }
}

fn run_with(args: &[&str], stdin: Option<&[u8]>, env: &[(&str, &str)]) -> Run {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_higgsfield-lint"));
    cmd.args(args)
        .env_remove("CLAUDE_PROJECT_DIR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("spawn");
    {
        let mut handle = child.stdin.take().unwrap();
        if let Some(bytes) = stdin {
            handle.write_all(bytes).ok();
        }
        // dropping the handle closes the pipe: None means "0 bytes then EOF"
    }
    let out = child.wait_with_output().expect("wait");
    let run = Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    };
    assert!(
        !run.stdout.contains('\x1b') && !run.stderr.contains('\x1b'),
        "robot output must never contain ANSI escapes"
    );
    run
}

fn run(args: &[&str], stdin: Option<&[u8]>) -> Run {
    run_with(args, stdin, &[])
}

const OFFENDING_PARAMS: &str =
    r#"{"model":"seedance_2_0","prompt":"shot in 4k","resolution":"720p"}"#;
const CLEAN_PARAMS: &str =
    r#"{"model":"seedance_2_0","prompt":"a quiet garden at dawn","resolution":"720p"}"#;
const EXPECTED_RESOLUTION_MESSAGE: &str = "prompt says \"4k\" but resolution selects the \"720p\" output tier. Choose a supported higher tier, or revise the claim.";

// --- no arguments -----------------------------------------------------------------

#[test]
fn no_args_and_empty_piped_stdin_prints_the_json_quick_start() {
    // Claude Code always writes the event JSON to a hook's stdin, so 0 bytes is never a
    // hook invocation. It IS what `higgsfield-lint` looks like from an agent's shell.
    let r = run(&[], None);
    assert_eq!(r.code, 0, "stdout={:?} stderr={:?}", r.stdout, r.stderr);
    assert!(r.stderr.is_empty());
    let v = r.json();
    assert_eq!(v["ok"], true);
    assert_eq!(v["exit"], 0);
    assert!(v["commands"]["lint"].is_string(), "{v}");
    assert!(v["commands"]["doctor"].is_string(), "{v}");
    assert!(v["hook"].is_string(), "must explain the hook path: {v}");
    assert!(
        r.stdout.len() <= 700,
        "quick start must stay around 100 tokens, got {} bytes",
        r.stdout.len()
    );
}

#[test]
fn no_args_and_a_payload_on_stdin_is_still_the_hook() {
    let payload = format!(
        r#"{{"tool_name":"mcp__higgsfield__generate_video","tool_input":{{"params":{OFFENDING_PARAMS}}}}}"#
    );
    let r = run(&[], Some(payload.as_bytes()));
    assert_eq!(r.code, 0);
    assert!(r.stderr.is_empty());
    let v = r.json();
    assert_eq!(v["hookSpecificOutput"]["hookEventName"], "PreToolUse");
    assert!(
        v.get("ok").is_none(),
        "hook output must not grow CLI fields: {v}"
    );
}

#[test]
fn newline_only_stdin_with_no_args_is_the_silent_hook() {
    // The quick-start branch is exactly 0 bytes; `echo | higgsfield-lint` is a hook call.
    let r = run(&[], Some(b"\n"));
    assert_eq!(r.code, 0);
    assert!(r.stdout.is_empty(), "{:?}", r.stdout);
    assert!(r.stderr.is_empty());
}

#[test]
fn format_flags_alone_show_the_quick_start_never_the_hook() {
    let r = run(&["--text"], Some(b"{\"tool_input\":{}}"));
    assert_eq!(r.code, 0);
    assert!(r.stdout.starts_with("higgsfield-lint "), "{:?}", r.stdout);
    assert!(r.stdout.contains("usage:"), "{:?}", r.stdout);
}

// --- help -------------------------------------------------------------------------

#[test]
fn help_text_is_a_short_quick_start() {
    let r = run(&["help", "--text"], None);
    assert_eq!(r.code, 0);
    assert!(r.stderr.is_empty());
    assert!(!r.stdout.starts_with('{'), "must be text: {:?}", r.stdout);
    assert!(r.stdout.contains("usage:"));
    for cmd in ["lint", "doctor", "checks", "version", "help"] {
        assert!(r.stdout.contains(cmd), "quick start must list {cmd}");
    }
    assert!(r.stdout.contains("exit:"), "must list exit codes");
    assert!(
        r.stdout.len() <= 620,
        "quick start must stay around 100 tokens, got {} bytes:\n{}",
        r.stdout.len(),
        r.stdout
    );
}

#[test]
fn help_aliases_match_help() {
    let canonical = run(&["help"], None).stdout;
    for alias in [&["-h"][..], &["--help"][..]] {
        assert_eq!(run(alias, None).stdout, canonical, "{alias:?}");
    }
}

#[test]
fn help_for_one_command_is_structured() {
    let r = run(&["help", "lint"], None);
    assert_eq!(r.code, 0);
    let v = r.json();
    assert_eq!(v["command"], "lint");
    assert!(v["usage"]
        .as_str()
        .unwrap()
        .starts_with("higgsfield-lint lint"));
    assert!(v["exit_codes"]["1"].is_string(), "{v}");
    assert!(v["examples"].as_array().is_some_and(|e| !e.is_empty()));
    let t = run(&["help", "lint", "--text"], None);
    assert!(
        t.stdout.contains("usage: higgsfield-lint lint"),
        "{:?}",
        t.stdout
    );
}

#[test]
fn help_for_an_unknown_command_is_a_usage_error() {
    let r = run(&["help", "bogus"], None);
    assert_eq!(r.code, 2);
    assert_eq!(r.error_code(), "unknown_command");
}

// --- structured errors ------------------------------------------------------------

#[test]
fn unknown_command_exits_2_with_a_nearest_match_suggestion() {
    let r = run(&["lnt"], None);
    assert_eq!(r.code, 2);
    assert!(
        r.stderr.is_empty(),
        "JSON mode keeps stderr empty: {:?}",
        r.stderr
    );
    let v = r.json();
    assert_eq!(v["ok"], false);
    assert_eq!(v["exit"], 2);
    assert_eq!(v["error"]["code"], "unknown_command");
    assert!(v["error"]["message"].as_str().unwrap().contains("lnt"));
    assert!(
        r.suggestions().iter().any(|s| s.contains("lint")),
        "suggestions={:?}",
        r.suggestions()
    );
}

#[test]
fn unknown_flag_exits_2() {
    let r = run(&["version", "--bogus"], None);
    assert_eq!(r.code, 2);
    assert_eq!(r.error_code(), "unknown_flag");
    assert!(r.json()["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--bogus"));
}

#[test]
fn stray_argument_with_a_hook_payload_is_a_usage_error_not_the_hook() {
    // Replaces fail_silent's `an_unrecognised_argument_still_behaves_as_the_hook`: any
    // argument selects the CLI. doctor's matcher-present check is what guards
    // settings.json against ever carrying an argument (see doctor.rs).
    let payload = br#"{"tool_input": {"params": {"prompt": "x"}}}"#;
    let r = run(&["--nonsense"], Some(payload));
    assert_eq!(r.code, 2);
    assert!(r.stderr.is_empty());
    assert!(r.json().get("hookSpecificOutput").is_none());
    assert_eq!(r.error_code(), "unknown_flag");
}

#[test]
fn format_flags_apply_to_argv_errors_too() {
    let r = run(&["--text", "--bogus"], None);
    assert_eq!(r.code, 2);
    assert!(r.stdout.is_empty(), "{:?}", r.stdout);
    assert!(
        r.stderr.starts_with("error[unknown_flag]:"),
        "{:?}",
        r.stderr
    );
    let r = run(&["--json", "--bogus"], None);
    assert_eq!(r.error_code(), "unknown_flag");
}

#[test]
fn conflicting_format_flags_exit_2() {
    let r = run(&["version", "--json", "--text"], None);
    assert_eq!(r.code, 2);
    assert_eq!(r.error_code(), "conflicting_flags");
}

#[test]
fn text_mode_errors_go_to_stderr_and_stdout_stays_empty() {
    let r = run(&["lnt", "--text"], None);
    assert_eq!(r.code, 2);
    assert!(r.stdout.is_empty(), "{:?}", r.stdout);
    assert!(
        r.stderr.starts_with("error[unknown_command]:"),
        "{:?}",
        r.stderr
    );
    assert!(
        r.stderr.contains("lint"),
        "suggestion missing: {:?}",
        r.stderr
    );
}

#[test]
fn every_error_document_carries_ok_false_and_the_exit_code() {
    for (args, stdin, code) in [
        (&["nope"][..], None, 2),
        (
            &["lint", "--tool", "video", "/definitely/missing.json"][..],
            None,
            3,
        ),
    ] {
        let r = run(args, stdin);
        assert_eq!(r.code, code, "{args:?}");
        let v = r.json();
        assert_eq!(v["ok"], false, "{args:?}");
        assert_eq!(v["exit"], code, "{args:?}");
        assert!(v["error"]["message"].is_string(), "{args:?}");
        assert!(v["error"]["suggestions"].is_array(), "{args:?}");
    }
}

// --- version ----------------------------------------------------------------------

#[test]
fn version_json_is_exact() {
    let r = run(&["version"], None);
    assert_eq!(r.code, 0);
    assert_eq!(
        r.json(),
        serde_json::json!({"ok":true,"name":"higgsfield-lint","version":env!("CARGO_PKG_VERSION"),"exit":0})
    );
    assert!(
        r.stdout.ends_with('\n'),
        "one JSON line, newline-terminated"
    );
}

#[test]
fn version_text_and_aliases() {
    let expected = format!("higgsfield-lint {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(run(&["version", "--text"], None).stdout, expected);
    assert_eq!(run(&["--version", "--text"], None).stdout, expected);
    assert_eq!(run(&["-V", "--text"], None).stdout, expected);
}

#[test]
fn global_flags_may_appear_before_the_command() {
    assert_eq!(
        run(&["--json", "version"], None).stdout,
        run(&["version", "--json"], None).stdout
    );
}

// --- lint: shapes and tool selection ------------------------------------------------

#[test]
fn lint_bare_params_with_findings_exits_1() {
    let r = run(
        &["lint", "--tool", "video"],
        Some(OFFENDING_PARAMS.as_bytes()),
    );
    assert_eq!(r.code, 1, "stdout={:?} stderr={:?}", r.stdout, r.stderr);
    assert!(r.stderr.is_empty());
    let v = r.json();
    assert_eq!(v["ok"], false);
    assert_eq!(v["exit"], 1);
    assert_eq!(v["tool"], "video");
    assert_eq!(v["shape"], "params");
    assert_eq!(
        v["ran"],
        serde_json::json!([
            "resolution_claim",
            "prompt_negative_conflict",
            "duration_claim"
        ])
    );
    let findings = v["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["check"], "resolution_claim");
    assert_eq!(findings[0]["message"], EXPECTED_RESOLUTION_MESSAGE);
    assert!(
        findings[0].get("request").is_none(),
        "no batch index outside a batch"
    );
}

#[test]
fn lint_clean_params_exits_0() {
    let r = run(&["lint", "--tool", "video"], Some(CLEAN_PARAMS.as_bytes()));
    assert_eq!(r.code, 0, "{:?}", r.stdout);
    let v = r.json();
    assert_eq!(v["ok"], true);
    assert_eq!(v["exit"], 0);
    assert_eq!(v["findings"], serde_json::json!([]));
    assert_eq!(v["ran"].as_array().unwrap().len(), 3);
}

#[test]
fn lint_image_runs_two_checks() {
    let r = run(&["lint", "--tool", "image"], Some(CLEAN_PARAMS.as_bytes()));
    assert_eq!(
        run(&["lint", "--tool", "image"], Some(CLEAN_PARAMS.as_bytes())).json()["ran"],
        serde_json::json!(["resolution_claim", "prompt_negative_conflict"]),
        "{}",
        r.stdout
    );
}

#[test]
fn lint_event_shape_takes_the_tool_from_tool_name() {
    let payload = format!(
        r#"{{"tool_name":"mcp__claude_ai_Higgsfield__generate_video","tool_input":{{"params":{OFFENDING_PARAMS}}}}}"#
    );
    let r = run(&["lint"], Some(payload.as_bytes()));
    assert_eq!(r.code, 1, "{:?}", r.stdout);
    let v = r.json();
    assert_eq!(v["shape"], "event");
    assert_eq!(v["tool"], "video");
}

#[test]
fn lint_tool_input_shape_needs_the_tool_flag() {
    let payload = format!(r#"{{"params":{OFFENDING_PARAMS}}}"#);
    let r = run(&["lint"], Some(payload.as_bytes()));
    assert_eq!(r.code, 2);
    assert_eq!(r.error_code(), "missing_tool");
    assert!(r.suggestions().iter().any(|s| s.contains("--tool")));

    let r = run(&["lint", "--tool", "video"], Some(payload.as_bytes()));
    assert_eq!(r.code, 1);
    assert_eq!(r.json()["shape"], "tool_input");
}

#[test]
fn lint_tool_flag_overrides_the_payload_tool_name() {
    let payload = format!(
        r#"{{"tool_name":"mcp__x__generate_audio","tool_input":{{"params":{OFFENDING_PARAMS}}}}}"#
    );
    let r = run(&["lint", "--tool", "video"], Some(payload.as_bytes()));
    assert_eq!(r.code, 1, "{:?}", r.stdout);
    assert_eq!(r.json()["tool"], "video");
}

#[test]
fn lint_tool_flag_accepts_mcp_tool_names() {
    for name in ["generate_video_batch", "mcp__x__generate_video"] {
        let r = run(&["lint", "--tool", name], Some(OFFENDING_PARAMS.as_bytes()));
        assert_eq!(r.json()["tool"], "video", "{name}");
    }
}

#[test]
fn lint_unknown_tool_name_in_payload_exits_2() {
    let payload = format!(
        r#"{{"tool_name":"mcp__x__generate_3d","tool_input":{{"params":{OFFENDING_PARAMS}}}}}"#
    );
    let r = run(&["lint"], Some(payload.as_bytes()));
    assert_eq!(r.code, 2);
    assert_eq!(r.error_code(), "unknown_tool");
    assert!(r.suggestions().iter().any(|s| s.contains("video")));
}

#[test]
fn lint_unknown_tool_flag_value_exits_2() {
    let r = run(&["lint", "--tool", "3d"], Some(OFFENDING_PARAMS.as_bytes()));
    assert_eq!(r.code, 2);
    assert_eq!(r.error_code(), "unknown_tool");
}

#[test]
fn lint_tool_flag_without_a_value_exits_2() {
    let r = run(&["lint", "--tool"], Some(OFFENDING_PARAMS.as_bytes()));
    assert_eq!(r.code, 2);
    assert_eq!(r.error_code(), "missing_value");
}

#[test]
fn lint_audio_runs_nothing_and_says_so() {
    let r = run(
        &["lint", "--tool", "audio"],
        Some(OFFENDING_PARAMS.as_bytes()),
    );
    assert_eq!(r.code, 0);
    let v = r.json();
    assert_eq!(v["ok"], true);
    assert_eq!(v["ran"], serde_json::json!([]));
    assert!(
        v["note"].as_str().is_some_and(|n| n.contains("audio")),
        "{v}"
    );
}

#[test]
fn lint_batch_findings_carry_the_request_index_as_a_field() {
    let payload = format!(
        r#"{{"requests":[{{"index":10,"params":{CLEAN_PARAMS}}},{{"index":20,"params":{OFFENDING_PARAMS}}}]}}"#
    );
    let r = run(&["lint", "--tool", "video"], Some(payload.as_bytes()));
    assert_eq!(r.code, 1, "{:?}", r.stdout);
    let v = r.json();
    assert_eq!(v["shape"], "tool_input");
    let f = &v["findings"][0];
    assert_eq!(f["request"], "20");
    assert_eq!(f["message"], EXPECTED_RESOLUTION_MESSAGE);
}

// --- lint: input errors -------------------------------------------------------------

#[test]
fn lint_missing_file_exits_3() {
    let r = run(
        &["lint", "/definitely/not/here.json", "--tool", "video"],
        None,
    );
    assert_eq!(r.code, 3);
    assert_eq!(r.error_code(), "file_not_found");
}

#[test]
fn lint_invalid_json_exits_3() {
    let r = run(&["lint", "--tool", "video"], Some(b"not json"));
    assert_eq!(r.code, 3);
    assert_eq!(r.error_code(), "invalid_json");
}

#[test]
fn lint_invalid_utf8_exits_3() {
    let r = run(&["lint", "--tool", "video"], Some(&[0xff, 0xfe, 0x00]));
    assert_eq!(r.code, 3);
    assert_eq!(r.error_code(), "invalid_utf8");
}

#[test]
fn lint_non_object_document_exits_3() {
    let r = run(&["lint", "--tool", "video"], Some(b"[1,2]"));
    assert_eq!(r.code, 3);
    assert_eq!(r.error_code(), "unsupported_shape");
}

#[test]
fn lint_empty_stdin_exits_3_as_invalid_json() {
    let r = run(&["lint", "--tool", "video"], None);
    assert_eq!(r.code, 3);
    assert_eq!(r.error_code(), "invalid_json");
}

#[test]
fn lint_extra_positional_argument_exits_2() {
    let r = run(&["lint", "a.json", "b.json", "--tool", "video"], None);
    assert_eq!(r.code, 2);
    assert_eq!(r.error_code(), "unexpected_argument");
}

// --- lint: file and dash ------------------------------------------------------------

#[test]
fn lint_reads_a_file_argument_and_dash_means_stdin() {
    let dir = std::env::temp_dir().join(format!("higgsfield-lint-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("call.json");
    std::fs::write(&file, OFFENDING_PARAMS).unwrap();
    let from_file = run(&["lint", file.to_str().unwrap(), "--tool", "video"], None);
    assert_eq!(from_file.code, 1, "{:?}", from_file.stdout);
    let from_dash = run(
        &["lint", "-", "--tool", "video"],
        Some(OFFENDING_PARAMS.as_bytes()),
    );
    assert_eq!(from_dash.stdout, from_file.stdout);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn lint_text_mode_prints_the_hook_rendering() {
    let r = run(
        &["lint", "--tool", "video", "--text"],
        Some(OFFENDING_PARAMS.as_bytes()),
    );
    assert_eq!(r.code, 1);
    assert_eq!(
        r.stdout,
        format!("higgsfield-lint: 1 finding\n- resolution_claim: {EXPECTED_RESOLUTION_MESSAGE}\n")
    );
}

#[test]
fn lint_internal_panic_exits_4_with_a_structured_error() {
    // Same debug-only injection point the hook uses (main.rs `__panic__`); in a release
    // build nothing panics and this input is just an unrecognised field.
    let r = run(
        &["lint", "--tool", "video"],
        Some(br#"{"prompt":"x","__panic__":true}"#),
    );
    if cfg!(debug_assertions) {
        assert_eq!(r.code, 4, "{:?}", r.stdout);
        assert!(r.stderr.is_empty());
        assert_eq!(r.error_code(), "internal");
    } else {
        assert_eq!(r.code, 0);
    }
}

// --- checks -------------------------------------------------------------------------

#[test]
fn checks_json_describes_the_three_live_checks() {
    let r = run(&["checks"], None);
    assert_eq!(r.code, 0);
    let v = r.json();
    assert_eq!(v["ok"], true);
    let checks = v["checks"].as_array().unwrap();
    let ids: Vec<&str> = checks.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert_eq!(
        ids,
        [
            "resolution_claim",
            "prompt_negative_conflict",
            "duration_claim"
        ]
    );
    for c in checks {
        for key in ["tools", "fires", "fix", "silent_when"] {
            assert!(!c[key].is_null(), "{} lacks {key}", c["id"]);
        }
    }
    assert!(checks[1]["phrases"]
        .as_array()
        .unwrap()
        .contains(&Value::from("slow motion")));
    assert!(checks[2]["models"]
        .as_array()
        .unwrap()
        .contains(&Value::from("kling2_6")));
    assert_eq!(checks[2]["tools"], serde_json::json!(["video"]));
}

#[test]
fn checks_text_mode_is_one_line_per_check() {
    let r = run(&["checks", "--text"], None);
    assert_eq!(r.code, 0);
    let lines: Vec<&str> = r.stdout.lines().collect();
    assert_eq!(lines.len(), 3, "{:?}", r.stdout);
    assert!(lines[0].starts_with("resolution_claim"));
    assert!(lines[2].starts_with("duration_claim"));
}

// --- doctor -------------------------------------------------------------------------

#[test]
#[cfg(not(debug_assertions))] // doctor resolves .claude/settings.json's release path; see ci/verify.sh
fn doctor_json_reports_every_check_and_a_summary() {
    let r = run(&["doctor"], None);
    assert_eq!(r.code, 0, "{:?}", r.stdout);
    assert!(r.stderr.is_empty());
    let v = r.json();
    assert_eq!(v["ok"], true);
    assert_eq!(v["exit"], 0);
    let checks = v["checks"].as_array().unwrap();
    assert_eq!(checks.len(), 7);
    for c in checks {
        assert!(c["name"].is_string());
        assert_eq!(c["status"], "pass");
        assert!(c["detail"].is_string());
        assert!(!c["detail"]
            .as_str()
            .unwrap()
            .starts_with(c["name"].as_str().unwrap()));
    }
    assert_eq!(
        v["summary"],
        serde_json::json!({"pass":7,"warn":0,"fail":0})
    );
    assert!(v["note"].is_string());
}

#[test]
#[cfg(not(debug_assertions))] // doctor resolves .claude/settings.json's release path; see ci/verify.sh
fn doctor_json_counts_failures_and_exits_1() {
    let r = run_with(
        &["doctor"],
        None,
        &[(
            "CLAUDE_PROJECT_DIR",
            "/tmp/higgsfield-lint-cli-test-elsewhere",
        )],
    );
    assert_eq!(r.code, 1);
    let v = r.json();
    assert_eq!(v["ok"], false);
    assert_eq!(v["exit"], 1);
    assert_eq!(v["summary"]["fail"], 2);
    assert_eq!(v["summary"]["warn"], 1);
    let names: Vec<&str> = v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["status"] == "fail")
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["matcher present", "matcher correctness"]);
}

#[test]
#[cfg(not(debug_assertions))] // doctor resolves .claude/settings.json's release path; see ci/verify.sh
fn doctor_text_mode_is_the_classic_report() {
    let r = run(&["doctor", "--text"], None);
    assert_eq!(r.code, 0);
    assert!(
        r.stdout.starts_with("higgsfield-lint doctor\n"),
        "{:?}",
        r.stdout
    );
    assert_eq!(r.stdout.matches("[PASS]").count(), 7);
}
