# Python Frontend Plan

## Goal

Provide a Python-facing interface for Mercurio that can drive parsing, linting, semantic compilation, graph inspection, editor previews, and project workflows without reimplementing SysML v2, KerML, KIR, or runtime semantics in Python.

The Python frontend should treat Rust as the authoritative modeling kernel. Python should provide orchestration, scripting, notebook ergonomics, local UI integration, and later packaging convenience.

## Assumptions

- The first implementation uses an installed Mercurio executable.
- The executable launches a local HTTP backend.
- The Python package can later bundle platform-specific backend binaries without changing its public API.
- The server must support multiple open projects in one backend process.
- The HTTP API should remain product-neutral and reusable by Python, desktop, web, notebooks, and other local tools.

## Target Architecture

```text
Python package
  -> launches or attaches to Mercurio backend executable
    -> localhost HTTP API
      -> per-workspace server registry
        -> WorkspaceService instances
          -> mercurio-core parser/compiler/runtime
```

The Python package is a client and process manager. It should not contain semantic behavior beyond request validation, DTO mapping, and convenience helpers.

## Backend Executable

Add a stable server entrypoint:

```powershell
mercurio server --host 127.0.0.1 --port 0
```

`--port 0` allows the operating system to select an available port. On startup, the backend should print one machine-readable JSON line:

```json
{
  "url": "http://127.0.0.1:49152",
  "pid": 12345,
  "version": "0.1.0",
  "api_version": 1
}
```

Python should parse this startup contract rather than scraping log text.

Required backend metadata endpoints:

```text
GET /api/health
GET /api/version
```

The Python client should verify the API version before opening workspaces.

## Executable Discovery

For the first release, Python should locate the backend executable in this order:

1. Explicit `executable=` argument.
2. `MERCURIO_EXE` environment variable.
3. `mercurio` found on `PATH`.
4. Future bundled executable inside the Python wheel.

This keeps option 1 simple while making option 2 additive.

## Multi-Workspace Server State

The current server state is effectively one active `WorkspaceService`. Replace that with a workspace registry:

```rust
struct ServerState {
    workspaces: RwLock<HashMap<WorkspaceId, Arc<RwLock<WorkspaceService>>>>,
}
```

The registry lock should only be held long enough to create, find, list, or remove workspace handles. Request handlers should then operate through the selected workspace handle. This avoids one global lock blocking unrelated projects.

Workspace IDs should be opaque stable strings, for example:

```text
ws_01HZX8V8Y3G2T5QW4K9P9VPGZQ
```

## Workspace Lifecycle API

Add explicit multi-workspace routes:

```text
POST   /api/workspaces
GET    /api/workspaces
GET    /api/workspaces/{workspace_id}
DELETE /api/workspaces/{workspace_id}
```

Open request:

```json
{
  "path": "C:/models/demo",
  "mode": "lazy"
}
```

Open response:

```json
{
  "workspace_id": "ws_01HZX8V8Y3G2T5QW4K9P9VPGZQ",
  "workspace_root": "C:/models/demo",
  "active_path": null,
  "project": {}
}
```

Supported open modes:

- `shell`: open quickly and defer compile work.
- `lazy`: open quickly and compile on demand.
- `compiled`: compile immediately and populate model state.

Default mode should be `lazy`.

## Workspace-Scoped API

Add workspace-scoped equivalents for model, graph, element, editor, and semantic operations:

```text
GET  /api/workspaces/{workspace_id}/model
GET  /api/workspaces/{workspace_id}/graph
GET  /api/workspaces/{workspace_id}/elements/{element_id}
GET  /api/workspaces/{workspace_id}/search

GET  /api/workspaces/{workspace_id}/editor/files
GET  /api/workspaces/{workspace_id}/editor/file?path=...
PUT  /api/workspaces/{workspace_id}/editor/file?path=...
POST /api/workspaces/{workspace_id}/editor/parse
POST /api/workspaces/{workspace_id}/editor/format
POST /api/workspaces/{workspace_id}/editor/semantic-compile
POST /api/workspaces/{workspace_id}/editor/lint
POST /api/workspaces/{workspace_id}/editor/refresh

GET  /api/workspaces/{workspace_id}/semantic/workspace-session
POST /api/workspaces/{workspace_id}/semantic/project/compile
POST /api/workspaces/{workspace_id}/semantic/project/lint
```

Existing single-workspace routes can remain as compatibility aliases backed by a default workspace. New Python work should use workspace-scoped routes only.

