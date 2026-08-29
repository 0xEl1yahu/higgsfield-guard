# Preflight Gate — Implementation Pseudocode

Transcription of `PREFLIGHT_GATE_SPEC.md` into module structure, type signatures and
control flow. Rust-flavoured pseudocode, not compilable. Bodies are elided where obvious;
signatures are written out in full, because that is where the spec's invariants either
hold or leak.

Inline markers:

- `// SPEC GAP: Gn` — the spec does not say; an assumption is stated and carried.
- `// SPEC CONFLICT: Cn` — the spec says two incompatible things, or says something that
  cannot be implemented as written.

Both are collected at the end. Nothing here attempts to fix the spec.

---

## 1. `gate-core`

The request tuple, `Verdict`, `Violation`, and the `BlockAuthority` witness.

§4 claims the type system enforces constraints 4, 7 and 2. This module is written to make
that claim concrete, and to show precisely where it holds and where it does not.

### 1.1 Sealing primitives

```rust
// ===========================================================================
// crates/gate-core/src/seal.rs
// ===========================================================================

/// Private unit type. No other crate can *name* `Seal`, therefore no other
/// crate can write a struct literal for any type that owns a `Seal` field.
/// This is the only privacy primitive Rust actually offers across a crate
/// boundary. Everything below is built out of it.
struct Seal;

// Note what `Seal` does NOT do: it does not stop another crate from calling a
// `pub fn` in this crate that returns a sealed value. Literal construction and
// constructor-call are separate holes and must be closed separately.
```

### 1.2 Rule identity

```rust
// ===========================================================================
// crates/gate-core/src/rule.rs
// ===========================================================================

pub struct RuleId(String);                 // field private: ids come from registry parse
pub struct CatalogPath(String);            // e.g. "cinematic_studio_3_0.duration_range" (§7.3)

#[derive(Clone, Copy, PartialEq)]
pub enum Stage {
    Warn,      // §7.5: "New Class A rules start at `warn`. Always."
    Block,     // reachable only through gate-promote (§12)
    Suspended, // §19: dependency drift auto-suspension target
               // SPEC GAP: G1 — §7.3 says a changed dependency "auto-suspends the rule
               // to warn". "Suspended" and "warn" are used interchangeably. Assuming a
               // distinct Suspended state that *behaves* as Warn but is excluded from
               // promotion statistics, because folding it into Warn would let a
               // suspended rule re-promote off pre-drift evidence.
}

#[derive(Clone, Copy)]
pub enum PatchShape {
    Mechanical,    // suggested_patch populated (§7.5)
    Alternatives,  // suggested_patch: null, alternatives populated (§20)
}

/// A Class A rule. This is the *only* type from which block authority is mintable.
///
/// `_seal` is private, so `ClassARule { .. }` literal syntax does not compile in
/// gate-registry, gate-eval, gate-optimise, or anywhere else outside gate-core.
pub struct ClassARule {
    pub id:         RuleId,
    pub stage:      Stage,
    pub depends_on: Vec<CatalogPath>,
    pub shape:      PatchShape,
    pub predicate:  PredicateSpec,   // data, not code (§4 "Rules are data, never Rust code")
    _seal: Seal,
}

/// A Class B rule. Structurally near-identical to ClassARule and deliberately a
/// *different nominal type*. This is load-bearing: it is what makes the optimiser's
/// signatures unable to name a ClassARule (see 1.5).
pub struct ClassBRule {
    pub id:            RuleId,
    pub stage:         Stage,
    pub depends_on:    Vec<CatalogPath>,
    pub shape:         PatchShape,
    pub predicate:     PredicateSpec,
    pub budget_weight: f64,   // §11 "weighted by answer cost"
                              // SPEC GAP: G2 — §11 says alternatives-shaped rules "draw a
                              // reduced slice" of the confirm budget; no number is given.
                              // Assuming weight 1.0 for Mechanical and 2.0 for Alternatives
                              // (a choice consumes twice the budget of a yes/no), config-
                              // overridable, because any constant is invented either way.
    _seal: Seal,
}
```

### 1.3 The construction regress, stated honestly

```rust
// ===========================================================================
// The problem §4 does not address.
// ===========================================================================
//
// §4 crate layout puts rules in gate-registry ("catalog fetch, merge, TTL, rule
// loading") and puts ClassARule in gate-core. §4 also says rules are JSON data.
// So gate-registry must *build* ClassARule values from parsed JSON.
//
// gate-registry cannot use literal syntax (the `_seal` field is private to
// gate-core). Therefore gate-core must expose a constructor. The visibility of
// that constructor is the visibility of block authority itself:
//
//   pub(crate) fn new(..) -> ClassARule
//       => gate-registry cannot build rules. The registry is impossible.
//
//   pub fn new(..) -> ClassARule
//       => gate-optimise, which also depends on gate-core, can build a
//          ClassARule, hand it to BlockAuthority::mint, and construct
//          Verdict::Block. Constraint 7 is dead.
//
// Adding a capability token does not terminate the regress, it relocates it:
//
//   pub fn new(raw: RawRule, _t: &RegistryWriteToken) -> ClassARule
//   pub struct RegistryWriteToken { _seal: Seal }
//   impl RegistryWriteToken { pub fn ??? }   // <-- same two bad options
//
// SPEC CONFLICT: C1 — §4 states the BlockAuthority constructor is "private to
// gate-core". Rust privacy is crate-scoped, so a constructor private to gate-core
// is unreachable from gate-eval, which is a *separate crate* in the §4 layout.
// A constructor private to gate-core and callable by gate-eval does not exist.
// The mechanism as literally described cannot be built.
//
// What CAN be built, and what this document assumes from here on, is stated in
// 1.4 and 1.5. It delivers the same guarantee against the failure mode §14 names
// ("Class B leakage: quality rules inheriting block authority because they share
// a repo"), but it is delivered by the Cargo dependency graph and by return
// types, NOT by a private constructor.
```

