//! The lint checks: the boundary scan helpers (`has_term`, `find_original_case`,
//! `has_term_excluding`), `check_resolution_claim`, `check_prompt_negative_conflict`,
//! and `check_duration_claim`,
//! plus its shared helpers `strip_negations` and `quoted_spans`. The `lint`/`render`
//! orchestration lives in `lint.rs`, which calls these checks from `lint_params`.
//!
//! `check_negation_placement` was disabled (SPEC.md §5.3; HIGGSFIELD_PROMPT_RESEARCH.md
//! §6, §8.3) - contradicted by first-party guidance, with five confirmed false-positive
//! classes, now permanent negative controls in `rust/tests/golden/expectations.json`.
//! Its noise filter (`PLACEMENT_NEGATION_RE`, `is_placement_noise`, the stoplist) stays
//! below, unused, because HFR-006 (research §10) reuses it unchanged in a volume-based
//! replacement.
//!
//! `check_prompt_negative_conflict` was recalibrated (SPEC.md §5.2;
//! HIGGSFIELD_PROMPT_RESEARCH.md §8.2, §10) after three confirmed false positives against
//! the shipped binary: `pan` (common-noun homograph), `text` (a quoted literal Higgsfield
//! itself recommends pairing with a typography exclusion), and `sharp` (`not sharp` in
//! `negative_prompt` is agreement, not conflict, once negations are stripped from both
//! fields rather than the prompt alone). `quoted_spans` is the exclusion primitive that
//! change needed; resolution and duration reuse it with normalized text.
//!
//! Rust is the sole implementation, governed by SPEC and canonical evidence; there is
//! no second live implementation (see `SPEC.md` §9 and §12.2). A rule-behaviour change
//! here must update `SPEC.md` first, then `rust/tests/golden/expectations.json`
//! deliberately (see `rust/tests/golden/README.md` for the procedure).

use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

use crate::lint::{Finding, ToolKind};

// Ordered claim thresholds are a local convention, not universal model geometry.
const RESOLUTION_CLAIMS: &[(&str, i64)] =
    &[("4k", 2160), ("1080p", 1080), ("720p", 720), ("480p", 480)];

/// Each emitted scalar owns its original byte interval. No full-input source map.
struct Folded<'a> {
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
}
fn fold_scalar(c: char) -> char {
    match c {
        '\u{201c}'..='\u{201f}' => '"',
        '\u{ff01}'..='\u{ff5e}' => char::from_u32(c as u32 - 0xfee0).unwrap(),
        '\u{00a0}' | '\u{3000}' | '\t' => ' ',
        '\r' => '\n',
        _ => c,
    }
}
impl<'a> Folded<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            chars: text.char_indices().peekable(),
        }
    }
}
impl Iterator for Folded<'_> {
    type Item = (std::ops::Range<usize>, char);
    fn next(&mut self) -> Option<Self::Item> {
        let (start, original) = self.chars.next()?;
        let c = fold_scalar(original);
        let mut end = start + original.len_utf8();
        if original == '\r' && self.chars.peek().is_some_and(|(_, c)| *c == '\n') {
            let (i, c) = self.chars.next()?;
            end = i + c.len_utf8();
        } else if c == ' ' {
            while self
                .chars
                .peek()
                .is_some_and(|(_, c)| fold_scalar(*c) == ' ')
            {
                let (i, c) = self.chars.next()?;
                end = i + c.len_utf8();
            }
        }
        Some((start..end, c))
    }
}
fn normalize_for_matching(text: &str) -> std::borrow::Cow<'_, str> {
    let mut output: Option<String> = None;
    for (range, c) in Folded::new(text) {
        if output.is_none() && text[range.clone()].chars().eq(std::iter::once(c)) {
            continue;
        }
        let out = output.get_or_insert_with(|| {
            let mut out = String::with_capacity(text.len());
            out.push_str(&text[..range.start]);
            out
        });
        out.push(c);
    }
    output.map_or(std::borrow::Cow::Borrowed(text), std::borrow::Cow::Owned)
}
fn original_range_for_match(
    original: &str,
    range: std::ops::Range<usize>,
) -> Option<std::ops::Range<usize>> {
    if range.start >= range.end {
        return None;
    }
    let mut offset = 0;
    let mut start = None;
    for (source, c) in Folded::new(original) {
        if offset == range.start {
            start = Some(source.start);
        }
        offset += c.len_utf8();
        if offset == range.end {
            return Some(start?..source.end);
        }
        if offset > range.end {
            return None;
        }
    }
    None
}

/// Case-SENSITIVE "does `term` occur in `text` as a standalone token" check.
///
/// The archived Python oracle's `_has_term` (`docs/archive/verified-port/higgsfield_lint.py`)
/// was `re.search(rf"(?<![a-z0-9]){escaped}(?![a-z0-9])", text)` - **no** `re.IGNORECASE`.
/// Both the literal and the `[a-z0-9]` boundary class are case-sensitive, so an uppercase
/// neighbour is not in the class and does not block a match, and the literal itself only
/// matches text with the exact same case as `term`. This is deliberately different from
/// `find_original_case` below, which folds case in both the literal and the boundary class
/// (the oracle passed `re.IGNORECASE` there). Do not unify them: the archived oracle kept
/// these two functions different on purpose, and this file - now the specification in its
/// own right (see the header above) - preserves that distinction.
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
///   be found.
pub fn has_term(text: &str, term: &str) -> bool {
    find_term_match(text, term, false).is_some()
}

/// Same as `has_term`, but a match whose start position falls inside one of `excluded`'s
/// byte ranges is skipped rather than accepted - scanning continues past it, so a later,
/// non-excluded occurrence of the same term can still be found. Built for
/// `check_prompt_negative_conflict`'s quoted-literal exclusion (research §10, SPEC.md
/// §5.2), and shared - not reimplemented - for Task 4's resolution lexicon; see
/// `quoted_spans` below for how the ranges themselves are produced.
pub fn has_term_excluding(text: &str, term: &str, excluded: &[(usize, usize)]) -> bool {
    find_term_match_excluding(text, term, false, excluded).is_some()
}

/// Same scan as `has_term`, but case-INSENSITIVE (both the literal and the boundary
/// class), and returns the matched substring in its ORIGINAL casing from `text` (e.g.
/// quoting `"4K"` as the user wrote it, not the lowercase lexicon key `"4k"`). Mirrors
/// Python's `_find_original_case`, which passes `re.IGNORECASE`.
#[cfg(test)]
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
    find_term_match_excluding(text, term, ignore_case, &[])
}

/// Is byte offset `pos` inside any of `excluded`'s `[start, end)` ranges?
///
/// `excluded` is always `quoted_spans`' output: sorted by `start` and non-overlapping,
/// because `quoted_spans` builds spans by scanning `text` once, left to right, pairing
/// quotes in order. That invariant is what makes binary search valid here: since starts
/// are non-decreasing, the predicate `start <= pos` is true for a prefix of `excluded` and
/// false for the rest, so `partition_point` finds the index just past the last span that
/// could possibly contain `pos` in O(log S) comparisons, and only that one candidate span
/// needs an end-bound check.
///
/// F1: this used to be `excluded.iter().any(|&(start, end)| pos >= start && pos < end)`,
/// justified as cheap because "`excluded` is a handful of ranges per prompt at most". That
/// premise holds for ordinary input but is unbounded and false for adversarial input - this
/// function runs once per lexicon term per candidate match in `check_prompt_negative_conflict`
/// (`checks.rs::CONFLICT_PHRASES`'s hot loop), so a prompt built from many short quoted
/// segments made the whole check O(matches x spans). Measured against the pre-fix binary:
/// a repeated `'"close-up" '` prompt with `negative_prompt: "close-up"` went from 0.073s at
/// 2.8MB (this fix) to 9.149s (the linear scan) - see the task-3 fix report, finding F1, for
/// the full table. `quote_lookup_has_logarithmic_comparison_bound` below is
/// the regression test.
fn is_excluded(pos: usize, excluded: &[(usize, usize)]) -> bool {
    let idx = span_partition(pos, excluded, || {});
    idx > 0 && pos < excluded[idx - 1].1
}

