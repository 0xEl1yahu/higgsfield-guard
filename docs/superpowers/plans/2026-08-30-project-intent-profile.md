# Project Intent Profile Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the lint compare each call against an approved project profile, so a forgotten directive, a stray palette colour, or a wrong aspect ratio is caught before the render is paid for.

**Architecture:** A `data/project-profile.json` file, derived once from generation history and approved by a human, is loaded at hook start and passed to every check inside a context object. Checks perform fixed comparisons against it. No inference happens at runtime. The profile is optional: absent, no inter-call check fires and behaviour is identical to today.

**Tech Stack:** Python 3 standard library only. pytest. Extends the existing `src/higgsfield_lint.py`.

**Spec:** `SPEC.md` (amended by Task 4 of this plan)

**Predecessor:** `docs/superpowers/plans/2026-08-30-higgsfield-lint.md`, complete at commit `abd1238`, 58 tests passing.

## Why this exists

The three existing checks are **intra-call**: they compare a prompt against its own parameters. Analysis of the real corpus found they are structurally inert on image generations, which are roughly 88% of activity (~67 image events against 9 video). Meanwhile the corpus carries a declared, repeated intent that is retyped by hand on every call: `no text / no lettering / no words` in 100% of image prompts, `16:9` in every call of both types, `#ff2d7a` in every image call. Nothing currently notices the day one of those is forgotten.

## Global Constraints

Every constraint from the predecessor plan still binds, verbatim:

- **Standard library only at runtime.** A failed import breaks every Higgsfield generation call.
- **Never block.** Never emit `permissionDecision: "deny"`; never exit non-zero.
- **Fail silent.** Any exception, malformed stdin, or missing field exits 0 with no stdout and no stderr.
- **No network, no MCP calls, no filesystem writes.** Permitted reads are `data/negative_prompt_models.json` and now `data/project-profile.json`.
- **Precision is the product, in both directions.** A check that fires on a known-good call is a bug; a check that silently drops what it was built for is worse.

New constraints specific to this plan:

- **The profile is optional.** Absent, unreadable, or malformed → every profile check returns `None` and the tool behaves exactly as at `abd1238`. No regression is acceptable.
- **No runtime inference.** Checks compare against stored values. They never derive intent from the call.
- **All 58 existing tests must keep passing**, and both existing fixtures must keep their behaviour: `kling2_6_4k_slowmo.json` → 3 findings, `seedance_tree_clean.json` → `[]`.

---

## File Structure

| File | Responsibility |
|---|---|
| `src/higgsfield_lint.py` | Extended: context object, profile loader, three new checks. |
| `data/project-profile.json` | The approved profile. Data, edited by a human, never written by code. |
| `tests/fixtures/recraft_banner_clean.json` | Real captured image call. Negative fixture. |
| `tests/fixtures/nano_banana_inline_negative.json` | Real captured image call carrying an inline `Negative prompt:` block. |
| `tests/test_lint.py` | Extended. |
| `SPEC.md` | Amended in Task 4. |

---

### Task 1: Context object, profile loader, and image fixtures

**Files:**
- Modify: `src/higgsfield_lint.py`
- Create: `tests/fixtures/recraft_banner_clean.json`
- Create: `tests/fixtures/nano_banana_inline_negative.json`
- Modify: `tests/test_lint.py`

**Interfaces:**
- Consumes: `Finding`, `_has_term`, `CHECKS`, `lint`, `load_models` from the predecessor plan.
- Produces: `class Ctx(NamedTuple)` with fields `models: frozenset`, `profile: Optional[dict]`, `tool_name: str`; `load_profile() -> Optional[dict]`; `PROFILE_FILE: Path`. Every check's signature becomes `check_x(params: dict, ctx: Ctx) -> Optional[Finding]`.

**Read `src/higgsfield_lint.py` in full before editing.** It has been through a fix wave and a patch round, so its current text is the authority — the snippets below give the target shape, not a literal diff.

This task adds no new checks. It is the enabling refactor plus the fixtures that later tasks assert against. The final reviewer of the predecessor plan predicted this exact change: "when a fourth check needs [more context], widen the signature to take a small context object rather than threading another positional argument."

- [ ] **Step 1: Write the two image fixtures**

`tests/fixtures/recraft_banner_clean.json` — a real `recraft_v4_1` call:

