# Robot-mode CLI — design

Date: 2026-09-10. Branch: `cli-robotcreation`. Status: approved by the requester in the
same message that asked for the implementation ("design and implement"); implemented
against this document in the same change.

## 1. Goal

Make `higgsfield-lint` usable by AI coding agents as a first-class CLI, without touching
the one contract that already ships: the Claude Code `PreToolUse` hook path.

Requirements, as given:

1. `--json` on every command.
2. No arguments shows help in about 100 tokens.
3. Errors are structured: code, message, suggestions.
4. Auto-switch to JSON when piped.
5. Meaningful exit codes.
6. Dense output that respects context limits.

## 2. The constraint that shapes everything

The deployed hook (`.claude/settings.json`) runs the binary **with no arguments** and the
hook event JSON on stdin. SPEC.md §7 requires that path to exit 0, write nothing to
stderr, and emit either nothing or one `hookSpecificOutput` document. Requirement 2
("no args shows help") collides with that head-on.

Measured facts that resolve it:

- Claude Code always writes the event JSON to a hook's stdin. An empty stdin never occurs
  on a real hook invocation.
- In an agent's shell (Claude Code's Bash tool, measured in this session) stdin is not a
  terminal and reads zero bytes; stdout is not a terminal.
- A human at a terminal has a TTY on stdin.

So "no arguments" splits three ways on stdin, and the hook keeps every byte it has today:

| argv empty and… | behaviour | exit |
|---|---|---|
| stdin is a terminal | quick start (text) | 0 |
| stdin is not a terminal, 0 bytes | quick start (JSON if stdout is piped, else text) | 0 |
| stdin is not a terminal, ≥1 byte | **hook path, unchanged** | 0 always |

The only observable change to the hook path is the 0-byte case, which Claude Code never
produces. It keeps the two hard guarantees (exit 0, empty stderr) and changes only "empty
stdout" to "quick-start document on stdout" for that one impossible input.

