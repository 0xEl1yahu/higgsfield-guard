#!/usr/bin/env python3
"""CI-only mutation driver. Never used by the lint runtime or as a behavior oracle.

Work only in a TemporaryDirectory copy, require exact source anchors, establish each
selected test passes first, and accept only a named Rust test assertion failure as a
kill. Compiler errors, zero selected tests, command errors and timeouts fail this job.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
MUTANTS = [
    ("always-silent", "rust/src/main.rs",
     "let findings = lint::lint(tool_input, kind);",
     "let findings: Vec<lint::Finding> = Vec::new();",
     "release", "test", "adversarial", "paired_bad_claims_and_clean_counterexamples"),
    ("audio-scope-removed", "rust/src/lint.rs",
     "if kind == ToolKind::Audio {", "if false {",
     "release", "test", "adversarial", "scope_and_unknown_capabilities_cannot_create_output_claims"),
    ("duration-comparison-inverted", "rust/src/checks.rs",
     "if claimed <= supplied {", "if claimed > supplied {",
     "release", "test", "adversarial", "paired_bad_claims_and_clean_counterexamples"),
    ("resolution-quotes-ignored", "rust/src/checks.rs",
     "let quoted = quoted_spans(&normalized);", "let quoted = Vec::new();",
     "release", "test", "adversarial", "quoted_typography_and_boundary_near_misses_do_not_fire"),
    ("negative-side-negation-ignored", "rust/src/checks.rs",
     "let negative_stripped = strip_negations(&normalize_for_matching(negative)).to_lowercase();",
     "let negative_stripped = normalize_for_matching(negative).to_lowercase();",
     "release", "test", "adversarial", "paired_bad_claims_and_clean_counterexamples"),
    ("batch-branch-ignored", "rust/src/lint.rs",
     "if !requests.is_empty() {", "if false {",
     "release", "test", "adversarial", "batch_invalid_entries_cannot_mask_neighbors_or_trigger_fallback"),
    ("quote-lookup-linear", "rust/src/checks.rs",
     "spans.partition_point(|&(start, _)| {\n        observe();\n        start <= pos\n    })",
     "spans.iter().take_while(|&&(start, _)| { observe(); start <= pos }).count()",
     "release", "bin", "higgsfield-lint", "checks::tests::quote_lookup_has_logarithmic_comparison_bound"),
    ("panic-hook-not-silenced", "rust/src/main.rs",
     "panic::set_hook(Box::new(|_| {}));", "// MUTANT: default panic hook leaks stderr",
     "debug", "test", "fail_silent", "an_internal_panic_is_still_silent_and_zero"),
    ("panic-not-caught", "rust/src/main.rs",
     "let result = panic::catch_unwind(run).unwrap_or(None);", "let result = run();",
     "debug", "test", "fail_silent", "an_internal_panic_is_still_silent_and_zero"),
]


def outcome(returncode, text, test):
    """Do not confuse a broken build or a stale test filter with a killed mutant."""
    if "running 1 test\n" not in text:
        return "invalid"
    if returncode == 0 and f"test {test} ... ok" in text:
        return "passed"
    if (returncode == 101 and f"test {test} ... FAILED" in text
            and "test result: FAILED. 0 passed; 1 failed;" in text):
        return "killed"
    return "invalid"


def self_test():
    assert outcome(101, "could not compile", "x") == "invalid"
    assert outcome(0, "running 0 tests\ntest result: ok", "x") == "invalid"
    assert outcome(101, "running 1 test\ntest other ... FAILED", "x") == "invalid"
    assert outcome(0, "running 1 test\ntest x ... ok", "x") == "passed"
    assert outcome(101, "running 1 test\ntest x ... FAILED\ntest result: FAILED. 0 passed; 1 failed;", "x") == "killed"


def run_test(scratch, spec, log):
    profile, target_type, target, test = spec
    args = ["cargo", "test", "--locked"]
    if profile == "release":
        args.append("--release")
    args += [f"--{target_type}", target, test, "--", "--exact", "--nocapture", "--color", "never"]
    env = dict(os.environ, CARGO_TERM_COLOR="never", CARGO_TARGET_DIR=str(scratch / "target"))
    env.pop("CLAUDE_PROJECT_DIR", None)
    try:
        result = subprocess.run(args, cwd=scratch, env=env, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=300)
    except subprocess.TimeoutExpired as error:
        captured = error.stdout or b""
        log.write_text(captured.decode(errors="replace") if isinstance(captured, bytes) else captured)
        raise RuntimeError(f"test timed out; not counted as a kill: {test}") from error
    log.write_text(result.stdout)
    return outcome(result.returncode, result.stdout, test)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", default="target/ci-mutations")
    args = parser.parse_args()
    output = (ROOT / args.output).resolve()
    if not output.is_relative_to(ROOT / "target"):
        parser.error("output must be under this checkout's target directory")
    output.mkdir(parents=True, exist_ok=True)
    self_test()
    results = []
    try:
        with tempfile.TemporaryDirectory(prefix="higgsfield-mutations-") as directory:
            scratch = Path(directory)
            for path in ["Cargo.toml", "Cargo.lock"]:
                shutil.copy2(ROOT / path, scratch / path)
            for path in ["rust", "tests/fixtures", "docs/evidence"]:
                shutil.copytree(ROOT / path, scratch / path)
            # Compile-time snapshot and fixture dependencies are local; no provider calls.
            baselines = set()
            for name, file, before, after, *spec in MUTANTS:
                path = scratch / file
                original = path.read_text()
                if original.count(before) != 1:
                    raise RuntimeError(f"{name}: source anchor must match exactly once; update the probe after refactors")
                spec_key = tuple(spec)
                if spec_key not in baselines:
                    state = run_test(scratch, spec, output / f"{name}-baseline.log")
                    if state != "passed":
                        raise RuntimeError(f"{name}: baseline {state}; refusing to score mutants")
                    baselines.add(spec_key)
                try:
                    path.write_text(original.replace(before, after, 1))
                    state = run_test(scratch, spec, output / f"{name}-mutant.log")
                finally:
                    path.write_text(original)
                results.append({"mutation": name, "test": spec[-1], "outcome": state})
                print(f"{name}: {state}", flush=True)
                if state != "killed":
                    raise RuntimeError(f"{name}: {state}; test did not catch the regression")
    finally:
        (output / "summary.json").write_text(json.dumps({"expected": len(MUTANTS), "results": results}, indent=2) + "\n")
    print(f"All {len(MUTANTS)} targeted regressions caught; compiler failures never count as kills.")


if __name__ == "__main__":
    main()
