import json
import re
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "src"))
import higgsfield_lint as hl

FIXTURES = Path(__file__).resolve().parent / "fixtures"
SCRIPT = Path(__file__).resolve().parent.parent / "src" / "higgsfield_lint.py"


def load(name):
    return json.loads((FIXTURES / f"{name}.json").read_text())


def ids(findings):
    return sorted(f.check_id for f in findings)


def test_resolution_claim_fires_on_4k_at_768px():
    params = load("kling2_6_4k_slowmo")["params"]
    finding = hl.check_resolution_claim(params, frozenset())
    assert finding is not None
    assert finding.check_id == "resolution_claim"
    assert "4k" in finding.message.lower()
    assert "768" in finding.message


def test_resolution_claim_silent_on_clean_seedance_call():
    params = load("seedance_tree_clean")["params"]
    assert hl.check_resolution_claim(params, frozenset()) is None


def test_resolution_claim_silent_when_no_claim_in_prompt():
    params = {"prompt": "a cat on a windowsill", "width": 100, "height": 100}
    assert hl.check_resolution_claim(params, frozenset()) is None


def test_resolution_claim_silent_when_dimensions_satisfy_claim():
    params = {"prompt": "shot in 720p", "width": 1920, "height": 1080}
    assert hl.check_resolution_claim(params, frozenset()) is None


def test_resolution_claim_prefers_resolution_param_over_dimensions():
    params = {"prompt": "4K hero shot", "resolution": "4k", "width": 100, "height": 100}
    assert hl.check_resolution_claim(params, frozenset()) is None


def test_lint_returns_empty_for_non_dict_params():
    assert hl.lint({"params": "a bare string"}, frozenset()) == []
    assert hl.lint({}, frozenset()) == []
    assert hl.lint("not a dict", frozenset()) == []


def test_render_formats_one_line_per_finding():
    out = hl.render([hl.Finding("check_a", "thing happened."), hl.Finding("check_b", "other.")])
    assert out.splitlines()[0] == "higgsfield-lint: 2 findings"
    assert "- check_a: thing happened." in out


def run_hook(stdin_text):
    return subprocess.run(
        [sys.executable, str(SCRIPT)],
        input=stdin_text, capture_output=True, text=True,
    )


@pytest.mark.parametrize("payload", ["", "not json", "{}", '{"tool_input": null}'])
def test_hook_exits_zero_and_silent_on_malformed_input(payload):
    proc = run_hook(payload)
    assert proc.returncode == 0
    assert proc.stdout == ""
    assert proc.stderr == ""


def test_hook_emits_additional_context_for_a_real_finding():
    payload = json.dumps({"tool_name": "mcp__x__generate_video",
                          "tool_input": load("kling2_6_4k_slowmo")})
    proc = run_hook(payload)
    assert proc.returncode == 0
    out = json.loads(proc.stdout)
    assert out["hookSpecificOutput"]["hookEventName"] == "PreToolUse"
    assert "resolution_claim" in out["hookSpecificOutput"]["additionalContext"]
    assert "permissionDecision" not in out["hookSpecificOutput"]


def test_hook_is_silent_on_clean_call():
    payload = json.dumps({"tool_name": "mcp__x__generate_video",
                          "tool_input": load("seedance_tree_clean")})
    proc = run_hook(payload)
    assert proc.returncode == 0
    assert proc.stdout == ""


def test_conflict_fires_on_slow_motion_in_both_fields():
    params = load("kling2_6_4k_slowmo")["params"]
    finding = hl.check_prompt_negative_conflict(params, frozenset())
    assert finding is not None
    assert finding.check_id == "prompt_negative_conflict"
    assert "slow motion" in finding.message


def test_conflict_does_not_fire_on_text_because_prompt_negates_it():
    # "no text" in the prompt agrees with "random text" in negative_prompt.
    params = load("kling2_6_4k_slowmo")["params"]
    finding = hl.check_prompt_negative_conflict(params, frozenset())
    assert '"text"' not in finding.message


def test_conflict_silent_without_negative_prompt():
    params = load("seedance_tree_clean")["params"]
    assert hl.check_prompt_negative_conflict(params, frozenset()) is None


def test_strip_negations_removes_negated_spans():
    assert "text" not in hl.strip_negations("cinematic, no text, warm light").lower()
    assert "logos" not in hl.strip_negations("without logos please").lower()
    assert "motion" in hl.strip_negations("slow motion shot").lower()


