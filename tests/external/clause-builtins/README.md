# Clause text built-ins: external reference

PLAN 8.4 records the Clause implementation consumed by the pinned North
checkout for SPEC §3.5. `cases.jsonl` contains actual admitted projections,
not expectations computed by Galileo or by Rust text operations in the recorder.
Each line has a stable case ID, the exact Clause function name, its ordered
arguments and its JSON text or Boolean output. JSON escapes preserve control
characters and distinguish composed from decomposed Unicode.

There are 24 input pairs and six functions per pair: 144 recorded cases.
Inputs cover empty text, ASCII case and whitespace, accents, dotted I,
Greek sigma, sharp S, CJK, emoji, combining marks, Unicode whitespace,
zero-width space, NUL and the preservation of remainder spacing.

## Reproduce

Use an owned North lane at revision
`eaee1838d1968b7caba65195866e5d52fd30b2e1`. Read the pinned fixtures in place;
do not change North's immutable pin or its Cargo dependencies. From that lane,
set `GALILEO_FIXTURES` to this directory's absolute path, then run:

```sh
mkdir -p examples
cp "$GALILEO_FIXTURES/recorder.rs" examples/galileo_clause_builtins.rs
bun "$(dirname "$(agents path machine-capacity)")/scripts/machine-capacity.mjs" run --class moderate --owner galileo-clause-builtins --timeout-seconds 1800 -- nix develop -c cargo run --offline --example galileo_clause_builtins -- authoring-card > target/clause-authoring-card.txt
```

Read that exact pin's authoring card before editing Clause source. Check and
record through the built executable:

```sh
target/debug/examples/galileo_clause_builtins "$GALILEO_FIXTURES" check-source
target/debug/examples/galileo_clause_builtins "$GALILEO_FIXTURES" > target/clause-builtins.jsonl
cmp "$GALILEO_FIXTURES/cases.jsonl" target/clause-builtins.jsonl
```

The source check opens a checked resident source session, reaching reading,
elaboration, lowering and session opening. Every input invokes `record` with
typed runtime Text arguments. Outputs are decoded only from the admitted
projection's declared document fields. `source.clause` and `recorder.rs` are authored
research harnesses; no North or Clause source is vendored here.

## Names and semantic boundary

Galileo `contains` corresponds to Clause `contains-text`. Galileo `rest-words`
corresponds to Clause `remaining-words`. The pin's word functions are
`first-word` and `remaining-words`; it exposes no list-valued `words` primitive.
These records establish those exact primitive results. A future Galileo `words`
implementation must use their whitespace behavior without treating the
remainder as normalized text. `trim` and `starts-with` are recorded alongside
the requested lowercase, containment and word functions.

The authoring card describes exact substring membership, Unicode lowercase
mapping without normalization or locale collation, Unicode whitespace word
boundaries and preservation of internal and trailing remainder whitespace.
The recorded cases are the executable reference for these claims.

## Provenance

- Recorded on 2026-10-10 on x86_64 Linux.
- North source: `https://github.com/tompassarelli/north`, commit
  `eaee1838d1968b7caba65195866e5d52fd30b2e1`.
- Clause source: `https://github.com/tompassarelli/clause`, commit
  `258e6956aa11dab2e724e89439e864e5e5dafdd6`, as declared by that North
  checkout's `Cargo.toml` and `Cargo.lock` (MIT OR Apache-2.0).
- Rust: `rustc 1.96.1 (31fca3adb 2026-06-26)`, compiler commit
  `31fca3adb283cc9dfd56b49cdee9a96eb9c96ffd`, LLVM 22.1.2, from North's
  pinned Nix development shell. The compiler version matters for Unicode
  tables used by Clause's native text evaluation.
- North `Cargo.toml` SHA-256:
  `a1eda315452a3edbd7b4e49ff8a917472016bbabd0bd19a60dd96ef0297c5ec8`.
- North `Cargo.lock` SHA-256:
  `5895f9d0a41f3fc4bf469994c4e7491c43dd748f466ebdda4e8178ab44129660`.
- North `flake.lock` SHA-256:
  `c805f4f75cd1e1e180c59f8a6df326c6454da21f272c0106c4baa646cf4739c0`.
- North `rust-toolchain.toml` SHA-256:
  `d378cc6bc4c46cf85f3bc26a191bcb13a6f9efab78f758b39029555f88dc6e79`.
- `SHA256SUMS` identifies the recorded data and exact local harness inputs.

## Done when

1. The pinned Clause source session check accepts the research harness.
2. The admitted output has at least 30 cases, including empty, ASCII and
   non-ASCII inputs for lowercase, containment and both word functions.
3. The recorded data and reproducible provenance are retained here.

Not required: North publication, a menu migration, changes to Galileo semantics,
or the full North test suite. This box records an external reference only.
