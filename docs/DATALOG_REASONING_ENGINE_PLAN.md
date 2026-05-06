# Datalog Reasoning Engine Plan

## Purpose

This document captures the proposed role, integration point, rule-generation timing, and cost controls for introducing a Datalog-style reasoning engine into Mercurio.

The engine should be a derived semantic query and validation layer over compiled KIR. It should not replace KIR as the semantic contract or the existing graph as the primary runtime representation.

```text
SysML / KerML source
        |
        v
      KIR
        |
        v
 semantic graph
        |
        v
 extracted facts + versioned rule packs
        |
        v
 materialized derived indexes
        |
        v
 views / validation / impact analysis / explanations
```

## Design Boundary

KIR remains the canonical semantic compiler boundary.

Datalog owns derived reasoning:

- transitive specialization
- ownership and membership closure
- inherited features
- normalized relationship predicates
- requirements traceability
- coverage and validation diagnostics
- proposal impact analysis
- view-supporting query indexes

Datalog should not own:

- source parsing
- KIR emission
- accepted source authority
- expression evaluation
- source rewriting
- arbitrary user code execution

The guiding rule is:

```text
Models generate facts. Mercurio owns rules.
```

## Why Introduce It

A Datalog-style layer fits Mercurio where semantic behavior is relational, recursive, and explainable.

Good candidates include:

- `subtype(A, B)` through recursive `specializes` edges
- `owns(Owner, Child)` through `features`, `members`, and `owned_element`
- `inherited_feature(Type, Feature)` through ownership plus specialization
- `satisfies(Source, Requirement)` across direct edges and relationship elements
- `verifies(Source, Requirement)` across direct edges and relationship elements
- `impacted_element(Changed, Affected)` for proposal and PR analysis
- `missing_trace_evidence(Requirement)` for validation

The expected benefit is less duplicated graph-walk logic in runtime queries, views, validation, semantic comparison, and future server APIs. The engine can also provide explanations when derived facts retain their source facts and rule ids.

## Risks

The main risk is architectural gravity. If the rule engine becomes the hidden source of truth, the system becomes harder to debug and cache.

Specific risks:

- another representation to synchronize with KIR and `Graph`
- expensive joins or recursive closure on large workspaces
- unclear ownership between Rust code and generated rules
- diagnostics that lose source provenance
- rule packs becoming an untyped second language
- editor latency regressions if rules run on every keystroke
- difficult incremental invalidation if all facts are treated as one global database

The mitigation is to keep rule generation off the normal project hot path and to materialize bounded indexes from named rule packs.

## Rule Pack Types

### Core Rule Packs

Core rule packs are hand-authored and versioned with Mercurio runtime code.

They define stable semantic infrastructure:

- specialization closure
- ownership closure
- inherited features
- relationship normalization hooks
- impact traversal primitives
- common validation predicates

Example shape:

```text
subtype(A, B) :-
  edge(A, "specializes", B).

subtype(A, C) :-
  subtype(A, B),
  subtype(B, C).

owns(Owner, Child) :-
  edge(Owner, "features", Child).

owns(Owner, Child) :-
  edge(Owner, "members", Child).

inherited_feature(Type, Feature) :-
  owns(Type, Feature).

inherited_feature(Type, Feature) :-
  subtype(Type, Parent),
  owns(Parent, Feature).
```

### Metamodel Adapter Rule Packs

Metamodel adapter rule packs are generated from normalized library or metamodel artifacts. In the current implementation path, the first generation point should be Pilot stdlib import.

These rules normalize version-specific SysML/KerML concepts into stable Mercurio predicates.

Examples:

- which metaclasses count as requirements
- which kinds represent satisfy, verify, derive, refine, allocate, or dependency relationships
- which properties represent source and target
- which properties imply ownership, membership, features, or typing
- which metaclasses declare which attributes
- which relationship forms are direct edges versus relationship elements

The adapter rules should be generated from normalized stdlib/metamodel KIR or a metamodel descriptor, not from raw Pilot internals directly.

### View Query Rule Packs

Runtime-defined views may later compile into bounded rule packs or query plans.

This should come after hardcoded views prove the common projection shape. View-generated rules must be type-checkable, bounded, deterministic, and suitable for server, desktop, web, CI, and proposal contexts.

### Validation Policy Rule Packs

Project, profile, or package validation policies may compile into opt-in rule packs.

Examples:

- every authored requirement must have satisfy or verify evidence
- every safety requirement must link to a verification case
- every allocation must resolve to existing source and target elements

These packs should be named, versioned, and included in semantic artifact keys.

## Fact Generation

Project compilation should generate facts, not rules.

A KIR element such as:

```json
{
  "id": "type.Demo.Vehicle",
  "kind": "SysML::Systems::PartDefinition",
  "layer": 2,
  "properties": {
    "features": ["feature.Demo.Vehicle.engine"],
    "specializes": ["type.Base.Vehicle"]
  }
}
```

should become facts similar to:

```text
element("type.Demo.Vehicle").
kind("type.Demo.Vehicle", "SysML::Systems::PartDefinition").
layer("type.Demo.Vehicle", 2).
edge("type.Demo.Vehicle", "features", "feature.Demo.Vehicle.engine").
edge("type.Demo.Vehicle", "specializes", "type.Base.Vehicle").
```

Fact extraction should preserve provenance where available:

```text
source_file(Element, File).
source_span(Element, StartLine, StartCol, EndLine, EndCol).
fact_origin(FactId, Element, Property).
```

Facts should be generated from the existing `Graph` and KIR element properties so the rule engine remains downstream of the current semantic compiler boundary.

## Generation Timing

Rule generation should happen only when the semantic environment changes, not on every project compile.

| Time | Generate | Notes |
| --- | --- | --- |
| Runtime or code build | core rule packs | Hand-authored, reviewed, tested, benchmarked |
| Pilot stdlib import | metamodel adapter rule pack | Generated beside `resources/stdlib.kir.json` and keyed by stdlib hash |
| Native metamodel parse/import | metamodel adapter rule pack | Future replacement or supplement for Pilot-derived generation |
| Workspace/project compile | facts only | Hot path; do not regenerate rules |
| Profile/package load | optional validation/view packs | Named and versioned policy inputs |
| Proposal overlay compile | delta facts | Enables incremental proposal analysis |

For the current repository, the first implementation should extend `crates/mercurio-tools/src/bin/import_pilot_stdlib.rs` so import produces both:

```text
resources/stdlib.kir.json
resources/stdlib.rulepack.json
```

At workspace or server load:

```text
load stdlib KIR
load stdlib rule pack
compile/cache rules keyed by stdlib digest and rulepack digest
```

At project compile:

```text
compile project KIR
build graph
extract facts
apply cached rule packs
materialize derived indexes
```

## Semantic Artifact Keys

Semantic artifact keys should include rule-related inputs so derived reasoning remains reproducible.

Add or retain:

```text
compiler_version_or_digest
kir_schema_version
stdlib_digest
metamodel_rulepack_digest
core_rulepack_digest
validation_policy_digest
dependency_package_digests
mapping_rules_digest
```

This matches the existing server plan principle that semantic artifacts must vary when the compiler, stdlib, dependency set, or mapping files change.

## Cost Controls

The engine should be designed around bounded computation from the start.

Required controls:

- precompile and cache rule packs
- generate project facts incrementally from changed KIR or graph regions
- keep stdlib facts and project facts partitioned
- precompute stable library closures once per stdlib digest
- apply proposal overlays as delta facts over immutable base facts
- materialize hot closures instead of recomputing them per request
- invalidate by dependency neighborhood rather than whole workspace when possible
- run editor fast-path diagnostics separately from deep server analysis
- cap UI queries by rows, traversal depth, elapsed time, and output size
- reject or warn on broad unbound joins
- require indexed join patterns for expensive relations
- avoid recursive negation and unrestricted aggregation
- track per-rule timing and output cardinality

The first rule packs should be small enough to benchmark against the existing compile performance data.

## Initial Spike

The first implementation should be deliberately narrow.

Scope:

- add a rule-pack artifact format
- extend Pilot stdlib import to emit `resources/stdlib.rulepack.json`
- extract facts from `Graph`
- implement or wrap a minimal Datalog evaluation path
- materialize:
  - specialization closure
  - ownership closure
  - inherited features
  - requirement satisfy/verify normalization
- compare output against existing Rust graph queries and requirement table behavior
- add benchmarks for unchanged warm compile, one-file edit, and proposal overlay analysis

Success criteria:

- no project compile rule generation
- rule output is deterministic
- derived facts can explain their source facts and rule id
- warm unchanged compile remains cache-friendly
- one-file edit invalidates only affected fact partitions
- existing requirements table behavior can be reproduced from materialized rule output

## Non-Goals For The First Spike

- runtime-defined view language
- arbitrary project-authored recursive rules
- replacing `Graph`
- replacing KIR
- full SysML expression semantics
- source patch generation
- global whole-workspace reasoning on every editor edit

## Recommended Direction

Adopt Datalog as an optional derived reasoning layer first.

The implementation should prove value in traceability, inherited semantics, validation, and proposal impact before expanding into runtime-defined views or project policy packs.

