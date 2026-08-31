// `doctor` is a human-invoked diagnostic subcommand, not part of the hook path.
// Unlike the hook (see main.rs), it is allowed to write to stdout/stderr and to
// exit non-zero. That permission must never leak into the hook path: main.rs
// only reaches `run()` here when the first CLI argument is exactly "doctor".

/// The outcome of a single diagnostic check.
// `Warn`/`Fail` are unused until Task 2 adds checks that can produce them; the
// stub check here only ever returns `Pass`.
#[allow(dead_code)]
pub enum Check {
    Pass(String),
    Warn(String),
    Fail(String),
}

/// Runs all diagnostic checks, prints a human-readable report to stdout, and
/// returns the process exit code: 0 if every check passed or warned, 1 if any
/// check failed.
pub fn run() -> i32 {
    let checks = collect_checks();
    report(&checks);
    exit_code(&checks)
}

fn collect_checks() -> Vec<Check> {
    vec![stub_check()]
}

// Task 1 ships no real diagnostics (see task brief). This stub proves the
// dispatch, reporter, and exit-code plumbing work before Task 2 adds checks
// that can actually fail.
fn stub_check() -> Check {
    Check::Pass("doctor scaffold is wired up".to_string())
}

fn report(checks: &[Check]) {
    println!("higgsfield-lint doctor");
    println!("=======================");
    for check in checks {
        match check {
            Check::Pass(msg) => println!("[PASS] {msg}"),
            Check::Warn(msg) => println!("[WARN] {msg}"),
            Check::Fail(msg) => println!("[FAIL] {msg}"),
        }
    }
}

fn exit_code(checks: &[Check]) -> i32 {
    if checks.iter().any(|c| matches!(c, Check::Fail(_))) {
        1
    } else {
        0
    }
}
