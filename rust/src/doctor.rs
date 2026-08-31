// `doctor` is a human-invoked diagnostic subcommand, not part of the hook path.
// Unlike the hook (see main.rs), it is allowed to write to stdout/stderr and to
// exit non-zero. That permission must never leak into the hook path: main.rs
// only reaches `run()` here when the first CLI argument is exactly "doctor".

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use regex::Regex;
use serde_json::Value;

use crate::lint;

/// The outcome of a single diagnostic check.
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

// Checks are ordered so the earliest failure is the most fundamental: each one
// answers a question that, answered wrongly, means the hook silently does
// nothing, and each depends on the ones before it actually being true.
fn collect_checks() -> Vec<Check> {
    let mut checks = Vec::new();

    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(err) => {
            checks.push(Check::Fail(format!(
                "binary location: could not determine this executable's own path: {err}"
            )));
            checks.push(check_self_test());
            return checks;
        }
    };

    let repo_root = find_repo_root(&exe);
    checks.push(match &repo_root {
        Some(root) => Check::Pass(format!(
            "binary location: {} is under {}, which contains a data/ directory",
            exe.display(),
            root.display()
        )),
        None => Check::Fail(format!(
            "binary location: no ancestor directory of {} contains a data/ directory; \
             load_models() will find nothing and every model-aware check will run silently blind",
            exe.display()
        )),
    });

    match &repo_root {
        Some(root) => {
            checks.push(check_data_file(root));
            checks.push(check_binary_freshness_real(root, &exe));
            checks.push(check_settings_file(root));

            // A live Claude Code session always has CLAUDE_PROJECT_DIR set when
            // it substitutes it into a hook `command`; `doctor` run by hand
            // usually doesn't. Read the real value here (the one real call site)
            // and hand it to the pure resolver below.
            let env_value = std::env::var("CLAUDE_PROJECT_DIR").ok();
            let project_dir = resolve_project_dir(env_value.as_deref(), root);
            if let Some(warning) = project_dir.warning() {
                checks.push(warning);
            }
            let note = project_dir.note();
            checks.push(check_matcher_present(root, project_dir.path(), &exe, &note));
            checks.push(check_matcher_correctness(root, project_dir.path(), &exe, &note));
        }
        None => {
            let skip = |name: &str| {
                Check::Fail(format!(
                    "{name}: skipped, no repo root found (see binary location check)"
                ))
            };
            checks.push(skip("data file"));
            checks.push(skip("binary freshness"));
            checks.push(skip("settings file"));
            checks.push(skip("matcher present"));
            checks.push(skip("matcher correctness"));
        }
    }

    checks.push(check_self_test());
    checks
}

/// Walks up from `exe`'s parent directories looking for one that contains a
/// `data/` directory - the layout `models::load_models` depends on. Mirrors
/// `models::locate_data_file`'s walk, but stops at directory presence rather
/// than requiring the specific JSON file inside it, since that distinction is
/// check 2's job, not check 1's.
fn find_repo_root(exe: &Path) -> Option<PathBuf> {
    let mut dir = exe.parent();
    while let Some(d) = dir {
        if d.join("data").is_dir() {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

/// Check 2: `data/negative_prompt_models.json` exists, parses as JSON, and is
/// specifically a JSON array of strings (the shape `load_models` requires -
/// anything else and its `and_then` chain fails silently to an empty set).
fn check_data_file(repo_root: &Path) -> Check {
    let path = repo_root.join("data").join("negative_prompt_models.json");
    let contents = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(err) => {
            return Check::Fail(format!(
                "data file: missing or unreadable at {}: {err}",
                path.display()
            ))
        }
    };
    let value: Value = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(err) => {
            return Check::Fail(format!(
                "data file: {} is not valid JSON: {err}",
                path.display()
            ))
        }
    };
    match value {
        Value::Array(items) if items.iter().all(Value::is_string) => Check::Pass(format!(
            "data file: {} is a JSON array of {} string(s)",
            path.display(),
            items.len()
        )),
        _ => Check::Fail(format!(
            "data file: {} is valid JSON but not an array of strings",
            path.display()
        )),
    }
}

/// Check 3, the one that matters most: a stale binary runs old logic and is
/// completely indistinguishable from a current one at runtime, so this is the
/// only check that can ever catch it. Fails when any tracked source file -
/// `rust/src/*.rs`, `Cargo.toml`, or `data/*.json` - is newer than the running
/// executable.
fn check_binary_freshness(repo_root: &Path, exe_mtime: SystemTime) -> Check {
    let mut newer = Vec::new();

    newer.extend(newer_files_with_ext(
        &repo_root.join("rust").join("src"),
        "rs",
        exe_mtime,
    ));

    let cargo_toml = repo_root.join("Cargo.toml");
    if let Ok(modified) = fs::metadata(&cargo_toml).and_then(|m| m.modified()) {
        if modified > exe_mtime {
            newer.push(cargo_toml.display().to_string());
        }
    }

    newer.extend(newer_files_with_ext(&repo_root.join("data"), "json", exe_mtime));

    if newer.is_empty() {
        Check::Pass(
            "binary freshness: binary is newer than every tracked source \
             (rust/src/*.rs, Cargo.toml, data/*.json)"
                .to_string(),
        )
    } else {
        newer.sort();
        Check::Fail(format!(
            "binary freshness: STALE BINARY, rebuild needed - newer than the binary: {}",
            newer.join(", ")
        ))
    }
}

fn newer_files_with_ext(dir: &Path, ext: &str, exe_mtime: SystemTime) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out; // directory absent: nothing to report here, another check's job
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(ext) {
            continue;
        }
        if let Ok(modified) = entry.metadata().and_then(|m| m.modified()) {
            if modified > exe_mtime {
                out.push(path.display().to_string());
            }
        }
    }
    out
}

