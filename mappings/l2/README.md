# L2 Mapping Files

This directory separates the L2 parser binding into two layers.

## `pilot_constructs.seed.json`

Pilot-derived syntax binding:

- textual construct name
- pilot metaclass
- source grammar file and line

This file is intended to be refreshed from the pilot grammar.

## `kir_emission.seed.json`

Mercurio-owned normalization binding:

- metaclass
- KIR kind
- KIR id template
- direct properties to emit
- metadata fields to preserve

This file is intentionally independent of the pilot runtime so KIR remains canonical here.
The KIR document and element contract is documented in [docs/KIR_SPEC.md](../../docs/KIR_SPEC.md).