// The observer counts the exact production comparisons; optimized away in normal calls.
fn span_partition(pos: usize, spans: &[(usize, usize)], mut observe: impl FnMut()) -> usize {
    spans.partition_point(|&(start, _)| {
        observe();
        start <= pos
    })
}
fn range_excluded(start: usize, end: usize, spans: &[(usize, usize)]) -> bool {
    if start >= end {
        return false;
    }
    let idx = span_partition(end - 1, spans, || {});
    idx > 0 && spans[idx - 1].1 > start
}

/// Same scan as `find_term_match`, with one addition: a match that would otherwise be
/// valid is skipped - not returned, but not treated as ending the search either - when
/// its start position falls inside one of `excluded`'s byte ranges. `find_term_match`
/// is this function called with an empty exclusion slice.
fn find_term_match_excluding(
    text: &str,
    term: &str,
    ignore_case: bool,
    excluded: &[(usize, usize)],
) -> Option<(usize, usize)> {
    let term_chars: Vec<char> = term.chars().collect();
    let m = term_chars.len();
    if m == 0 {
        return None;
    }
    let eq = |a: char, b: char| {
        if ignore_case {
            a.eq_ignore_ascii_case(&b)
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
            if before_ok && after_ok && !is_excluded(start, excluded) {
                return Some((start, match_end));
            }
        }
        // Boundary check failed, the term didn't match here, or the match was excluded:
        // advance by one char and keep scanning rather than giving up (e.g. "mp4k 4K"
        // must still match on the second occurrence; a term with one excluded and one
        // live occurrence must still find the live one).
        let (_, c0) = positions.next().unwrap();
        before = Some(c0);
    }
    None
}

/// Byte ranges of `text` that fall inside a MATCHED pair of ASCII double quotes (`"`).
/// Quotes are paired left-to-right, first with second, third with fourth, and so on; an
/// odd trailing `"` (an unterminated literal) has no partner and is not paired - nothing
/// after it is treated as quoted, per research §10 ("matched double quotes" only, not
/// "everything after the first quote"). Each returned range spans from the opening quote
/// character through the character immediately after the closing one, so a match whose
/// start position falls anywhere in `[open, close]` - including on the quote characters
/// themselves - counts as quoted.
///
/// Pairing is purely positional (1st-with-2nd, 3rd-with-4th, ...), not semantic, which has
/// two edges worth knowing about because neither one is "unterminated" and both look
/// surprising in isolation: an unrelated pair of inch-mark `"` characters (measurements,
/// not quotation) can accidentally pair up and swallow everything between them, including
/// a live directive - `a 24" monitor, close-up, 6" lens` pairs the `"` after `24` with the
/// `"` after `6`, and `close-up` sits inside that span and is excluded, even though nobody
/// quoted anything and the term should still fire. Conversely, back-to-back quote
/// characters pair with each other and leave what sits between the two pairs untouched -
/// `""close-up""` pairs the first two quote characters with each other and the last two
/// with each other, so `close-up` itself falls between the pairs, in neither span, and
/// still fires. Both follow directly from the left-to-right positional rule above; no
/// special casing is added for either, since research §10 specifies "matched double
/// quotes", full stop, not a smarter quote detector.
///
/// Shared, not reimplemented: this is the one helper `check_prompt_negative_conflict`
/// (this file) and Task 4's resolution-lexicon work both need for "a quoted literal is
/// content, not direction" (research §10; SPEC.md §5.2).
pub fn quoted_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut quotes = text.char_indices().filter(|&(_, c)| c == '"');
    while let Some((open, _)) = quotes.next() {
        let Some((close, close_char)) = quotes.next() else {
            break; // Unterminated - the trailing quote has no partner.
        };
        spans.push((open, close + close_char.len_utf8()));
    }
    spans
}

// Canonical MCP capture: docs/evidence/higgsfield-contract-2026-09-09.json.
// Positive recognition constants, not a runtime validator or a ceiling snapshot.
fn resolution_options(model: &str, kind: ToolKind) -> &'static [&'static str] {
    match (model, kind) {
        (
            "gpt_image_2"
            | "cinematic_studio_2_5"
            | "marketing_studio_image"
            | "ms_image"
            | "nano_banana_pro"
            | "nano_banana_2"
            | "gpt_image_2_5",
            ToolKind::Image,
        ) => &["1k", "2k", "4k"],
        ("nano_banana_2_lite", ToolKind::Image) => &["1k"],
        (
            "flux_2" | "kling_omni_image" | "grok_image" | "grok_image_2_0" | "recraft_v4_1",
            ToolKind::Image,
        ) => &["1k", "2k"],
        ("seedream_v5_pro", ToolKind::Image) => &["1k", "1.5k", "2k"],
        ("bytedance_image_upscale", ToolKind::Image) => &["2k", "4k"],
        ("cinematic_studio_3_0" | "seedance_2_0", ToolKind::Video) => {
            &["480p", "720p", "1080p", "4k"]
        }
        (
            "marketing_studio_video"
            | "grok_video_v15"
            | "seedance1_5"
            | "seedance_2_5"
            | "ad_multiplier"
            | "hf_mult_motion_control"
            | "hf_mult_replace_object"
            | "wan3_0"
            | "wan3_0_prime",
            ToolKind::Video,
        ) => &["480p", "720p", "1080p"],
        ("flux_3_video" | "kling3_0_turbo" | "happy_horse_video" | "wan2_7", ToolKind::Video) => {
            &["720p", "1080p"]
        }
        ("minimax_h3", ToolKind::Video) => &["2k"],
        ("minimax_h3_max", ToolKind::Video) => &["480p", "768p"],
        ("seedance_2_0_mini", ToolKind::Video) => &["480p", "720p"],
        ("topaz_video", ToolKind::Video) => &["1080p", "2160p"],
        ("bytedance_video_upscale", ToolKind::Video) => &["1080p", "2k", "4k"],
        ("gemini_omni", ToolKind::Video) => &["720p"],
        ("gemini_omni_flash_1_1", ToolKind::Video) => &["360p", "720p", "1080p", "4k"],
        _ => &[],
    }
}

fn known_mode(params: &Value, model: &str) -> bool {
    let options: &[&str] = match model {
        "grok_image" => &["std", "quality"],
        "cinematic_studio_video_v2" => &["pro", "std"],
        "seedance_2_0" => &["std", "fast"],
        "seedance_2_5" | "ad_multiplier" => {
            &["t2v", "omni_reference", "video_edit", "video_extension"]
        }
        "kling3_0" | "kling_video_edit" => &["std", "pro", "4k"],
        "gemini_omni_flash_1_1" => &[
            "text-to-video",
            "image-to-video",
            "reference-to-video",
            "edit",
        ],
        _ => return true,
    };
    match params.get("mode") {
        None => model != "gemini_omni_flash_1_1", // required in this catalog
        Some(Value::String(mode)) => options.contains(&mode.as_str()),
        _ => false,
    }
}

#[derive(Clone, Copy, Debug)]
enum Tier {
    NominalK(u8), // twice the nominal K label; not a pixel dimension
    VideoP(u16),
}

