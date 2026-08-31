use std::process::{Command, Stdio};

#[test]
fn doctor_runs_and_exits_zero_with_a_stub_check() {
    let exe = env!("CARGO_BIN_EXE_higgsfield-lint");
    let child = Command::new(exe)
        .arg("doctor")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let out = child.wait_with_output().expect("wait");
    assert_eq!(out.status.code().unwrap_or(-1), 0);
    assert!(
        !out.stdout.is_empty(),
        "doctor must write a report to stdout"
    );
}