### 1.4 `BlockAuthority`, as it can actually be built

```rust
// ===========================================================================
// crates/gate-authority/src/lib.rs        <-- NOT gate-core
//
// New crate, not in the §4 layout. Depends on gate-core. gate-eval depends on
// gate-authority. gate-registry depends on gate-authority. gate-optimise's
// Cargo.toml does NOT list gate-authority, and CI asserts that (1.6).
//
// A crate cannot name an item from a crate it does not depend on. That is a
// compile-time guarantee of exactly the same kind as `&T` vs `&mut T`: it is
// checked by the build, not by review. It is the strongest available mechanism.
// ===========================================================================

/// Witness that a block was authorised by a Class A rule at stage Block.
/// Opaque: no public fields, no public constructor, no Default, no Clone from
/// nothing. It can only come out of `mint`.
pub struct BlockAuthority {
    rule_id: RuleId,          // private: every block traces to a rule id (constraint 4)
    _seal: gate_core::Seal,   // re-exported pub(crate)-style marker; see note below
}

impl BlockAuthority {
    /// The single mint site in the system.
    ///
    /// Fallible on purpose: a Class A rule at stage Warn/Suspended has no block
    /// authority, so "is this rule promoted?" is answered here, once, rather
    /// than at every call site.
    pub fn mint(rule: &ClassARule) -> Option<BlockAuthority> {
        match rule.stage {
            Stage::Block => Some(BlockAuthority { rule_id: rule.id.clone(), _seal: .. }),
            Stage::Warn | Stage::Suspended => None,
        }
    }

    pub fn rule_id(&self) -> &RuleId { &self.rule_id }
}

// The residual leak, named rather than hidden:
//   `mint` is `pub`. Any crate that (a) depends on gate-authority and (b) holds
//   a `&ClassARule` can mint. gate-optimise fails (a) by construction of the
//   dependency graph. It also fails (b) by construction of the registry views
//   in 1.5. Both would have to be broken, and breaking (a) is a visible
//   Cargo.toml edit that CI rejects.
```

### 1.5 `Verdict`, and why `gate-optimise` cannot construct `Block`

```rust
// ===========================================================================
// crates/gate-core/src/verdict.rs
// ===========================================================================

/// The full verdict lattice (§7.5 table). Lives in gate-core because the
/// responder, the log and the proxy all need to name every variant.
pub enum Verdict {
    /// Contract violation, certain. gate-eval only.
    /// The BlockAuthority field is what makes this variant unconstructible
    /// without gate-authority: you cannot write `Verdict::Block { .. }` without
    /// producing a value of a type whose only constructor lives in a crate you
    /// do not depend on.
    Block {
        violation: Violation,
        authority: gate_authority::BlockAuthority,
    },
    RequireConfirm {
        violation:       Violation,
        confirmation_id: ConfirmationId,
    },
    Warn    { violation: Violation },
    Suggest { violation: Violation },
    Pass,
}

/// The optimiser's return type. Note the absence. This is the second, and in
/// practice the more useful, half of the guarantee: even if gate-optimise
/// somehow obtained a BlockAuthority, its own public signatures could not carry
/// a Block out of the crate.
pub enum ClassBVerdict {
    RequireConfirm { violation: Violation, confirmation_id: ConfirmationId },
    Warn           { violation: Violation },
    Suggest        { violation: Violation },
    Pass,
}

/// Total, and structurally incapable of producing Block. The proxy lifts
/// optimiser output into the shared lattice through here and nowhere else.
impl From<ClassBVerdict> for Verdict {
    fn from(b: ClassBVerdict) -> Verdict {
        match b {
            ClassBVerdict::RequireConfirm { violation, confirmation_id } =>
                Verdict::RequireConfirm { violation, confirmation_id },
            ClassBVerdict::Warn    { violation } => Verdict::Warn { violation },
            ClassBVerdict::Suggest { violation } => Verdict::Suggest { violation },
            ClassBVerdict::Pass                  => Verdict::Pass,
        }
    }
}
```

Summary of the enforcement, layer by layer:

| Layer | Mechanism | Stops |
|---|---|---|
| 1 | `Verdict::Block` carries `BlockAuthority`, which has private fields | literal construction of `Block` anywhere outside `gate-authority` |
| 2 | `BlockAuthority::mint` lives in `gate-authority`; `gate-optimise` does not depend on it | `gate-optimise` naming `mint` at all |
| 3 | `mint` returns `Option`, `None` unless `stage == Block` | a warn-stage rule blocking |
| 4 | `gate-optimise` returns `ClassBVerdict`, which has no `Block` variant | a `Block` crossing the crate boundary even if 1–3 were defeated |
| 5 | registry hands `gate-optimise` a `ClassBView` that yields only `&ClassBRule` | `gate-optimise` ever holding the `&ClassARule` that `mint` requires |

