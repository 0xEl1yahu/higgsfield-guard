# Canonical Higgsfield contract reconciliation

Implementation status (2026-09-09): the Rust rules now consume the supported associations
as compiled constants. See [execution report](rule-regrounding-implementation-report.md)
for tests, exclusions and remaining session-verification limits. This document retains
the acquisition-time evidence; it is not a live schema cache.

Captured 2026-09-09. This is a design-time evidence and implementation-decision record, not a runtime schema validator. No generation, upload, cost preflight, purchase, or account-history query was submitted for this reconciliation.

## Sources and coverage

The [official MCP connection guide](https://higgsfield.ai/creator-hub/help-center/integrations/how-do-i-connect-higgsfield-to-ai-agent) supplies the endpoint and authentication route. The [official MCP page](https://higgsfield.ai/mcp) recommends CLI for coding agents. Installed the official CLI 1.1.24, completed its browser OAuth flow, and selected the sole available workspace. An access token obtained through the CLI was accepted by the official MCP endpoint; credentials stayed in process memory and were not written into repository evidence.

[Sanitized machine-readable capture](evidence/higgsfield-contract-2026-09-09.json) contains:

- MCP endpoint: https://mcp.higgsfield.ai/mcp; server Higgsfield AI 1.0.0; protocol 2025-03-26.
- `initialize`, `notifications/initialized`, and `tools/list`: 101 names on one page, no next cursor; all six in-scope generation input schemas and `models_explore` input schema retained. Unrelated tool schemas/descriptions omitted deliberately.
- `models_explore(action="list", type=..., limit=20, after=...)`: **79 models: 34 image, 39 video, 6 audio**. Image pages 20+14; video pages 20+19; audio page 6. Every terminal response has `has_more=false`; cursors are retained.
- Fresh `models_explore(action="get")` details for 16 fixture, carrier, dimension, and duration-exception models.
- CLI `model list --json`: 90 entries across all types (32 image, 34 video, 6 audio, 14 3d, 3 data, 1 text). Details for all 72 image/video/audio entries; no errors.
- CLI list is a bare array without a pagination flag. It is **not** a substitute for, or the same catalog as, MCP.
- Capture timestamps, versions, field types, options, descriptions, rules, and pagination evidence. Account/workspace identifiers, balances, tokens, session ids, and account-level unlimited entitlement state excluded.

The MCP schema/capability acquisition gate is complete for the current hook's six-tool scope. Remaining semantic unknowns below are explicit restrictions, not reasons to invent output guarantees. Model availability and schemas can change; refresh before implementing if this snapshot is no longer current.

## Contract matrix

All machine facts below use the dated capture. Paths are relative to the named tool's `inputSchema` or the named model detail. For catalog-only rows, find the model by `id` under `mcp.catalog[].pages[].items[]`. CLI detail paths use `cli.models[].schema.params[]`, selected by `name`.

| Tool/model | Field/path and canonical contract | Meaning / limit | Current repo | Required decision and tests |
|---|---|---|---|---|
| All three single generation tools | `properties.params.anyOf`: object or string; outer params required | JSON-string alternative exists; string schema does not itself require valid JSON | Resolver accepts object or JSON-encoded object | Preserve tolerance. Test valid object/string equivalence and malformed string silence |
| Single image/video | Object params require model, not prompt; image/video count integer 1–4 | Some workflows do not require prompt | Checks require prompt text to lint | Missing prompt remains silent; do not add universal prompt-required validation |
| Single audio | Object params require model and nonempty prompt; count number const 1 | Different from image/video | Broad current hook runs visual checks | ToolKind skips visual and proposed duration checks for audio |
| All three batches | `properties.requests`: 1–12 entries; items require index and params; item additionalProperties false | Entry wrapper is strict | Traversal accepts unusual/missing indices | Preserve fail-open tolerance, label malformed inputs as robustness fixtures |
| Batch params | Object only; requires model, plus prompt for audio; additionalProperties {}; count number const 1 | Strict wrapper does not imply strict inner model fields | JSON-string batch params are tolerated | Correct SPEC's overbroad single/batch assertion; retain string batch cases only as schema-invalid tolerance |
| Batch index | Integer 0–9007199254740991 | Caller-provided stable index | Positional fallback and Python-compatible formatting | Do not replace current local formatting under guise of canonical validation |
| Single and batch params | `additionalProperties: {}` | Accepting a field does not establish that a model honors it | Generic dimensions/negative_prompt treated too strongly | Separate shape acceptance, declared model control, and local textual consistency |
| Video single/batch | `aspect_ratio.description` documents explicit width/height as an alternative for deriving ratio | Dimension pair is not generally a pixel-size promise | Uses min(width,height) as generated short side | Remove generic model-output inference; Kling historical fixture loses resolution finding |
| Video single/batch | duration integer, safe-integer limits; description says omitted values default, unsupported values adjusted/clamped | Supplied duration need not equal rendered length | Proposed rule compares supplied number | Keep arithmetic advisory wording; no guaranteed truncation or effective-duration claim |
| Audio single/batch | duration positive number, required for two music/SFX models and omitted for TTS by description | Not the video integer contract | Proposed broad number parser | Do not apply video rule to audio |
| All 79 MCP models | No parameter named negative_prompt declared in any returned parameters list | Absence of published support is not proof of ignoring/rejecting passthrough | Conflict check active; placement disabled | Keep overlap as local text consistency only; do not recommend moving inline exclusions; retain disabled placement |
| kling2_6 | parameters duration [5, 10], sound boolean; ratio list; no resolution/width/height/negative_prompt declaration | No selectable size carrier established; public Kling guide separately lists 1080p ceiling | Historical 1376×768 payload emits output-size warning | Expect conflict only for historical fixture. Ceiling rule remains separately deferred; do not manufacture a resolution setting |
| wan2_6 | parameters[name=quality]: string, 720p/1080p, default720p | Explicit output resolution/quality carrier | Not read | Add model-scoped carrier, submitted-tier wording; 4K vs 720p positive, 720p vs 720p silent, stray width/height cannot override |
| kling3_0 | mode std/pro/4k, defaultstd; 4k explicitly means 4K | std/pro do not establish numeric resolution | Proposed mode mapping too vague | Recognize 4k only as a known matching tier; no invented mode 1080p/720p and no assumed ceiling for std/pro |
| kling_video_edit | mode std/pro/4k; no duration parameter in model detail | Edit mode quality; source video determines other properties | Not specifically handled | Same limited mode recognition; do not use supplied passthrough duration as render length |
| soul_2, soul_cinematic, soul_v2 | quality 1.5k/2k, default2k | Output tiers, not a proven 1536/2048 short side | Not read | Use these exact MCP ids; compare nominal K tiers only, never assert fixed pixels. CLI text2image_soul_v2 is not an MCP id |
| nano_banana_pro | resolution 1k/2k/4k, default2k | Nominal output tiers | 2k not read; proposed2048 short side overclaims | 4K vs 2k can flag tier mismatch without dimensional arithmetic; add rectangular-ratio regression |
| seedream_v5_pro | resolution 1k/1.5k/2k; width/height optional number, minimum1, described as stored with generation | Storage wording does not establish overriding render control or precedence | Generic dimensions take precedence over carriers | Prefer recognized tier; suppress size inference from these dimensions |
| topaz_image, topaz_image_generative | output_width/output_height required positive numbers, described as target pixels; CLI uses integer | Explicit requested pixel targets, different field names | Not recognized | Add narrow model-scoped target-dimension source if prompt present. Reject floats as a documented conservative local subset. Never call raw width/height equivalent |
| marketing_studio_video | resolution 480p/720p/1080p plus optional width/height described as output dimensions | Generic video envelope also describes ratio derivation; override precedence unresolved | Generic fallback assumes output dimensions | Use unambiguous tier alone; when multiple declared size controls disagree, suppress output inference until precedence verified |
| clipify | max_height: processing maximum, 144–2160; segment_seconds2–60 | Source processing cap and segmenting, not guaranteed output size or global duration | Generic future maps risk false association | Explicit exclusions; do not alias max_height or segment_seconds |
| minimax_hailuo | resolution strings512/768/1080; duration6/10 | Variant/media/duration constraints apply | Partial map; 1080 missing | Preserve full inventory. No naked-number-to-short-side claim without units proof; defer ambiguous mapping, retain CLI rule evidence |
| minimax_h3 / minimax_h3_max | H3 resolution 2K; Max480p/768p | Different vocabularies even within one provider | Incomplete generic map | H3 uses nominal K-tier comparison; include768p in p-tier ordering for verified Max carrier |
| seedance_2_0 | resolution 480p/720p/1080p/4k, fast limited480p/720p | Conditional support; not all listed options valid together | No condition validation | Do not state rendered size for invalid combos. Preserve combination tests; no generic CEL evaluator |
| seedance_2_5, ad_multiplier | mode video_edit ignores duration/aspect_ratio; video_extension follows source ratio | Supplied duration may not control output; extension total semantics not established | Proposed duration check initially indiscriminate | Suppress duration rule in video_edit and conservatively video_extension; test both ids and compare valid t2v controls |
| gpt_image_2, openai_hazel and other generic quality fields | low/medium/high etc | Quality/detail, not pixels | Risk of value-only carrier inference | Model+field+option allowlist; do not equate high or pro to numeric resolution |
| Any model with 1k/1.5k/2k/4k resolution | parameters options | Nominal K tiers; aspect-ratio-specific geometry not given | Proposed universal short-side map | No 1k=1024, 1.5k=1536, 2k=2048 short-side promises. Compare like tiers; ambiguous mixed-unit cases silent |
| Matched MCP tools vs CLI | .claude/settings.json matcher covers mcp__...__generate_* only | CLI shell commands and dedicated upscalers/workflow tools bypass it | Project-local Claude Code MCP hook | Explicit coverage boundary. CLI authentication is evidence access, not deployment. Separate future adapter design if requested |

## Binding implementation decisions

### Resolution comparison and message contract

Replace the single universal `short_side: i64` model with typed comparisons:

1. **Nominal K tier**: 1k < 1.5k < 2k < 4k, only on model/field associations that publish those options. A 4K prompt against a selected2k tier is a supported local tier contradiction. It does not say the render is2048 pixels on its short side.
2. **P-labelled video tier**: order declared 360p/480p/720p/768p/1080p/2160p. Comparing a 4K prompt to an explicitly selected sub-4K video tier is a documented local resolution-label convention, not universal pixel geometry. Never compare nominal image2k to 1080p by inventing a common short side.
3. **Explicit target pixels**: Topaz output_width/output_height are requested pixel controls. Compare min(targets) to the retained claim threshold convention (4K→2160, 1080p→1080, 720p→720, 480p→480), clearly labeled as a local comparison to requested targets, not a promise of successful output. Bare width/height, stored dimensions, and source-processing caps are excluded.
4. **Unknown or conflicting controls**: silence. Known model with undeclared resolution, unknown model, unsupported option, and unresolved same-model precedence must not become a generated-output assertion.

Messages identify actual field and supplied value: `prompt says "4K" but resolution selects the "2k" output tier. Choose a supported higher tier, or revise the claim.` For Wan name quality; for Soul name quality and nominal tier. For Topaz describe requested target dimensions and suggest changing output_width/output_height. Do not advise raising a field that model does not expose. Omitted defaults are not synthesized by the initial rule.

Retain the approved removal of 8k/UHD/full hd claims, quote exclusions, narrow Unicode folding, original spelling, and conservative parser boundaries. Exact evidence-based associations are checked-in constants; there is no hook-time network access, schema service, or external model-list loader.

### Duration scope

Compare supplied seconds, not effective seconds. Gate to known video models with declared duration semantics, excluding source-driven/ignored modes above. Unsupported numeric inputs can remain a fail-open local subset; fractional tolerance is not evidence that the video envelope accepts fractions. Keep missing/default duration silent. The diagnostic should say `prompt describes 15s of action but the supplied duration is 5s. Check the model's supported duration or retime the beats.`

### Historical evidence and positive controls

Keep captured input bytes unchanged. The Kling fixture expects exactly prompt_negative_conflict after output-size inference is removed. The known-clean Seedance fixture stays silent. Add an explicit nano_banana_pro2k or wan2_6quality720p resolution positive; pair quoted cases with identical unquoted cases to avoid vacuous passing tests. A synthetic case with resolution+conflict+duration must name a supported video model and valid controls.

Do not overwrite old golden results en masse. Classify each flip: approved lexicon, quote fix, canonical field correction, new carrier, tool gating, or new duration. Preserve schema-invalid robustness cases separately from schema-valid end-to-end samples.

## Surface differences and unresolved questions

MCP-only relative to the CLI's 72 image/video/audio ids: soul_2, cinematic_studio_2_5, marketing_studio_image, ms_image, autosprite, soul_v2, nano_banana_2, cinematic_studio_3_0, cinematic_studio_video, cinematic_studio_video_v2, marketing_studio_video, higgsfield_preset.

CLI-only relative to MCP: text2image_soul_v2, nano_banana_flash, nano_banana_2_relight, nano_banana_2_skin_enhancer, nano_banana_2_ai_stylist.

These are observed id differences, not a proven alias mapping. Match the MCP ids for this MCP hook. The live nano_banana_2 is not interchangeable with nano_banana_pro just because an older published CLI snapshot used a different label.

The MCP catalog expresses duration ranges and descriptions; CLI detail additionally exposes validation rules but sometimes uses string enum members with integer field types. Preserve raw evidence. Do not silently coerce source schemas, declare one transport a complete substitute for the other, or build a universal validator from this capture.

Still unresolved/deferred: exact K-tier geometry by model/aspect ratio, naked Hailuo resolution units, marketing dimension-vs-resolution precedence, effective clamped output durations, extension final-duration semantics, model ceiling enforcement, ratio mismatch lint, and interception outside the six configured MCP tools. These restrictions do not block quote/noise fixes or the verified comparisons above.

The official public connection guide's blanket paid-generation wording differs from the fresh tool schemas' free-trial unlimited-choice fields. No cost behavior was exercised. Preserve the account-independent schema facts; do not infer free access, billing eligibility, or a spend cap.

## Python-to-Rust status

The migration is already complete: hook cutover 58c046c, golden harness replacement 95edc00, Python archive a1b0e68. Runtime remains the Rust binary. Historical Python is evidence, not a second production implementation or future migration workstream. The subsequent implementation updates Rust, SPEC and reviewed golden expectations; see the execution report linked above.

## Verification and next implementation boundary

- Verify JSON parses, all 6 tool schemas retained, 101 unique names, every MCP catalog cursor exhausted, 79 unique scoped model ids and 72 successful CLI details.
- Preserve full HIGGSFIELD_PROMPT_RESEARCH.md as historical research; link this dated reconciliation instead of rewriting its measurements.
- Amend SPEC§5.1/§5.2/§5.5/§6/§9 and the associated code/tests together during implementation.
- Refresh the complete git-style PLAN diff from the original plan; inspect both forward and reverse applicability.
- Do not interpret successful schema queries as a generation test, installed-hook trace, or completed implementation.
