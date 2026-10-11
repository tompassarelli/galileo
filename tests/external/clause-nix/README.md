# Clause Nix-module census checkpoint (4.6)

The source is `$FIRN_ROOT/native/nix` at immutable Firn revision
`10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9`. Read the sources in place;
these ledgers contain measurements and references, not copied fixtures.

There are **185 Clause source files**, consisting of **184 module entries**
and the shared declaration file `nixpkgs.clause`. All 184 module entries
import that shared file. The shared file has four exported helper functions;
the module files have 184 exports, for 188 exports across the corpus. This
explains the difference between PLAN's “185 modules” and SPEC's 184 authored
module entries.

`census.tsv` records occurrence counts, source counts, and source filenames
for 32 construct categories and each punctuation token. The temporary
lexical census scanned all 185 files, skipped both comment forms, treated
triple-quoted blocks as raw text, and recursively scanned interpolated
expressions including nested strings. It found no fractional literals.
Type declarations containing `F64` therefore require no literal conversion
in this corpus.

The categories distinguish record and opaque foreign types, generic foreign
and ordinary functions, local functions, foreign call/get/root modes,
records, sequences, paths, dynamic dictionaries, record-at, typed lambdas,
field access, equality, conditionals, text blocks, interpolation and every
escape spelling present. `call-names.tsv` inventories names immediately
followed by `(`, including declaration signatures. Its counts are lexical
spans, not counts of executed or elaborated calls. `foreign.tsv` references
all 62 foreign declarations and records their mode, target and source root.

`covering.tsv` selects 20 actual module entries. Coverage includes each
module's `nixpkgs.clause` import closure; the shared source is not counted
as a module. A greedy structural cover takes bash, gtk, north-profile,
bench-shield and go-env, then sorted module entries fill the set to 20.
The “new constructs” column accounts for all 32 categories. Each parse
identity remains **NOT RUN**. This is a census checkpoint, not a completed
import or a parser acceptance claim.

The existing `muove build` can build a manually authored btop module.
The following form, required for imported explicit option calls by
SPEC §10.2, currently fails:

```text
nix module
  arguments: config, lib, ...
  options.myConfig.modules.system.stateVersion: lib.mkOption({type: lib.types.str, description: "state version"})
  config
    system.stateVersion: config.myConfig.modules.system.stateVersion
```

The error is at `3:3`: `expected a module item: let name = expr, option
path: type, imports: expr, config [when expr] with indented entries, or a
for block`. The importer crate/API, exact normalization for quoted and
nested Clause records, and the representation of explicit root option
records await the required Opus ruling. No importer or alternate lowering
was implemented to bypass that boundary.
