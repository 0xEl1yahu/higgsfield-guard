//! Golden-output regression corpus.
//!
//! This harness used to be a differential parity harness against `src/higgsfield_lint.py`,
//! a hand-written Python port of the original design. That job is finished: the Rust was
//! proven output-equivalent to the Python at 320 distinct inputs / 0 divergences, plus
//! 4,200 independently-fuzzed inputs from a separate reviewer. The Python oracle is now
//! archived, read-only, at `docs/archive/verified-port/` - see its README there for the
//! full history and the exact commit that was true at.
//!
//! Rust is the sole implementation; SPEC and canonical evidence govern intended behavior.
//! This file preserves the historical corpus - the real fixtures,
//! the pathological-input set, and the generated sweep across every lexicon entry,
//! dimension pair, and negation form - but compares the compiled binary's output against
//! **recorded expectations** in `rust/tests/golden/expectations.json` instead of against a
//! second live process. Original expectations were captured before the oracle retired;
//! subsequent reviewed revisions are recorded in metadata and the per-label change ledger.
//!
//! Every subsequent rule change in this project's plan deliberately flips some of these
//! expectations. That is expected and healthy - a flip is a reviewed behaviour change, not
//! a bug - but it must be a *conscious* edit to `expectations.json`, never a silent one:
//! there is no mechanism here that regenerates the file automatically, on purpose.
//!
//! Comparison rules (unchanged in spirit from the retired differential version):
//!   - the binary must exit 0 and write nothing to stderr (the hook-path contract, checked
//!     here as well as in `fail_silent.rs`, since every input in this corpus exercises it),
//!   - stdout must be empty exactly when the recorded expectation is `null`, or parse as
//!     JSON and be structurally equal to the recorded expectation - never compared as raw
//!     bytes, since `serde_json::to_string`'s separator whitespace and key ordering are not
//!     part of the contract.
//!   - the exact input bytes generated for a label must still hash to what was recorded at
//!     capture time. A hash mismatch means the corpus-generation code changed what a label
//!     produces without anyone re-capturing it - a silent drift in what is being tested,
//!     not a rule-behaviour change, and it is reported as its own failure category.

use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

// =============================================================================================
// Process plumbing
// =============================================================================================

struct Outcome {
    code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run(mut cmd: Command, input: &[u8]) -> Outcome {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn process");
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(input)
        .ok(); // a process that closes stdin early (it shouldn't) must not fail the harness
    let out = child.wait_with_output().expect("failed to wait on process");
    Outcome {
        code: out.status.code().unwrap_or(-1),
        stdout: out.stdout,
        stderr: out.stderr,
    }
}

fn run_rust(input: &[u8]) -> Outcome {
    let exe = env!("CARGO_BIN_EXE_higgsfield-lint");
    run(Command::new(exe), input)
}

/// FNV-1a, 64-bit. Chosen over `std::collections::hash_map::DefaultHasher` deliberately:
/// this hash is persisted in a checked-in file and compared across separate `cargo test`
/// invocations (possibly different machines, different Rust versions), and the standard
/// library documents `DefaultHasher`'s exact algorithm as an implementation detail that is
/// free to change between releases. FNV-1a is a fixed, fully specified algorithm with no
/// such caveat, and is trivial to implement from stdlib alone.
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

// =============================================================================================
// Shared corpus-building vocabulary (mirrors the specification's lexicons - see
// rust/src/checks.rs lines 21-271. CONFLICT_PHRASES below was updated in task 3 to mirror
// checks.rs's recalibrated list (SPEC.md §5.2; HIGGSFIELD_PROMPT_RESEARCH.md §10) -
// everywhere else in this section is unchanged from the retired differential harness.)
// =============================================================================================

const RESOLUTION_TERMS: &[&str] = &["8k", "4k", "uhd", "1080p", "full hd", "720p", "480p"];

// F3 (task-3 fix report): `letterbox` and `watermark` demoted to bigrams (`letterbox
// bars`, `watermark overlay`) - mirrors checks.rs's CONFLICT_PHRASES; see the comment
// there for why.
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

const NEGATION_FORMS: &[&str] = &["no", "without", "avoid", "not"];

const KNOWN_RESOLUTION_PARAM_VALUES: &[&str] = &["4k", "1080p", "720p", "480p"];
// Not in the lexicon at all - must be silently ignored by `resolution`, not crash on it.
const UNKNOWN_RESOLUTION_PARAM_VALUES: &[&str] = &["1k", "2k"];

// Dimension pairs from the task brief, exercising the MIN_SANE_DIMENSION floor (keyed on
// the LARGER side) and the short-side calculation. `(true, true)` and float dimensions are
// handled separately below since they need non-integer JSON types, not extra pairs here.
const DIMENSION_PAIRS: &[(i64, i64)] = &[(16, 9), (64, 64), (1920, 100), (1376, 768), (3840, 2160)];

const UNKNOWN_MODEL: &str = "not_a_real_model_xyz";
const TOOL_SINGLE: &str = "mcp__higgsfield__generate_video";
const TOOL_BATCH: &str = "mcp__higgsfield__generate_video_batch";

/// Historically, the one model `check_negation_placement` treated as
/// negative_prompt-capable, read from `data/negative_prompt_models.json` rather than
/// hardcoded so the corpus kept working if the list changed. That check and its data
/// file were both retired (SPEC.md §5.3; HIGGSFIELD_PROMPT_RESEARCH.md §6, §8.3) - no
/// reads that retired model list. Active checks now use canonical model constants.
/// The literal is kept unchanged rather than deleted: this function still feeds every
/// sweep below that varies a `model` field, and hardcoding the same value keeps those
/// sweeps generating byte-identical input to what they always have (recorded
/// `input_len`/`input_fnv1a64` in `rust/tests/golden/expectations.json` are unaffected) -
/// only the *expected outputs* for the cases that used to trigger
/// `check_negation_placement` changed when the check was disabled.
fn known_model() -> String {
    "kling2_6".to_string()
}

fn wrap(tool_name: &str, tool_input: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"tool_name": tool_name, "tool_input": tool_input}))
        .expect("generated corpus input must serialize")
}

/// A single, non-batch call: `{"params": <params>}`.
fn single(params: Value) -> Value {
    json!({"params": params})
}

/// The same params, but as the JSON-ENCODED-STRING shape the schema also allows (`anyOf:
/// [object, string]` per SPEC §9 / oracle `_resolve_params`, lines 206-219).
fn single_str(params: &Value) -> Value {
    json!({"params": serde_json::to_string(params).unwrap()})
}

/// A batch call with one request entry carrying an explicit `index`.
fn batch_one(index: Value, params: Value) -> Value {
    json!({"requests": [{"index": index, "params": params}]})
}

