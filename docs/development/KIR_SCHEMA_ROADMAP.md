# KIR Schema Roadmap

Status: active contract roadmap.

## Purpose

KIR is already the semantic handoff between frontends and runtime services. This roadmap defines the remaining work needed to make that handoff a stricter, versioned, testable schema.

The current implementation contract is documented in [KIR Specification](KIR_SPEC.md).

## Goals

- Define a formal JSON Schema for `KirDocument` and `KirElement`.
- Version the KIR document contract.
- Stabilize `expression_ir`.
- Stabilize source provenance metadata.
- Define a field contract for scalar data, references, arrays, metadata, and expression data.
- Derive kind-family and capability validation from metamodel facts instead of maintaining duplicate kind-profile tables.

## Compatibility Policy

Do not preserve backward compatibility while tightening this contract.

The repository is still early enough that KIR schema work should prefer a clean semantic boundary over compatibility shims. Breaking changes should be explicit, versioned, and reflected in generated resources, tests, packages, and derived artifacts. Old KIR artifacts may be regenerated from source instead of migrated.

## Current Flexible Areas

KIR currently allows flexible JSON properties. That is useful while frontend coverage grows, but downstream services increasingly rely on stable fields:

- graph edge discovery
- runtime expression evaluation
- constraints
- Datalog fact extraction
- views and diagrams
- semantic comparison
- server cache identity
- verification evidence
- package provenance

The schema work should tighten these contracts without duplicating SysML/KerML metamodel information in static kind-profile tables.

## Design Direction

Separate three concerns:

```text
KIR field contract -> graph construction -> derived metamodel validation
```

- The KIR field contract is hand-authored because it is Mercurio-specific: it defines which property names are scalars, references, reference arrays, expression IR, metadata, or extension fields.
- Graph construction uses only the field contract. It must not depend on kind-family inference, because metamodel inference itself needs graph edges.
- Kind-family behavior is derived after graph construction from KIR `kind`, `metatype`, `specializes`, and the bundled metamodel/stdlib graph.
- Detailed legality remains in the metamodel/rule layer, not in duplicated JSON schema tables.

This means there should be no static milestone that manually lists required fields for every supported SysML/KerML kind. If the runtime needs to know whether an element is classifier-like, feature-like, relationship-like, or package-like, it should infer that from metamodel facts when those facts are available.

## Schema Milestones

### Milestone 1: Document Envelope

Define:

- required `version`
- required `elements`
- optional `metadata`
- metadata fields for source set, compiler, stdlib, mappings, and schema version
- duplicate-id and supported-layer validation

Exit condition:

- KIR documents declare a schema version and validate through one schema entry point.

### Milestone 2: Element Core

Define:

- `id`
- `kind`
- `layer`
- `properties`
- `properties.metadata`
- source provenance fields

Exit condition:

- Every KIR element has stable identity, kind, layer, and validated provenance shape when provenance is present.

### Milestone 3: Field Contract

Define which properties are semantic fields and how each field is shaped.

Initial candidates:

- `owner`
- `owning_type`
- `type`
- `features`
- `members`
- `specializes`
- `subsets`
- `redefines`
- `imports`
- `source`
- `target`
- `declared_name`
- `name`
- `documentation`
- `expression_ir`
- `metadata`

Field kinds:

- scalar string
- scalar number
- scalar boolean
- reference id
- reference id array
- structured expression IR
- structured metadata
- preserved extension data

Exit condition:

- Graph edge discovery is driven by the field contract rather than ad hoc string matching.
- Non-reference string fields never create accidental graph edges.
- Unknown fields are rejected unless they use an explicit extension namespace such as `x_*`.

### Milestone 4: Expression IR

Define the `expression_ir` schema:

- `literal`
- `self`
- `path`
- `tuple`
- `unary`
- `binary`
- `call`
- comparison and constraint forms, if kept in expression IR
- source span or provenance hooks where needed

Exit condition:

- Frontend emission, runtime evaluation, constraint rendering, simulation guards, and verification assertions share one expression contract.

### Milestone 5: Derived Metamodel Validation

Derive broad semantic capabilities from the metamodel graph instead of maintaining static kind profiles.

Derived capabilities should answer questions such as:

- is this element classifier-like?
- is this element feature-like?
- is this element package- or namespace-like?
- is this element relationship-like?
- can this element own features?
- can this element have source and target endpoints?

Implementation shape:

```text
element.kind / metatype / specializes
  -> graph lookup against stdlib/metamodel facts
  -> inferred semantic capabilities
  -> validation diagnostics
```

The validation layer may check that common semantic expectations are satisfied, for example:

- feature-like elements have an owner when required by the metamodel
- relationship-like elements use endpoint fields consistently
- classifier-like elements expose feature ownership consistently
- unresolved required references are reported as diagnostics

Exit condition:

- Runtime services can ask for derived capabilities without a duplicated kind-family registry.
- Validation diagnostics distinguish field-shape errors from metamodel-derived semantic errors.

### Milestone 6: Regeneration And Cache Invalidation

Define:

- generated-resource refresh path for bundled KIR artifacts
- package rebuild expectations after schema changes
- cache invalidation rules tied to schema version
- hard-error behavior for unsupported schema versions
- test fixtures that prove old schema artifacts are rejected

Exit condition:

- Breaking KIR changes invalidate compile artifacts, derived indexes, view caches, verification evidence, and package artifacts.
- Existing source fixtures can regenerate valid KIR under the current schema.

## Relationship To Artifact Keys

Every semantic artifact key should include `kir_schema_version`. See [Semantic Artifact Keys](SEMANTIC_ARTIFACT_KEYS.md).

Changing the KIR schema version should invalidate compile artifacts, derived indexes, view caches, verification evidence, and package artifacts.

## Non-Goals

- Freezing every SysML/KerML concept before frontend coverage matures.
- Replacing KIR with parser ASTs or metamodel-native objects.
- Requiring product UI code to understand parser-specific source forms.
- Duplicating SysML/KerML metamodel kind hierarchy in a hand-maintained KIR kind-profile table.
- Maintaining migration code for old KIR schema versions during this tightening phase.

## Near-Term Recommendation

Add `version` to the KIR envelope, define the field contract, change graph construction to consult that contract, and reject unknown non-extension fields. After that, add derived metamodel validation over the graph for broad capabilities such as classifier-like, feature-like, relationship-like, and package-like behavior.
