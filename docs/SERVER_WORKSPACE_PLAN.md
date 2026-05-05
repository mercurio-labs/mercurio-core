# Server Workspace Plan

## Goal

Define the Mercurio server as a semantic reasoning layer over source authorities.

The server's first-class job is not to become the authoritative source repository. Its first-class job is to watch, compile, validate, index, compare, explain, and package model repositories whose source of truth lives elsewhere.

The concrete delivery sequence is covered in [SERVER_IMPLEMENTATION_PLAN.md](SERVER_IMPLEMENTATION_PLAN.md).

## Core Principle

The user-facing model should be:

```text
Source authority -> Mercurio semantic index -> review, reasoning, proposals, packages
```

Mercurio Server owns derived semantic state:

- KIR artifacts
- validation results
- semantic graph indexes
- semantic diffs
- design reasoning records
- proposal overlays
- package registry entries

Mercurio Server should not own normal source history by default. External Git providers and local Git workspaces already provide repository authority, branching, merge, access control, review, backup, and audit primitives.

## Source Authorities

Mercurio should reason over three source authority shapes.

### Local Workspace

Authority: local filesystem or local Git working tree.

Primary client: desktop.

Mercurio role:

- edit local files
- compile and validate locally
- generate KIR and graph state
- publish package artifacts to a Mercurio package registry

The server does not own local files or local Git history.

### Project Repository

Authority: external Git repository.

Primary clients: web for review/reasoning, desktop for local authoring through a normal clone.

Mercurio role:

- fetch indexed refs through configured provider credentials
- compile immutable commits into semantic artifacts
- cache consolidated KIR, diagnostics, graphs, and semantic diffs
- show validation and semantic history
- support proposal overlays against known commits
- create external PRs or export patches when configured
- release packages from validated commits or tags

The indexed mirror is derived from external Git. Mercurio should not silently mutate it.

### Mercurio-Hosted Workspace

Authority: Mercurio Server.

This mode should be optional and deferred until there is a clear need for browser-first governed modeling that external Git cannot satisfy. It can share the same semantic services, but it should not be the strategic center of the server architecture.

Possible future use cases:

- starter/demo projects
- organizations that do not want to expose Git concepts to model authors
- server-native draft/review workflows
- controlled baselines not represented as external Git commits

## Product Role

The server is a semantic reasoner and collaboration surface.

It should provide:

- project repository dashboards
- repository catalog for desktop clone discovery
- commit, branch, tag, and PR validation
- semantic diffs across commits
- KIR and graph inspection
- model-aware review summaries
- design rationale linked to commits and proposals
- package release and package registry workflows
- optional proposal editing in the browser
- provider integrations for GitHub, GitLab, Azure DevOps, or self-hosted Git

The desktop app remains the primary heavy authoring surface for local files and local Git clones.

## Architecture

The main shape is:

```text
External Git Provider
  |
  v
Mercurio Tracker / Fetcher
  |
  v
Semantic Artifact Cache
  |
  +-- consolidated KIR
  +-- diagnostics
  +-- graph index
  +-- semantic diffs
  +-- summaries
  |
  v
Mercurio Web UI / API
```

Core services:

- `RepositoryWatcher`: tracks configured external repositories and refs
- `ProviderIntegration`: authenticates and talks to Git providers
- `SemanticCompiler`: compiles source trees into KIR and diagnostics
- `SemanticArtifactStore`: caches KIR, graph indexes, and summaries
- `SemanticDiffService`: compares compiled artifacts
- `ProposalService`: manages browser-created change overlays
- `PackageRegistry`: stores released reusable packages
- `ReasoningHistoryService`: stores decisions, critiques, assumptions, and review notes

## Semantic Artifact Cache

Git commits are immutable, so they are good cache anchors. A cache key should include more than the commit id:

```text
repository_id
commit_sha
compiler_version_or_digest
kir_schema_version
stdlib_digest
dependency_package_digests
mapping_rules_digest
```

The cached value can include:

- consolidated KIR
- diagnostics and lint results
- graph indexes
- source span indexes
- semantic summaries
- precomputed semantic diffs
- package readiness metadata

KIR should not be committed back into normal source repositories by default. It is a derived artifact. Released packages may contain frozen KIR or KPAR artifacts with provenance.

## Project Repository Workflow

Recommended flow:

1. Admin connects an external Git repository.
2. Mercurio stores provider identity, indexed refs, and read permissions.
3. Background tracker fetches new commits.
4. Server compiles each relevant commit into semantic artifacts.
5. Web UI shows validation status, graph views, semantic diffs, and reasoning history.
6. A validated commit or tag can be released as a package.

The same project repository records can serve as a desktop clone catalog. Desktop clients may list project repositories, let the user choose one, then clone the external Git URL locally using local Git credentials. The server remains a catalog and semantic index, not the clone authority.

Mercurio should expose repository state in Git-native terms when Git is the authority:

- repository URL
- clone URL or provider-specific clone URLs
- branch
- tag
- commit SHA
- PR or merge request id
- provider check/status

## Proposal Workflow

Web edits for project repositories should be proposals, not direct writes to the indexed mirror.

```text
base commit + file overlay -> virtual source tree -> KIR -> diagnostics -> semantic diff
```

A proposal stores:

- project repository id
- base commit SHA
- changed file overlays
- author and rationale
- validation result
- semantic diff
- review comments
- submission status

