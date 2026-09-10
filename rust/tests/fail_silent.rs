use std::io::Write;
use std::process::{Command, Stdio};

fn run(input: &[u8]) -> (i32, Vec<u8>, Vec<u8>) {
    let exe = env!("CARGO_BIN_EXE_higgsfield-lint");
    let mut child = Command::new(exe)
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
    // A 0-byte stdin is deliberately absent: Claude Code always writes the event JSON to
    // a hook's stdin, so that case is the robot quick start (SPEC.md §6.1, §7), covered by
    // rust/tests/cli.rs. Everything that IS a hook invocation stays silent.
    let cases: Vec<Vec<u8>> = vec![
        b"\n".to_vec(), // one byte: the boundary of the quick-start branch, still the hook
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
        let (code, out, err) = run(case);
        assert!(out.is_empty(), "case {i} wrote stdout");
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
    assert!(out.is_empty());
}

#[test]
fn argument_dispatch_does_not_change_the_hook_path() {
    // Every pathological payload must behave identically when the binary is invoked
    // with no arguments, exactly as before `doctor` or the robot CLI existed.
    let payload = br#"{"tool_name":"mcp__higgsfield__generate_video","tool_input":{"params":{"model":"seedance_2_0","prompt":"shot in 4k","resolution":"720p"}}}"#;
    let (code, out, err) = run(payload);
    assert_eq!(code, 0);
    assert!(err.is_empty());
    assert!(!out.is_empty(), "the hook path must still emit findings");
}
