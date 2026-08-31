use std::io::{Read, Write};
use std::panic;

mod checks;
mod lint;
mod models;

fn run() -> Option<String> {
    let mut buf = Vec::new();
    std::io::stdin().read_to_end(&mut buf).ok()?;
    let text = String::from_utf8(buf).ok()?;            // invalid UTF-8 -> silence
    let payload: serde_json::Value = serde_json::from_str(&text).ok()?;
    let tool_input = payload.get("tool_input")?;
    let models = lint::load_models();
    let findings = lint::lint(tool_input, &models);
    if findings.is_empty() {
        return None;
    }
    let out = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "additionalContext": lint::render(&findings),
        }
    });
    serde_json::to_string(&out).ok()
}

fn main() {
    // SPEC 7: the hook must be invisible when it fails. A default panic hook would
    // print to stderr, which on PreToolUse is a blocking signal. Silence it first,
    // then swallow the unwind.
    panic::set_hook(Box::new(|_| {}));
    let result = panic::catch_unwind(run).unwrap_or(None);
    if let Some(text) = result {
        let _ = std::io::stdout().write_all(text.as_bytes());
    }
    std::process::exit(0);
}
