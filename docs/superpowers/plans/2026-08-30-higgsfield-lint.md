# Higgsfield Lint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `PreToolUse` hook that warns when a Higgsfield generation prompt contradicts its own parameters, plus a weekly credit-expiry reminder.

**Architecture:** One stdlib-only Python file reading hook JSON on stdin and writing `additionalContext` on stdout. Pure functions for every check, with the I/O boundary confined to `main()`. Findings are advisory; the hook never returns `deny`. A separate scheduled Claude Code task handles credit expiry, because hooks cannot call MCP tools.

**Tech Stack:** Python 3 standard library only. pytest for tests. Claude Code `PreToolUse` hooks.

**Spec:** `SPEC.md`

## Global Constraints

- **Standard library only.** A failed import breaks every Higgsfield generation call. No third-party runtime dependencies (SPEC §7).
- **Never block.** The hook must never emit `permissionDecision: "deny"` and must never exit non-zero (SPEC §4, §7).
- **Fail silent.** Any exception, malformed stdin, or missing field exits 0 with no stdout and no stderr (SPEC §7).
- **No network, no MCP, no filesystem writes.** The only read is `data/negative_prompt_models.json` (SPEC §7).
- **Negative fixtures are the priority.** A check that fires on a known-good call is a bug (SPEC §9).
- Matcher is `mcp__.*__generate_(image|video|audio)(_batch)?$`, matching tool shape rather than server id (SPEC §3).

---

## File Structure

| File | Responsibility |
|---|---|
| `src/higgsfield_lint.py` | All checks, rendering, and the stdin/stdout boundary. One file because it is one hook and it must stay readable in a single view. |
| `data/negative_prompt_models.json` | Evidence-seeded list of models known to accept `negative_prompt`. Data, so it changes without touching code. |
| `tests/fixtures/kling2_6_4k_slowmo.json` | Real captured tool input. Positive fixture for checks 5.1, 5.2, 5.3. |
| `tests/fixtures/seedance_tree_clean.json` | Real captured tool input. Negative fixture: must produce zero findings. |
| `tests/test_lint.py` | pytest over the pure functions plus the malformed-input contract. |
| `.claude/settings.json` | Hook registration. |

---

### Task 1: Hook scaffold and the resolution-claim check

**Files:**
- Create: `src/higgsfield_lint.py`
- Create: `tests/fixtures/kling2_6_4k_slowmo.json`
- Create: `tests/fixtures/seedance_tree_clean.json`
- Create: `tests/test_lint.py`

**Interfaces:**
- Consumes: nothing.
- Produces: `Finding(check_id: str, message: str)` NamedTuple; `lint(tool_input: dict, negative_prompt_models: frozenset[str]) -> list[Finding]`; `render(findings: list[Finding]) -> str`; every check has the uniform signature `check_x(params: dict, models: frozenset[str]) -> Finding | None`.

- [ ] **Step 1: Write the fixtures**

`tests/fixtures/kling2_6_4k_slowmo.json` (real `kling2_6` call, trimmed to the linted fields):

```json
{
  "params": {
    "model": "kling2_6",
    "prompt": "cinematic, ultra-clean commercial style, 4K, shallow depth of field, soft natural lighting, minimal composition, calm pacing, premium, modern, no text, no logos, no clutter, visually poetic, slow motion, tasteful colour grading, warm highlights",
    "negative_prompt": "transitions, scene cuts, fading, bad quality, low quality, overexposed, slow motion, static, blurred details, ugly, unnatural movement, random text, bad anatomy",
    "duration": 5,
    "width": 1376,
    "height": 768
  }
}
```

`tests/fixtures/seedance_tree_clean.json` (real `seedance_2_5` call):

```json
{
  "params": {
    "model": "seedance_2_5",
    "prompt": "Flat 2D vector cartoon animation on a solid bright sunny yellow background that never changes. A tiny seedling sprout character wearing black sunglasses grows steadily into a large full tree over the shot. Camera locked off, no camera movement, no zoom, no pan. No text, no lettering, no words at any point.",
    "aspect_ratio": "16:9",
    "duration": 10,
    "width": 1920,
    "height": 1080,
    "resolution": "1080p",
    "generate_audio": false
  }
}
```

- [ ] **Step 2: Write the failing tests**

`tests/test_lint.py`:

```python
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
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `python3 -m pytest tests/test_lint.py -v`
Expected: FAIL, `ModuleNotFoundError: No module named 'higgsfield_lint'`

- [ ] **Step 4: Write the implementation**

`src/higgsfield_lint.py`:

```python
#!/usr/bin/env python3
"""Advisory PreToolUse lint for Higgsfield generation calls. See SPEC.md.

Never blocks. Never exits non-zero. Standard library only.
"""
import json
import re
import sys
from pathlib import Path
from typing import NamedTuple, Optional


