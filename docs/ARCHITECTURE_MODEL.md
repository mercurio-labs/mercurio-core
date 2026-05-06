# Mercurio Architecture Model

## Purpose

This document summarizes the logical model that the rest of the planning docs should reinforce.

Mercurio is a semantic modeling environment with four distinct responsibilities:

```text
Source authority -> KIR compiler/runtime -> semantic services -> user surfaces
```

## Source Authority

Accepted source lives outside the semantic cache.

Supported authority shapes:

- `local_files`: local filesystem owns the editable source
- `local_git`: local Git working tree and configured remotes own source history
- `external_git_indexed`: external Git owns source history; Mercurio indexes it
- `mercurio_hosted`: Mercurio Server owns source history, deferred

The default shared authority should be external Git, not Mercurio Server.

## Backing Systems

Mercurio is provider-neutral. It treats Git providers, issue trackers, pull request systems, and review systems as backing systems that can be attached to a project. When attached, Mercurio syncs or links provider-native objects into its reasoning graph while keeping semantic context, discussions, decisions, and verification evidence in the Mercurio layer.

An MRS project exists independently of any single backing provider. A project may attach one or more backing systems:

- source repositories
- issue or task providers
- pull request and review providers
- artifact or release providers

These bindings may point to the same provider, such as Gitea, or to different systems, such as GitHub for source and Jira for tasks. Provider integrations must not define project identity; they are external bindings on a Mercurio project.

```text
Mercurio project
  -> semantic artifacts and reasoning graph
  -> discussions, decisions, proposals, problem reports
  -> verification evidence and semantic impact
  -> optional provider bindings for repositories, issues, PRs, reviews, and releases
```

## Canonical Nouns

Use these nouns at product and domain boundaries:

- `Repository`: external source authority location
- `Project`: semantic compilation unit inside a repository
- `Workspace`: local desktop working context
- `Semantic Artifact`: compiled semantic state for a commit or proposal
- `Discussion`: Mercurio-owned reasoning thread that can contain design rationale, problem reports, proposal snapshots, decisions, and AI context
- `Proposal`: Mercurio-owned overlay against an immutable base commit
- `PR Binding`: optional link from a proposal to an external pull request
- `External Binding`: provider-neutral link from a Mercurio object to a backing-system object such as an issue, pull request, commit, review, or release
- `Package`: released semantic artifact from validated source

Implementation names such as watched repository or watched project should remain internal cache/indexing language.

## KIR Boundary

KIR is the semantic compiler boundary.

```text
.sysml / .kerml / libraries
          |
          v
       KIR documents
          |
          v
   consolidated KIR
          |
          v
    graph/runtime/index
```

Source files and library providers produce KIR. The runtime consumes KIR. UI and AI features should reason over compiled semantic state, not parser-specific ASTs.

The KIR contract is documented in [KIR_SPEC.md](KIR_SPEC.md).

## Server Role

Mercurio Server is primarily a semantic reasoner, not a source repository.

It owns derived state:

- consolidated KIR artifacts
- diagnostics and validation results
- graph indexes
- semantic diffs
- reasoning history
- discussions, decisions, and verification evidence
- proposal overlays
- provider-neutral external bindings
- package artifacts and package registry metadata

For project repositories, the server mirrors or fetches external Git state only to index it. The mirror is a cache, not the accepted source.

## Desktop Role

Desktop is the heavy authoring surface.

It should own:

- local source editing
- local parse diagnostics while typing
- local Git status and commits
- package build/publish from local projects
- applying patches or checking out PR branches from proposals

Desktop may use Mercurio Server as:

- a project repository catalog
- a semantic status service for commits
- a semantic diff service
- a package registry
- a web review/proposal entry point

## Web Role

Web is the reasoning, review, and governance surface.

It should own:

- project repository dashboards
- validation history
- semantic diffs across commits, branches, tags, and PRs
- source-linked graph and diagram inspection
- design rationale and decision records
- lightweight proposals against immutable commits
- package release review

