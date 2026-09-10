use std::io::{Read, Write};
use std::panic;

use higgsfield_lint::lint;

fn run() -> Option<String> {
    let mut buf = Vec::new();
    std::io::stdin().read_to_end(&mut buf).ok()?;
    let text = String::from_utf8(buf).ok()?; // invalid UTF-8 -> silence
    let payload: serde_json::Value = serde_json::from_str(&text).ok()?;
    let tool_input = payload.get("tool_input")?;
    #[cfg(debug_assertions)]
    {
        // Debug-only panic injection point. Nothing else in this codebase reads a
        // `__panic__` field - without this, fail_silent.rs's
        // `an_internal_panic_is_still_silent_and_zero` test passes identically whether
        // or not the panic hook + catch_unwind in `main()` actually work, since there
        // is nothing to trigger a panic for them to catch. This makes that test
        // genuinely discriminating: build and run it in a plain debug profile (`cargo
        // test`, not `--release`) and the assertion only holds because the panic
        // really happens and really gets caught. `#[cfg(debug_assertions)]` keeps this
        // out of `cargo build --release` entirely - the shipped hook has no code path
        // that panics on this or any other field name, so a real call happening to
        // contain a field named `__panic__` behaves exactly like any other unrecognised
        // field.
        if tool_input.get("params").and_then(|p| p.get("__panic__"))
            == Some(&serde_json::Value::Bool(true))
        {
            panic!("debug-only panic injection triggered by __panic__ field");
        }
    }
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
        let mut stdout = std::io::stdout();
        let _ = stdout.write_all(text.as_bytes());
        // The emitted JSON contains no literal newline, so a LineWriter over stdout
        // never auto-flushes on its own. This has worked without an explicit flush
        // because `process::exit` runs `rt::cleanup()`, which flushes stdout - but
        // that is a non-obvious dependency for the tool's only write, so make it
        // explicit rather than correct-by-accident.
        let _ = stdout.flush();
    }
    std::process::exit(0);
}
