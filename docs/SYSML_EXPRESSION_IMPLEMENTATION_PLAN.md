# SysML Expression Implementation Plan

## Goal

Move SysML-derived feature expressions from "ignored parser tail" to a compiled and executable path in the existing frontend -> resolver -> KIR -> runtime pipeline.

## Current State

- The parser accepts `=` tails but discards them.
- The frontend AST has no expression node.
- The transpiler does not emit expression semantics from `.sysml`.
- The runtime can only evaluate legacy string expressions stored directly in KIR JSON.

## First Slice

Implement a narrow end-to-end slice that matches the runtime already present in the repo:

- usage declarations can carry an optional expression
- the parser builds expression AST nodes instead of skipping `= ...`
- supported syntax is limited to:
  - `self`
  - dotted paths like `self.parts.mass`
  - function calls `count(...)` and `sum(...)`
  - integer, string, and boolean literals
  - qualified names for initializer-style references
- the resolver validates and carries expression structure
- the transpiler emits structured `expression_ir`
- the runtime evaluates `expression_ir` first and falls back to the old string form for legacy JSON models

## Data Model

Frontend AST:

- `Expr::Literal`
- `Expr::Name`
- `Expr::SelfRef`
- `Expr::Path`
- `Expr::Call`

Resolved form:

- keep the same narrow shape
- enforce:
  - only `count` and `sum` are callable
  - only single-argument calls
  - only `self`-rooted paths are executable in the runtime slice

KIR emission:

```json
"expression_ir": {
  "kind": "call",
  "function": "sum",
  "args": [
    {
      "kind": "path",
      "root": "self",
      "segments": ["parts", "mass"]
    }
  ]
}
```

## File-Level Changes

`mercurio-core/src/frontend/ast.rs`

- add expression node types
- add `expression: Option<Expr>` to usage declarations

`mercurio-core/src/frontend/sysml.rs`

- replace the current `consume_expression_tail()` path with a real expression parser
- keep grammar intentionally narrow for the first slice

`mercurio-core/src/frontend/resolver.rs`

- carry expressions through collected and resolved usages
- validate supported call names and path roots

`mercurio-core/src/frontend/transpile.rs`

- emit `is_derived` based on the presence of an expression or the `derived` modifier
- emit `expression_ir` for resolved expressions

`mercurio-core/src/runtime.rs`

- evaluate `expression_ir` before reading the legacy raw string `expression`
- support:
  - `count(self.x)`
  - `sum(self.x.y)`
  - literal values

## Explicit Non-Goals For This Slice

- full operator grammar
- action language or behavior execution
- assignments or side effects
- type inference beyond existing feature/type lookup
- exhaustive SysML expression coverage

## Acceptance Criteria

- `.sysml` source with a derived feature expression survives parse and resolve
- transpiled KIR contains `expression_ir`
- runtime can evaluate that KIR expression using `ExecutionContext`
- existing legacy JSON examples continue to evaluate through the fallback string path