```json
{
  "params": {
    "model": "recraft_v4_1",
    "prompt": "Wide banner illustration, bold flat vector style, on a solid bright sunny yellow background. A very tiny cute cartoon seedling sprout character with cool black sunglasses and a big happy smile, sitting low near the bottom right of the frame. Thick bold black outlines, flat colour, no gradients, no shading, playful cheerful character design. The left two thirds of the frame is empty flat yellow. No text, no lettering, no words.",
    "aspect_ratio": "16:9",
    "width": 1344,
    "height": 756,
    "resolution": "2k",
    "model_type": "vector",
    "batch_size": 1,
    "colors": ["#000000", "#16c1df", "#ff2d7a", "#ff4d00"],
    "background_color": "#fee83f"
  }
}
```

`tests/fixtures/nano_banana_inline_negative.json` — a real `nano_banana_2` call whose prompt carries an inline negative section:

```json
{
  "params": {
    "model": "nano_banana_2",
    "prompt": "A hilarious 8-bit pixel-art scene of a grumpy Korat cat sitting against a romantic Greek seaside background at sunset. Style: crisp handcrafted 8-bit pixel art, expressive sprite design, nostalgic 1990s adventure-game aesthetic. Wide cinematic composition, 16:9. Ensure the dialogue text is perfectly legible and spelled exactly as written.\nNegative prompt: photorealism, 3D render, anime, Disney style, smooth vector art, cute smiling cat, blurry pixel art, illegible text, misspelled words, watermark, logo.",
    "aspect_ratio": "16:9",
    "width": 1376,
    "height": 768,
    "resolution": "1k",
    "batch_size": 1
  }
}
```

- [ ] **Step 2: Write the failing tests**

Append to `tests/test_lint.py`:

```python
def test_ctx_carries_models_profile_and_tool_name():
    ctx = hl.Ctx(models=frozenset({"kling2_6"}), profile={"aspect_ratio": "16:9"},
                 tool_name="mcp__x__generate_image")
    assert ctx.models == frozenset({"kling2_6"})
    assert ctx.profile["aspect_ratio"] == "16:9"
    assert ctx.tool_name == "mcp__x__generate_image"


def test_load_profile_returns_none_when_file_absent(monkeypatch, tmp_path):
    monkeypatch.setattr(hl, "PROFILE_FILE", tmp_path / "nope.json")
    assert hl.load_profile() is None


def test_load_profile_returns_none_on_malformed_json(monkeypatch, tmp_path):
    bad = tmp_path / "bad.json"
    bad.write_text("{not json")
    monkeypatch.setattr(hl, "PROFILE_FILE", bad)
    assert hl.load_profile() is None


def test_load_profile_returns_none_when_top_level_is_not_a_dict(monkeypatch, tmp_path):
    arr = tmp_path / "arr.json"
    arr.write_text('["a", "b"]')
    monkeypatch.setattr(hl, "PROFILE_FILE", arr)
    assert hl.load_profile() is None


def test_load_profile_reads_a_valid_profile(monkeypatch, tmp_path):
    good = tmp_path / "p.json"
    good.write_text('{"aspect_ratio": "16:9"}')
    monkeypatch.setattr(hl, "PROFILE_FILE", good)
    assert hl.load_profile() == {"aspect_ratio": "16:9"}


def test_real_image_fixtures_produce_no_findings():
    ctx = hl.Ctx(models=hl.load_models(), profile=None, tool_name="mcp__x__generate_image")
    for name in ("recraft_banner_clean", "nano_banana_inline_negative"):
        assert hl.lint(load(name), ctx) == [], f"{name} must stay silent"


def test_existing_fixtures_unchanged_under_ctx():
    ctx = hl.Ctx(models=hl.load_models(), profile=None, tool_name="mcp__x__generate_video")
    assert len(hl.lint(load("kling2_6_4k_slowmo"), ctx)) == 3
    assert hl.lint(load("seedance_tree_clean"), ctx) == []
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `python3 -m pytest tests/test_lint.py -v -k "ctx or profile or image_fixtures"`
Expected: FAIL with `AttributeError: module 'higgsfield_lint' has no attribute 'Ctx'`

- [ ] **Step 4: Implement**

Add near `Finding`:

```python
class Ctx(NamedTuple):
    """Everything a check may consult beyond the call's own params."""
    models: frozenset          # models known to accept negative_prompt
    profile: Optional[dict]    # approved project profile, or None
    tool_name: str             # e.g. "mcp__x__generate_image"; "" when unknown
