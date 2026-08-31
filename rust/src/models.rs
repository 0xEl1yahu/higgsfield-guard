//! Port of `load_models` (`src/higgsfield_lint.py` lines ~18, ~195-198).

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

/// Locate `data/negative_prompt_models.json` at RUNTIME relative to the running
/// executable, walking up through ancestor directories until one contains it (or the
/// filesystem root is reached).
///
/// Python's oracle resolves its `DATA_FILE` via `Path(__file__).resolve().parent.parent /
/// "data" / ...` - relative to the *script's own location on disk*, evaluated every time
/// it runs, so it keeps working if the whole repo is moved. Baking the path in at Rust
/// *compile* time (e.g. via `env!("CARGO_MANIFEST_DIR")`) would defeat that: the binary
/// would keep pointing at the build machine's original path even after the repo moves,
/// which is exactly the portability the port is supposed to deliver. Resolving via
/// `current_exe()` at runtime instead is the equivalent: the binary is expected to live
/// under `<repo>/target/{debug,release}/higgsfield-lint` (or one level deeper, under
/// `target/{debug,release}/deps/`, for `cargo test` binaries), so walking upward from
/// wherever the running executable actually is finds `<repo>/data/...` regardless of
/// which of those it is, and regardless of where `<repo>` itself sits on disk.
fn locate_data_file() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent();
    while let Some(d) = dir {
        let candidate = d.join("data").join("negative_prompt_models.json");
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

/// Port of `load_models`: fail silent on any error - executable path unavailable, data
/// file not found by walking up from it, unreadable, malformed JSON, or JSON that isn't a
/// list of strings - by returning an empty set, exactly as the Python's bare
/// `except Exception: return frozenset()` does.
pub fn load_models() -> HashSet<String> {
    locate_data_file()
        .and_then(|path| fs::read_to_string(path).ok())
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