Web is not a full authoring IDE. Browser editing, when present, must happen inside a proposal overlay and must not mutate accepted project source directly.

## Proposal Model

A proposal is Mercurio-owned reasoning state over an external source base.

```text
base commit + file overlay -> virtual compile -> KIR -> diagnostics -> semantic diff
```

Proposal outcomes:

- export patch
- create external branch and PR
- bind to an existing external PR
- abandon
- supersede

Accepted source changes still flow through external Git commits and merges.

## Pull Request Model

A pull request is the source-control acceptance vehicle for a Mercurio proposal.

```text
Proposal overlay -> semantic impact -> external branch/PR -> provider merge -> indexed commit
```

Gitea or another Git provider owns branches, reviews, protection rules, and merges. Mercurio may create branches, open PRs, post status checks, and attach semantic-impact comments, but it is not the merge authority.

## Issue and Task Model

Problem reports and task discussions may live in Mercurio, in a backing issue provider, or in both. Mercurio should keep a provider-neutral work/discussion object as the reasoning anchor and attach external issue bindings when a provider-native issue exists.

Provider issue systems own provider-native collaboration details such as comments, assignments, labels, milestones, notifications, and provider-specific workflow rules. Mercurio owns semantic links, artifact evidence, AI summaries, proposal relationships, verification status, and cross-provider traceability.

## Package Model

Packages are Mercurio-owned released artifacts.

```text
validated commit/tag or local workspace -> package artifact -> package registry
```

Publishing a package does not transfer source authority to Mercurio Server.

The first concrete package type is the Mercurio Model Package (`mercurio.model.package`), stored as a `.kpar` artifact with POM-like metadata:

- `name`, `version`, `groupId`, display name, description, license, tags, and arbitrary string properties
- dependency declarations on other model packages or platform packages
- provenance back to the watched project, repository URL, branch/ref, Git commit, and semantic artifact id
- the compiled semantic artifact payload: manifest, semantic JSON, model view, graph view, and indexed elements

The server package registry is the local abstraction. A backing provider such as Gitea packages or GitHub Packages can mirror or host the bytes later without changing the model-package contract.

## Documentation Map

- [CENTAUR_STRATEGIC_PLAN.md](CENTAUR_STRATEGIC_PLAN.md): product identity and reasoning loop
- [KIR_SPEC.md](KIR_SPEC.md): semantic IR contract
- [SERVER_WORKSPACE_PLAN.md](SERVER_WORKSPACE_PLAN.md): server as semantic reasoner over source authorities
- [SERVER_IMPLEMENTATION_PLAN.md](SERVER_IMPLEMENTATION_PLAN.md): project repository and semantic artifact implementation path
- [DESKTOP_WORKSPACE_SYNC_PLAN.md](DESKTOP_WORKSPACE_SYNC_PLAN.md): desktop local authoring and server semantic-service integration
- [MODEL_EDITOR_MODE.md](MODEL_EDITOR_MODE.md): desktop editor mode
- [PROJECT_DESCRIPTOR_AND_MOUNT_PLAN.md](PROJECT_DESCRIPTOR_AND_MOUNT_PLAN.md): source/provider/cache separation
- [VIEWS_ARCHITECTURE.md](VIEWS_ARCHITECTURE.md): semantic projections for tables, diagrams, matrices, dashboards, and future runtime-defined views
- [DATALOG_REASONING_ENGINE_PLAN.md](DATALOG_REASONING_ENGINE_PLAN.md): derived fact/rule reasoning layer over KIR graph state

## Guiding Rule

Keep these responsibilities separate:

```text
Git/files and backing providers own their native objects.
KIR owns semantics.
Server owns derived reasoning and provider-neutral bindings.
Desktop changes accepted models.
Web judges proposals, PRs, and semantic impact.
```