fn with_optional(
    mut obj: serde_json::Map<String, Value>,
    negative: Option<&str>,
    model: Option<&str>,
) -> Value {
    if let Some(n) = negative {
        obj.insert("negative_prompt".to_string(), json!(n));
    }
    if let Some(m) = model {
        obj.insert("model".to_string(), json!(m));
    }
    Value::Object(obj)
}

// =============================================================================================
// Corpus source 1: the real fixtures
// =============================================================================================

fn fixture_inputs() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&fixtures_dir)
        .expect("tests/fixtures must exist")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();

    // NOTE ON COUNT: task-5-brief.md and this task's instructions both say "the four real
    // fixtures", but tests/fixtures/ currently holds only two files: kling2_6_4k_slowmo.json
    // and seedance_tree_clean.json. Confirmed by directory listing and by
    // .superpowers/sdd/2026-08-31-rust-port/progress.md, where Task 4's controller
    // spot-checked parity on "both real fixtures" by name. Rather than fabricate two
    // fictional fixtures to hit a literal count of four, this harness uses every *.json
    // file actually present and wraps each one BOTH as a single top-level call and as a
    // one-entry batch (2 fixtures x 2 shapes = 4 inputs) - which honestly satisfies both
    // "four" and the separate "single-call and batch shapes" requirement without inventing
    // fixture data. If more fixtures are added to tests/fixtures/ later, this scales
    // automatically.
    for path in paths {
        let text = std::fs::read_to_string(&path).unwrap();
        let fixture: Value = serde_json::from_str(&text).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().to_string();

        out.push((
            format!("fixture:{name}:single"),
            wrap(TOOL_SINGLE, fixture.clone()),
        ));

        let params = fixture
            .get("params")
            .cloned()
            .unwrap_or_else(|| panic!("fixture {name} has no top-level params"));
        out.push((
            format!("fixture:{name}:batch"),
            wrap(TOOL_BATCH, batch_one(json!(0), params)),
        ));
    }
    out
}

// =============================================================================================
// Corpus source 2: every pathological payload from rust/tests/fail_silent.rs
//
// Verbatim copies of both test functions' cases in that file. fail_silent.rs is a
// standalone `[[test]]` binary (not part of the library), so there is nothing importable
// to share - these lists are duplicated by hand. If fail_silent.rs ever gains or loses a
// case, update this list to match.
// =============================================================================================

fn pathological_inputs() -> Vec<(String, Vec<u8>)> {
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty", b"".to_vec()),
        ("garbage", b"not json".to_vec()),
        ("empty_object", b"{}".to_vec()),
        ("tool_input_null", br#"{"tool_input": null}"#.to_vec()),
        (
            "tool_input_string",
            br#"{"tool_input": "a string"}"#.to_vec(),
        ),
        (
            "params_bare_string",
            br#"{"tool_input": {"params": "a bare string"}}"#.to_vec(),
        ),
        (
            "params_non_dict_list",
            br#"{"tool_input": {"params": []}}"#.to_vec(),
        ),
        (
            "requests_not_a_list",
            br#"{"tool_input": {"requests": "not a list"}}"#.to_vec(),
        ),
        (
            "requests_junk_entries",
            br#"{"tool_input": {"requests": [null, 3, "x"]}}"#.to_vec(),
        ),
        ("invalid_utf8", vec![0xff, 0xfe, 0x00, 0x01]),
        (
            "huge_prompt",
            format!(
                r#"{{"tool_input":{{"params":{{"prompt":"{}"}}}}}}"#,
                "a".repeat(5_000_000)
            )
            .into_bytes(),
        ),
        (
            "deeply_nested_array",
            format!("{}{}", "[".repeat(50_000), "]".repeat(50_000)).into_bytes(),
        ),
        // From `an_internal_panic_is_still_silent_and_zero`: a harmless-looking key that
        // exists purely to guard against a future panic hook keyed on it. Today it does
        // nothing special in either implementation (no field named `__panic__` is read by
        // either the oracle or the port), so both sides simply find no prompt/resolution
        // and stay silent - which is itself worth confirming stays true.
        (
            "magic_panic_key",
            br#"{"tool_input": {"params": {"__panic__": true}}}"#.to_vec(),
        ),
    ];
    cases
        .into_iter()
        .map(|(name, bytes)| (format!("pathological:{name}"), bytes))
        .collect()
}

// =============================================================================================
// Corpus source 3: the generated sweep
// =============================================================================================

/// Resolution lexicon terms (`RESOLUTION_CLAIMS`) in both cases, crossed with the dimension
/// pairs and both `params` shapes; plus the `resolution` param's own values, including
/// `1k`/`2k` which the lexicon does not know.
fn resolution_sweep() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for term in RESOLUTION_TERMS {
        for variant in [term.to_string(), term.to_uppercase()] {
            let prompt = format!("epic cinematic scene, {variant}, highly detailed");
            for (w, h) in DIMENSION_PAIRS {
                let params = json!({"prompt": prompt, "width": w, "height": h});
                out.push((
                    format!("resolution:{variant}:{w}x{h}:obj"),
                    wrap(TOOL_SINGLE, single(params.clone())),
                ));
                out.push((
                    format!("resolution:{variant}:{w}x{h}:str"),
                    wrap(TOOL_SINGLE, single_str(&params)),
                ));
            }
            // Booleans must be excluded from dimension logic even though
            // `isinstance(True, int)` is `True` in Python - the oracle explicitly guards
            // against this (`_is_int_dimension`).
            let bool_params = json!({"prompt": prompt, "width": true, "height": true});
            out.push((
                format!("resolution:{variant}:bool_dims"),
                wrap(TOOL_SINGLE, single(bool_params)),
            ));
        }
    }
    for res_val in KNOWN_RESOLUTION_PARAM_VALUES
        .iter()
        .chain(UNKNOWN_RESOLUTION_PARAM_VALUES.iter())
    {
        let params =
            json!({"prompt": "shot in 4k, cinematic establishing shot", "resolution": res_val});
        out.push((
            format!("resolution_param:{res_val}"),
            wrap(TOOL_SINGLE, single(params)),
        ));
    }
    out
}

/// Every `CONFLICT_PHRASES` entry: a true conflict (same term, unnegated, in both fields),
/// its JSON-string-params twin, and a negated-in-prompt "agreement" variant that must stay
/// silent (the term is stripped from the positive reading by `strip_negations`).
fn conflict_phrase_sweep() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for phrase in CONFLICT_PHRASES {
        let params = json!({
            "prompt": format!("a scene featuring {phrase}, cinematic composition"),
            "negative_prompt": format!("{phrase}, low quality")
        });
        out.push((
            format!("conflict:{phrase}:obj"),
            wrap(TOOL_SINGLE, single(params.clone())),
        ));
        out.push((
            format!("conflict:{phrase}:str"),
            wrap(TOOL_SINGLE, single_str(&params)),
        ));

        let negated_params = json!({
            "prompt": format!("no {phrase}, cat on a windowsill"),
            "negative_prompt": format!("{phrase}, low quality")
        });
        out.push((
            format!("conflict_agreement:{phrase}"),
            wrap(TOOL_SINGLE, single(negated_params)),
        ));
    }
    out
}

