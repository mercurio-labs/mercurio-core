# Proposal API Plan

## Goal

Add a V2 web/server API and UI where git remains the model version history, while Jira-like work management and semantic intelligence live outside git.

## Architecture Choice

Use this split:

```text
Git repo:
  .sysml source files
  branches
  commits

Mercurio external store:
  proposals
  board state
  commit links
  semantic indexes
  comments/activity
  review state
```

The initial external store should be SQLite. For local desktop use, store it under app/workspace data. For team/shared use later, the same schema can move behind a central server database.

## Phase 1: Backend Foundation

Add a V2 module to `mercurio-core`:

```text
mercurio-core/src/v2/
  mod.rs
  routes.rs
  store.rs
  git.rs
  proposals.rs
  semantic_index.rs
```

Mount the API under:

```text
/api/v2/...
```

Initial endpoints:

```text
GET   /api/v2/status
GET   /api/v2/git/status
GET   /api/v2/proposals
POST  /api/v2/proposals
GET   /api/v2/proposals/{key}
PATCH /api/v2/proposals/{key}
```

## Phase 2: SQLite Store

Create tables:

```text
proposals
proposal_events
git_commits
commit_proposal_links
semantic_snapshots
semantic_changes
workspace_settings
```

Keep proposals independent from git commits. A commit can exist without a proposal, and a proposal can exist without commits.

## Phase 3: Git Read/Index Layer

Add read-only git support first:

```text
current branch
dirty files
recent commits
changed files per commit
file contents at commit
```

Index commits into SQLite:

```text
git sha
branch
author
timestamp
message
changed files
linked proposal if inferred
semantic index status
```

Infer proposal links from:

```text
branch name: proposal/MER-123-title
commit message: MER-123
commit trailer: Mercurio-Proposal: MER-123
manual user link
```

## Phase 4: Semantic Indexing

For each commit touching `.sysml`:

1. Load previous version.
2. Load commit version.
3. Compile both using the existing semantic compiler.
4. Store diagnostics.
5. Store semantic changes:
   - added elements
   - removed elements
   - changed properties
   - changed relationships
   - affected metatypes
   - affected source files

Expose:

```text
GET  /api/v2/commits/{sha}/semantic-impact
POST /api/v2/semantic/reindex
GET  /api/v2/proposals/{key}/semantic-impact
```

## Phase 5: PR Binding

Do not let the proposal API write accepted source directly.

Proposal outcomes:

```text
export patch
create provider branch and pull request
bind to existing pull request
abandon
supersede
```

Server behavior:

1. Compile the proposal overlay virtually.
2. Compute semantic impact.
3. Ask the source-control provider to create a branch/PR when the user submits.
4. Link the provider PR to the proposal.
5. Post semantic status and comments back to the provider.

Direct desktop and CLI git commits remain valid. They are indexed later and can be linked manually.

## Phase 6: Web UI

Add a Proposals view to the web app.

First UI pass:

```text
proposal list
proposal detail panel
commit/history panel
semantic impact panel
git status strip
```

Later UI pass:

```text
kanban board
backlog filtering
commit composer
manual commit-to-work linking
semantic review checklist
```

Statuses:

```text
Backlog
Ready
In Progress
Review
Done
```

proposal fields:

```text
key
title
description
type
status
priority
labels
created_at
updated_at
linked commits
linked semantic elements
```

## Phase 7: Tests

Backend tests:

```text
creates proposal
updates proposal status
indexes git commit
infers proposal from branch/message
links commit manually
rejects unsafe paths
compiles staged sysml before API commit
stores semantic changes
handles direct git commit without work metadata
```

Frontend tests:

```text
renders proposal list
opens proposal detail
shows linked commits
shows semantic impact
handles unlinked commits
```

## Recommended First Milestone

Deliver a thin but complete vertical slice:

1. SQLite store.
2. Create/list/update proposals.
3. Read git status and recent commits.
4. Manually link commit to proposal.
5. Show proposals and linked commits in UI.

Do semantic indexing in milestone two. That keeps the first cut small while locking down the key architectural decision: git remains independent, and Mercurio work tracking lives outside git.

