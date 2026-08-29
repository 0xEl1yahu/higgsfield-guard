import json
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