/// Each negation form (`no X`, `without X`, `avoid X`, `not X`), crossed with
/// with/without `negative_prompt` and with/without a known/unknown/absent model.
fn negation_form_sweep(known: &str) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for form in NEGATION_FORMS {
        let prompt = format!("{form} clutter");
        for negative in [Some("blurry"), None] {
            for model in [Some(known), Some(UNKNOWN_MODEL), None] {
                let mut m = serde_json::Map::new();
                m.insert("prompt".to_string(), json!(prompt));
                let params = with_optional(m, negative, model);
                out.push((
                    format!("negation_form:{form}:neg={negative:?}:model={model:?}"),
                    wrap(TOOL_SINGLE, single(params)),
                ));
            }
        }
        let base = json!({"prompt": prompt, "negative_prompt": "blurry"});
        out.push((
            format!("negation_form:{form}:str"),
            wrap(TOOL_SINGLE, single_str(&base)),
        ));
    }
    out
}

/// The stopword idioms (`no more`, `no longer`, `no matter`) that must stay silent despite
/// starting with "no", crossed with model support; plus each idiom mixed alongside a real
/// directive negation that must still fire.
fn stopword_sweep(known: &str) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let idioms = [
        (
            "no_more",
            "a busy street with no more than three people visible",
        ),
        ("no_longer", "a building that is no longer standing"),
        ("no_matter", "a hero who fights no matter what happens"),
    ];
    for (name, prompt) in idioms {
        for model in [Some(known), Some(UNKNOWN_MODEL), None] {
            let mut m = serde_json::Map::new();
            m.insert("prompt".to_string(), json!(prompt));
            let params = with_optional(m, Some("blurry"), model);
            out.push((
                format!("stopword:{name}:model={model:?}"),
                wrap(TOOL_SINGLE, single(params)),
            ));
        }
        let mixed_prompt = format!("{prompt}, no clutter");
        let params = json!({"prompt": mixed_prompt, "negative_prompt": "blurry"});
        out.push((
            format!("stopword:{name}:mixed_with_directive"),
            wrap(TOOL_SINGLE, single(params)),
        ));
    }
    out
}

/// The `without + gerund` manner-adverbial shape (must stay silent) and the `no`/`avoid` +
/// gerund directive shapes (must still fire), crossed with model support.
fn gerund_sweep(known: &str) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let noise_phrases = ["without moving", "without breathing", "without blinking"];
    let directive_phrases = [
        "no flickering",
        "no warping",
        "avoid flickering",
        "avoid shimmering",
    ];

    for phrase in noise_phrases {
        for model in [Some(known), None] {
            let mut m = serde_json::Map::new();
            m.insert(
                "prompt".to_string(),
                json!(format!("a monk sitting {phrase}, breathing slowly")),
            );
            let params = with_optional(m, Some("blurry"), model);
            out.push((
                format!("gerund_noise:{phrase}:model={model:?}"),
                wrap(TOOL_SINGLE, single(params)),
            ));
        }
    }
    for phrase in directive_phrases {
        for model in [Some(known), None] {
            let mut m = serde_json::Map::new();
            m.insert(
                "prompt".to_string(),
                json!(format!("a scene, {phrase}, cinematic")),
            );
            let params = with_optional(m, Some("blurry"), model);
            out.push((
                format!("gerund_directive:{phrase}:model={model:?}"),
                wrap(TOOL_SINGLE, single(params)),
            ));
        }
    }
    out
}

/// The dimension pairs on their own (no resolution claim in the prompt), both `params`
/// shapes, plus boolean dimensions and float dimensions (which must NOT count - Python's
/// `isinstance(100.0, int)` is `False`, and `serde_json`'s float `Value`s are a distinct
/// variant from its integer ones).
fn dimension_pair_sweep() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for (w, h) in DIMENSION_PAIRS {
        // F2: this prompt MUST carry a resolution claim. With no claim,
        // `check_resolution_claim` returns at `claimed_short_side(prompt)?` and never
        // calls `actual_short_side` at all - every pair here would compare silence to
        // silence regardless of the MIN_SANE_DIMENSION floor logic this sweep exists to
        // exercise. "shot in 8k" makes every pair's outcome depend on the actual
        // width/height math: pairs below the 128px floor ((16,9), (64,64)) stay silent,
        // and pairs that clear the floor ((1920,100), (1376,768), (3840,2160)) all fire,
        // since none reaches the 8k claim's 4320px minimum short side.
        let params = json!({"prompt": "a calm nature scene shot in 8k", "width": w, "height": h});
        out.push((
            format!("dims:{w}x{h}:obj"),
            wrap(TOOL_SINGLE, single(params.clone())),
        ));
        out.push((
            format!("dims:{w}x{h}:str"),
            wrap(TOOL_SINGLE, single_str(&params)),
        ));
    }
    let bool_params =
        json!({"prompt": "shot in 8k, dramatic lighting", "width": true, "height": true});
    out.push((
        "dims:true_true".to_string(),
        wrap(TOOL_SINGLE, single(bool_params)),
    ));
    let float_params =
        json!({"prompt": "shot in 4k, dramatic lighting", "width": 100.0, "height": 100.0});
    out.push((
        "dims:floats_excluded".to_string(),
        wrap(TOOL_SINGLE, single(float_params)),
    ));

    // F1: an ASYMMETRIC boolean/integer pair. Every other boolean case in this corpus
    // pairs `true` with `true` (or `false` with `false`), which is non-discriminating: a
    // buggy `is_int_dimension` that coerces `Bool(true)` to `1` still computes
    // `max(1,1)=1 < MIN_SANE_DIMENSION`, so it stays silent exactly like the correct
    // code, and no test would ever notice. Pairing `width: true` with a real,
    // claim-defeating `height` closes that hole: a buggy implementation would compute
    // max(1, 3840) = 3840 (clears the floor), min(1, 3840) = 1 (a 1px short side), and
    // WOULD fire against the "8k" claim's 4320px minimum, while the correct
    // implementation excludes the boolean `width` entirely and stays silent.
    let asymmetric_bool_params = json!({"prompt": "shot in 8k", "width": true, "height": 3840});
    out.push((
        "dims:asymmetric_bool".to_string(),
        wrap(TOOL_SINGLE, single(asymmetric_bool_params)),
    ));
    out
}

