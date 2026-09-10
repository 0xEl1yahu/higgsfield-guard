//! Bounded behavioural gates. CI runs these as three separately-named steps so a red
//! build names the class that broke rather than just "a test failed".
//!
//! An advisory lint has exactly three ways to become worth deleting:
//!
//!   1. It chokes or talks nonsense on a bad prompt (SPEC 7 - it is in the path of every
//!      generation call, so its own failure must be invisible).
//!   2. It mishandles a call that omits details, guessing where SPEC 5.3 requires silence.
//!   3. It gives advice that is inaccurate, or that would damage the shot if followed.
//!
//! Every case here was measured against the release binary before being written down.
//! Add a case when a real call proves one, not when one is imagined.

use std::io::Write;
use std::process::{Command, Stdio};

/// Feed one `tool_input` to the hook and return the check ids it emitted, in order.
/// An empty vec means the hook was silent.
///
/// Also asserts the two global constraints on every single case, for free: exit 0 and an
/// empty stderr. A gate case that broke either would otherwise pass unnoticed.
fn fired(tool_input: &str) -> Vec<String> {
    let exe = env!("CARGO_BIN_EXE_higgsfield-lint");
    let mut child = Command::new(exe)
        // Harmless before the findings log exists; suppresses it afterwards. The gates
        // are not usage and must not land in the log.
        .env("HIGGSFIELD_LINT_NO_LOG", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(format!(r#"{{"tool_input":{tool_input}}}"#).as_bytes())
        .ok();
    let out = child.wait_with_output().expect("wait");
    assert_eq!(out.status.code(), Some(0), "the hook must always exit 0");
    assert!(
        out.stderr.is_empty(),
        "the hook must never write to stderr, got: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    if out.stdout.is_empty() {
        return Vec::new();
    }
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("hook output must be valid JSON");
    v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("additionalContext must be a string")
        .lines()
        .skip(1) // the "higgsfield-lint: N finding(s)" header
        .filter_map(|l| {
            l.strip_prefix("- ")
                .and_then(|rest| rest.split(':').next())
                .map(str::to_string)
        })
        .collect()
}

/// Run every case in a gate and report ALL failures together. A gate that stops at the
/// first mismatch makes you re-run CI once per broken case.
fn run_gate(name: &str, cases: &[(&str, &str, &[&str])]) {
    let mut failures = Vec::new();
    for (label, tool_input, expected) in cases {
        let got = fired(tool_input);
        let want: Vec<String> = expected.iter().map(|s| (*s).to_string()).collect();
        if got != want {
            failures.push(format!(
                "  {label}\n    expected: {want:?}\n    got:      {got:?}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "gate '{name}': {} of {} case(s) failed:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

// ---------------------------------------------------------------------------------------
// Gate 1: bad prompts
// ---------------------------------------------------------------------------------------

#[test]
fn gate_1_bad_prompts() {
    let cases: &[(&str, &str, &[&str])] = &[
        (
            "empty prompt",
            r#"{"params":{"prompt":"","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "whitespace-only prompt",
            r#"{"params":{"prompt":"   ","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "punctuation-only prompt",
            r#"{"params":{"prompt":"!!! ??? ---","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "prompt is a number",
            r#"{"params":{"prompt":123,"width":1280,"height":720}}"#,
            &[],
        ),
        (
            "prompt is null",
            r#"{"params":{"prompt":null,"width":1280,"height":720}}"#,
            &[],
        ),
        // Characterisation, not endorsement. A prompt of bare negators matches the
        // negation pattern and yields a nonsense suggestion. Pinned so it cannot drift
        // silently; not worth a rule, because the input is already nonsense and no real
        // call looks like this.
        (
            "prompt is bare negators",
            r#"{"params":{"model":"kling2_6","prompt":"no no no no no","width":1280,"height":720}}"#,
            &["negation_placement"],
        ),
        // Real prompts are not always comma-delimited; newline-separated directives must
        // still be found.
        (
            "newline-separated directives",
            r#"{"params":{"model":"kling2_6","prompt":"a street\nno logos\nno text","width":1280,"height":720}}"#,
            &["negation_placement"],
        ),
    ];
    run_gate("bad prompts", cases);
}

// ---------------------------------------------------------------------------------------
// Gate 2: missing details
// ---------------------------------------------------------------------------------------

#[test]
fn gate_2_missing_details() {
    let cases: &[(&str, &str, &[&str])] = &[
        // The documented blind spot, pinned deliberately. With no `model` and no
        // `negative_prompt`, nothing proves this model accepts one, and SPEC 5.3 requires
        // silence over a guess. If this ever starts firing, someone has made the lint
        // infer negative-prompt support - which is exactly what 5.3 forbids.
        (
            "no model, no negative_prompt",
            r#"{"params":{"prompt":"a street, no logos, no text overlays","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "no model, negative_prompt present",
            r#"{"params":{"prompt":"a street, no logos","negative_prompt":"blurry","width":1280,"height":720}}"#,
            &["negation_placement"],
        ),
        // An all-whitespace negative_prompt is not evidence of support.
        (
            "negative_prompt is whitespace",
            r#"{"params":{"prompt":"a street, no logos","negative_prompt":"   ","width":1280,"height":720}}"#,
            &[],
        ),
        // A malformed negative_prompt is not evidence either - but the model is in the
        // known-accepts list, so the check still fires on that basis alone.
        (
            "negative_prompt is not a string, model is known",
            r#"{"params":{"model":"kling2_6","prompt":"a street, no logos","negative_prompt":123,"width":1280,"height":720}}"#,
            &["negation_placement"],
        ),
        ("params absent entirely", r#"{"model":"kling2_6"}"#, &[]),
        (
            "prompt absent",
            r#"{"params":{"width":1280,"height":720}}"#,
            &[],
        ),
        // No way to know what the call will actually produce, so the claim cannot be
        // contradicted.
        (
            "no dimensions and no resolution",
            r#"{"params":{"prompt":"cinematic 4k"}}"#,
            &[],
        ),
        (
            "batch entry missing params",
            r#"{"requests":[{"index":0}]}"#,
            &[],
        ),
    ];
    run_gate("missing details", cases);
}

// ---------------------------------------------------------------------------------------
// Gate 3: inaccurate or harmful advice
// ---------------------------------------------------------------------------------------

#[test]
fn gate_3_inaccurate_or_harmful_advice() {
    let cases: &[(&str, &str, &[&str])] = &[
        // SPEC 5.3 names these explicitly: moving any of them into negative_prompt would
        // do nothing, or damage the shot. Firing here is worse than staying quiet.
        (
            "quantifier idiom: no more than",
            r#"{"params":{"model":"kling2_6","prompt":"a room with no more than three people","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "quantifier idiom: no longer",
            r#"{"params":{"model":"kling2_6","prompt":"the sun no longer visible","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "quantifier idiom: no matter",
            r#"{"params":{"model":"kling2_6","prompt":"walking on, no matter the weather","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "manner adverbial: without moving",
            r#"{"params":{"model":"kling2_6","prompt":"a figure sitting without moving","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "narrative negation: has not yet risen",
            r#"{"params":{"model":"kling2_6","prompt":"the sun has not yet risen","width":1280,"height":720}}"#,
            &[],
        ),
        // The other half of the same contract: the guards above must not be over-applied.
        // "no flickering" and "avoid warping" name a defect to exclude and must fire, even
        // though the first also ends in "-ing".
        (
            "real defect: no flickering",
            r#"{"params":{"model":"kling2_6","prompt":"a steady shot, no flickering","width":1280,"height":720}}"#,
            &["negation_placement"],
        ),
        (
            "real defect: avoid warping",
            r#"{"params":{"model":"kling2_6","prompt":"a steady shot, avoid warping","width":1280,"height":720}}"#,
            &["negation_placement"],
        ),
        // A satisfied resolution claim must not be second-guessed: telling someone to
        // raise a resolution that is already correct is advice that costs attention and
        // buys nothing.
        (
            "4k claim at 3840x2160",
            r#"{"params":{"prompt":"cinematic 4k","width":3840,"height":2160}}"#,
            &[],
        ),
        (
            "4k claim with resolution=4k",
            r#"{"params":{"prompt":"cinematic 4k","resolution":"4k","width":100,"height":100}}"#,
            &[],
        ),
        (
            "720p claim at 1920x1080",
            r#"{"params":{"prompt":"shot in 720p","width":1920,"height":1080}}"#,
            &[],
        ),
        // An aspect-ratio pair is not a render size. Reading 16x9 as a 9px short side
        // would fire on essentially every call that expresses its shape this way.
        (
            "aspect ratio pair is not a render size",
            r#"{"params":{"prompt":"cinematic 4k","width":16,"height":9}}"#,
            &[],
        ),
    ];
    run_gate("inaccurate or harmful advice", cases);
}

#[test]
fn gate_3_known_good_fixtures_stay_silent() {
    // SPEC 9: "a check that fires on a known-good call is a bug". The seedance fixtures
    // are real captured calls that must produce nothing. They matter more than the
    // positives - the whole value of an advisory tool is that its advice is worth reading.
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let text = std::fs::read_to_string(dir.join("seedance_tree_clean.json"))
        .expect("seedance_tree_clean.json must exist");
    let got = fired(&text);
    assert!(
        got.is_empty(),
        "the known-good seedance fixture fired: {got:?}"
    );
}
