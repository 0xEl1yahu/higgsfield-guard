//! Robot-mode command-line interface (SPEC.md §6.1;
//! docs/superpowers/specs/2026-09-10-robot-mode-cli-design.md).
//!
//! Everything here is reached only when the binary is given at least one argument, or
//! when it is run with no arguments and a terminal (or nothing at all) on stdin. The
//! `PreToolUse` hook path in `main.rs` - no arguments, event JSON on stdin - never enters
//! this module, so nothing here can change what Claude Code sees.
//!
//! Contract in one breath: one document per invocation, on stdout in JSON mode and on
//! stdout (results) or stderr (errors) in text mode; every document carries `ok` and
//! `exit`; exit 0 ok, 1 negative result, 2 usage, 3 input, 4 internal; no ANSI, ever.

use std::io::{IsTerminal, Read, Write};
use std::panic::{self, AssertUnwindSafe};
use std::sync::Mutex;

use serde_json::{json, Value};

use crate::lint::{self, ToolKind};
use crate::{checks, doctor};

pub const NAME: &str = "higgsfield-lint";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const SUMMARY: &str = "lint Higgsfield generate_* calls for prompt/param contradictions";
const HOOK_LINE: &str = "no args + JSON on stdin = Claude Code hook (always exit 0)";

pub const EXIT_OK: i32 = 0;
pub const EXIT_NEGATIVE: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_INPUT: i32 = 3;
pub const EXIT_INTERNAL: i32 = 4;
const EXIT_CODES: &[(&str, &str)] = &[
    ("0", "ok"),
    ("1", "findings/fail"),
    ("2", "usage"),
    ("3", "input"),
    ("4", "internal"),
];

// --- output format -------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Json,
    Text,
}

impl Format {
    /// JSON when stdout is not a terminal (a pipe, a file, an agent's shell), text when
    /// a human is looking at it. `--json` / `--text` override this.
    pub fn detect() -> Self {
        if std::io::stdout().is_terminal() {
            Format::Text
        } else {
            Format::Json
        }
    }
}

// --- results and errors ---------------------------------------------------------------

struct Output {
    exit: i32,
    json: Value,
    text: String,
}

#[derive(Debug)]
pub struct CliError {
    code: &'static str,
    exit: i32,
    message: String,
    suggestions: Vec<String>,
}

impl CliError {
    fn usage(code: &'static str, message: impl Into<String>, suggestions: &[&str]) -> Self {
        Self::new(code, EXIT_USAGE, message, suggestions)
    }
    fn input(code: &'static str, message: impl Into<String>, suggestions: &[&str]) -> Self {
        Self::new(code, EXIT_INPUT, message, suggestions)
    }
    fn new(
        code: &'static str,
        exit: i32,
        message: impl Into<String>,
        suggestions: &[&str],
    ) -> Self {
        CliError {
            code,
            exit,
            message: message.into(),
            suggestions: suggestions.iter().map(|s| s.to_string()).collect(),
        }
    }
    fn json(&self) -> Value {
        json!({
            "ok": false,
            "exit": self.exit,
            "error": {
                "code": self.code,
                "message": self.message,
                "suggestions": self.suggestions,
            }
        })
    }
    fn text(&self) -> String {
        let mut s = format!("error[{}]: {}\n", self.code, self.message);
        for hint in &self.suggestions {
            s.push_str("  ");
            s.push_str(hint);
            s.push('\n');
        }
        s
    }
}

type CmdResult = Result<Output, CliError>;

// --- argv ------------------------------------------------------------------------------

const COMMAND_NAMES: &[&str] = &["lint", "doctor", "checks", "version", "help"];

#[derive(Default)]
struct Parsed {
    format: Option<Format>,
    command: Option<String>,
    positionals: Vec<String>,
    tool: Option<String>,
    help: bool,
    version: bool,
}