class Finding(NamedTuple):
    check_id: str
    message: str


DATA_FILE = Path(__file__).resolve().parent.parent / "data" / "negative_prompt_models.json"

# Claimed resolution -> minimum acceptable short side in pixels.
RESOLUTION_CLAIMS = {
    "8k": 4320,
    "4k": 2160,
    "uhd": 2160,
    "1080p": 1080,
    "full hd": 1080,
    "720p": 720,
    "480p": 480,
}
# Values the `resolution` parameter itself can take.
RESOLUTION_PARAM = {"4k": 2160, "1080p": 1080, "720p": 720, "480p": 480}


def _has_term(text: str, term: str) -> bool:
    return re.search(rf"(?<![a-z0-9]){re.escape(term)}(?![a-z0-9])", text) is not None


def _claimed_short_side(prompt: str):
    """Highest resolution claim present in the prompt, as (term, pixels)."""
    low = prompt.lower()
    best = None
    for term, px in RESOLUTION_CLAIMS.items():
        if _has_term(low, term) and (best is None or px > best[1]):
            best = (term, px)
    return best


def _actual_short_side(params: dict) -> Optional[int]:
    res = params.get("resolution")
    if isinstance(res, str) and res.lower() in RESOLUTION_PARAM:
        return RESOLUTION_PARAM[res.lower()]
    width, height = params.get("width"), params.get("height")
    if isinstance(width, int) and isinstance(height, int) and width > 0 and height > 0:
        return min(width, height)
    return None


def check_resolution_claim(params: dict, models: frozenset) -> Optional[Finding]:
    prompt = params.get("prompt")
    if not isinstance(prompt, str):
        return None
    claim = _claimed_short_side(prompt)
    if claim is None:
        return None
    actual = _actual_short_side(params)
    if actual is None or actual >= claim[1]:
        return None
    return Finding(
        "resolution_claim",
        f'prompt says "{claim[0]}" but the call produces a {actual}px short side. '
        f"Raise resolution, or drop the claim from the prompt.",
    )


CHECKS = (check_resolution_claim,)


def lint(tool_input, models: frozenset) -> list:
    params = tool_input.get("params") if isinstance(tool_input, dict) else None
    if not isinstance(params, dict):
        return []
    findings = []
    for check in CHECKS:
        finding = check(params, models)
        if finding is not None:
            findings.append(finding)
    return findings


def render(findings: list) -> str:
    plural = "" if len(findings) == 1 else "s"
    lines = [f"higgsfield-lint: {len(findings)} finding{plural}"]
    lines += [f"- {f.check_id}: {f.message}" for f in findings]
    return "\n".join(lines)


def load_models() -> frozenset:
    try:
        return frozenset(json.loads(DATA_FILE.read_text()))
    except Exception:
        return frozenset()


def main() -> int:
    try:
        payload = json.load(sys.stdin)
        tool_input = payload.get("tool_input") or {}
        findings = lint(tool_input, load_models())
        if findings:
            json.dump(
                {
                    "hookSpecificOutput": {
                        "hookEventName": "PreToolUse",
                        "additionalContext": render(findings),
                    }
                },
                sys.stdout,
            )
    except Exception:
        pass  # SPEC 7: the hook must be invisible when it fails.
    return 0


if __name__ == "__main__":
    sys.exit(main())
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `python3 -m pytest tests/test_lint.py -v`
Expected: PASS, 13 passed

- [ ] **Step 6: Commit**

```bash
git add SPEC.md src/higgsfield_lint.py tests/ docs/
git commit -m "feat: hook scaffold and resolution-claim check"
```

---

### Task 2: Prompt versus negative_prompt conflict check

**Files:**
- Modify: `src/higgsfield_lint.py`
- Modify: `tests/test_lint.py`

**Interfaces:**
- Consumes: `Finding`, `_has_term`, `CHECKS` from Task 1.
- Produces: `NEGATION_RE: re.Pattern`; `strip_negations(text: str) -> str`; `check_prompt_negative_conflict(params: dict, models: frozenset) -> Finding | None`.

The subtlety that makes this check usable: a term negated in the positive prompt (`"no text"`) and also present in `negative_prompt` is agreement, not conflict. Negated spans are stripped from the positive prompt before comparison. Without that, the real `kling2_6` fixture produces a false finding on `"text"`.

- [ ] **Step 1: Write the failing tests**

Append to `tests/test_lint.py`:

```python
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `python3 -m pytest tests/test_lint.py -v -k "conflict or negations"`
Expected: FAIL with `AttributeError: module 'higgsfield_lint' has no attribute 'strip_negations'`

- [ ] **Step 3: Write the implementation**

Insert into `src/higgsfield_lint.py` above `CHECKS`:

```python
# Directive phrases worth comparing across the two prompt fields. A curated list,
# because ordinary words legitimately appear in both and would drown the signal.
CONFLICT_PHRASES = (
    "slow motion", "slow-mo", "motion blur", "static", "blurred", "blurry",
    "sharp", "text", "close-up", "wide shot", "zoom", "pan", "handheld", "grain",
)

# "no text", "without a logo", "avoid harsh shadows" - up to three trailing words.
NEGATION_RE = re.compile(r"\b(?:no|without|avoid|not)\s+(?:[a-z-]+\s+){0,2}[a-z-]+", re.IGNORECASE)


def strip_negations(text: str) -> str:
    """Remove negated spans so a negated term does not read as a positive request."""
    return NEGATION_RE.sub(" ", text)


def check_prompt_negative_conflict(params: dict, models: frozenset) -> Optional[Finding]:
    prompt = params.get("prompt")
    negative = params.get("negative_prompt")
    if not isinstance(prompt, str) or not isinstance(negative, str):
        return None
    positive = strip_negations(prompt).lower()
    negative_low = negative.lower()
    hits = [p for p in CONFLICT_PHRASES if _has_term(positive, p) and _has_term(negative_low, p)]
    if not hits:
        return None
    listed = ", ".join(f'"{h}"' for h in hits)
    return Finding(
        "prompt_negative_conflict",
        f"{listed} appears in both prompt and negative_prompt. Keep each term in one.",
    )
```

Then extend the tuple:

```python
CHECKS = (check_resolution_claim, check_prompt_negative_conflict)
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `python3 -m pytest tests/test_lint.py -v`
Expected: PASS, 17 passed

- [ ] **Step 5: Commit**

```bash
git add src/higgsfield_lint.py tests/test_lint.py
git commit -m "feat: prompt versus negative_prompt conflict check"
```

---

### Task 3: Negation placement check

**Files:**
- Modify: `src/higgsfield_lint.py`
- Create: `data/negative_prompt_models.json`
- Modify: `tests/test_lint.py`

**Interfaces:**
- Consumes: `Finding`, `NEGATION_RE`, `CHECKS` from Tasks 1 and 2.
- Produces: `check_negation_placement(params: dict, models: frozenset) -> Finding | None`.

Fires only when the model is *known* to accept `negative_prompt`: either the field is already present in this call, or the model appears in `data/negative_prompt_models.json`. The catalog does not publish negative-prompt support, so inferring it would produce confident advice about a field the model silently ignores (SPEC §5.3).

- [ ] **Step 1: Create the data file**

`data/negative_prompt_models.json`:

```json
["kling2_6"]
```

Seeded from evidence: `kling2_6` is the only model observed accepting `negative_prompt` in the generation history. Add entries only when a real call proves support.

- [ ] **Step 2: Write the failing tests**

Append to `tests/test_lint.py`:

```python
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
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `python3 -m pytest tests/test_lint.py -v -k "negation_placement or load_models"`
Expected: FAIL with `AttributeError: module 'higgsfield_lint' has no attribute 'check_negation_placement'`

- [ ] **Step 4: Write the implementation**

Insert into `src/higgsfield_lint.py` above `CHECKS`:

```python
def check_negation_placement(params: dict, models: frozenset) -> Optional[Finding]:
    prompt = params.get("prompt")
    if not isinstance(prompt, str):
        return None
    model = params.get("model")
    supports = isinstance(params.get("negative_prompt"), str) or (
        isinstance(model, str) and model in models
    )
    if not supports:
        return None
    negations = [m.group(0).strip() for m in NEGATION_RE.finditer(prompt)]
    if not negations:
        return None
    shown = ", ".join(f'"{n}"' for n in negations[:3])
    more = f" and {len(negations) - 3} more" if len(negations) > 3 else ""
    return Finding(
        "negation_placement",
        f"{shown}{more} sit in the positive prompt while this model accepts "
        f"negative_prompt. Move them there; models follow negation poorly in the prompt.",
    )
```

Then extend the tuple:

```python
CHECKS = (check_resolution_claim, check_prompt_negative_conflict, check_negation_placement)
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `python3 -m pytest tests/test_lint.py -v`
Expected: PASS, 22 passed

- [ ] **Step 6: Verify the clean fixture still produces zero findings**

Run: `python3 -c "import sys,json; sys.path.insert(0,'src'); import higgsfield_lint as h; print(h.lint(json.load(open('tests/fixtures/seedance_tree_clean.json')), h.load_models()))"`
Expected: `[]`