impl Tier {
    fn from_label(label: &str, kind: ToolKind) -> Option<Self> {
        match label {
            "1k" => Some(Self::NominalK(2)),
            "1.5k" => Some(Self::NominalK(3)),
            "2k" => Some(Self::NominalK(4)),
            "4k" => Some(Self::NominalK(8)),
            "360p" | "480p" | "720p" | "768p" | "1080p" | "2160p" if kind == ToolKind::Video => {
                Some(Self::VideoP(label.strip_suffix('p')?.parse().ok()?))
            }
            _ => None,
        }
    }

    fn below_claim(self, claim: i64) -> bool {
        match self {
            Self::NominalK(rank) => claim == 2160 && rank < 8,
            Self::VideoP(p) => claim > i64::from(p),
        }
    }
}

enum SubmittedSize<'a> {
    Tier {
        field: &'static str,
        label: &'a str,
        value: Tier,
    },
    Targets {
        width: i64,
        height: i64,
    },
}

fn int_target(v: Option<&Value>) -> Option<i64> {
    let n = v?;
    if !(n.is_i64() || n.is_u64()) {
        return None;
    }
    n.as_i64().filter(|n| *n > 0)
}

fn submitted_size(params: &Value, kind: ToolKind) -> Option<SubmittedSize<'_>> {
    if !matches!(kind, ToolKind::Image | ToolKind::Video) {
        return None;
    }
    let model = params.get("model")?.as_str()?;
    if !known_mode(params, model) {
        return None;
    }
    if kind == ToolKind::Image && matches!(model, "topaz_image" | "topaz_image_generative") {
        return Some(SubmittedSize::Targets {
            width: int_target(params.get("output_width"))?,
            height: int_target(params.get("output_height"))?,
        });
    }
    let (field, options): (&str, &[&str]) = match (model, kind) {
        ("wan2_6", ToolKind::Video) => ("quality", &["720p", "1080p"]),
        ("soul_2" | "soul_cinematic" | "soul_v2", ToolKind::Image) => ("quality", &["1.5k", "2k"]),
        ("kling3_0" | "kling_video_edit", ToolKind::Video) => ("mode", &["4k"]),
        _ => ("resolution", resolution_options(model, kind)),
    };
    let label = params.get(field)?.as_str()?;
    let lower = label.to_ascii_lowercase();
    if !options.contains(&lower.as_str()) {
        return None;
    }
    if model == "marketing_studio_video"
        && (params.get("width").is_some() || params.get("height").is_some())
    {
        return None; // two declared controls, unknown precedence
    }
    if model == "seedance_2_0"
        && params.get("mode").and_then(Value::as_str) == Some("fast")
        && matches!(lower.as_str(), "1080p" | "4k")
    {
        return None;
    }
    Some(SubmittedSize::Tier {
        field,
        label,
        value: Tier::from_label(&lower, kind)?,
    })
}

/// Compares only supplied, model-declared controls. Never promises a rendered size.
pub fn check_resolution_claim(params: &Value, kind: ToolKind) -> Option<Finding> {
    let source = submitted_size(params, kind)?;
    let prompt = params.get("prompt")?.as_str()?;
    let normalized = normalize_for_matching(prompt);
    let quoted = quoted_spans(&normalized);
    let (start, end, claim) = RESOLUTION_CLAIMS.iter().find_map(|(term, threshold)| {
        find_term_match_excluding(&normalized, term, true, &quoted)
            .map(|(start, end)| (start, end, *threshold))
    })?;
    let mismatch = match source {
        SubmittedSize::Tier { value, .. } => value.below_claim(claim),
        SubmittedSize::Targets { width, height } => width.min(height) < claim,
    };
    if !mismatch {
        return None;
    }
    let range = original_range_for_match(prompt, start..end)?;
    let spelling = prompt.get(range)?;
    let body = match source {
        SubmittedSize::Tier { field, label, .. } => format!(
            "{field} selects the {label:?} output tier. Choose a supported higher tier, or revise the claim."
        ),
        SubmittedSize::Targets { width, height } => format!(
            "requested output_width {width} x output_height {height} has a {}px short side under the local claim convention. Increase output_width/output_height, or revise the claim.",
            width.min(height)
        ),
    };
    Some(Finding {
        check_id: "resolution_claim".into(),
        message: format!("prompt says {spelling:?} but {body}"),
    })
}

// Output-duration declarations only. Source-processing durations are excluded.
// These ids are tested against the same dated MCP catalog as resolution_options.
const DURATION_MODELS: &[&str] = &[
    "cinematic_studio_3_0",
    "cinematic_studio_video",
    "cinematic_studio_video_v2",
    "marketing_studio_video",
    "flux_3_video",
    "grok_video_v15",
    "minimax_hailuo",
    "minimax_h3",
    "minimax_h3_max",
    "wan2_6",
    "seedance1_5",
    "seedance_2_0",
    "seedance_2_0_mini",
    "seedance_2_5",
    "ad_multiplier",
    "kling2_6",
    "kling3_0",
    "kling3_0_turbo",
    "happy_horse_video",
    "grok_video",
    "gemini_omni",
    "gemini_omni_flash_1_1",
    "wan2_7",
    "wan3_0",
    "wan3_0_prime",
    "veo3_1",
    "veo3_1_lite",
];

fn duration_eligible(params: &Value) -> bool {
    let Some(model) = params.get("model").and_then(Value::as_str) else {
        return false;
    };
    if !DURATION_MODELS.contains(&model) || !known_mode(params, model) {
        return false;
    }
    let mode = params.get("mode").and_then(Value::as_str);
    !((matches!(model, "seedance_2_5" | "ad_multiplier")
        && matches!(mode, Some("video_edit" | "video_extension")))
        || (model == "gemini_omni_flash_1_1" && mode == Some("edit")))
}

// Span branches precede single-number branches so a reversed/invalid range cannot
// be rescued as a shorter valid tail. Horizontal spaces only; ASCII decimals only.
static DURATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)(?:",
        r"total:[ \t]*(?P<total>[0-9]+(?:\.[0-9]+)?)[ \t]*(?:seconds?|s)",
        r"|(?P<from>[0-9]+(?:\.[0-9]+)?)[ \t]*(?:[-–—][ \t]*|s[ \t]+to[ \t]+)(?P<to>[0-9]+(?:\.[0-9]+)?)[ \t]*s",
        r"|(?P<seconds>[0-9]+(?:\.[0-9]+)?)(?:[-–—]|[ \t]+)seconds?",
        r"|(?P<ending>[0-9]+(?:\.[0-9]+)?)[ \t]*s[ \t]+total",
        r")"
    )).unwrap()
});