fn parse(args: &[String]) -> Result<Parsed, CliError> {
    let mut p = Parsed::default();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--json" | "--text" => {
                let wanted = if arg == "--json" {
                    Format::Json
                } else {
                    Format::Text
                };
                if p.format.is_some_and(|f| f != wanted) {
                    return Err(CliError::usage(
                        "conflicting_flags",
                        "--json and --text cannot both be given",
                        &["pass one of --json, --text, or neither (auto-detects from stdout)"],
                    ));
                }
                p.format = Some(wanted);
            }
            "--tool" => {
                i += 1;
                match args.get(i) {
                    Some(v) if !v.starts_with('-') => p.tool = Some(v.clone()),
                    _ => {
                        return Err(CliError::usage(
                            "missing_value",
                            "--tool needs a value",
                            &["--tool image|video|audio, or a generate_(image|video|audio)[_batch] tool name"],
                        ))
                    }
                }
            }
            _ if arg.starts_with("--tool=") => p.tool = Some(arg["--tool=".len()..].to_string()),
            "-h" | "--help" => p.help = true,
            "-V" | "--version" => p.version = true,
            "-" => p.positionals.push(arg.to_string()),
            _ if arg.starts_with('-') => {
                return Err(CliError::usage(
                    "unknown_flag",
                    format!("unknown flag {arg:?}"),
                    &[
                        "global flags: --json, --text, -h/--help, -V/--version",
                        "lint flags: --tool T",
                        "run: higgsfield-lint help",
                    ],
                ))
            }
            _ if p.command.is_none() => p.command = Some(arg.to_string()),
            _ => p.positionals.push(arg.to_string()),
        }
        i += 1;
    }
    Ok(p)
}

// --- entry -----------------------------------------------------------------------------

/// The format the caller asked for, read before full parsing so that even an argv error
/// comes out in the requested format. Both flags present resolves to auto-detect and
/// `parse` reports the conflict itself.
fn format_hint(args: &[String]) -> Option<Format> {
    let json = args.iter().any(|a| a == "--json");
    let text = args.iter().any(|a| a == "--text");
    match (json, text) {
        (true, false) => Some(Format::Json),
        (false, true) => Some(Format::Text),
        _ => None,
    }
}

static PANIC_MESSAGE: Mutex<Option<String>> = Mutex::new(None);

/// Runs the CLI for `args` (argv without the program name) and returns the exit code.
/// Never panics out: an internal panic becomes a structured `internal` error, exit 4.
pub fn run(args: Vec<String>) -> i32 {
    // Parsing cannot panic and decides the output format, so do it outside the guard:
    // even an internal error has to come out in the format the caller asked for.
    let parsed = match parse(&args) {
        Ok(p) => p,
        Err(e) => return emit(format_hint(&args).unwrap_or_else(Format::detect), Err(e)),
    };
    let format = parsed.format.unwrap_or_else(Format::detect);

    panic::set_hook(Box::new(|info| {
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic".to_string());
        if let Ok(mut slot) = PANIC_MESSAGE.lock() {
            *slot = Some(msg);
        }
    }));
    let result = panic::catch_unwind(AssertUnwindSafe(|| execute(parsed)));
    match result {
        Ok(result) => emit(format, result),
        Err(_) => {
            let detail = PANIC_MESSAGE
                .lock()
                .ok()
                .and_then(|s| s.clone())
                .unwrap_or_else(|| "panic".to_string());
            emit(
                format,
                Err(CliError::new(
                    "internal",
                    EXIT_INTERNAL,
                    format!("internal error: {detail}"),
                    &["this is a bug in higgsfield-lint; report it with the exact input"],
                )),
            )
        }
    }
}

/// The document the hook path prints when it is invoked with no arguments and 0 bytes
/// on a non-terminal stdin (SPEC.md §7): an agent asking "what are you?", never a hook.
pub fn quick_start_document(format: Format) -> String {
    let out = quick_start();
    match format {
        Format::Json => format!("{}\n", out.json),
        Format::Text => out.text,
    }
}

fn emit(format: Format, result: CmdResult) -> i32 {
    // Never println!: a closed pipe (`| head`) would turn a clean result into a panic.
    let (stream, body, exit): (Box<dyn Write>, String, i32) = match (format, result) {
        (Format::Json, Ok(o)) => (Box::new(std::io::stdout()), format!("{}\n", o.json), o.exit),
        (Format::Text, Ok(o)) => (Box::new(std::io::stdout()), o.text, o.exit),
        (Format::Json, Err(e)) => (
            Box::new(std::io::stdout()),
            format!("{}\n", e.json()),
            e.exit,
        ),
        (Format::Text, Err(e)) => (Box::new(std::io::stderr()), e.text(), e.exit),
    };
    let mut stream = stream;
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
    exit
}

