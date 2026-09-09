// Task 2's seven real checks read the filesystem (exe location, data file,
// source mtimes, settings.json, matcher regex) via `current_exe()`-relative
// paths, exactly like the hook itself - so they cannot be pointed at a temp
// directory from here the way `models::locate_data_file` can't either. The
// per-check pass/fail discrimination (temp dirs standing in for the repo)
// therefore lives as unit tests inside `rust/src/doctor.rs`, next to the
// functions they exercise. This file is limited to process-level assertions
// against the real checkout - the only thing a plain integration test can see.
//
// One exception: `collect_checks` is the single place that reads the real
// `CLAUDE_PROJECT_DIR` environment variable and threads it through to the
// matcher checks (see `rust/src/doctor.rs`'s `ProjectDir`/`resolve_project_dir`).
// Every unit test for that logic calls `resolve_project_dir`/
// `check_matcher_present`/`check_matcher_correctness` directly with an explicit
// `project_dir` argument, which proves the LOGIC is correct but does nothing to
// prove `collect_checks` actually WIRES the real environment variable through to
// it - a previous review round found exactly that gap by reverting
// `collect_checks` to pass `root` in place of `project_dir.path()` at both call
// sites (the literal original bug) and watching the entire suite, including this
// file's other tests, stay green. `doctor_fails_when_claude_project_dir_disagrees
// _with_the_checkout` and its agreeing-case sibling below close that gap: they
// set `CLAUDE_PROJECT_DIR` on the spawned `Command`'s own environment (never
// touching this test process's real environment, so there is no
// `std::env::set_var` race to worry about) and assert on the resulting report,
// which only a correctly-wired `collect_checks` can produce.
use std::process::{Command, Stdio};

fn run_doctor_with_env(vars: &[(&str, &str)]) -> (i32, String, String) {
    let exe = env!("CARGO_BIN_EXE_higgsfield-lint");
    let mut cmd = Command::new(exe);
    cmd.arg("doctor")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in vars {
        cmd.env(key, value);
    }
    let child = cmd.spawn().expect("spawn");
    let out = child.wait_with_output().expect("wait");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn run_doctor() -> (i32, String, String) {
    run_doctor_with_env(&[])
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

#[test]
fn doctor_fails_when_claude_project_dir_disagrees_with_the_checkout() {
    // Exercises the real `collect_checks` wiring, not just the pure
    // `resolve_project_dir`/`check_matcher_*` functions the unit tests call
    // directly. Setting `CLAUDE_PROJECT_DIR` on the spawned child's own
    // environment via `Command::env` is safe: it never touches this test
    // process's real environment, so there's no `std::env::set_var` race
    // even though tests run in parallel.
    let (code, stdout, stderr) = run_doctor_with_env(&[(
        "CLAUDE_PROJECT_DIR",
        "/tmp/higgsfield-lint-doctor-test-a-different-checkout-entirely",
    )]);
    assert_eq!(
        code, 1,
        "a disagreeing CLAUDE_PROJECT_DIR must fail the matcher checks, got exit {code}, stdout:\n{stdout}"
    );
    assert!(stderr.is_empty(), "doctor wrote to stderr:\n{stderr}");
    assert!(
        stdout.contains("[WARN] project dir:"),
        "expected a project dir disagreement warning, got:\n{stdout}"
    );
    assert!(
        stdout.contains("[FAIL] matcher present:"),
        "expected matcher present to fail under the wrong CLAUDE_PROJECT_DIR, got:\n{stdout}"
    );
    assert!(
        stdout.contains("[FAIL] matcher correctness:"),
        "expected matcher correctness to fail under the wrong CLAUDE_PROJECT_DIR, got:\n{stdout}"
    );
    let fail_count = stdout.matches("[FAIL]").count();
    assert_eq!(
        fail_count, 2,
        "expected exactly the two matcher checks to fail (everything else is independent \
         of CLAUDE_PROJECT_DIR), got:\n{stdout}"
    );
}

#[test]
fn doctor_passes_when_claude_project_dir_agrees_with_the_checkout() {
    // `CARGO_MANIFEST_DIR` is the package root at compile time - the same
    // repo root `doctor` discovers at runtime by walking up from its own
    // binary - so this simulates a live session whose CLAUDE_PROJECT_DIR is
    // set correctly, and must still be all-PASS.
    let repo_root = env!("CARGO_MANIFEST_DIR");
    let (code, stdout, stderr) = run_doctor_with_env(&[("CLAUDE_PROJECT_DIR", repo_root)]);
    assert_eq!(code, 0, "doctor stdout:\n{stdout}");
    assert!(stderr.is_empty(), "doctor wrote to stderr:\n{stderr}");
    assert!(
        !stdout.contains("[FAIL]"),
        "doctor reported a failure with an agreeing CLAUDE_PROJECT_DIR:\n{stdout}"
    );
    assert!(
        !stdout.contains("[WARN]"),
        "doctor reported a warning with an agreeing CLAUDE_PROJECT_DIR:\n{stdout}"
    );
    assert!(
        stdout.contains("agrees with the discovered repo root"),
        "expected the matcher checks to note the agreement, got:\n{stdout}"
    );
    let pass_count = stdout.matches("[PASS]").count();
    assert_eq!(
        pass_count, 7,
        "expected exactly 7 passing checks with an agreeing CLAUDE_PROJECT_DIR, got:\n{stdout}"
    );
}