/// Batch shapes: an explicit-`index` entry for a handful of SCALAR index types, positional
/// fallback, a multi-entry batch mixing valid/invalid/string-params entries, and the two
/// documented `requests`-vs-top-level-`params` interactions.
fn batch_sweep(known: &str) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();

    // --- Deliberate exclusion: SCALAR `index` values only ---------------------------------
    // `index` is schema-typed as an integer, so reaching a non-scalar index (a JSON array or
    // object) needs a malformed call the schema does not allow. When it does happen, the two
    // implementations render it differently: Python's f-string calls `str()` on the decoded
    // value, and `str()` of a Python list is its `repr()` - `[1, 2]`, space after the comma -
    // and of a dict is `{'a': 1}`, single-quoted keys. Rust's `format_index` (rust/src/lint.rs)
    // falls through to `serde_json::Value`'s `Display` for anything that isn't a
    // string/number/bool/null, which renders the same values as `[1,2]` (no space) and
    // `{"a":1}` (double-quoted keys, JSON syntax, not Python repr). This is a KNOWN, ACCEPTED
    // divergence, not a bug: `index` needs a malformed call to reach a compound value at all,
    // and a partial emulation of Python's `repr()` in Rust would look like parity without
    // being it - it would cover exactly the shapes someone thought to test (a flat list, a
    // flat dict) and silently diverge on everything else a real `repr()` can produce (nested
    // containers, floats inside them, non-ASCII string values, etc.). So: only scalar
    // (integer/string/bool/null/float) index values are generated below. Do not "fix" this
    // corpus later by adding compound indices - it will fail, and this comment is why that
    // failure is expected, not a regression.
    let scalar_indices = [
        json!(0),
        json!(7),
        json!(-3),
        json!("req-alpha"),
        json!(true),
        json!(false),
        Value::Null,
        json!(2.5),
    ];
    for idx in &scalar_indices {
        // width/height chosen so this ACTUALLY fires (max side 1280 clears the 128px
        // floor, short side 100 misses the 4k claim's 2160px minimum) - a silent
        // params object would make every one of these comparisons a trivial
        // silence-equals-silence match, never exercising `format_index`'s rendering of
        // the index into the "request {index}: ..." message text at all.
        let params = json!({"prompt": "shot in 4k, cinematic", "width": 1280, "height": 100});
        out.push((
            format!("batch:index={idx}"),
            wrap(TOOL_BATCH, batch_one(idx.clone(), params)),
        ));
    }

    let positional_fallback = json!({"requests": [
        {"params": {"prompt": "a cat on a windowsill", "width": 100, "height": 100}},
        {"params": {"prompt": "shot in 4k, cinematic", "width": 1280, "height": 720}},
    ]});
    out.push((
        "batch:positional_fallback".to_string(),
        wrap(TOOL_BATCH, positional_fallback),
    ));

    let mixed = json!({"requests": [
        {"index": "req-a", "params": {"prompt": "shot in 4k, cinematic", "width": 1280, "height": 720}},
        {"not_a_params_field": true},
        "not a dict",
        {"params": "{\"prompt\": \"no logos, no clutter\", \"negative_prompt\": \"blurry\"}"},
        {"index": known, "params": {"prompt": "a plain scene", "width": 100, "height": 100}},
    ]});
    out.push(("batch:mixed_entries".to_string(), wrap(TOOL_BATCH, mixed)));

    let empty_requests_falls_back = json!({
        "requests": [],
        "params": {"prompt": "shot in 4k, cinematic", "width": 1280, "height": 720}
    });
    out.push((
        "batch:empty_requests_falls_back_to_top_level".to_string(),
        wrap(TOOL_BATCH, empty_requests_falls_back),
    ));

    let never_falls_back = json!({
        "requests": [{"no_params_field": true}],
        "params": {"prompt": "shot in 4k, cinematic", "width": 100, "height": 100}
    });
    out.push((
        "batch:non_empty_requests_never_falls_back".to_string(),
        wrap(TOOL_BATCH, never_falls_back),
    ));

    out
}

/// A richer prompt that can trip all three checks at once (resolution claim + prompt/
/// negative conflict + negation placement), crossed with with/without `negative_prompt`,
/// with/without a known/unknown/absent model, and both `params` shapes - exercises finding
/// order and the render() pluralisation logic together.
fn combined_sweep(known: &str) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let prompt = "cinematic 4k shot, no text, no logos, slow motion, sharp focus";
    for negative in [Some("slow motion, sharp, blurry"), None] {
        for model in [Some(known), Some(UNKNOWN_MODEL), None] {
            let mut m = serde_json::Map::new();
            m.insert("prompt".to_string(), json!(prompt));
            m.insert("width".to_string(), json!(1280));
            m.insert("height".to_string(), json!(100));
            let params = with_optional(m, negative, model);
            out.push((
                format!("combined:neg={negative:?}:model={model:?}:obj"),
                wrap(TOOL_SINGLE, single(params.clone())),
            ));
            out.push((
                format!("combined:neg={negative:?}:model={model:?}:str"),
                wrap(TOOL_SINGLE, single_str(&params)),
            ));
        }
    }
    out
}

/// Permanent negative controls for the five false-positive classes that
/// `negation_placement` was confirmed to fire on (HIGGSFIELD_PROMPT_RESEARCH.md §8.3).
/// Each is a *real* firing against the shipped binary, not hypothetical: FP1 quotes
/// Higgsfield's own published style header, and FP4 is the span-spill case where the
/// <=3-word window crossed a clause boundary and quoted "no logos a beautiful", a phrase
/// nobody wrote. They exist to guard against the original rule returning unchanged.
fn negation_placement_fp_controls() -> Vec<(String, Vec<u8>)> {
    vec![
        // FP1 - Higgsfield house style. Every quoted span is copied from their own ad-guide
        // style header, and a negative_prompt is present so the old gate would open.
        (
            "fp_control:negation_placement:higgsfield_style_header",
            json!({
                "model": "seedance_2_5",
                "prompt": "Style: 8K IMAX commercial. No jitter, no music, no subtitles, no readable text.",
                "negative_prompt": "watermark"
            }),
        ),
        // FP2 - scene description. These describe what is in the frame, not an artifact to
        // suppress; moving them to negative_prompt would remove the subject.
        (
            "fp_control:negation_placement:scene_description",
            json!({
                "model": "kling2_6",
                "prompt": "A road with no cars. A man with no hat walks along it."
            }),
        ),
        // FP3 - quoted literal. The text is the subject of the shot.
        (
            "fp_control:negation_placement:quoted_literal",
            json!({
                "model": "kling2_6",
                "prompt": "A sign reading \"NO PARKING\" hangs above the door."
            }),
        ),
        // FP4 - span spill. Unpunctuated prose let the window cross a clause boundary.
        (
            "fp_control:negation_placement:span_spill",
            json!({
                "model": "kling2_6",
                "prompt": "a wide establishing shot with no logos a beautiful sunlit plaza and no signage anywhere in frame"
            }),
        ),
        // FP5 - the seed-list branch alone. No negative_prompt in the call at all; the rule
        // fired purely because the model name was on a one-entry list seeded from the field
        // having once been *sent*, never from evidence it is honoured.
        (
            "fp_control:negation_placement:seed_list_only_no_negative_prompt",
            json!({
                "model": "kling2_6",
                "prompt": "a calm harbour at dawn, no text and no watermark"
            }),
        ),
    ]
    .into_iter()
    .map(|(name, params)| (name.to_string(), wrap(TOOL_SINGLE, single(params))))
    .collect()
}

