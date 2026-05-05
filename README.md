# Mercurio Core

Mercurio Core is the open source Rust library and CLI workspace for working with SysML v2, KerML, and Mercurio's KIR JSON model representation.

The goal of this repository is to make the modeling kernel useful on its own: parse source models, compile them into semantic KIR, lint them, package them, and expose reusable library APIs that private products and external tools can build on.

## Objectives

- Provide a reusable Rust library for SysML v2 and KerML model processing.
- Keep the core model semantics independent from any particular server, desktop app, or hosted product.
- Offer a small public CLI that demonstrates the library without requiring the private product repo.
- Use KIR as the stable semantic interchange format for graph queries, derived values, package loading, and downstream applications.
- Keep maintainer-only diagnostics, benchmarks, and Pilot comparison workflows separate from the public CLI.

## What Lives Here

- `mercurio-core` parses, compiles, lints, loads libraries, builds runtime graphs, computes derived values, and exposes shared domain DTOs.
- `mercurio-cli` provides the public `mercurio` command for parse, compile, lint, and package workflows.
- `mercurio-tools` contains maintainer tools for diagnostics, benchmarks, demos, and Pilot comparison/export workflows.
- `resources/` contains bundled runtime and standard library artifacts.
- `examples/` and `fixtures/` provide SysML, KerML, and KIR models for tests and demonstrations.

The HTTP server and UI live in the private `mercurio-product` repository. They depend on this repository for domain behavior; this repository remains the open source library and command-line surface.

## Core Concepts

### Source Languages

Mercurio reads SysML v2 and KerML source files. Files ending in `.sysml` are treated as SysML, and files ending in `.kerml` are treated as KerML. Inline CLI text defaults to SysML unless `--language kerml` is provided.

### KIR

KIR is Mercurio's semantic model document format. It is JSON, validated by the core library, and used by graph queries, derived values, requirements views, package loading, and product hosts.

### Standard Library

Semantic compilation and linting use the bundled default standard library unless a command is given `--stdlib PATH`. The default path is provided by `mercurio_core::default_stdlib_path()`.

### KPAR Packages

A `.kpar` package is a source-backed zip package containing SysML/KerML sources plus package metadata. Mercurio can build these packages from source files and load them later as baseline libraries.

## Requirements

- Rust toolchain with Cargo
- Java, only for the Pilot comparison/export tools under `tools/pilot-exporter`

Most commands below assume you are running them from the repository root.

## Quick Start

Build the workspace:

```powershell
cargo build
```

Run the test suite:

```powershell
cargo test
```

Show the public CLI:

```powershell
cargo run -p mercurio-cli --bin mercurio -- --help
```

Parse an inline SysML model:

```powershell
cargo run -p mercurio-cli --bin mercurio -- parse --text "package Demo { part def Vehicle; }"
```

## CLI Examples

The public CLI is one cohesive `mercurio` binary with `parse`, `compile`, `lint`, and `package` subcommands. `parse`, `compile`, and `lint` accept `--file PATH` or `--text TEXT`; inline text defaults to SysML, and file input defaults from `.sysml` or `.kerml`.

### Parse SysML or KerML

Parse one file and print a syntax summary:

```powershell
cargo run -p mercurio-cli --bin mercurio -- parse --file "examples/src/examples/Simple Tests/PartTest.sysml"
```

Parse inline SysML:

```powershell
cargo run -p mercurio-cli --bin mercurio -- parse --text "package Demo { part def Vehicle; }"
```

Emit the syntax AST as JSON:

```powershell
cargo run -p mercurio-cli --bin mercurio -- parse --file "examples/src/examples/Simple Tests/PartTest.sysml" --format json
```

### Compile to KIR

Compile a file to KIR using the default stdlib:

```powershell
cargo run -p mercurio-cli --bin mercurio -- compile --file "examples/src/examples/Simple Tests/PartTest.sysml"
```

Compile inline KerML with an explicit language:

```powershell
cargo run -p mercurio-cli --bin mercurio -- compile --text "package Demo { classifier Vehicle; }" --language kerml
```

Emit the KIR document as JSON:

```powershell
cargo run -p mercurio-cli --bin mercurio -- compile --text "package Demo { part def Vehicle; }" --format json
```

Override the stdlib:

```powershell
cargo run -p mercurio-cli --bin mercurio -- compile --file model.sysml --stdlib resources/stdlib.full.kir.json
```

### Lint SysML or KerML

Lint one file:

```powershell
cargo run -p mercurio-cli --bin mercurio -- lint --file "examples/src/examples/Simple Tests/PartTest.sysml"
```

Lint every `.sysml` and `.kerml` file under a directory:

```powershell
cargo run -p mercurio-cli --bin mercurio -- lint --file "examples/src/examples/Simple Tests"
```

Emit JSON diagnostics:

```powershell
cargo run -p mercurio-cli --bin mercurio -- lint --file "examples/src/examples/Simple Tests" --format json
```

