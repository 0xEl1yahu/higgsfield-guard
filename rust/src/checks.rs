//! Port of the lint checks in `src/higgsfield_lint.py` (the READ-ONLY oracle for this
//! port; never edit that file). This module currently ports lines 20-108: the boundary
//! scan helpers (`has_term`, `find_original_case`) and `check_resolution_claim`. The
//! negation checks (lines 111-200) and the `lint`/`render` orchestration (lines 203+)
//! are later tasks and are not wired up here yet, so some items in this file are not
//! called from anywhere but their own tests.
#![allow(dead_code)]

use serde_json::Value;

use crate::lint::Finding;

// Claimed resolution -> minimum acceptable short side in pixels. Order matters: when a
// prompt claims multiple resolutions, the highest-pixel claim wins; ties are broken by
// first-encountered, mirroring Python dict iteration order plus the strict `>` in
// `_claimed_short_side`.
const RESOLUTION_CLAIMS: &[(&str, i64)] = &[
    ("8k", 4320),
    ("4k", 2160),
    ("uhd", 2160),
    ("1080p", 1080),
    ("full hd", 1080),
    ("720p", 720),
    ("480p", 480),
];

// Values the `resolution` parameter itself can take.
const RESOLUTION_PARAM: &[(&str, i64)] = &[
    ("4k", 2160),
    ("1080p", 1080),
    ("720p", 720),
    ("480p", 480),
];

// A dimension only counts when it looks like a real render size. The 128px floor is meant
// to ignore aspect-ratio expressions (e.g. width=16, height=9), which have BOTH sides
// small - so the floor is keyed on the LARGER of the two dimensions, not applied to each
// side individually. A pair like 1920x100 has a genuinely narrow short side and must
// still be reported; requiring the floor on both sides would suppress it too, which is
// exactly the false negative the Python's "Patch 2" fixed.
const MIN_SANE_DIMENSION: i64 = 128;

/// Case-SENSITIVE "does `term` occur in `text` as a standalone token" check.
///
/// Python's oracle (`_has_term`) is `re.search(rf"(?<![a-z0-9]){escaped}(?![a-z0-9])",
/// text)` - **no** `re.IGNORECASE`. Both the literal and the `[a-z0-9]` boundary class are
/// case-sensitive, so an uppercase neighbour is not in the class and does not block a
/// match, and the literal itself only matches text with the exact same case as `term`.
/// This is deliberately different from `find_original_case` below, which folds case in
/// both the literal and the boundary class (Python passes `re.IGNORECASE` there). Do not
/// unify them: this is a parity port, and the oracle treats these two functions
/// differently on purpose.
///
/// In practice this case-sensitivity is currently unreachable: the only production call
/// site (`check_prompt_negative_conflict`, oracle line 164) always pre-lowercases both
/// arguments with `.lower()` before calling `_has_term`, so it only ever sees all-lowercase
/// input. Task 3's `check_prompt_negative_conflict` port must replicate those `.lower()`
/// calls itself rather than relying on `has_term` to fold case - it no longer does.
///
/// Rust's `regex` crate has no lookaround support (and `fancy-regex` is deliberately not
/// used - see the plan), so this is a manual boundary scan: find each occurrence of the
/// literal term (case-sensitive), then reject it if the character immediately before or
/// after is an ASCII lowercase letter or digit. A rejected match does not stop the search
/// - scanning resumes from the next start position so a later, valid occurrence can still
/// be found.
pub fn has_term(text: &str, term: &str) -> bool {
    find_term_match(text, term, false).is_some()
}

/// Same scan as `has_term`, but case-INSENSITIVE (both the literal and the boundary
/// class), and returns the matched substring in its ORIGINAL casing from `text` (e.g.
/// quoting `"4K"` as the user wrote it, not the lowercase lexicon key `"4k"`). Mirrors
/// Python's `_find_original_case`, which passes `re.IGNORECASE`.
pub fn find_original_case(text: &str, term: &str) -> Option<String> {
    find_term_match(text, term, true).map(|(start, end)| text[start..end].to_string())
}

