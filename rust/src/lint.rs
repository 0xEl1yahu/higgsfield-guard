use serde_json::Value;
use std::collections::HashSet;

// `checks` and `models` live in sibling files (rust/src/checks.rs, rust/src/models.rs)
// per the task-2 brief, but are declared here (rather than in main.rs) via an explicit
// `#[path]` so this task's file list ("Modify: rust/src/lint.rs" only, plus the two new
// files) holds: without `#[path]`, `mod checks;` declared in a non-root module would
// resolve to rust/src/lint/checks.rs instead of the sibling rust/src/checks.rs.
#[path = "checks.rs"]
mod checks;
#[path = "models.rs"]
mod models;

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