```

Add beside `DATA_FILE` and `load_models`:

```python
PROFILE_FILE = Path(__file__).resolve().parent.parent / "data" / "project-profile.json"


def load_profile() -> Optional[dict]:
    """The approved project profile, or None when absent or unusable.

    None disables every profile check. That is the designed default: a project
    without an approved profile gets exactly the pre-profile behaviour.
    """
    try:
        data = json.loads(PROFILE_FILE.read_text())
    except Exception:
        return None
    return data if isinstance(data, dict) else None
```

Then change **every** existing check's second parameter from `models` to `ctx`, and replace each internal use of `models` with `ctx.models`. Update `lint()` to take `ctx` and pass it through, and update `main()` to build the context:

```python
ctx = Ctx(models=load_models(), profile=load_profile(),
          tool_name=payload.get("tool_name") or "")
findings = lint(tool_input, ctx)
```

Update every existing test that calls a check or `lint()` with a bare frozenset to pass a `Ctx` instead. Do not change any check's behaviour in this task.

- [ ] **Step 5: Run the full suite**

Run: `python3 -m pytest tests/ -v`
Expected: PASS, 65 passed (58 existing, migrated to `Ctx`, plus 7 new)

- [ ] **Step 6: Commit**

```bash
git add src/higgsfield_lint.py tests/
git commit -m "refactor: context object and profile loader, plus image fixtures"
```

---

### Task 2: `profile_aspect_drift` and `profile_palette_drift`

**Files:**
- Modify: `src/higgsfield_lint.py`
- Modify: `tests/test_lint.py`

**Interfaces:**
- Consumes: `Ctx`, `Finding`, `CHECKS` from Task 1.
- Produces: `check_profile_aspect_drift(params, ctx)`, `check_profile_palette_drift(params, ctx)`.

Both are exact comparisons against stored values, which is why they share a task. Neither infers anything.

- [ ] **Step 1: Write the failing tests**

```python
PROFILE = {
    "aspect_ratio": "16:9",
    "forbid_in_output": ["text", "lettering", "words", "numbers", "signage", "logos"],
    "palette": {
        "core": ["#ff2d7a", "#c6ff3d", "#0b0a08"],
        "secondary": ["#00e5ff", "#ff4d00", "#7b5cff", "#16c1df", "#000000"],
        "backgrounds": ["#0b0a08", "#fee83f"],
    },
}


def pctx(profile=PROFILE):
    return hl.Ctx(models=frozenset({"kling2_6"}), profile=profile, tool_name="mcp__x__generate_image")


def test_aspect_drift_fires_on_mismatch():
    f = hl.check_profile_aspect_drift({"aspect_ratio": "9:16"}, pctx())
    assert f is not None and f.check_id == "profile_aspect_drift"
    assert "9:16" in f.message and "16:9" in f.message


def test_aspect_drift_silent_when_matching():
    assert hl.check_profile_aspect_drift({"aspect_ratio": "16:9"}, pctx()) is None


def test_aspect_drift_silent_when_call_omits_aspect_ratio():
    # Absent means the model default applies, which we cannot know reliably.
    assert hl.check_profile_aspect_drift({"prompt": "a shot"}, pctx()) is None


def test_aspect_drift_silent_without_profile():
    assert hl.check_profile_aspect_drift({"aspect_ratio": "9:16"}, pctx(None)) is None


def test_palette_drift_fires_on_stray_colour():
    f = hl.check_profile_palette_drift({"colors": ["#ff2d7a", "#123456"]}, pctx())
    assert f is not None and f.check_id == "profile_palette_drift"
    assert "#123456" in f.message and "#ff2d7a" not in f.message


def test_palette_drift_checks_background_colour_too():
    f = hl.check_profile_palette_drift({"colors": ["#ff2d7a"], "background_color": "#abcdef"}, pctx())
    assert f is not None and "#abcdef" in f.message


def test_palette_drift_is_case_insensitive():
    assert hl.check_profile_palette_drift({"colors": ["#FF2D7A", "#C6FF3D"]}, pctx()) is None


def test_palette_drift_silent_on_the_real_recraft_fixture():
    params = load("recraft_banner_clean")["params"]
    assert hl.check_profile_palette_drift(params, pctx()) is None


def test_palette_drift_silent_without_profile():
    assert hl.check_profile_palette_drift({"colors": ["#123456"]}, pctx(None)) is None
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `python3 -m pytest tests/test_lint.py -v -k "aspect_drift or palette_drift"`
Expected: FAIL with `AttributeError: module 'higgsfield_lint' has no attribute 'check_profile_aspect_drift'`