/// Permanent negative controls for `prompt_negative_conflict`'s recalibration
/// (HIGGSFIELD_PROMPT_RESEARCH.md §8.2, §10; SPEC.md §5.2). The first three are *real*
/// firings against the shipped pre-recalibration binary, not hypothetical - the audit ran
/// them and recorded the actual output. They must all be silent once the four changes from
/// research §10 land:
///   - a common-noun homograph (`pan` the cookware vs `pan` the camera move),
///   - a quoted literal the prompt is supposed to contain per Nano Banana's own guidance
///     (`text` inside `"OPEN"` vs `misspelled text` as a thing to avoid),
///   - agreement rather than conflict (`sharp` in the prompt vs `not sharp` in
///     `negative_prompt` - both fields want the same thing once negations are stripped
///     from both, not just the prompt).
///
/// **`sharp_focus_soft_focus` and `not_sharp_leading_in_negative` are confounded and prove
/// nothing about change 1.** They were written to cover the third bullet above (strip
/// negations from BOTH fields, not just the prompt), but `sharp` was ALSO dropped from
/// `CONFLICT_PHRASES` outright, in the very same recalibration commit (change 4: a
/// confirmed-obvious homograph). A term that is not in the lexicon at all can never fire,
/// independent of whether negations are stripped from one field or two - so both controls
/// stay silent for a reason that has nothing to do with change 1. Confirmed by reverting
/// only `strip_negations(negative)` back to `negative` in a scratch copy of this repo and
/// re-running the golden suite: it stayed 344/344/0, naming neither control (see the
/// task-3 fix report, finding F2, for the full before/after). They are kept below because
/// they still legitimately cover the homograph drop (change 4) for `sharp` specifically -
/// just relabelled to say so - but `handheld_survivor_negation_strip` is the control that
/// actually exercises change 1: it uses `handheld`, a term that SURVIVES the recalibrated
/// lexicon, so reverting change 1 alone has something left to flip. The same reviewer
/// re-run with change 1 reverted DOES flip `handheld_survivor_negation_strip` (and does not
/// flip anything else), proving this new control discriminates where the old two did not.
///
/// The remaining entries: one more exercises the negation-in-negative-field strip with a
/// minimal `negative_prompt` value; a quoted-survivor entry covers change 2 (the quote
/// exclusion) with a term that also survives the lexicon, for the same reason;
/// `letterbox_mail_slot` and `paper_watermark` are F3's real confirmed firings that
/// motivated demoting `letterbox` and `watermark` to bigrams (see the comment on
/// `CONFLICT_PHRASES` above); and the final two are the `""` / whitespace-only
/// `negative_prompt` edge cases research §8.2's "Benchmark additions" row calls for.
/// `whitespace_only_negative_prompt` deliberately uses different prompt text from
/// `extra_edge_sweep`'s `edge:negative_prompt_whitespace_only` case - identical stdin
/// bytes under two labels would be silently dropped by the corpus's
/// de-duplication guard (see the collision check in
/// `rust_matches_recorded_golden_expectations` below).
fn prompt_negative_conflict_fp_controls() -> Vec<(String, Vec<u8>)> {
    vec![
        // FP1 - common-noun homograph. Research §8.2: confirmed firing on the shipped
        // binary before recalibration.
        (
            "fp_control:prompt_negative_conflict:cast_iron_pan",
            json!({
                "prompt": "A cast-iron pan sizzles on the stove.",
                "negative_prompt": "pan, zoom, camera movement"
            }),
        ),
        // FP2 - quoted literal text, exactly the pairing Nano Banana's own help article
        // recommends: quote the literal, suppress typography artifacts in the negative.
        (
            "fp_control:prompt_negative_conflict:quoted_literal_text",
            json!({
                "prompt": "A poster with the text \"OPEN\" in bold.",
                "negative_prompt": "misspelled text, distorted letters"
            }),
        ),
        // FP3 - agreement, not conflict, in intent: "not sharp" in negative_prompt already
        // says the same thing "soft focus" does. BUT: `sharp` was dropped from
        // CONFLICT_PHRASES entirely (change 4, homograph), so this control is silent for
        // that reason alone and proves nothing about change 1 (strip negations from both
        // fields) - see the doc comment above this function. Kept as a homograph-drop
        // control, not a change-1 control.
        (
            "fp_control:prompt_negative_conflict:sharp_focus_soft_focus",
            json!({
                "prompt": "sharp focus on the blade",
                "negative_prompt": "soft focus, not sharp"
            }),
        ),
        // "not sharp" in the negative field specifically, isolated from FP3's longer
        // string and phrased so the negation sits at the very start of the field. Same
        // caveat as FP3: `sharp` is not in the lexicon at all, so this is a homograph-drop
        // control, not a change-1 control - see the doc comment above this function.
        (
            "fp_control:prompt_negative_conflict:not_sharp_leading_in_negative",
            json!({
                "prompt": "a knife with a sharp edge on the counter",
                "negative_prompt": "not sharp, oversaturated colors"
            }),
        ),
        // F2 (task-3 fix report): the control that actually exercises change 1 (strip
        // negations from BOTH fields, not just the prompt). Unlike the two `sharp` controls
        // above, `handheld` SURVIVES the recalibrated lexicon - so reverting change 1 alone
        // (leaving `negative` unstripped) has something to flip here. Verified: on this
        // change 1 fires and this control is silent; reverting only `strip_negations(negative)`
        // -> `negative` in a scratch copy makes the golden suite FAIL, naming exactly this
        // label (see the task-3 fix report for the full transcript).
        (
            "fp_control:prompt_negative_conflict:handheld_survivor_negation_strip",
            json!({
                "prompt": "a handheld tracking shot",
                "negative_prompt": "not handheld, tripod-mounted only"
            }),
        ),
        // A quoted occurrence of a term that SURVIVES the recalibrated lexicon
        // (`close-up` is kept, per SPEC.md §5.2) - proves the quote-exclusion helper does
        // real work here, rather than the case only passing because the term itself was
        // dropped as a homograph (as FP2's `text` is).
        (
            "fp_control:prompt_negative_conflict:quoted_survivor_term",
            json!({
                "prompt": "The subtitle card reads \"close-up\" as the scene direction.",
                "negative_prompt": "close-up, motion blur"
            }),
        ),
        // F3 (task-3 fix report) - the two demonstrated false positives that motivated
        // demoting `letterbox` and `watermark` from bare tokens to bigrams
        // (`letterbox bars`, `watermark overlay`): both confirmed firing on the shipped
        // binary before this change, same homograph shape as FP1's cast-iron pan.
        (
            "fp_control:prompt_negative_conflict:letterbox_mail_slot",
            json!({
                "prompt": "A red letterbox on a quiet village street at dawn.",
                "negative_prompt": "letterbox, black bars, borders"
            }),
        ),
        (
            "fp_control:prompt_negative_conflict:paper_watermark",
            json!({
                "prompt": "A close look at handmade paper with a visible watermark.",
                "negative_prompt": "watermark, logo, signature"
            }),
        ),
        // negative_prompt of "" - no term can match an empty string.
        (
            "fp_control:prompt_negative_conflict:empty_negative_prompt",
            json!({
                "prompt": "a scene with sharp focus, slow motion, camera pan across the skyline",
                "negative_prompt": ""
            }),
        ),
        // negative_prompt of whitespace only - distinct prompt text from
        // extra_edge_sweep's edge:negative_prompt_whitespace_only to avoid a byte
        // collision (see the doc comment above).
        (
            "fp_control:prompt_negative_conflict:whitespace_only_negative_prompt",
            json!({
                "prompt": "slow motion establishing shot, panning across the skyline",
                "negative_prompt": "   "
            }),
        ),
    ]
    .into_iter()
    .map(|(name, params)| (name.to_string(), wrap(TOOL_SINGLE, single(params))))
    .collect()
}