Layers 1, 3 and 4 are pure type-system. Layer 2 is the Cargo dependency graph. Layer 5 is
a type-system consequence of the view split, subject to `gate-optimise` not calling
`Registry::class_a_view()` directly, which its own signatures make impossible to do
*silently* but which the compiler alone would not prevent if someone wrote it deliberately.

```rust
// ===========================================================================
// crates/gate-core/src/violation.rs — the payload (§7.5 schema)
// ===========================================================================

pub struct Violation {
    pub rule_id:         RuleId,
    pub class:           Class,            // A | B
    pub field:           FieldPath,        // "duration", "medias", "aspect_ratio"
    pub observed:        serde_json::Value,
    pub allowed:         serde_json::Value,
    pub resolution:      Resolution,       // see below
}

/// §7.5: "suggested_patch is nullable ... only claim-versus-claim conflicts use
/// the alternatives form." Modelled as a sum type so "neither" is not
/// representable ... except that it must be, see C7.
pub enum Resolution {
    Patch(Patch),                 // mechanical, one line to describe
    Alternatives(Vec<Patch>),     // claim-vs-claim (§20)
    None,
    // SPEC CONFLICT: C7 — §7.5 says "Every Class A verdict and every
    // claim-versus-parameter verdict carries a patch; only claim-versus-claim
    // conflicts use the alternatives form." Two Class A rules in the spec can
    // produce neither:
    //   (a) the TikTok photo sink rule. §7.3: TikTok photo accepts JPEG/WebP
    //       only, "PNG rejected, and Higgsfield image gen emits PNG". There is
    //       no parameter to change; no Higgsfield image model can be asked for
    //       JPEG. The violation is real, provable, and unresolvable.
    //   (b) the §20 "model capability" row: "silent" on a model with no audio
    //       control. There is no parameter to patch and no alternative to
    //       offer.
    // `None` therefore has to exist, which contradicts "every Class A verdict
    // carries a patch".
}
```

### 1.6 The request tuple `R` (§5)

```rust
// ===========================================================================
// crates/gate-core/src/request.rs
// ===========================================================================

/// R = (tool, model_id, params, medias, destination, destination_source, budget_ctx)
///
/// Everything downstream assumes R is canonical (§5). "Canonical" is given
/// teeth by ResolvedParams below: defaults are already folded in and provenance
/// is preserved.
pub struct R {
    pub tool:               ToolName,
    pub model:              ModelResolution,
    // SPEC GAP: G3 — §5 writes `model_id`, implying a resolved id, but §7.2
    // permits `Ambiguous(Vec<model_id>)` and §18's variant walkthrough builds
    // an R for the ambiguous case (the call still forwards, and a verdict row
    // recording `normalisation_ambiguous` still gets logged). Assuming R
    // carries the *resolution*, not the id, because an R that cannot represent
    // ambiguity cannot be logged for the ambiguous path.
    pub params:             ResolvedParams,
    pub medias:             Vec<MediaRef>,
    pub destination:        Destination,        // tiktok | client_deck | ad_account | none
    pub destination_source: DestinationSource,  // explicit | default  (§5, load-bearing)
    pub budget_ctx:         BudgetCtx,
    // SPEC GAP: G4 — `budget_ctx` appears in the §5 tuple and is never defined
    // anywhere else in the document. §6 shows a cost gate with an "over budget"
    // edge and §15 step 8 pairs "get_cost lattice and budget threshold".
    // Assuming BudgetCtx = { per_call_credit_ceiling: Option<u32>,
    // session_spent: u32, session_ceiling: Option<u32> } sourced from proxy
    // config, because that is the minimum that makes the §6 edge evaluable.
    pub request_hash:       RequestHash,
    pub request_id:         RequestId,          // §18 step 1
}

pub enum ModelResolution {
    Resolved(ModelId),
    Ambiguous(Vec<ModelId>),   // constraint 8: forwards untouched
    NotApplicable,             // upscale_*, reframe, outpaint_image carry no model
                               // SPEC GAP: G5 — §7.1 lists the edit tools as in-scope
                               // for interception but §5's tuple mandates a model_id.
                               // Assuming NotApplicable, and that every model-specific
                               // predicate is skipped for these tools exactly as it is
                               // for Ambiguous.
    Unresolvable,              // name matched nothing in the catalog
                               // SPEC GAP: G6 — §7.2 defines only Resolved and Ambiguous.
                               // A name matching zero models is not covered. Assuming it
                               // behaves as Ambiguous (forward untouched) under constraint 8,
                               // since "uncertainty must never resolve toward blocking".
}

/// Provenance-preserving parameter value. §20 turns on this distinction:
/// "there is no unspecified parameter, only an unstated default".
pub enum ParamValue {
    Explicit(Json),        // the caller wrote it
    Defaulted(Json),       // catalog `default` supplied it
    Unset,                 // catalog publishes no default for this param
    // SPEC CONFLICT: C2 — §20 asserts that once defaults are resolved "there is
    // no unspecified parameter, only an unstated default". §18 step 5 then
    // reports `observed: {"prompt_claim": "vertical", "resolved": null}` for
    // aspect_ratio, i.e. a parameter that resolved to nothing because
    // cinematic_studio_3_0 publishes aspect_ratios but no aspect_ratio default.
    // Unset must exist, so §20's collapse of incompleteness into contradiction
    // is only total for parameters that publish a default.
}

pub struct ResolvedParams {
    map: BTreeMap<FieldPath, ParamValue>,   // BTree: deterministic hash ordering
    unknown: BTreeMap<String, Json>,        // §4: serde must not silently drop
                                            // unmodelled fields; they are carried
                                            // through to upstream verbatim
}

impl ResolvedParams {
    /// The comparison surface §20 needs: value regardless of provenance.
    pub fn resolved(&self, f: &FieldPath) -> Option<&Json>;
    /// The provenance surface §20 needs: was it stated or inferred?
    pub fn provenance(&self, f: &FieldPath) -> Provenance;
}

pub struct MediaRef {          // §7.2: media refs to (id, kind, status) triples
    pub id:     Uuid,
    pub kind:   MediaKind,     // media_input | image_job | video_job
    pub status: MediaStatus,   // completed | processing | ip_checking | nsfw | failed | Unknown
    pub role:   Option<Role>,  // start_image, image, ...
    pub raw:    String,        // pre-normalisation, so media_value_is_uuid can see an https URL
}
```

