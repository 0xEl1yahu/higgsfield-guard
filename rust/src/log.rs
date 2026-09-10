//! One JSONL record per linted call, appended best-effort. SPEC 9.1.
//!
//! This exists because SPEC 4 deleted the outcome log, which left the project unable to
//! answer the only two questions that decide whether the checks in 5 should exist: is
//! the advice ever right, and how often does it fire on a good call. Both are queries
//! over this file.
//!
//! Three properties are load-bearing:
//!
//! - **Metadata only.** Check ids, model names and booleans. No prompt text ever reaches
//!   this file - that keeps user content off disk and keeps every record far under the
//!   atomic-append budget below.
//! - **Every call, not every finding.** A clean call is logged with an empty `fired`.
//!   Without the clean calls there is no denominator and no rate.
//! - **Silent on every failure.** SPEC 7. An unlocatable root, an unwritable path, a
//!   serialisation failure and an oversized record all return quietly. The lint's stdout
//!   is identical whether the append happened or not.

use serde_json::Value;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use crate::lint::{resolve_params, Finding};
use crate::models::locate_repo_root;

/// Records are metadata only and run to a couple of hundred bytes; anything larger is a
/// bug upstream and is dropped rather than written.
///
/// Note on atomicity, because the obvious citation is wrong: `PIPE_BUF` governs pipes and
/// FIFOs, not regular files, and it is 512 on macOS rather than 4096. What POSIX actually
/// guarantees for `O_APPEND` on a regular file is that the seek-to-end and the write
/// happen atomically with respect to other writers - so concurrent hook processes never
/// clobber each other. A short write could in principle still split a line; for a
/// sub-kilobyte write to a local filesystem it does not (measured: 40 concurrent
/// appenders, 40/40 well-formed lines). This cap keeps records far inside that regime.
const MAX_RECORD_BYTES: usize = 4000;

/// Set `HIGGSFIELD_LINT_NO_LOG` to any value to suppress the append. The test harnesses
/// set it: ~320 synthetic parity calls per run would otherwise be indistinguishable from
/// real usage in every rate the review tool reports.
fn logging_disabled() -> bool {
    std::env::var_os("HIGGSFIELD_LINT_NO_LOG").is_some()
}

/// Where the record goes. `HIGGSFIELD_LINT_LOG` overrides the default so each test can
/// own a private file: cargo runs integration tests in parallel threads, and tests that
/// shared the one real log would race each other and pollute it. Returns `None` when the
/// repo root cannot be found and no override is set - one more silent no-op.
fn log_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("HIGGSFIELD_LINT_LOG") {
        return Some(PathBuf::from(p));
    }
    Some(locate_repo_root()?.join("data").join("findings.jsonl"))
}

/// Every `params` object this call will actually lint, in either shape. Mirrors `lint`'s
/// batch-before-top-level precedence closely enough for counting; it is not part of the
/// lint's output and is not held to parity.
fn linted_params(tool_input: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    if let Some(Value::Array(requests)) = tool_input.get("requests") {
        for entry in requests {
            if let Some(p) = entry
                .as_object()
                .and_then(|e| resolve_params(e.get("params")))
            {
                out.push(p);
            }
        }
        if !requests.is_empty() {
            return out;
        }
    }
    if let Some(p) = resolve_params(tool_input.get("params")) {
        out.push(p);
    }
    out
}

/// Append one record. Returns nothing: there is no failure a caller could act on, and
/// SPEC 7 forbids reporting one.
pub fn record(payload: &Value, tool_input: &Value, findings: &[Finding]) {
    if logging_disabled() {
        return;
    }
    let Some(path) = log_path() else {
        return;
    };

    let params = linted_params(tool_input);
    let mut models: Vec<&str> = Vec::new();
    let mut neg = false;
    for p in &params {
        if let Some(m) = p.get("model").and_then(Value::as_str) {
            if !models.contains(&m) {
                models.push(m);
            }
        }
        if p.get("negative_prompt")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.trim().is_empty())
        {
            neg = true;
        }
    }

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let fired: Vec<&str> = findings.iter().map(|f| f.check_id.as_str()).collect();

    let rec = serde_json::json!({
        "ts": ts,
        "tool": payload.get("tool_name").and_then(Value::as_str).unwrap_or(""),
        "models": models,
        "neg": neg,
        "batch": tool_input.get("requests").and_then(Value::as_array).map_or(0, |r| r.len()),
        "fired": fired,
    });

    let Ok(mut line) = serde_json::to_string(&rec) else {
        return;
    };
    if line.len() >= MAX_RECORD_BYTES {
        return;
    }
    line.push('\n');

    let _ = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| f.write_all(line.as_bytes()));
}