fn duration_prefix_bad(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-' | '–' | '—' | '.' | ',' | ':')
}
fn duration_max(prompt: &str) -> Option<f64> {
    let text = normalize_for_matching(prompt);
    let quotes = quoted_spans(&text);
    let mut best: Option<f64> = None;
    for cap in DURATION_RE.captures_iter(&text) {
        let whole = cap.get(0)?;
        if text[..whole.start()]
            .chars()
            .next_back()
            .is_some_and(duration_prefix_bad)
            || text[whole.end()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            || range_excluded(whole.start(), whole.end(), &quotes)
        {
            continue;
        }
        let number = cap
            .name("total")
            .or_else(|| cap.name("to"))
            .or_else(|| cap.name("seconds"))
            .or_else(|| cap.name("ending"))?;
        let Ok(value) = number.as_str().parse::<f64>() else {
            continue;
        };
        if !value.is_finite() {
            continue;
        }
        if let Some(from) = cap.name("from") {
            let Ok(start) = from.as_str().parse::<f64>() else {
                continue;
            };
            if !start.is_finite() || start > value {
                continue;
            }
        }
        if best.is_none_or(|b| value > b) {
            best = Some(value);
        }
    }
    best
}
fn seconds_display(n: f64) -> String {
    // f64 formatting is safe for the full finite range; avoid giant fixed displays.
    let plain = n.to_string();
    if plain.len() <= 24 {
        plain
    } else {
        format!("{n:.6e}")
    }
}
pub fn check_duration_claim(params: &Value) -> Option<Finding> {
    if !duration_eligible(params) {
        return None;
    }
    let supplied = params.get("duration")?.as_f64()?;
    if !supplied.is_finite() || !(1.0..=600.0).contains(&supplied) {
        return None;
    }
    let claimed = duration_max(params.get("prompt")?.as_str()?)?;
    if claimed <= supplied {
        return None;
    }
    Some(Finding {
        check_id: "duration_claim".into(),
        message: format!(
            "prompt describes {}s of action but the supplied duration is {}s. Check the model's supported duration or retime the beats.",
            seconds_display(claimed), seconds_display(supplied)
        ),
    })
}

// Directive phrases worth comparing across the two prompt fields. A curated list,
// because ordinary words legitimately appear in both and would drown the signal.
//
// Recalibrated (SPEC.md §5.2; HIGGSFIELD_PROMPT_RESEARCH.md §10) after three confirmed
// false positives: bare `pan`, `text`, and `sharp` each collided with an ordinary-English
// sense of the same word (a cast-iron pan, a quoted on-screen literal, "not sharp" as
// agreement rather than conflict). The rule going forward: every entry is either >=2
// tokens, or a single token from a set judged unambiguous, with a 4-character minimum
// (enforced by `conflict_phrases_single_token_entries_meet_the_four_char_minimum` below).
// `static`, `zoom`, and `grain` are dropped for the same homograph reason (a static shot
// vs static noise, a zoom lens vs a zoom move, wood grain vs film grain) even without a
// confirmed-in-the-wild hit; where the directive sense matters, a bigram takes the bare
// token's place instead.
//
// F3 (task-3 fix report): `letterbox` and `watermark` were demoted from bare single-token
// entries to bigrams after the same treatment turned up two more confirmed false
// positives, the same shape as `pan`:
//   {"prompt":"A red letterbox on a quiet village street at dawn.","negative_prompt":"letterbox, black bars, borders"}
//   {"prompt":"A close look at handmade paper with a visible watermark.","negative_prompt":"watermark, logo, signature"}
// `letterbox` names an actual mail slot/postbox as readily as it names black aspect-ratio
// bars, and `watermark` names actual paper watermarks (security paper, stationery,
// currency) as readily as an image overlay - both are common-noun homographs, not the
// "strict, unambiguous" single-token set SPEC.md §5.2 (recalibration change 3) requires.
// Research §10 listed both as single-token entries, but §10 also states that same
// unambiguity criterion; where the two conflict, the criterion governs, per SPEC.md §5.2's
// recorded ruling. `letterbox bars` and `watermark overlay` take their place, carrying the
// same directive sense without the homograph.
const CONFLICT_PHRASES: &[&str] = &[
    "slow motion",
    "slow-mo",
    "motion blur",
    "close-up",
    "wide shot",
    "handheld",
    "blurred",
    "blurry",
    "camera pan",
    "whip pan",
    "slow zoom",
    "on-screen text",
    "film grain",
    "static shot",
    "lens flare",
    "letterbox bars",
    "watermark overlay",
    "subtitles",
];

// "no text", "without a logo", "avoid harsh shadows" - up to three trailing words.
// Feeds strip_negations (via check_prompt_negative_conflict): "not blurry" must still read
// as a negation there, or it looks like a request for blur. `not` stays in this pattern.
static NEGATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:no|without|avoid|not)\s+(?:[a-z-]+\s+){0,2}[a-z-]+").unwrap()
});

// SPEC §5.3 named exactly three directive forms: "no X", "without X", "avoid X" - `not`
// was deliberately excluded, since "the sun has not yet risen" and "does not move" are
// ordinary narrative negation, not an instruction to move anything into negative_prompt.
// `check_negation_placement`, which used this narrower pattern instead of NEGATION_RE so
// it would stop firing on plain English, was disabled (SPEC.md §5.3;
// HIGGSFIELD_PROMPT_RESEARCH.md §6, §8.3: contradicted by first-party guidance, five
// confirmed false-positive classes). This regex, the stoplist below, and
// `is_placement_noise` are kept, unused, because HFR-006 (research §10) - a volume-based
// replacement that counts exclusions instead of judging their placement - reuses this
// exact noise filter unchanged; research's "Keep" section is explicit these filters are
// correct and carry over. `#[allow(dead_code)]` is deliberate, not an oversight: nothing
// calls these until HFR-006 lands.
#[allow(dead_code)]
static PLACEMENT_NEGATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:no|without|avoid)\s+(?:[a-z-]+\s+){0,2}[a-z-]+").unwrap()
});

// Quantifier idioms where "no" modifies a comparative or adverb rather than introducing an
// excluded noun - "no more than three [people]" is a count, not a directive, and moving
// "more than three" into negative_prompt is meaningless. Kept for HFR-006 (see the comment
// on `PLACEMENT_NEGATION_RE` above); add an entry only when a real false positive
// demonstrates the idiom.
#[allow(dead_code)]
const PLACEMENT_STOPWORDS: &[(&str, &str)] = &[("no", "more"), ("no", "longer"), ("no", "matter")];

/// Was `check_negation_placement`'s noise filter (port of `_is_placement_noise`, archived
/// Python oracle `docs/archive/verified-port/higgsfield_lint.py`). Kept unused for
/// HFR-006 - see the comment on `PLACEMENT_NEGATION_RE` above.
#[allow(dead_code)]
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

