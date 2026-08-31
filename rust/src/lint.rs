use serde_json::Value;
use std::collections::HashSet;

// `checks` and `models` are top-level sibling modules declared in main.rs (the crate
// root), which resolves rust/src/checks.rs and rust/src/models.rs natively since they
// are plain siblings of lint.rs - no `#[path]` juggling needed. `lint` is the
// orchestrator and depends on both.
use crate::checks;
use crate::models;

pub struct Finding {
    pub check_id: String,
    pub message: String,
}

pub fn load_models() -> HashSet<String> {
    models::load_models()
}

/// Port of `_resolve_params` (oracle lines 206-219). All three Higgsfield tool schemas
/// declare `params` as `anyOf: [object, string]`: a JSON-encoded string is a legal shape,
/// not a malformed fixture. A string is parsed and linted normally when it decodes to an
/// object; anything else (a string that fails to parse, or that parses to a non-object,
/// or a `raw` that is neither an object nor a string - e.g. missing, null, a list, a bool)
/// falls through to `None`/silence.
fn resolve_params(raw: Option<&Value>) -> Option<Value> {
    match raw {
        Some(Value::Object(_)) => raw.cloned(),
        Some(Value::String(s)) => {
            let parsed: Value = serde_json::from_str(s).ok()?;
            if parsed.is_object() {
                Some(parsed)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Port of `_lint_params` (oracle lines 222-228). Runs the three checks against a single
/// resolved `params` object, in the exact order of the oracle's `CHECKS` tuple
/// (`check_resolution_claim`, `check_prompt_negative_conflict`, `check_negation_placement`)
/// - the rendered output is order-dependent, so this sequence of pushes (rather than a
/// set or a map iteration) is what keeps that order stable.
fn lint_params(params: &Value, models: &HashSet<String>) -> Vec<Finding> {
    let mut findings = Vec::new();
    if let Some(f) = checks::check_resolution_claim(params) {
        findings.push(f);
    }
    if let Some(f) = checks::check_prompt_negative_conflict(params) {
        findings.push(f);
    }
    if let Some(f) = checks::check_negation_placement(params, models) {
        findings.push(f);
    }
    findings
}

/// Python's `str()` of the decoded JSON value bound to `entry.get("index", i)`, used to
/// render `f"request {index}: ..."` (oracle line 248). `index` can be whatever JSON type
/// the caller supplied (string, number, bool, null) as well as the positional `int`
/// fallback; this mirrors Python's default stringification for each of those, not just
/// the plain-integer case the tests exercise.
fn format_index(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => {
            if *b {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Value::Null => "None".to_string(),
        // KNOWN, ACCEPTED DIVERGENCE: `other` covers Value::Array and Value::Object, i.e. a
        // compound `index`. The Python oracle's `str()` renders these Python-repr style -
        // `[1, 2]` with a space after the comma, `{'a': 1}` with single quotes - while
        // serde_json's `Display` renders `[1,2]` and `{"a":1}`. This was ruled acceptable
        // rather than hand-rolling a Python-repr emulator in Rust: `index` is schema-typed
        // as an integer, so a compound value here only arises from a malformed call, and a
        // partial repr emulation would *look* like parity without *being* parity. Because of
        // this, rust/tests/parity.rs deliberately generates scalar indices only.
        other => other.to_string(),
    }
}

/// Port of `lint` (oracle lines 231-253).
pub fn lint(tool_input: &Value, models: &HashSet<String>) -> Vec<Finding> {
    let Some(tool_input) = tool_input.as_object() else {
        return Vec::new();
    };

    // Batch tools (generate_*_batch) use {"requests": [{"index": 0, "params": {...}}, ...]}
    // instead of a top-level `params`. When `requests` is a non-empty list the top-level
    // `params` is NEVER consulted - the oracle returns from this branch unconditionally
    // (line 249), even if every entry is skipped and `findings` ends up empty.
    if let Some(Value::Array(requests)) = tool_input.get("requests") {
        if !requests.is_empty() {
            let mut findings = Vec::new();
            for (i, entry) in requests.iter().enumerate() {
                let Some(entry) = entry.as_object() else {
                    continue; // not a dict: skipped, does NOT fall back to top-level params
                };
                let Some(params) = resolve_params(entry.get("params")) else {
                    continue; // params didn't resolve: skipped, same as above
                };
                let index = entry
                    .get("index")
                    .map(format_index)
                    .unwrap_or_else(|| i.to_string()); // entry.get("index", i) - positional fallback
                for finding in lint_params(&params, models) {
                    findings.push(Finding {
                        check_id: finding.check_id,
                        message: format!("request {}: {}", index, finding.message),
                    });
                }
            }
            return findings;
        }
    }

    let Some(params) = resolve_params(tool_input.get("params")) else {
        return Vec::new();
    };
    lint_params(&params, models)
}

/// Port of `render` (oracle lines 256-260).
pub fn render(findings: &[Finding]) -> String {
    let plural = if findings.len() == 1 { "" } else { "s" };
    let mut lines = vec![format!("higgsfield-lint: {} finding{}", findings.len(), plural)];
    lines.extend(
        findings
            .iter()
            .map(|f| format!("- {}: {}", f.check_id, f.message)),
    );
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn empty_models() -> HashSet<String> {
        HashSet::new()
    }

    // --- batch path ----------------------------------------------------------------

    #[test]
    fn batch_prefixes_only_the_offending_entry_with_its_explicit_index() {
        let tool_input = json!({
            "requests": [
                {"index": 10, "params": {"prompt": "a cat on a windowsill", "width": 100, "height": 100}},
                {"index": 20, "params": {"prompt": "shot in 4k", "width": 1280, "height": 720}},
                {"index": 30, "params": {"prompt": "a dog in a garden", "width": 100, "height": 100}},
            ]
        });
        let findings = lint(&tool_input, &empty_models());
        assert_eq!(findings.len(), 1);
        assert!(
            findings[0].message.starts_with("request 20: "),
            "got: {}",
            findings[0].message
        );
    }

    #[test]
    fn batch_falls_back_to_positional_index_when_index_field_absent() {
        let tool_input = json!({
            "requests": [
                {"params": {"prompt": "a cat on a windowsill", "width": 100, "height": 100}},
                {"params": {"prompt": "shot in 4k", "width": 1280, "height": 720}},
                {"params": {"prompt": "a dog in a garden", "width": 100, "height": 100}},
            ]
        });
        let findings = lint(&tool_input, &empty_models());
        assert_eq!(findings.len(), 1);
        assert!(
            findings[0].message.starts_with("request 1: "),
            "got: {}",
            findings[0].message
        );
    }

    #[test]
    fn batch_entry_accepts_json_string_params() {
        let tool_input = json!({
            "requests": [
                {"params": "{\"prompt\": \"shot in 4k\", \"width\": 1280, \"height\": 720}"}
            ]
        });
        let findings = lint(&tool_input, &empty_models());
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.starts_with("request 0: "));
    }

    #[test]
    fn batch_skips_non_dict_entries_and_entries_with_unresolvable_params() {
        let tool_input = json!({
            "requests": [
                1,
                "not a dict",
                {"params": "not valid json"},
                {"params": {"prompt": "shot in 4k", "width": 1280, "height": 720}}
            ]
        });
        let findings = lint(&tool_input, &empty_models());
        assert_eq!(findings.len(), 1);
        assert!(
            findings[0].message.starts_with("request 3: "),
            "got: {}",
            findings[0].message
        );
    }

    #[test]
    fn empty_requests_list_falls_through_to_top_level_params() {
        let tool_input = json!({
            "requests": [],
            "params": {"prompt": "shot in 4k", "width": 1280, "height": 720}
        });
        let findings = lint(&tool_input, &empty_models());
        assert_eq!(findings.len(), 1);
        assert!(!findings[0].message.starts_with("request "));
    }

    #[test]
    fn non_empty_requests_never_falls_back_to_top_level_params() {
        // Every entry is skipped (no "params" field), so `findings` is empty - but the
        // oracle still returns unconditionally from the batch branch (line 249) rather
        // than falling back to the top-level `params`, which would otherwise fire.
        let tool_input = json!({
            "requests": [ {"no_params_field": true} ],
            "params": {"prompt": "shot in 4k", "width": 100, "height": 100}
        });
        let findings = lint(&tool_input, &empty_models());
        assert!(findings.is_empty());
    }

    // --- top-level (non-batch) path --------------------------------------------------

    #[test]
    fn top_level_json_string_params_is_parsed_and_linted() {
        let tool_input = json!({
            "params": "{\"prompt\": \"shot in 4k\", \"width\": 1280, \"height\": 720}"
        });
        let findings = lint(&tool_input, &empty_models());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "resolution_claim");
    }

    #[test]
    fn top_level_non_json_string_params_is_silent() {
        // Regression guard matching the existing Python test: a bare string that is not
        // valid JSON must still return no findings, not panic or error.
        let tool_input = json!({"params": "a bare string"});
        assert!(lint(&tool_input, &empty_models()).is_empty());
    }

    #[test]
    fn a_single_call_can_produce_multiple_findings_in_checks_order() {
        let tool_input = json!({
            "params": {
                "prompt": "shot in 4k, no logos, sharp focus",
                "negative_prompt": "sharp, blurry",
                "width": 1280,
                "height": 720
            }
        });
        let findings = lint(&tool_input, &empty_models());
        let ids: Vec<&str> = findings.iter().map(|f| f.check_id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "resolution_claim",
                "prompt_negative_conflict",
                "negation_placement"
            ]
        );
    }

    #[test]
    fn non_dict_tool_input_is_silent() {
        assert!(lint(&json!("not an object"), &empty_models()).is_empty());
        assert!(lint(&json!(null), &empty_models()).is_empty());
    }

    // --- render ----------------------------------------------------------------------

    #[test]
    fn render_uses_singular_header_for_one_finding() {
        let findings = vec![Finding {
            check_id: "resolution_claim".to_string(),
            message: "some message".to_string(),
        }];
        let rendered = render(&findings);
        let mut lines = rendered.lines();
        assert_eq!(lines.next(), Some("higgsfield-lint: 1 finding"));
        assert_eq!(lines.next(), Some("- resolution_claim: some message"));
        assert_eq!(lines.next(), None);
    }

    #[test]
    fn render_uses_plural_header_for_multiple_findings() {
        let findings = vec![
            Finding {
                check_id: "resolution_claim".to_string(),
                message: "first".to_string(),
            },
            Finding {
                check_id: "negation_placement".to_string(),
                message: "second".to_string(),
            },
        ];
        let rendered = render(&findings);
        let mut lines = rendered.lines();
        assert_eq!(lines.next(), Some("higgsfield-lint: 2 findings"));
        assert_eq!(lines.next(), Some("- resolution_claim: first"));
        assert_eq!(lines.next(), Some("- negation_placement: second"));
        assert_eq!(lines.next(), None);
    }

    #[test]
    fn render_uses_plural_header_for_zero_findings() {
        let rendered = render(&[]);
        assert_eq!(rendered, "higgsfield-lint: 0 findings");
    }
}