- [ ] **Step 3: Implement**

```python
def check_profile_aspect_drift(params: dict, ctx: Ctx) -> Optional[Finding]:
    if not ctx.profile:
        return None
    want = ctx.profile.get("aspect_ratio")
    got = params.get("aspect_ratio")
    # An absent aspect_ratio means the model default applies, and we do not
    # reliably know it per model. Silence is correct there.
    if not isinstance(want, str) or not isinstance(got, str) or got == want:
        return None
    return Finding(
        "profile_aspect_drift",
        f'aspect_ratio is "{got}" but this project uses "{want}". '
        f"Change it, or update the profile if the project has moved on.",
    )


def _profile_palette(profile: dict) -> set:
    """Every colour in the profile palette, lowercased. Empty set if unusable."""
    palette = profile.get("palette")
    if not isinstance(palette, dict):
        return set()
    return {
        c.lower()
        for group in palette.values() if isinstance(group, list)
        for c in group if isinstance(c, str)
    }


def check_profile_palette_drift(params: dict, ctx: Ctx) -> Optional[Finding]:
    if not ctx.profile:
        return None
    allowed = _profile_palette(ctx.profile)
    if not allowed:
        return None
    used = [c for c in params.get("colors", []) if isinstance(c, str)]
    background = params.get("background_color")
    if isinstance(background, str):
        used.append(background)
    stray = [c for c in used if c.lower() not in allowed]
    if not stray:
        return None
    shown = ", ".join(stray[:4])
    more = f" and {len(stray) - 4} more" if len(stray) > 4 else ""
    verb = "is" if len(stray) == 1 else "are"
    return Finding(
        "profile_palette_drift",
        f"{shown}{more} {verb} outside the project palette. "
        f"Use a profile colour, or add it to the profile.",
    )
```

Extend `CHECKS` with both, then verify neither fires on the existing fixtures with a profile loaded.

- [ ] **Step 4: Run the full suite**

Run: `python3 -m pytest tests/ -v`
Expected: PASS, 74 passed

- [ ] **Step 5: Commit**

```bash
git add src/higgsfield_lint.py tests/test_lint.py
git commit -m "feat: profile aspect and palette drift checks"
```

---

### Task 3: `profile_missing_directive`

**Files:**
- Modify: `src/higgsfield_lint.py`
- Modify: `tests/test_lint.py`

**Interfaces:**
- Consumes: `Ctx`, `Finding`, `NEGATION_RE`, `_has_term`, `CHECKS`. Also the test helpers `PROFILE` and `pctx()` added to `tests/test_lint.py` by Task 2 — reuse them, do not redefine.
- Produces: `check_profile_missing_directive(params, ctx)`.

This is the highest-value check in the plan and the one most able to become noise, so its firing rule is deliberately narrow.

**A profile describes one project, not one account.** The captured corpus holds at least
two distinct bodies of work: the Recraft brand banners (flat vector, no text, fixed palette)
and a one-off `nano_banana_2` birthday card that deliberately asks for legible text. A single
profile spanning both is incoherent. This check firing on work from another project is correct
behaviour, and is the signal to scope the profile — one per project directory, since the hook
is registered per project in `.claude/settings.json`.

**The firing rule, and why.** The profile lists several forbidden terms (`text`, `lettering`, `words`, `numbers`, `signage`, `logos`). Real prompts negate some but never all — `"No text, no lettering, no words"` covers three of six. Firing when *any* term is uncovered would fire on every historical call, which is useless. So it fires only when **none** of the forbidden terms appear in any negated span: the case where the directive was forgotten entirely. That is precisely the failure being targeted.

- [ ] **Step 1: Write the failing tests**