/// Originally a port of `check_prompt_negative_conflict` (archived Python oracle,
/// `docs/archive/verified-port/higgsfield_lint.py` lines 157-172); recalibrated since -
/// see the module doc comment, SPEC.md §5.2, and HIGGSFIELD_PROMPT_RESEARCH.md §8.2/§10.
pub fn check_prompt_negative_conflict(params: &Value) -> Option<Finding> {
    let prompt = params.get("prompt").and_then(Value::as_str)?;
    let negative = params.get("negative_prompt").and_then(Value::as_str)?;
    // `has_term` (Task 2) is deliberately case-sensitive, matching the oracle's
    // `_has_term`. The oracle's `check_prompt_negative_conflict` compensates by
    // lowercasing BOTH arguments before ever calling it (oracle lines 162-163); these
    // two `.lower()`/`to_lowercase()` calls are that compensation, replicated
    // explicitly rather than folded into `has_term` itself.
    //
    // Recalibration change 1 (research §10): negations are now stripped from BOTH
    // fields, not the prompt alone. "not sharp" in negative_prompt is a double negative
    // and must not read as agreement-turned-conflict with "sharp" in the prompt.
    let positive_stripped = strip_negations(&normalize_for_matching(prompt)).to_lowercase();
    let negative_stripped = strip_negations(&normalize_for_matching(negative)).to_lowercase();
    let positive = normalize_for_matching(&positive_stripped);
    let negative_low = normalize_for_matching(&negative_stripped);
    // Recalibration change 2: a quoted literal in the prompt is content the user asked
    // for verbatim (Nano Banana's own guidance), not a directive - so an occurrence
    // inside a matched `"..."` pair does not count towards a hit. Spans are computed on
    // `positive` itself (post-negation-strip, post-lowercase) so their byte offsets line
    // up exactly with the string `has_term_excluding` scans; quote characters are ASCII
    // and untouched by either transform, so this does not miss or misplace a pair.
    let quoted = quoted_spans(&positive);
    let hits: Vec<&str> = CONFLICT_PHRASES
        .iter()
        .filter(|p| has_term_excluding(&positive, p, &quoted) && has_term(&negative_low, p))
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_reconciliation_red_controls() {
        assert!(check_resolution_claim(&json!({"model":"seedance_2_0","prompt":"Style: 8K IMAX commercial","resolution":"1080p"}), ToolKind::Video).is_none(), "8K style must be silent");
        assert!(
            check_resolution_claim(
                &json!({"model":"nano_banana_pro","prompt":"poster, 4K","resolution":"2k"}),
                ToolKind::Image
            )
            .is_some(),
            "nominal tier mismatch must fire"
        );
        assert!(
            check_prompt_negative_conflict(
                &json!({"prompt":"“close-up”","negative_prompt":"close-up"})
            )
            .is_none(),
            "curly quotes protect literal"
        );
        assert!(
            check_prompt_negative_conflict(
                &json!({"prompt":"slow   motion","negative_prompt":"slow motion"})
            )
            .is_some(),
            "horizontal whitespace normalizes"
        );
    }

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

    // quoted_spans / has_term_excluding: the shared helper behind recalibration change 2
    // (SPEC.md §5.2), also earmarked for Task 4's resolution lexicon.

    #[test]
    fn quoted_spans_pairs_quotes_left_to_right_and_ignores_a_trailing_unpaired_quote() {
        let text = r#"A sign reads "OPEN" and another reads "CLOSED" past midnight"#;
        let spans = quoted_spans(text);
        assert_eq!(spans.len(), 2);
        assert_eq!(&text[spans[0].0..spans[0].1], "\"OPEN\"");
        assert_eq!(&text[spans[1].0..spans[1].1], "\"CLOSED\"");

        // An odd number of quotes: the third, trailing quote has no partner and is not
        // paired - only the first two are excluded.
        let unterminated = r#"A sign reads "OPEN" but this "one never closes"#;
        let spans = quoted_spans(unterminated);
        assert_eq!(spans.len(), 1);
        assert_eq!(&unterminated[spans[0].0..spans[0].1], "\"OPEN\"");
    }

    #[test]
    fn quoted_spans_is_empty_when_there_are_no_quotes() {
        assert!(quoted_spans("no quotes anywhere in this prompt").is_empty());
    }

    // Quote-pairing edges: positional pairing (1st-with-2nd, 3rd-with-4th, ...) has two
    // surprising consequences beyond the trailing-unpaired-quote case already covered
    // above - see the doc comment on `quoted_spans`.
    #[test]
    fn quoted_spans_lets_unrelated_inch_marks_accidentally_pair_and_swallow_a_directive() {
        // Two independent measurements, not a quotation - but the `"` after "24" and the
        // `"` after "6" pair up positionally, and "close-up" sits inside that accidental
        // span and reads as excluded.
        let text = r#"a 24" monitor, close-up, 6" lens"#;
        let spans = quoted_spans(text);
        assert_eq!(
            spans.len(),
            1,
            "the two inch-marks must pair into a single span"
        );
        assert!(
            !has_term_excluding(text, "close-up", &spans),
            "close-up sits inside the accidental inch-mark span and must be excluded"
        );
    }

    #[test]
    fn quoted_spans_leaves_a_doubled_quote_sandwiched_term_live() {
        // Back-to-back quote characters on each side pair with their immediate neighbour,
        // not across the term - so "close-up" itself sits between the two spans, not
        // inside either, and stays live.
        let doubled = "\"\"close-up\"\"";
        let spans = quoted_spans(doubled);
        assert_eq!(
            spans.len(),
            2,
            "the doubled quotes on each side must pair with each other"
        );
        assert!(
            has_term_excluding(doubled, "close-up", &spans),
            "close-up sits between the two quote pairs, not inside either, and must still match"
        );
    }

    #[test]
    fn has_term_excluding_skips_a_quoted_occurrence_but_still_finds_a_live_one() {
        let quoted_only = r#"the poster reads "close-up" in bold letters"#;
        let spans = quoted_spans(quoted_only);
        assert!(!has_term_excluding(quoted_only, "close-up", &spans));
        // Unexcluded (empty exclusion list), the same text still matches - proves the
        // exclusion, not some other change, is what suppressed it above.
        assert!(has_term_excluding(quoted_only, "close-up", &[]));

        // A term appearing once inside quotes and once outside must still be found via
        // the live, unquoted occurrence - exclusion skips a match, it does not stop the
        // scan.
        let mixed = r#"a close-up shot; the caption also reads "close-up" on screen"#;
        let spans = quoted_spans(mixed);
        assert!(has_term_excluding(mixed, "close-up", &spans));
    }

    // F1: is_excluded switched from a linear scan to a binary search over quoted_spans'
    // sorted, non-overlapping output. These pin down the boundary behaviour directly
    // (private fn, tested from within this module) so a future edit can't silently widen
    // or narrow which positions count as excluded.
    #[test]
    fn is_excluded_checks_boundaries_gaps_and_the_empty_case() {
        let spans: Vec<(usize, usize)> = vec![(5, 10), (20, 25), (40, 50)];
        // Empty exclusion list: nothing is excluded, anywhere.
        assert!(!is_excluded(0, &[]));
        assert!(!is_excluded(usize::MAX, &[]));
        // Before the first span.
        assert!(!is_excluded(0, &spans));
        assert!(!is_excluded(4, &spans));
        // On the opening boundary (inclusive) through the closing boundary (exclusive).
        assert!(is_excluded(5, &spans));
        assert!(is_excluded(9, &spans));
        assert!(!is_excluded(10, &spans));
        // In the gap between the first and second span.
        assert!(!is_excluded(15, &spans));
        assert!(!is_excluded(19, &spans));
        // The second span, by the same rule.
        assert!(is_excluded(20, &spans));
        assert!(is_excluded(24, &spans));
        assert!(!is_excluded(25, &spans));
        // The last span, and past the end of everything.
        assert!(is_excluded(40, &spans));
        assert!(is_excluded(49, &spans));
        assert!(!is_excluded(50, &spans));
        assert!(!is_excluded(1000, &spans));
    }

    #[test]
    fn quote_lookup_has_logarithmic_comparison_bound() {
        for count in [0usize, 1, 8, 8_000, 128_000] {
            let spans: Vec<_> = (0..count).map(|i| (i * 4, i * 4 + 2)).collect();
            for pos in [
                0,
                1,
                2,
                count * 2,
                count * 4,
                count.saturating_sub(1) * 4 + 1,
                usize::MAX,
            ] {
                let mut comparisons = 0;
                let index = span_partition(pos, &spans, || comparisons += 1);
                let bound = if count == 0 {
                    0
                } else {
                    usize::BITS - (count).leading_zeros() + 2
                };
                assert!(
                    comparisons <= bound,
                    "{count} spans: {comparisons} comparisons > {bound}"
                );
                assert_eq!(index, spans.iter().take_while(|&&(s, _)| s <= pos).count());
            }
        }
    }

    #[test]
    fn repeated_quoted_prompt_is_silent_but_final_live_occurrence_fires() {
        let mut p = json!({"prompt":"\"close-up\" ".repeat(64_000),"negative_prompt":"close-up"});
        assert!(check_prompt_negative_conflict(&p).is_none());
        p["prompt"] = json!(format!("{}close-up", p["prompt"].as_str().unwrap()));
        assert!(check_prompt_negative_conflict(&p).is_some());
        assert!(range_excluded(3, 8, &[(5, 10)]));
        assert!(!range_excluded(0, 5, &[(5, 10)]));
    }

    #[test]
    fn conflict_phrases_single_token_entries_meet_the_four_char_minimum() {
        // Recalibration change 3 (SPEC.md §5.2): every CONFLICT_PHRASES entry is either
        // >=2 tokens (contains a space) or a single token of at least 4 characters. This
        // does not (and cannot) enforce the "unambiguous" half of that rule - that is a
        // judgment call recorded in the comment above CONFLICT_PHRASES - but it does
        // enforce the length floor mechanically, so a future one-off short token cannot
        // slip back in unnoticed.
        for phrase in CONFLICT_PHRASES {
            if !phrase.contains(' ') {
                assert!(
                    phrase.chars().count() >= 4,
                    "single-token entry {phrase:?} is below the 4-character minimum"
                );
            }
        }
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
    fn resolution_claim_domains_sources_and_exclusions() {
        let cases = [
            (
                ToolKind::Image,
                json!({"model":"nano_banana_pro","prompt":"poster, 4K","resolution":"2k","aspect_ratio":"21:9"}),
                Some("resolution selects the \"2k\""),
            ),
            (
                ToolKind::Image,
                json!({"model":"seedream_v5_pro","prompt":"4K","resolution":"1.5k","width":4096,"height":4096}),
                Some("resolution selects the \"1.5k\""),
            ),
            (
                ToolKind::Video,
                json!({"model":"wan2_6","prompt":"4K","quality":"720p","width":4096,"height":4096,"resolution":"4k"}),
                Some("quality selects the \"720p\""),
            ),
            (
                ToolKind::Video,
                json!({"model":"minimax_h3","prompt":"4K","resolution":"2K"}),
                Some("resolution selects the \"2K\""),
            ),
            (
                ToolKind::Video,
                json!({"model":"minimax_h3_max","prompt":"1080p","resolution":"768p"}),
                Some("resolution selects the \"768p\""),
            ),
            (
                ToolKind::Video,
                json!({"model":"gemini_omni_flash_1_1","mode":"text-to-video","prompt":"480p","resolution":"360p"}),
                Some("resolution selects the \"360p\""),
            ),
            (
                ToolKind::Video,
                json!({"model":"kling2_6","prompt":"4K","width":1376,"height":768,"resolution":"720p"}),
                None,
            ),
            (
                ToolKind::Video,
                json!({"prompt":"4K","resolution":"720p","width":1280,"height":720}),
                None,
            ),
            (
                ToolKind::Unknown,
                json!({"model":"seedance_2_0","prompt":"4K","resolution":"720p"}),
                None,
            ),
            (
                ToolKind::Audio,
                json!({"model":"seedance_2_0","prompt":"4K","resolution":"720p"}),
                None,
            ),
            (
                ToolKind::Image,
                json!({"model":"seedance_2_0","prompt":"4K","resolution":"720p"}),
                None,
            ),
            (
                ToolKind::Image,
                json!({"model":"nano_banana_pro","prompt":"1080p","resolution":"2k"}),
                None,
            ),
            (
                ToolKind::Image,
                json!({"model":"nano_banana_pro","prompt":"4K","resolution":"4k"}),
                None,
            ),
            (
                ToolKind::Video,
                json!({"model":"wan2_6","prompt":"720p","quality":"720p"}),
                None,
            ),
            (
                ToolKind::Video,
                json!({"model":"kling3_0","prompt":"4K","mode":"4k"}),
                None,
            ),
            (
                ToolKind::Video,
                json!({"model":"kling3_0","prompt":"4K","mode":"1080p"}),
                None,
            ),
            (
                ToolKind::Video,
                json!({"model":"kling3_0","prompt":"4K","mode":"pro"}),
                None,
            ),
            (
                ToolKind::Video,
                json!({"model":"seedance_2_0","prompt":"4K","resolution":"1080p","mode":"fast"}),
                None,
            ),
            (
                ToolKind::Video,
                json!({"model":"seedance_2_0","prompt":"4K","resolution":"720p","mode":"typo"}),
                None,
            ),
            (
                ToolKind::Video,
                json!({"model":"marketing_studio_video","prompt":"4K","resolution":"720p","width":1280,"height":720}),
                None,
            ),
            (
                ToolKind::Video,
                json!({"model":"minimax_hailuo","prompt":"4K","resolution":"768"}),
                None,
            ),
            (
                ToolKind::Image,
                json!({"model":"text2image_soul_v2","prompt":"4K","quality":"2k"}),
                None,
            ),
            (
                ToolKind::Image,
                json!({"model":"gpt_image_2","prompt":"4K","quality":"high"}),
                None,
            ),
            (
                ToolKind::Image,
                json!({"model":"nano_banana_pro","prompt":"4K","resolution":720}),
                None,
            ),
            (
                ToolKind::Image,
                json!({"model":"nano_banana_pro","prompt":"4K","resolution":"native"}),
                None,
            ),
            (
                ToolKind::Image,
                json!({"model":"nano_banana_pro","prompt":123,"resolution":"2k"}),
                None,
            ),
        ];
        for (kind, params, expected) in cases {
            let result = check_resolution_claim(&params, kind);
            assert_eq!(result.is_some(), expected.is_some(), "{params} {kind:?}");
            if let (Some(f), Some(text)) = (result, expected) {
                assert_eq!(f.check_id, "resolution_claim");
                assert!(f.message.contains(text), "{}", f.message);
                assert!(!f.message.contains("produces") && !f.message.contains("short side"));
            }
        }
        for model in ["soul_2", "soul_cinematic", "soul_v2"] {
            assert!(check_resolution_claim(
                &json!({"model":model,"prompt":"4K","quality":"2k"}),
                ToolKind::Image
            )
            .is_some());
        }
    }

    #[test]
    fn resolution_claim_target_pixels_reject_malformed_and_keep_asymmetric_controls() {
        for model in ["topaz_image", "topaz_image_generative"] {
            for (w, h, fires) in [
                (1920, 100, true),
                (1, 1, true),
                (1376, 768, true),
                (3840, 2160, false),
            ] {
                let p = json!({"model":model,"prompt":"4K","output_width":w,"output_height":h});
                let f = check_resolution_claim(&p, ToolKind::Image);
                assert_eq!(f.is_some(), fires, "{p}");
                if let Some(f) = f {
                    assert!(f.message.contains("requested output_width"));
                }
            }
            for w in [
                json!(true),
                json!(false),
                json!(1280.0),
                json!("1280"),
                json!(null),
                json!(-1),
                json!(0),
                json!(u64::MAX),
            ] {
                let p = json!({"model":model,"prompt":"4K","output_width":w,"output_height":3840});
                assert!(check_resolution_claim(&p, ToolKind::Image).is_none(), "{p}");
            }
            assert!(check_resolution_claim(
                &json!({"model":model,"prompt":"4K","width":1280,"height":720}),
                ToolKind::Image
            )
            .is_none());
        }
    }

    #[test]
    fn resolution_claim_typography_quotes_original_spelling() {
        for prompt in [
            "Style: 8K IMAX commercial",
            "UHD Full HD",
            "The presenter says \"shot in 4K\"",
            "“shot in 4K”",
            "＂４Ｋ＂",
        ] {
            assert!(
                check_resolution_claim(
                    &json!({"model":"seedance_2_0","prompt":prompt,"resolution":"720p"}),
                    ToolKind::Video
                )
                .is_none(),
                "{prompt}"
            );
        }
        for (prompt, spelling) in [
            ("café 🎥 shot in ４Ｋ", "４Ｋ"),
            ("“4K” then shot in ４Ｋ", "４Ｋ"),
            ("shot in 4K; says \"4K\"", "4K"),
        ] {
            let f = check_resolution_claim(
                &json!({"model":"seedance_2_0","prompt":prompt,"resolution":"720p"}),
                ToolKind::Video,
            )
            .unwrap();
            assert!(
                f.message.starts_with(&format!("prompt says {spelling:?}")),
                "{}",
                f.message
            );
        }
    }

    #[test]
    fn normalization_is_idempotent_and_recovers_only_valid_source_boundaries() {
        for text in [
            "plain ASCII",
            "café 🎥  ４Ｋ",
            "\tｓｌｏｗ　ｍｏｔｉｏｎ",
            "a\r\nb\rc",
            "“a”„b‟",
            "x\u{00a0}\t  y",
        ] {
            let normalized = normalize_for_matching(text);
            assert_eq!(normalize_for_matching(&normalized), normalized);
            let mut offset = 0;
            for (source, c) in Folded::new(text) {
                assert_eq!(
                    original_range_for_match(text, offset..offset + c.len_utf8()),
                    Some(source)
                );
                offset += c.len_utf8();
            }
        }
        assert!(matches!(
            normalize_for_matching("already normalized"),
            std::borrow::Cow::Borrowed(_)
        ));
        assert_eq!(original_range_for_match("é", 1..2), None);
        assert_eq!(original_range_for_match("é", 0..1), None);
        assert_eq!(original_range_for_match("x", 0..2), None);
        assert_eq!(original_range_for_match("x", 0..0), None);
        let original = "é  slow\t \u{00a0}motion";
        let n = normalize_for_matching(original);
        let (s, e) = find_term_match(&n, "slow motion", false).unwrap();
        assert_eq!(
            &original[original_range_for_match(original, s..e).unwrap()],
            "slow\t \u{00a0}motion"
        );
    }

    #[test]
    fn normalized_conflict_whitespace_punctuation_and_negations() {
        for prompt in [
            "slow   motion",
            "slow\tmotion",
            "slow\u{00a0}motion",
            "ｓｌｏｗ　ｍｏｔｉｏｎ",
            "(slow motion),",
        ] {
            assert!(
                check_prompt_negative_conflict(
                    &json!({"prompt":prompt,"negative_prompt":"slow motion"})
                )
                .is_some(),
                "{prompt}"
            );
        }
        for prompt in [
            "slow\nmotion",
            "slow, motion",
            "slow / motion",
            "“slow motion”",
            "＂ｓｌｏｗ　ｍｏｔｉｏｎ＂",
            "ｎｏ ｓｌｏｗ ｍｏｔｉｏｎ",
        ] {
            assert!(
                check_prompt_negative_conflict(
                    &json!({"prompt":prompt,"negative_prompt":"slow motion"})
                )
                .is_none(),
                "{prompt}"
            );
        }
        assert!(check_prompt_negative_conflict(
            &json!({"prompt":"close - up","negative_prompt":"close-up"})
        )
        .is_none());
        assert!(check_prompt_negative_conflict(
            &json!({"prompt":"handheld","negative_prompt":"ｎｏ ｈａｎｄｈｅｌｄ"})
        )
        .is_none());
    }

    #[test]
    fn duration_claim_parser_positive_grammar_and_maximum() {
        for (text, value) in [
            ("Total: 15s / 6 shots / 16:9", 15.0),
            ("A 12-second sportswear ad", 12.0),
            ("0–3s … 12–15s", 15.0),
            ("3s to 7s", 7.0),
            ("15s total", 15.0),
            ("Total: 5.5s", 5.5),
            ("TOTAL: 15 seconds", 15.0),
            ("0—7s", 7.0),
            ("0-7s", 7.0),
            ("holds for 12 seconds", 12.0),
            ("Total: 900s", 900.0),
            ("“Total: 15s” then 12-second action", 12.0),
            ("Total: １５s", 15.0),
            ("-15 seconds; Total: 12s", 12.0),
        ] {
            assert_eq!(duration_max(text), Some(value), "{text}");
        }
        assert_eq!(
            duration_max("3 seconds, 12-second action, Total: 15s"),
            Some(15.0)
        );
    }

    #[test]
    fn duration_claim_parser_does_not_rescue_partial_invalid_tokens() {
        for text in [
            "subtotal: 15s",
            "x12-second",
            "12-secondhand",
            "15shots",
            "-15 seconds",
            "+15 seconds",
            "1e3 seconds",
            "1,000 seconds",
            "1.2.3 seconds",
            "15s2",
            "01:15 seconds",
            ".5 seconds",
            "15–3s",
            "-3s to 15s",
            "1e3–15s",
            "Total: -15s",
            "Total: 1.2.3s",
            "\"Total: 15s\"",
            "“Total: 15s”",
            "＂Total: 15s＂",
            "15 frames",
            "fifteen seconds",
            "15 secs",
            "Total 15s",
            "15s\ntotal",
            "12\nseconds",
            "shot on a 50mm at T2.0",
        ] {
            assert_eq!(duration_max(text), None, "must not rescue: {text}");
        }
        let overflow = format!("Total: {}s", "9".repeat(400));
        assert_eq!(duration_max(&overflow), None);
        assert_eq!(duration_max(&(overflow + "; Total: 15s")), Some(15.0));
        assert_eq!(duration_max("0.5 seconds"), Some(0.5));
        assert!(seconds_display(1e200).len() < 25);
    }

    #[test]
    fn duration_claim_eligibility_bounds_and_exact_message() {
        let p = json!({"model":"kling2_6","prompt":"Total: 15s","duration":5});
        assert_eq!(check_duration_claim(&p).unwrap().message,
            "prompt describes 15s of action but the supplied duration is 5s. Check the model's supported duration or retime the beats.");
        for n in [
            json!(null),
            json!("5"),
            json!(true),
            json!(0),
            json!(601),
            json!(-1),
        ] {
            let mut q = p.clone();
            q["duration"] = n;
            assert!(check_duration_claim(&q).is_none(), "{q}");
        }
        for n in [1.0, 5.0, 5.5, 600.0] {
            let q = json!({"model":"kling2_6","prompt":"Total: 900s","duration":n});
            assert!(check_duration_claim(&q).is_some());
        }
        for (prompt, seconds) in [
            ("Total: 5s", 5.0),
            ("Total: 5s", 5.5),
            ("holds for 3 seconds", 15.0),
        ] {
            assert!(check_duration_claim(
                &json!({"model":"kling2_6","prompt":prompt,"duration":seconds})
            )
            .is_none());
        }
        for model in ["seedance_2_5", "ad_multiplier"] {
            for mode in ["video_edit", "video_extension", "typo"] {
                assert!(check_duration_claim(
                    &json!({"model":model,"mode":mode,"prompt":"Total: 15s","duration":5})
                )
                .is_none());
            }
            assert!(check_duration_claim(
                &json!({"model":model,"mode":"t2v","prompt":"Total: 15s","duration":5})
            )
            .is_some());
        }
        for model in [
            "unknown",
            "kling_video_edit",
            "video_upscale",
            "video_deflicker",
            "clipify",
            "nano_banana_pro",
        ] {
            assert!(check_duration_claim(
                &json!({"model":model,"prompt":"Total: 15s","duration":5})
            )
            .is_none());
        }
        assert!(check_duration_claim(&json!({"model":"gemini_omni_flash_1_1","mode":"edit","prompt":"Total: 15s","duration":5})).is_none());
        assert!(check_duration_claim(&json!({"model":"gemini_omni_flash_1_1","mode":"text-to-video","prompt":"Total: 15s","duration":5})).is_some());
        assert!(check_duration_claim(&json!({"prompt":"Total: 15s","duration":5})).is_none());
    }

    #[test]
    fn canonical_allowlists_match_saved_mcp_not_cli_catalog() {
        let capture: Value = serde_json::from_str(include_str!(
            "../../docs/evidence/higgsfield-contract-2026-09-09.json"
        ))
        .unwrap();
        let models: Vec<&Value> = capture["mcp"]["catalog"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|c| c["pages"].as_array().unwrap())
            .flat_map(|p| p["items"].as_array().unwrap())
            .collect();
        for model in &models {
            let id = model["id"].as_str().unwrap();
            let kind = match model["output_type"].as_str().unwrap() {
                "image" => ToolKind::Image,
                "video" => ToolKind::Video,
                _ => ToolKind::Audio,
            };
            for label in resolution_options(id, kind) {
                let param = model["parameters"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|p| p["name"] == "resolution")
                    .unwrap();
                assert!(
                    param["options"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|v| v.as_str().unwrap().eq_ignore_ascii_case(label)),
                    "{id} {label}"
                );
                let mut p = json!({"model":id,"resolution":label});
                if id == "gemini_omni_flash_1_1" {
                    p["mode"] = json!("text-to-video");
                }
                assert!(submitted_size(&p, kind).is_some(), "{id} {label}");
            }
        }
        for id in DURATION_MODELS {
            let model = models.iter().find(|m| m["id"] == *id).unwrap();
            assert_eq!(model["output_type"], "video");
            assert!(
                model.get("duration_range").is_some()
                    || model.get("durations").is_some()
                    || model["parameters"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|p| p["name"] == "duration"),
                "{id}"
            );
        }
    }

    // --- Task 3: negation checks -------------------------------------------------
    //
    // `check_negation_placement`'s own tests lived here. That check was disabled
    // (SPEC.md §5.3; see the comment on `PLACEMENT_NEGATION_RE` above) and deleted along
    // with them; `is_placement_noise` (below, kept for HFR-006) still has its own direct
    // test, `is_placement_noise_flags_quantifier_idioms_and_without_gerund_only`.

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
        // "no blurry" in the prompt strips "blurry" from the positive reading, so its
        // appearance in negative_prompt is agreement, not conflict. (Was "text" before
        // the task 3 recalibration dropped that bare token as a homograph - see
        // prompt_negative_conflict_recalibration_drops_pan_text_and_sharp below for the
        // regression test that replaced this scenario's original purpose.)
        let params = json!({
            "prompt": "no blurry, cat on a windowsill",
            "negative_prompt": "random blurry"
        });
        assert!(check_prompt_negative_conflict(&params).is_none());
    }

    // Recalibration (SPEC.md §5.2; HIGGSFIELD_PROMPT_RESEARCH.md §8.2, §10): three
    // confirmed false positives against the shipped binary, each now a permanent
    // regression test as well as a golden-corpus negative control
    // (rust/tests/parity.rs's prompt_negative_conflict_fp_controls).

    #[test]
    fn prompt_negative_conflict_recalibration_drops_pan_text_and_sharp() {
        // FP1: a cast-iron pan is not a camera pan.
        assert!(check_prompt_negative_conflict(&json!({
            "prompt": "A cast-iron pan sizzles on the stove.",
            "negative_prompt": "pan, zoom, camera movement"
        }))
        .is_none());
        // FP2: quoting a literal and suppressing typography artifacts is the pairing
        // Nano Banana's own guidance recommends, not a conflict.
        assert!(check_prompt_negative_conflict(&json!({
            "prompt": "A poster with the text \"OPEN\" in bold.",
            "negative_prompt": "misspelled text, distorted letters"
        }))
        .is_none());
        // FP3: "not sharp" in negative_prompt agrees with "sharp focus" in the prompt;
        // it is not a conflict even before considering that "sharp" is dropped outright.
        assert!(check_prompt_negative_conflict(&json!({
            "prompt": "sharp focus on the blade",
            "negative_prompt": "soft focus, not sharp"
        }))
        .is_none());
    }

    // F3 (task-3 fix report): `letterbox` and `watermark` demoted from bare single-token
    // entries to bigrams after both turned up confirmed false positives, the same
    // homograph shape as FP1's cast-iron pan.
    #[test]
    fn prompt_negative_conflict_demotes_letterbox_and_watermark_to_bigrams() {
        // FP4: a red letterbox (mail slot) on a village street is not a directive about
        // black aspect-ratio bars.
        assert!(check_prompt_negative_conflict(&json!({
            "prompt": "A red letterbox on a quiet village street at dawn.",
            "negative_prompt": "letterbox, black bars, borders"
        }))
        .is_none());
        // FP5: a watermark on handmade paper is not a directive about an image overlay.
        assert!(check_prompt_negative_conflict(&json!({
            "prompt": "A close look at handmade paper with a visible watermark.",
            "negative_prompt": "watermark, logo, signature"
        }))
        .is_none());
        // The bigram replacements still carry the directive sense and fire when genuinely
        // duplicated across both fields.
        assert!(check_prompt_negative_conflict(&json!({
            "prompt": "cinematic shot with visible letterbox bars top and bottom",
            "negative_prompt": "letterbox bars, low quality"
        }))
        .is_some());
        assert!(check_prompt_negative_conflict(&json!({
            "prompt": "a product photo with a faint watermark overlay in the corner",
            "negative_prompt": "watermark overlay, low quality"
        }))
        .is_some());
    }

    #[test]
    fn prompt_negative_conflict_strips_negation_from_the_negative_field_too() {
        // Recalibration change 1. Even for a phrase that DID survive the lexicon,
        // negating it in negative_prompt must read as agreement, not conflict - the
        // strip has to apply to both fields, not the prompt alone.
        assert!(check_prompt_negative_conflict(&json!({
            "prompt": "a handheld tracking shot",
            "negative_prompt": "not handheld, tripod-mounted only"
        }))
        .is_none());
    }

    #[test]
    fn prompt_negative_conflict_excludes_a_quoted_occurrence_in_the_prompt() {
        // Recalibration change 2, exercised with a term that SURVIVES the lexicon
        // (close-up), so this cannot pass merely because the term was dropped - it
        // proves the quote-exclusion path itself does the work.
        assert!(check_prompt_negative_conflict(&json!({
            "prompt": "The subtitle card reads \"close-up\" as the scene direction.",
            "negative_prompt": "close-up, motion blur"
        }))
        .is_none());
        // Sanity check the same phrase still fires when it is NOT quoted.
        assert!(check_prompt_negative_conflict(&json!({
            "prompt": "a close-up on the actor's eyes",
            "negative_prompt": "close-up, motion blur"
        }))
        .is_some());
    }

    // Quote-pairing edges, at the full check_prompt_negative_conflict level (see
    // quoted_spans' doc comment and its direct unit tests above for the mechanism).
    #[test]
    fn prompt_negative_conflict_end_to_end_quote_pairing_edges() {
        // Two unrelated inch-marks accidentally pair up and swallow the live "close-up"
        // between them - a real directive goes silent because of stray measurement marks,
        // not because anything was actually quoted.
        assert!(check_prompt_negative_conflict(&json!({
            "prompt": "a 24\" monitor, close-up, 6\" lens",
            "negative_prompt": "close-up, motion blur"
        }))
        .is_none());
        // Doubled quotes on each side pair with their own neighbour and leave the term
        // between them live, so this still fires despite looking "extra quoted".
        assert!(check_prompt_negative_conflict(&json!({
            "prompt": "the caption reads \"\"close-up\"\" on screen",
            "negative_prompt": "close-up, motion blur"
        }))
        .is_some());
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
        // Also doubles as the bigram-replacement regression test: each dropped bare
        // token's directive sense now lives in one of these bigrams.
        let params = json!({
            "prompt": "camera pan, film grain, static shot, cinematic scene",
            "negative_prompt": "camera pan, film grain, static shot"
        });
        let finding = check_prompt_negative_conflict(&params).expect("should fire");
        assert!(finding.message.contains("\"camera pan\""));
        assert!(finding.message.contains("\"film grain\""));
        assert!(finding.message.contains("\"static shot\""));
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