fn check_binary_freshness_real(repo_root: &Path, exe: &Path) -> Check {
    match fs::metadata(exe).and_then(|m| m.modified()) {
        Ok(mtime) => check_binary_freshness(repo_root, mtime),
        Err(err) => Check::Warn(format!(
            "binary freshness: could not read this binary's own mtime, skipping: {err}"
        )),
    }
}

fn pretooluse_array(settings: &Value) -> Option<&Vec<Value>> {
    settings.get("hooks")?.get("PreToolUse")?.as_array()
}

fn load_settings(repo_root: &Path) -> Result<Value, String> {
    let path = repo_root.join(".claude").join("settings.json");
    let contents = fs::read_to_string(&path)
        .map_err(|err| format!("{} missing or unreadable: {err}", path.display()))?;
    serde_json::from_str(&contents)
        .map_err(|err| format!("{} is not valid JSON: {err}", path.display()))
}

/// Check 4: `.claude/settings.json` exists, parses, and has a non-empty
/// `hooks.PreToolUse` array.
fn check_settings_file(repo_root: &Path) -> Check {
    let settings = match load_settings(repo_root) {
        Ok(v) => v,
        Err(err) => return Check::Fail(format!("settings file: {err}")),
    };
    match pretooluse_array(&settings) {
        Some(arr) if !arr.is_empty() => Check::Pass(format!(
            "settings file: hooks.PreToolUse has {} entry(ies)",
            arr.len()
        )),
        Some(_) => Check::Fail("settings file: hooks.PreToolUse is present but empty".to_string()),
        None => Check::Fail("settings file: missing hooks.PreToolUse array".to_string()),
    }
}

/// The project directory a live Claude Code session substitutes for
/// `$CLAUDE_PROJECT_DIR` when it resolves a hook `command`. A session always
/// has the real environment variable set; `doctor`, run by hand, usually
/// doesn't, so it falls back to the repo root it discovered by walking up
/// from its own binary (check 1) - but that fallback is only ever an
/// approximation. In a git worktree, a symlinked checkout, or a project
/// opened from a subdirectory, the discovered root can silently disagree with
/// what a live session actually uses, which would make checks 5 and 6 report
/// PASS for wiring that a live session resolves completely differently.
enum ProjectDir {
    /// `CLAUDE_PROJECT_DIR` isn't set - the normal case for a manual `doctor`
    /// run outside a live session. Falls back to the discovered repo root.
    Unset(PathBuf),
    /// `CLAUDE_PROJECT_DIR` is set and agrees with the discovered repo root.
    Agrees(PathBuf),
    /// `CLAUDE_PROJECT_DIR` is set but does NOT agree with the discovered repo
    /// root - the exact blind spot this type exists to catch. `env` is what a
    /// live session actually substitutes.
    Disagrees { env: PathBuf, discovered: PathBuf },
}

impl ProjectDir {
    /// The directory to substitute for `$CLAUDE_PROJECT_DIR`: the real
    /// environment value whenever one was given (that's what a live session
    /// actually does), the discovered root only when there is no real value
    /// to prefer.
    fn path(&self) -> &Path {
        match self {
            ProjectDir::Unset(p) | ProjectDir::Agrees(p) => p,
            ProjectDir::Disagrees { env, .. } => env,
        }
    }