This is the check that matters most. A finding here means a false positive on a call that succeeded (SPEC §9).

- [ ] **Step 7: Commit**

```bash
git add src/higgsfield_lint.py data/negative_prompt_models.json tests/test_lint.py
git commit -m "feat: negation placement check"
```

---

### Task 4: Register the hook and verify end to end

**Files:**
- Create or modify: `.claude/settings.json`

**Interfaces:**
- Consumes: `src/higgsfield_lint.py` from Tasks 1 to 3.
- Produces: a live `PreToolUse` hook.

- [ ] **Step 1: Register the hook**

Add to `.claude/settings.json` in the project directory, merging with any existing keys rather than replacing the file:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "mcp__.*__generate_(image|video|audio)(_batch)?$",
        "hooks": [
          {
            "type": "command",
            "command": "python3 /Users/elibelilty/Documents/GitHub/higgsfield-preprocessor/src/higgsfield_lint.py"
          }
        ]
      }
    ]
  }
}
```

- [ ] **Step 2: Verify the matcher against real tool names**

Run:
```bash
python3 -c "
import re
m = re.compile(r'mcp__.*__generate_(image|video|audio)(_batch)?\$')
for name, want in [
    ('mcp__ada4cfac-9b58-4bfb-9631-4a588b7a2e4b__generate_video', True),
    ('mcp__ada4cfac-9b58-4bfb-9631-4a588b7a2e4b__generate_image_batch', True),
    ('mcp__ada4cfac-9b58-4bfb-9631-4a588b7a2e4b__generate_3d', False),
    ('mcp__ada4cfac-9b58-4bfb-9631-4a588b7a2e4b__show_generations', False),
]:
    got = bool(m.match(name))
    print(('OK  ' if got == want else 'FAIL'), name, got)
"
```
Expected: four `OK` lines.

- [ ] **Step 3: Verify the hook end to end from the shell**

Run:
```bash
python3 -c "
import json,subprocess,sys
p=json.dumps({'tool_name':'mcp__x__generate_video','tool_input':json.load(open('tests/fixtures/kling2_6_4k_slowmo.json'))})
r=subprocess.run([sys.executable,'src/higgsfield_lint.py'],input=p,capture_output=True,text=True)
print('exit',r.returncode); print(json.loads(r.stdout)['hookSpecificOutput']['additionalContext'])
"
```
Expected: `exit 0`, then three finding lines (`resolution_claim`, `prompt_negative_conflict`, `negation_placement`).

- [ ] **Step 4: Confirm the hook is loaded in a fresh Claude Code session**

Restart Claude Code, then run `/hooks` from an interactive terminal and confirm the `PreToolUse` entry appears. This cannot be verified from inside a non-interactive session.

- [ ] **Step 5: Commit**

```bash
git add .claude/settings.json
git commit -m "feat: register higgsfield-lint PreToolUse hook"
```

---

### Task 5: Credit expiry reminder

**Files:**
- Create: `docs/credit-expiry-task.md`

**Interfaces:**
- Consumes: nothing from earlier tasks. Independent of the hook, because hooks cannot call MCP tools (SPEC §8).
- Produces: a scheduled Claude Code task.

On the SPEC §1 numbers this is worth more than the three checks combined: 6,910 credits were forfeited to expiry over six months, against ~732 ever spent generating.

- [ ] **Step 1: Record the task prompt**

`docs/credit-expiry-task.md`:

```markdown
# Credit expiry reminder

Weekly scheduled task. Renewal falls between the 5th and 7th of each month, per the
transaction log (`Subscription Credits Reset` rows).

Prompt:

> Call the Higgsfield `balance` tool. Report the credit total and how many days remain
> until the next monthly reset, assuming reset on the 6th. If more than 400 credits
> remain and fewer than 10 days are left, say so plainly and note that unused credits
> are wiped at reset rather than carried over.
```

- [ ] **Step 2: Create the scheduled task**

Use the `schedule` skill to create a weekly recurring task with the prompt above, running Mondays at 09:00 local time.

- [ ] **Step 3: Verify it appears**

Run: `/schedule list` from an interactive terminal, or the equivalent listing command, and confirm the task is registered with a next-run timestamp.

- [ ] **Step 4: Commit**

```bash
git add docs/credit-expiry-task.md
git commit -m "docs: credit expiry reminder task"
```

---

## Notes for the executor

- `git init` first if needed. This directory is currently untracked inside the parent `Documents/GitHub` repository, so it has no history of its own.
- Run `python3 -m pytest tests/ -v` after every task. The suite is fast and the clean-fixture assertion is the one that catches real regressions.
- If a check starts firing on `seedance_tree_clean.json`, stop and fix the check. Do not adjust the fixture.
