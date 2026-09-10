//! `higgsfield-lint-review` - the only reader of `data/findings.jsonl` (SPEC 9.1).
//!
//! This binary is the opposite of the hook in every way that matters: it is never on the
//! generation path, it is allowed to print, and it may exit non-zero. It never writes.
//!
//! It answers two questions the project could not previously ask:
//!
//! 1. **Is the advice worth reading?** Per-check fire rates over real traffic. SPEC 9
//!    states that a check firing on a known-good call is a bug; this is the instrument
//!    that finds those.
//! 2. **Which models actually accept a negative_prompt?** SPEC 5.3 requires that list to
//!    be evidence-seeded rather than guessed. Every call carrying a non-empty
//!    negative_prompt is evidence. This reports the candidates and prints the exact line
//!    to add - it does not edit the file, because promotion is a judgement and a commit,
//!    not a side effect of a hook run.

use serde_json::Value;
use std::collections::BTreeMap;

use higgsfield_lint::models::locate_repo_root;

/// A model needs this many distinct calls carrying a non-empty `negative_prompt` before
/// it is worth proposing. Five is a judgement call, not a measurement: low enough to be
/// reachable at this volume (SPEC 1 counts nine lifetime video calls), high enough that
/// one experiment does not rewrite the lexicon. Raise it if a promotion turns out wrong.
const PROMOTION_THRESHOLD: usize = 5;

fn main() {
    let Some(root) = locate_repo_root() else {
        eprintln!("higgsfield-lint-review: could not locate the repo root from the running binary");
        std::process::exit(1);
    };
    let log = root.join("data").join("findings.jsonl");
    let Ok(text) = std::fs::read_to_string(&log) else {
        println!("No findings log at {}.", log.display());
        println!("The hook writes one on its first Higgsfield generate_* call.");
        return;
    };

    let mut total = 0usize;
    let mut with_findings = 0usize;
    let mut per_check: BTreeMap<String, usize> = BTreeMap::new();
    let mut neg_calls: BTreeMap<String, usize> = BTreeMap::new();
    let mut first_ts = u64::MAX;
    let mut last_ts = 0u64;
    let mut malformed = 0usize;

    for line in text.lines() {
        let Ok(rec) = serde_json::from_str::<Value>(line) else {
            malformed += 1;
            continue;
        };
        total += 1;
        if let Some(ts) = rec.get("ts").and_then(Value::as_u64) {
            first_ts = first_ts.min(ts);
            last_ts = last_ts.max(ts);
        }
        let fired: Vec<&str> = rec
            .get("fired")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if !fired.is_empty() {
            with_findings += 1;
        }
        for id in fired {
            *per_check.entry(id.to_string()).or_insert(0) += 1;
        }
        if rec.get("neg").and_then(Value::as_bool) == Some(true) {
            if let Some(models) = rec.get("models").and_then(Value::as_array) {
                for m in models.iter().filter_map(Value::as_str) {
                    *neg_calls.entry(m.to_string()).or_insert(0) += 1;
                }
            }
        }
    }

    let pct = |n: usize| {
        if total == 0 {
            0.0
        } else {
            n as f64 * 100.0 / total as f64
        }
    };

    println!("higgsfield-lint review - {}", log.display());
    println!();
    println!("calls logged:           {total}");
    if total > 0 {
        println!("  span:                 {first_ts} .. {last_ts} (unix seconds)");
    }
    if malformed > 0 {
        println!("  malformed lines:      {malformed}  (skipped)");
    }
    println!(
        "calls with >=1 finding: {with_findings}  ({:.1}%)",
        pct(with_findings)
    );
    println!();
    println!("per-check fire rate");
    if per_check.is_empty() {
        println!("  (nothing has fired yet)");
    }
    for (id, n) in &per_check {
        println!("  {id:<26} {n:>5}  ({:.1}%)", pct(*n));
    }
    println!();

    let listed = higgsfield_lint::models::load_models();
    let mut candidates: Vec<(&String, &usize)> = neg_calls
        .iter()
        .filter(|(m, n)| **n >= PROMOTION_THRESHOLD && !listed.contains(*m))
        .collect();
    candidates.sort_by(|a, b| b.1.cmp(a.1));

    println!("promotion candidates for data/negative_prompt_models.json");
    println!(
        "  (models seen with a non-empty negative_prompt on >= {PROMOTION_THRESHOLD} calls, not already listed)"
    );
    if candidates.is_empty() {
        println!("  (none)");
    }
    for (model, n) in &candidates {
        println!("  {model:<26} {n:>5} calls");
    }
    if !candidates.is_empty() {
        let mut all: Vec<String> = listed.into_iter().collect();
        all.extend(candidates.iter().map(|(m, _)| (*m).clone()));
        all.sort();
        println!();
        println!("  To promote, edit data/negative_prompt_models.json to:");
        println!("  {}", serde_json::to_string(&all).unwrap_or_default());
        println!("  Then re-run `cargo test --release` and commit the change with the");
        println!("  evidence above in the commit message (SPEC 5.3: evidence-seeded).");
    }
}