def test_negation_placement_fires_when_negative_prompt_present():
    params = load("kling2_6_4k_slowmo")["params"]
    finding = hl.check_negation_placement(params, frozenset())
    assert finding is not None
    assert finding.check_id == "negation_placement"
    assert "no text" in finding.message.lower()


def test_negation_placement_fires_for_known_model_without_negative_prompt():
    params = {"model": "kling2_6", "prompt": "a field, no text"}
    assert hl.check_negation_placement(params, frozenset({"kling2_6"})) is not None


def test_negation_placement_silent_for_unknown_model():
    params = load("seedance_tree_clean")["params"]
    assert hl.check_negation_placement(params, frozenset({"kling2_6"})) is None


def test_negation_placement_silent_when_prompt_has_no_negations():
    params = {"model": "kling2_6", "prompt": "a bright field at noon"}
    assert hl.check_negation_placement(params, frozenset({"kling2_6"})) is None


def test_load_models_reads_the_data_file():
    assert "kling2_6" in hl.load_models()


# --- F1: check_negation_placement must not fire on ordinary English ---------------

def test_negation_placement_silent_on_narrative_not():
    params = {"model": "kling2_6",
              "prompt": "a lighthouse at dawn, the sun has not yet risen over the water"}
    assert hl.check_negation_placement(params, frozenset({"kling2_6"})) is None


def test_negation_placement_silent_on_quantifier_no_more_than():
    params = {"model": "kling2_6",
              "prompt": "a busy street with no more than three people visible"}
    assert hl.check_negation_placement(params, frozenset({"kling2_6"})) is None


def test_negation_placement_silent_on_without_moving():
    params = {"model": "kling2_6",
              "prompt": "a monk sitting without moving, breathing slowly"}
    assert hl.check_negation_placement(params, frozenset({"kling2_6"})) is None


def test_negation_placement_still_fires_on_real_directives():
    params = {"model": "kling2_6",
              "prompt": "clean scene, no text, no logos, no clutter"}
    finding = hl.check_negation_placement(params, frozenset({"kling2_6"}))
    assert finding is not None
    lowered = finding.message.lower()
    for phrase in ('"no text"', '"no logos"', '"no clutter"'):
        assert phrase in lowered


def test_negation_placement_still_fires_on_without_a_watermark():
    params = {"model": "kling2_6", "prompt": "clean scene, without a watermark"}
    finding = hl.check_negation_placement(params, frozenset({"kling2_6"}))
    assert finding is not None
    assert '"without a watermark"' in finding.message.lower()


def test_strip_negations_still_strips_not_phrases():
    # strip_negations backs check_prompt_negative_conflict, where "not blurry" must
    # still read as a negation so it doesn't look like a request for blur.
    assert "blurry" not in hl.strip_negations("not blurry, cinematic").lower()


# --- F2: batch tool shape ({"requests": [{"index", "params"}, ...]}) --------------

def test_lint_handles_batch_with_one_offending_entry():
    tool_input = {"requests": [{"index": 0, "params": load("kling2_6_4k_slowmo")["params"]}]}
    findings = hl.lint(tool_input, frozenset({"kling2_6"}))
    assert len(findings) == 3
    assert all("request 0:" in f.message for f in findings)


def test_lint_handles_batch_where_only_second_entry_offends():
    tool_input = {"requests": [
        {"index": 0, "params": load("seedance_tree_clean")["params"]},
        {"index": 1, "params": load("kling2_6_4k_slowmo")["params"]},
    ]}
    findings = hl.lint(tool_input, frozenset({"kling2_6"}))
    assert len(findings) == 3
    assert all("request 1:" in f.message for f in findings)
    assert all("request 0:" not in f.message for f in findings)


def test_lint_handles_batch_with_missing_requests_list():
    assert hl.lint({}, frozenset()) == []


def test_lint_handles_batch_with_empty_requests_list():
    assert hl.lint({"requests": []}, frozenset()) == []


# --- F3: params as a JSON string is a legal shape, not malformed -----------------

def test_lint_parses_params_given_as_json_string():
    params = {"model": "kling2_6", "prompt": "4k hero shot", "width": 200, "height": 150}
    findings = hl.lint({"params": json.dumps(params)}, frozenset())
    assert any(f.check_id == "resolution_claim" for f in findings)