    /// One-line explanation appended to checks 5 and 6's own messages so the
    /// reader always sees which project dir was used and why - including the
    /// unset case, where silent fallback would otherwise recreate the same
    /// blind spot in a quieter form.
    fn note(&self) -> String {
        match self {
            ProjectDir::Unset(p) => format!(
                "CLAUDE_PROJECT_DIR not set, used the discovered repo root {}",
                p.display()
            ),
            ProjectDir::Agrees(p) => format!(
                "CLAUDE_PROJECT_DIR={} agrees with the discovered repo root",
                p.display()
            ),
            ProjectDir::Disagrees { env, .. } => {
                format!("CLAUDE_PROJECT_DIR={} - see the project dir warning above", env.display())
            }
        }
    }

    /// A standalone `Warn` surfaced in the report only when there is a real
    /// disagreement to flag - not on every run, so a manual `doctor` from an
    /// unusual cwd doesn't cry wolf.
    fn warning(&self) -> Option<Check> {
        match self {
            ProjectDir::Disagrees { env, discovered } => Some(Check::Warn(format!(
                "project dir: CLAUDE_PROJECT_DIR={} disagrees with the discovered repo root {} - \
                 a live Claude Code session substitutes the environment variable when resolving hook \
                 commands, not the path doctor discovers by walking up from its own binary; the matcher \
                 checks below use CLAUDE_PROJECT_DIR, since that is what a live session actually does",
                env.display(),
                discovered.display()
            ))),
            _ => None,
        }
    }
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        // Can't canonicalize both (e.g. a fixture path that was never created
        // on disk) - fall back to plain equality rather than silently calling
        // it a match.
        _ => a == b,
    }
}

/// Pure by construction: takes the environment value as a parameter instead
/// of reading `std::env::var` internally, so all three states (unset /
/// set-and-agreeing / set-and-disagreeing) are directly testable without
/// mutating real process-global environment state - the same reason
/// `check_binary_freshness` takes `exe_mtime` as a parameter instead of
/// reading the running binary's real mtime itself. `collect_checks` is the
/// one call site that reads the actual `CLAUDE_PROJECT_DIR`.
fn resolve_project_dir(env_value: Option<&str>, discovered_root: &Path) -> ProjectDir {
    match env_value {
        None | Some("") => ProjectDir::Unset(discovered_root.to_path_buf()),
        Some(v) => {
            let env_root = PathBuf::from(v);
            if paths_equal(&env_root, discovered_root) {
                ProjectDir::Agrees(env_root)
            } else {
                ProjectDir::Disagrees {
                    env: env_root,
                    discovered: discovered_root.to_path_buf(),
                }
            }
        }
    }
}

/// Resolves a hook `command` string the way `.claude/settings.json` writes it:
/// wrapped in literal double quotes (to survive spaces in the path) with
/// `$CLAUDE_PROJECT_DIR` standing in for `project_dir` - the directory a live
/// session would actually substitute (see `ProjectDir`), not necessarily the
/// repo root `doctor` discovered on its own.
fn resolve_command(command: &str, project_dir: &Path) -> PathBuf {
    let trimmed = command.trim().trim_matches('"');
    let substituted = trimmed.replace("$CLAUDE_PROJECT_DIR", &project_dir.to_string_lossy());
    PathBuf::from(substituted)
}

fn command_resolves_to_exe(command: &str, project_dir: &Path, exe: &Path) -> bool {
    let candidate = resolve_command(command, project_dir);
    match (fs::canonicalize(&candidate), fs::canonicalize(exe)) {
        (Ok(a), Ok(b)) => a == b,
        // One or both don't exist on disk (e.g. a test fixture that never
        // creates the exe file) - fall back to comparing the resolved paths
        // directly rather than silently treating "can't canonicalize" as "no
        // match".
        _ => candidate == exe,
    }
}

fn find_own_hook_group<'a>(entries: &'a [Value], project_dir: &Path, exe: &Path) -> Option<&'a Value> {
    entries.iter().find(|entry| {
        entry
            .get("hooks")
            .and_then(Value::as_array)
            .map(|hooks| {
                hooks.iter().any(|h| {
                    h.get("command")
                        .and_then(Value::as_str)
                        .is_some_and(|cmd| command_resolves_to_exe(cmd, project_dir, exe))
                })
            })
            .unwrap_or(false)
    })
}

