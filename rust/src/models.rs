//! Port of `load_models` (`src/higgsfield_lint.py` lines ~18, ~195-198).

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

/// Locate the repository root at RUNTIME by walking up from the running executable until
/// a directory containing `data/negative_prompt_models.json` is found.
///
/// The data file is the anchor rather than the target: callers that need a file which
/// does not exist yet (`data/findings.jsonl` on a first run) need the directory, not the
/// file. See the note below on why this is resolved at run time and not baked in at
/// compile time.
///
/// Python's oracle resolves its `DATA_FILE` via
/// `Path(__file__).resolve().parent.parent` - relative to the script's own location on
/// disk, evaluated every time it runs, so it keeps working if the whole repo is moved. Baking the path in at Rust *compile* time
/// (e.g. via `env!("CARGO_MANIFEST_DIR")`) would defeat that. The binary is expected to
/// live under `<repo>/target/{debug,release}/higgsfield-lint` (or one level deeper under
/// `deps/` for `cargo test` binaries), so walking upward finds `<repo>` regardless of
/// which it is and regardless of where `<repo>` sits on disk.
pub fn locate_repo_root() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent();
    while let Some(d) = dir {
        if d.join("data").join("negative_prompt_models.json").is_file() {
            return Some(d.to_path_buf());
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
    locate_repo_root()
        .and_then(|root| {
            fs::read_to_string(root.join("data").join("negative_prompt_models.json")).ok()
        })
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