fn execute(p: Parsed) -> CmdResult {
    if p.version {
        return version();
    }
    if p.help {
        return match p.command.as_deref() {
            Some(name) => command_help(name),
            None => Ok(quick_start()),
        };
    }
    let Some(command) = p.command.as_deref() else {
        return Ok(quick_start());
    };
    if p.tool.is_some() && command != "lint" {
        return Err(CliError::usage(
            "unknown_flag",
            format!("--tool is only valid for lint, not {command}"),
            &["run: higgsfield-lint help lint"],
        ));
    }
    match command {
        "help" => match p.positionals.as_slice() {
            [] => Ok(quick_start()),
            [name] => command_help(name),
            more => Err(unexpected(&more[1..])),
        },
        "version" => no_positionals(&p.positionals).and_then(|_| version()),
        "checks" => no_positionals(&p.positionals).and_then(|_| checks_command()),
        "doctor" => no_positionals(&p.positionals).and_then(|_| doctor_command()),
        "lint" => match p.positionals.as_slice() {
            [] => lint_command(None, p.tool.as_deref()),
            [file] => lint_command(Some(file), p.tool.as_deref()),
            more => Err(unexpected(&more[1..])),
        },
        other => Err(unknown_command(other)),
    }
}

fn no_positionals(positionals: &[String]) -> Result<(), CliError> {
    if positionals.is_empty() {
        Ok(())
    } else {
        Err(unexpected(positionals))
    }
}

fn unexpected(extra: &[String]) -> CliError {
    CliError::usage(
        "unexpected_argument",
        format!("unexpected argument(s): {}", extra.join(" ")),
        &["run: higgsfield-lint help <command>"],
    )
}

fn unknown_command(name: &str) -> CliError {
    let mut suggestions = Vec::new();
    if let Some(near) = nearest_command(name) {
        suggestions.push(format!("did you mean {near:?}?"));
    }
    if name.ends_with(".json") || name.contains('/') {
        suggestions.push(format!(
            "to lint a file: higgsfield-lint lint {name} --tool image|video|audio"
        ));
    }
    suggestions.push("run: higgsfield-lint help".to_string());
    CliError {
        code: "unknown_command",
        exit: EXIT_USAGE,
        message: format!("unknown command {name:?}"),
        suggestions,
    }
}

/// Closest known command by edit distance, if it is within two edits.
fn nearest_command(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    COMMAND_NAMES
        .iter()
        .map(|c| (levenshtein(&lower, c), *c))
        .filter(|(d, _)| *d <= 2)
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

// --- help -------------------------------------------------------------------------------

struct CommandDoc {
    name: &'static str,
    args: &'static str,
    summary: &'static str,
    description: &'static str,
    arguments: &'static [(&'static str, &'static str)],
    flags: &'static [(&'static str, &'static str)],
    input: &'static str,
    output: &'static str,
    exit_codes: &'static [(&'static str, &'static str)],
    examples: &'static [&'static str],
}

const GLOBAL_FLAGS: &[(&str, &str)] = &[
    (
        "--json",
        "JSON output (default when stdout is not a terminal)",
    ),
    ("--text", "text output (default when stdout is a terminal)"),
];