Fail when warnings are present, useful for CI:

```powershell
cargo run -p mercurio-cli --bin mercurio -- lint --file "examples/src/examples/Simple Tests" --warnings-as-errors
```

### Build KPAR Packages

Build a source-backed `.kpar` package from a model file:

```powershell
cargo run -p mercurio-cli --bin mercurio -- package build --file model.sysml --out model.kpar
```

Build a package from every `.sysml` and `.kerml` file under a directory:

```powershell
cargo run -p mercurio-cli --bin mercurio -- package build --file examples/src/examples --out examples.kpar
```

Override the package metadata:

```powershell
cargo run -p mercurio-cli --bin mercurio -- package build --file model.sysml --out model.kpar --name Demo --version 0.1.0
```

## Developer Tools

The `mercurio-tools` crate contains diagnostics, benchmark, demo, and Pilot comparison binaries. These are useful for maintainers, but they are separate from the public CLI surface.

### Inspect Connection Resolution

Dump parsed connection declarations and resolved usages for a SysML file:

```powershell
cargo run -p mercurio-tools --bin inspect_connection -- "examples/src/examples/Simple Tests/ConnectionTest.sysml"
```

### Run the Runtime Demo

Run graph subtype queries, feature queries, and a derived value calculation against the vehicle example model:

```powershell
cargo run -p mercurio-tools --bin runtime_demo
```

### Diagnose Example Corpus

Compile the default example corpus and emit a JSON diagnostic summary:

```powershell
cargo run -p mercurio-tools --bin diagnose_examples
```

Diagnose each top-level folder separately:

```powershell
cargo run -p mercurio-tools --bin diagnose_examples -- --folders --root examples/src/examples --out target/example-diagnostics.json
```

### Benchmark Example Compilation

Benchmark each top-level example folder:

```powershell
cargo run -p mercurio-tools --bin benchmark_examples -- --folders
```

Benchmark the full examples tree as one workspace:

```powershell
cargo run -p mercurio-tools --bin benchmark_examples -- --all --root examples/src/examples
```

Benchmark incremental edited-file behavior:

```powershell
cargo run -p mercurio-tools --bin benchmark_examples -- --edited --root examples/src/examples
```

## Pilot Comparison Tools

Several binaries compare Mercurio output against the Pilot implementation. These tools expect a Pilot checkout or exported Pilot artifacts.

Audit a Pilot corpus:

```powershell
cargo run -p mercurio-tools --bin audit_pilot_corpus -- --corpus small --pilot-root path/to/pilot --out target/pilot-audit.json
```

Compare one KerML example:

```powershell
cargo run -p mercurio-tools --bin compare_kerml_examples -- --examples-root examples/kerml/examples --relative-path "Vehicle Example/VehicleDefinitions.kerml" --pilot-root path/to/pilot --out target/kerml-compare.json
```

Compare Pilot AST, compile diagnostics, or semantics for one case:

```powershell
cargo run -p mercurio-tools --bin compare_pilot_ast -- --pilot-root path/to/pilot --relative-path "examples/Simple Tests/PartTest.sysml" --out target/pilot-ast.json
cargo run -p mercurio-tools --bin compare_pilot_compile_errors -- --pilot-root path/to/pilot --relative-path "examples/Simple Tests/PartTest.sysml" --out target/pilot-errors.json
cargo run -p mercurio-tools --bin compare_pilot_semantics -- --pilot-root path/to/pilot --relative-path "examples/Simple Tests/PartTest.sysml" --out target/pilot-semantics.json
```

Import Pilot standard library export data into KIR:

```powershell
cargo run -p mercurio-tools --bin import_pilot_stdlib -- --from-export path/to/pilot-stdlib-export.json --out resources/stdlib.kir.json
```

Or export directly from a Pilot checkout:

```powershell
cargo run -p mercurio-tools --bin import_pilot_stdlib -- --pilot-root path/to/pilot --out resources/stdlib.kir.json
```

## Repository Layout

- `Cargo.toml` - workspace manifest
- `crates/mercurio-core/` - library crate
- `crates/mercurio-cli/` - public command-line binaries
- `crates/mercurio-tools/` - maintainer diagnostics, benchmarks, demos, and Pilot comparison tools
- `crates/mercurio-core/src/frontend/` - SysML, KerML, linting, formatting, and resolver code
- `crates/mercurio-core/src/api/` - shared application DTOs and router helpers consumed by private hosts
- `examples/` - KIR JSON models and SysML/KerML example corpora
- `resources/` - bundled runtime and library resources
- `docs/` - deeper architecture and implementation notes
- `crates/mercurio-core/tests/` - integration and corpus tests
- `tools/pilot-exporter/` - Java helper used by Pilot comparison workflows

## More Documentation

See [docs/README.md](docs/README.md) for architecture notes, language support plans, runtime details, server plans, and semantic service documentation.
