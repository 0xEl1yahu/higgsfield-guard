//! Port of `load_models` (`src/higgsfield_lint.py` lines ~18, ~195-198).
#![allow(dead_code)]

use std::collections::HashSet;
use std::fs;

/// Path to the model lexicon, baked in at compile time. Python resolves its `DATA_FILE`
/// relative to the script's own location (`Path(__file__).resolve().parent.parent /
/// "data" / ...`) rather than the process's cwd, so it works no matter where the hook is
/// invoked from. `CARGO_MANIFEST_DIR` is the directory containing this crate's
/// `Cargo.toml` - the repo root - which is the Rust equivalent: it is resolved at compile
/// time, is stable across `cargo test`, `cargo build`, and `cargo build --release`
/// regardless of where the resulting executable ends up on disk, and needs no runtime
/// environment variable or cwd assumption.
const DATA_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data/negative_prompt_models.json");

/// Port of `load_models`: fail silent on any error - missing file, unreadable, malformed
/// JSON, or JSON that isn't a list of strings - by returning an empty set, exactly as the
/// Python's bare `except Exception: return frozenset()` does.
pub fn load_models() -> HashSet<String> {
    fs::read_to_string(DATA_FILE)
        .ok()
        .and_then(|contents| serde_json::from_str::<Vec<String>>(&contents).ok())
        .map(|list| list.into_iter().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_models_reads_the_real_data_file() {
        let models = load_models();
        assert!(models.contains("kling2_6"));
    }
}