const COMMAND_DOCS: &[CommandDoc] = &[
    CommandDoc {
        name: "lint",
        args: "[FILE|-] --tool image|video|audio",
        summary: "lint a call; exit 1 = findings",
        description: "Runs the same checks the PreToolUse hook runs, on one Higgsfield generate_* call, and reports findings as data.",
        arguments: &[("FILE", "JSON document to lint; '-' or omitted reads stdin")],
        flags: &[(
            "--tool T",
            "image, video, audio, or a tool name (generate_video_batch, mcp__x__generate_image); overrides tool_name in the payload; required unless the payload is a hook event with tool_name",
        )],
        input: "one JSON object in any of three shapes, echoed back as `shape`: event {tool_name, tool_input} as Claude Code sends it; tool_input {params} or {requests:[{index, params}]}; params (any other object, treated as the params of one call)",
        output: "{ok, exit, tool, shape, ran:[check ids that apply to this tool], findings:[{check, message, request?}], note?}; message is the hook's finding text; request is the batch index",
        exit_codes: &[
            ("0", "no findings"),
            ("1", "findings"),
            ("2", "usage: bad flag, missing or unknown --tool"),
            ("3", "input: file missing, invalid UTF-8/JSON, non-object document"),
            ("4", "internal"),
        ],
        examples: &[
            "echo '{\"model\":\"seedance_2_0\",\"prompt\":\"shot in 4k\",\"resolution\":\"720p\"}' | higgsfield-lint lint --tool video",
            "higgsfield-lint lint call.json --tool image",
            "higgsfield-lint lint hook-event.json",
        ],
    },
    CommandDoc {
        name: "doctor",
        args: "",
        summary: "hook install checks; exit 1 = FAIL",
        description: "Seven ordered checks on this checkout and binary: location, retired data file, binary freshness (mtime heuristic), settings file, matcher present, matcher correctness, self-test. WARN does not fail.",
        arguments: &[],
        flags: &[],
        input: "none; reads CLAUDE_PROJECT_DIR when set",
        output: "{ok, exit, checks:[{name, status: pass|warn|fail, detail}], summary:{pass, warn, fail}, note}",
        exit_codes: &[("0", "every check passed or warned"), ("1", "at least one FAIL"), ("4", "internal")],
        examples: &["higgsfield-lint doctor", "CLAUDE_PROJECT_DIR=$PWD higgsfield-lint doctor --text"],
    },
    CommandDoc {
        name: "checks",
        args: "",
        summary: "list check ids and fixes",
        description: "Static description of every live check: when it fires, how to fix it, when it stays silent, and the lexicons a caller can act on.",
        arguments: &[],
        flags: &[],
        input: "none",
        output: "{ok, exit, checks:[{id, tools, fires, fix, silent_when, phrases?, models?}]}",
        exit_codes: &[("0", "ok")],
        examples: &["higgsfield-lint checks"],
    },
    CommandDoc {
        name: "version",
        args: "",
        summary: "print name and version",
        description: "Package name and version.",
        arguments: &[],
        flags: &[],
        input: "none",
        output: "{ok, exit, name, version}",
        exit_codes: &[("0", "ok")],
        examples: &["higgsfield-lint version", "higgsfield-lint -V"],
    },
    CommandDoc {
        name: "help",
        args: "[COMMAND]",
        summary: "help for one command",
        description: "Quick start with no argument; usage, arguments, flags, input, output, exit codes and examples for one command.",
        arguments: &[("COMMAND", "one of lint, doctor, checks, version, help")],
        flags: &[],
        input: "none",
        output: "{ok, exit, command, usage, description, arguments, flags, input, output, exit_codes, examples}",
        exit_codes: &[("0", "ok"), ("2", "unknown command")],
        examples: &["higgsfield-lint help lint"],
    },
];

fn quick_start() -> Output {
    let width = COMMAND_DOCS
        .iter()
        .map(|d| d.name.len() + usize::from(!d.args.is_empty()) + d.args.len())
        .max()
        .unwrap_or(0);
    let mut text =
        format!("{NAME} {VERSION} - {SUMMARY}\nusage: {NAME} <command> [--json|--text]\n");
    let mut commands = serde_json::Map::new();
    for d in COMMAND_DOCS {
        let head = if d.args.is_empty() {
            d.name.to_string()
        } else {
            format!("{} {}", d.name, d.args)
        };
        text.push_str(&format!("  {head:<width$}  {}\n", d.summary));
        let line = if d.args.is_empty() {
            d.summary.to_string()
        } else {
            format!("{}: {}", d.args, d.summary)
        };
        commands.insert(d.name.to_string(), Value::String(line));
    }
    text.push_str(HOOK_LINE);
    text.push('\n');
    text.push_str("exit: ");
    text.push_str(
        &EXIT_CODES
            .iter()
            .map(|(c, m)| format!("{c} {m}"))
            .collect::<Vec<_>>()
            .join(", "),
    );
    text.push('\n');
    Output {
        exit: EXIT_OK,
        json: json!({
            "ok": true,
            "exit": EXIT_OK,
            "name": NAME,
            "version": VERSION,
            "summary": SUMMARY,
            "usage": format!("{NAME} <command> [--json|--text]"),
            "commands": commands,
            "hook": HOOK_LINE,
            "exit_codes": pairs(EXIT_CODES),
        }),
        text,
    }
}

fn pairs(list: &[(&str, &str)]) -> Value {
    let mut m = serde_json::Map::new();
    for (k, v) in list {
        m.insert(k.to_string(), Value::String(v.to_string()));
    }
    Value::Object(m)
}

