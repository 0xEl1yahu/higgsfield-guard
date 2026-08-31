use std::io::Write;
use std::process::{Command, Stdio};

fn run(input: &[u8]) -> (i32, Vec<u8>, Vec<u8>) {
    let exe = env!("CARGO_BIN_EXE_higgsfield-lint");
    let mut child = Command::new(exe)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().expect("spawn");
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
        vec![0xff, 0xfe, 0x00, 0x01],                       // invalid UTF-8
        format!(r#"{{"tool_input":{{"params":{{"prompt":"{}"}}}}}}"#, "a".repeat(5_000_000)).into_bytes(),
        format!("{}{}", "[".repeat(50_000), "]".repeat(50_000)).into_bytes(),
    ];
    for (i, case) in cases.iter().enumerate() {
        let (code, _out, err) = run(case);
        assert_eq!(code, 0, "case {i} exited {code}");
        assert!(err.is_empty(), "case {i} wrote to stderr: {:?}", String::from_utf8_lossy(&err));
    }
}

#[test]
fn an_internal_panic_is_still_silent_and_zero() {
    // Guarded by the panic hook plus catch_unwind, not by the absence of bugs.
    let (code, out, err) = run(br#"{"tool_input": {"params": {"__panic__": true}}}"#);
    assert_eq!(code, 0);
    assert!(err.is_empty());
    let _ = out;
}
