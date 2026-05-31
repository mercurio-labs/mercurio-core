# Language Module Architecture

Status: initial in-tree boundary.

Mercurio separates the shared model/runtime stack from concrete source languages:

```text
KIR + graph/runtime/query
  <- shared frontend infrastructure
      <- KerML language module
          <- SysML language module
          <- future KerML-based language modules
```

KerML is the semantic foundation. It can parse without any library context and can compile trivial self-contained models with an empty context. Non-trivial KerML should use a Kernel/KerML baseline library. SysML should use Kernel/KerML plus SysML libraries, mappings, aliases, rulepacks, and profile data.

## Current Boundary

- `frontend::ast::ParsedModule` is the shared parsed module type.
- `frontend::ast::SysmlModule` remains as a compatibility alias.
- `language::SourceLanguage` is the single source language enum for linting, formatting, profiles, and registry dispatch.
- `language::LanguageModule` defines parse, compile, mappings, extensions, and default baseline behavior.
- `language::kerml::parser` and `language::sysml::parser` are the language-facing parser/compiler entrypoint modules.
- `language::KermlLanguageModule` and `language::SysmlLanguageModule` are in-tree module implementations that delegate through those parser modules.
- `language::BaselineLibrary` distinguishes empty, Kernel, SysML, and custom library contexts.
- `default_sysml_library_path()` and `default_sysml_rulepack_path()` name the current SysML artifacts directly; `default_stdlib_path()` and `default_stdlib_rulepack_path()` remain compatibility wrappers.
- `default_kernel_library_path()` points to the committed bootstrap Kernel KIR artifact and can be overridden with `MERCURIO_KERNEL_LIBRARY_PATH`.
- `mercurio-kerml` and `mercurio-sysml` are facade crates over the in-tree language modules. They establish the crate boundary before the parser implementation is physically moved out of `mercurio-core`.

## Migration Rules

1. Keep public SysML/KerML wrappers while moving internals to registry dispatch.
2. Prefer `library_context` for generic compiler inputs; reserve `stdlib` for concrete packaged standard libraries.
3. Keep KerML independent of SysML library defaults.
4. Move SysML aliases, mappings, rulepacks, and bundled libraries behind the SysML module boundary before considering a crate or repo split.
5. Keep KIR as the canonical output for every language module.

## Default Loading

When no project descriptor or explicit standard-library override is present, source-oriented commands should load the baseline selected by the requested language module:

- KerML: committed bootstrap Kernel baseline, or the file pointed to by `MERCURIO_KERNEL_LIBRARY_PATH`.
- SysML: bundled SysML library, through the compatibility `default_stdlib_path()` path.

Project descriptors and explicit `--stdlib` options take precedence over language defaults. Descriptor resolution also has a language-aware entrypoint, `resolve_project_context_for_language`, so descriptor-less KerML uses the Kernel baseline while the compatibility `resolve_project_context` keeps SysML as the default.