### 1.7 Constraint 2: no short-circuit, enforced by return type

```rust
// ===========================================================================
// crates/gate-core/src/predicate.rs
// ===========================================================================

/// §4: "Predicates return `Option<Violation>`; the evaluator returns
/// `Vec<Violation>`. There is no `?` to early-return through."
///
/// This holds, narrowly. `?` on an Option is only well-typed inside a function
/// returning Option/Try, and the evaluator returns Vec, so `predicate(..)?`
/// there is a type error. It does not stop `return` or `break`, so the claim is
/// "no accidental short-circuit via the ergonomic path", not "short-circuit is
/// unrepresentable".
pub type PredicateFn = fn(&Registry, &R) -> Option<Violation>;

// Lint fence, since the type system stops the ergonomic hole and not the rest:
#![deny(clippy::needless_return)]
// plus a CI grep asserting no `return` / `?` inside crates/gate-eval/src/predicates/.
```

### 1.8 Constraint 4: registry read-only to the evaluator

```rust
// Enforced entirely at the signature level. These are the only two shapes that
// touch a Registry in the whole system:

pub fn evaluate(reg: &Registry,      r: &R)                 -> Vec<Verdict>;         // gate-eval
pub fn run_promotion(reg: &mut Registry, log: &OutcomeLog)  -> PromotionReport;      // gate-promote

// SPEC CONFLICT: C3 — the `&Registry` guarantee is vacuous the moment anything
// in the Registry needs interior mutability, and three things in the spec do:
//   (a) §10's get_cost lattice: "Memoise per (model, param-shape) and refresh
//       on the registry TTL". Memoisation behind `&Registry` requires a
//       Mutex/RefCell/OnceCell, at which point gate-eval holds a `&Registry`
//       that can be written through.
//   (b) §7.3's TTL refresh and §19's per-TTL contract tests, which mutate rule
//       stage (auto-suspension) from a path that is not gate-promote, directly
//       contradicting constraint 4's "Only the promotion job writes rules".
//   (c) §11's trailing-window confirm-budget counters and §19's holdout RNG /
//       epsilon schedule, both of which are per-call mutable state.
// Assumed resolution, carried through this document: Registry is genuinely
// immutable, held as `Arc<Registry>` and hot-swapped wholesale on refresh
// (arc-swap). (a) and (c) move OUT of Registry into separate `&mut`-owned
// stores. This preserves the constraint but makes "only gate-promote holds
// &mut Registry" true in a trivial sense: nobody holds &mut Registry, because
// Registry is never mutated at all.

// SPEC CONFLICT: C4 — §6's flowchart routes `COST -->|"over budget"| LLM`,
// i.e. the cost gate can stop a call from being forwarded. §1's table says
// `block` authority lives in gate-eval alone, and §7.5's table says `block` is
// emitted by "gate-eval only". The budget threshold is therefore a third
// blocking authority that the authority model does not account for, and it is
// not a contract violation, so it cannot be a Class A rule. Assumed resolution:
// the budget stop is modelled as a distinct `Verdict`-adjacent outcome
// (`BudgetHalt`) emitted by gate-proxy, carrying no BlockAuthority, logged
// separately, and never counted in the false-block metric.
```

---

## 2. `gate-registry`

Catalog load, merge, TTL, fail-open, `depends_on` dependency diffing and auto-suspension.

```rust
// ===========================================================================
// crates/gate-registry/src/lib.rs
// ===========================================================================

pub struct Registry {
    pub version:     RegistryVersion,   // monotonic, bumped on every successful build
    pub fetched_at:  Instant,
    pub ttl:         Duration,
    catalog:         Catalog,           // from models_explore(action='get')
    class_a:         Vec<ClassARule>,   // private: only reachable through class_a_view()
    class_b:         Vec<ClassBRule>,
    sinks:           BTreeMap<Destination, SinkProfile>,
}

/// §7.3: three sources merged. Ordering is fixed and total, so a merge is
/// reproducible and a version diff is meaningful.
///   1. catalog  -> derives Membership and Presence predicates (§7.4)
///   2. rules JSON (seed §8 + mined §8) -> ClassARule / ClassBRule
///   3. sink profiles JSON -> SinkProfile
pub fn build(
    catalog:   Catalog,
    rules_src: &Path,
    sinks_src: &Path,
    prev:      Option<&Registry>,     // for depends_on diffing (§7.3, §19)
) -> Result<Registry, RegistryError>;
```

### 2.1 Freshness and fail-open (constraint 3)

