//! Loads the negative-prompt model lexicon from `data/negative_prompt_models.json`. This
//! file is the specification for that load path - there is no second implementation to
//! port against any more (see `SPEC.md` §9 and §12.2).
//!
//! The data file itself was retired (SPEC.md §5.3; HIGGSFIELD_PROMPT_RESEARCH.md §6,
//! §8.3) along with `check_negation_placement`, its only reader - `load_models` now
//! always returns an empty set, via the same not-found path it always used for a missing
//! file, and is left as-is rather than special-cased for that.

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
    fn load_models_returns_empty_now_that_the_data_file_is_retired() {
        // Was `load_models_reads_the_real_data_file`, asserting `models.contains("kling2_6")`.
        // data/negative_prompt_models.json no longer exists (SPEC.md §5.3), so this now
        // exercises the same not-found fallback `locate_data_file` always had - proving
        // retirement fails open here rather than panicking or reading a stale file left
        // over on disk.
        let models = load_models();
        assert!(models.is_empty());
    }
}
