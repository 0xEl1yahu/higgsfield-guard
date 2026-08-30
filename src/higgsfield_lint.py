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


def _find_original_case(text: str, term: str) -> Optional[str]:
    """Like _has_term, but returns the matched substring in its original casing so a
    finding can quote the prompt as written (e.g. "4K") instead of the lexicon key."""
    match = re.search(rf"(?<![a-z0-9]){re.escape(term)}(?![a-z0-9])", text, re.IGNORECASE)
    return match.group(0) if match else None


def _claimed_short_side(prompt: str):
    """Highest resolution claim present in the prompt, as (original_text, pixels)."""
    best = None
    for term, px in RESOLUTION_CLAIMS.items():
        original = _find_original_case(prompt, term)
        if original is not None and (best is None or px > best[1]):
            best = (original, px)
    return best


# A dimension only counts when it looks like a real render size. `isinstance(True, int)`
# is True in Python, so booleans are excluded explicitly. Anything below 128px is treated
# as an aspect-ratio hint (e.g. width=16, height=9) rather than an actual pixel count, so
# it is ignored instead of producing a nonsensical "9px short side" finding.
MIN_SANE_DIMENSION = 128


def _is_valid_dimension(value) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= MIN_SANE_DIMENSION


class _Actual(NamedTuple):
    short_side: int
    width: Optional[int] = None
    height: Optional[int] = None


def _actual_short_side(params: dict) -> Optional[_Actual]:
    res = params.get("resolution")
    if isinstance(res, str) and res.lower() in RESOLUTION_PARAM:
        return _Actual(RESOLUTION_PARAM[res.lower()])
    width, height = params.get("width"), params.get("height")
    if _is_valid_dimension(width) and _is_valid_dimension(height):
        return _Actual(min(width, height), width, height)
    return None


def check_resolution_claim(params: dict, models: frozenset) -> Optional[Finding]:
    prompt = params.get("prompt")
    if not isinstance(prompt, str):
        return None
    claim = _claimed_short_side(prompt)
    if claim is None:
        return None
    claim_text, claim_px = claim
    actual = _actual_short_side(params)
    if actual is None or actual.short_side >= claim_px:
        return None
    if actual.width is not None:
        source = f"width {actual.width} x height {actual.height} gives a {actual.short_side}px short side"
    else:
        source = f"the call produces a {actual.short_side}px short side"
    return Finding(
        "resolution_claim",
        f'prompt says "{claim_text}" but {source}. '
        f"Raise resolution, or drop the claim from the prompt.",
    )


# Directive phrases worth comparing across the two prompt fields. A curated list,
# because ordinary words legitimately appear in both and would drown the signal.
CONFLICT_PHRASES = (
    "slow motion", "slow-mo", "motion blur", "static", "blurred", "blurry",
    "sharp", "text", "close-up", "wide shot", "zoom", "pan", "handheld", "grain",
)

# "no text", "without a logo", "avoid harsh shadows" - up to three trailing words.
# Feeds strip_negations (via check_prompt_negative_conflict): "not blurry" must still read
# as a negation there, or it looks like a request for blur. `not` stays in this pattern.
NEGATION_RE = re.compile(r"\b(?:no|without|avoid|not)\s+(?:[a-z-]+\s+){0,2}[a-z-]+", re.IGNORECASE)

# SPEC §5.3 names exactly three directive forms: "no X", "without X", "avoid X". `not` is
# not one of them - "the sun has not yet risen" and "does not move" are ordinary narrative
# negation, not an instruction to move anything into negative_prompt. check_negation_placement
# uses this narrower pattern instead of NEGATION_RE so it stops firing on plain English.
PLACEMENT_NEGATION_RE = re.compile(r"\b(?:no|without|avoid)\s+(?:[a-z-]+\s+){0,2}[a-z-]+", re.IGNORECASE)