```rust
pub enum Freshness {
    Fresh,
    Stale { age: Duration },   // constraint 3: warn and forward, never block
}

impl Registry {
    pub fn freshness(&self, now: Instant) -> Freshness;
}

/// §18 step 3: "Had it been stale, evaluation stops here and the call forwards
/// with a warning (constraint 3), no exceptions."
///
/// SPEC GAP: G7 — §18 stops *evaluation*. It does not say whether the OPTIMISER
/// also stops on a stale registry. The optimiser reads the same registry for
/// the genre lexicon (§9), sink floors (§10) and, critically, parameter
/// defaults (§20). Assuming the whole gate degrades to passthrough on stale,
/// because a Band A contradiction computed against a default the registry can
/// no longer vouch for is exactly the false-positive §9 is built to avoid.
///
/// SPEC GAP: G8 — §7.3 gives no TTL value and §17's config exposes no TTL flag.
/// Assuming 6h, config-overridable, chosen only because it is short enough that
/// §19's per-TTL contract tests run at a useful rate.
```

### 2.2 Registry views: the layer-5 barrier from §1.5

```rust
/// gate-eval receives this. Only source of `&ClassARule` in the system.
pub struct ClassAView<'r> { reg: &'r Registry }

/// gate-optimise receives this. Structurally cannot yield a &ClassARule.
pub struct ClassBView<'r> { reg: &'r Registry }

impl Registry {
    pub fn class_a_view(&self) -> ClassAView<'_>;
    pub fn class_b_view(&self) -> ClassBView<'_>;
}

impl<'r> ClassAView<'r> {
    pub fn rules(&self)                       -> impl Iterator<Item = &'r ClassARule>;
    pub fn catalog(&self)                     -> &'r Catalog;
    pub fn sink(&self, d: Destination)        -> Option<&'r SinkProfile>;
}

impl<'r> ClassBView<'r> {
    pub fn rules(&self)                       -> impl Iterator<Item = &'r ClassBRule>;
    pub fn catalog(&self)                     -> &'r Catalog;   // needs defaults + genre enum (§9)
    pub fn sink(&self, d: Destination)        -> Option<&'r SinkProfile>;
    // deliberately absent: any accessor returning &ClassARule
}
```

### 2.3 Catalog model

```rust
pub struct Catalog { models: BTreeMap<ModelId, ModelSpec>, name_index: NameIndex }

pub struct ModelSpec {
    pub id:             ModelId,
    pub friendly_names: Vec<String>,         // non-injective (§7.2 hazard)
    pub aspect_ratios:  Vec<AspectRatio>,
    pub duration:       DurationSpec,
    pub parameters:     BTreeMap<FieldPath, ParamSpec>,
    pub medias:         Vec<MediaSlotSpec>,  // roles, max, required
    pub supports_unlim: bool,                // §10
}

/// §7.3: "Duration is a sum type, not a list."
pub enum DurationSpec {
    Enum(Vec<u32>),               // cinematic_studio_video: [5, 10]
    Range { min: u32, max: u32 }, // cinematic_studio_3_0: {4, 15}
    Unconstrained,                // clipify
}

pub struct ParamSpec {
    pub ty:       ParamType,          // bool | int | float | string | enum
    pub options:  Option<Vec<Json>>,
    pub min:      Option<Json>,
    pub max:      Option<Json>,
    pub default:  Option<Json>,       // None => ParamValue::Unset (see C2)
    pub required: bool,
}

pub struct MediaSlotSpec { pub roles: Vec<Role>, pub max: Option<u32>, pub required: bool }
```

### 2.4 `depends_on` diffing and auto-suspension (§7.3, §19)

```rust
/// A rule declares depends_on: ["cinematic_studio_3_0.duration_range", ...].
/// §19: the assertion is TWO-part, "the field exists, and its shape matches
/// what the predicate assumes". Shape is therefore part of the dependency, not
/// just the path.
pub struct DependencyFingerprint {
    pub path:   CatalogPath,
    pub exists: bool,
    pub shape:  ShapeTag,   // Range | Enum | Unconstrained | Bool | Int | StringEnum | Absent
    pub digest: u64,        // hash of the field's *value*, e.g. the {4,15} bounds
}

pub fn fingerprint(catalog: &Catalog, path: &CatalogPath) -> DependencyFingerprint;

/// Run once per registry build, against the previous registry's catalog.
pub fn diff_dependencies(prev: &Catalog, next: &Catalog, rules: &[&dyn HasDependsOn])
    -> Vec<DependencyChange>;

pub struct DependencyChange {
    pub rule_id: RuleId,
    pub path:    CatalogPath,
    pub kind:    ChangeKind,   // Vanished | ShapeChanged | ValueChanged
}

/// §7.3: "a changed dependency auto-suspends the rule to warn pending
/// re-verification (§19)". §19 adds "ε 0.25 for 14 days" on the same trigger.
pub fn apply_suspensions(next: &mut RegistryUnderConstruction, changes: &[DependencyChange]) {
    for c in changes {
        // ShapeChanged: §19's worked case, "a duration_range that becomes a
        // durations enum should suspend the rule, not silently mis-evaluate it".
        // ValueChanged: bounds moved but shape held.
        // SPEC GAP: G9 — §7.3 says "a changed dependency" without distinguishing
        // shape change from value change. Suspending on every ValueChanged means
        // any catalog tweak to any bound demotes every promoted range rule, and
        // §12's promotion needs n>=30 fresh firings to climb back, so a rule
        // could ping-pong indefinitely. Assuming: Vanished and ShapeChanged
        // suspend to Warn; ValueChanged keeps the stage but raises holdout
        // epsilon to 0.25 for 14 days (§19), because the predicate is still
        // structurally valid, only its constants moved.
        rule.stage = Stage::Suspended;
        rule.epsilon_override = Some(EpsilonOverride { eps: 0.25, until: now + 14.days() });
        // SPEC GAP: G10 — "pending re-verification" is never defined. There is
        // no described mechanism that lifts a suspension other than §12's normal
        // promotion path. Assuming a suspended rule re-enters at Warn, discards
        // its pre-change (k, n), and must re-earn Block through §12 from zero,
        // because reusing counts gathered against a different catalog shape is
        // exactly the unsoundness the suspension exists to prevent.
    }
}

// SPEC CONFLICT: C5 — this function mutates rule stage, and it runs on the TTL
// refresh path, not in gate-promote. Constraint 4 says "Only the promotion job
// writes rules." §19 also runs contract tests "every TTL refresh", which is the
// same path. Under the arc-swap resolution in C3 this is nominally fine (a new
// Registry is built rather than an old one mutated), but the *decision* to
// change a rule's stage is being made outside gate-promote, which is the
// substance of constraint 4 rather than its letter.
```