## State Semantics

The API should make preview state and persisted state explicit.

Preview operations:

- `POST editor/parse`
- `POST editor/semantic-compile`
- `POST semantic/project/compile`
- `POST semantic/project/lint`

Preview operations accept request content or staged files and do not write to disk. They may use and update server-side caches, but they should not make the supplied content authoritative.

Persistent operations:

- `PUT editor/file`
- `POST editor/refresh`
- workspace open/delete

Saving an editor file writes to disk. If a compiled workspace state exists, saving should invalidate relevant caches and rebuild or mark the compiled state stale according to the selected implementation.

Project compile requests should support unsaved content through staged files:

```json
{
  "project_path": ".",
  "staged_files": [
    {
      "path": "src/main.sysml",
      "content": "package Demo { part def Vehicle; }"
    }
  ]
}
```

The staged file overlay should affect only that request.

## Concurrency Rules

Expected behavior:

- Different workspaces can compile concurrently.
- Reads within one workspace can run concurrently.
- Mutations within one workspace serialize.
- Long compiles in one workspace do not block unrelated workspaces.
- Deleting a workspace prevents new requests from acquiring it.

If long-running compiles need cancellation or progress, add an asynchronous job layer later. The first version can use synchronous request/response compile calls.

## Python Package Shape

Create a Python package with a small public API:

```text
python/mercurio/
  __init__.py
  backend.py
  client.py
  workspace.py
  process.py
  models.py
  errors.py
```

Example use:

```python
from mercurio import Mercurio

with Mercurio.launch() as backend:
    ws1 = backend.open_workspace("C:/models/demo-a")
    ws2 = backend.open_workspace("C:/models/demo-b")

    result = ws1.compile_project()
    graph = ws2.graph()
```

Attach mode:

```python
from mercurio import Mercurio

backend = Mercurio.connect("http://127.0.0.1:49152")
workspace = backend.open_workspace("C:/models/demo")
```

Workspace convenience methods:

```python
workspace.parse_preview("src/main.sysml", text)
workspace.compile_file_preview("src/main.sysml", text)
workspace.compile_project_preview(".", staged_files={"src/main.sysml": text})
workspace.save_file("src/main.sysml", text)
workspace.refresh("src/main.sysml")
workspace.graph()
workspace.element("pkg.Demo")
```

Use method names that distinguish previews from persistent saves.

## Option 2 Compatibility

Bundling the backend executable in wheels should not change Python application code. The only difference should be executable discovery.

Future wheel layout can include:

```text
mercurio/
  bin/
    windows-x86_64/mercurio.exe
    linux-x86_64/mercurio
    macos-arm64/mercurio
```

The Python launcher should prefer an explicit executable or environment override before using a bundled binary. This makes local development and debugging straightforward.

## Testing Plan

Rust tests:

- Open two workspaces in one server state and verify distinct workspace IDs.
- Compile both workspaces and verify results do not cross-contaminate.
- Compile with staged files and verify disk content is unchanged.
- Save a file in one workspace and verify only that workspace state changes.
- Delete a workspace and verify future workspace-scoped requests fail clearly.
- Verify legacy single-workspace routes still work through the compatibility path.

Python tests:

- Connect to a test server.
- Launch a real backend when an executable is available.
- Verify version handshake behavior.
- Open multiple workspaces.
- Compile staged content.
- Save, refresh, and re-query graph/model state.
- Verify process cleanup on context manager exit.

## Implementation Order

1. Add `mercurio server` entrypoint with startup JSON, health, and version endpoints.
2. Refactor server state into a workspace registry.
3. Add workspace lifecycle routes.
4. Add workspace-scoped model/editor/semantic routes.
5. Keep existing routes as default-workspace compatibility aliases.
6. Add Rust integration tests for isolation, staged files, saving, and deletion.
7. Build the Python client in attach mode.
8. Add Python process launching and executable discovery.
9. Add Python tests and examples.
10. Later add bundled executable wheels.

## Open Questions

- Should workspace IDs be random, path-derived, or caller-supplied aliases?
- Should saved-file rebuilds be eager, lazy, or configurable per workspace?
- Should long project compiles become asynchronous jobs before the Python UI grows progress reporting?
- Should the server expose a shutdown endpoint for Python-managed backend processes?
- Which existing product-shaped server routes should be workspace-scoped, feature-gated, or moved before publishing this API as a Python contract?