Proposal states:

- `Draft`
- `Validated`
- `Needs Fixes`
- `Ready for PR`
- `Submitted`
- `Superseded`
- `Abandoned`

Proposal actions:

- save draft
- validate
- show semantic diff
- ask critic
- export patch
- create external branch and PR
- abandon

Creating an external PR requires a provider integration. It may use a server-owned app credential, a user-delegated credential, or both depending on policy.

## Provider Credentials

Prefer provider-native app credentials for server integrations.

For GitHub, this means a GitHub App rather than a shared user PAT. Equivalent app or service-principal mechanisms should be used for GitLab and Azure DevOps.

Use server-owned app credentials for:

- tracking repositories
- fetching refs and files
- posting validation checks
- posting bot review comments
- creating bot-authored proposal branches when allowed

Use user-delegated credentials when:

- the action should be authored by the user
- the repo is private and only the user has access
- org policy requires user identity for branch creation or PR submission

Do not ask users for raw Git passwords or broad long-lived tokens when provider OAuth or app installation is available.

## Projects And Packages

In this direction, a "project" in Mercurio is usually an indexed external repository plus semantic configuration.

Packages remain a Mercurio-owned output:

```text
External Git commit/tag -> validated semantic artifact -> released package
```

Packages should support:

- package name and version
- immutable contents for a released version
- dependency metadata
- checksums and provenance
- compatibility metadata
- exported KPAR or KIR-backed artifact
- source repository and commit/tag links
- read-only consumption by other projects

For local Git projects, publishing should still mean publishing a released package artifact to the server package registry. It should not mean making the server authoritative for the local repository.

## Browser UI

The web client is useful even when Git remains authoritative.

Recommended capabilities:

- project repository dashboard
- repository catalog for desktop clone discovery
- branch, tag, and PR validation status
- semantic file browser
- source-linked graph and explorer views
- commit-to-commit semantic diff
- model-aware PR review
- proposal editor with virtual validation
- design rationale and decision records linked to commits
- package release cockpit

The web UI should make authority clear:

```text
Source: GitHub / acme/brake-system @ abc123
Mercurio: semantic index and proposal workspace
```

## SysML v2 API And Extensions

SysML v2 compatibility should be layered over semantic artifacts where the mapping is natural:

- projects or repositories
- commits or revisions
- elements
- queries

Mercurio-specific endpoints should remain explicit for:

- project repository configuration
- semantic validation
- semantic diffs
- KIR preview and graph inspection
- proposal overlays
- package release
- design reasoning history

Do not force proposal editing, package release, or provider integration into SysML v2 concepts if they do not fit.

## Deferred Server-Authoritative Mode

Server-authoritative projects may still be useful later, but they should be treated as a separate source provider:

```text
mercurio://server/projects/{project_id}
```

They should reuse the same compile/cache/diff/package services. The difference is only source authority.

Do not let this mode drive the near-term server design unless a concrete customer workflow requires Mercurio to own the source repository.

## Recommended Delivery Plan

### Phase 1: Semantic Server Foundation

- expose health and structured errors
- define repository/project authority records
- add semantic artifact cache schema
- compile a source tree at a known revision into consolidated KIR

Exit condition:

- server can compile and cache semantic artifacts for a known source snapshot

### Phase 2: Project Repository Foundation

- connect one external Git provider or a generic external repo URL
- fetch indexed refs into a server mirror/cache
- index new commits
- show validation status per commit

Exit condition:

- server tracks an external repo and displays semantic status for commits

### Phase 3: Semantic Diff And Review

- compare two cached semantic artifacts
- expose branch/tag/commit compare endpoints
- add web UI for semantic diffs
- add review summary generation from semantic changes

Exit condition:

- user can compare two commits semantically in the browser

### Phase 4: Proposals

- store proposal overlays against a base commit
- validate `base commit + overlay`
- show proposal semantic diff
- export patch

Exit condition:

- user can make a browser proposal without mutating the external repo

### Phase 5: Provider PR Integration

- add provider app credential support
- create branch/commit/PR from a proposal
- post validation status or review comment where configured

Exit condition:

- a validated proposal can become an external PR

### Phase 6: Package Registry

- publish package from validated commit/tag
- record source repository, commit SHA, dependency digests, compiler digest, and KIR schema version
- expose package listing and download/mount metadata

Exit condition:

- an external Git commit can produce a reusable Mercurio package

## Risks

### Authority Confusion

Users must know whether Git or Mercurio owns the source. The UI should always show the source authority.

### Credential Scope

Provider credentials must be narrow, revocable, and auditable. Avoid broad PATs and shared user secrets.

### Cache Staleness

Semantic artifacts must be keyed by compiler, stdlib, dependency, mapping, and schema fingerprints, not only by commit SHA.

### Proposal Drift

Proposals are based on a commit. If the target branch moves, the proposal must be revalidated or rebased before PR creation.

### Web Editor Scope Creep

The browser proposal editor should stay lightweight. Heavy authoring remains a desktop/local Git workflow until a strong browser-first authoring requirement appears.

## Recommendation

Build Mercurio Server first as a semantic reasoner over externally authoritative source repositories.

Keep external Git as the default source authority, use the server for semantic indexing, validation, diffs, proposals, reasoning history, and package release, and defer Mercurio-hosted source authority until it is clearly needed.
