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
- `language::KermlLanguageModule` and `language::SysmlLanguageModule` are in-tree module implementations that delegate to the existing parser/compiler code.
- `language::BaselineLibrary` distinguishes empty, Kernel, SysML, and custom library contexts.

## Migration Rules

1. Keep public SysML/KerML wrappers while moving internals to registry dispatch.
2. Prefer `library_context` for generic compiler inputs; reserve `stdlib` for concrete packaged standard libraries.
3. Keep KerML independent of SysML library defaults.
4. Move SysML aliases, mappings, rulepacks, and bundled libraries behind the SysML module boundary before considering a crate or repo split.
5. Keep KIR as the canonical output for every language module.