```python
def test_missing_directive_fires_when_prompt_has_no_exclusion_at_all():
    f = hl.check_profile_missing_directive({"prompt": "a cheerful tree on a yellow background"}, pctx())
    assert f is not None and f.check_id == "profile_missing_directive"
    assert "text" in f.message


def test_missing_directive_silent_when_prompt_covers_one_term():
    # "No text" covers the family; real prompts never enumerate all six.
    assert hl.check_profile_missing_directive(
        {"prompt": "a cheerful tree, flat vector. No text."}, pctx()) is None


def test_missing_directive_silent_on_in_project_fixtures():
    # Both negate the family ("No text, no lettering, no words"), so both are covered.
    for name in ("recraft_banner_clean", "seedance_tree_clean"):
        params = load(name)["params"]
        assert hl.check_profile_missing_directive(params, pctx()) is None, name


def test_missing_directive_fires_on_a_call_from_a_different_project():
    # The nano_banana fixture is a birthday card that DELIBERATELY wants text
    # ("Ensure the dialogue text is perfectly legible") and negates nothing.
    # Firing is correct: it departs from the brand-banner profile, and it is the
    # signal that this profile does not describe that piece of work.
    params = load("nano_banana_inline_negative")["params"]
    f = hl.check_profile_missing_directive(params, pctx())
    assert f is not None and f.check_id == "profile_missing_directive"


def test_missing_directive_silent_without_profile():
    assert hl.check_profile_missing_directive({"prompt": "a tree"}, pctx(None)) is None


def test_missing_directive_silent_when_profile_has_no_forbid_list():
    assert hl.check_profile_missing_directive({"prompt": "a tree"}, pctx({"aspect_ratio": "16:9"})) is None


def test_missing_directive_silent_when_prompt_absent():
    assert hl.check_profile_missing_directive({"model": "recraft_v4_1"}, pctx()) is None
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `python3 -m pytest tests/test_lint.py -v -k "missing_directive"`
Expected: FAIL with `AttributeError: module 'higgsfield_lint' has no attribute 'check_profile_missing_directive'`

- [ ] **Step 3: Implement**

```python
def check_profile_missing_directive(params: dict, ctx: Ctx) -> Optional[Finding]:
    if not ctx.profile:
        return None
    forbidden = ctx.profile.get("forbid_in_output")
    prompt = params.get("prompt")
    if not isinstance(forbidden, list) or not isinstance(prompt, str):
        return None
    terms = [t.lower() for t in forbidden if isinstance(t, str) and t]
    if not terms:
        return None
    # Everything the prompt already negates, as one searchable blob.
    negated = " ".join(m.group(0).lower() for m in NEGATION_RE.finditer(prompt))
    # Partial coverage is normal: real prompts say "no text, no lettering" and
    # never enumerate the whole list. Only a prompt with NO exclusion at all
    # has forgotten the directive.
    if any(_has_term(negated, t) for t in terms):
        return None
    shown = ", ".join(terms[:3])
    return Finding(
        "profile_missing_directive",
        f"this project excludes {shown} from output, but the prompt does not say so. "
        f'Add an exclusion such as "no {terms[0]}", or update the profile.',
    )
```

Extend `CHECKS`.

- [ ] **Step 4: Run the full suite**

Run: `python3 -m pytest tests/ -v`
Expected: PASS, 81 passed

- [ ] **Step 5: Verify zero false positives across every real fixture with the profile active**

Run:
```bash
python3 -c "
import sys, json; sys.path.insert(0,'src'); import higgsfield_lint as h
P={'aspect_ratio':'16:9',
 'forbid_in_output':['text','lettering','words','numbers','signage','logos'],
 'palette':{'core':['#ff2d7a','#c6ff3d','#0b0a08'],
            'secondary':['#00e5ff','#ff4d00','#7b5cff','#16c1df','#000000'],
            'backgrounds':['#0b0a08','#fee83f']}}
ctx=h.Ctx(models=h.load_models(), profile=P, tool_name='mcp__x__generate_image')
for n in ['recraft_banner_clean','nano_banana_inline_negative','seedance_tree_clean']:
    fs=h.lint(json.load(open(f'tests/fixtures/{n}.json')), ctx)
    print(n, '->', [f.check_id for f in fs])
