//! Port of the lint checks in `src/higgsfield_lint.py` (the READ-ONLY oracle for this
//! port; never edit that file). This module ports lines 20-200: the boundary scan
//! helpers (`has_term`, `find_original_case`), `check_resolution_claim`, and the two
//! negation checks (`check_prompt_negative_conflict`, `check_negation_placement`) plus
//! their shared helpers (`strip_negations`, `is_placement_noise`). The `lint`/`render`
//! orchestration (lines 203+) lives in `lint.rs`, which calls these checks from
//! `lint_params`.

use regex::Regex;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::LazyLock;

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
const RESOLUTION_PARAM: &[(&str, i64)] =
    &[("4k", 2160), ("1080p", 1080), ("720p", 720), ("480p", 480)];

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
/// after is an ASCII lowercase letter or digit. A rejected match does not stop the
/// search - scanning resumes from the next start position so a later, valid occurrence
/// can still be found.
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
/// Walks `text` as a `char` sequence, not bytes: this is what keeps non-ASCII input
/// (smart quotes, accented characters, multi-byte UTF-8 anywhere in the prompt) safe,
/// since every offset used here comes from `char_indices` and can never land
/// mid-codepoint.
///
/// F9: this used to `collect()` the ENTIRE text into a `Vec<(usize, char)>` on every
/// call, and this function is called roughly 35 times per params object (once per
/// resolution-claim term, once per conflict phrase in each of two fields, ...). On an
/// 18MB adversarial input that measured 2.31GB RSS against the Python oracle's 93MB -
/// the only path in this codebase that can blow both the memory AND the "never write to
/// stderr" constraints at once, because an allocator failure in Rust `abort()`s with a
/// message to stderr that `catch_unwind` cannot intercept (it isn't a panic).
///
/// This version never materialises the text as a buffer: `text.char_indices()` is a
/// cheap `Clone` (a couple of pointers over the existing `&str`, no allocation), so each
/// candidate start position gets its own cloned iterator to probe forward `m` characters
/// (`m` = the term length, a handful of characters at most for every lexicon entry this
/// is called with), then that probe is discarded. The only heap allocation left is
/// `term_chars`, sized to the short lexicon term, not the (potentially huge) haystack.
fn find_term_match(text: &str, term: &str, ignore_case: bool) -> Option<(usize, usize)> {
    let term_chars: Vec<char> = term.chars().collect();
    let m = term_chars.len();
    if m == 0 {
        return None;
    }
    // Replaced wholesale in the Unicode-fold task; clippy's `eq_ignore_ascii_case`
    // suggestion is the wrong shape because the fold is about to stop being ASCII-only.
    #[allow(clippy::manual_ignore_case_cmp)]
    let eq = |a: char, b: char| {
        if ignore_case {
            a.to_ascii_lowercase() == b.to_ascii_lowercase()
        } else {
            a == b
        }
    };

    let mut before: Option<char> = None;
    let mut positions = text.char_indices().peekable();
    while let Some(&(start, _)) = positions.peek() {
        let mut probe = positions.clone();
        let mut matched = true;
        let mut match_end = start;
        for term_c in &term_chars {
            match probe.next() {
                Some((idx, c)) if eq(c, *term_c) => match_end = idx + c.len_utf8(),
                _ => {
                    matched = false;
                    break;
                }
            }
        }
        if matched {
            let before_ok = before.is_none_or(|c| !is_boundary_char(c, ignore_case));
            let after_ok = probe
                .peek()
                .is_none_or(|&(_, c)| !is_boundary_char(c, ignore_case));
            if before_ok && after_ok {
                return Some((start, match_end));
            }
        }
        // Boundary check failed, or the term didn't match here: advance by one char and
        // keep scanning rather than giving up (e.g. "mp4k 4K" must still match on the
        // second occurrence).
        let (_, c0) = positions.next().unwrap();
        before = Some(c0);
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

// Directive phrases worth comparing across the two prompt fields. A curated list,
// because ordinary words legitimately appear in both and would drown the signal.
const CONFLICT_PHRASES: &[&str] = &[
    "slow motion",
    "slow-mo",
    "motion blur",
    "static",
    "blurred",
    "blurry",
    "sharp",
    "text",
    "close-up",
    "wide shot",
    "zoom",
    "pan",
    "handheld",
    "grain",
];

// "no text", "without a logo", "avoid harsh shadows" - up to three trailing words.
// Feeds strip_negations (via check_prompt_negative_conflict): "not blurry" must still read
// as a negation there, or it looks like a request for blur. `not` stays in this pattern.
static NEGATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:no|without|avoid|not)\s+(?:[a-z-]+\s+){0,2}[a-z-]+").unwrap()
});