### 2.5 Rule loading

```rust
/// §4: rules are JSON, so tuning a predicate never triggers a rebuild.
/// The engine is fixed; the data selects from PredicateSpec's closed set.
pub enum PredicateSpec {
    Membership  { field: FieldPath, source: CatalogPath },
    Implication { antecedent: Box<PredicateSpec>, consequent: Box<PredicateSpec> },
    Projection  { property: ProjectedProperty, against: SinkField },
    Presence    { field: FieldPath, requires: Requirement },
    Range       { field: FieldPath, source: CatalogPath },
    // SPEC CONFLICT: C6 — §7.4 states there are "Four predicate shapes"
    // (membership, implication, projection, presence) and §6's diagram lists
    // three. §18 step 4 then evaluates a fifth shape it labels "*Range:*"
    // (`duration: 30` against `duration_range {4,15}`) and attributes the whole
    // worked block to it. A range check over DurationSpec::Range is not a
    // membership test over a finite option list, and §7.3 explicitly models
    // duration as Enum | Range | Unconstrained, so the two cannot be collapsed
    // without either discretising the interval or abandoning the sum type.
    // Modelled here as a fifth shape.
    MediaCount  { role: Option<Role>, max: u32 },   // derived from MediaSlotSpec.max
}

pub fn load_rules(path: &Path) -> Result<(Vec<ClassARule>, Vec<ClassBRule>), RuleParseError>;
// Class is a field in the JSON. gate-registry is the only crate that can
// construct either type (it depends on gate-authority; see §1.4).
```

### 2.6 Refresh loop

```rust
pub struct RegistryHandle(arc_swap::ArcSwap<Registry>);   // per C3

impl RegistryHandle {
    pub fn current(&self) -> Arc<Registry>;   // cheap, lock-free, per-request

    /// Background task. Never blocks a request.
    pub async fn refresh(&self, upstream: &UpstreamClient) -> RefreshOutcome {
        let catalog = match upstream.models_explore_get().await {
            Ok(c)  => c,
            Err(e) => return RefreshOutcome::FetchFailed(e),
            // Constraint 3: on fetch failure the OLD registry stays live and ages
            // past TTL, at which point freshness() reports Stale and the gate
            // fails open. Failure to refresh is never itself a block.
        };
        let prev    = self.current();
        let changes = diff_dependencies(prev.catalog(), &catalog, prev.all_rules());
        let next    = build(catalog, RULES_PATH, SINKS_PATH, Some(&prev))?;
        run_contract_tests(&next);   // §19, fourth regime
        self.0.store(Arc::new(next));
        RefreshOutcome::Swapped { changes }
    }
}
```

---

## 3. `gate-normalise`

Fallible model-name resolution, width/height folding, media ref triples, **default
resolution**, `request_hash`.

```rust
// ===========================================================================
// crates/gate-normalise/src/lib.rs
//
// SPEC GAP: G11 — §4's crate layout has no crate for the normaliser, although
// §6 and §7.2 make it a first-class component sitting between the interceptor
// and both the evaluator and the optimiser. Assuming a dedicated crate,
// because folding it into gate-proxy would put the ClassAView/ClassBView split
// and the R constructor inside the binary, and folding it into gate-core would
// make gate-core depend on gate-registry.
// ===========================================================================
```

### 3.1 Ordering is the whole design

§20 rests on defaults being resolved before any comparison runs. §18 shows default
resolution happening in two different places downstream. Resolving that is the point of
this module.