# Quantifier idioms where "no" modifies a comparative or adverb rather than introducing an
# excluded noun - "no more than three [people]" is a count, not a directive, and moving
# "more than three" into negative_prompt is meaningless. Keep this list small; add an entry
# only when a real false positive demonstrates the idiom.
_PLACEMENT_STOPWORDS = frozenset({("no", "more"), ("no", "longer"), ("no", "matter")})


def _is_placement_noise(match_text: str) -> bool:
    words = match_text.lower().split()
    if len(words) >= 2 and tuple(words[:2]) in _PLACEMENT_STOPWORDS:
        return True
    # A bare gerund right after the negator ("without moving", "avoid running") reads as
    # narrative manner - "sitting without moving" describes what the subject is doing, it
    # does not name an object to exclude from the shot. A gerund with a determiner or
    # adjective in front ("without a watermark", "no clutter", "no lettering") is a noun
    # phrase and still fires; only the bare two-word "negator + gerund" span is excluded.
    if len(words) == 2 and words[1].endswith("ing"):
        return True
    return False


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
    verb = "appears" if len(hits) == 1 else "appear"
    return Finding(
        "prompt_negative_conflict",
        f"{listed} {verb} in both prompt and negative_prompt. Keep each term in one.",
    )


def check_negation_placement(params: dict, models: frozenset) -> Optional[Finding]:
    prompt = params.get("prompt")
    if not isinstance(prompt, str):
        return None
    model = params.get("model")
    negative = params.get("negative_prompt")
    supports = (isinstance(negative, str) and negative.strip() != "") or (
        isinstance(model, str) and model in models
    )
    if not supports:
        return None
    negations = [
        m.group(0).strip()
        for m in PLACEMENT_NEGATION_RE.finditer(prompt)
        if not _is_placement_noise(m.group(0).strip())
    ]
    if not negations:
        return None
    shown = ", ".join(f'"{n}"' for n in negations[:3])
    more = f" and {len(negations) - 3} more" if len(negations) > 3 else ""
    verb = "sits" if len(negations) == 1 else "sit"
    return Finding(
        "negation_placement",
        f"{shown}{more} {verb} in the positive prompt while this model accepts "
        f"negative_prompt. Move them there; models follow negation poorly in the prompt.",
    )


CHECKS = (check_resolution_claim, check_prompt_negative_conflict, check_negation_placement)


def _resolve_params(raw) -> Optional[dict]:
    """All three Higgsfield tool schemas declare `params` as anyOf [object, string]
    (SPEC §9): a JSON-encoded string is a legal shape, not a malformed fixture. Parse
    it and lint normally; fall through to None (silence) only when it is neither a
    dict nor a string that parses to one."""
    if isinstance(raw, dict):
        return raw
    if isinstance(raw, str):
        try:
            parsed = json.loads(raw)
        except (json.JSONDecodeError, ValueError):
            return None
        return parsed if isinstance(parsed, dict) else None
    return None


def _lint_params(params: dict, models: frozenset) -> list:
    findings = []
    for check in CHECKS:
        finding = check(params, models)
        if finding is not None:
            findings.append(finding)
    return findings


def lint(tool_input, models: frozenset) -> list:
    if not isinstance(tool_input, dict):
        return []
    # Batch tools (generate_*_batch) use {"requests": [{"index": 0, "params": {...}}, ...]}
    # instead of a top-level `params`. Lint each entry's params and prefix its findings
    # with the request index so the user knows which request a finding refers to.
    requests = tool_input.get("requests")
    if isinstance(requests, list) and requests:
        findings = []
        for i, entry in enumerate(requests):
            if not isinstance(entry, dict):
                continue
            params = _resolve_params(entry.get("params"))
            if params is None:
                continue
            index = entry.get("index", i)
            for finding in _lint_params(params, models):
                findings.append(Finding(finding.check_id, f"request {index}: {finding.message}"))
        return findings
    params = _resolve_params(tool_input.get("params"))
    if params is None:
        return []
    return _lint_params(params, models)


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