"
```
Expected: `recraft_banner_clean` and `seedance_tree_clean` print `-> []`. `nano_banana_inline_negative` prints `['profile_missing_directive']`, which is correct — different project, negates nothing. For the two in-project fixtures a firing check is a false positive, and must be fixed in the check, never in the fixture.

- [ ] **Step 6: Commit**

```bash
git add src/higgsfield_lint.py tests/test_lint.py
git commit -m "feat: profile missing-directive check"
```

---

### Task 4: Ship the profile and amend the spec

**Files:**
- Create: `data/project-profile.json`
- Modify: `SPEC.md`

**Interfaces:**
- Consumes: `load_profile()` from Task 1 and all three checks.
- Produces: the live profile, and spec sections documenting it.

- [ ] **Step 1: Write the profile**

`data/project-profile.json`, derived from the generation corpus and approved:

```json
{
  "aspect_ratio": "16:9",
  "forbid_in_output": ["text", "lettering", "words", "numbers", "signage", "logos"],
  "palette": {
    "core": ["#ff2d7a", "#c6ff3d", "#0b0a08"],
    "secondary": ["#00e5ff", "#ff4d00", "#7b5cff", "#16c1df", "#000000", "#f6f3ee", "#fbf8ef"],
    "backgrounds": ["#0b0a08", "#fee83f"]
  }
}
```

`style` and `composition` are deliberately omitted: no check consumes them, and a field nothing reads is a field that goes stale silently.

- [ ] **Step 2: Verify the shipped profile against every real fixture**

Run the Task 3 Step 5 command again, but loading the real file:

```bash
python3 -c "
import sys, json; sys.path.insert(0,'src'); import higgsfield_lint as h
ctx=h.Ctx(models=h.load_models(), profile=h.load_profile(), tool_name='mcp__x__generate_image')
assert ctx.profile is not None, 'profile did not load'
for n in ['recraft_banner_clean','seedance_tree_clean','nano_banana_inline_negative']:
    fs=h.lint(json.load(open(f'tests/fixtures/{n}.json')), ctx)
    print(n, '->', [f.check_id for f in fs])
print('kling2_6_4k_slowmo ->', [f.check_id for f in h.lint(json.load(open('tests/fixtures/kling2_6_4k_slowmo.json')), ctx)])
"
```
Expected: `recraft_banner_clean` and `seedance_tree_clean` print `-> []`; `nano_banana_inline_negative` prints `['profile_missing_directive']`, correct because that call belongs to a different project and negates nothing; the kling line prints its three original findings and no profile findings. If a profile check fires on an **in-project** call, widen the profile rather than weakening the check — the profile is the thing that is supposed to describe reality.

- [ ] **Step 3: Amend SPEC.md**

Add a new section documenting the profile:

```markdown
## 13. Project intent profile

The checks in §5 are intra-call: they compare a prompt against its own parameters. They
are structurally inert on image generations, which are roughly 88% of real activity.

`data/project-profile.json` holds a project's declared intent, derived once from
generation history and approved by a human. It is loaded at hook start and passed to
every check. Three checks consume it:

| Check | Fires when |
|---|---|
| `profile_aspect_drift` | the call sets an `aspect_ratio` differing from the profile's |
| `profile_palette_drift` | `colors[]` or `background_color` carries a hex outside the profile palette |
| `profile_missing_directive` | the profile forbids terms in output and the prompt negates none of them |

Three properties are load-bearing:

- **Optional.** No profile file means no profile check fires, and behaviour is identical
  to the pre-profile tool. A project without an approved profile loses nothing.
- **No runtime inference.** Checks compare against stored values. Deriving the profile is
  a human-gated, offline act; the hook never guesses what a call was going for.
- **One profile describes one project, not one account.** The hook is registered per
  project in `.claude/settings.json` and the profile lives beside it. Pointing a brand
  profile at unrelated work makes it fire correctly and uselessly.
- **Partial coverage is normal.** `profile_missing_directive` fires only when a prompt
  excludes *nothing* from the forbidden list. Real prompts say "no text, no lettering"
  and never enumerate every term, so firing on partial coverage would fire on everything.

The profile is edited by hand and never written by code. When a check fires on a call that
was correct, the profile is what changed under you: widen the profile, do not weaken the
check.
```

Also amend §11 (Non-goals). The existing "judging whether a prompt is good" non-goal still
holds and must stay; add one line clarifying that comparing a call against an approved
profile is not that, because the standard is stated by a human rather than inferred per call.

- [ ] **Step 4: Run the full suite one last time**

Run: `python3 -m pytest tests/ -v`
Expected: PASS, 81 passed, output pristine.

- [ ] **Step 5: Commit**

```bash
git add data/project-profile.json SPEC.md
git commit -m "feat: ship the derived project profile and document it"
```

---

## Notes for the executor

- Run `python3 -m pytest tests/ -v` after every task.
- The four real fixtures are the regression corpus. If a profile check fires on one of them, the check is wrong or the profile is too narrow. Never edit a fixture to make a check pass.
- The predecessor plan shipped two bugs that its own suite passed, both because no test covered the class. When you add a check, add the test that would catch it silencing or over-firing, not just the happy path.