```rust
// SPEC CONFLICT: C8 — where defaults are resolved is stated three different ways.
//   §7.2 lists the normaliser's duties as exactly three items (name -> model_id,
//        w/h -> aspect_ratio, media refs -> triples). Default resolution is not
//        among them.
//   §20  says "Once the normaliser resolves defaults from the catalog, there is
//        no unspecified parameter, only an unstated default", placing it in the
//        normaliser and making it a precondition of every §20 comparison.
//   §18  shows it happening downstream twice: step 4 has the EVALUATOR resolving
//        `resolution` to its default 720p, and step 5 has the OPTIMISER resolving
//        `generate_audio` to its default false.
// These cannot all hold. If evaluator and optimiser each resolve defaults
// independently they can disagree, and §18 step 5's cross-class dedup ("Class A
// already owns that field") assumes they agree on the resolved value.
// Resolution carried here: defaults are resolved ONCE, in the normaliser, per
// §20, and both downstream consumers read ResolvedParams. §18's phrasing is
// treated as narrative rather than placement.

/// The pipeline. Order is load-bearing at every step and each step is written to
/// state what it depends on.
pub fn normalise(
    reg:  &Registry,          // &, not &mut: constraint 4 holds here too
    raw:  RawToolCall,
    cfg:  &ProxyConfig,
) -> Normalisation {

    // --- Step 0: freshness gate (constraint 3, §18 step 3) -----------------
    // Checked BEFORE default resolution, not after. A stale catalog's defaults
    // are exactly as untrustworthy as its membership lists, and §20 turns a
    // wrong default into a require_confirm the caller did not earn.
    if let Freshness::Stale { age } = reg.freshness(now()) {
        return Normalisation::FailOpen { raw, reason: StaleRegistry(age) };
    }

    // --- Step 1: destination and its provenance (§5) -----------------------
    let (destination, destination_source) = match raw.take("destination") {
        Some(d) => (parse_destination(d)?, DestinationSource::Explicit),
        None    => (cfg.default_destination, DestinationSource::Default),
    };
    // §5: a Default source caps sink-profile rules at `warn`. Carried in R,
    // consumed in gate-eval 4.4.

    // --- Step 2: model name resolution, FALLIBLE (§7.2 hazard) -------------
    // Must run before anything model-specific, because every later step in this
    // function reads the per-model catalog entry.
    let model = resolve_model_name(reg.catalog(), raw.get("model"), raw.tool);

    // --- Step 3: dimension folding (§7.2) ----------------------------------
    // Runs before default resolution so that an explicit width/height counts as
    // an EXPLICIT aspect_ratio and does not get overwritten by a catalog default.
    let mut params = fold_dimensions(raw.params);

    // --- Step 4: media refs to triples (§7.2) ------------------------------
    let medias = resolve_media_refs(&raw.medias, reg, upstream)?;

    // --- Step 5: DEFAULT RESOLUTION (§20) ----------------------------------
    // The precondition for every comparison in gate-eval and gate-optimise.
    // Skipped entirely when `model` is not Resolved: defaults are per-model, and
    // guessing them across an Ambiguous pair is precisely the misresolution
    // constraint 8 forbids.
    let params = match &model {
        ModelResolution::Resolved(id) => resolve_defaults(reg.catalog().get(id), params),
        _                             => params.all_explicit_or_unset(),
    };

    // --- Step 6: canonical hash (§10) --------------------------------------
    let request_hash = hash_canonical(&model, &params, &medias, &destination, raw.tool);

    Normalisation::Canonical(R { tool: raw.tool, model, params, medias,
                                 destination, destination_source,
                                 budget_ctx: cfg.budget_ctx(),
                                 request_hash, request_id: RequestId::new() })
}

pub enum Normalisation {
    Canonical(R),
    /// Forward untouched, log the reason, run no predicates (constraint 3 / 8).
    FailOpen { raw: RawToolCall, reason: FailOpenReason },
}

pub enum FailOpenReason { StaleRegistry(Duration), NormalisationAmbiguous(Vec<ModelId>) }
// §18: the ambiguous path logs `normalisation_ambiguous`.
// Note that Ambiguous does NOT produce FailOpen here: R is still built (with
// model: Ambiguous) so the call can be logged and hashed. It is gate-eval and
// gate-optimise that skip every model-specific predicate. See 4.1.
```

### 3.2 Model name resolution

```rust
/// §7.2: not injective. "Cinema Studio Video" maps to both
/// cinematic_studio_video and cinematic_studio_video_v2, which disagree on
/// exactly the fields the gate checks.
pub fn resolve_model_name(cat: &Catalog, name: Option<&str>, tool: ToolName)
    -> ModelResolution
{
    let Some(name) = name else {
        return match tool.requires_model() {
            false => ModelResolution::NotApplicable,   // upscale_*, reframe, outpaint
            true  => ModelResolution::Unresolvable,    // G6
        };
    };
    match cat.name_index.lookup(&canonical_name(name)) {
        []      => ModelResolution::Unresolvable,        // G6
        [one]   => ModelResolution::Resolved(one.clone()),
        many    => ModelResolution::Ambiguous(many.to_vec()),
    }
}

/// SPEC GAP: G12 — §7.2 gives no matching rule. "Cinema Studio Video 3.0" must
/// reach `cinematic_studio_3_0` while "Cinema Studio Video" must reach BOTH v1
/// and v2, which means matching cannot be pure substring (else "Cinema Studio
/// Video" would also match 3.0 and produce a three-way ambiguity that §18's
/// worked variant explicitly says is a two-way one). Assuming exact match on a
/// case-folded, punctuation-stripped canonical form against the catalog's
/// published names only, with no fuzzy fallback, because any fuzziness here
/// converts a safe Ambiguous into an unsafe Resolved.
fn canonical_name(s: &str) -> String;   // lowercase, collapse whitespace, strip [-_.]
```

### 3.3 Dimension folding