/// Is `c` a boundary-blocking character under the given case sensitivity? Mirrors the
/// regex character class `[a-z0-9]`, which under `re.IGNORECASE` also matches `A-Z` (a
/// Python/`re` behaviour: `IGNORECASE` case-folds literal ranges inside character classes
/// too), and without it matches only lowercase letters and digits - an uppercase letter is
/// simply not a member of `[a-z0-9]` and does not block anything.
fn is_boundary_char(c: char, ignore_case: bool) -> bool {
    if ignore_case {
        c.is_ascii_alphanumeric()
    } else {
        c.is_ascii_digit() || c.is_ascii_lowercase()
    }
}

/// Returns the byte range of the first valid (boundary-respecting) match, if any.
///
/// Walks `text` and `term` as `char` sequences throughout, not bytes: this is what keeps
/// non-ASCII input (smart quotes, accented characters, multi-byte UTF-8 anywhere in the
/// prompt) safe, since indexing by byte offset into `text_chars`/`term_chars` can never
/// land mid-codepoint. Byte offsets are only reintroduced at the very end, taken from
/// `char_indices`, purely to slice the ORIGINAL (not case-folded) text for
/// `find_original_case`.
fn find_term_match(text: &str, term: &str, ignore_case: bool) -> Option<(usize, usize)> {
    let term_chars: Vec<char> = term.chars().collect();
    let text_chars: Vec<(usize, char)> = text.char_indices().collect();
    let m = term_chars.len();
    let n = text_chars.len();
    if m == 0 || n < m {
        return None;
    }
    for start in 0..=(n - m) {
        let is_match = (0..m).all(|k| {
            let (_, tc) = text_chars[start + k];
            if ignore_case {
                tc.to_ascii_lowercase() == term_chars[k].to_ascii_lowercase()
            } else {
                tc == term_chars[k]
            }
        });
        if !is_match {
            continue;
        }
        let before_ok =
            start == 0 || !is_boundary_char(text_chars[start - 1].1, ignore_case);
        let after_ok =
            start + m == n || !is_boundary_char(text_chars[start + m].1, ignore_case);
        if before_ok && after_ok {
            let match_start = text_chars[start].0;
            let match_end = if start + m < n {
                text_chars[start + m].0
            } else {
                text.len()
            };
            return Some((match_start, match_end));
        }
        // Boundary check failed: keep scanning past this rejected match rather than
        // giving up (e.g. "mp4k 4K" must still match on the second occurrence).
    }
    None
}

/// Highest resolution claim present in the prompt, as (original_text, pixels). Port of
/// `_claimed_short_side`.
fn claimed_short_side(prompt: &str) -> Option<(String, i64)> {
    let mut best: Option<(String, i64)> = None;
    for (term, px) in RESOLUTION_CLAIMS {
        if let Some(original) = find_original_case(prompt, term) {
            let should_replace = match &best {
                None => true,
                Some((_, best_px)) => px > best_px,
            };
            if should_replace {
                best = Some((original, *px));
            }
        }
    }
    best
}

/// Port of `_is_int_dimension`, adapted to `serde_json::Value`. Unlike Python (where
/// `isinstance(True, int)` is `True`, forcing an explicit bool exclusion), `serde_json`
/// represents JSON booleans as `Value::Bool`, a distinct variant from `Value::Number`, so
/// no separate bool check is needed here. A JSON number written with a decimal point or
/// exponent (e.g. `100.0`) is an `f64` in `serde_json` and correctly does NOT count,
/// mirroring Python's `isinstance(100.0, int)` being `False`.
fn is_int_dimension(value: Option<&Value>) -> Option<i64> {
    let v = value?;
    if v.is_i64() || v.is_u64() {
        v.as_i64()
    } else {
        None
    }
}

struct Actual {
    short_side: i64,
    width: Option<i64>,
    height: Option<i64>,
}

/// Port of `_actual_short_side`.
fn actual_short_side(params: &Value) -> Option<Actual> {
    if let Some(res) = params.get("resolution").and_then(Value::as_str) {
        let lower = res.to_lowercase();
        if let Some((_, px)) = RESOLUTION_PARAM.iter().find(|(k, _)| *k == lower) {
            return Some(Actual {
                short_side: *px,
                width: None,
                height: None,
            });
        }
    }
    let width = is_int_dimension(params.get("width"));
    let height = is_int_dimension(params.get("height"));
    if let (Some(w), Some(h)) = (width, height) {
        if w.max(h) >= MIN_SANE_DIMENSION {
            return Some(Actual {
                short_side: w.min(h),
                width: Some(w),
                height: Some(h),
            });
        }
    }
    None
}

