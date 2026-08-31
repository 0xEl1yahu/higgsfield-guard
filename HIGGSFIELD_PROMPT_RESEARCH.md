# Higgsfield Prompting Guidance vs. `higgsfield-lint` — Research and Gap Analysis

**Status:** research only. No implementation, oracle, test, hook-config, or `SPEC.md` change is proposed as part of this document, and none was made.
**Access date for every web source below:** 2026-08-31.
**Binary audited:** `target/release/higgsfield-lint`, built 2026-08-31 22:33:52, newer than every file in `rust/src/` and `data/`, so the probes in §8 exercise current logic.

---

## 1. Executive summary

1. **Higgsfield's own official guidance directly contradicts lint rule 5.3 (`negation_placement`).** The help-center prompting article tells users to keep exclusions in the positive prompt — "naming a few key things to avoid (like 'no readable text' or 'no plastic skin') can help on detailed shots" — and every long-form official example prompt Higgsfield publishes does exactly that, inline, dozens of times per prompt. The Seedance 2.5 guide goes further and reports that inline exclusions are the *highest-leverage* part of a prompt: explicitly excluding what shouldn't appear "prevents more failures than adding more positive description does." Rule 5.3 tells the model to move that text somewhere else.

2. **No Higgsfield model in the live catalog declares a `negative_prompt` parameter — not one of ~30 video models, not one of ~33 image models, including `kling2_6`.** `data/negative_prompt_models.json` seeds `kling2_6` from generation history (the field was *sent*), not from evidence it is *honored*. Rule 5.3's gate is therefore built on the presence of a field that the platform does not publish support for, and its remedy ("Move them there") may silently discard the exclusion.

3. **Rule 5.1 (`resolution_claim`) fires on Higgsfield's own house style.** Higgsfield's official ad-production guide opens every scene prompt with `Style: 8K IMAX commercial, 16:9 widescreen. … Technical: 24fps smooth motion. 8K detail.` Fed to the linter with a realistic `resolution: "1080p"`, that prompt produces `resolution_claim: prompt says "8K"…`. "8K IMAX" here is a stylistic register, not a render-size assertion, and Higgsfield writes it deliberately.

4. **Rule 5.1 also has a large, closable blind spot.** It is silent whenever neither `width`/`height` nor a recognized `resolution` value is present — which is the normal shape of a `kling2_6` call, since `kling2_6` declares neither `resolution` nor `width`/`height`. Its published ceiling is 1080p (help center) and its only knobs are `duration` and `sound`. A "4K" claim on `kling2_6` is a *documented* contradiction the rule cannot see. Meanwhile the historical `width: 1376, height: 768` that seeded the rule are parameters `kling2_6` does not declare at all, so the "768px short side" the finding asserts was probably never the real output.