def test_lint_still_silent_on_unparseable_string_params():
    # "a bare string" is not valid JSON, so this stays malformed -> silent.
    assert hl.lint({"params": "a bare string"}, frozenset()) == []


# --- F4: absent prompt key entirely (SPEC §9 malformed fixture) ------------------

def test_lint_returns_empty_when_prompt_key_is_entirely_absent():
    params = {"model": "kling2_6", "width": 1920, "height": 1080}
    assert hl.lint({"params": params}, frozenset()) == []


# --- F5: empty negative_prompt is not evidence of support ------------------------

def test_negation_placement_silent_when_negative_prompt_is_empty_string():
    params = {"model": "seedance_2_5", "negative_prompt": "", "prompt": "a shot, no text"}
    assert hl.check_negation_placement(params, frozenset()) is None


# --- F6: booleans must not satisfy the int dimension check -----------------------

def test_resolution_claim_ignores_boolean_dimensions():
    params = {"prompt": "a 4k shot", "width": True, "height": True}
    assert hl.check_resolution_claim(params, frozenset()) is None


# --- F7: dimensions below the sanity floor (aspect-ratio hints) are ignored ------

def test_resolution_claim_ignores_dimensions_below_sanity_floor():
    params = {"prompt": "a 4k shot", "width": 16, "height": 9}
    assert hl.check_resolution_claim(params, frozenset()) is None


# --- F8: subject-verb agreement ---------------------------------------------------

def test_negation_placement_uses_singular_verb_for_one_negation():
    params = {"model": "kling2_6", "prompt": "a bright field, no clutter"}
    finding = hl.check_negation_placement(params, frozenset({"kling2_6"}))
    assert finding is not None
    assert re.search(r"\bsits\b", finding.message)
    assert not re.search(r"\bsit\b", finding.message)


def test_negation_placement_uses_plural_verb_for_multiple_negations():
    params = {"model": "kling2_6", "prompt": "a bright field, no clutter, no text"}
    finding = hl.check_negation_placement(params, frozenset({"kling2_6"}))
    assert finding is not None
    assert re.search(r"\bsit\b", finding.message)
    assert not re.search(r"\bsits\b", finding.message)


def test_conflict_uses_singular_verb_for_one_shared_term():
    params = {"prompt": "static shot, nice light", "negative_prompt": "static, bad quality"}
    finding = hl.check_prompt_negative_conflict(params, frozenset())
    assert finding is not None
    assert re.search(r"\bappears\b", finding.message)
    assert not re.search(r"\bappear\b", finding.message)


def test_conflict_uses_plural_verb_for_multiple_shared_terms():
    params = {"prompt": "static, sharp shot", "negative_prompt": "static, sharp, bad quality"}
    finding = hl.check_prompt_negative_conflict(params, frozenset())
    assert finding is not None
    assert re.search(r"\bappear\b", finding.message)
    assert not re.search(r"\bappears\b", finding.message)


# --- F9: raw dimensions in the message, original prompt casing for the claim ----

def test_resolution_claim_message_includes_raw_dimensions_and_original_casing():
    params = load("kling2_6_4k_slowmo")["params"]
    finding = hl.check_resolution_claim(params, frozenset())
    assert finding is not None
    assert '"4K"' in finding.message
    assert "width 1376 x height 768 gives a 768px short side" in finding.message


def test_resolution_claim_message_omits_raw_dimensions_when_source_is_resolution_param():
    params = {"prompt": "shot in 4k", "resolution": "1080p"}
    finding = hl.check_resolution_claim(params, frozenset())
    assert finding is not None
    assert "width" not in finding.message
    assert "the call produces a 1080px short side" in finding.message


# --- Patch 1 regression: the bare-gerund noise clause must key on "without" only --
#
# `_is_placement_noise` used to fire on `len(words) == 2 and words[1].endswith("ing")`
# regardless of which negator introduced it, so "no lettering", "no flickering",
# "no warping" and "avoid flickering" were all silently dropped even though these are
# exactly the artifact-exclusion directives this check exists to catch. All four use
# a model known to accept negative_prompt (kling2_6) so the check actually runs.

@pytest.mark.parametrize("phrase", ["no lettering", "no flickering", "no warping", "avoid flickering"])
def test_patch1_fires_on_previously_silenced_gerund_directives(phrase):
    params = {"model": "kling2_6", "prompt": f"clean scene, {phrase}"}
    finding = hl.check_negation_placement(params, frozenset({"kling2_6"}))
    assert finding is not None, f'"{phrase}" must fire negation_placement'
    assert phrase in finding.message.lower()


