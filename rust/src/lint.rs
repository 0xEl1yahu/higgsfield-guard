use serde_json::Value;
use std::collections::HashSet;

// `checks` and `models` are top-level sibling modules declared in main.rs (the crate
// root), which resolves rust/src/checks.rs and rust/src/models.rs natively since they
// are plain siblings of lint.rs - no `#[path]` juggling needed. `lint` is the
// orchestrator and depends on both; nesting them under `lint` (as an earlier revision
// did, to satisfy a file-list constraint from the brief) had that backwards.
use crate::models;

pub struct Finding {
    pub check_id: String,
    pub message: String,
}

pub fn load_models() -> HashSet<String> {
    models::load_models()
}

// `lint()` still ignores its inputs: wiring checks::check_resolution_claim (and the
// negation checks from Task 3) into the params-resolution/batch pipeline is Task 4's
// job. Task 2 only needs check_resolution_claim to be independently callable, which the
// tests in checks.rs exercise directly.
pub fn lint(_tool_input: &Value, _models: &HashSet<String>) -> Vec<Finding> {
    Vec::new()
}

pub fn render(findings: &[Finding]) -> String {
    findings
        .iter()
        .map(|f| format!("[{}] {}", f.check_id, f.message))
        .collect::<Vec<_>>()
        .join("\n")
}
