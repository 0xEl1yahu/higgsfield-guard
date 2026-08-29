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


CHECKS = (check_resolution_claim, check_prompt_negative_conflict, check_negation_placement)


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