def test_patch1_stays_silent_on_without_plus_bare_gerund():
    # "without moving" is a manner adverbial ("sitting without moving") describing how
    # the subject behaves, not a noun naming an artifact to exclude from the shot.
    params = {"model": "kling2_6", "prompt": "a monk sitting without moving, breathing slowly"}
    assert hl.check_negation_placement(params, frozenset({"kling2_6"})) is None


def test_patch1_stays_silent_on_narrative_negation():
    params = {"model": "kling2_6",
              "prompt": "a lighthouse at dawn, the sun has not yet risen over the water"}
    assert hl.check_negation_placement(params, frozenset({"kling2_6"})) is None


def test_patch1_stays_silent_on_quantifier_idiom():
    params = {"model": "kling2_6",
              "prompt": "a busy street with no more than three people visible"}
    assert hl.check_negation_placement(params, frozenset({"kling2_6"})) is None


@pytest.mark.parametrize("phrase", ["no text", "no logos", "no clutter", "without a watermark"])
def test_patch1_still_fires_on_noun_phrase_directives(phrase):
    params = {"model": "kling2_6", "prompt": f"clean scene, {phrase}"}
    finding = hl.check_negation_placement(params, frozenset({"kling2_6"}))
    assert finding is not None, f'"{phrase}" must still fire negation_placement'
    assert phrase in finding.message.lower()


# --- Patch 2 regression: the dimension floor must key on the larger side ---------
#
# `_is_valid_dimension` used to be required of BOTH width and height, so any pair
# with one side under MIN_SANE_DIMENSION (128) suppressed the resolution check
# entirely -- silencing genuine complaints like 1920x100. The floor should instead
# be keyed on the larger dimension, since the aspect-ratio hints it was meant to
# ignore (e.g. width=16, height=9) have BOTH sides small.

def test_patch2_fires_on_narrow_short_side_with_large_long_side():
    params = {"prompt": "a 4k banner shot", "width": 1920, "height": 100}
    finding = hl.check_resolution_claim(params, frozenset())
    assert finding is not None
    assert "width 1920 x height 100 gives a 100px short side" in finding.message


def test_patch2_stays_silent_on_aspect_ratio_style_pair():
    params = {"prompt": "a 4k banner shot", "width": 16, "height": 9}
    assert hl.check_resolution_claim(params, frozenset()) is None


def test_patch2_stays_silent_when_both_dimensions_below_floor():
    params = {"prompt": "a 4k banner shot", "width": 64, "height": 64}
    assert hl.check_resolution_claim(params, frozenset()) is None


def test_homograph_in_prose_does_not_conflict():
    """Measured false positives, 2026-09-10. Each has the term buried in a prose clause
    on the positive side while the negative_prompt uses it as a directive."""
    for prompt, negative in [
        ("a cast iron pan of sizzling garlic, a woman reading a text message", "pan, text, blurry"),
        ("a field of grain at golden hour", "grain, noise"),
        ("a static caravan in a field", "static, jitter"),
    ]:
        params = {"prompt": prompt, "negative_prompt": negative, "width": 1280, "height": 720}
        assert hl.check_prompt_negative_conflict(params, frozenset()) is None, prompt


def test_directive_as_a_tag_still_conflicts():
    """The real kling2_6 fixture behaviour must survive: "slow motion" is its own
    two-word comma-delimited tag on both sides."""
    params = {
        "prompt": "cinematic, ultra-clean commercial style, slow motion, warm highlights",
        "negative_prompt": "transitions, slow motion, bad quality",
        "width": 1376,
        "height": 768,
    }
    finding = hl.check_prompt_negative_conflict(params, frozenset())
    assert finding is not None
    assert '"slow motion"' in finding.message


def test_tag_segment_boundary_is_four_words():
    long_tag = {"prompt": "a very wide shot of a valley", "negative_prompt": "wide shot", "width": 1280, "height": 720}
    short_tag = {"prompt": "a valley, wide shot, dawn", "negative_prompt": "wide shot", "width": 1280, "height": 720}
    assert hl.check_prompt_negative_conflict(long_tag, frozenset()) is None
    assert hl.check_prompt_negative_conflict(short_tag, frozenset()) is not None