/// Check 5: some `PreToolUse` entry's `hooks[].command` resolves to this exact
/// executable when `$CLAUDE_PROJECT_DIR` is resolved as `project_dir` - i.e.
/// the hook is actually wired up for this binary the way a live session would
/// see it, not just present in the settings file under some other root.
fn check_matcher_present(repo_root: &Path, project_dir: &Path, exe: &Path, project_dir_note: &str) -> Check {
    let settings = match load_settings(repo_root) {
        Ok(v) => v,
        Err(err) => return Check::Fail(format!("matcher present: {err}")),
    };
    let entries = match pretooluse_array(&settings) {
        Some(arr) => arr,
        None => {
            return Check::Fail(
                "matcher present: settings file has no hooks.PreToolUse array (see settings file check)"
                    .to_string(),
            )
        }
    };
    match find_own_hook_group(entries, project_dir, exe) {
        Some(_) => Check::Pass(format!(
            "matcher present: a PreToolUse entry's command resolves to {} ({project_dir_note})",
            exe.display()
        )),
        None => Check::Fail(format!(
            "matcher present: no PreToolUse entry's command resolves to {} when $CLAUDE_PROJECT_DIR \
             is {} - the hook is not installed for this binary ({project_dir_note})",
            exe.display(),
            project_dir.display()
        )),
    }
}

const SHOULD_MATCH: &str = "mcp__ada4cfac-9b58-4bfb-9631-4a588b7a2e4b__generate_video";
const SHOULD_NOT_MATCH: &[&str] = &["mcp__x__generate_3d", "mcp__x__show_generations"];

/// Check 6: the matcher on this binary's own `PreToolUse` entry (resolved the
/// same way check 5 does, via `project_dir`) matches a real Higgsfield tool
/// name and rejects the two nearest lookalikes.
fn check_matcher_correctness(repo_root: &Path, project_dir: &Path, exe: &Path, project_dir_note: &str) -> Check {
    let settings = match load_settings(repo_root) {
        Ok(v) => v,
        Err(err) => return Check::Fail(format!("matcher correctness: {err}")),
    };
    let entries = match pretooluse_array(&settings) {
        Some(arr) => arr,
        None => {
            return Check::Fail(
                "matcher correctness: settings file has no hooks.PreToolUse array (see settings file check)"
                    .to_string(),
            )
        }
    };
    let Some(entry) = find_own_hook_group(entries, project_dir, exe) else {
        return Check::Fail(format!(
            "matcher correctness: no matching PreToolUse entry found for this binary when \
             $CLAUDE_PROJECT_DIR is {} (see matcher present check) ({project_dir_note})",
            project_dir.display()
        ));
    };
    let Some(matcher) = entry.get("matcher").and_then(Value::as_str) else {
        return Check::Fail(
            "matcher correctness: the matching PreToolUse entry has no \"matcher\" string".to_string(),
        );
    };
    let re = match Regex::new(matcher) {
        Ok(r) => r,
        Err(err) => {
            return Check::Fail(format!(
                "matcher correctness: matcher {matcher:?} is not a valid regex: {err}"
            ))
        }
    };

    if !re.is_match(SHOULD_MATCH) {
        return Check::Fail(format!(
            "matcher correctness: matcher {matcher:?} does not match {SHOULD_MATCH:?} - \
             a real Higgsfield call would be missed entirely"
        ));
    }
    for bad in SHOULD_NOT_MATCH {
        if re.is_match(bad) {
            return Check::Fail(format!(
                "matcher correctness: matcher {matcher:?} wrongly matches {bad:?}"
            ));
        }
    }
    Check::Pass(format!(
        "matcher correctness: matcher {matcher:?} matches {SHOULD_MATCH:?} and rejects lookalikes ({project_dir_note})"
    ))
}

const SELF_TEST_EXPECTED_CHECK_IDS: [&str; 3] =
    ["resolution_claim", "prompt_negative_conflict", "negation_placement"];

/// Pure comparison used by check 7, factored out so it can be exercised with
/// fabricated `check_id` lists independent of whether the real lint pipeline
/// currently happens to be correct.
fn evaluate_self_test(ids: &[&str]) -> Check {
    if ids == SELF_TEST_EXPECTED_CHECK_IDS {
        Check::Pass(format!(
            "self-test: known-offending payload produced exactly {SELF_TEST_EXPECTED_CHECK_IDS:?}"
        ))
    } else {
        Check::Fail(format!(
            "self-test: known-offending payload produced {ids:?}, expected \
             {SELF_TEST_EXPECTED_CHECK_IDS:?} - the lint logic itself is broken, \
             or this binary is running stale code"
        ))
    }
}

