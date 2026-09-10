use std::io::Write;
use std::process::{Command, Stdio};

fn run(input: &[u8]) -> (i32, Vec<u8>, Vec<u8>) {
    let exe = env!("CARGO_BIN_EXE_higgsfield-lint");
    let mut child = Command::new(exe)
        .env("HIGGSFIELD_LINT_NO_LOG", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child.stdin.as_mut().unwrap().write_all(input).ok();
    let out = child.wait_with_output().expect("wait");
    (out.status.code().unwrap_or(-1), out.stdout, out.stderr)
}

#[test]
fn every_pathological_input_exits_zero_and_silent() {
    let cases: Vec<Vec<u8>> = vec![
        b"".to_vec(),
        b"not json".to_vec(),
        b"{}".to_vec(),
        br#"{"tool_input": null}"#.to_vec(),
        br#"{"tool_input": "a string"}"#.to_vec(),
        br#"{"tool_input": {"params": "a bare string"}}"#.to_vec(),
        br#"{"tool_input": {"params": []}}"#.to_vec(),
        br#"{"tool_input": {"requests": "not a list"}}"#.to_vec(),
        br#"{"tool_input": {"requests": [null, 3, "x"]}}"#.to_vec(),
        vec![0xff, 0xfe, 0x00, 0x01], // invalid UTF-8
        format!(
            r#"{{"tool_input":{{"params":{{"prompt":"{}"}}}}}}"#,
            "a".repeat(5_000_000)
        )
        .into_bytes(),
        format!("{}{}", "[".repeat(50_000), "]".repeat(50_000)).into_bytes(),
    ];
    for (i, case) in cases.iter().enumerate() {
        let (code, _out, err) = run(case);
        assert_eq!(code, 0, "case {i} exited {code}");
        assert!(
            err.is_empty(),
            "case {i} wrote to stderr: {:?}",
            String::from_utf8_lossy(&err)
        );
    }
}

#[test]
fn an_internal_panic_is_still_silent_and_zero() {
    // Guarded by the panic hook plus catch_unwind, not by the absence of bugs.
    //
    // `__panic__` is read by nothing in the shipped hook - main.rs only wires a real
    // panic to it behind `#[cfg(debug_assertions)]` (see main.rs's `run`), which is
    // compiled OUT of `cargo build --release` / `cargo test --release`'s
    // CARGO_BIN_EXE_higgsfield-lint entirely. So under `--release` this input takes
    // the same silent, panic-free path as any other unrecognised field, and this
    // assertion is a smoke test rather than proof the panic hook works.
    //
    // The panic hook + catch_unwind guarantee itself IS genuinely exercised: build and
    // run this test in a plain debug profile (`cargo test`, no `--release`), where
    // `debug_assertions` is on by default and this input actually panics inside
    // `run()` before being caught. Confirmed by hand: with `main()`'s
    // `panic::set_hook`/`catch_unwind` temporarily bypassed (calling `run()` directly),
    // `cargo test --test fail_silent` on this exact input failed with exit code 101 -
    // proof the panic really happens. Restoring the hook and `catch_unwind` makes the
    // test pass again, for the right reason: the panic happened AND got caught, not
    // because nothing panicked.
    let (code, out, err) = run(br#"{"tool_input": {"params": {"__panic__": true}}}"#);
    assert_eq!(code, 0);
    assert!(err.is_empty());
    let _ = out;
}

use std::path::PathBuf;

/// A private log file for one test, named after the test and this process.
fn temp_log(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "higgsfield-lint-test-{name}-{}.jsonl",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&p);
    p
}

/// Run with logging ON, directed at `log`, whatever the harness default is.
fn run_logging(input: &[u8], log: &PathBuf) -> (i32, Vec<u8>, Vec<u8>) {
    let exe = env!("CARGO_BIN_EXE_higgsfield-lint");
    let mut child = Command::new(exe)
        .env_remove("HIGGSFIELD_LINT_NO_LOG")
        .env("HIGGSFIELD_LINT_LOG", log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child.stdin.as_mut().unwrap().write_all(input).ok();
    let out = child.wait_with_output().expect("wait");
    (out.status.code().unwrap_or(-1), out.stdout, out.stderr)
}

fn lines_of(path: &PathBuf) -> Vec<String> {
    std::fs::read_to_string(path)
        .map(|s| s.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

#[test]
fn a_clean_call_is_logged_too() {
    // The denominator is the entire point: a log of only the firings cannot produce a
    // rate. A call with no findings must still leave a record, with `fired` empty.
    let log = temp_log("clean");
    let (code, out, err) = run_logging(
        br#"{"tool_name":"mcp__higgsfield__generate_video","tool_input":{"params":{"prompt":"a cat on a windowsill","width":100,"height":100}}}"#,
        &log,
    );
    assert_eq!(code, 0);
    assert!(err.is_empty());
    assert!(out.is_empty(), "a clean call must produce no stdout");

    let lines = lines_of(&log);
    assert_eq!(lines.len(), 1, "a clean call must still be logged");
    let rec: serde_json::Value = serde_json::from_str(&lines[0]).expect("valid JSON record");
    assert_eq!(rec["fired"], serde_json::json!([]));
    assert_eq!(rec["tool"], "mcp__higgsfield__generate_video");
    assert_eq!(rec["neg"], false);
    let _ = std::fs::remove_file(&log);
}

#[test]
fn a_record_never_contains_prompt_text() {
    // SPEC 9.1: metadata only. This is the test that fails loudly if someone later adds
    // a "prompt" field for debugging convenience.
    let log = temp_log("noprompt");
    let (_code, _out, _err) = run_logging(
        br#"{"tool_name":"mcp__higgsfield__generate_video","tool_input":{"params":{"model":"kling2_6","prompt":"a very distinctive canary phrase","negative_prompt":"blurry","width":1280,"height":720}}}"#,
        &log,
    );
    let lines = lines_of(&log);
    assert_eq!(lines.len(), 1);
    assert!(
        !lines[0].contains("canary"),
        "prompt text leaked into the log: {}",
        lines[0]
    );
    // The metadata that IS allowed must still be there.
    let rec: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(rec["models"], serde_json::json!(["kling2_6"]));
    assert_eq!(rec["neg"], true);
    let _ = std::fs::remove_file(&log);
}

#[test]
fn the_kill_switch_suppresses_the_append() {
    let log = temp_log("killswitch");
    let exe = env!("CARGO_BIN_EXE_higgsfield-lint");
    let mut child = Command::new(exe)
        .env("HIGGSFIELD_LINT_NO_LOG", "1")
        .env("HIGGSFIELD_LINT_LOG", &log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child.stdin.as_mut().unwrap()
        .write_all(br#"{"tool_name":"mcp__higgsfield__generate_video","tool_input":{"params":{"prompt":"shot in 4k","width":1280,"height":720}}}"#)
        .ok();
    let out = child.wait_with_output().expect("wait");
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stderr.is_empty());
    assert!(
        !out.stdout.is_empty(),
        "the finding itself must still be emitted"
    );
    assert!(
        lines_of(&log).is_empty(),
        "HIGGSFIELD_LINT_NO_LOG must suppress the write"
    );
}

#[test]
fn concurrent_appends_never_interleave() {
    // The atomicity claim is structural: O_APPEND plus a single write_all of a line
    // POSIX makes the seek-to-end and the write atomic against other writers, so
    // appends never clobber each other. A short write could in principle still split a
    // line; it does not happen for small writes to a local filesystem (measured: 40
    // concurrent appenders, 40/40 well-formed). This is the test that would catch it if
    // that ever stopped being true.
    let log = temp_log("concurrent");
    let input = br#"{"tool_name":"mcp__higgsfield__generate_video","tool_input":{"params":{"model":"kling2_6","prompt":"cinematic 4k, no logos","negative_prompt":"blurry","width":1280,"height":720}}}"#;
    let mut kids = Vec::new();
    for _ in 0..20 {
        let exe = env!("CARGO_BIN_EXE_higgsfield-lint");
        let mut child = Command::new(exe)
            .env_remove("HIGGSFIELD_LINT_NO_LOG")
            .env("HIGGSFIELD_LINT_LOG", &log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");
        child.stdin.as_mut().unwrap().write_all(input).ok();
        kids.push(child);
    }
    for k in kids {
        let out = k.wait_with_output().expect("wait");
        assert_eq!(out.status.code(), Some(0));
        assert!(out.stderr.is_empty());
    }
    let lines = lines_of(&log);
    assert_eq!(
        lines.len(),
        20,
        "every process must have appended exactly one line"
    );
    for (i, line) in lines.iter().enumerate() {
        serde_json::from_str::<serde_json::Value>(line)
            .unwrap_or_else(|e| panic!("line {i} is not valid JSON ({e}): {line}"));
    }
    let _ = std::fs::remove_file(&log);
}
