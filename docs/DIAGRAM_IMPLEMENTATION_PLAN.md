# Diagram Implementation Plan

## Goal

Add a well-defined diagram system that supports:

- enumerated diagram types
- ephemeral diagrams generated during exploration or AI chat
- saved `.diagram` files stored in the project folder
- deterministic backend rendering from the compiled semantic graph
- chat-visible diagram artifacts that can open as popouts or docked workbench panels

The first useful workflow should be:

1. A user asks AI for a diagram, such as "show the Package to Element specialization tree."
2. AI produces a structured diagram request.
3. Mercurio renders the diagram from the current semantic graph.
4. The diagram appears in chat as an ephemeral artifact with a popout action.
5. The user can save it as a project `.diagram` file.
6. Saved diagrams can be reopened from the project navigator.

Diagrams should render against an explicit semantic artifact:

- local desktop compiled workspace
- project repository commit artifact
- proposal artifact
- future Mercurio-hosted workspace artifact

The diagram system should not care which source authority produced the artifact.

## Design Principles

- Diagram semantics live in structured data, not in prose or prompt-only behavior.
- The backend resolves diagrams against a selected semantic graph/artifact so the result is deterministic and testable.
- Ephemeral and saved diagrams share the same `DiagramSpec`.
- Saved `.diagram` files describe intent, not cached layout output.
- The UI renderer should consume normalized graph data and remain mostly diagram-kind agnostic.
- Diagram generation should reuse the existing graph, metatype explorer, L2 explorer, `@xyflow/react`, and `dagre` patterns.

## Initial Diagram Types

Implement diagram types as an explicit enum. Start with the first four as MVP.

| Kind | MVP | Purpose |
| --- | --- | --- |
| `metamodel_specialization` | Yes | L0/L1 specialization trees, such as Package to Element. |
| `element_specialization` | Yes | L2 authored type specialization trees. |
| `package_tree` | Yes | Package, namespace, and owned element containment. |
| `composition_graph` | Yes | Part, feature, and owned-feature decomposition. |
| `reference_graph` | Later | Typed references and cross-links between model elements. |
| `dependency_graph` | Later | Imports, package dependencies, mounted libraries, and external references. |
| `metatype_instance_map` | Later | L2 elements linked to their L1 metatypes. |
| `impact_view` | Later | Inbound and outbound related elements from one seed. |
| `property_inheritance` | Later | Effective property origins along specialization chains. |
| `validation_view` | Later | Diagnostics and invalid or unresolved elements in graph form. |

## File Format

Saved diagrams use a project-local `.diagram` JSON file.

Example:

```json
{
  "version": 1,
  "kind": "metamodel_specialization",
  "title": "Package to Element specialization tree",
  "description": "L0/L1 specialization view rooted at Package.",
  "root": "SysML::Package",
  "query": {
    "relations": ["specializes"],
    "direction": "children",
    "depth": 4,
    "includeLibraries": true,
    "includeUserModel": false
  },
  "layout": {
    "engine": "dagre",
    "direction": "TB"
  },
  "style": {
    "showAttributes": true,
    "showEdgeLabels": true,
    "groupByLayer": false
  }
}
```

### Schema Notes

- `version`: required, currently `1`.
- `kind`: required enum value.
- `title`: user-visible title.
- `description`: optional.
- `root`: optional seed element id, qualified name, or alias resolved by the backend.
- `query`: diagram-kind-specific options with common defaults.
- `layout`: layout preferences only; not persisted absolute node positions in v1.
- `style`: rendering preferences only.

Do not persist compiled node positions in the MVP. That keeps files stable and avoids churn when the model changes.

## Shared Data Model

Add compatible Rust DTOs and TypeScript types.

```ts
export type DiagramKind =
  | 'metamodel_specialization'
  | 'element_specialization'
  | 'package_tree'
  | 'composition_graph'
  | 'reference_graph'
  | 'dependency_graph'
  | 'metatype_instance_map'
  | 'impact_view'
  | 'property_inheritance'
  | 'validation_view';

export type DiagramSource = 'ephemeral' | 'saved';

export type DiagramSpec = {
  version: 1;
  kind: DiagramKind;
  title: string;
  description?: string | null;
  root?: string | null;
  query?: DiagramQueryOptions;
  layout?: DiagramLayoutOptions;
  style?: DiagramStyleOptions;
};

export type DiagramView = {
  spec: DiagramSpec;
  nodes: DiagramNode[];
  edges: DiagramEdge[];
  warnings: string[];
};
```