// SPEC §5.3 names exactly three directive forms: "no X", "without X", "avoid X". `not` is
// not one of them - "the sun has not yet risen" and "does not move" are ordinary narrative
// negation, not an instruction to move anything into negative_prompt. check_negation_placement
// uses this narrower pattern instead of NEGATION_RE so it stops firing on plain English.
static PLACEMENT_NEGATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:no|without|avoid)\s+(?:[a-z-]+\s+){0,2}[a-z-]+").unwrap()
});

// Quantifier idioms where "no" modifies a comparative or adverb rather than introducing an
// excluded noun - "no more than three [people]" is a count, not a directive, and moving
// "more than three" into negative_prompt is meaningless. Keep this list small; add an entry
// only when a real false positive demonstrates the idiom.
const PLACEMENT_STOPWORDS: &[(&str, &str)] = &[("no", "more"), ("no", "longer"), ("no", "matter")];

/// Port of `_is_placement_noise`.
pub fn is_placement_noise(match_text: &str) -> bool {
    let lower = match_text.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    if words.len() >= 2 && PLACEMENT_STOPWORDS.contains(&(words[0], words[1])) {
        return true;
    }
    // The distinguishing feature here is the negator, not the gerund. "without moving"
    // is a manner adverbial - "sitting without moving" describes how the subject
    // behaves, not an object to exclude from the shot. "no flickering", "no warping",
    // and "avoid flickering" name an artifact to exclude and must still fire; they
    // happen to end in "-ing" too, so the gerund suffix alone can't be the test. Only
    // a bare two-word "without + gerund" span is manner-adverbial noise; the same
    // shape after "no" or "avoid" is a directive naming a defect, so it is left alone.
    if words.len() == 2 && words[0] == "without" && words[1].ends_with("ing") {
        return true;
    }
    false
}

/// Remove negated spans so a negated term does not read as a positive request. Port of
/// `strip_negations`.
pub fn strip_negations(text: &str) -> String {
    NEGATION_RE.replace_all(text, " ").into_owned()
}

/// Port of `check_prompt_negative_conflict` (higgsfield_lint.py lines 157-172).
pub fn check_prompt_negative_conflict(params: &Value) -> Option<Finding> {
    let prompt = params.get("prompt").and_then(Value::as_str)?;
    let negative = params.get("negative_prompt").and_then(Value::as_str)?;
    // `has_term` (Task 2) is deliberately case-sensitive, matching the oracle's
    // `_has_term`. The oracle's `check_prompt_negative_conflict` compensates by
    // lowercasing BOTH arguments before ever calling it (oracle lines 162-163); these
    // two `.lower()`/`to_lowercase()` calls are that compensation, replicated
    // explicitly rather than folded into `has_term` itself.
    let positive = strip_negations(prompt).to_lowercase();
    let negative_low = negative.to_lowercase();
    let hits: Vec<&str> = CONFLICT_PHRASES
        .iter()
        .filter(|p| has_term(&positive, p) && has_term(&negative_low, p))
        .copied()
        .collect();
    if hits.is_empty() {
        return None;
    }
    let listed = hits
        .iter()
        .map(|h| format!("\"{}\"", h))
        .collect::<Vec<_>>()
        .join(", ");
    let verb = if hits.len() == 1 { "appears" } else { "appear" };
    Some(Finding {
        check_id: "prompt_negative_conflict".to_string(),
        message: format!(
            "{} {} in both prompt and negative_prompt. Keep each term in one.",
            listed, verb
        ),
    })
}