Any argument at all means the CLI, never the hook. That replaces the old rule "an
unrecognised argument falls through to the hook". The reason for the old rule was a
stray argument typed into `settings.json` turning into a diagnostic dump; under the new
rule a stray argument is a usage error that exits 2, which Claude Code would treat as a
blocking hook error. That hazard is already caught by `doctor`: check 5 ("matcher
present") only passes when the settings command resolves to exactly this executable,
and any trailing argument makes it fail to resolve. A unit test pins that property.

## 3. Interface

```
higgsfield-lint <command> [args] [--json|--text]

  lint [FILE|-] --tool image|video|audio   lint one call; exit 1 = findings
  doctor                                   check hook install; exit 1 = FAIL
  checks                                   list check ids, scope, fixes
  version                                  print version
  help [COMMAND]                           full help for one command

aliases: -h, --help → help; -V, --version → version
```

Global flags may appear anywhere in argv. `--json` and `--text` together is a usage
error. Without either, output is JSON when stdout is not a terminal, text otherwise.
No colour, no ANSI, ever.

### 3.1 Exit codes

| code | meaning |
|---|---|
| 0 | ok: clean lint, every doctor check passed, or an informational command |
| 1 | negative result: lint reported findings, or doctor reported a FAIL |
| 2 | usage: unknown command or flag, conflicting flags, missing or unknown `--tool` |
| 3 | input: file missing or unreadable, invalid UTF-8 or JSON, unrecognised payload shape |
| 4 | internal: a caught panic |

The hook path is outside this table: it exits 0 unconditionally.

### 3.2 Structured errors

JSON mode writes the error document to **stdout** and nothing to stderr, so a robot
always parses exactly one stream. Text mode writes it to stderr.

```json
{"ok":false,"error":{"code":"unknown_command","message":"unknown command \"lnt\"","suggestions":["did you mean \"lint\"?","run: higgsfield-lint help"]},"exit":2}
```

Error codes by exit class:

- usage (2): `unknown_command`, `unknown_flag`, `unexpected_argument`, `conflicting_flags`,
  `missing_value`, `missing_input`, `missing_tool`, `unknown_tool`
- input (3): `file_not_found`, `read_failed`, `invalid_utf8`, `invalid_json`,
  `unsupported_shape`
- internal (4): `internal`

Every document, success or error, carries `ok` and `exit` so an agent that only sees
stdout still knows the outcome.

### 3.3 `lint`

Input is FILE, `-`, or stdin when FILE is omitted. Omitted FILE with a terminal on stdin
is `missing_input` rather than a hang.

`--tool` accepts `image`, `video`, `audio`, or a tool name in either style the hook
accepts (`generate_video_batch`, `mcp__x__generate_video`). It overrides a `tool_name`
in the payload.

Three payload shapes are accepted, tested in this order, and the chosen one is echoed
back as `shape`:

1. `event`: object with a `tool_input` key. The hook event as Claude Code sends it.
   Tool comes from `--tool`, else `tool_name`.
2. `tool_input`: object with a `params` or `requests` key. Needs `--tool`.
3. `params`: any other object; wrapped as `{"params": …}`. Needs `--tool`.

A non-object document is `unsupported_shape`. A `tool_name` the hook would classify as
unknown, with no `--tool`, is `unknown_tool`; no tool at all is `missing_tool`.

Output:

```json
{"ok":false,"tool":"video","shape":"params","ran":["resolution_claim","prompt_negative_conflict","duration_claim"],"findings":[{"check":"resolution_claim","message":"prompt says \"4k\" but resolution selects the \"720p\" output tier. Choose a supported higher tier, or revise the claim."}],"exit":1}
```

- `ran` is the list of checks that apply to this tool kind, so an empty `findings` is
  read as "these checks passed", not "everything is fine". Audio runs nothing and adds
  `note`.
- Batch findings carry `request` (the same rendered index the hook prefixes), instead of
  embedding it in `message`.
- `message` is byte-identical to the hook's finding line minus the `- <id>: ` prefix.

Text mode prints exactly what the hook's `additionalContext` would contain.

### 3.4 `doctor`

Text mode is byte-identical to today. JSON:

```json
{"ok":true,"checks":[{"name":"binary location","status":"pass","detail":"…"}],"summary":{"pass":7,"warn":0,"fail":0},"note":"doctor checks this checkout and binary; confirm the hook is loaded with /hooks and a tool trace in the target session","exit":0}
```

### 3.5 `checks`

Static description of each live check: `id`, `tools`, `fires`, `fix`, `silent_when`,
plus the enumerable lexicons a robot can act on (`phrases` for the conflict check,
`models` for the duration check). The resolution check's model table is a match
expression, not a list; it points at `docs/higgsfield-canonical-contract.md` instead.

### 3.6 `version`, `help`

`version`: `{"ok":true,"name":"higgsfield-lint","version":"0.1.0","exit":0}`.

`help` with no argument is the quick start, targeted at about 100 tokens in both modes.
`help COMMAND` renders one command's usage, arguments, flags, output shape, exit codes
and examples from a single data table, so text and JSON cannot drift apart.

## 4. Code layout

| file | change |
|---|---|
| `rust/src/cli.rs` (new) | argv parsing, `Format`, `CliError`, help table, `lint`/`checks`/`version`, renderers, panic-to-`internal` wrapper |
| `rust/src/main.rs` | dispatch: any argv → `cli`; empty argv + TTY stdin → quick start; else hook. Hook body untouched; the 0-byte stdin branch is three lines inside `run()` so the mutation anchors survive |
| `rust/src/lint.rs` | `Finding.request: Option<String>`; `render` adds the `request N: ` prefix; `checks_for(kind)` |
| `rust/src/checks.rs` | `CONFLICT_PHRASES` and `DURATION_MODELS` become `pub`; `Finding` literals gain `request: None` |
| `rust/src/doctor.rs` | `run(Format)`; JSON report; a name/detail split with a test pinning every check name |
| `rust/tests/cli.rs` (new) | process-level contract: every command, both formats, every exit class, empty-stdin quick start, stray-argument behaviour |
| `rust/tests/fail_silent.rs` | the 0-byte case and the stray-argument test move to `cli.rs` with their new expectations; every other pathological input keeps its assertion |
| `rust/tests/doctor.rs` | text assertions pass `--text`; one JSON assertion added |
| `ci/verify.sh` | `doctor --text` so the CI log stays human-readable |
| `SPEC.md` | new §6.1 (CLI contract), §7 amendment (0-byte stdin), §10 layout |
| `README.md` | CLI section |

No new dependencies. TTY detection is `std::io::IsTerminal`.

## 5. Testing

Process-level tests spawn the release binary with piped stdio, which is exactly the
robot's environment. Each command is asserted in both formats; each exit class has at
least one test; the hook path's existing golden, adversarial and fail-silent suites must
stay green untouched except for the two cases named above. `ci/mutation_checks.py`'s
source anchors are preserved verbatim.
