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
    println!("doctor checks this checkout and binary; confirm the hook is loaded with /hooks and verify a hook trace in the target Claude Code session.");
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
            checks.push(check_self_test(None));
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
            "binary location: no ancestor directory of {} contains a data/ directory - \
             the diagnostic-only repository landmark",
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
            checks.push(check_matcher_correctness(
                root,
                project_dir.path(),
                &exe,
                &note,
            ));
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

    checks.push(check_self_test(repo_root.as_deref()));
    checks
}

/// Diagnostic-only root discovery; no data-file search runs on the hook path.
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

/// Check 2: `data/negative_prompt_models.json` was retired together with
/// `check_negation_placement`, the only thing that ever read it (SPEC.md §5.3;
/// HIGGSFIELD_PROMPT_RESEARCH.md §6, §8.3). This check now guards the retirement itself
/// rather than the file's shape: it passes when the file is absent, and fails if it has
/// reappeared, since its presence would mean something started reading unsupported
/// model-acceptance data again without doctor noticing. Keep this retirement check even if a later feature adds a separate data file.
fn check_data_file(repo_root: &Path) -> Check {
    let path = repo_root.join("data").join("negative_prompt_models.json");
    if path.is_file() {
        Check::Fail(format!(
            "data file: {} still exists but was retired in SPEC.md §5.3 - remove the unsupported file after reviewing why it reappeared",
            path.display()
        ))
    } else {
        Check::Pass(format!(
            "data file: {} is absent, as expected since its retirement (SPEC.md §5.3)",
            path.display()
        ))
    }
}

/// Diagnostic-only mtime check. Missing/unreadable build inputs are failures.
/// Timestamps are a heuristic, not proof of source identity.
fn check_binary_freshness(repo_root: &Path, exe_mtime: SystemTime) -> Check {
    fn inputs(root: &Path) -> Result<Vec<PathBuf>, String> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                let ty = entry.file_type()?;
                if ty.is_symlink() {
                    return Err(std::io::Error::other(format!(
                        "symlinked source needs manual verification: {}",
                        path.display()
                    )));
                }
                if ty.is_dir() {
                    walk(&path, out)?;
                } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                    out.push(path);
                }
            }
            Ok(())
        }
        let mut out = vec![root.join("Cargo.toml"), root.join("Cargo.lock")];
        for name in ["main.rs", "lint.rs", "checks.rs", "doctor.rs"] {
            let path = root.join("rust/src").join(name);
            if !path.is_file() {
                return Err(format!("missing required input {}", path.display()));
            }
        }
        walk(&root.join("rust/src"), &mut out)
            .map_err(|e| format!("cannot enumerate runtime sources: {e}"))?;
        Ok(out)
    }
    let files = match inputs(repo_root) {
        Ok(files) => files,
        Err(e) => return Check::Fail(format!("binary freshness: {e}")),
    };
    let mut newer = Vec::new();
    for path in files {
        let mtime = match fs::metadata(&path).and_then(|m| m.modified()) {
            Ok(time) if path.is_file() => time,
            Ok(_) => {
                return Check::Fail(format!(
                    "binary freshness: {} is not a file",
                    path.display()
                ))
            }
            Err(e) => {
                return Check::Fail(format!(
                    "binary freshness: {} missing/unreadable: {e}",
                    path.display()
                ))
            }
        };
        if mtime > exe_mtime {
            newer.push(path.display().to_string());
        }
    }
    newer.sort();
    if newer.is_empty() {
        Check::Pass("binary freshness: binary is newer than every build input (rust/src/**/*.rs, Cargo.toml, Cargo.lock); mtime is not an identity proof".into())
    } else {
        Check::Fail(format!(
            "binary freshness: STALE BINARY, rebuild needed - newer than the binary: {}",
            newer.join(", ")
        ))
    }
}

