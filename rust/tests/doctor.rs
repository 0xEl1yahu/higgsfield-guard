// Task 2's seven real checks read the filesystem (exe location, data file,
// source mtimes, settings.json, matcher regex) via `current_exe()`-relative
// paths, exactly like the hook itself - so they cannot be pointed at a temp
// directory from here the way `models::locate_data_file` can't either. The
// per-check pass/fail discrimination (temp dirs standing in for the repo)
// therefore lives as unit tests inside `rust/src/doctor.rs`, next to the
// functions they exercise. This file is limited to process-level assertions
// against the real checkout - the only thing a plain integration test can see.
use std::process::{Command, Stdio};

fn run_doctor() -> (i32, String, String) {
    let exe = env!("CARGO_BIN_EXE_higgsfield-lint");
    let child = Command::new(exe)
        .arg("doctor")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let out = child.wait_with_output().expect("wait");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn doctor_writes_a_report_to_stdout_and_exits_zero_on_a_healthy_checkout() {
    let (code, stdout, _stderr) = run_doctor();
    assert_eq!(code, 0, "doctor stdout:\n{stdout}");
    assert!(!stdout.is_empty(), "doctor must write a report to stdout");
}

#[test]
fn doctor_against_the_real_checkout_reports_all_seven_checks_passing() {
    // `CARGO_BIN_EXE_higgsfield-lint` under `cargo test --release` IS
    // `target/release/higgsfield-lint` - the exact path `.claude/settings.json`
    // names - so this is the first real evidence the install chain is sound,
    // re-checked on every test run rather than only by hand.
    let (code, stdout, stderr) = run_doctor();
    assert_eq!(code, 0, "doctor stdout:\n{stdout}");
    assert!(stderr.is_empty(), "doctor wrote to stderr:\n{stderr}");
    assert!(
        !stdout.contains("[FAIL]"),
        "doctor reported a failure against the real checkout:\n{stdout}"
    );
    assert!(
        !stdout.contains("[WARN]"),
        "doctor reported a warning against the real checkout:\n{stdout}"
    );
    let pass_count = stdout.matches("[PASS]").count();
    assert_eq!(
        pass_count, 7,
        "expected exactly 7 passing checks against a healthy checkout, got:\n{stdout}"
    );
}
