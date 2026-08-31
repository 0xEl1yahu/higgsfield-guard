use serde_json::Value;
use std::collections::HashSet;

pub struct Finding {
    pub check_id: String,
    pub message: String,
}

// Populated in later tasks: no checks exist yet.
const CHECKS: &[&str] = &[];

pub fn load_models() -> HashSet<String> {
    HashSet::new()
}

pub fn lint(_tool_input: &Value, _models: &HashSet<String>) -> Vec<Finding> {
    let _ = CHECKS;
    Vec::new()
}

pub fn render(findings: &[Finding]) -> String {
    findings
        .iter()
        .map(|f| format!("[{}] {}", f.check_id, f.message))
        .collect::<Vec<_>>()
        .join("\n")
}