fn check_binary_freshness_real(repo_root: &Path, exe: &Path) -> Check {
    match fs::metadata(exe).and_then(|m| m.modified()) {
        Ok(mtime) => check_binary_freshness(repo_root, mtime),
        Err(err) => Check::Fail(format!(
            "binary freshness: could not read this binary's own mtime: {err}"
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
                format!(
                    "CLAUDE_PROJECT_DIR={} - see the project dir warning above",
                    env.display()
                )
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

fn find_own_hook_group<'a>(
    entries: &'a [Value],
    project_dir: &Path,
    exe: &Path,
) -> Option<&'a Value> {
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
fn check_matcher_present(
    repo_root: &Path,
    project_dir: &Path,
    exe: &Path,
    project_dir_note: &str,
) -> Check {
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

const SHOULD_MATCH: &[&str] = &[
    "mcp__ada4cfac-9b58-4bfb-9631-4a588b7a2e4b__generate_image",
    "mcp__ada4cfac-9b58-4bfb-9631-4a588b7a2e4b__generate_video",
    "mcp__ada4cfac-9b58-4bfb-9631-4a588b7a2e4b__generate_audio",
    "mcp__ada4cfac-9b58-4bfb-9631-4a588b7a2e4b__generate_image_batch",
    "mcp__ada4cfac-9b58-4bfb-9631-4a588b7a2e4b__generate_video_batch",
    "mcp__ada4cfac-9b58-4bfb-9631-4a588b7a2e4b__generate_audio_batch",
    "mcp__claude_ai_Higgsfield__generate_image",
    "mcp__claude_ai_Higgsfield__generate_video",
    "mcp__claude_ai_Higgsfield__generate_audio",
    "mcp__claude_ai_Higgsfield__generate_image_batch",
    "mcp__claude_ai_Higgsfield__generate_video_batch",
    "mcp__claude_ai_Higgsfield__generate_audio_batch",
];
const SHOULD_NOT_MATCH: &[&str] = &[
    "mcp__x__generate_3d",
    "mcp__x__show_generations",
    "mcp__x__generate_audio_clone",
    "mcp__x__show_generate_audio",
    "mcp__x__generate_video_extra",
];

/// Check 6: the matcher on this binary's own `PreToolUse` entry (resolved the
/// same way check 5 does, via `project_dir`) matches a real Higgsfield tool
/// name and rejects the two nearest lookalikes.
fn check_matcher_correctness(
    repo_root: &Path,
    project_dir: &Path,
    exe: &Path,
    project_dir_note: &str,
) -> Check {
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
            "matcher correctness: the matching PreToolUse entry has no \"matcher\" string"
                .to_string(),
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

    for good in SHOULD_MATCH {
        if !re.is_match(good) {
            return Check::Fail(format!(
                "matcher correctness: matcher {matcher:?} does not match {good:?}"
            ));
        }
    }
    for bad in SHOULD_NOT_MATCH {
        if re.is_match(bad) {
            return Check::Fail(format!(
                "matcher correctness: matcher {matcher:?} wrongly matches {bad:?}"
            ));
        }
    }
    Check::Pass(format!(
        "matcher correctness: matcher {matcher:?} matches all six tool shapes under both server-name styles and rejects lookalikes ({project_dir_note})"
    ))
}

// `negation_placement` dropped out of this list when `check_negation_placement` was
// disabled (SPEC.md §5.3; HIGGSFIELD_PROMPT_RESEARCH.md §6, §8.3) - the payload below
// still contains "no logos" deliberately, so this self-test also proves that disabled
// check stays silent on real input, not just that nothing here calls it any more.
const SELF_TEST_EXPECTED_CHECK_IDS: [&str; 2] = ["resolution_claim", "prompt_negative_conflict"];

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

/// Check 7: independent synthetic controls and the two committed input fixtures.
fn check_self_test(repo_root: Option<&Path>) -> Check {
    use lint::ToolKind;
    let synthetic = serde_json::json!({"params":{
        "model":"seedance_2_0","resolution":"720p",
        "prompt":"shot in 4k, no logos, handheld camera","negative_prompt":"handheld, blurry"
    }});
    let findings = lint::lint(&synthetic, ToolKind::Video);
    let ids: Vec<&str> = findings.iter().map(|f| f.check_id.as_str()).collect();
    if let fail @ Check::Fail(_) = evaluate_self_test(&ids) {
        return fail;
    }
    let duration =
        serde_json::json!({"params":{"model":"kling2_6","prompt":"Total: 15s","duration":5}});
    let quoted = serde_json::json!({"params":{"model":"seedance_2_0","prompt":"“４Ｋ”","resolution":"720p"}});
    for (name, input, kind, expected) in [
        (
            "duration video",
            &duration,
            ToolKind::Video,
            vec!["duration_claim"],
        ),
        ("duration image", &duration, ToolKind::Image, vec![]),
        ("duration audio", &duration, ToolKind::Audio, vec![]),
        ("audio all checks", &synthetic, ToolKind::Audio, vec![]),
        ("quoted typography", &quoted, ToolKind::Video, vec![]),
    ] {
        let actual = lint::lint(input, kind);
        let ids: Vec<&str> = actual.iter().map(|f| f.check_id.as_str()).collect();
        if ids != expected {
            return Check::Fail(format!(
                "self-test: {name} produced {ids:?}, expected {expected:?}"
            ));
        }
    }
    let Some(root) = repo_root else {
        return Check::Fail(
            "self-test: synthetic controls passed but no repo root to verify committed fixtures"
                .into(),
        );
    };
    for (name, expected) in [
        (
            "tests/fixtures/kling2_6_4k_slowmo.json",
            vec!["prompt_negative_conflict"],
        ),
        ("tests/fixtures/seedance_tree_clean.json", vec![]),
    ] {
        let input = match fs::read_to_string(root.join(name))
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<Value>(&s).map_err(|e| e.to_string()))
        {
            Ok(input) if input.is_object() && input.get("params").is_some_and(Value::is_object) => {
                input
            }
            Ok(_) => {
                return Check::Fail(format!(
                    "self-test: {name} has wrong shape; expected object params"
                ))
            }
            Err(e) => {
                return Check::Fail(format!("self-test: {name} missing/unreadable/invalid: {e}"))
            }
        };
        let actual = lint::lint(&input, ToolKind::Video);
        let ids: Vec<&str> = actual.iter().map(|f| f.check_id.as_str()).collect();
        if ids != expected {
            return Check::Fail(format!(
                "self-test: {name} produced {ids:?}, expected {expected:?}"
            ));
        }
    }
    Check::Pass("self-test: synthetic scope/ordering/typography controls and both committed fixtures match expected ids".into())
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
            let path =
                std::env::temp_dir().join(format!("higgsfield-lint-doctor-test-{pid}-{label}-{n}"));
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
        let exe = repo
            .path()
            .join("target")
            .join("release")
            .join("higgsfield-lint");
        assert_eq!(find_repo_root(&exe), Some(repo.path().to_path_buf()));
    }

    #[test]
    fn binary_location_fails_when_no_ancestor_has_a_data_dir() {
        let repo = TestRepo::new("loc-fail");
        // No `data/` anywhere under repo.path() - and repo.path() itself is a
        // freshly created, uniquely-named directory under the system temp dir,
        // so none of ITS ancestors (the temp dir, "/", etc.) are expected to
        // contain a literal `data/` directory either.
        let exe = repo
            .path()
            .join("target")
            .join("release")
            .join("higgsfield-lint");
        assert_eq!(find_repo_root(&exe), None);
    }

    // --- check 2: data file (retired, SPEC.md §5.3) --------------------------------------

    #[test]
    fn data_file_passes_when_absent() {
        let repo = TestRepo::new("data-pass-absent");
        assert_pass(
            &check_data_file(repo.path()),
            "data file correctly absent after its SPEC.md §5.3 retirement",
        );
    }

    #[test]
    fn data_file_fails_when_still_present() {
        let repo = TestRepo::new("data-fail-present");
        // Content doesn't matter any more - the check no longer parses it, only checks
        // for existence - but a real-shaped payload is closer to what an accidental
        // resurrection would actually look like.
        repo.write("data/negative_prompt_models.json", r#"["kling2_6"]"#);
        assert_fail(
            &check_data_file(repo.path()),
            "data file present despite being retired",
        );
    }

    // --- check 3: binary freshness ------------------------------------------------------

    #[test]
    fn binary_freshness_passes_when_the_binary_is_newer_than_every_tracked_source() {
        let repo = TestRepo::new("fresh-pass");
        repo.write("rust/src/main.rs", "fn main() {}");
        repo.write("Cargo.toml", "[package]\nname=\"x\"");
        repo.write("Cargo.lock", "# fixture");
        for name in ["checks.rs", "lint.rs", "doctor.rs"] {
            repo.write(&format!("rust/src/{name}"), "// fixture");
        }
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
    fn binary_freshness_fails_when_build_inputs_are_missing() {
        let repo = TestRepo::new("fresh-fail-data");
        // A retired data file cannot substitute for required build inputs.
        repo.write("data/negative_prompt_models.json", "[]");
        assert_fail(
            &check_binary_freshness(repo.path(), SystemTime::UNIX_EPOCH),
            "required build inputs missing",
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
        assert_fail(
            &check_settings_file(repo.path()),
            "unparseable settings file",
        );
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
        let exe = repo
            .path()
            .join("target")
            .join("release")
            .join("higgsfield-lint");
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
        let exe = repo
            .path()
            .join("target")
            .join("release")
            .join("higgsfield-lint");
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
        let exe = repo
            .path()
            .join("target")
            .join("release")
            .join("higgsfield-lint");
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
        let exe = repo
            .path()
            .join("target")
            .join("release")
            .join("higgsfield-lint");
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
        let exe = repo
            .path()
            .join("target")
            .join("release")
            .join("higgsfield-lint");
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
        let exe = repo
            .path()
            .join("target")
            .join("release")
            .join("higgsfield-lint");
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
        let exe = repo
            .path()
            .join("target")
            .join("release")
            .join("higgsfield-lint");
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
        let exe = repo
            .path()
            .join("target")
            .join("release")
            .join("higgsfield-lint");
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
        assert_pass(
            &check_self_test(Some(Path::new(env!("CARGO_MANIFEST_DIR")))),
            "self-test on the current build",
        );
    }

    #[test]
    fn self_test_evaluation_passes_for_the_expected_ids() {
        assert_pass(
            &evaluate_self_test(&["resolution_claim", "prompt_negative_conflict"]),
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
            &evaluate_self_test(&["resolution_claim", "prompt_negative_conflict", "extra"]),
            "an extra unexpected finding",
        );
        assert_fail(
            &evaluate_self_test(&[
                "resolution_claim",
                "prompt_negative_conflict",
                "negation_placement",
            ]),
            "the retired check_id reappearing would mean check_negation_placement (or \
             something like it) is running again without SPEC.md being updated",
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
    #[test]
    fn freshness_rejects_missing_required_inputs_and_nested_new_source() {
        let repo = TestRepo::new("required");
        for name in ["main.rs", "checks.rs", "lint.rs", "doctor.rs"] {
            repo.write(&format!("rust/src/{name}"), "// fixture");
        }
        repo.write("Cargo.toml", "[package]");
        assert_fail(
            &check_binary_freshness(repo.path(), SystemTime::now() + Duration::from_secs(3600)),
            "missing Cargo.lock",
        );
        repo.write("Cargo.lock", "# fixture");
        let before = SystemTime::now() - Duration::from_secs(10);
        for name in ["main.rs", "checks.rs", "lint.rs", "doctor.rs"] {
            fs::File::open(repo.path().join("rust/src").join(name))
                .unwrap()
                .set_times(fs::FileTimes::new().set_modified(before))
                .unwrap();
        }
        for name in ["Cargo.toml", "Cargo.lock"] {
            fs::File::open(repo.path().join(name))
                .unwrap()
                .set_times(fs::FileTimes::new().set_modified(before))
                .unwrap();
        }
        let exe_time = SystemTime::now() - Duration::from_secs(5);
        assert_pass(&check_binary_freshness(repo.path(), exe_time), "old inputs");
        repo.write("rust/src/nested/new.rs", "// new");
        assert_fail(
            &check_binary_freshness(repo.path(), exe_time),
            "nested source",
        );
    }

    #[test]
    fn freshness_detects_only_new_cargo_lock() {
        let repo = TestRepo::new("lock-only");
        for name in ["main.rs", "checks.rs", "lint.rs", "doctor.rs"] {
            repo.write(&format!("rust/src/{name}"), "// fixture");
        }
        repo.write("Cargo.toml", "[package]");
        repo.write("Cargo.lock", "# fixture");
        let future = SystemTime::now() + Duration::from_secs(100);
        fs::File::open(repo.path().join("Cargo.lock"))
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(future))
            .unwrap();
        let check =
            check_binary_freshness(repo.path(), SystemTime::now() + Duration::from_secs(50));
        assert_fail(&check, "Cargo.lock only newer");
        assert!(describe(&check).contains("Cargo.lock"));
    }

    #[test]
    fn fixture_self_test_reports_missing_corrupt_shape_and_changed_findings() {
        let repo = TestRepo::new("fixture-failures");
        const KLING: &str = "tests/fixtures/kling2_6_4k_slowmo.json";
        const SEED: &str = "tests/fixtures/seedance_tree_clean.json";
        assert_fail(&check_self_test(None), "no root");
        for contents in [
            "not JSON",
            "[]",
            r#"{"params":"wrong"}"#,
            r#"{"params":{"prompt":"clean"}}"#,
        ] {
            repo.write(KLING, contents);
            let check = check_self_test(Some(repo.path()));
            assert_fail(&check, "bad fixture");
            assert!(describe(&check).contains(KLING));
        }
        repo.write(
            KLING,
            include_str!("../../tests/fixtures/kling2_6_4k_slowmo.json"),
        );
        assert_fail(&check_self_test(Some(repo.path())), "missing Seedance");
        repo.write(
            SEED,
            include_str!("../../tests/fixtures/seedance_tree_clean.json"),
        );
        assert_pass(&check_self_test(Some(repo.path())), "both fixtures");
    }

    #[test]
    fn matcher_must_cover_every_tool_family_and_batches() {
        let repo = TestRepo::new("matcher-six");
        let exe = repo.path().join("target/release/higgsfield-lint");
        for matcher in [
            r"mcp__.*__generate_(image|video)(_batch)?$",
            r"mcp__.*__generate_(image|video|audio)$",
        ] {
            repo.write(
                ".claude/settings.json",
                &settings_json(
                    "\"$CLAUDE_PROJECT_DIR/target/release/higgsfield-lint\"",
                    matcher,
                ),
            );
            assert_fail(
                &check_matcher_correctness(repo.path(), repo.path(), &exe, "test"),
                "incomplete matcher",
            );
        }
    }
}