/// Type-confusion and other edges not otherwise covered: wrong-typed fields, whitespace-only
/// negative_prompt, non-string resolution, negative/string-encoded dimensions, missing
/// prompt, and a non-ASCII prompt (exercises the char-based, not byte-based, boundary scan).
fn extra_edge_sweep() -> Vec<(String, Vec<u8>)> {
    vec![
        ("edge:negative_prompt_non_string", json!({"prompt": "no text", "negative_prompt": 123})),
        ("edge:model_non_string", json!({"prompt": "no text", "model": 42})),
        ("edge:negative_prompt_whitespace_only", json!({"prompt": "no text", "negative_prompt": "   "})),
        ("edge:resolution_non_string", json!({"prompt": "shot in 4k", "resolution": 4})),
        ("edge:resolution_null", json!({"prompt": "shot in 4k", "resolution": null})),
        ("edge:width_negative", json!({"prompt": "shot in 4k", "width": -5000, "height": -5000})),
        ("edge:width_as_string_number", json!({"prompt": "shot in 4k", "width": "1920", "height": "1080"})),
        ("edge:prompt_non_string", json!({"prompt": 12345, "width": 100, "height": 100})),
        ("edge:no_prompt_at_all", json!({"width": 100, "height": 100})),
        (
            // F2: 100x100 is below MIN_SANE_DIMENSION (128), so `actual_short_side`
            // returns None and `check_resolution_claim` stays silent regardless of
            // whether the Unicode boundary scan correctly found "4K" in the prompt -
            // the very thing this case exists to exercise becomes unobservable. 1280
            // clears the floor so the "4K" claim (2160px minimum) actually reaches the
            // final comparison and fires.
            "edge:unicode_prompt",
            json!({"prompt": "caf\u{e9} shot in \u{201c}4K\u{201d}, no logos, na\u{ef}ve style", "width": 1280, "height": 1280}),
        ),
    ]
    .into_iter()
    .map(|(name, params)| (name.to_string(), wrap(TOOL_SINGLE, single(params))))
    .collect()
}

// =============================================================================================
// Golden expectations
// =============================================================================================

/// One recorded expectation: the label it was captured under, the length and FNV-1a hash of
/// the exact input bytes the corpus generator produced for that label at capture time, and
/// the expected output (`None` means the hook must be silent - empty stdout).
struct GoldenEntry {
    input_len: usize,
    input_hash: String,
    output: Option<Value>,
}

/// Loads `rust/tests/golden/expectations.json` into a label -> entry map. Panics (not a
/// soft failure) on a malformed golden file or a duplicate label - both mean the file was
/// hand-edited into an inconsistent state, which is a setup bug, not a rule-behaviour
/// question the divergence report should have to explain.
fn load_golden() -> HashMap<String, GoldenEntry> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rust/tests/golden/expectations.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
    let doc: Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()));
    let entries = doc
        .get("entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{} has no top-level \"entries\" array", path.display()));

    let mut map = HashMap::with_capacity(entries.len());
    for entry in entries {
        let label = entry
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("a golden entry is missing its \"label\" string: {entry}"))
            .to_string();
        let input_len = entry
            .get("input_len")
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("golden entry {label:?} is missing \"input_len\""))
            as usize;
        let input_hash = entry
            .get("input_fnv1a64")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("golden entry {label:?} is missing \"input_fnv1a64\""))
            .to_string();
        let output = entry.get("output").cloned().filter(|v| !v.is_null());
        let prior = map.insert(
            label.clone(),
            GoldenEntry {
                input_len,
                input_hash,
                output,
            },
        );
        assert!(
            prior.is_none(),
            "duplicate golden label {label:?} in {}",
            path.display()
        );
    }
    map
}

// =============================================================================================
// Comparison
// =============================================================================================