Use structurally similar Rust structs in `mercurio-core`, with serde defaults for optional fields.

## Backend API

Add these API endpoints to `mercurio-core/src/api.rs`:

- `GET /api/diagrams/kinds`
- `POST /api/diagrams/render`
- `GET /api/diagrams/files`
- `GET /api/diagrams/file?path=...`
- `PUT /api/diagrams/file?path=...`

### Render Request

```json
{
  "spec": {
    "version": 1,
    "kind": "metamodel_specialization",
    "title": "Package specialization tree",
    "root": "SysML::Package"
  },
  "stagedFiles": []
}
```

`stagedFiles` should follow the existing editor compile pattern so unsaved source edits can participate later. The MVP can render from the current authoritative compiled workspace and reject staged rendering with a clear message if needed.

### Render Response

```json
{
  "spec": {},
  "nodes": [
    {
      "id": "SysML::Package",
      "label": "Package",
      "kind": "SysML::Package",
      "layer": 1,
      "badges": ["L1"],
      "properties": {}
    }
  ],
  "edges": [
    {
      "id": "edge-1",
      "source": "SysML::Package",
      "target": "KerML::Namespace",
      "relation": "specializes",
      "label": "specializes"
    }
  ],
  "warnings": []
}
```

## Backend Implementation

Add a new diagram module rather than growing `api.rs` directly:

- `mercurio-core/src/diagrams.rs`
- `mercurio-core/src/diagrams/metamodel_specialization.rs` if the module grows
- `mercurio-core/src/diagrams/schema.rs` if DTOs become large

Recommended core functions:

```rust
pub fn list_diagram_kinds() -> Vec<DiagramKindDto>;

pub fn render_diagram(
    graph: &Graph,
    metamodel_registry: &MetamodelAttributeRegistry,
    spec: DiagramSpecDto,
) -> Result<DiagramViewDto, DiagramError>;
```

`AppState` should expose a small wrapper:

```rust
pub fn render_diagram(&self, spec: DiagramSpecDto) -> Result<DiagramViewDto, ApiError>;
```

### MVP Renderers

`metamodel_specialization`:

- Resolve `root`.
- If no root is provided, require the caller to choose one.
- Walk `specializes` edges.
- Include children, parents, or both based on `query.direction`.
- Default `depth` to `3`.
- Include only L0/L1 elements by default.
- Attach attributes using the existing `MetamodelAttributeRegistry`.

`element_specialization`:

- Same traversal mechanics.
- Include L2 elements by default.
- Exclude library elements unless `includeLibraries` is true.

`package_tree`:

- Use owner/containment-like properties and existing library tree concepts where available.
- Show packages and owned elements.
- Start with authored project elements, then add library support later.

`composition_graph`:

- Traverse owned features, parts, and feature references.
- Start from a selected element/root.
- Show decomposition, not every cross-reference.

## Frontend API Layer

Extend `mercurio-desktop-ui/src/lib/types.ts` with diagram types.

Extend `mercurio-desktop-ui/src/lib/api.ts` transport with:

- `fetchDiagramKinds()`
- `renderDiagram(spec)`
- `fetchDiagramFiles()`
- `fetchDiagramFile(path)`
- `saveDiagramFile(path, spec)`

Implement both HTTP and Tauri transports. Tauri should invoke Rust commands once desktop commands are added; HTTP should call the REST endpoints.

## Frontend UI

Add these components:

- `mercurio-desktop-ui/src/components/DiagramPanel.tsx`
- `mercurio-desktop-ui/src/components/DiagramRenderer.tsx`
- `mercurio-desktop-ui/src/components/DiagramToolbar.tsx`
- `mercurio-desktop-ui/src/components/DiagramPopout.tsx`

Reuse `@xyflow/react` for rendering and `dagre` for automatic layout.

### Diagram Panel

The docked panel should support:

- title
- refresh
- fit view
- save as `.diagram` when source is ephemeral
- reveal file when source is saved
- simple relation/style toggles where supported

### Diagram Popout

The chat popout should support:

- ephemeral diagram rendering
- error state if the render request fails
- save action
- open as workbench panel
- close

## Project Navigator Integration

Treat `.diagram` files as project artifacts:

- include them in the project file list
- show a diagram icon or label
- open in `DiagramPanel`
- allow text editing later, but MVP can open them as rendered diagrams only

File creation can be simple:

- default folder: current `projectFolder`
- generated filename from title, such as `package-specialization.diagram`
- if a file exists, append `-2`, `-3`, etc.

## AI Chat Integration

Extend AI chat from text-only responses to artifact-aware responses.

Recommended response shape:

```ts
export type AiArtifact =
  | {
      type: 'diagram';
      source: 'ephemeral';
      spec: DiagramSpec;
    };
```

The short-term fallback is to detect fenced JSON blocks with a `diagram` marker, but the durable implementation should use structured provider output.

AI system behavior:

- When the user asks to "show", "draw", "diagram", "visualize", or "map" model structure, produce a `DiagramSpec`.
- Prefer known diagram kinds.
- Ask the backend to resolve ambiguous roots rather than inventing ids.
- If a selected element exists, use it as the default root for element-oriented diagrams.

Example:

```json
{
  "type": "diagram",
  "source": "ephemeral",
  "spec": {
    "version": 1,
    "kind": "metamodel_specialization",
    "title": "Package to Element specialization tree",
    "root": "Package",
    "query": {
      "relations": ["specializes"],
      "direction": "both",
      "depth": 4,
      "includeLibraries": true,
      "includeUserModel": false
    }
  }
}
```

## Implementation Phases

### Phase 1: Schema And Backend Render MVP

- Add Rust diagram DTOs.
- Add TypeScript diagram types.
- Add `POST /api/diagrams/render`.
- Implement `metamodel_specialization`.
- Add backend tests for valid render, missing root, depth limiting, and layer filtering.

Exit criteria:

- A JSON `DiagramSpec` can be posted to the backend.
- The backend returns nodes and edges for a metamodel specialization tree.

### Phase 2: Workbench Renderer

- Add `DiagramRenderer` using XYFlow and dagre.
- Add `DiagramPanel`.
- Add a temporary developer path that opens a hardcoded `metamodel_specialization` diagram.
- Add basic Vitest coverage for empty, loading, error, and rendered states.

Exit criteria:

- The app can render a backend diagram response in a workbench panel.

### Phase 3: Ephemeral AI Diagrams

- Add AI artifact type support.
- Render diagram artifacts inline in the AI chat message list.
- Add popout support.
- Wire popout render to `renderDiagram`.

Exit criteria:

- A chat-generated `DiagramSpec` can appear as an ephemeral diagram and open as a popout.

### Phase 4: Saved `.diagram` Files

- Add diagram file APIs.
- Include `.diagram` files in project file discovery.
- Open saved diagrams in `DiagramPanel`.
- Add "Save Diagram" for ephemeral diagrams.

Exit criteria:

- An ephemeral diagram can be saved and reopened from the project navigator.

### Phase 5: Expand Diagram Kinds

Implement in this order:

1. `element_specialization`
2. `package_tree`
3. `composition_graph`
4. `reference_graph`
5. `dependency_graph`

Each new kind should add:

- backend renderer tests
- UI snapshot/behavior tests where meaningful
- one example `.diagram` fixture

## Test Plan

Backend:

- serde round trip for `.diagram` files
- reject unknown diagram kind
- reject invalid version
- validate path safety for diagram file APIs
- render metamodel specialization with expected nodes and `specializes` edges
- depth limiting
- root not found
- empty result warning

Frontend:

- `renderDiagram` transport builds the expected request
- `DiagramRenderer` renders nodes and edges
- loading, empty, and error states
- save button appears only for ephemeral diagrams
- saved `.diagram` file opens as a diagram panel
- AI diagram artifact renders in chat and opens popout

Manual verification:

- ask AI for a Package to Element specialization diagram
- pop it out
- save it to the project folder
- close and reopen it from the navigator
- change the model, recompile, refresh the diagram

## Open Questions

- Should `.diagram` files be included in the same editor file list as `.sysml` and `.kerml`, or should diagram files have a separate project artifact list?
- Should saved diagrams eventually support pinned manual node positions?
- Should AI create only `DiagramSpec`, or should it also be allowed to propose custom query expressions?
- How should ambiguous roots be resolved in the UI: search picker, backend warning, or AI clarification?
- Should diagrams be allowed to render against dirty staged editor contents in v1, or only after compile?

## Recommended MVP Scope

Build only this first:

- `metamodel_specialization`
- `POST /api/diagrams/render`
- `DiagramRenderer`
- ephemeral AI diagram artifact
- popout
- save as `.diagram`
- reopen saved `.diagram`

This proves the complete lifecycle while keeping the first implementation narrow and testable.
