# Development Roadmap

Status: roadmap and coordination reference.

## Purpose

This page ties the development-doc corpus into a practical delivery order. It does not replace the detailed plans; it identifies which work should happen before other work can safely depend on it.

## Current Source Of Truth

The canonical architecture is:

- [Architecture Model](ARCHITECTURE_MODEL.md)
- [KIR Spec](KIR_SPEC.md)
- [KIR Schema Roadmap](KIR_SCHEMA_ROADMAP.md)
- [Frontend To KIR Theory Of Operation](FRONTEND_TO_KIR_THEORY_OF_OPERATION.md)
- [Project Descriptor and Library Provider Plan](PROJECT_DESCRIPTOR_AND_MOUNT_PLAN.md)
- [Semantic Artifact Keys](SEMANTIC_ARTIFACT_KEYS.md)
- [Proposal And Draft Overlay Lifecycle](PROPOSAL_DRAFT_LIFECYCLE.md)

Plans that conflict with these documents should be updated or treated as historical.

## Implementation State

Current partially implemented areas:

- SysML/KerML frontend path: parsing, resolution, KIR emission, and linting exist, but robust KerML coverage and conformance tracking are still growing.
- `expression_ir`: parsing, emission, runtime evaluation, and constraint rendering exist for a useful subset; remaining work is coverage and schema.
- Datalog-derived indexes: core rule packs, graph fact extraction, and materialized indexes exist; remaining work is scope, cache policy, explanation surface, and benchmark-backed integration.
- Views and diagrams: core DTOs/projections exist for selected slices; remaining work is shared context handling, product API/UI alignment, and view IR consolidation.
- Project descriptors and library providers: several provider kinds exist; API-backed snapshots and richer package-set workflows remain open.

Historical areas:

- [L2 Parser Plan](L2_PARSER_PLAN.md) is historical baseline.

## Recommended Delivery Order

1. Stabilize KIR and source provenance.

   Finish enough of the KIR contract to support downstream artifacts without schema drift: `expression_ir`, source spans, id templates, element metadata, and version fields.

2. Finish the shared frontend boundary.

   Make `.sysml` and `.kerml` flow through the same source-set, resolver, mapping, and library-provider path. This unlocks robust KerML, package-set libraries, server indexing, and reliable semantic comparison.

3. Lock semantic artifact identity.

   Implement artifact key/provenance DTOs using [Semantic Artifact Keys](SEMANTIC_ARTIFACT_KEYS.md). Server caches, Datalog derived indexes, views, verification, and package release should all use compatible identity fields.

4. Convert Datalog from spike to runtime service.

   Treat Datalog as an optional derived-index service over KIR and graph state. The next useful milestone is benchmarked specialization/ownership/traceability indexes with explanations and cache invalidation rules.

5. Unify views and diagrams around semantic context.

   Views and diagrams should evaluate against explicit semantic contexts: accepted artifact, local workspace snapshot, draft overlay, proposal, or compare pair. Diagrams should remain a graphical view implementation, not a parallel semantic system.

6. Make proposal and draft overlays executable.

   Implement the lifecycle in [Proposal And Draft Overlay Lifecycle](PROPOSAL_DRAFT_LIFECYCLE.md): draft semantic operations, virtual compile, validation, semantic diff, source patch preview, and optional PR binding.

7. Build server semantics on stable artifacts.

   Server work should start with repository authority records, artifact key/provenance models, commit validation, semantic artifact cache, and semantic diff. Package registry and browser proposals should follow once commit-indexed artifacts are reliable.

8. Add verification and simulation after semantic contexts are stable.

   Verification and simulation should consume compiled semantic artifacts and record reproducible evidence. Avoid building them against ad hoc workspace state or parser-specific source paths.

9. Harden distribution and Python integration.

   Python and installer work should follow the stable local backend contract: explicit workspace lifecycle, startup JSON, version handshake, workspace-scoped APIs, and deterministic executable discovery.

## Near-Term Documentation Work

1. Convert [Datalog Reasoning Engine Plan](DATALOG_REASONING_ENGINE_PLAN.md) into architecture plus remaining work.
2. Convert [SysML Expression Implementation Plan](SYSML_EXPRESSION_IMPLEMENTATION_PLAN.md) into expression coverage and schema roadmap.
3. Reconcile [Diagram Implementation Plan](DIAGRAM_IMPLEMENTATION_PLAN.md) with [Views Architecture](VIEWS_ARCHITECTURE.md).
4. Keep [KIR Schema Roadmap](KIR_SCHEMA_ROADMAP.md) aligned with [KIR Spec](KIR_SPEC.md) as schema/version decisions land.
5. Split root README user workflows into focused docs under `docs/user/`.

## Decision Rules

- If a feature needs source authority, use [Architecture Model](ARCHITECTURE_MODEL.md).
- If a feature needs compiled semantic identity, use [Semantic Artifact Keys](SEMANTIC_ARTIFACT_KEYS.md).
- If a feature changes source indirectly, use [Proposal And Draft Overlay Lifecycle](PROPOSAL_DRAFT_LIFECYCLE.md).
- If a feature reads semantic data, consume KIR/graph/derived indexes, not parser ASTs.
- If a doc describes product HTTP routes or UI files, distinguish reusable `mercurio-core` DTOs/services from product-repository implementation.
