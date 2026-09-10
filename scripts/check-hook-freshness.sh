#!/bin/sh
# SessionStart hook: the PreToolUse lint is a compiled binary that is gitignored, so a
# fresh clone or worktree has no binary at all. A missing or stale binary fails silently
# and non-blockingly at PreToolUse time (SPEC 12.1) - indistinguishable from a clean
# lint. This is the one place that difference can be surfaced without risking the
# generation path, because SessionStart is not in it.
#
# Exits 0 always. Prints nothing when the binary is present and newer than every source
# file. Never writes to stderr.

set -u

root="${CLAUDE_PROJECT_DIR:-.}"
bin="$root/target/release/higgsfield-lint"
src="$root/rust/src"

[ -d "$src" ] || exit 0   # not this repo; say nothing

if [ ! -x "$bin" ]; then
    msg="higgsfield-lint: the PreToolUse hook binary is MISSING (target/release/higgsfield-lint). Higgsfield generate_* calls are NOT being linted right now, and a missing binary looks exactly like a clean lint. Run: cargo build --release"
elif [ -n "$(find "$src" -name '*.rs' -newer "$bin" -print 2>/dev/null | head -n 1)" ]; then
    msg="higgsfield-lint: the PreToolUse hook binary is STALE - rust/src/*.rs is newer than target/release/higgsfield-lint, so the lint running is not the lint in the tree. Run: cargo build --release"
else
    exit 0
fi

# The message is deliberately free of double quotes, backslashes and control characters,
# so it needs no JSON escaping.
printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"%s"}}' "$msg"
exit 0