/// Port of `check_resolution_claim` (higgsfield_lint.py lines 89-108).
pub fn check_resolution_claim(params: &Value) -> Option<Finding> {
    let prompt = params.get("prompt").and_then(Value::as_str)?;
    let (claim_text, claim_px) = claimed_short_side(prompt)?;
    let actual = actual_short_side(params)?;
    if actual.short_side >= claim_px {
        return None;
    }
    let source = match actual.width {
        Some(w) => format!(
            "width {} x height {} gives a {}px short side",
            w,
            actual.height.expect("width present implies height present"),
            actual.short_side
        ),
        None => format!("the call produces a {}px short side", actual.short_side),
    };
    Some(Finding {
        check_id: "resolution_claim".to_string(),
        message: format!(
            "prompt says \"{}\" but {}. Raise resolution, or drop the claim from the prompt.",
            claim_text, source
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn has_term_respects_alphanumeric_boundaries() {
        // Must match. `has_term` is case-sensitive (see its doc comment), so these use
        // the lowercase literal that the real call site actually produces after its own
        // `.lower()` - matching mixed-case text is a separate, explicitly-tested behaviour
        // below, not this test's concern.
        for t in ["a 4k shot", "4k, cinematic", "shot in 4k.", "(4k)"] {
            assert!(has_term(t, "4k"), "should match: {t}");
        }
        // Must NOT match - lowercase-letter-or-digit on either side blocks the match.
        for t in ["mp4k", "4kb", "x4k", "a4k", "24k", "4k9"] {
            assert!(!has_term(t, "4k"), "should not match: {t}");
        }
        // Multi-word terms
        assert!(has_term("shot in full hd today", "full hd"));
        assert!(!has_term("xfull hd", "full hd"));
    }

    #[test]
    fn has_term_resumes_scanning_past_a_rejected_match() {
        // The first "4k" is glued to "mp" and must be rejected; the scan must not give up
        // there - the second, standalone "4k" must still be found.
        assert!(has_term("mp4k then a 4k shot", "4k"));
    }

    #[test]
    fn has_term_is_case_sensitive_unlike_find_original_case() {
        // Oracle: `_has_term` has no `re.IGNORECASE`. The boundary class `[a-z0-9]` only
        // blocks on lowercase letters/digits, so an UPPERCASE neighbour does not count as
        // an alphanumeric boundary and does NOT block the match.
        //   Python  _has_term("A4k respectfully", "4k") -> True
        //   Python  _has_term("4kB respectfully", "4k") -> True
        assert!(
            has_term("A4k respectfully", "4k"),
            "uppercase neighbour before the term must not block a case-sensitive match"
        );
        assert!(
            has_term("4kB respectfully", "4k"),
            "uppercase neighbour after the term must not block a case-sensitive match"
        );
        // The literal match itself is exact-case: "4K" (uppercase K) is not a
        // case-sensitive match for the lowercase term "4k" at all, regardless of
        // boundaries.
        assert!(!has_term("a 4K shot", "4k"));
        assert!(!has_term("shot in 4K.", "4k"));
    }

    #[test]
    fn find_original_case_returns_the_prompt_spelling() {
        assert_eq!(
            find_original_case("a 4K hero shot", "4k").as_deref(),
            Some("4K")
        );
        assert_eq!(
            find_original_case("UHD master", "uhd").as_deref(),
            Some("UHD")
        );
        assert_eq!(
            find_original_case("Full HD please", "full hd").as_deref(),
            Some("Full HD")
        );
        assert_eq!(find_original_case("nothing here", "4k"), None);
    }

    #[test]
    fn boundary_scan_handles_non_ascii_surroundings_without_panicking() {
        // Smart quotes and accented characters around/near the term must not cause a
        // byte-index panic (char vs byte indexing) and must not themselves count as
        // alphanumeric boundaries. Uses the lowercase literal for `has_term` (case-
        // sensitive); the uppercase-in-text case is exercised separately via
        // `find_original_case` (case-insensitive) below.
        assert!(has_term("\u{201c}4k\u{201d} caf\u{e9} shot", "4k"));
        assert!(has_term("na\u{ef}ve 4k cr\u{e8}me br\u{fb}l\u{e9}e scene", "4k"));
        assert_eq!(
            find_original_case("\u{201c}4K\u{201d} caf\u{e9} shot", "4k").as_deref(),
            Some("4K")
        );
    }

    #[test]
    fn resolution_claim_fires_at_1376x768_with_4k_claim() {
        // Mirrors the kling2_6_4k_slowmo fixture used by the Python test suite.
        let params = json!({
            "prompt": "cinematic, ultra-clean commercial style, 4K, shallow depth of field",
            "width": 1376,
            "height": 768
        });
        let finding = check_resolution_claim(&params).expect("should fire");
        assert_eq!(finding.check_id, "resolution_claim");
        assert!(finding.message.contains("\"4K\""));
        assert!(finding
            .message
            .contains("width 1376 x height 768 gives a 768px short side"));
    }

    #[test]
    fn resolution_claim_silent_when_resolution_param_satisfies_claim() {
        let params = json!({"prompt": "shot in 4k", "resolution": "4k"});
        assert!(check_resolution_claim(&params).is_none());
    }

    #[test]
    fn resolution_claim_resolution_param_takes_precedence_over_dimensions() {
        let params = json!({
            "prompt": "4K hero shot",
            "resolution": "4k",
            "width": 100,
            "height": 100
        });
        assert!(check_resolution_claim(&params).is_none());
    }

    #[test]
    fn resolution_claim_message_omits_dimensions_when_source_is_resolution_param() {
        let params = json!({"prompt": "shot in 4k", "resolution": "1080p"});
        let finding = check_resolution_claim(&params).expect("should fire");
        assert!(!finding.message.contains("width"));
        assert!(finding
            .message
            .contains("the call produces a 1080px short side"));
    }

    #[test]
    fn resolution_claim_ignores_boolean_dimensions() {
        let params = json!({"prompt": "a 4k shot", "width": true, "height": true});
        assert!(check_resolution_claim(&params).is_none());
    }

    #[test]
    fn resolution_claim_silent_on_aspect_ratio_style_pair() {
        let params = json!({"prompt": "a 4k banner shot", "width": 16, "height": 9});
        assert!(check_resolution_claim(&params).is_none());
    }

    #[test]
    fn resolution_claim_silent_when_both_dimensions_below_floor() {
        let params = json!({"prompt": "a 4k banner shot", "width": 64, "height": 64});
        assert!(check_resolution_claim(&params).is_none());
    }

    #[test]
    fn resolution_claim_fires_on_narrow_short_side_despite_wide_side() {
        let params = json!({"prompt": "shot in 4k", "width": 1920, "height": 100});
        let finding = check_resolution_claim(&params).expect("should fire");
        assert!(finding
            .message
            .contains("width 1920 x height 100 gives a 100px short side"));
    }

    #[test]
    fn resolution_claim_silent_when_no_claim_in_prompt() {
        let params = json!({"prompt": "a cat on a windowsill", "width": 100, "height": 100});
        assert!(check_resolution_claim(&params).is_none());
    }

    #[test]
    fn resolution_claim_silent_when_dimensions_satisfy_claim() {
        let params = json!({"prompt": "shot in 720p", "width": 1920, "height": 1080});
        assert!(check_resolution_claim(&params).is_none());
    }

    #[test]
    fn resolution_claim_silent_when_prompt_missing_or_not_string() {
        assert!(check_resolution_claim(&json!({"width": 100, "height": 100})).is_none());
        assert!(check_resolution_claim(&json!({"prompt": 123})).is_none());
    }

    #[test]
    fn resolution_claim_silent_on_clean_seedance_call() {
        // Mirrors the seedance_tree_clean fixture: "resolution": "1080p" with matching
        // 1920x1080 dimensions and no unmet claim.
        let params = json!({
            "prompt": "Flat 2D vector cartoon animation, no camera movement, no zoom, no pan.",
            "width": 1920,
            "height": 1080,
            "resolution": "1080p"
        });
        assert!(check_resolution_claim(&params).is_none());
    }
}