fn command_help(name: &str) -> CmdResult {
    let Some(d) = COMMAND_DOCS.iter().find(|d| d.name == name) else {
        return Err(unknown_command(name));
    };
    let usage = if d.args.is_empty() {
        format!("{NAME} {} [--json|--text]", d.name)
    } else {
        format!("{NAME} {} {} [--json|--text]", d.name, d.args)
    };
    let mut text = format!("usage: {usage}\n{}\n", d.description);
    if !d.arguments.is_empty() {
        text.push_str("arguments:\n");
        for (k, v) in d.arguments {
            text.push_str(&format!("  {k:<10} {v}\n"));
        }
    }
    text.push_str("flags:\n");
    for (k, v) in d.flags.iter().chain(GLOBAL_FLAGS) {
        text.push_str(&format!("  {k:<10} {v}\n"));
    }
    text.push_str(&format!(
        "input: {}\noutput: {}\nexit:\n",
        d.input, d.output
    ));
    for (k, v) in d.exit_codes {
        text.push_str(&format!("  {k} {v}\n"));
    }
    text.push_str("examples:\n");
    for e in d.examples {
        text.push_str(&format!("  {e}\n"));
    }
    let mut flags: Vec<(&str, &str)> = d.flags.to_vec();
    flags.extend_from_slice(GLOBAL_FLAGS);
    Ok(Output {
        exit: EXIT_OK,
        json: json!({
            "ok": true,
            "exit": EXIT_OK,
            "command": d.name,
            "usage": usage,
            "description": d.description,
            "arguments": pairs(d.arguments),
            "flags": pairs(&flags),
            "input": d.input,
            "output": d.output,
            "exit_codes": pairs(d.exit_codes),
            "examples": d.examples,
        }),
        text,
    })
}

// --- version, checks, doctor -------------------------------------------------------------

fn version() -> CmdResult {
    Ok(Output {
        exit: EXIT_OK,
        json: json!({"ok": true, "exit": EXIT_OK, "name": NAME, "version": VERSION}),
        text: format!("{NAME} {VERSION}\n"),
    })
}

fn checks_command() -> CmdResult {
    let table = json!([
        {
            "id": "resolution_claim",
            "tools": ["image", "video"],
            "fires": "prompt claims 4k, 1080p, 720p or 480p above the output tier a known model declares via resolution, quality or mode, or above Topaz output_width/output_height",
            "fix": "choose a supported higher tier, or drop the claim from the prompt",
            "silent_when": "model, field or option is outside the dated MCP catalog (docs/higgsfield-canonical-contract.md); the claim sits inside double quotes; 8k, uhd and full hd are style words, not claims",
        },
        {
            "id": "prompt_negative_conflict",
            "tools": ["image", "video"],
            "fires": "a curated directive phrase appears, unnegated, in both prompt and negative_prompt",
            "fix": "keep each term in one field",
            "silent_when": "the phrase is negated (no, without, avoid, not) in either field, or sits inside double quotes in the prompt",
            "phrases": checks::CONFLICT_PHRASES,
        },
        {
            "id": "duration_claim",
            "tools": ["video"],
            "fires": "prompt describes more seconds of action (\"Total: 15s\", \"8 seconds\", \"3-10s\") than params.duration, for a known video model and mode",
            "fix": "raise duration, or retime the beats to fit it",
            "silent_when": "model is not in the list below, mode is an edit/extension mode, duration is missing or outside 1-600, or the timing sits inside double quotes",
            "models": checks::DURATION_MODELS,
        },
    ]);
    let mut text = String::new();
    for c in table.as_array().unwrap() {
        let tools: Vec<&str> = c["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        text.push_str(&format!(
            "{}  [{}]  {}  fix: {}\n",
            c["id"].as_str().unwrap(),
            tools.join(","),
            c["fires"].as_str().unwrap(),
            c["fix"].as_str().unwrap()
        ));
    }
    Ok(Output {
        exit: EXIT_OK,
        json: json!({"ok": true, "exit": EXIT_OK, "checks": table}),
        text,
    })
}

fn doctor_command() -> CmdResult {
    let checks = doctor::collect_checks();
    let exit = doctor::exit_code(&checks);
    Ok(Output {
        exit,
        json: doctor::report_json(&checks, exit),
        text: doctor::report_text(&checks),
    })
}

// --- lint ---------------------------------------------------------------------------------