/// Compares one corpus input's actual behaviour against its recorded golden expectation.
/// Returns `None` when everything matches, or a `Some(String)` naming the input and showing
/// expected vs. actual - callers accumulate these across the whole corpus rather than
/// failing on the first one, so a single test run reports every offending input at once.
fn compare(
    label: &str,
    input: &[u8],
    rust_out: &Outcome,
    golden: Option<&GoldenEntry>,
) -> Option<String> {
    let mut problems = Vec::new();

    // The hook-path contract applies to every input in this corpus, golden entry or not:
    // never a non-zero exit, never stderr. Checked here in addition to `fail_silent.rs`
    // because this corpus's sweep covers far more shapes than that file's fixed list.
    if rust_out.code != 0 {
        problems.push(format!("rust exited {} (expected 0)", rust_out.code));
    }
    if !rust_out.stderr.is_empty() {
        problems.push(format!(
            "rust stderr was non-empty: {:?}",
            String::from_utf8_lossy(&rust_out.stderr)
        ));
    }

    // Computed regardless of whether a golden entry exists: a "missing golden entry"
    // report is far more useful with the actual output attached, so whoever adds this
    // label to expectations.json can see exactly what to paste in rather than having to
    // re-run the binary by hand to find out.
    let rust_empty = rust_out.stdout.is_empty();
    let actual_value: Option<Value> = if rust_empty {
        None
    } else {
        match serde_json::from_slice::<Value>(&rust_out.stdout) {
            Ok(v) => Some(v),
            Err(e) => {
                problems.push(format!(
                    "rust stdout did not parse as JSON: {e}; raw stdout={:?}",
                    String::from_utf8_lossy(&rust_out.stdout)
                ));
                None
            }
        }
    };

    let Some(golden) = golden else {
        problems.push(format!(
            "no golden expectation recorded for this label - this is a NEW input, not a \
             flip; add it deliberately to rust/tests/golden/expectations.json (see that \
             file's README/note on how it was captured)\n    actual output: {}",
            render_expectation(actual_value.as_ref()),
        ));
        return Some(format!(
            "MISSING GOLDEN ENTRY [{label}]\n  stdin: {}\n  {}",
            truncated_lossy(input, 2000),
            problems.join("\n  ")
        ));
    };

    // Corpus-drift check: did the generator actually build the same bytes for this label
    // that were recorded at capture time? This is orthogonal to whether the *behaviour*
    // changed - it catches someone editing a sweep function (e.g. changing a prompt
    // string) without recapturing, which would otherwise silently compare the new bytes'
    // output against the OLD bytes' recorded expectation.
    let actual_hash = format!("{:016x}", fnv1a64(input));
    if actual_hash != golden.input_hash || input.len() != golden.input_len {
        problems.push(format!(
            "input bytes for this label no longer match what was captured (len {} -> {}, \
             hash {} -> {}) - the corpus generator changed what this label produces; \
             recapture deliberately if that was intended",
            golden.input_len,
            input.len(),
            golden.input_hash,
            actual_hash
        ));
    }

    let expected_empty = golden.output.is_none();
    if rust_empty != expected_empty {
        problems.push(format!(
            "stdout emptiness differs from the golden expectation: expected_empty={expected_empty}, \
             actual_empty={rust_empty}\n    expected: {}\n    actual:   {}",
            render_expectation(golden.output.as_ref()),
            render_expectation(actual_value.as_ref()),
        ));
    } else if let (Some(expected_v), Some(actual_v)) = (&golden.output, &actual_value) {
        if expected_v != actual_v {
            problems.push(format!(
                "parsed JSON differs from the golden expectation:\n    expected: {expected_v}\n    actual:   {actual_v}"
            ));
        }
    }

    if problems.is_empty() {
        None
    } else {
        Some(format!(
            "GOLDEN MISMATCH [{label}]\n  stdin: {}\n  {}",
            truncated_lossy(input, 2000),
            problems.join("\n  ")
        ))
    }
}

fn render_expectation(v: Option<&Value>) -> String {
    match v {
        None => "<no output (silent)>".to_string(),
        Some(v) => v.to_string(),
    }
}

/// Echoes stdin in a failure message, capped so a divergence on one of the pathological
/// multi-megabyte inputs (`huge_prompt`, `deeply_nested_array`) can't turn one test failure
/// into megabytes of terminal output.
fn truncated_lossy(input: &[u8], max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        String::from_utf8_lossy(input).to_string()
    } else {
        format!(
            "{}... [truncated, {} bytes total]",
            String::from_utf8_lossy(&input[..max_bytes]),
            input.len()
        )
    }
}

// =============================================================================================
// The test
// =============================================================================================

fn all_inputs() -> Vec<(String, Vec<u8>)> {
    let known = known_model();

    let mut all: Vec<(String, Vec<u8>)> = Vec::new();
    all.extend(fixture_inputs());
    all.extend(pathological_inputs());
    all.extend(resolution_sweep());
    all.extend(conflict_phrase_sweep());
    all.extend(negation_form_sweep(&known));
    all.extend(stopword_sweep(&known));
    all.extend(gerund_sweep(&known));
    all.extend(dimension_pair_sweep());
    all.extend(batch_sweep(&known));
    all.extend(combined_sweep(&known));
    all.extend(extra_edge_sweep());
    all.extend(negation_placement_fp_controls());
    all.extend(prompt_negative_conflict_fp_controls());

    for case in semantic_cases() {
        all.push((
            case["label"].as_str().unwrap().to_string(),
            serde_json::to_vec(&case["input"]).unwrap(),
        ));
    }
    all
}

#[test]
fn research_mapping_references_live_controls() {
    let labels: HashSet<String> = all_inputs().into_iter().map(|(label, _)| label).collect();
    let sources = concat!(
        include_str!("../src/checks.rs"),
        include_str!("fail_silent.rs"),
        include_str!("../src/lint.rs")
    );
    let mut seen = HashSet::new();
    let mut covered = 0;
    let mut deferred = 0;
    for line in include_str!("golden/README.md")
        .lines()
        .filter(|line| line.starts_with("| F-"))
    {
        let fields: Vec<&str> = line.split('|').map(str::trim).collect();
        let id = fields[1];
        assert!(seen.insert(id), "duplicate {id}");
        let reference = fields[3].trim_matches('`');
        match fields[2] {
            "covered" => {
                covered += 1;
                if let Some(name) = reference.strip_prefix("unit:") {
                    assert!(
                        sources.contains(&format!("fn {name}(")),
                        "missing test {id}: {name}"
                    );
                } else {
                    assert!(
                        labels.contains(reference),
                        "missing corpus {id}: {reference}"
                    );
                }
            }
            "deferred" => {
                deferred += 1;
                assert!(["HFR-002", "HFR-003", "HFR-004", "HFR-007"].contains(&reference));
            }
            status => panic!("unknown mapping status {status}"),
        }
        assert!(!fields[4].is_empty(), "missing explanation {id}");
    }
    let expected: HashSet<String> = (1..=6)
        .chain([101, 102, 103, 201, 202, 203, 301, 302])
        .chain(401..=416)
        .chain(501..=508)
        .chain(601..=624)
        .chain(701..=703)
        .map(|n| format!("F-{n:03}"))
        .collect();
    assert_eq!(
        seen.into_iter().map(str::to_string).collect::<HashSet<_>>(),
        expected
    );
    println!("research mapping: {covered} covered / {deferred} deferred / 0 accepted = 65; representative coverage explicitly marked");
}

fn semantic_cases() -> Vec<Value> {
    let data: Value = serde_json::from_str(include_str!("golden/semantic-cases.json")).unwrap();
    data["cases"].as_array().unwrap().clone()
}