/// Port of `check_negation_placement` (higgsfield_lint.py lines 175-200).
pub fn check_negation_placement(params: &Value, models: &HashSet<String>) -> Option<Finding> {
    let prompt = params.get("prompt").and_then(Value::as_str)?;
    let model = params.get("model").and_then(Value::as_str);
    let negative = params.get("negative_prompt").and_then(Value::as_str);
    let supports =
        negative.is_some_and(|n| !n.trim().is_empty()) || model.is_some_and(|m| models.contains(m));
    if !supports {
        return None;
    }
    let negations: Vec<String> = PLACEMENT_NEGATION_RE
        .find_iter(prompt)
        .map(|m| m.as_str().trim().to_string())
        .filter(|s| !is_placement_noise(s))
        .collect();
    if negations.is_empty() {
        return None;
    }
    let shown = negations
        .iter()
        .take(3)
        .map(|n| format!("\"{}\"", n))
        .collect::<Vec<_>>()
        .join(", ");
    let more = if negations.len() > 3 {
        format!(" and {} more", negations.len() - 3)
    } else {
        String::new()
    };
    let verb = if negations.len() == 1 { "sits" } else { "sit" };
    Some(Finding {
        check_id: "negation_placement".to_string(),
        message: format!(
            "{}{} {} in the positive prompt while this model accepts negative_prompt. \
             Move them there; models follow negation poorly in the prompt.",
            shown, more, verb
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashSet;

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
        assert!(has_term(
            "na\u{ef}ve 4k cr\u{e8}me br\u{fb}l\u{e9}e scene",
            "4k"
        ));
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
    fn resolution_claim_ignores_asymmetric_boolean_width() {
        // F1: every other boolean case in the corpus pairs `true` with `true`, which
        // does not discriminate a buggy `is_int_dimension` that coerces `Bool(true)` to
        // `1` (max(1,1)=1 already misses the floor, so correct and buggy code produce
        // identical silence). Pairing a boolean `width` with a real `height` that would
        // defeat the claim on its own closes that hole: if `is_int_dimension` wrongly
        // returned `Some(1)` for `Bool(true)`, this would compute a 1px short side and
        // FIRE against the "8k" claim; the correct implementation excludes the boolean
        // entirely and stays silent.
        let params = json!({"prompt": "shot in 8k", "width": true, "height": 3840});
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

    // --- Task 3: negation checks -------------------------------------------------

    fn empty_models() -> HashSet<String> {
        HashSet::new()
    }

    fn model_set(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    // check_negation_placement must stay SILENT on ordinary narrative negation and on
    // quantifier idioms - the false-positive class a full fix wave closed in the Python.
    #[test]
    fn negation_placement_silent_on_not_as_narrative_negation() {
        let params = json!({
            "prompt": "a lighthouse at dawn, the sun has not yet risen over the water",
            "negative_prompt": "blurry"
        });
        assert!(check_negation_placement(&params, &empty_models()).is_none());
    }

    #[test]
    fn negation_placement_silent_on_no_more_than_quantifier_idiom() {
        let params = json!({
            "prompt": "a busy street with no more than three people visible",
            "negative_prompt": "blurry"
        });
        assert!(check_negation_placement(&params, &empty_models()).is_none());
    }

    #[test]
    fn negation_placement_silent_on_without_gerund_manner_adverbial() {
        let params = json!({
            "prompt": "a monk sitting without moving, breathing slowly",
            "negative_prompt": "blurry"
        });
        assert!(check_negation_placement(&params, &empty_models()).is_none());
    }

    // check_negation_placement must FIRE on directive negations naming an artifact to
    // exclude, including "-ing" directives that are not manner adverbials.
    #[test]
    fn negation_placement_fires_on_directive_negations() {
        for prompt in [
            "no text",
            "no logos",
            "no clutter",
            "without a watermark",
            "no lettering",
            "no flickering",
            "no warping",
            "avoid flickering",
        ] {
            let params = json!({"prompt": prompt, "negative_prompt": "blurry"});
            assert!(
                check_negation_placement(&params, &empty_models()).is_some(),
                "should fire on: {prompt}"
            );
        }
    }

    #[test]
    fn negation_placement_requires_negative_prompt_support_or_model_support() {
        // Neither a non-empty negative_prompt nor a model in `models` -> silent, even
        // though the prompt itself contains a directive negation.
        let params = json!({"prompt": "no text"});
        assert!(check_negation_placement(&params, &empty_models()).is_none());

        let params_blank_negative = json!({"prompt": "no text", "negative_prompt": "   "});
        assert!(check_negation_placement(&params_blank_negative, &empty_models()).is_none());

        // A model present in `models` is sufficient even with no negative_prompt field.
        let params_model = json!({"prompt": "no text", "model": "kling2_6"});
        assert!(check_negation_placement(&params_model, &model_set(&["kling2_6"])).is_some());
    }

    #[test]
    fn negation_placement_pluralises_the_verb() {
        let one = json!({"prompt": "no text", "negative_prompt": "blurry"});
        let finding_one = check_negation_placement(&one, &empty_models()).expect("fires");
        assert!(finding_one.message.contains(" sits in the positive prompt"));

        let two = json!({"prompt": "no text, no clutter", "negative_prompt": "blurry"});
        let finding_two = check_negation_placement(&two, &empty_models()).expect("fires");
        assert!(finding_two.message.contains(" sit in the positive prompt"));
    }

    #[test]
    fn negation_placement_lists_at_most_three_and_counts_the_rest() {
        let params = json!({
            "prompt": "no text, no logos, no clutter, no watermark, no flickering",
            "negative_prompt": "blurry"
        });
        let finding = check_negation_placement(&params, &empty_models()).expect("fires");
        assert!(finding.message.contains("and 2 more"));
    }

    // check_prompt_negative_conflict: agreement (a term negated in the prompt but
    // present in negative_prompt) is not a conflict; the same term un-negated in both
    // fields is.
    #[test]
    fn prompt_negative_conflict_fires_on_slow_motion_in_both_fields() {
        let params = json!({
            "prompt": "dynamic scene, slow motion, cinematic",
            "negative_prompt": "blurry, slow motion"
        });
        let finding = check_prompt_negative_conflict(&params).expect("should fire");
        assert_eq!(finding.check_id, "prompt_negative_conflict");
        assert!(finding.message.contains("\"slow motion\""));
        assert!(finding
            .message
            .contains("appears in both prompt and negative_prompt"));
    }

    #[test]
    fn prompt_negative_conflict_silent_when_prompt_negates_the_shared_term() {
        // "no text" in the prompt strips "text" from the positive reading, so its
        // appearance in negative_prompt is agreement, not conflict.
        let params = json!({
            "prompt": "no text, cat on a windowsill",
            "negative_prompt": "random text"
        });
        assert!(check_prompt_negative_conflict(&params).is_none());
    }

    #[test]
    fn prompt_negative_conflict_is_case_insensitive_via_explicit_lower() {
        // Regression guard for the .lower() calls: has_term itself is case-sensitive
        // (Task 2), so check_prompt_negative_conflict must lowercase both arguments
        // itself before comparing.
        let params = json!({
            "prompt": "Slow Motion hero shot, cinematic",
            "negative_prompt": "Blurry, Slow Motion"
        });
        let finding =
            check_prompt_negative_conflict(&params).expect("should fire despite mixed case");
        assert!(finding.message.contains("\"slow motion\""));
    }

    #[test]
    fn prompt_negative_conflict_pluralises_the_verb_for_three_hits() {
        let params = json!({
            "prompt": "sharp, static, grain, cinematic scene",
            "negative_prompt": "sharp, static, grain"
        });
        let finding = check_prompt_negative_conflict(&params).expect("should fire");
        assert!(finding
            .message
            .contains(" appear in both prompt and negative_prompt"));
    }

    #[test]
    fn prompt_negative_conflict_silent_when_either_field_missing_or_not_string() {
        assert!(check_prompt_negative_conflict(&json!({"prompt": "slow motion"})).is_none());
        assert!(check_prompt_negative_conflict(
            &json!({"prompt": 1, "negative_prompt": "slow motion"})
        )
        .is_none());
    }

    #[test]
    fn strip_negations_removes_no_without_avoid_and_not_spans() {
        // The pattern's middle group is greedy, so it consumes up to its full 3-word
        // span when the text supports it - "no text here" is entirely swallowed (no,
        // text, here), matching the oracle's `NEGATION_RE.sub(" ", ...)` exactly.
        assert_eq!(strip_negations("no text here").trim(), "");
        assert_eq!(strip_negations("not blurry").trim(), "");
        assert_eq!(strip_negations("without a watermark").trim(), "");
        assert_eq!(strip_negations("avoid harsh shadows").trim(), "");
        // A trailing word beyond the pattern's 3-word reach survives untouched.
        assert_eq!(strip_negations("no text please").trim(), "");
        assert_eq!(
            strip_negations("no clutter, cat on a windowsill").trim(),
            ", cat on a windowsill"
        );
    }

    #[test]
    fn is_placement_noise_flags_quantifier_idioms_and_without_gerund_only() {
        assert!(is_placement_noise("no more than three"));
        assert!(is_placement_noise("no longer visible"));
        assert!(is_placement_noise("no matter what"));
        assert!(is_placement_noise("without moving"));
        // Not noise: "no" + gerund is a directive, not a manner adverbial.
        assert!(!is_placement_noise("no flickering"));
        assert!(!is_placement_noise("avoid flickering"));
        // Not noise: "without" + gerund + a third word is no longer the bare two-word
        // manner-adverbial shape.
        assert!(!is_placement_noise("without a watermark"));
    }
}