fn lint_command(file: Option<&str>, tool_flag: Option<&str>) -> CmdResult {
    let bytes = read_input(file)?;
    let source = file.filter(|f| *f != "-").unwrap_or("stdin");
    let text = String::from_utf8(bytes).map_err(|_| {
        CliError::input(
            "invalid_utf8",
            format!("{source} is not valid UTF-8"),
            &["the document must be UTF-8 encoded JSON"],
        )
    })?;
    let doc: Value = serde_json::from_str(&text).map_err(|e| {
        CliError::input(
            "invalid_json",
            format!("{source} is not valid JSON: {e}"),
            &["pass one JSON object: a hook event, a tool_input, or a params object"],
        )
    })?;
    let Some(obj) = doc.as_object() else {
        return Err(CliError::input(
            "unsupported_shape",
            format!("{source} is not a JSON object"),
            &["pass one JSON object: a hook event, a tool_input, or a params object"],
        ));
    };
    let (shape, tool_input, payload_tool) = if obj.contains_key("tool_input") {
        let inner = obj["tool_input"].clone();
        if !inner.is_object() {
            return Err(CliError::input(
                "unsupported_shape",
                "tool_input must be an object",
                &["a hook event looks like {\"tool_name\": ..., \"tool_input\": {\"params\": {...}}}"],
            ));
        }
        (
            "event",
            inner,
            obj.get("tool_name")
                .and_then(Value::as_str)
                .map(str::to_string),
        )
    } else if obj.contains_key("params") || obj.contains_key("requests") {
        ("tool_input", doc.clone(), None)
    } else {
        ("params", json!({ "params": doc }), None)
    };
    let kind = resolve_tool(tool_flag, payload_tool.as_deref())?;

    #[cfg(debug_assertions)]
    {
        // Same debug-only injection point as the hook path in main.rs; see the comment
        // there. Compiled out of release builds entirely.
        if tool_input.get("params").and_then(|p| p.get("__panic__")) == Some(&Value::Bool(true)) {
            panic!("debug-only panic injection triggered by __panic__ field");
        }
    }

    let findings = lint::lint(&tool_input, kind);
    let ran = lint::checks_for(kind);
    let exit = if findings.is_empty() {
        EXIT_OK
    } else {
        EXIT_NEGATIVE
    };
    let findings_json: Vec<Value> = findings
        .iter()
        .map(|f| {
            let mut m = json!({"check": f.check_id, "message": f.message});
            if let Some(r) = &f.request {
                m["request"] = Value::String(r.clone());
            }
            m
        })
        .collect();
    let mut out = json!({
        "ok": findings.is_empty(),
        "exit": exit,
        "tool": tool_label(kind),
        "shape": shape,
        "ran": ran,
        "findings": findings_json,
    });
    let mut text = format!("{}\n", lint::render(&findings));
    if kind == ToolKind::Audio {
        let note = "audio: prompt is spoken text, no text checks apply";
        out["note"] = Value::String(note.to_string());
        text.push_str(&format!("note: {note}\n"));
    }
    Ok(Output {
        exit,
        json: out,
        text,
    })
}

fn read_input(file: Option<&str>) -> Result<Vec<u8>, CliError> {
    match file {
        Some(path) if path != "-" => std::fs::read(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CliError::input(
                    "file_not_found",
                    format!("{path}: no such file"),
                    &["check the path, or pipe the document on stdin"],
                )
            } else {
                CliError::input(
                    "read_failed",
                    format!("{path}: {e}"),
                    &["check the path and permissions"],
                )
            }
        }),
        _ => {
            let stdin = std::io::stdin();
            if file.is_none() && stdin.is_terminal() {
                return Err(CliError::usage(
                    "missing_input",
                    "no FILE given and stdin is a terminal",
                    &["pass a FILE, or pipe JSON on stdin"],
                ));
            }
            let mut buf = Vec::new();
            stdin
                .lock()
                .read_to_end(&mut buf)
                .map_err(|e| CliError::input("read_failed", format!("stdin: {e}"), &[]))?;
            Ok(buf)
        }
    }
}

const TOOL_VALUES: &str =
    "valid --tool values: image, video, audio, or a generate_(image|video|audio)[_batch] tool name";

