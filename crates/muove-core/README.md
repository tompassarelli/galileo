# Core IR

`ir.rs` owns the dependency-free program representation and builders. Relation
IDs index `Program.relations`. Names supplied to the IR are already scoped by
the front end. Builders retain source spans and foreign fact origins; semantic
validation runs through `check(&Program)` before evaluation.

`Program::builder()` adds relations, entities, facts, initial state, rules,
queries, definitions, events, handlers, effects and requirements. Relations
carry kind, typed slots, template, functional mode and checkpoint status.
`Expr` represents known expressions and queries. `Value::Nix` holds normalized
opaque syntax supplied by lowering, whose equality never evaluates Nix.
`Literal` includes every SPEC §4.3 form. `StateClaim` keeps handler postconditions
and invariant builders separate from temporal claims.

Reusable fixtures are `examples::dungeon_static()`, `examples::tags()` and
`examples::r13()`. Dungeon-static preserves the prototype's static model for
interpreter comparisons; the stateful acceptance dungeon is a later runtime
fixture. Tags follows SPEC §9.6, including its sample facts and requirements;
those sample facts intentionally need further opt-in and platform declarations
to satisfy the requirements. R13 has 20 independent stages, two choices per
stage and one final conjunction, for 2^20 minimal supports.

Done when:
- `cargo test -p muove-core`: the external-reference fixture check passes.
- `cargo run -p muove-core --example program-counts`: prints dungeon-static
  10 relations / 5 rules, tags 13 / 5 and R13 4 / 3.
- `cargo metadata --no-deps`: muove-core has 0 dependencies.

Not required: evaluation, proof search, runtime admission,
parser/lowering, or changes outside muove-core and the 3.1 plan row.

## Static checks (SPEC §4.3)

`check(&program)` returns `CheckedProgram` or a list of `StaticError`s.
The checked result retains each rule's grounding order as body indexes,
relation strata, strongly connected components, and dependencies. Input body
order has no semantic significance. Nested aggregates, argmin and queries
validate their bodies before grounding their output; their local bindings do
not escape. Queries reached through definitions or named queries add strict
dependencies to every relation they read.

Diagnostics retain the rule's source span (or the head span for invented
values), identify ungrounded literals, print the relation cycle with its
edge kinds, and name the recursive head slot that invents a value.

Done when:
- `cargo test -p muove-core`: four SPEC §4.3 rejection scenarios identify a
  growing membership, hidden query cycle, binding cycle and negation cycle;
  the dungeon-static, tags and R13 builders pass after body reversal.
- `cargo metadata --no-deps`: muove-core retains 0 dependencies.

Not required: model evaluation, budget charging, front-end type checking,
or changes outside muove-core and the 3.2 plan row.

## Source record

Dungeon fixture adapted from
`inputs/claude-muove-plan/prototype/examples/dungeon.clause`, imported in
`6df2bc261a82a569fe40fd06e14d7c4d7049b6bc`. Its license is MIT OR Apache-2.0;
this adaptation uses MIT. The required copyright and license are retained in
`LICENSE-MIT`. Core types/builders are an independent implementation of
SPEC §§3–6. Tags follows SPEC §9.6. R13 implements the behavioral scenario in
`inputs/chatgpt-muove-feedback/chatgpt-comment.md` (same import, lines 319–329);
no reviewer code is reused.