/// Check 7: lint a known-offending payload in-process (using the real,
/// currently-running `lint`/`models` code, not a mock) and confirm exactly the
/// three expected `check_id`s come back, in order.
fn check_self_test() -> Check {
    let tool_input = serde_json::json!({
        "params": {
            "prompt": "shot in 4k, no logos, sharp focus",
            "negative_prompt": "sharp, blurry",
            "width": 1280,
            "height": 720
        }
    });
    let models = lint::load_models();
    let findings = lint::lint(&tool_input, &models);
    let ids: Vec<&str> = findings.iter().map(|f| f.check_id.as_str()).collect();
    evaluate_self_test(&ids)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    // --- temp-dir test fixture -------------------------------------------------------

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// A repo-root stand-in that cleans itself up on drop, so a failing test
    /// run doesn't litter the real temp directory.
    struct TestRepo {
        path: PathBuf,
    }

    impl TestRepo {
        fn new(label: &str) -> Self {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let pid = std::process::id();
            let path = std::env::temp_dir().join(format!(
                "higgsfield-lint-doctor-test-{pid}-{label}-{n}"
            ));
            fs::create_dir_all(&path).expect("create temp repo root");
            TestRepo { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write(&self, rel: &str, contents: &str) -> PathBuf {
            let full = self.path.join(rel);
            fs::create_dir_all(full.parent().unwrap()).expect("create parent dirs");
            fs::write(&full, contents).expect("write fixture file");
            full
        }
    }

    impl Drop for TestRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn assert_pass(check: &Check, context: &str) {
        assert!(
            matches!(check, Check::Pass(_)),
            "{context}: expected Pass, got {}",
            describe(check)
        );
    }

    fn assert_fail(check: &Check, context: &str) {
        assert!(
            matches!(check, Check::Fail(_)),
            "{context}: expected Fail, got {}",
            describe(check)
        );
    }

    fn describe(check: &Check) -> String {
        match check {
            Check::Pass(m) => format!("Pass({m})"),
            Check::Warn(m) => format!("Warn({m})"),
            Check::Fail(m) => format!("Fail({m})"),
        }
    }

    // --- check 1: binary location -----------------------------------------------------

    #[test]
    fn binary_location_passes_when_an_ancestor_has_a_data_dir() {
        let repo = TestRepo::new("loc-pass");
        repo.write("data/.keep", "");
        let exe = repo.path().join("target").join("release").join("higgsfield-lint");
        assert_eq!(find_repo_root(&exe), Some(repo.path().to_path_buf()));
    }

    #[test]
    fn binary_location_fails_when_no_ancestor_has_a_data_dir() {
        let repo = TestRepo::new("loc-fail");
        // No `data/` anywhere under repo.path() - and repo.path() itself is a
        // freshly created, uniquely-named directory under the system temp dir,
        // so none of ITS ancestors (the temp dir, "/", etc.) are expected to
        // contain a literal `data/` directory either.
        let exe = repo.path().join("target").join("release").join("higgsfield-lint");
        assert_eq!(find_repo_root(&exe), None);
    }

    // --- check 2: data file -------------------------------------------------------------

    #[test]
    fn data_file_passes_for_a_valid_array_of_strings() {
        let repo = TestRepo::new("data-pass");
        repo.write("data/negative_prompt_models.json", r#"["kling2_6"]"#);
        assert_pass(&check_data_file(repo.path()), "valid array of strings");
    }

    #[test]
    fn data_file_fails_when_missing() {
        let repo = TestRepo::new("data-fail-missing");
        assert_fail(&check_data_file(repo.path()), "missing data file");
    }

    #[test]
    fn data_file_fails_for_valid_json_with_the_wrong_shape() {
        let repo = TestRepo::new("data-fail-shape");
        repo.write("data/negative_prompt_models.json", r#"{"a":1}"#);
        assert_fail(
            &check_data_file(repo.path()),
            "valid JSON that isn't an array of strings",
        );
    }

    #[test]
    fn data_file_fails_for_unparseable_json() {
        let repo = TestRepo::new("data-fail-parse");
        repo.write("data/negative_prompt_models.json", "not json at all");
        assert_fail(&check_data_file(repo.path()), "unparseable JSON");
    }

    // --- check 3: binary freshness ------------------------------------------------------

    #[test]
    fn binary_freshness_passes_when_the_binary_is_newer_than_every_tracked_source() {
        let repo = TestRepo::new("fresh-pass");
        repo.write("rust/src/main.rs", "fn main() {}");
        repo.write("Cargo.toml", "[package]\nname=\"x\"");
        repo.write("data/negative_prompt_models.json", "[]");
        let exe_mtime = SystemTime::now() + Duration::from_secs(3600);
        assert_pass(
            &check_binary_freshness(repo.path(), exe_mtime),
            "binary newer than all sources",
        );
    }

    #[test]
    fn binary_freshness_fails_when_a_source_file_is_newer_than_the_binary() {
        let repo = TestRepo::new("fresh-fail");
        repo.write("rust/src/main.rs", "fn main() {}");
        repo.write("Cargo.toml", "[package]\nname=\"x\"");
        repo.write("data/negative_prompt_models.json", "[]");
        // The fixture files above were just written (mtime ~= now); an exe
        // "built" at the Unix epoch is unambiguously stale relative to them.
        let exe_mtime = SystemTime::UNIX_EPOCH;
        assert_fail(
            &check_binary_freshness(repo.path(), exe_mtime),
            "source newer than a stale binary",
        );
    }

    #[test]
    fn binary_freshness_fails_when_only_the_data_file_is_newer() {
        let repo = TestRepo::new("fresh-fail-data");
        // No rust/src, no Cargo.toml at all - only the data file is present
        // and newer, which alone must still be enough to fail.
        repo.write("data/negative_prompt_models.json", "[]");
        assert_fail(
            &check_binary_freshness(repo.path(), SystemTime::UNIX_EPOCH),
            "only data file newer",
        );
    }

    // --- check 4: settings file ----------------------------------------------------------

    #[test]
    fn settings_file_passes_with_a_pretooluse_array() {
        let repo = TestRepo::new("settings-pass");
        repo.write(
            ".claude/settings.json",
            r#"{"hooks":{"PreToolUse":[{"matcher":"x","hooks":[{"type":"command","command":"y"}]}]}}"#,
        );
        assert_pass(&check_settings_file(repo.path()), "valid settings file");
    }

    #[test]
    fn settings_file_fails_when_missing() {
        let repo = TestRepo::new("settings-fail-missing");
        assert_fail(&check_settings_file(repo.path()), "missing settings file");
    }

    #[test]
    fn settings_file_fails_when_unparseable() {
        let repo = TestRepo::new("settings-fail-parse");
        repo.write(".claude/settings.json", "not json");
        assert_fail(&check_settings_file(repo.path()), "unparseable settings file");
    }

    #[test]
    fn settings_file_fails_without_a_pretooluse_array() {
        let repo = TestRepo::new("settings-fail-shape");
        repo.write(".claude/settings.json", r#"{"hooks":{}}"#);
        assert_fail(
            &check_settings_file(repo.path()),
            "settings file missing hooks.PreToolUse",
        );
    }

    // --- check 5: matcher present ----------------------------------------------------------

    fn settings_json(command: &str, matcher: &str) -> String {
        serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": matcher,
                        "hooks": [{"type": "command", "command": command}]
                    }
                ]
            }
        })
        .to_string()
    }

    const REAL_MATCHER: &str = r"mcp__.*__generate_(image|video|audio)(_batch)?$";

    #[test]
    fn matcher_present_passes_when_a_command_resolves_to_this_exe() {
        let repo = TestRepo::new("present-pass");
        let exe = repo.path().join("target").join("release").join("higgsfield-lint");
        repo.write(
            ".claude/settings.json",
            &settings_json(
                "\"$CLAUDE_PROJECT_DIR/target/release/higgsfield-lint\"",
                REAL_MATCHER,
            ),
        );
        assert_pass(
            &check_matcher_present(repo.path(), repo.path(), &exe, "test"),
            "command resolves to this exe",
        );
    }

    #[test]
    fn matcher_present_fails_when_no_command_resolves_to_this_exe() {
        let repo = TestRepo::new("present-fail");
        let exe = repo.path().join("target").join("release").join("higgsfield-lint");
        repo.write(
            ".claude/settings.json",
            &settings_json("\"$CLAUDE_PROJECT_DIR/some/other/tool\"", REAL_MATCHER),
        );
        assert_fail(
            &check_matcher_present(repo.path(), repo.path(), &exe, "test"),
            "no command resolves to this exe",
        );
    }

    #[test]
    fn matcher_present_fails_when_project_dir_disagrees_with_the_settings_command() {
        // The exact bug the reviewer flagged: settings.json's command would
        // resolve correctly if $CLAUDE_PROJECT_DIR were substituted with the
        // discovered repo root - but a live session substitutes its OWN
        // CLAUDE_PROJECT_DIR, which here points somewhere else entirely (a
        // worktree, a symlinked checkout). Passing a `project_dir` that
        // differs from `repo_root` must make this Fail, proving the function
        // actually uses `project_dir` for substitution rather than silently
        // falling back to `repo_root`.
        let repo = TestRepo::new("present-fail-wrong-project-dir");
        let exe = repo.path().join("target").join("release").join("higgsfield-lint");
        repo.write(
            ".claude/settings.json",
            &settings_json(
                "\"$CLAUDE_PROJECT_DIR/target/release/higgsfield-lint\"",
                REAL_MATCHER,
            ),
        );
        let wrong_project_dir = repo.path().join("not-the-real-project-dir");
        assert_fail(
            &check_matcher_present(repo.path(), &wrong_project_dir, &exe, "test"),
            "CLAUDE_PROJECT_DIR pointing at a different checkout than the discovered repo root",
        );
    }

    // --- check 6: matcher correctness -------------------------------------------------------

    #[test]
    fn matcher_correctness_passes_for_the_real_matcher() {
        let repo = TestRepo::new("correct-pass");
        let exe = repo.path().join("target").join("release").join("higgsfield-lint");
        repo.write(
            ".claude/settings.json",
            &settings_json(
                "\"$CLAUDE_PROJECT_DIR/target/release/higgsfield-lint\"",
                REAL_MATCHER,
            ),
        );
        assert_pass(
            &check_matcher_correctness(repo.path(), repo.path(), &exe, "test"),
            "the real matcher regex",
        );
    }

    #[test]
    fn matcher_correctness_fails_when_the_matcher_misses_generate_video() {
        let repo = TestRepo::new("correct-fail-misses");
        let exe = repo.path().join("target").join("release").join("higgsfield-lint");
        repo.write(
            ".claude/settings.json",
            &settings_json(
                "\"$CLAUDE_PROJECT_DIR/target/release/higgsfield-lint\"",
                r"mcp__.*__generate_image$",
            ),
        );
        assert_fail(
            &check_matcher_correctness(repo.path(), repo.path(), &exe, "test"),
            "matcher that misses generate_video",
        );
    }

    #[test]
    fn matcher_correctness_fails_when_the_matcher_wrongly_matches_generate_3d() {
        let repo = TestRepo::new("correct-fail-3d");
        let exe = repo.path().join("target").join("release").join("higgsfield-lint");
        repo.write(
            ".claude/settings.json",
            &settings_json(
                "\"$CLAUDE_PROJECT_DIR/target/release/higgsfield-lint\"",
                r"mcp__.*__generate_.*$",
            ),
        );
        assert_fail(
            &check_matcher_correctness(repo.path(), repo.path(), &exe, "test"),
            "matcher that overmatches generate_3d",
        );
    }

    #[test]
    fn matcher_correctness_fails_when_the_matcher_wrongly_matches_show_generations() {
        let repo = TestRepo::new("correct-fail-show");
        let exe = repo.path().join("target").join("release").join("higgsfield-lint");
        repo.write(
            ".claude/settings.json",
            &settings_json(
                "\"$CLAUDE_PROJECT_DIR/target/release/higgsfield-lint\"",
                // Deliberately overmatches: no anchor to `generate_`, so it also
                // matches `..._show_generations` via the `generat` substring.
                r"mcp__.*generat.*",
            ),
        );
        assert_fail(
            &check_matcher_correctness(repo.path(), repo.path(), &exe, "test"),
            "matcher that overmatches show_generations",
        );
    }

    #[test]
    fn matcher_correctness_fails_when_project_dir_disagrees_with_the_settings_command() {
        // Same regression as check 5's version: an otherwise-correct settings
        // file must not be reported correct when resolved under a
        // `project_dir` other than the one it actually names.
        let repo = TestRepo::new("correct-fail-wrong-project-dir");
        let exe = repo.path().join("target").join("release").join("higgsfield-lint");
        repo.write(
            ".claude/settings.json",
            &settings_json(
                "\"$CLAUDE_PROJECT_DIR/target/release/higgsfield-lint\"",
                REAL_MATCHER,
            ),
        );
        let wrong_project_dir = repo.path().join("not-the-real-project-dir");
        assert_fail(
            &check_matcher_correctness(repo.path(), &wrong_project_dir, &exe, "test"),
            "CLAUDE_PROJECT_DIR pointing at a different checkout than the discovered repo root",
        );
    }

    // --- ProjectDir: the three CLAUDE_PROJECT_DIR states --------------------------------------
    //
    // `resolve_project_dir` takes the environment value as a plain parameter
    // rather than reading `std::env::var` itself, specifically so these three
    // states are testable without mutating real process-global environment
    // state (which would need serialising across the whole test binary, since
    // Rust tests run in parallel threads by default).

    #[test]
    fn project_dir_falls_back_to_discovered_root_when_env_var_is_unset() {
        let discovered = PathBuf::from("/some/discovered/repo/root");
        let project_dir = resolve_project_dir(None, &discovered);
        // Asserts the actual variant, not just the derived path()/warning() -
        // a bug that conflates "unset" with "agrees" (e.g. forgetting to
        // branch on `Some(_)` at all) would otherwise slip past a test that
        // only checked the derived values, since both variants resolve to the
        // same `path()` and both have no warning.
        assert!(
            matches!(&project_dir, ProjectDir::Unset(p) if p == &discovered),
            "expected Unset({}), matched a different variant or path",
            discovered.display()
        );
        assert_eq!(project_dir.path(), discovered.as_path());
        assert!(
            project_dir.warning().is_none(),
            "unset is the normal case for a manual run and must not warn"
        );
        assert!(
            project_dir.note().contains("not set"),
            "the unset case must still say so in the output: {}",
            project_dir.note()
        );
    }

    #[test]
    fn project_dir_agrees_when_env_var_matches_discovered_root() {
        let repo = TestRepo::new("projdir-agrees");
        let discovered = repo.path().to_path_buf();
        let env_value = discovered.to_string_lossy().into_owned();
        let project_dir = resolve_project_dir(Some(&env_value), &discovered);
        assert!(
            matches!(&project_dir, ProjectDir::Agrees(p) if p == &discovered),
            "expected Agrees({}), matched a different variant or path",
            discovered.display()
        );
        assert_eq!(project_dir.path(), discovered.as_path());
        assert!(
            project_dir.warning().is_none(),
            "agreement must not produce a warning"
        );
    }

    #[test]
    fn project_dir_disagrees_and_warns_when_env_var_points_elsewhere() {
        let discovered = PathBuf::from("/discovered/repo/root");
        let env_value = "/completely/different/checkout";
        let project_dir = resolve_project_dir(Some(env_value), &discovered);
        assert!(
            matches!(&project_dir, ProjectDir::Disagrees { env, discovered: d }
                if env.as_path() == Path::new(env_value) && d.as_path() == discovered.as_path()),
            "expected Disagrees{{ env: {env_value}, discovered: {} }}, matched something else",
            discovered.display()
        );
        assert_eq!(
            project_dir.path(),
            Path::new(env_value),
            "a live session substitutes the real env value, not the discovered root"
        );
        assert!(
            matches!(project_dir.warning(), Some(Check::Warn(_))),
            "disagreement must surface a standalone Warn"
        );
    }

    // --- check 7: self-test --------------------------------------------------------------

    #[test]
    fn self_test_passes_on_the_current_build() {
        assert_pass(&check_self_test(), "self-test on the current build");
    }

    #[test]
    fn self_test_evaluation_passes_for_the_expected_ids() {
        assert_pass(
            &evaluate_self_test(&["resolution_claim", "prompt_negative_conflict", "negation_placement"]),
            "exactly the expected check_ids",
        );
    }

    #[test]
    fn self_test_evaluation_fails_for_wrong_ids() {
        assert_fail(
            &evaluate_self_test(&["resolution_claim"]),
            "missing findings the payload should have produced",
        );
        assert_fail(&evaluate_self_test(&[]), "no findings at all");
        assert_fail(
            &evaluate_self_test(&["resolution_claim", "prompt_negative_conflict", "negation_placement", "extra"]),
            "an extra unexpected finding",
        );
    }

    // --- exit code / report plumbing (still covered, from Task 1) -----------------------

    #[test]
    fn exit_code_is_zero_when_all_pass() {
        let checks = vec![Check::Pass("a".into()), Check::Pass("b".into())];
        assert_eq!(exit_code(&checks), 0);
    }

    #[test]
    fn exit_code_is_zero_when_worst_is_warn() {
        let checks = vec![Check::Pass("a".into()), Check::Warn("b".into())];
        assert_eq!(exit_code(&checks), 0);
    }

    #[test]
    fn exit_code_is_one_when_any_fail_present() {
        let checks = vec![Check::Pass("a".into()), Check::Fail("b".into())];
        assert_eq!(exit_code(&checks), 1);
    }
}