```rust
/// §7.2 and §10: "Nano Banana Pro" + width/height must collide with
/// nano_banana_pro + aspect_ratio under the content hash.
fn fold_dimensions(mut p: RawParams) -> ResolvedParams {
    if let (Some(w), Some(h)) = (p.take_int("width"), p.take_int("height")) {
        let ar = AspectRatio::nearest_canonical(w, h);
        // SPEC GAP: G13 — no tolerance is specified for w/h to aspect_ratio.
        // 1920x1080 is exactly 16:9; 1080x1350 is 4:5, which no Higgsfield model
        // publishes. Assuming exact-ratio match only, and that a non-canonical
        // ratio leaves aspect_ratio Unset and preserves width/height in
        // `unknown` for passthrough, because snapping 4:5 to 1:1 would silently
        // change the artifact, which §14's "silent downgrade" row forbids.
        // SPEC GAP: G14 — §9's Aspect claim is "Compared against `aspect_ratio`,
        // `width`/`height`", but if folding is total then width/height no longer
        // exist by the time §9 runs. Assuming §9 compares against the folded
        // aspect_ratio and, when folding failed, against the retained raw pair.
        if let Some(ar) = ar { p.set_explicit("aspect_ratio", ar); }
    }
    p.into_resolved()
}
```

### 3.4 Media ref triples

```rust
/// §7.2: media refs to (id, kind, status).
///
/// SPEC CONFLICT: C9 — `status` is not present in the caller's tool call. The
/// caller supplies an id. Two Class A rules in §8 depend on status:
/// `element_status_completed` ("must be status: completed, not
/// processing/ip_checking/nsfw") and `job_ref_terminal` ("must be terminal AND
/// successful"). Obtaining status requires an upstream read call
/// (show_reference_elements / show_medias / job_display) during preflight. The
/// spec never says this. It also collides directly with §19's performance
/// budget: "added latency on the happy path (target under 10ms)". One upstream
/// HTTP round trip is one to two orders of magnitude over that budget, on every
/// call carrying a media reference.
///
/// Resolution carried here: a bounded TTL cache of media/job status, populated
/// opportunistically from read-tool traffic the proxy already sees (§3: the gate
/// proxies jobs_wait, job_display, show_generations, show_generation_by_ids), and
/// MediaStatus::Unknown on a miss. Unknown never blocks, per constraint 8.
fn resolve_media_refs(raw: &[RawMedia], reg: &Registry, up: &UpstreamClient)
    -> Result<Vec<MediaRef>, NormaliseError>
{
    raw.iter().map(|m| MediaRef {
        raw:    m.value.clone(),                     // kept so media_value_is_uuid can see a URL
        id:     Uuid::parse(&m.value).ok(),          // None => the rule fires on `raw`
        kind:   infer_kind(m),                       // media_input | image_job | video_job
        status: status_cache.get(&m.value).unwrap_or(MediaStatus::Unknown),
        role:   m.role.clone(),
    }).collect()
}
```

### 3.5 Default resolution (§20's precondition)

```rust
/// The step §20 depends on. Produces the tri-state in §1.6.
///
/// Invariant asserted here and relied on everywhere downstream:
///   for every field in spec.parameters, params.get(field) is Explicit,
///   Defaulted, or Unset. Never absent-from-the-map.
fn resolve_defaults(spec: &ModelSpec, mut p: ResolvedParams) -> ResolvedParams {
    for (field, pspec) in &spec.parameters {
        match (p.map.get(field), &pspec.default) {
            (Some(ParamValue::Explicit(_)), _) => { /* caller wins, always */ }
            (None, Some(d))                    => p.set(field, ParamValue::Defaulted(d.clone())),
            (None, None)                       => p.set(field, ParamValue::Unset),   // see C2
            _                                  => {}
        }
    }
    p
    // Not done here: cross-parameter default interaction. §9's table notes
    // `sound: "on"` is "(v2 default)" and `generate_audio: true` is "(marketing
    // default)", i.e. the same claim compares against different fields on
    // different models.
    // SPEC GAP: G15 — the spec never says whether a default can be conditional
    // on another parameter (e.g. a resolution default that varies by duration).
    // Assuming defaults are unconditional per-field values as published in
    // `parameters[].default`, because ModelSpec offers no way to express
    // anything else.
}
```

### 3.6 `request_hash`

```rust
/// §10: "Hash the canonical R post-normaliser."
///
/// SPEC GAP: G16 — the spec never says which fields enter the hash, and the
/// choice changes behaviour in three visible ways:
///   - include Defaulted values? If yes, an explicit `resolution: 720p` collides
///     with an omitted one, which is what makes the cache useful. If no, the
///     "same call twice" case misses whenever the caller varies explicitness.
///   - include `destination`? It never reaches upstream, so two calls differing
///     only in destination produce byte-identical artifacts, yet §13's `calls`
///     table stores `destination` per row.
///   - include `request_id`/nonce? Must not, or the cache never hits.
/// Assuming: tool + resolved model_id + all Explicit and Defaulted param values
/// (canonically ordered, Unset omitted) + media ids and roles. destination and
/// destination_source EXCLUDED, because the hash addresses the artifact and the
/// destination does not affect it.
fn hash_canonical(..) -> RequestHash;   // blake3 over a canonical CBOR encoding

// SPEC CONFLICT: C10 — §13 declares `calls(request_hash, job_id, submitted_at,
// r_json, projection_json, cost_estimate)` with request_hash first, reading as
// the primary key. §10's content-addressed cache exists precisely because the
// same canonical R recurs, and it explicitly expects "a repeat call is
// frequently a deliberate reroll". A deliberate reroll is the same
// request_hash with a different job_id, so request_hash cannot be unique in
// `calls`. Assuming job_id is the primary key and request_hash is a non-unique
// index.