fn actual_context(out: &Outcome) -> Option<String> {
    if out.stdout.is_empty() {
        return None;
    }
    let value: Value = serde_json::from_slice(&out.stdout).expect("hook JSON");
    Some(
        value["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("context")
            .to_string(),
    )
}
fn actual_ids(context: Option<&str>) -> Vec<&str> {
    context
        .into_iter()
        .flat_map(|s| s.lines().skip(1))
        .map(|line| line.strip_prefix("- ").unwrap().split_once(": ").unwrap().0)
        .collect()
}

#[test]
fn semantic_contract_and_repeated_process_determinism() {
    let cases = semantic_cases();
    let mut positives: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    let mut negative = 0;
    let mut limitations = 0;
    let mut robustness = 0;
    let mut repeated = 0;
    for case in &cases {
        let label = case["label"].as_str().unwrap();
        let input = serde_json::to_vec(&case["input"]).unwrap();
        let out = run_rust(&input);
        assert_eq!(out.code, 0, "{label}");
        assert!(out.stderr.is_empty(), "{label}");
        let context = actual_context(&out);
        let expected = case["expected_context"].as_str();
        assert_eq!(context.as_deref(), expected, "semantic message: {label}");
        let actual = actual_ids(context.as_deref());
        let expected_ids: Vec<&str> = case["expected_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(actual, expected_ids, "semantic ids: {label}");
        match case["class"].as_str().unwrap() {
            "negative" => {
                negative += 1;
                assert!(actual.is_empty(), "{label}");
            }
            "positive" => {
                assert!(!expected_ids.is_empty(), "{label}");
                for id in expected_ids.into_iter().collect::<HashSet<_>>() {
                    let counts = positives.entry(id.to_string()).or_default();
                    counts.0 += 1;
                    counts.1 += usize::from(actual.contains(&id));
                }
            }
            "robustness" => robustness += 1,
            "limitation" => {
                limitations += 1;
                assert!(
                    case.get("ideal_ids").is_some(),
                    "{label}: missing ideal behavior"
                );
                println!(
                    "known limitation {label}: actual={actual:?}, ideal={}",
                    case["ideal_ids"]
                );
            }
            class => panic!("unknown semantic class {class}"),
        }
        // Named selection, not a timing test. Covers all-check/scope, Unicode and batch paths.
        if label.starts_with("canonical:scope:") || label == "canonical:text:fullwidth" {
            let again = run_rust(&input);
            assert_eq!(
                (again.code, again.stdout, again.stderr),
                (out.code, out.stdout, out.stderr),
                "{label}: nondeterministic"
            );
            repeated += 1;
        }
    }
    println!("semantic mandatory negatives: {negative}, unexpected firings: 0");
    for (id, (total, correct)) in positives {
        println!("supported positives {id}: {correct}/{total}");
    }
    println!("known limitations: {limitations}; robustness: {robustness}; repeated process cases: {repeated}; output mismatches: 0");
}

/// Read-only review aid; never writes or accepts a golden expectation.
#[test]
#[ignore = "read-only mismatch inspection for an intentional behavior review"]
fn audit_golden_changes() {
    let golden = load_golden();
    for (label, input) in all_inputs() {
        let out = run_rust(&input);
        let value = if out.stdout.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&out.stdout).unwrap()
        };
        if golden
            .get(&label)
            .is_none_or(|g| g.output.as_ref().unwrap_or(&Value::Null) != &value)
        {
            let preview: Value = serde_json::from_slice(&input).unwrap_or(Value::Null);
            let preview =
                truncated_lossy(serde_json::to_string(&preview).unwrap().as_bytes(), 1500);
            println!(
                "AUDIT{}",
                json!({"label":label,"input_len":input.len(),"input_fnv1a64":format!("{:016x}",fnv1a64(&input)),"preview":preview,"output":value,"code":out.code,"stderr_empty":out.stderr.is_empty()})
            );
        }
    }
}

#[test]
fn rust_matches_recorded_golden_expectations() {
    let all = all_inputs();
    // De-duplicate by exact stdin byte content. Different sweep groups can legitimately
    // land on the same bytes (e.g. two groups both happening to build an identical params
    // object); the harness should report a distinct-input count, not an inflated one.
    //
    // Discarding the duplicate *input* is fine; discarding the *label* silently is not. If
    // a later task adds a case - e.g. a negative control - whose bytes happen to collide
    // with something already in the corpus, that label would otherwise vanish with no
    // MISSING report, no ORPHANED report, no count change, and a green test: it would just
    // never run. So every collision is named here and fails the test up front, before the
    // golden comparison even starts, rather than disappearing into a `HashSet`.
    let mut owner: HashMap<Vec<u8>, String> = HashMap::new();
    let mut corpus: Vec<(String, Vec<u8>)> = Vec::new();
    let mut collisions: Vec<String> = Vec::new();
    for (label, bytes) in all {
        match owner.get(&bytes) {
            Some(existing_label) => {
                collisions.push(format!(
                    "  [{label}] was dropped by de-duplication: its stdin bytes are identical to \
                     [{existing_label}], already in the corpus. If [{label}] is meant to be a distinct \
                     case, vary its input so it doesn't collide - [{existing_label}] is likely already \
                     covering what it tests. If the duplication really is intentional, delete one of the \
                     two labels from the generator instead of leaving both to collide."
                ));
            }
            None => {
                owner.insert(bytes.clone(), label.clone());
                corpus.push((label, bytes));
            }
        }
    }

    assert!(
        collisions.is_empty(),
        "{} corpus label(s) were silently dropped by de-duplication because their stdin bytes \
         collided with an input already generated under a different label:\n\n{}",
        collisions.len(),
        collisions.join("\n")
    );

    assert!(
        corpus.len() >= 300,
        "corpus has only {} distinct inputs after de-duplication; the task requires >= 300 - widen the sweep",
        corpus.len()
    );

    let golden = load_golden();

    let missing = corpus
        .iter()
        .filter(|(label, _)| !golden.contains_key(label))
        .count();
    let changed_inputs = corpus
        .iter()
        .filter(|(label, input)| {
            golden.get(label).is_some_and(|g| {
                g.input_len != input.len() || g.input_hash != format!("{:016x}", fnv1a64(input))
            })
        })
        .count();
    let mut divergences: Vec<String> = Vec::new();
    let mut seen_labels: HashSet<String> = HashSet::new();
    for (label, input) in &corpus {
        seen_labels.insert(label.clone());
        let rust_out = run_rust(input);
        if let Some(problem) = compare(label, input, &rust_out, golden.get(label)) {
            divergences.push(problem);
        }
    }

    // The reverse of "missing golden entry": a label the golden file still records that the
    // corpus no longer generates at all. Left unreported, `expectations.json` would quietly
    // accumulate dead entries for inputs nothing exercises any more.
    let mut orphaned: Vec<&String> = golden
        .keys()
        .filter(|l| !seen_labels.contains(*l))
        .collect();
    orphaned.sort();
    for label in &orphaned {
        divergences.push(format!(
            "ORPHANED GOLDEN ENTRY [{label}]\n  recorded in expectations.json but the corpus \
             no longer generates this label - either the corpus generator changed or this \
             entry is stale and should be removed"
        ));
    }

    println!(
        "golden integrity: missing {missing}; orphaned {}; changed input cases {changed_inputs}",
        orphaned.len()
    );
    println!(
        "golden corpus: compared {} distinct inputs against {} recorded expectations - {} divergence(s)",
        corpus.len(),
        golden.len(),
        divergences.len()
    );

    assert!(
        divergences.is_empty(),
        "{} divergence(s) found between the Rust binary and the recorded golden expectations:\n\n{}",
        divergences.len(),
        divergences.join("\n\n")
    );
}