5. **Rule 5.2 (`prompt_negative_conflict`) has confirmed common-noun false positives** on the single-token entries: `pan` (a cast-iron pan), `text` (`the text "OPEN"` plus `misspelled text` in the negative — the exact pairing Nano Banana's help article recommends), `sharp` (`sharp focus` vs `not sharp`, which is agreement, not conflict).

6. **The best-evidenced deterministic rules Higgsfield's guidance supports are not implemented.** Higgsfield tells users to state total duration and aspect ratio *inside* the prompt ("state the number of shots, the total duration, and the aspect ratio at the top of the prompt"). That makes prompt-vs-parameter contradictions on **duration** and **aspect ratio** first-party-grounded, purely mechanical, and currently invisible: a prompt ending `Total: 15s / 6 shots / 16:9` sent with `duration: 5` and `aspect_ratio: "16:9"` produces no finding today.

7. **Higgsfield does use non-natural-language prompt syntax**, which matters for tokenization and for any future rule: `@element-name` references, `[VFX: …]` bracket inserts, `Shot N:` / `CUT N —` labels, ALL-CAPS section headers (`GLOBAL STYLE`, `FIRST FRAME AND BLOCKING`), timecode spans (`0–3s:`, `around 2.4 to 4.0 seconds`), the `Hard cut.` terminator, and the trailing `Total: 15s / 6 shots / 16:9` line.

8. **The observability gap is real and separable.** Exit 127 (missing/stale binary) and exit 0 with no output ("ran, found nothing") are indistinguishable to the user. This is fixable inside the existing contract via the already-scaffolded `doctor` subcommand, with no change to fail-open behavior in the hook path.

---

## 2. Current-system understanding

### 2.1 Contract (from `SPEC.md`, verified against source)

| Invariant | Where enforced | Verified |
|---|---|---|
| Advisory only; never `deny` | `rust/src/main.rs` emits only `hookSpecificOutput.additionalContext` | yes — no `permissionDecision` anywhere in the crate |
| Always exit 0 on the hook path | `main.rs` ends in `std::process::exit(0)` unconditionally | yes |
| Never write stderr on the hook path | `panic::set_hook(Box::new(|_| {}))` + `catch_unwind` | yes; `panic = "unwind"` pinned in `Cargo.toml` with a comment forbidding `abort` |
| Fail open on any malformed input | every stage is `?`/`ok()?` into `None` | yes; `rust/tests/fail_silent.rs` covers it |
| Rust-only runtime path | `.claude/settings.json` invokes `$CLAUDE_PROJECT_DIR/target/release/higgsfield-lint` | yes |
| Python is a fixed parity oracle, never the runtime | `src/higgsfield_lint.py`; nothing in the hook path calls it | yes |
| Deterministic, repeatable output | no clock, no RNG, no network; only `data/negative_prompt_models.json` is read | yes |
| `doctor` is the one non-hook path allowed to exit non-zero / print | `main.rs` dispatches on `args().nth(1) == "doctor"` **before touching stdin** | yes; any other argument falls through to the hook path |

### 2.2 Hook wiring

`.claude/settings.json` matcher: `mcp__.*__generate_(image|video|audio)(_batch)?$`.
The live Higgsfield MCP server exposes `generate_image`, `generate_video`, `generate_audio`, `generate_image_batch`, `generate_video_batch`, `generate_audio_batch` — all six match. Matching on tool shape rather than server id is correct and currently working.

`main.rs` reads **only** `payload.tool_input`. It never reads `payload.tool_name`, so no check can currently be scoped per tool. That is a limitation, not a bug — see §10 (HFR-006) and §14.

### 2.3 The three rules as implemented

**`resolution_claim`** (`checks.rs::check_resolution_claim`)
- Prompt lexicon → min short side: `8k`→4320, `4k`/`uhd`→2160, `1080p`/`full hd`→1080, `720p`→720, `480p`→480. Highest claim wins; ties break first-encountered.
- Actual = `params.resolution` mapped through `{4k, 1080p, 720p, 480p}`, else `min(width, height)` when both are JSON integers (not bools, not floats) and `max(width, height) >= 128`.
- Fires when claimed > actual. Silent when actual is unresolvable.
- Match is a manual, allocation-free, char-boundary scan (`find_term_match`) with `[a-z0-9]` boundary semantics; the case-insensitive variant returns the prompt's own spelling so the message quotes `"4K"`, not `"4k"`.

**`prompt_negative_conflict`** (`checks.rs::check_prompt_negative_conflict`)
- Requires both `prompt` and `negative_prompt` to be strings.
- `strip_negations` removes `(no|without|avoid|not) + up to 3 words` spans from the **prompt only**, then both fields are lowercased and scanned for 14 curated phrases: `slow motion, slow-mo, motion blur, static, blurred, blurry, sharp, text, close-up, wide shot, zoom, pan, handheld, grain`.

**`negation_placement`** (`checks.rs::check_negation_placement`)
- Gate ("supports negative_prompt") = `negative_prompt` present **and non-empty**, **or** `model` ∈ `data/negative_prompt_models.json` (= `["kling2_6"]`).
- Pattern is the narrower `(no|without|avoid) + up to 3 words` — `not` deliberately excluded.
- Noise filter: bigram stoplist `("no","more") ("no","longer") ("no","matter")`, plus bare two-word `without + <gerund>`.
- Lists at most 3 spans, then `and N more`.

### 2.4 Orchestration and known divergences

- Batch path: non-empty `requests` array is authoritative — the top-level `params` is never consulted, even when every entry is skipped (`lint.rs`, mirrored from oracle line 249).
- `params` accepted as object or JSON-encoded string (`resolve_params`).
- Accepted divergence: compound / non-integer-scalar `index` renders differently in Python `repr` vs `serde_json` `Display`; parity sweeps only integer indices.
- **SPEC inaccuracy found:** §9 says "all three tool schemas declare `params` as `anyOf: [object, string]`". That is true of `generate_image` / `generate_video` / `generate_audio`, but **not** of the batch tools: `generate_video_batch.requests[]` sets `additionalProperties: false`, requires both `index` and `params`, and types `params` as an object with no string alternative. Harmless (the extra tolerance only ever fails open), but the claim is over-broad.

---

## 3. Research method and evidence classes

Method: web search restricted where possible to `higgsfield.ai`, then direct retrieval of `higgsfield.ai/sitemap.xml` → `creator-hub/sitemap.xml`, `blog/sitemap.xml`, `sitemap-marketing.xml` to enumerate first-party prompting material rather than relying on search ranking. 33 first-party pages retrieved and read in full. Platform-side evidence was taken from the live MCP tool schemas and from `models_explore` (`action:"list"`, `type:"video"` and `type:"image"`), which is read-only.

Every claim below carries one of these classes:

| Class | Meaning | Weight for rule design |
|---|---|---|
| **A** | First-party **explicit** — Higgsfield states it in prose on its own site | Strongest. Can ground a rule on its own. |
| **A′** | First-party **machine-readable** — MCP tool schema or `models_explore` catalog | Strong but versioned; a snapshot goes stale, so any rule built on it needs a data file and a staleness story. |
| **B** | First-party **inferred from examples** — a pattern consistent across official example prompts, never stated | Can support a rule only alongside A or A′; never alone. |
| **C** | **Third-party / secondary** | Context only. Never grounds a rule. |
| **D** | **Our engineering proposal** — no Higgsfield backing | Must be justified as an internal agency convention, and labeled as one. |

Secondary sources encountered and deliberately **not** used as guidance: `blog.segmind.com`'s Higgsfield prompt-format post, `filmart.ai`'s "Ultimate Higgsfield Guide", a `higgsfieldai.gumroad.com` product page, the `OSideMedia/higgsfield-ai-prompt-skill` GitHub repo, and `higgesfieldcannesmanual.netlify.app` (a third-party domain whose name misspells "Higgsfield"). None is first-party; none is cited as guidance anywhere below. `github.com/higgsfield-ai/higgsfield` is a same-org repo but is an unrelated distributed-training library, not the media product.

Marketing copy is likewise excluded from the guidance classes. In particular the "AI Negative Prompt Builder" tile on `higgsfield.ai/ai-prompt-generator` is a feature bullet on a landing page, not prompting guidance, and it names no model that accepts the field.

---

## 4. Higgsfield prompt anatomy

### 4.1 The one canonical first-party statement (class A)

`How do I write a good prompt?` (help center, published 2026-07-30) is the closest thing to a normative Higgsfield prompt spec. Its core claim: "the single biggest thing you can do to get a better result is simple: be specific." It defines a **layer model** for stills — *subject* (who/what is in frame), *details* (appearance, clothing, expression), *environment* (where), *style* (photorealistic, cinematic…) — and adds *camera* and *mood* for video. It is explicit that you need not fill every layer every time.

For video it says prompting "changes completely": when animating an image, "your prompt stops describing what things look like and starts describing what happens." It then names six blocks — Style and mood / Cinematography and camera / Lighting and color / Action (beat by beat) / Audio (score, ambient, dialogue with timing) / Shot list (sequence with durations) — and calls a video prompt "more like a shot list than a sentence."

Its four "habits": say what you want rather than long lists of what you don't (with the exception quoted in §6); keep it focused for simple tasks; avoid contradictions between prompt and input image; write in English, with the noted exception that Chinese-developed models (Seedance, Kling) "sometimes respond better to Chinese for nuanced motion." Iteration advice: "generate, look, change one thing, generate again."

### 4.2 The block template actually used in official examples (class B, and class A for Seedance 2.5)

The article's own worked cowboy example is far denser than its six named blocks. Its labeled sections, in order as published: **Style, Cinematography, Lighting, Color, Camera, Skin, Acting, Physics, Composition, Continuity, Editing, Technical, Audio, Character description, Location, Hero prop, Mood and tempo.** The same skeleton recurs, near-identically, in the official ad-production guide's reusable "style header."

For Seedance 2.5 the ordering is stated outright (class A): "GLOBAL STYLE, SCENE, CHARACTERS, LOCATION, FIRST FRAME AND BLOCKING, a shot-by-shot breakdown, OPTICS, CAMERA, PHYSICS, LIGHTING, then AUDIO last. Labeled sections outperform one continuous paragraph."

The Sora 2 guide publishes a third, overlapping template: Format & Style / Camera / Lens / Main Subject(s) / Wardrobe & Props / Location / Lighting & Palette / Actions & Camera Beats (timecoded) / Montage Plan / Dialogue / Sound & Foley / Finish.

### 4.3 Guidance by component

| Component | Higgsfield guidance | Class | Source |
|---|---|---|---|
| Subject / identity | Exact age, build, wardrobe, plus one distinguishing detail; say plainly it stays identical across cuts. Reference images "hold identity across cuts more reliably than a text description does." | A | help center; mistakes guide; Seedance 2.5 |
| Identity across generations | Train once as **Soul ID** (real person) or save as an **Element** and reference it; `@character` holds a face only *within* one clip | A | Seedance help; Soul help |
| Appearance / wardrobe | Its own labeled block (`CHARACTERS`, `Wardrobe & Props`); may be short when a reference carries it | A | Seedance 2.5 FAQ; Sora 2 |
| Action / motion | One clear action per beat, written as start position → end position, not a mood word; name every point of contact | A | mistakes guide |
| Environment | `LOCATION` is its own block, separate from the people: "A vague location is the most common reason a multi-shot sequence starts drifting between cuts." | A | Seedance 2.5 |
| Composition / blocking | `FIRST FRAME AND BLOCKING` pins exact starting positions and facing before any motion; keep screen direction and the 180° axis fixed | A | Seedance 2.5; mistakes guide |
| Camera framing / angle | Define the camera as a physical setup — field of view, distance, static vs handheld vs moving — "not a mood" | A | mistakes guide |
| Lens language | Focal length, aperture, shutter; per-shot lens locks | A | Sora 2; Nano Banana Pro; Cinema Studio 2.0 (12–135 mm guidance) |
| Camera movement | Seedance reads these terms directly: *dolly in, truck left, arc shot, push in, pull back wide, handheld follow, crane up, orbital move*. For POV, state what the camera is **not** doing. One camera move per shot. | A | Seedance help; Seedance 2.0 guide; Sora 2 |
| Lighting / color | One named source, one direction, one color temperature; rule out doubles explicitly. Color as a 60/30/10 split in official examples. | A (source/direction), B (60/30/10) | mistakes guide; ad guide |
| Style / references | Define the look up front and reuse it verbatim as a prefix across every shot of a sequence | A | Sora 2; ad guide |
| Mood / atmosphere | Never a bare mood word — tie it to a visible action: a held breath, a gaze, a mouth that doesn't quite smile | A | mistakes guide |
| Temporal sequencing | Label shots `Shot 1`, `Shot 2`…, each with its own action, camera position, and end point, closing with `Hard cut.`; or break the clip into timed segments (`0–3s`, `3–6s`) | A | Seedance 2.5 FAQ; Seedance 2.0 guide |
| Prompt ordering | **Contradictory across sources — see §5.4** | A | — |
| Prompt length | "As long as the scene needs." One shot can be a few sentences with a reference; a multi-shot sequence needs every section. Match complexity to the task. | A | Seedance 2.5 FAQ; help center |
| Specificity | The single most-repeated instruction on the site | A | help center |
| Negation / exclusions | See §6 | A | — |
| Resolution / quality words | See §7 | A/A′ | — |
| Aspect ratio | State it numerically in the prompt (`16:9`, `2:3`) and at the top for Seedance | A | Nano Banana Pro guide; Seedance 2.0 guide |
| Duration | State total duration at the top; official examples close with `Total: 15s / 6 shots / 16:9` | A (state it), B (that exact form) | Seedance 2.0 guide |
| Text / logos / brands | Put exact rendered text **in quotes**, and describe font, size, placement. Separately, official prompts suppress unwanted typography with inline exclusions (`no readable text`, `no brand logos`, `zero AI-text artifacts`). | A | Nano Banana help; Nano Banana Pro guide; ad guide |
| Contradictions | "Make sure your prompt and your input image agree" — named as one of four habits | A | help center |

### 4.4 Non-natural-language syntax (answers "do any features use special prompt structures?")

Yes. All class A unless noted:

- `@element-name` — Elements referenced inline. Kling requires it (a directly connected image is treated as a *start frame*, not a character reference). Seedance uses typed references `@character`, `@style`, `@motion`, `@audio`.
- `@image` used as "first keyframe and style reference" inside a Seedance animation prompt (class B).
- `[VFX: …]` — bracketed inline VFX spec, recommended for Orb/POV formats so the perspective description isn't broken.
- `Shot 1:` / `Shot 2:` … and `CUT 1 —` labels; `Hard cut.` as a shot terminator.
- ALL-CAPS block headers: `GLOBAL STYLE`, `SCENE`, `CHARACTERS`, `LOCATION`, `FIRST FRAME AND BLOCKING`, `OPTICS`, `CAMERA`, `PHYSICS`, `LIGHTING`, `AUDIO`, and in examples `STILLNESS LOCK`, `LENS LOCK`, `SCREEN DIRECTION LOCK`, `SEA AND WIND EVENT TRACK`.
- Timecode spans: `0–3s:`, `3.0s to 7.2s`, `around 2.4 to 4.0 seconds`.
- Trailing spec line: `Total: 15s / 6 shots / 16:9`.
- Double quotes around literal text to be rendered in the image, and around spoken dialogue lines.

**Consequence for the linter:** naïve word scanning over these prompts will encounter `@tokens`, bracketed inserts, ALL-CAPS runs, ratio tokens containing `:`, and quoted literals. `find_term_match`'s `[a-z0-9]` boundary class already handles most of this safely, but any *new* rule must be designed against this syntax rather than against plain prose.

---

## 5. Model and workflow differences

### 5.1 Catalog facts that matter (class A′, `models_explore`, retrieved 2026-08-31)

- **No video model and no image model declares a `negative_prompt` parameter.** Checked across ~30 video and ~33 image catalog entries.
- Resolution is expressed in **at least five incompatible vocabularies**:
  - video `resolution`: `480p / 720p / 1080p / 4k` (seedance_2_0, cinematic_studio_3_0), `720p / 1080p` (flux_3_video, kling3_0_turbo), `480p / 768p` (minimax_h3_max), `512 / 768 / 1080` (minimax_hailuo), `2K` (minimax_h3), `360p / 720p / 1080p / 4k` (gemini_omni_flash_1_1), `1080p / 2160p` (topaz_video), `1080p / 2k / 4k` (bytedance_video_upscale);
  - a different parameter name entirely: `quality` (`720p / 1080p`) on wan2_6; `mode` (`std / pro / 4k`) on kling3_0;
  - image `resolution`: `1k / 2k / 4k`, and `quality`: `1.5k / 2k` (Soul) or `low / medium / high` (GPT Image 2, ms_image).
- **`kling2_6` declares only `duration` and `sound`.** No `resolution`, no `width`, no `height`. Its aspect ratios are `16:9 / 9:16 / 1:1`. The help center's Kling table gives its resolution as **1080p**.
- `width` / `height` are declared by exactly two models in the catalog: `marketing_studio_video` ("Optional explicit output width/height") and `seedream_v5_pro` ("Optional output width stored with the generation" — wording that suggests metadata, not a render directive). `topaz_image` uses `output_width` / `output_height`.
- Cross-parameter constraints exist and are machine-readable, e.g. `seedance_2_0`: "4k/1080p require mode='std'; mode='fast' supports 480p/720p only"; `minimax_hailuo`: "'512' is incompatible with end_image, and is not supported by the 'minimax-2.3' variants."
- `params` on the single-generation tools is `anyOf: [object, string]` with `additionalProperties: {}` — arbitrary top-level model params pass through unvalidated, which is exactly how `negative_prompt`, `width`, and `height` reached `kling2_6` in the first place.

### 5.2 Documented per-model prompting differences (class A)

- **Seedance**: structured prompt, image-to-video at 720p first, one change per iteration, then rerun the identical prompt at 1080p/4K and add audio last. Camera-term whitelist as in §4.3. Up to 9 images + 3 video + 3 audio references. Seedance 2.5 adds ≤30 s clips, native audio, up to 50 references, and region edit.
- **Kling**: scene-based multi-shot; start/end frames anchor transitions; chain shots by reusing the final frame; genres and speed ramps are *settings*, not prompt text.
- **Nano Banana Pro**: five named prompt variables (Subject, Composition, Action, Location, Style — the page says "six variables" and lists five, an internal inconsistency), short one-line prompts in every published example, command-style phrasing with polite filler removed, exact text in double quotes, camera-gear tags to force photorealism, seed reuse for series consistency.
- **Sora 2**: two explicit formulas — *Short* ("let the model handle lensing and lighting") vs *High-Control* (the full block template) — plus a dedicated `Dialogue` block for lip-sync accuracy and "one camera move and one subject action per shot."
- **Soul**: when a reference image is attached, **the prompt field becomes unavailable**. Soul HEX carries color, Moodboards carry style, Soul ID carries identity — all out-of-prompt controls.
- **Cinema Studio**: most direction moves out of the prompt into structured settings (sensor, lens, focal length 12–135 mm, aperture, genre, speed ramp, up to 3 stacked camera movements, color grade). 3.5 splits per-shot camera settings from global project settings.
- **Canvas**: Seedance nodes require the reference's role to be described "at the beginning of the prompt"; Kling nodes need `@element-name` because a connected image is read as a start frame.
- **Supercomputer**: the documented escape hatch from prompt craft — plain language in, routed and expanded automatically.
- **MCP** (our path): always deducts credits; Unlimited access and free generations apply on the web only.

### 5.3 Contradictions and drift across official sources — flagged as required

| # | Conflict | Sources |
|---|---|---|
| 1 | **Where style goes.** Seedance help center: "subject and action first, setting and lighting second, camera move third, mood or style **last**." Seedance 2.5 guide: "**GLOBAL STYLE comes first** … Everything after this section needs to match it." Both are current (2026-08-01 and 2026-08-13). | help center vs blog |
| 2 | **Block count.** The help-center article names six video blocks; its own worked example uses seventeen labeled blocks. | within one article |
| 3 | **Prompt length register.** Nano Banana Pro's published examples are single lines (≈15 words); Seedance 2.5 / Sora 2 / the ad guide publish 500–1,500-word block prompts. Both are presented as correct practice for their model. | across guides |
| 4 | **Variable count.** Nano Banana Pro guide: "Include these **six** variables," then lists five. | within one page |
| 5 | **Aspect ratio inside one official prompt.** The Seedance 2.0 "Desert Hero" animation prompt ends `… 2.35:1, 24fps.` and then `Total: 15s / 1 shot / 16:9`. | within one prompt |
| 6 | **Failure cost.** `Why AI Video Generations Fail` (2026-08-09): "A failed generation on a standard credit plan still draws down the balance the same way a successful one does." `SPEC.md` §1 measured the opposite from transaction history (every failure refunded in full). Not a prompting matter, but it bears on §13 of the spec. | blog vs our own measurement |
| 7 | **Negative prompts.** No prose guidance recommends them; one marketing tile advertises a builder for them; exactly one published example uses the field. See §6. | across site |

---

## 6. Official guidance on negative prompts

This is the decisive section for rule 5.3, so the evidence is laid out in full.

**What Higgsfield explicitly says about exclusions (class A).** From the help-center prompting article, under "What habits make any prompt better?":

> "Say what you want, not what you don't. Positive descriptions generally work better than long lists of 'no this, no that,' though naming a few key things to avoid (like 'no readable text' or 'no plastic skin') can help on detailed shots."

Both example exclusions are given as **positive-prompt text**. The article never mentions a `negative_prompt` field.

**What the Seedance 2.5 testing section concludes (class A).** Under "What the Testing Shows":

> "The GLOBAL STYLE and AUDIO sections function as guardrails more than instructions. Explicitly excluding what shouldn't appear, modern artifacts, music, dialogue, prevents more failures than adding more positive description does."

`GLOBAL STYLE` is defined earlier on the same page as the section carrying "anything that shouldn't appear." That is an explicit instruction to put exclusions in the positive prompt, and a claim that doing so is the highest-yield part of the prompt.

**What official example prompts do (class B, overwhelming).** Inline exclusions appear in essentially every long-form official prompt:
- cowboy example: `NO body-mounted rig`, `NO fisheye, NO warp`, `NOT bright, NOT flat, NOT studio`, `NOT teal-orange`, `NO glowing eyes`, `NO on-screen text, NO subtitles`, `no brand logos`, `zero AI-text artifacts`;
- Seedance 2.0 pro tip: add `"no 3D, no cartoon, no VFX"` to force realism; POV pro tip: `"No cuts, no zoom, natural head movement"` — "that instruction alone is what keeps the perspective locked";
- Seedance 2.5 drama prompt: `No logos, no readable text on anything`, `No music, no narration, no subtitles`;
- ad guide style header: `Photorealistic — no 3D render, no game engine. … No floating props. … No jitter. … No music. No subtitles.`;
- the linter's own clean fixture, `tests/fixtures/seedance_tree_clean.json`: `no camera movement, no zoom, no pan. No text, no lettering, no words at any point.`

**What Higgsfield says about the `negative_prompt` field.** Almost nothing.
- `higgsfield.ai/ai-prompt-generator` has an "AI Negative Prompt Builder" tile: "drafts negative prompts to remove unwanted elements. Cleaner results in fewer tries." Marketing copy; no model named; excluded from class A per §3.
- `higgsfield.ai/text-to-video-ai` mentions refining "with reference clips, presets, or negative prompts." Landing-page copy.
- Exactly **one** published example anywhere in the corpus uses the field: a still-image prop sheet in the ad guide, written as a labeled line — `Negative prompt: brand logos, watermark, hand, cluttered background, harsh shadows, blurry, distorted letters, misspelled text, heavy wear, large rips, holes, oversized patches, large patches, tiny small bag`. One occurrence, against dozens of inline exclusions in the same document, on an image model.
- No help-center article, no model page, and no catalog entry documents a `negative_prompt` parameter for any model.

**Conclusion.**

| Question | Answer | Class |
|---|---|---|
| Does Higgsfield recommend moving exclusions to `negative_prompt`? | **No.** No first-party source says this. | A (absence across the full first-party corpus) |
| Does Higgsfield recommend inline exclusions in the positive prompt? | **Yes, explicitly**, with the caveat that long "no this, no that" lists are worse than positive description. | A |
| Does any model publish `negative_prompt` support? | **No**, across ~63 catalog entries. | A′ |
| Is `negative_prompt` usable at all? | It passes the schema (`additionalProperties: {}`) and one official image example uses it. Whether any model honors it is **unverified from any first-party source.** | A′ + B |

Rule 5.3 is therefore **not supported by, and is in tension with, first-party guidance**, and its remedy sentence — "Move them there; models follow negation poorly in the prompt" — asserts a mechanism Higgsfield's own testing writeup contradicts.

---

## 7. Official guidance on resolution and quality language

**Structured resolution is a parameter, and Higgsfield treats it as one (class A).** The Seedance help article's workflow is explicit: prototype at 720p, lock the prompt, "then run it at 1080p or 4K with the identical prompt." Nano Banana: "Start at 1K when iterating, scale to 2K or up to native 4K for final assets." `Why AI Video Generations Fail`: "Start at a lower resolution, 480p or 720p rather than 1080p or 4K." In every case the resolution is a *setting*, changed while the prompt stays identical — which is precisely the premise rule 5.1 relies on.

**But resolution words in the prompt are used as a stylistic register, deliberately and officially (class A/B).** The ad guide's reusable style header — glued to *every* scene prompt in that production — reads `Style: 8K IMAX commercial, 16:9 widescreen.` and `Technical: 24fps smooth motion. 8K detail.` No Higgsfield video model outputs 8K. The phrase is doing the same job as `ARRI ALEXA aesthetic`, `35mm film quality`, and `shot on medium format camera` elsewhere in the corpus: naming a look, not asserting a pixel count. The Seedance 2.0 library's shared prefix is `… cinematic lighting, photorealistic, 35mm film quality, professional color grading, sharp focus, high detail texture, film grain, depth of field mastery, ARRI ALEXA aesthetic`.

Conversely, the mistakes guide's *bad* prompts are studded with `4k, hyperrealistic, best quality` and `8k, hyperrealistic, best quality` — and Higgsfield's stated objection to them is never the number. It is that the cast, the light source, the blocking, and the camera path were left unspecified. Higgsfield does not anywhere say that a resolution word in a prompt is itself an error.

**Practical taxonomy for rule design.**

| Term | Measurable against a parameter? | Notes |
|---|---|---|
| `480p`, `720p`, `1080p`, `4k`, `8k`, `2160p`, `UHD`, `Full HD` | **Yes** — each maps to a definite short side | Only `8k` has no attainable Higgsfield target; every other term is a real option somewhere in the catalog |
| `1k`, `1.5k`, `2k`, `512`, `768` | **Yes, but currently unmapped by the linter** | These are the *image*-model and `minimax_*` vocabularies; `RESOLUTION_PARAM` knows none of them |
| `high resolution`, `ultra-high resolution`, `hyperrealistic`, `best quality`, `cinematic quality`, `award winning`, `ultra detailed`, `8K IMAX`, `IMAX 15-perf 65mm` | **No** — marketing/stylistic register with no numeric referent | Must never become an error. `8K IMAX` in particular is Higgsfield's own house phrasing. |

The honest line is: **fire only on a term with a defined pixel floor, and only against a resolution the call actually determines.** Today rule 5.1 crosses that line in one direction (`8k` as a style word) and stops short of it in another (`kling2_6`'s documented 1080p ceiling, and the whole `1k/2k` vocabulary).

---

## 8. Existing-rule audit

Every probe below was run against the current binary. `⇒` shows actual output.

### 8.1 `resolution_claim`

| Field | Finding |
|---|---|
| **Behavior** | Highest prompt claim vs `min(width,height)` or mapped `resolution`; fires when claim > actual; silent when actual unresolvable |
| **Applicable tools** | All six matched tools. Effectively dead on `generate_audio*` (no dimension params exist for audio models). |
| **Higgsfield evidence** | **Reasonably inferred.** A: resolution is a setting, and the prompt is meant to stay identical when it changes; A: "avoid contradictions" is a named habit (stated about prompt-vs-image, not prompt-vs-parameter). No first-party source says a resolution word in a prompt is an error. |
| **Support level** | Reasonably inferred, not explicitly supported. |
| **Confirmed false positives** | (1) **Style register.** `{"prompt":"Style: 8K IMAX commercial, 16:9 widescreen. … 8K detail.","resolution":"1080p"}` ⇒ `prompt says "8K" but the call produces a 1080px short side.` This is Higgsfield's own published prompt. (2) **Quoted / diegetic text.** `{"prompt":"The presenter says \"shot in 4K\" to the camera.","width":1920,"height":1080}` ⇒ fires. (3) Any `8k`/`4k` used as a look tag alongside `ARRI ALEXA aesthetic` / `35mm film quality`. |
| **Confirmed false negatives** | (1) **`kling2_6` 4K claim, no dims, no `resolution`** ⇒ silent, though the model's documented ceiling is 1080p — the highest-value miss. (2) `minimax_h3` `resolution:"2K"` with a `4k` claim ⇒ silent (`2k` unmapped). (3) Image models: `nano_banana_pro` `resolution:"2k"` with a `4K` claim ⇒ silent. (4) `wan2_6` uses `quality`, `kling3_0` uses `mode:"4k"` — neither field is read. (5) `1k`/`1.5k`/`512`/`768`/`2160p` prompt claims are not in the claim lexicon either. |
| **Message accuracy** | Mostly good — it quotes the prompt's own casing and shows the raw dimensions. Two problems: it says **"Raise resolution"** when the model may have no `resolution` parameter at all (`kling2_6`), and `SPEC.md` §6's sample output still reads **"Set resolution"**, so spec and code disagree on the sentence. Where the dimensions came from an undeclared passthrough field, "width 1376 x height 768 gives a 768px short side" asserts an output size the call may not actually produce. |
| **Valid across models?** | **No.** The `RESOLUTION_PARAM` map encodes the video vocabulary only. Silence is the safe failure, so this is a coverage bug rather than a correctness bug — except for the "Raise resolution" wording, which is wrong advice on models without the parameter. |
| **Test coverage** | Good. Python: 8 tests incl. boolean dims, the 128px floor, `resolution`-param precedence, message shape. Rust: 13 tests incl. asymmetric boolean width, narrow-short-side-with-wide-long-side, non-ASCII boundary safety. Parity sweeps every lexicon term × 5 dimension pairs × both `params` shapes × `1k`/`2k` unknown values. |
| **Benchmark additions** | The `8K IMAX` style header (must **not** fire under a revised rule); `kling2_6` + `4K` + no dims (should fire under HFR-002); `2K`/`1k`/`1.5k`/`768` param values; a resolution term inside quotes; `mode:"4k"` and `quality:"1080p"` as the resolution carrier. |

### 8.2 `prompt_negative_conflict`

| Field | Finding |
|---|---|
| **Behavior** | 14 curated phrases; negations stripped from the **prompt only**; both fields lowercased; boundary-scanned |
| **Applicable tools** | All six; only meaningful where a caller passes `negative_prompt` at all |
| **Higgsfield evidence** | **Internal agency convention.** A: "avoid contradictions" exists as a general habit. Nothing first-party addresses cross-field contradiction, and no model publishes the field. |
| **Support level** | Weakly supported. The underlying instinct (one instruction, one place) is sound; the specific mechanism is ours. |
| **Confirmed false positives** | (1) `{"prompt":"A cast-iron pan sizzles on the stove.","negative_prompt":"pan, zoom, camera movement"}` ⇒ `"pan" appears in both`. Common-noun homograph. (2) `{"prompt":"sharp focus on the blade","negative_prompt":"soft focus, not sharp"}` ⇒ fires, but `not sharp` in the negative field is *agreement*; negations are stripped from the prompt only. (3) `{"prompt":"A poster with the text \"OPEN\" in bold.","negative_prompt":"misspelled text, distorted letters"}` ⇒ `"text" appears in both` — this is precisely the pairing Nano Banana's help article recommends (quote the literal text; suppress typography artifacts). Same shape for `static` (static shot vs static noise), `grain` (wood grain vs film grain), `zoom` (zoom lens vs zoom move). |
| **Confirmed false negatives** | Inflectional and near variants: `blur` vs `blurry`, `slowmo` vs `slow-mo`, `close up` (unhyphenated) vs `close-up`, `hand-held` vs `handheld`. Non-English overlap. Any directive outside the 14 phrases (`letterbox`, `watermark`, `lens flare`, `speed ramp`, `subtitles`). |
| **Message accuracy** | Accurate and mechanical: names the terms, says "Keep each term in one." It does not claim which field should win, which is the right restraint. It does overstate certainty when the overlap is a homograph. |
| **Valid across models?** | Uniform, but see the field-support caveat — on a model that ignores `negative_prompt` entirely, the "conflict" is not a conflict, it is one live instruction and one inert string. |
| **Test coverage** | Python: 5 tests. Rust: 5. Parity sweeps all 14 phrases as true conflict, JSON-string twin, and negated-in-prompt variant. **No test asserts a homograph must not fire** — the FP class is entirely uncovered. |
| **Benchmark additions** | All three confirmed FPs as negative controls; `not sharp` in the negative field; inflectional near-misses; a `negative_prompt` of `""` and of whitespace. |

### 8.3 `negation_placement`

| Field | Finding |
|---|---|
| **Behavior** | Gate = non-empty `negative_prompt` in this call, or `model ∈ {"kling2_6"}`; then every `(no\|without\|avoid) + ≤3 words` span not caught by the noise filter is reported |
| **Applicable tools** | All six. On `generate_audio*` the `prompt` **is the spoken text**, so the advice is nonsensical there. |
| **Higgsfield evidence** | **Contradicted.** See §6. |
| **Support level** | **Not supported.** Internal convention that runs against explicit first-party guidance. |
| **Confirmed false positives** | (1) **Higgsfield house style.** `{"model":"seedance_2_5","prompt":"Style: 8K IMAX commercial. No jitter, no music, no subtitles, no readable text.","negative_prompt":"watermark"}` ⇒ `"No jitter", "no music", "no subtitles" and 1 more sit in the positive prompt…`. Every quoted span is copied from Higgsfield's own published style header. (2) **Scene description.** `A road with no cars. A man with no hat.` ⇒ both fire; these describe the frame, not an artifact to suppress. (3) **Quoted literal text.** `A sign reading "NO PARKING" hangs above the door.` ⇒ `"NO PARKING" sits in the positive prompt…` — the text is the *subject*. (4) **Span spill.** On unpunctuated text the ≤3-word window crosses clause boundaries: a long prompt yielded `"no logos a beautiful"` repeatedly, which is not a phrase anyone wrote. (5) `kling2_6` fires with **no `negative_prompt` in the call at all**, purely from the seed list. |
| **Correct non-triggers (verified)** | `Film noir`, `The statue is not moving`, `A never-before-seen creature`, `no more than three`, `without moving` — all silent. The `not`-exclusion and the noise filter work as designed. |
| **Confirmed false negatives** | Non-English negation (`sans logos`, `ohne Text`, `不要文字` — all silent); `NOT teal-orange` / `NOT plastic` (excluded with `not`); `zero AI-text artifacts`; `never`; `free of`; `excluding`. |
| **Message accuracy** | **Inaccurate on two counts.** "this model accepts negative_prompt" is unverified for every model in the catalog. "models follow negation poorly in the prompt" is asserted as fact and is contradicted by Higgsfield's published testing, which reports inline exclusions as the highest-yield guardrail. |
| **Valid across models?** | **No.** The gate is evidence-thin for `kling2_6` and unfounded for the `negative_prompt`-is-present branch, which fires on any model the moment a caller supplies the field. |
| **Test coverage** | Strongest of the three: 12 Python tests, 7 Rust tests, plus a parity sweep over 4 negation forms × with/without `negative_prompt` × known/unknown/absent model. But every test asserts the *current* semantics; none tests whether the semantics are right, and none covers quoted literals, style headers, span spill, or scene-descriptive negation. |
| **Benchmark additions** | All five FPs as negative controls; the `seedance_tree_clean` fixture *with* a `negative_prompt` added (currently silent only by accident of the model list); a `generate_audio` TTS line containing "no". |

---

## 9. Traceability matrix

| # | Higgsfield guidance (class) | SPEC | Rust | Python oracle | Tests | Coverage verdict |
|---|---|---|---|---|---|---|
| G1 | Be specific: subject / details / environment / style (+camera, mood for video) (**A**) | — | — | — | — | **Not covered — correctly.** Subjective; SPEC §11 non-goal. |
| G2 | Video prompt = shot list with labeled blocks (**A**) | — | — | — | — | Not covered. Structural, but scoring it is subjective. |
| G3 | Seedance 2.5 section order, `Hard cut.` terminators (**A**) | — | — | — | — | Not covered. Would be a style rule; rejected in §11. |
| G4 | State shots + **total duration** + **aspect ratio** at top of prompt (**A**) | — | — | — | — | **Gap. Highest-value addition** — see HFR-001, HFR-003. |
| G5 | Aspect ratio stated numerically in prompt (**A**) | — | — | — | — | **Gap** — HFR-003. |
| G6 | Resolution is a setting; prompt unchanged when scaling (**A**) | §5.1 | `check_resolution_claim` | `check_resolution_claim` | 21 tests + sweep | **Partially covered.** Video vocabulary only; misses `1k/1.5k/2k/512/768/2160p`, `quality`, `mode`. |
| G7 | Per-model documented resolution ceilings (**A** help center, **A′** catalog) | — | — | — | — | **Gap** — HFR-002. Closes rule 5.1's largest FN. |
| G8 | Resolution/quality words also used as style register (`8K IMAX`, `35mm`, `ARRI ALEXA`) (**A/B**) | — | not distinguished | not distinguished | none | **Uncovered FP class** — HFR-R1 mitigation. |
| G9 | Exclusions belong inline in the positive prompt (**A**) | §5.3 asserts the opposite | `check_negation_placement` | same | 19 tests + sweep | **Contradicted.** Rule and guidance point opposite ways. |
| G10 | Long "no this, no that" lists are worse than positive description (**A**) | §5.3 (approximately) | fires on ≥1 negation | same | — | **Directionally aligned, badly calibrated.** Higgsfield's objection is to *volume*, not to *placement*. A count threshold would match the guidance; a placement remedy does not. |
| G11 | No model publishes `negative_prompt` support (**A′**) | §5.3 concedes "the catalog does not publish negative-prompt support" | `data/negative_prompt_models.json` = `["kling2_6"]` | same | `load_models` tests | **Weak.** SPEC's framing was "not published"; the catalog is stronger — the field is absent from all ~63 entries. |
| G12 | Keep prompt and inputs from contradicting each other (**A**) | §5.2 (cross-field variant) | `check_prompt_negative_conflict` | same | 10 tests + sweep | Covered in spirit; homograph FPs uncovered. |
| G13 | Quote literal text; describe font/size/placement (**A**) | — | — | — | — | Not covered. Interacts badly with 5.2 (`text`) and 5.3 (`"NO PARKING"`). |
| G14 | Model-specific cross-parameter constraints, e.g. seedance_2_0 `4k` needs `mode:"std"` (**A′**) | — | — | — | — | **Gap** — HFR-004. |
| G15 | Aspect ratio must be one the model supports (**A′**) | — | — | — | — | **Gap** — HFR-005. |
| G16 | Duration ranges/options per model; unsupported values are silently clamped (**A′**) | — | — | — | — | **Gap** — HFR-007. Also invalidates prompt timecodes. |
| G17 | `medias[].value` must be a media_id/job_id, never a URL (**A′**) | §11 non-goal ("duplicating docstrings") | — | — | — | Deliberately uncovered; noted in §11. |
| G18 | `@element` / `[VFX:]` / `Shot N:` syntax (**A**) | — | — | — | — | Not covered; matters as a *constraint on new rules*, not as a rule. |
| G19 | Soul: prompt field disabled when a reference image is attached (**A**) | — | — | — | — | Not covered; not observable from `tool_input`. |
| G20 | MCP always spends credits; no Unlimited (**A**) | §1 economics | — | — | — | Out of scope for the hook; relevant to SPEC §13. |

---

## 10. Candidate deterministic rules

Every candidate below inspects only `tool_input` (plus `tool_name` where noted), is pure, and is advisory. None infers aesthetic quality, creative intent, factual plausibility, or broad semantics.

---

### HFR-001 — `duration_claim_conflict`
- **Name:** Prompt states a total duration that the `duration` parameter will not produce.
- **Evidence class:** **A** — "state the number of shots, the total duration, and the aspect ratio at the top of the prompt"; Sora 2 guide's timecoded beats; **B** — the `Total: 15s / 6 shots / 16:9` line closing most official Seedance prompts.
- **Source:** `higgsfield.ai/blog/seedance-prompting-guide`; `higgsfield.ai/sora-2-prompt-guide`.
- **Applies to:** `generate_video`, `generate_video_batch`. **Not** image or audio.
- **Fields:** `prompt`, `duration`.
- **Algorithm (deterministic):**
  1. Require `duration` to be a JSON integer or float in `1..=600`. Otherwise silent.
  2. Scan `prompt` for these forms only, case-insensitive, with the existing `[a-z0-9]` boundary semantics:
     - `Total: <N>s` / `Total: <N> s` / `Total: <N> seconds`
     - `<N>-second` / `<N> second` / `<N> seconds` / `<N>s total`
     - a trailing timecode bound `<N>–<M>s` or `<N>s to <M>s`, taking `max(M)` across all matches
  3. Collect all values; let `claimed = max(values)`.
  4. Fire when `claimed > duration` **or** `claimed < duration` by more than 20 %. (Asymmetric tolerance is deliberate: a prompt that describes a 15 s beat sheet inside a 5 s render is a guaranteed truncation, whereas small under-claims are common rounding.)
- **Severity:** advisory (all findings are).
- **Diagnostic:** `duration_claim: prompt describes 15s of action but duration is 5. Raise duration, or retime the beats.`
- **Triggers:** `"…Total: 15s / 6 shots / 16:9"` with `duration: 5`; `"A 12-second cinematic sportswear ad…"` with `duration: 5`; `"0–3s … 12–15s"` with `duration: 8`.
- **Must NOT trigger:** `"a 5 second pause between lines"` with `duration: 15` (claimed < duration and within no rule — the max is 5, under-claim beyond 20 % — **note:** this is the rule's main FP risk; see below); `"shot on a 50mm"` (not a duration); `"15 shots"` (count, not seconds); duration absent; duration non-numeric.
- **Likely FPs:** a *within-scene* duration (`"holds for 3 seconds"`) read as a total. **Mitigation:** for the MVP, fire on the over-claim branch only (`claimed > duration`) and drop the under-claim branch entirely. That removes the FP class at the cost of one FN class, which is the right trade for an advisory tool.
- **Likely FNs:** durations written in words ("fifteen seconds"); frame counts; `Total: 15s` inside a batch entry whose `duration` is inherited from a model default.
- **Safe normalization:** lowercase; treat `–` (en dash) and `-` alike; ignore thousands separators.
- **MVP:** **Yes**, over-claim branch only.
- **Fixtures:** F-201…F-206 (§12). Unit + parity.

---

### HFR-002 — `resolution_ceiling_conflict`
- **Name:** Prompt claims a resolution above the selected model's published maximum.
- **Evidence class:** **A** (help-center Kling table: Kling 2.6 → 1080p; Seedance variants → 480p–4K) + **A′** (`models_explore` `resolution`/`quality`/`mode` option lists).
- **Source:** `…/help-center/ai-models/how-do-i-use-kling`; `…/how-do-i-use-seedance`; MCP `models_explore`.
- **Applies to:** all `generate_image*` / `generate_video*` where `model` is present and in the ceiling table.
- **Fields:** `prompt`, `model`.
- **Algorithm:** load `data/model_resolution_ceilings.json` (a new evidence-seeded file, same pattern and same fail-open loader as `negative_prompt_models.json`): `{ "<model_id>": <max_short_side_px> }`, e.g. `kling2_6: 1080`, `seedance_2_0: 2160`, `seedance_2_5: 1080`, `nano_banana_pro: 2160`, `soul_2: 1536`. Compute the prompt claim exactly as `claimed_short_side` does today. Fire when `claim_px > ceiling` **and** the existing dimension-based check did not already fire (one finding per call, not two).
- **Diagnostic:** `resolution_claim: prompt says "4K" but kling2_6 renders at most 1080p. Drop the claim, or switch to a model that reaches it.`
- **Triggers:** `{"model":"kling2_6","prompt":"cinematic 4K commercial"}` — silent today.
- **Must NOT trigger:** a model absent from the table (silent); `{"model":"seedance_2_0","prompt":"4k"}` (ceiling 2160, claim 2160, not greater).
- **Likely FPs:** style-register claims — **`8k` must be excluded from this rule entirely** (see HFR-R1), otherwise the `8K IMAX` header fires on every model.
- **Likely FNs:** every model not yet in the table; models whose ceiling changes after the snapshot.
- **Staleness:** the file is a dated snapshot. A `doctor` check can print its capture date; the rule stays advisory and silent on unknown models, so drift degrades to silence.
- **MVP:** **Yes**, if paired with HFR-R1.
- **Fixtures:** F-101…F-108.

---

### HFR-003 — `aspect_ratio_claim_conflict`
- **Name:** Prompt states an aspect ratio different from the `aspect_ratio` parameter.
- **Evidence class:** **A** — Nano Banana Pro guide: "Specify aspect ratios numerically (e.g., 16:9, 2:3)"; Seedance guide: state the aspect ratio at the top.
- **Source:** `higgsfield.ai/nano-banana-pro-prompt-guide`; `higgsfield.ai/blog/seedance-prompting-guide`.
- **Applies to:** `generate_image*`, `generate_video*`.
- **Fields:** `prompt`, `aspect_ratio` (fallback: reduced `width:height`).
- **Algorithm:**
  1. Require `aspect_ratio` to be a string matching `^\d{1,2}:\d{1,2}$`. If absent, and `width`/`height` are both integers ≥128, derive `w:h` reduced by GCD. Otherwise silent.
  2. Scan the prompt for `\b\d{1,2}:\d{1,2}\b` tokens, **excluding** any token that is preceded within 24 characters by a timecode cue (`at `, `from `, `to `, `s `, `min`) — this is what prevents `2:30` clock times from matching.
  3. Normalize both sides by GCD reduction.
  4. Fire only when the prompt contains **exactly one distinct** ratio and it differs from the parameter's reduced form.
- **Diagnostic:** `aspect_ratio_claim: prompt says "9:16" but aspect_ratio is "16:9". Set one to match the other.`
- **Triggers:** `{"prompt":"Vertical 9:16 TikTok clip","aspect_ratio":"16:9"}` — silent today.
- **Must NOT trigger:** the Desert Hero prompt containing both `2.35:1` and `16:9` (two distinct ratios ⇒ silent, by design, since Higgsfield itself publishes that prompt); decimal ratios like `2.35:1` (excluded by `\d{1,2}:\d{1,2}`); `60:30:10` colour splits (three parts, not matched); a `2:30` runtime; `aspect_ratio: "auto"`.
- **Likely FPs:** a ratio mentioned as scene content ("a 4:3 television set in the corner"). Rare; accepted for an advisory hook, and mitigated by the exactly-one-ratio condition.
- **Likely FNs:** worded ratios ("widescreen", "vertical", "portrait"); prompts naming two ratios where one really is wrong.
- **Safe normalization:** GCD reduction only. Never map words to ratios.
- **MVP:** **Yes.**
- **Fixtures:** F-301…F-308.

---

### HFR-004 — `mode_resolution_incompatible`
- **Name:** `resolution` and `mode` cannot both hold for this model.
- **Evidence class:** **A′** — `seedance_2_0`: "4k/1080p require mode='std'; mode='fast' supports 480p/720p only"; `minimax_hailuo`: "'512' … not supported by the 'minimax-2.3' variants."
- **Applies to:** `generate_video*` for the models carrying the constraint.
- **Fields:** `model`, `mode`, `resolution`, `variant`.
- **Algorithm:** table lookup in the same snapshot file; fire on an exact declared incompatibility only. No inference.
- **Diagnostic:** `param_conflict: seedance_2_0 mode "fast" supports 480p and 720p only, but resolution is "1080p". The call will be adjusted or rejected.`
- **Must NOT trigger:** any model/mode pair not in the table.
- **MVP:** No — worthwhile but subordinate to HFR-001/002/003, and it edges toward SPEC §11's "duplicating docstrings" line (the constraint is in the catalog description the model already sees). Include only if the catalog snapshot lands for HFR-002 anyway.

---

### HFR-005 — `aspect_ratio_unsupported_by_model`
- **Evidence class:** **A′** — each catalog entry publishes `aspect_ratios`.
- **Algorithm:** fire when `aspect_ratio` is a string, `model` is in the snapshot, and the value is not in that model's list (e.g. `21:9` on `kling2_6`, whose list is `16:9 / 9:16 / 1:1`).
- **Diagnostic:** `param_conflict: kling2_6 supports 16:9, 9:16, 1:1 — "21:9" is not one of them.`
- **MVP:** No. Same §11 tension as HFR-004; also the server may coerce silently, making the finding cosmetic.

---

### HFR-006 — `negation_placement` re-grounding (**replaces**, not adds)
- **Name:** Retarget the rule from *placement* to *volume*.
- **Evidence class:** **A** — "Positive descriptions generally work better than long lists of 'no this, no that,' though naming a few key things to avoid … can help."
- **Rationale:** Higgsfield's objection is to volume, not to location. A rule that counts, rather than relocates, is the one the guidance actually licenses.
- **Algorithm:** count distinct negation spans using the existing `PLACEMENT_NEGATION_RE` + noise filter, **excluding** spans inside `"…"` or `'…'`, and **excluding** spans whose window crosses a sentence terminator. Fire only when the count exceeds a threshold **and** the prompt is short enough that exclusions dominate: e.g. `count ≥ 12` **and** `count / word_count ≥ 0.08`. Both thresholds are class **D** and must be calibrated on the corpus before shipping — Higgsfield's own long prompts carry 15–30 exclusions across 800+ words and must stay silent (ratio ≈ 0.02–0.04).
- **Diagnostic:** `negation_density: 14 exclusions in a 90-word prompt. Higgsfield recommends describing what you want; keep only the few exclusions that matter.`
- **Gate change:** drop the `negative_prompt`-present branch and the model seed list entirely. Neither has first-party support.
- **Tool scoping:** requires reading `payload.tool_name`; skip on `generate_audio*`, where `prompt` is spoken text.
- **MVP:** **No.** The right MVP action is to *disable* 5.3 (§14), not to replace it in the same change. Ship the replacement only after the thresholds are calibrated against the fixture corpus.

---

### HFR-007 — `duration_out_of_model_range`
- **Evidence class:** **A′** — per-model `duration_range` / `durations`; schema: "Unsupported → nearest allowed value or clamped to range."
- **Algorithm:** fire when `duration` is numeric, `model` is in the snapshot, and the value is outside the range or not in the discrete option list (e.g. `duration: 7` on `kling2_6`, whose options are `[5, 10]`).
- **Diagnostic:** `param_conflict: kling2_6 accepts 5 or 10 seconds; 7 will be adjusted. Beats timed to 7s will not land.`
- **Why it matters beyond the docstring:** silent clamping invalidates every timecode in the prompt, which is the exact failure HFR-001 exists to catch. That linkage is what lifts it above §11's non-goal.
- **MVP:** No; second wave, alongside HFR-002's snapshot.

---

### Rule 5.2 recalibration (**modification**, not a new rule)
- **Evidence class:** **D** (engineering), motivated by three confirmed FPs.
- **Recommended deterministic thresholds:**
  1. **Normalize** both fields: Unicode NFKC, lowercase, collapse whitespace, trim surrounding punctuation. **No stemming** — inflectional matching is where a curated list turns into a fuzzy matcher.
  2. **Strip negations from the negative field too.** `not sharp` in `negative_prompt` is a double negative; it must not count as a hit. This kills FP (2) outright and is a two-line change to the existing `strip_negations` call site.
  3. **Exclude occurrences inside matched double quotes in the prompt.** Quoted literals are Higgsfield-recommended (Nano Banana) and are content, not direction.
  4. **Require ≥2 tokens, or membership in a strict unambiguous single-token set.** Recommended list: keep `slow motion`, `slow-mo`, `motion blur`, `close-up`, `wide shot`, `handheld`, `blurred`, `blurry`, and add `lens flare`, `film grain`, `letterbox`, `watermark`, `subtitles`. **Drop** the bare `pan`, `text`, `sharp`, `static`, `zoom`, `grain` — each has a confirmed or obvious common-noun homograph. Where the directive sense matters, add the bigram instead (`camera pan`, `whip pan`, `slow zoom`, `on-screen text`, `film grain`, `static shot`).
  5. **Minimum length 4 characters** for any single-token entry.
- **Expected effect:** removes all three confirmed FPs; loses genuine `slow motion`-class detections in none of the observed history (the `kling2_6` fixture's real hit is `slow motion`, a two-token entry that survives).
- **Parity consequence:** this changes behavior, so it is a **deliberate rule change**, not an oracle edit. Per the invariant in §13, the sequence must be: change `SPEC.md` §5.2 first, then Rust, then re-derive parity — never adjust `src/higgsfield_lint.py` to make a diff go green.

---

## 11. Rejected or non-deterministic rule ideas

| Idea | Why rejected |
|---|---|
| Score prompt structure against the Seedance 2.5 block order | Requires judging whether prose "is" a `LOCATION` section. Semantic. SPEC §11 non-goal. Also the sources contradict each other on ordering (§5.3 #1), so there is no single target. |
| Warn when a prompt lacks a camera move / lighting source / blocking | Higgsfield says these matter — but also says "You don't have to fill every layer every time" and "Keep it focused for simple tasks." A required-section rule would fire on every legitimately short prompt. |
| Flag `hyperrealistic`, `best quality`, `award winning`, `8k` as filler | Higgsfield's own critique of these prompts is never about the adjectives; it is about unspecified cast, light, and blocking. Flagging vocabulary is aesthetic judgment. §7 says explicitly: a term must not become an error because it sounds inflated. |
| Flag quoted literal text alongside a `no text` exclusion | Fires on Higgsfield's own cowboy prompt, which carries quoted **dialogue** and `NO on-screen text, NO subtitles` simultaneously. Spoken text and rendered text are different things and the linter cannot tell them apart. |
| Require `@element` tokens to be backed by `medias[]` | Elements resolve server-side by account-scoped name; a bare `@hero` with no `medias` may be entirely valid. Unverifiable from `tool_input`. |
| Flag `count > 1` with `use_unlim: true` | The schema already states the cap and the server returns it in `adjustments`. SPEC §11 non-goal. |
| Flag `https://` in `medias[].value` | Genuinely deterministic and a guaranteed error, but the schema says it in the description the model already reads. Listed here rather than in §10 for that reason; revisit only if it is observed to happen in practice. |
| Detect contradictions between prompt text and an attached reference image | Requires reading the image. Out of scope for a text hook. |
| Non-English negation detection | Would need per-language negator lists and would immediately produce the same FP classes in languages we cannot spot-check. Higgsfield recommends English anyway; the FN is acceptable. |
| Infer whether a resolution word is stylistic or literal from context | Semantic. The deterministic substitute is the `8k`-exclusion carve-out in HFR-R1 below. |

**HFR-R1 (mitigation, not a rule): remove `8k` from the claim lexicon, or exempt it.**
`8k` is the only lexicon entry with no attainable Higgsfield target, and it is the one Higgsfield itself writes as a style tag (`8K IMAX commercial`, `8K detail`). Every `8k` firing is therefore either (a) a style register — a false positive — or (b) a claim no model could satisfy, where the finding is technically true but tells the user nothing actionable. Both argue for dropping it. Class **D**, but grounded in the class-A observation that Higgsfield publishes `8K IMAX` as house style. Same reasoning applies to `uhd` and `full hd`, which appear in no official example prompt at all and have never fired on real usage.

---

## 12. Benchmark and fixture plan

Design targets: the corpus must measure **false-positive rate**, not just recall. Negative controls outnumber positives roughly 2:1 below, deliberately, and every FP confirmed in §8 becomes a permanent negative control.

**Placement key:** `U` = Rust unit test, `P` = parity harness (must run through both implementations), `B` = both.

### A. Clean calls — must emit nothing

| ID | Tool | Input (abbreviated) | Expected | Rules | Rationale | Where |
|---|---|---|---|---|---|---|
| F-001 | `generate_video` | existing `seedance_tree_clean.json` | none | all | real captured clean call; the anchor negative fixture | B |
| F-002 | `generate_video` | Higgsfield cowboy prompt (17 blocks, ~20 inline exclusions), `resolution:"1080p"`, `duration:15`, no `negative_prompt` | none | all | official published prompt; must stay silent | B |
| F-003 | `generate_image` | Nano Banana Pro one-liner: `"Samurai in red armor wearing a fierce red oni mask"`, `resolution:"4k"` | none | all | official short-form image prompt | B |
| F-004 | `generate_video` | Seedance 2.5 drama prompt with `No logos, no readable text`, `No music, no narration, no subtitles`, no `negative_prompt`, `duration:15` | none | all | official long-form; the §6 anchor | B |
| F-005 | `generate_audio` | `{"model":"seed_audio","prompt":"I'm not going, and there's no way around it."}` | none | all | TTS line; negation is spoken content | B |
| F-006 | `generate_video` | prompt with no resolution word, no ratio, no duration claim; `duration:5`, `aspect_ratio:"16:9"` | none | all | minimal baseline | U |

### B. Genuine prompt-vs-parameter contradictions — must fire

| ID | Tool | Input | Expected | Rules | Where |
|---|---|---|---|---|---|
| F-101 | `generate_video` | existing `kling2_6_4k_slowmo.json` | `resolution_claim` + `prompt_negative_conflict` (+ `negation_placement` while 5.3 lives) | 5.1, 5.2, 5.3 | B |
| F-102 | `generate_video` | `{"model":"kling2_6","prompt":"cinematic 4K commercial","duration":5}` | `resolution_claim` (ceiling) | HFR-002 | B |
| F-103 | `generate_video` | `{"prompt":"shot in 4k","width":1280,"height":720}` | `resolution_claim` | 5.1 | B |
| F-201 | `generate_video` | `{"prompt":"…Total: 15s / 6 shots / 16:9","duration":5,"aspect_ratio":"16:9"}` | `duration_claim` only | HFR-001 | B |
| F-202 | `generate_video` | `{"prompt":"A 12-second cinematic sportswear ad…","duration":5}` | `duration_claim` | HFR-001 | B |
| F-203 | `generate_video` | `{"prompt":"0–3s … 12–15s","duration":8}` | `duration_claim` | HFR-001 | B |
| F-301 | `generate_video` | `{"prompt":"Vertical 9:16 TikTok clip","aspect_ratio":"16:9"}` | `aspect_ratio_claim` | HFR-003 | B |
| F-302 | `generate_image` | `{"prompt":"portrait, 2:3","width":1920,"height":1080}` | `aspect_ratio_claim` (derived 16:9) | HFR-003 | U |

### C. Borderline language — must NOT fire (false-positive controls)

| ID | Input | Must not fire | Rule under test | Where |
|---|---|---|---|---|
| F-401 | `Style: 8K IMAX commercial, 16:9 widescreen … 8K detail.` + `resolution:"1080p"` | `resolution_claim` | 5.1 / HFR-R1 | B |
| F-402 | `The presenter says "shot in 4K" to the camera.` + `1920x1080` | `resolution_claim` | 5.1 (quote exclusion) | B |
| F-403 | `A cast-iron pan sizzles.` / neg `pan, zoom` | `prompt_negative_conflict` | 5.2 recalibration | B |
| F-404 | `sharp focus on the blade` / neg `soft focus, not sharp` | `prompt_negative_conflict` | 5.2 (negative-side stripping) | B |
| F-405 | `A poster with the text "OPEN" in bold.` / neg `misspelled text` | `prompt_negative_conflict` | 5.2 (drop bare `text`) | B |
| F-406 | `A sign reading "NO PARKING" hangs above the door.` + `model:"kling2_6"` | `negation_placement` | 5.3 (quote exclusion) | B |
| F-407 | `A road with no cars. A man with no hat.` + `model:"kling2_6"` | `negation_placement` | 5.3 (scene-descriptive negation) | B |
| F-408 | `Film noir alley. The statue is not moving. A never-before-seen creature.` | anything | 5.3 (already passing — lock it in) | B |
| F-409 | `no more than three figures`, `no longer visible`, `no matter the weather` | anything | 5.3 stopwords | B |
| F-410 | `sitting without moving` | `negation_placement` | 5.3 gerund filter | B |
| F-411 | `no flickering, no warping, avoid banding` | must **still** fire | 5.3 (guards against over-suppression) | B |
| F-412 | Desert Hero prompt containing both `2.35:1` and `16:9` | `aspect_ratio_claim` | HFR-003 (exactly-one condition) | B |
| F-413 | `the 4:3 television set in the corner` + `aspect_ratio:"16:9"` | ideally nothing — **known accepted FP**, recorded so the rate is measured, not hidden | HFR-003 | U |
| F-414 | `holds for 3 seconds` + `duration:15` | `duration_claim` | HFR-001 (over-claim branch only) | B |
| F-415 | `60:30:10 colour split` + `aspect_ratio:"16:9"` | `aspect_ratio_claim` | HFR-003 (three-part token) | U |
| F-416 | `shot on a 50mm at T2.0` | anything | all | U |

### D. Model-specific behavior

| ID | Input | Expected | Where |
|---|---|---|---|
| F-501 | `{"model":"seedance_2_0","prompt":"4k hero shot","resolution":"4k"}` | none | B |
| F-502 | `{"model":"nano_banana_pro","prompt":"poster, 4K","resolution":"2k"}` | none today (FN); `resolution_claim` after the lexicon extension | B |
| F-503 | `{"model":"minimax_h3","prompt":"cinematic 4k","resolution":"2K"}` | none today (FN); fires after extension | B |
| F-504 | `{"model":"wan2_6","prompt":"1080p clip","quality":"720p"}` | none today (FN) | U |
| F-505 | `{"model":"kling3_0","prompt":"4k","mode":"4k"}` | none | U |
| F-506 | `{"model":"seedance_2_0","mode":"fast","resolution":"1080p"}` | `param_conflict` under HFR-004 | U |
| F-507 | `{"model":"kling2_6","duration":7}` | `param_conflict` under HFR-007 | U |
| F-508 | `{"model":"unknown_model_xyz","prompt":"8k"}` | none | B |

### E. Encoding, shape, and adversarial input — must exit 0, silent or correct

| ID | Input | Expected | Where |
|---|---|---|---|
| F-601 | `Un plan cinématographique en 4K — sans logos, ohne Text, 不要文字` + `1376x768` + neg `blurry` | `resolution_claim` only (non-English negation is a documented FN) | B |
| F-602 | prompt of RTL Arabic + emoji + combining marks + zero-width joiners | exit 0, no panic | B |
| F-603 | `4K` written with fullwidth `４Ｋ` | silent (documented FN of NFKC-free matching) | U |
| F-604 | `"4k"` at byte offset 0 and again glued as `mp4k` | one finding, from the standalone occurrence | U |
| F-605 | 3 MB prompt, 70 000 repetitions | exit 0, output produced, **< 250 ms** (measured: 51 ms) | P |
| F-606 | empty stdin | exit 0, no output | B |
| F-607 | `{}` | exit 0, no output | B |
| F-608 | `{"tool_input":null}` | exit 0, no output | B |
| F-609 | `{"tool_input":{"params":null}}` | exit 0, no output | B |
| F-610 | `{"tool_input":{"params":"a bare string"}}` | exit 0, no output | B |
| F-611 | `{"tool_input":{"params":"{\"prompt\":\"4k\",\"width\":1280,\"height\":720}"}}` | `resolution_claim` | B |
| F-612 | `{"tool_input":{"params":{"prompt":123,"width":"wide"}}}` | exit 0, no output | B |
| F-613 | `{"tool_input":{"requests":[]},"params":{…4k…}}` | falls through to top-level | B |
| F-614 | `{"tool_input":{"requests":[{"no_params":true}],"params":{…4k…}}}` | **no** fallback; silent | B |
| F-615 | batch, entries at indices 10/20/30, only 20 offends | one finding, prefixed `request 20:` | B |
| F-616 | batch entry with `index` absent | positional fallback `request 1:` | B |
| F-617 | `width: true`, `height: 768` | silent (bool excluded) | B |
| F-618 | `width: 16, height: 9` with a `4k` claim | silent (floor) | B |
| F-619 | `width: 1920, height: 100` with a `4k` claim | fires (floor is keyed on the larger side) | B |
| F-620 | `width: 1280.0` (float) | silent | B |
| F-621 | 4 MB of nested arrays as `tool_input` | exit 0, silent, no stderr | B |
| F-622 | invalid UTF-8 bytes on stdin | exit 0, silent | B |
| F-623 | `negative_prompt: ""` and `negative_prompt: "   "` with negations in prompt and an unknown model | silent (empty gate) | B |
| F-624 | `params.__panic__: true` (debug build only) | exit 0, silent — proves the panic hook is load-bearing | U |

### F. Intentional overlap — must NOT fire

| ID | Input | Rationale | Where |
|---|---|---|---|
| F-701 | prompt `handheld, slight wobble`; neg `tripod, gimbal, stabilized` | complementary, not overlapping — regression guard | U |
| F-702 | prompt `film grain`; neg `digital noise, banding` | near-miss vocabulary | U |
| F-703 | prompt `wide shot establishing`; neg `wide shot` | **genuine** conflict — must still fire | B |

**Corpus totals:** 6 clean · 8 positive · 16 FP controls · 8 model-specific · 24 robustness · 3 overlap = **65 fixtures**, of which **31 are negative controls**. That ratio is what makes a false-positive rate measurable rather than assumed.

**Metrics to report per run:** findings-per-call on the clean set (target: 0), FP rate on set C (target: 0 after recalibration; currently 6/16 fire), recall on set B, p99 wall-clock on set E, and byte-identical output across two consecutive runs of the whole corpus (determinism).

---

## 13. Observability and fail-open risks

### 13.1 The known limitation, stated precisely

`PreToolUse` treats any non-zero exit as non-blocking. So four distinct outcomes are indistinguishable downstream:

1. binary ran, found nothing → exit 0, no stdout;
2. binary ran, hit an internal error → exit 0, no stdout (by design, SPEC §7);
3. binary missing → exit 127, no stdout;
4. binary present but stale (built before the current `rust/src/*.rs`) → exit 0, **wrong** logic, possibly no stdout.

Case 4 is the worst, because it is silent *and* incorrect, and `SPEC.md` §12.1 already names it. Case 2 is the design and should stay.

### 13.2 What can be fixed without touching the contract

The `doctor` subcommand already exists (`rust/src/doctor.rs`), dispatches before stdin is touched, and is the one path explicitly permitted to print and exit non-zero. Three checks fit inside it with no change to hook behavior:

- **`binary_freshness`** — compare the executable's mtime against every `rust/src/*.rs` and `data/*.json`; `Fail` if any source is newer. (Today: binary 22:33:52, newest source 22:33:45 — passing.)
- **`golden_selftest`** — run the two committed fixtures through the in-process lint and assert the exact expected finding strings. This converts "did the logic survive the last build" from a guess into a check.
- **`data_file_present`** — assert `locate_data_file()` resolves and parses; today a missing file degrades silently to an empty model set, which silently disables one branch of rule 5.3.

None of these runs at `PreToolUse` time. None can print on the hook path. They are opt-in diagnostics for a human.

### 13.3 On changing fail-open — analysis, not a proposal

Making the hook signal its own failure would mean either writing to stderr or exiting non-zero on the hook path. Consequences:

- **stderr on `PreToolUse` is a blocking signal.** Exit 2 plus stderr denies the tool call. Even non-2 exits with stderr surface noise on every generation. This would convert an advisory tool into an intermittent blocker — a direct violation of SPEC §4, whose justification (§1) is that a false block costs the only scarce resource, the user's attention.
- **A heartbeat in `additionalContext`** ("higgsfield-lint: 0 findings") would distinguish case 1 from case 3, but at the cost of injecting a line into the model's context on *every* generation call, forever, to communicate nothing. That is a worse trade than the ambiguity it resolves.
- **Compatibility:** any change here alters behavior for every Higgsfield call in the project. There is no partial rollout.

**Recommendation: do not change fail-open.** Close the gap in `doctor`, which is where the contract already makes room for it, and note in the README that `cargo build --release` plus `higgsfield-lint doctor` is the setup ritual — which is what §12.1 already asks for, minus a way to verify it.

### 13.4 Parity-integrity risk

The one governance risk worth naming: several recommendations in §10 (the 5.2 recalibration, disabling 5.3, the resolution-lexicon extension) **change behavior**, and the parity harness compares Rust against `src/higgsfield_lint.py`. There is exactly one safe order, and it must be explicit in any resulting plan:

1. amend `SPEC.md` §5 to state the new intended behavior;
2. change the Rust;
3. re-derive the oracle from the amended spec **as a declared, reviewed step**, recorded in the commit message as an intentional rule change.

The failure mode to guard against is step 3 happening quietly, as a diff-suppression edit, so a red parity run turns green without anyone deciding the semantics changed. Per `SPEC.md` §9, "the fix is always to change the Rust, never the oracle" — that invariant holds for *ports*; a deliberate rule change is a different operation and must be labeled as one, never disguised as a port fix.

---

## 14. Prioritized recommendations

| # | Action | Why | Evidence | Effort |
|---|---|---|---|---|
| **P0** | **Disable `negation_placement` (5.3), or narrow its gate to "`negative_prompt` present AND non-empty" and rewrite its message.** | Contradicted by explicit first-party guidance (§6); five confirmed FP classes including Higgsfield's own house style; its remedy sentence asserts a mechanism Higgsfield's published testing contradicts; no model publishes the field. It fired on 7 of 9 historical calls, which SPEC §2 read as signal — the corpus says it was reading Higgsfield best practice. | **A** (contradicted) + **A′** | small |
| **P0** | **Add F-401 (`8K IMAX` style header) and F-406/F-407 as permanent negative controls before changing anything.** | Locks in the FP classes so no future change silently reintroduces them. | — | small |
| **P1** | **Drop `8k` (and `uhd`, `full hd`) from the claim lexicon.** | Only unattainable entry; the one Higgsfield writes as style; every firing is FP or unactionable. `uhd`/`full hd` appear in no official prompt and have never fired. | **A/B** + **D** | small |
| **P1** | **Recalibrate 5.2** per §10: strip negations on the negative side, exclude quoted spans, require ≥2 tokens or a strict single-token set, drop `pan`/`text`/`sharp`/`static`/`zoom`/`grain`. | Removes all three confirmed FPs; keeps the one real historical hit (`slow motion`). | **D**, FP-driven | medium |
| **P1** | **Add HFR-001 (`duration_claim_conflict`), over-claim branch only.** | Best-evidenced new rule: Higgsfield explicitly tells users to state total duration in the prompt; the contradiction is arithmetic; today it is invisible. | **A** | medium |
| **P2** | **Add HFR-003 (`aspect_ratio_claim_conflict`)** with the exactly-one-ratio condition. | Explicit guidance to state ratios numerically; purely mechanical; the exactly-one guard keeps Higgsfield's own dual-ratio prompt silent. | **A** | medium |
| **P2** | **Add HFR-002 (`resolution_ceiling_conflict`)** with a dated `data/model_resolution_ceilings.json`. | Closes 5.1's largest FN and finally makes the `kling2_6` "4K" case detectable on the evidence that actually supports it (the published ceiling) rather than on undeclared passthrough dimensions. | **A** + **A′** | medium |
| **P2** | **Extend `RESOLUTION_PARAM` to the image/`minimax` vocabulary** (`1k`, `1.5k`, `2k`, `512`, `768`, `2160p`) and read `quality` / `mode` as resolution carriers where the catalog says they are. | 5.1 is currently video-only; the image path is dead. | **A′** | medium |
| **P2** | **Add the three `doctor` checks in §13.2.** | Closes the stale/missing-binary observability gap inside the existing contract, with no change to fail-open. | — | small |
| **P3** | **Read `payload.tool_name` and scope checks per tool** (skip text checks on `generate_audio*`). | `prompt` on audio tools is spoken content; negation and directive rules are meaningless there. | **A′** | small |
| **P3** | Consider HFR-004 / HFR-005 / HFR-007 once the catalog snapshot exists. | Real contradictions, but closer to SPEC §11's docstring non-goal; value is mostly in HFR-007's interaction with prompt timecodes. | **A′** | medium |
| **P3** | **Correct `SPEC.md` §9's `anyOf` claim** to name the single-generation tools only, and align §6's sample message ("Set resolution") with the emitted text ("Raise resolution"). | Two small accuracy defects found during the audit. | — | trivial |

---

## 15. Source inventory

All accessed **2026-08-31**. Publisher is **Higgsfield, Inc.** unless noted. "Explicit" = the guidance is stated in prose; "example" = inferred from a published prompt.

### First-party — prompting guidance

| # | Title | URL | Published / updated | Model or feature | Guidance type |
|---|---|---|---|---|---|
| S1 | How do I write a good prompt? | https://higgsfield.ai/creator-hub/help-center/getting-started/how-do-i-write-a-good-prompt | 2026-07-30 | platform-wide | explicit + example |
| S2 | Seedance 2.0 — Complete Prompting Guide (Full Prompt Library) | https://higgsfield.ai/blog/seedance-prompting-guide | 2026-04-13 (by Rus Syzdykov) | Seedance 2.0 | explicit + example |
| S3 | Seedance 2.5: Complete Prompting Guide (Full Prompt Library) | https://higgsfield.ai/blog/seedance-2-5-prompting-guide | 2026-08-13 | Seedance 2.5 | explicit + example |
| S4 | Common AI Video Prompt Mistakes (And the Fixes That Actually Work) | https://higgsfield.ai/blog/ai-video-prompt-mistakes | 2026-08-03 | video, platform-wide | explicit + paired bad/good examples |
| S5 | Nano Banana Pro: High-Control Prompting & Templates | https://higgsfield.ai/nano-banana-pro-prompt-guide | undated | Nano Banana Pro | explicit + example |
| S6 | Free Sora 2 Prompt Guide & Templates for Viral Videos | https://higgsfield.ai/sora-2-prompt-guide | undated | Sora 2 | explicit + example |
| S7 | 3-Step Workflow To Make Ultra-Realistic AI Ads | https://higgsfield.ai/blog/cinematic_headphones | undated | Seedance + Soul, full ad pipeline | example (the only `Negative prompt:` field usage found) |
| S8 | How do I use Seedance? | https://higgsfield.ai/creator-hub/help-center/ai-models/how-do-i-use-seedance | 2026-08-01 | Seedance | explicit (ordering, camera-term list) |
| S9 | How do I use Kling? | https://higgsfield.ai/creator-hub/help-center/ai-models/how-do-i-use-kling | 2026-08-01 | Kling family | explicit (incl. the 1080p ceiling for Kling 2.6) |
| S10 | How do I use Nano Banana? | https://higgsfield.ai/creator-hub/help-center/ai-models/how-do-i-use-nano-banana | 2026-08-02 | Nano Banana family | explicit (quote literal text) |
| S11 | How do I use Soul to generate images? | https://higgsfield.ai/creator-hub/help-center/ai-models/how-do-i-use-soul-to-generate-images | 2026-08-01 | Soul family | explicit (prompt disabled with a reference image) |
| S12 | How do I use Cinema Studio? | https://higgsfield.ai/creator-hub/help-center/tools/how-do-i-use-cinema-studio | 2026-08-01 | Cinema Studio 2.0–3.5 | explicit (settings replace prompt text) |
| S13 | How do I use Canvas for multi-node generation? | https://higgsfield.ai/creator-hub/help-center/tools/how-do-i-use-canvas | 2026-08-01 | Canvas | explicit (`@element-name`, reference role first) |
| S14 | Which AI model should I use? | https://higgsfield.ai/creator-hub/help-center/ai-models/which-ai-model-should-i-use | 2026-08-01 | model families | explicit |
| S15 | Why AI Video Generations Fail And How to Fix Every Common Error | https://higgsfield.ai/blog/why-ai-video-generations-fail | 2026-08-09 | video, platform-wide | explicit (also the credit-refund contradiction, §5.3 #6) |
| S16 | What is Higgsfield MCP, and how is it different from the web? | https://higgsfield.ai/creator-hub/help-center/integrations/what-is-higgsfield-mcp | 2026-08-01 | MCP (our path) | explicit (MCP always spends credits) |
| S17 | Text to Video AI | https://higgsfield.ai/text-to-video-ai | undated | video | landing copy — **not** treated as guidance |
| S18 | AI Prompt Generator | https://higgsfield.ai/ai-prompt-generator | undated | prompt tooling | landing copy — **not** treated as guidance |
| S19 | Higgsfield Help Center (index) | https://higgsfield.ai/creator-hub/help-center | — | — | navigation |
| S20 | Sitemaps used for enumeration | https://higgsfield.ai/sitemap.xml, /creator-hub/sitemap.xml, /blog/sitemap.xml, /sitemap-marketing.xml | — | — | discovery |

### First-party — machine-readable (class A′)

| # | Source | Retrieved | Content used |
|---|---|---|---|
| S21 | MCP tool schemas: `generate_image`, `generate_video`, `generate_audio`, `generate_video_batch` (server `https://mcp.higgsfield.ai/mcp`) | 2026-08-31 | `params` shape, `additionalProperties`, batch `requests[]` constraints, absence of `negative_prompt`/`width`/`height` from declared properties |
| S22 | `models_explore` `action:"list"`, `type:"video"`, limit 30 | 2026-08-31 | per-model `parameters`, `aspect_ratios`, `duration_range`/`durations`, resolution option lists |
| S23 | `models_explore` `action:"list"`, `type:"image"`, limit 40 | 2026-08-31 | same, for ~33 image models |

### Secondary sources — reviewed, cited for context only, never as guidance

| # | Title | URL | Note |
|---|---|---|---|
| T1 | Higgsfield AI Prompt Format Guide for High Quality Videos | https://blog.segmind.com/higgsfield-ai-prompt-guide-video-creation/ | Third-party (Segmind). Its claims — "short prompts produce stronger control than long ones," "separate camera movement into Veo or Sora" — are **contradicted** by S3/S6/S7, which publish long block prompts with camera direction inline. Not used. |
| T2 | Ultimate Higgsfield AI Guide 2025 | https://filmart.ai/higgsfield-ai-guide-higgsfield-ai-prompts/ | Third-party. Not used. |
| T3 | Higgsfield — Prompting Manual | https://higgesfieldcannesmanual.netlify.app/ | Third-party domain; the hostname misspells "Higgsfield". Not first-party. Not used. |
| T4 | higgsfield-ai-prompt-skill | https://github.com/OSideMedia/higgsfield-ai-prompt-skill | Third-party community skill. Not used. |
| T5 | higgsfield-ai/higgsfield | https://github.com/higgsfield-ai/higgsfield | Same org, unrelated product (distributed training). Not relevant. |
| T6 | higgsfieldai.gumroad.com | https://higgsfieldai.gumroad.com/l/higgsfieldai | Commercial prompt-pack listing. Not used. |

### Local artifacts inspected (read-only)

`SPEC.md`, `README.md`, `Cargo.toml`, `.claude/settings.json`, `data/negative_prompt_models.json`, `rust/src/{main,lint,checks,models,doctor}.rs`, `rust/tests/{parity,fail_silent,doctor}.rs`, `src/higgsfield_lint.py`, `tests/test_lint.py`, `tests/fixtures/{kling2_6_4k_slowmo,seedance_tree_clean}.json`. Behavioral probes were run against the built binary with inputs written to a scratch directory; no repository file was created, modified, or deleted.

---

## Keep — existing behavior supported by evidence

- **The whole system contract.** Advisory-only, exit 0, no stderr on the hook path, no network, Rust-only runtime, deterministic output, `doctor` dispatched before stdin. Verified in source; nothing here should change.
- **`resolution_claim` as a concept.** Higgsfield treats resolution as a setting changed while the prompt stays identical (S8, S10, S15). A prompt asserting a resolution the call will not produce is a real, mechanical contradiction — the *right* class of rule for this linter.
- **`resolution_claim`'s hard-won implementation details:** boolean-dimension exclusion, the 128 px floor keyed on the *larger* side (so 1920×100 still reports), `resolution`-param precedence, original-casing quoting, allocation-free boundary scanning. All correct and all tested.
- **`negation_placement`'s noise filters** — the deliberate exclusion of `not`, the `no more/longer/matter` stoplist, the bare `without + gerund` carve-out. Verified silent on `Film noir`, `is not moving`, `never-before-seen`, `no more than three`, `without moving`. If the rule is retargeted (HFR-006) these filters carry over unchanged.
- **`prompt_negative_conflict`'s curated-list discipline** and its message, which names the terms without dictating which field should win.
- **The batch semantics**, including the non-obvious rule that a non-empty `requests` array never falls back to top-level `params` — matched exactly between Rust and oracle, and covered by tests.
- **The parity harness as the port's verification instrument**, and its structural (not byte-level) comparison.
- **`seedance_tree_clean.json` as the anchor negative fixture.** It contains textbook Higgsfield exclusions and must stay silent.

## Reconsider — existing behavior that may be noisy or weakly supported

- **`negation_placement` in its entirety.** Contradicted by S1 and S3; five confirmed FP classes; message asserts two unverified claims. Highest-priority item in the document.
- **`data/negative_prompt_models.json = ["kling2_6"]`.** Seeded on the field being *sent*, not *honored*. The catalog (S22) shows `kling2_6` declaring only `duration` and `sound`. Either re-ground the file's meaning explicitly ("models observed to honor the field") and gather that evidence, or retire it with the rule.
- **The `negative_prompt`-is-present gate branch.** It makes the check self-triggering: supplying the field is treated as proof the field works.
- **`8k`, `uhd`, `full hd` in the claim lexicon.** `8k` is Higgsfield's own style vocabulary and has no attainable target; `uhd` and `full hd` appear in no official prompt and have never fired.
- **Single-token entries in `CONFLICT_PHRASES`** — `pan`, `text`, `sharp`, `static`, `zoom`, `grain`. Three confirmed FPs; the other three have obvious homographs.
- **Negation stripping applied to the prompt only.** `not sharp` in `negative_prompt` reads as a hit when it is agreement.
- **The `negation_placement` message's ≤3-word window**, which on unpunctuated text quotes spans nobody wrote (`"no logos a beautiful"`).
- **"Raise resolution"** as advice on models with no `resolution` parameter (`kling2_6`, `wan2_6`, `kling3_0`).
- **`SPEC.md` §9's `anyOf` claim** (over-broad — batch entries are object-only) and **§6's sample message** (says "Set resolution"; the code says "Raise resolution").
- **Running any text check on `generate_audio*`**, where `prompt` is the spoken line.

## Add — high-confidence deterministic rules missing from the implementation

1. **`duration_claim_conflict` (HFR-001, over-claim branch).** Class A. Prompt states a total duration greater than `duration`. Higgsfield explicitly instructs users to put total duration in the prompt; the check is arithmetic; it is invisible today.
2. **`resolution_ceiling_conflict` (HFR-002).** Class A + A′. Claim exceeds the model's published maximum, from a dated snapshot file. Closes 5.1's largest false negative and re-grounds the `kling2_6` "4K" case on the published ceiling rather than on undeclared passthrough dimensions.
3. **`aspect_ratio_claim_conflict` (HFR-003).** Class A. Prompt states exactly one numeric ratio differing from `aspect_ratio` (or from reduced `width:height`). Guarded so Higgsfield's own dual-ratio prompt stays silent.
4. **Resolution-lexicon extension.** Class A′. Teach `RESOLUTION_PARAM` the image and `minimax` vocabularies (`1k`, `1.5k`, `2k`, `512`, `768`, `2160p`) and read `quality`/`mode` where the catalog makes them the resolution carrier. Without this, rule 5.1 is dead on every image model.
5. **Tool-name scoping.** Class A′. Read `payload.tool_name` and skip prompt-text checks on `generate_audio*`.
6. **`doctor` checks: `binary_freshness`, `golden_selftest`, `data_file_present`.** Not prompt rules — the observability fix, delivered where the contract already allows output.