fn resolve_tool(flag: Option<&str>, payload: Option<&str>) -> Result<ToolKind, CliError> {
    if let Some(flag) = flag {
        let kind = match flag {
            "image" => ToolKind::Image,
            "video" => ToolKind::Video,
            "audio" => ToolKind::Audio,
            other => lint::tool_kind(Some(other)),
        };
        return if kind == ToolKind::Unknown {
            Err(CliError::usage(
                "unknown_tool",
                format!("--tool {flag:?} is not a Higgsfield generate_* tool"),
                &[TOOL_VALUES],
            ))
        } else {
            Ok(kind)
        };
    }
    match payload {
        Some(name) => {
            let kind = lint::tool_kind(Some(name));
            if kind == ToolKind::Unknown {
                Err(CliError::usage(
                    "unknown_tool",
                    format!("payload tool_name {name:?} is not a Higgsfield generate_* tool"),
                    &["pass --tool image|video|audio to override", TOOL_VALUES],
                ))
            } else {
                Ok(kind)
            }
        }
        None => Err(CliError::usage(
            "missing_tool",
            "no tool given: the document has no tool_name and --tool was not passed",
            &[
                "pass --tool image|video|audio",
                "or lint a hook event that carries tool_name",
            ],
        )),
    }
}

fn tool_label(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::Image => "image",
        ToolKind::Video => "video",
        ToolKind::Audio => "audio",
        ToolKind::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn nearest_command_tolerates_two_edits_and_case() {
        assert_eq!(nearest_command("lnt"), Some("lint"));
        assert_eq!(nearest_command("Lint"), Some("lint"));
        assert_eq!(nearest_command("docter"), Some("doctor"));
        assert_eq!(nearest_command("chekcs"), Some("checks"));
        assert_eq!(nearest_command("frobnicate"), None);
    }

    #[test]
    fn parse_collects_global_flags_anywhere_and_positionals_in_order() {
        let p = parse(&args(&["--json", "lint", "a.json", "--tool", "video", "-"])).unwrap();
        assert_eq!(p.format, Some(Format::Json));
        assert_eq!(p.command.as_deref(), Some("lint"));
        assert_eq!(p.positionals, vec!["a.json", "-"]);
        assert_eq!(p.tool.as_deref(), Some("video"));
        let p = parse(&args(&["lint", "--tool=image"])).unwrap();
        assert_eq!(p.tool.as_deref(), Some("image"));
    }

    #[test]
    fn parse_rejects_conflicting_formats_and_unknown_flags() {
        assert_eq!(
            parse(&args(&["--json", "--text"])).err().map(|e| e.code),
            Some("conflicting_flags")
        );
        assert_eq!(
            parse(&args(&["--json", "--json"])).err().map(|e| e.code),
            None,
            "repeating the same flag is harmless"
        );
        assert_eq!(
            parse(&args(&["--frob"])).err().map(|e| e.code),
            Some("unknown_flag")
        );
        assert_eq!(
            parse(&args(&["lint", "--tool", "--json"]))
                .err()
                .map(|e| e.code),
            Some("missing_value")
        );
    }

    #[test]
    fn every_command_doc_has_an_exit_zero_row_and_an_example() {
        for d in COMMAND_DOCS {
            assert!(COMMAND_NAMES.contains(&d.name));
            assert!(d.exit_codes.iter().any(|(c, _)| *c == "0"), "{}", d.name);
            assert!(!d.examples.is_empty(), "{}", d.name);
        }
        assert_eq!(COMMAND_DOCS.len(), COMMAND_NAMES.len());
    }

    #[test]
    fn quick_start_stays_small_in_both_formats() {
        let q = quick_start();
        assert!(q.text.len() <= 620, "{} bytes:\n{}", q.text.len(), q.text);
        let j = q.json.to_string();
        assert!(j.len() <= 700, "{} bytes:\n{j}", j.len());
    }

    #[test]
    fn resolve_tool_prefers_the_flag_and_names_the_failure() {
        assert_eq!(
            resolve_tool(Some("video"), Some("mcp__x__generate_audio")).ok(),
            Some(ToolKind::Video)
        );
        assert_eq!(
            resolve_tool(None, Some("generate_image_batch")).ok(),
            Some(ToolKind::Image)
        );
        assert_eq!(
            resolve_tool(None, None).err().map(|e| e.code),
            Some("missing_tool")
        );
        assert_eq!(
            resolve_tool(None, Some("generate_3d"))
                .err()
                .map(|e| e.code),
            Some("unknown_tool")
        );
        assert_eq!(
            resolve_tool(Some("3d"), None).err().map(|e| e.code),
            Some("unknown_tool")
        );
    }
}
