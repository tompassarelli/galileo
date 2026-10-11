# Galileo decisions

This file records the decisions made for the MVP, the four that Tom made,
and how each review finding was resolved.

## 1. Decisions made

Each decision gives its evidence and, under "Rejected", the alternatives
considered. Evidence IDs refer to SPEC.md Appendix A, and `A-` IDs to its
Appendix B.

### D-1. A new Rust workspace in `~/code/galileo/main`

Galileo is built fresh, porting the prototype pieces listed in P6.

Rejected:

- **Grow Clause.** It is about 120k Rust LOC, stalled after 29 repins in 5
  days, and carries five P0 audit findings (N8).
- **Grow Beagle.** Racket costs 1.8 s per file to start, it has 17 recorded
  silent miscompiles, and its surface has been migrated repeatedly (F11,
  F13).
- **Grow the prototype in place.** It is positive-only Datalog with a naive
  engine, three critical review bugs and no value language (P2, P4, P5).

### D-2. Scope: a general-purpose language, proven first on firn and north

Galileo is meant to be a general-purpose language (T4). firn and north are its
first two targets because they prove it on real work with measured
baselines. The MVP covers firn's declarative and decision layer (modules,
hosts, tags, responsibilities, flake inputs and command plans) and north's
state machine. Their mechanics stay in host languages for now: firn's 59
`.bjs` files go to TypeScript on Bun (T1), and north's terminal, process and
transport code stays in Rust (F12, N6).

What the MVP leaves out is not yet rather than never. SPEC §12 gives each
item with what brings it in, and a construct still enters only with a
runnable program that needs it (guardrail G1). PLAN.md sketches the first
general-purpose milestone after A11.

Growing does not loosen the core. A program stays pure and does its I/O
through its host, as typed requests whose outcomes come back as events (G4,
D-15). Elm and Roc run whole applications this way (A-Elm, A-Roc), and it
keeps replay, explanation and review exact for any program.

Rejected:

- **Rules and data only, for good** (this decision before T4): Tom rejected
  it. It would leave every program's meaning split between Galileo and a host
  language.
- **General-purpose features in the MVP:** firn and north need none of them,
  and none is expensive to add later (T4). Building them first would repeat
  Beagle's surface churn (F13).

### D-3. State consistency through explicit state and events

Derived rules are evaluated within one state. Events are the only transitions.
Temporal claims are answered by explicit-state breadth-first search with
stated bounds (SPEC §5, R1, A-Dedalus, A-Alloy6).

Rejected:

- **Timestamps written by users** (Dedalus `@next` by hand): hard to read
  (A-Dedalus, where the critique comes from the source itself).
- **ASP or SAT planning in the MVP:** grounding blowup and unpredictable
  semantics (A-telingo).
- **Static reachability with "journey" annotations:** these do not carry
  over to north.

### D-4. Three-valued verdicts everywhere

`Unknown` never approves, and `check` exits 2 on it (R5–R8).

Rejected:

- **A boolean plus a status flag:** in the prototype these drifted apart in
  four places.

### D-5. `requires` means counterfactual dependence; strength means minimum failure set

Rejected:

- **Counting supports** (R4).
- **`requires` as proof-structure dependence under negation:** it is
  ill-defined once rules can be blocked through `not`.

### D-6. Lazy proofs of minimal height by default

Exhaustive supports are `--all`, and bounded (A-Souffle, A-KR2026).

Rejected:

- **Eager enumeration of all supports** (R13, R18).

### D-7. Two stages; Galileo never evaluates Nix

Galileo values are either known at Galileo compile time or `Nix` terms, which are
type-checked and emitted unevaluated (SPEC §3.2).

Rejected:

- **Embedding a Nix evaluator** such as snix or tvix: very large, and it
  would make Galileo answerable for Nix semantics.
- **Untyped pass-through of Nix:** Beagle issue #14 shows how that lets
  values bypass checks (F13).

### D-8. Migration proofs

- **Beagle output:** byte-identical files.
- **Clause modules:** parse-identical Nix, each listed with whether
  whiterabbit's drvPath covers it.
- **whiterabbit:** an unchanged toplevel drvPath. nixos-config verifies only
  whiterabbit; ashashi is nix-darwin, and its drvPath is reported where it
  evaluates on Linux.
- **north:** replay under the observation contract of SPEC §10.4.

The drvPath is a sound check because no host embeds the flake revision
(F15).

Rejected:

- **The drvPath alone:** a module that no evaluated host enables is
  invisible to it.
- **Parse identity alone for Beagle output:** `"" + x` and `"${x}"` parse
  differently, and they behave differently when `x` is a path.
- **Comparing evaluated option values:** that needs a custom evaluator.

### D-9. Module metadata as facts in the module's own file

Tags, opt-ins, tag overrides and flake inputs are written as facts beside the
module (SPEC §9.6).

Rejected:

- **184 separate `tags.clause` files** (F2).
- **Stripping metadata out of emitted Nix by text surgery** (F5).

### D-10. Declared sentence templates plus a record register chosen by block headers

SPEC §9.2 and §9.3.

Rejected:

- **S-expressions:** Beagle's surface had to be migrated again and again
  (F13), and the thesis is readability.
- **Free English:** "naturalness is an illusion" (A-Inform7).

### D-11. Vocabularies scoped by module, and explicit quantifiers

Rejected:

- **Global shapes:** importing an unrelated model broke an existing line
  (R16).
- **An implicit "some":** R15.

### D-12. `Int` first; fractional numbers when a program needs them

The MVP has `Int` only. North uses `F64` only as a workaround for missing
integers (N3). At the nixos-config pin, the only fractional literals in
`.nix`, `.bnix` and `.clause` sources are in `machine-capacity.clause`,
which moves to TypeScript (D-33). Fractional numbers are not yet (SPEC
§12). `Int` never converts implicitly and `1.5` is an error in the MVP (SPEC
§9.1), so adding them changes no MVP program.

Rejected:

- **F64 in the MVP:** no MVP program needs it, and floats raise questions,
  such as equality in facts and the order of sums, that the first program
  needing them should settle.

### D-13. Entities with records, create and delete, and derived relations

These replace maintenance handlers.

Rejected:

- **Clause's model**, which forces 6 maintenance handlers after every
  transition and 8-relation manual teardowns (N3).

### D-14. No match rejects by default

A handler opts into the empty step with `may skip`. `galileo import clause`
marks every imported handler `may skip`, so the import keeps Clause's
behaviour until each marker is removed under replay (D-28).

Rejected:

- **Clause's silent empty delta:** north needs 12 Rust postcondition
  re-checks because of it (N2, N4).

### D-15. Effects are typed requests; outcomes come back as receipt events

`at most one per step` keeps the mailbox's rule that competing requests
reject the step, and `settled by` defines which requests are pending after a
restore (SPEC §5.4).

Rejected:

- **A single-slot mailbox** (N3).
- **Effectful Galileo code:** it would break replay.

### D-16. A naive reference interpreter beside the fast engine, with differential tests

Rejected:

- **Oracle tests that call the production engine** (R14).

### D-17. Generated Rust bindings for north

Rejected:

- **Dynamic relation tables marshaled by hand:** about 2,200 boundary lines
  today (N4).

### D-18. Frozen baselines, restricted patches and checked `intends` contracts

Rejected:

- **Concatenating the base and the proposal source:** the proposal could
  approve itself (R9).

### D-19. Fixtures from nixos-config and north are read in place

They are never copied into this repository, which keeps Tom's private
configuration out of a repository he may publish.

### D-20. Dependencies only where they replace an external-format or protocol implementation

`rnix` parses Nix in `galileo-import`, which `galileo import nix` ships, and in
the dev tools, and `lsp-server`/`lsp-types` serve the LSP. The core crates
stay dependency-free (wording corrected by D-39).

Rejected:

- **Zero dependencies everywhere:** that means writing a Nix parser.

### D-21. The first rewrite is nixos-config `modules/kanata`

The pair is `default.bnix` (28 LOC) and `kanata.bnix` (29 LOC), and Beagle
reproduces both byte-identically today (F14). Runners-up: `modules/users`
and `modules/firefox`.

### D-22. The first north slice is the menu, `north.clause` L447–827

Fallback: the references slice (N10).

### D-23. File extension `.gal`

Rejected:

- **`.mv`:** the Move language already uses it.

### D-24. Container layout

`main/inputs/` holds the imported inputs as source next to the original zips.
The prototype's two-commit history stays inside `claude-galileo.zip`.

Rejected:

- **Nesting the prototype's `.git`:** it would create an embedded repository.

### D-25. `nix file` is the general output form; `nix module` is its special case

The flake root, `providerAdapter` and overlays are Nix values, not module
items (SPEC §10.2).

Rejected:

- **Keeping `flake.nix` outside Galileo:** north's `flake.clause` and
  `flake.nix` are already kept in lockstep by hand (N7).
- **A separate flake language:** two output forms over one Nix AST.

### D-26. Event slots are closed (`only in`) or open (`sampled from`)

A universal claim over an open slot is `Unknown`, with the sampled result as
evidence (SPEC §5.5).

Rejected:

- **Enumerating every Text slot:** north's menu query text cannot be
  enumerated.
- **Holds over samples:** the reviewed spec let a search over `"ok"` prove
  `never compromised` while the runtime admitted `"boom"`.

### D-27. Beagle's tools: replaced, except the repair pipeline

`galileo import nix` replaces `beagle-import-nix`, `galileo rename option`
replaces `beagle-rename`, and `galileo refs`, `galileo build --stdout`,
`galileo review` and `galileo schema explain` replace the compiler queries.
The evidence-ranked repair pipeline is dropped. PLAN box 4.9 checks for
callers before Beagle leaves the build.

Rejected:

- **Porting the repair pipeline:** ranking repairs for a failed compile is
  what Galileo's did-you-mean, closest-shape diagnostics and the review gate
  already do, and no caller is known.

### D-28. North migrates mechanically first, then refactors under replay

The importer keeps Clause's behaviour: every handler `may skip`,
maintenance handlers kept in order, FIFO rules as written, and the mailbox
as `at most one per step`. Each move to SPEC §9.7's end state is a separate
change that must replay identically.

Rejected:

- **Importing straight to the end state:** maintenance handlers run at
  different points from derived relations, so a sequence can read a stale
  value in Clause and a current one in Galileo, and an empty move rejects
  instead of skipping.

### D-29. Review compares failing instances, not verdicts

Rejected:

- **Comparing verdicts by requirement name:** a requirement that already
  failed for host A hides a new failure for host B.

### D-30. Invariants and `ensures` take state claims only

Rejected:

- **Temporal invariants:** admission would depend on itself. With
  `invariant every Clock ticks happens when Clock is ready`, admitting the
  tick breaks the invariant, and rejecting it satisfies the invariant.

### D-31. Ids are never reused, and the allocator is part of the state

Rejected:

- **Reusing ids:** an outstanding receipt can still name a deleted entity.
- **Searching up to id renaming:** it is sound when renaming keeps the order
  of ids, but it is a symmetry reduction, which waits until a search needs
  it (SPEC §12).

### D-32. Fixtures are pinned worktrees at named commits

Rejected:

- **Reading nixos-config and north `main` live:** the oracle would change as
  the migration lands.

### D-33. Beagle and Clause callers: replaced where a need remains, otherwise deleted with their feature

WP4 ends Beagle as the Nix compiler and Clause in nixos-config. Beagle
still compiles firn's `.bjs` until the TypeScript port (T1, SPEC §12), and
north uses Clause until A11 (T3). So a caller that serves `.bnix`, a Clause
source or the firn build is replaced or deleted before box 4.14 lands, and
a caller that serves `.bjs` or north stays until that later removal deletes
it. No tool is ported 1:1. Gaps and rows are those of
`notes/4.9-caller-matrix.md`, and line numbers are at `FIRN_ROOT`.

- **G1.** No command replaces `beagle-syntax`: parse errors are
  `galileo check` and `galileo build` diagnostics with file:line (box 2.2).
  None replaces `beagle-provides`: a `.gal` file exports every
  declaration not marked `private` (SPEC §2), and `galileo refs` finds its
  users. Their lines in `docs/compiler-queries.md` and
  `docs/fixtures/tags-example.bnix` go in box 4.22.
- **G2.** `beagle-daemon` is dropped, and Galileo gets no query cache:
  `galileo lsp` holds the memo (SPEC §11.2), and a full build is budgeted at
  2 s (§10.5). The PostToolUse hook its doc names does not exist in the
  pin. The doc lines go in 4.22.
- **G3.** `clause-workbench`, `FIRN_CLAUSE_WORKBENCH` and
  `config/clause-revision` are deleted with every caller in box 4.19: F1,
  F4, F9's Clause branch, F14 to F16, F19, and `firn-runtime-update`'s
  other Clause lines (7, 37–43, 256, 291–325, 542–546). firn gets `galileo`
  the way it got Clause. `config/galileo-revision` names a galileo commit,
  `firn-runtime-update` builds it from `~/code/galileo/main` (or
  `FIRN_GALILEO_REPO`) into the runtime's `bin/galileo`, and the launchers
  refuse a checkout that declares another revision. There is no override
  variable; a test builds a scratch runtime. SPEC §11.3 records this.
- **G4.** `prewarm.clause` and `firn-host-list.clause` go to box 4.12. It
  also deletes F3's compile-js branch, F11's workbench lines, the committed
  `native/firn-host-list.{js,d.ts}`, `native/firn-host-list-consumer.ts`,
  `native/firn-host-list-clause.test.sh` and `FIRN_CLAUSE_HOST_LIST_MODULE`
  (`dotfiles/bin/firn:32`, `native/firn_io.mjs:3`). `machine-capacity.clause`
  moves to TypeScript in the skill's own host (box 4.18), not to Galileo: it
  computes in F64 (D-12), it is neither firn nor north (SPEC §1.3,
  guardrail G1), and its bundle runs in spawn hooks that must not depend
  on a second binary. Its fixture assertions and
  `charged-resources.property.ts` are the oracle.
- **G5.** After box 4.8 the tags are facts. firn's tag loader reads them
  with one `galileo query --json`, and box 4.20 deletes the Clause bridge
  (`native/clause_source_host.mjs`, its four importers, F5's staging,
  `load-clause-tags`), the `.bnix` tag parser `load-beagle-module`,
  `native/module_metadata.clause`, and `native/nix/` with its README and
  two tests.
- **G6.** `galileo check` replaces `check-source`, and SPEC §11.1's row says
  so. F13 goes with `native/nix/` (4.20), F18 with 4.18, and F17 with the
  skill (G7).
- **G7.** `galileo help [TOPIC]` replaces `authoring-card`: the commands,
  plus a language card with one example per SPEC §9.4 production, each
  compiled by the tests (a new §11.1 row, box 2.16). AI writes most Galileo,
  and Beagle's 70 recorded hallucinations (F13) show what remembered
  syntax costs. `nixos-config:AGENTS.md:6-7` and its Checks name `.gal`,
  `firn repo build` and the runtime's `galileo help`, `galileo refs` and
  `galileo schema explain` (4.22). The clause-authoring skill stays while
  north writes Clause, and box 8.15 deletes it.
- **Outside 4.9's list, named there.** `galileo schema extract` (§11.1, box
  2.14) replaces `beagle-extract-schema`. `galileo check`, with 2.15's schema
  checks, and `galileo build --check` replace `beagle-validate` and
  `beagle validate` (§11.3). `firn repo validate`, `repo doctor`,
  `repo watch` and `repo upgrade` run those, and box 4.21 deletes firn's
  schema executable (`native/schema_transaction*.bjs` with its host, tests
  and runtime component). `beagle doctor` is dropped, because G3's
  revision check catches toolchain skew. Box 4.22 deletes `modules/beagle`
  and its ten wrappers with their callers: whiterabbit's `-beagle` tag
  edit and the `beagle` entry in `config/darwin-modules.json`. firn runs
  Beagle through `BEAGLE_PATH` directly, whiterabbit already excludes the
  module, and `galileo lsp` serves `.gal`.
- **Outside 4.9's list, found here.** 21 files under `native/` read or
  write `.bnix`, `enabled-tags.bnix`, `configuration.bnix`, `flake.bnix`
  or Clause source: tags, inventory, authoring templates, flake inputs,
  views, repo quality and the system-policy text. So do the checkout tests
  at `dotfiles/bin/firn:7` and `native/prewarm_host.mjs:63`. They fail
  silently once `.bnix` goes, because `load-beagle-module` returns an empty
  module when its file is missing. Readers take facts through
  `galileo query --json`, writers write `.gal`, and a command whose work
  Galileo now does runs that `galileo` command. No `.bnix` parser survives.
  Box 4.17 sizes this, and its boxes land with 4.14. The compile-nix call
  at `native/repo_build.bjs:120` goes in 4.14, where A3 counts it.
- **Kept until a later removal.** The Beagle/JS path stays until T1's
  port: the A rows' `core.js` copies, `beagle-build-all` for `.bjs`, and
  CI's Beagle checkout. `.beagle-cache/` and `config/beagle-validate.json`
  keep their paths until then, and SPEC §2 reads the schema there. Guard
  and inventory entries that name `~/code/beagle` or `~/code/clause` stay
  until that checkout is archived.

Rejected:

- **Deleting every gap with no replacement (the lead's draft):** it leaves
  AI authors of a new language without a checked reference (F13). It also
  maps `beagle-provides` to `galileo refs` and `beagle-extract-schema` to
  `galileo check`, and it gives firn no pinned `galileo`.
- **Galileo subcommands for syntax, provides, daemon, doctor and
  check-source:** `check`, `refs`, `lsp` and the revision check already
  meet each need.
- **machine-capacity in Galileo:** it needs F64, it is not a target, and a
  launch-critical hook would depend on a second binary.
- **A Galileo authoring skill:** feature detail goes in CLI help.
- **Running `galileo` live from `~/code/galileo/main`, as firn runs Beagle:**
  a galileo change would move nixos-config's drift check (D-32's reason).
- **`galileo` as a NixOS package:** fast-changing projects stay outside the
  system closure.
- **Renaming `.beagle-cache` now:** about 50 references change, and no
  check needs it; the rename goes with Beagle.

### D-34. The boot-coupling budget becomes Galileo requirements, and backticks quote a name anywhere

Line numbers are at `FIRN_ROOT`.

- **Box 4.33: Galileo owns the budget's verdict.** firn's boot-coupling
  budget (`native/responsibility_projection.bjs:416–433`) is decision logic
  over responsibility facts, and D-2 and T1 put decision logic in Galileo
  rules. Box 5.7 writes its five parts as requirements beside 5.4's
  realization requirement: no module of environment-domain software is
  enabled on the host; no boot-domain responsibility reaches an
  environment-domain one through `depends on`; every required software has
  a module, a domain, a criticality and, in the boot domain, a lifecycle;
  every required capability is provided; and every boot-domain software is
  a long-lived system service whose module is enabled. firn then reads no
  responsibility facts. The calculation's only caller is the test that
  `native/repo_build_native.test.sh` runs, so box 4.33 makes that step run
  `galileo check` and deletes the calculation and its test without porting
  them.
- **The oracle fails at the pin.** whiterabbit enables `obsidian` and
  `blender` through its `notes` and `creative` tags (nixos-config
  `61d004a6` and `2e0393a8`), while
  `hosts/whiterabbit/responsibilities.bnix:43–44` puts their workflows in
  the environment domain. No check noticed: CI has not passed since
  2026-08-16, before the budget landed (`ea236556`), and the live check
  loads module tags through the Clause workbench
  (`native/tag_resolve_family.bjs:171`), which CI does not set up. Until
  Tom changes the enables or the responsibilities, `galileo check`, and with
  it `firn repo validate` (4.21), will fail on that one instance. A5 means
  5.4's realization requirement, not the budget.
- **Box 2.2: backticks quote a name in every position that takes one.**
  That covers entity, type, shape, slot and field names, `define` names and
  parameters, `let` binders, `arguments:`, lambda parameters, option paths,
  attrpath segments and fields after `.`. The quoted name is the bare name
  and emits bare: `` let `when` = true `` binds `when`. The backticks hold
  one bare name, without spaces. Variables need none, since `?when` cannot
  collide, and a text literal stays the way to write an attribute key that
  is not a name. 2.3 already reads quoted names in expressions and
  attrpaths; 2.2's and 2.8's review fixes add the positions they read, so
  `` arguments: `config` `` and `` let `when` = true `` parse to `config`
  and `when`, and `` `foo bar` `` in a name position is an error with
  file:line. Kotlin, Swift and Scala accept a backticked name wherever a
  name goes, as Rust does with `r#name`, and a general-purpose language
  (T4) keeps adding keywords, so it needs the escape everywhere. Collisions
  already occur in firn: `modules/yazi/default.nix:2` names an attribute
  `for`.

Rejected:

- **Feeding the existing calculation through `galileo query --json` (the
  4.17 census):** the logic stays in `.bjs` for T1 to port to TypeScript,
  realization is checked twice (5.4 and `incomplete-software`), and a
  failure gets no `why not`, review label or LSP diagnostic. The unseen
  failure above is what a gate outside Galileo costs.
- **Violations as a query that firn checks for emptiness:** an incomplete
  search returns too few rows and would pass, and `Unknown` never approves
  (D-4).
- **TypeScript, the way machine-capacity went (D-33, G4):** none of G4's
  reasons hold. The budget is set logic with no F64, responsibilities are
  firn's (D-2), and it runs in a test, not in a launch hook.
- **Backticks for entity names only (§9.1 read literally):** a Nix binder
  named like a keyword could not be written, so importing one would mean
  renaming it, which changes the emitted Nix (D-8).
- **Contextual keywords** (reading `when` as a name wherever a name is
  due): the sentence register needs an escape anyway where a name and a
  template word both fit (§9.1), so this adds a second mechanism instead of
  replacing the first.
- **Any characters inside backticks, as in Kotlin's test names:** such a
  name cannot be a Nix binder, and it duplicates text-literal keys.
  Allowing more later breaks no program; allowing less would.

### D-35. The expression grammar left open by box 2.3

Box 2.3 (`b4fa04e`) parsed expressions with six choices open. Each ruling
keeps the MVP strict wherever no firn, north or acceptance program needs
more (G1), because allowing more later breaks no program and allowing less
would (D-34). SPEC §9.4 holds the grammar, and boxes 2.3b and 2.3c bring the
parser to it. Counts are at `FIRN_ROOT` and `NORTH_ROOT`.

- **Inline records, as landed.** `{attrpath: expr, …}`, with commas, on one
  line and without `when`. Keys nest and merge as entry keys do, and a
  conditional field is written as an indented entry.
- **Precedence, as landed, with two additions.** Loosest first: `or`,
  `and`, `not`, the comparisons at one level that does not chain, `//`,
  `+ -`, `*`, `++`, then field access and calls. `not` is never the bare
  operand of a tighter operator, so `a == not b` is an error (box 2.3b).
  Outside text, paths and comments, `! ? % & ^ ~ @ $ ; \` are reserved for
  later operators, except in `!=` and a variable's leading `?` (2.3b).
  Today `a%b` and `x;y` lex as names, which would make any such operator a
  breaking change.
- **Types: two grammars, parsed into trees.** `type` is a name with
  arguments (`List Text`), `maybe T` or `nix T`, and `option-type` is
  §10.2's table. The syntax crate parses both, a malformed type is a parse
  error, and a name that is not a type is a diagnostic (box 2.3c). Today
  the tokens reach `check.rs`, whose reader turns any type it does not
  know, such as `Strng` or `lsit of text`, into `Any`, and `nix.rs` reads
  option types a second time.
- **Negation: not yet, with its syntax kept free.** No program needs it:
  nixos-config's 235 `.nix` files and north's two Clause files write no
  negative number, and north's `-1` menu deltas arrive as event data. A `-`
  that begins a token lexes as the operator, and where an operand is due it
  is an error that suggests `0 - x` (2.3b). Today `-x` lexes as a name, and
  `-(x)` as a call to a function named `-`. The first program that writes a
  negative number brings in prefix `-`, binding tighter than every binary
  operator and looser than field access and calls, with a `-` before an
  integer literal folded into the literal so that the smallest `Int` can be
  written.
- **A `where` condition, as landed, made uniform.** It reaches to the end
  of the line, to the first `,` outside the brackets it opened, or to a
  closing bracket it did not open, for queries, aggregates and `first`
  alike. A nested `where` takes the rest of its condition, as 2.9's
  aggregates already do, so `?x in all ?y where A and B` puts both literals
  in the inner query. An `or` beside a separating `and` must be
  parenthesized (2.3b).
- **Field access, as landed: a name only,** bare or in backticks (D-34). In
  nixos-config's Nix, every computed attribute (`${…}`) and every quoted
  attribute that is not a name is a binding key, which attrpaths already
  cover, and no expression selects one. `x."key"` and `x.{e}` wait for a
  program that needs them, and a known map is read with `lookup`, which
  returns `maybe` (§3.5).

Rejected:

- **Nix's `{ a = 1; }`:** Galileo writes `name: value` everywhere else, and
  `=` binds `let`.
- **`when` on inline fields:** a record passed to a call would need
  `lib.optionalAttrs` where an entry gets `lib.mkIf`, so one keyword would
  lower two ways.
- **Nix's exact order,** with `!` above `//` and equality below ordering: a
  word `not` negates the comparison after it, as in Python and SQL, and two
  comparison levels would accept `a < b == c`.
- **Chained comparisons:** no program needs them, and while they are an
  error, adding them breaks nothing.
- **`++` below `+`, as in Haskell:** no well-typed expression tells the two
  orders apart, and Nix's order keeps imported and emitted trees alike.
- **Raw type tokens until a checker needs them:** each lenient reading
  accepted now would break once types are parsed.
- **One grammar for option types and value types:** option types are
  NixOS's, with their own merging, and §10.2 already names them.
- **Function types, record types and prefix `-` now:** no program needs
  them (G1).
- **Negative literals in the lexer:** `a -1` would lex as two operands, and
  `-x` would still need an operator.
- **A nested query that stops at the next `and`:** aggregates take the
  rest, so one keyword would end in two places.
- **Reading `a or b and c` in a condition as `(a or b) and c`:** an
  expression, like SQL, reads it as `a or (b and c)`.
- **Ending a condition at `then` or `else`:** a template may use any word,
  so a keyword cannot safely end a sentence; parentheses mark the end.
- **`x."key"` and `x.{e}` now:** no firn or north expression needs either,
  and `x.{e}` on a known map would hide the partiality that `lookup`
  returns as `maybe`.

### D-36. One owner for the language, the next win before breadth, and agents write Galileo

Tom's assistant raised four concerns about how Galileo is being built,
measured on 2026-10-10 at 20:00 (29 of 111 boxes, about 12,800 lines of
Rust, 61 DECISION lines). `notes/language.md` holds the working detail.

- **Coherence: accepted, with the owner's memory in the repository.**
  Fourteen Opus max planners have taken Galileo questions since 18:29, up to
  nine at once, and three of them held questions from box 2.11. The five
  rulings of 19:05 (`notes/language.md` L1–L5) reached worker briefs but
  never SPEC or DECISIONS, and two boxes then built their opposite: 3.4
  sorts by a body-local variable, and 2.11 accepted a declared variable
  that clashes with an outside one (fixed in `afd89d1`). Sequential `let`
  was asked again for 2.4 after L2 ruled it, and `?x is a T` went to two
  planners. The first pass over every ruling found eleven contradictions
  (K1–K11), one already fixed. So one Opus max planner, the language
  owner, now takes every question about what a Galileo program means and
  how it is spelled. The lead keeps it between batches with SendMessage
  and restarts it from the notebook, the only memory that counts. A ruling
  binds once SPEC states it, and lands
  with one DECISIONS entry per batch and the notebook's index lines.
  Workers keep writing `DECISION` lines, take the narrowest reading SPEC
  allows, and land; a box stays unticked only when its Done when depends
  on the answer. Other questions still go to one-shot planners.
- **The sentence register: accepted, and the measured risk is Galileo's own
  keywords, not English.** Declared templates already rule out reading
  English (D-10). At `f2802f2`, `backup requires capability gpu` reads as a
  fact at the top level but fails inside a `requirements` block, because
  `requires` is also a claim keyword, and firn's own vocabulary uses it
  (`lib/firn/responsibilities.gal`). A claim that writes `?h is a Hero`
  makes every claim in its file `Unknown`, and 2.10 lets a prepended `use`
  re-point an existing line (G5). The owner settles keyword collisions
  with one rule (notebook O1).
- **End to end before breadth: accepted as a standing order, and its first
  case is done.** A1 landed (2.6 `fc36795` and 2.7 `bb8a95f`; both kanata
  files byte-identical, 989 and 1,019 bytes), and A5 passes through
  `galileo check` on whiterabbit's real responsibility facts (5.4,
  `f2802f2`). No command yet explains. So the next win is A4 from source:
  `galileo build` emits both hosts' `_generated-enables.nix` byte-identically,
  `galileo why` prints the firefox proof, and `galileo check` runs
  `requirements for tags`. Box 5.1 now needs the reference interpreter
  (3.4) rather than the engine (3.6), which waits on a ruling, and 5.3 owns
  the `galileo why` command. When capacity limits staffing, the next win's
  boxes go first. New box 2.17 lowers `nix file`, which 4.5 and 8.10 need
  and no box owned.
- **Tom's taste: Galileo's authors are agents, and Tom approves direction,
  not syntax.** Amended on 2026-10-10 by Tom's ruling, relayed by his
  proxy after he was shown the kanata module, the dependent-responsibility
  rule and the dungeon facts: "its whatever the agent wants looks fine to
  me. im not writing the config the agent is"; on sentence style,
  "whatever the agent thinks... PL design is trending towards"; and on the
  general-purpose direction (T4), "look good". So the language owner
  decides syntax by what agents write correctly and what current PL design
  favours. The two exercises this concern first gave Tom, and 4.14's wait
  for them, are withdrawn, with no replacement.

Rejected:

- **One-shot planners per batch, as before:** five rulings never reached
  SPEC, two boxes built their opposite, and questions were asked twice.
- **One-shot planners that each read and update the notebook:** that fixes
  persistence, but planners running at once in one area still rule
  without seeing each other.
- **A resident owner whose context is its only memory:** a context fills
  and a restart loses it, and the lead's planners leave no transcript on
  disk.
- **Tom as the language owner:** he approves direction, not syntax.
- **The owner also ruling on Nix layout, migration and crate boundaries:**
  WP1 and WP4 would queue behind questions that do not change what a
  program means.
- **Holding every other box until A4 lands:** its path is at most two
  boxes wide, so workers would idle.

### D-37. Names follow one option tree, a sequential `let` and a target-free prelude; a work unit is one step

These rule on the open questions of boxes 2.4 and 3.3. The name probes ran
on main `35c9f83`, and again on `62a8257` after 2.15, against scratch copies
of `tests/external/kanata`, and nixos-config line numbers are at
`FIRN_ROOT`.

- **Box 2.4: one option tree, checked only where it is complete.** `config`
  in a `nix module` has its module system's option tree: the options that
  Galileo sources declare, together with the declared foreign schema. Where
  both declare a path, Galileo's declaration wins, because Galileo emits it. A
  name is an error only under a closed node. With a schema, every node down
  to a declared option is closed. Without one, Galileo sees only its own
  declarations, so a node is closed only when it directly holds a Galileo
  option, such as `myConfig.modules.kanata`. A node that also holds foreign
  options, such as `services.foo` once Galileo adds `services.foo.extra`,
  needs the schema. firn never meets that case: all 61 option declarations
  in its `.bnix` sources sit under `myConfig`. Past a declared option, the
  rest of a path is a key into its value, such as `main` in
  `services.kanata.keyboards.main`. A nested `nix module (…)` has its own
  module system, and Galileo's declarations do not apply to its `config`.
  The 2.4 checker closes every node above a Galileo option, the root
  included, so `config.services.kanata.enable` is an error today ("`services`
  isn't an option of `config`"), not a pass, and so is
  `config.home.username` in a nested home-manager module. The Beagle
  sources read foreign options 36 times (`config.home` 16, `config.lib` 17,
  `config.hardware`, `config.sops`, `config.stylix`), and 18 of the 63
  nest a module, so box 4.2 would fail on them. Box 2.15 (`62a8257`, which
  landed during this ruling) joins the schema with Galileo's declarations,
  Galileo's winning, but walks only entries, and only when a schema is
  declared. Box 2.4b makes reads walk the same tree and gives the tree its
  rule without a schema.
- **Box 2.4: entry paths are option paths, checked now.** An entry under
  `config` or `config when`, or in a module-level `for` block, walks the
  same tree as a `config.` read, so `myConfig.modules.kanata.enabel: true`
  will name `enable` and `default.gal:3`. Since 2.15 it does when a
  schema is declared, and without one it passes. A computed segment `{…}`
  ends the walk, and the entries of a `nix file` value are attribute keys,
  not option paths. Box 2.4b checks entries against Galileo's declarations
  when no schema is declared, so reads and entries take one walk either
  way.
- **Box 2.4: `let` stays sequential.** A `let` is visible to the items after
  it and to nothing before it, its own value included: `let cfg = cfg` and
  a forward reference both give "`cfg` isn't defined here" today. Beagle's
  `let` is the same. `modules/codex/default.bnix:11–14` binds four names,
  each using earlier ones, and emits four nested lambdas
  (`modules/codex/default.nix:3`). Recursion never comes through `let`, only
  with recursive definitions (SPEC §12.1). A block binds a name once, its
  arguments included, so a second `let cfg`, or `let pkgs = …` under
  `arguments: pkgs`, is an error that names the first; today both pass. An
  inner scope, such as a lambda, a `for` block or a nested module, may reuse
  an outer name. Under these rules T2's idiomatic `let … in`, which Nix
  makes recursive, means what the nested lambdas mean unless a value
  mentions its own name or a later binding's, and that is the case the
  printer refuses (SPEC §10.2). Local `let` in definitions (SPEC §12.1)
  takes the same rules.
- **Box 2.4: the prelude holds Galileo's functions, and the Nix target binds
  Nix's names.** The prelude is the outermost scope. It holds SPEC §3.5's
  functions: `map`, `filter`, `fold`, `join`, `count`, `sum`, `min`, `max`,
  `first`, `lookup` and the text built-ins. `sort … by`, `first … by` and
  the `… where` aggregates are syntax, not names. Inside a `nix module` or
  `nix file` body, the Nix target binds what Nix binds in every file:
  `builtins`, and the bare globals that `firn/nix-lib` types. Those are
  `import` and `toString`, the only two that the Beagle output calls bare
  (`flake.nix:101`, `modules/glide/glide.nix:20`); Clause's output writes
  `builtins."toString"`. Every other global is written `builtins.<name>`,
  and `firn/nix-lib` gives no bare name that a prelude function has, so
  `map` is Galileo's and Nix's is `builtins.map`, as the Beagle sources write
  it 3 times. Outside Nix bodies no Nix name is in scope. Any binding
  shadows a prelude or Nix name, so a new built-in never changes a program.
  Box 2.4c binds them this way. Today `builtins` sits in every file's
  outermost scope, which nothing observes yet because only Nix bodies are
  name-checked, and `toString` is not defined.
- **Box 3.3: the row's number changes, and a unit stays one step.** A work
  unit is one elementary step, charged before the step builds anything: a
  fact loaded, a tuple scanned, a literal tested, a call made, a value built
  into a list, record, tuple or call, an element that a membership or a
  built-in visits or that `++` or `//` copies, or a row or head tuple
  produced. A value grows only by units charged, so real cost stays
  proportional to the units (R12). Steps are counted rather than timed, so
  a budget gives the same verdict on every machine, and the two evaluators
  charge the same kinds of step, so box 3.6 can compare them (on main
  `9822490` the engine spends 55 units on tags to the reference's 179, and
  106 on dungeon-static to 687). The 1,000 in 3.3's row was written before
  the interpreter existed, and SPEC §9.6's tags program costs 179: 169 at
  `a036860`, plus the list items, call arguments and membership tests that
  3.3's review fix (`75fafa1`) began to charge. So the row now asks that
  every budget below a program's cost give `Unknown(work)` with no model,
  and that the cost itself give the model. The landed test checks this at
  every budget for both programs and passes, so 3.3 is ticked. Reading the
  charges found three gaps that neither example program reaches. The
  engine's expressions charge nothing. A call with no arguments charges
  nothing in either evaluator: 20 such definitions that each call the next
  twice made two million calls for 2 units in the reference and 1 in the
  engine, in 4.2 s and 3.0 s. And `++`, `//` and a copied list charge one
  unit or none however large the value: 20 doublings of a text built 1 MiB
  for 84 units in the reference and 1 in the engine, so 40 would ask for a
  terabyte. Box 3.3b closes the first two gaps and box 3.3c the third.
  Recursive definitions (SPEC §12.1) will run under the same unit.

None of this needs breaking for a general-purpose Galileo (T4). A foreign
namespace stays open until a declaration closes it, which a later target's
foreign sources follow too. The scope rules carry over to definition bodies
unchanged. Target names stay out of the prelude. A budget counts the same
kinds of step whatever runs them, and no value outgrows it.

Rejected:

- **Closing every node above a Galileo option (the 2.4 checker):** it
  rejects every foreign read, such as `config.services.kanata.enable`, and
  every read in a nested home-manager module.
- **Closing those nodes except the root:** it still catches
  `config.myConfig.modulez`, but it rejects every foreign option in a
  namespace that holds a Galileo one, such as `config.services.kanata.enable`
  once Galileo declares `services.foo.extra`, the case 2.4 asked about. The
  schema catches that typo instead.
- **Galileo's declarations replacing the schema under any node they reach:**
  the same rejection, with a schema present.
- **Checking entry paths only when a schema is declared, as 2.15 does:** a
  typo under `myConfig` is then emitted silently by every file without a
  schema, though Galileo's own declarations catch it.
- **A recursive `let`, as in Nix:** it would bring back the recursion SPEC
  §3.5 rules out, Beagle's lambda layout cannot print it, and no firn file
  needs it.
- **Rebinding a name in its own block, as Rust allows:** one `let … in`
  cannot hold it. Allowing it later breaks no program; forbidding it later
  would.
- **Nix's globals in the prelude:** every target's names would reach every
  program, and Nix's `map` would collide with Galileo's.
- **`builtins.` only, with no bare `import` or `toString`:**
  `builtins.import` emits other bytes than Beagle's `import`, which breaks
  byte identity (D-8).
- **A heavier unit, so that 1,000 fits:** a unit sized to a guess counts
  representation rather than work, and the two evaluators' costs would
  stop comparing.
- **Growing the tags program until it costs more than 1,000:** the row
  would then test the fixture, not the budget.
- **A budget in milliseconds:** the same program could Hold on one machine
  and be Unknown on a loaded one.
- **One unit for `++`, `//` or a copy, whatever its size:** 40 doublings
  then cost about 160 units and exhaust memory first, R12's failure.

### D-38. The Beagle layout's parentheses are printer rules over a semantic Nix AST

galileo-nix has no parenthesis node and no layout metadata. The `beagle`
layout prints every binary operation inside one pair of parentheses, and
parenthesizes a selection whose base is a selection, as Beagle's emitter
does (`emit-nix.bclj:1390–1398`, `:2165–2172`). With these two rules the
printer round-trips 49 of the 50 Beagle-generated files byte for byte.
`flake.nix` is parse-identical: its two one-operand
`(++ (builtins.map …))` calls (`flake.bnix:136`, `:304`) are Beagle
no-ops, and neither Nix nor Galileo can express their extra parentheses.
So A2 counts 49/50 plus `flake.nix` parse-identical, which box 4.5
already allows. This refines D-8 for `flake.nix`.

Rejected:

- **Keeping rnix's parentheses in the AST:** the round trip would pass
  with no parenthesis rules at all, and Galileo's lowering, which the
  migration depends on, could never supply them.
- **A rule for `builtins.map` with a lambda argument:** it fits the corpus
  by accident (`modules/beagle/default.nix:8` has none).
- **Removing the no-op from firn's source and re-pinning:** it changes the
  oracle to pass the check (D-32), and `flake.gal` drops it anyway.

### D-39. firn migrates through its committed Nix and one lift; parse identity is Nix's own, and north's flake is proven by evaluation

These rule on DECISION 4.6, 4.16 and 8.10, K11, and REVIEWED 2.17's flake
root. Counts were measured with nix-roundtrip as landed in `6b2cb58` and
with `nix-instantiate` 2.34.8, at `FIRN_ROOT` and `NORTH_ROOT`.

- **One importer, from the committed output.** Every `.bnix` and Clause
  module's committed `.nix` is what the system builds and what A3 compares,
  and it holds every form its source wrote: all 50 Beagle outputs and all
  184 Clause outputs parse into the galileo-nix AST, with no failure. So the
  migration lifts each committed output with `galileo import nix`, and no
  Beagle or Clause reader is written. `galileo import bnix` is cut;
  `galileo import clause` stays for north's relations and handlers (box 8.6,
  D-28). Module metadata comes from `bnix_tags` (box 5.1) through 4.8 and
  5.5, and the eight validator fixtures of box 4.21, six lines each, are
  written by hand. The source comments that never reached Nix, 81 lines in
  ten `.bnix` files and about 70 in 17 Clause files, are carried by hand
  (box 4.35).
- **The crate.** `crates/galileo-import` takes and returns data. `bnix_tags`
  stays as landed: text in, Galileo facts out, which is the shape the rest
  follows. `nix::parse(&str) -> Result<galileo_nix::Expr, String>` moves in
  from `tools/nix-roundtrip`, which then calls it (4.16a).
  `nix::lift(&galileo_nix::Expr) -> Result<RecordDecl, String>` returns the
  record-register declaration that lowers to the tree (4.16), and
  `galileo_syntax::record::print` is the one printer of that register (4.36).
  Spelling lives only in galileo-syntax, so T5's syntax audit changes the
  parser and the printer together and the lift not at all. Errors are one
  line that names the construct and its attribute path.
- **What the lift accepts: the image of the lowering.** A form lifts only
  when some Galileo form lowers to it, so an import builds back to its input:
  byte for byte in the printer's layout, which every Beagle output has, and
  under parse identity otherwise. The root is a `nix module` when it is a
  lambda over plain formals whose body, under applied one-parameter lambdas
  that become `let`s, is a set holding only `imports`, `config` (a set,
  under `lib.mkIf` for `config when`) and `options` leaves that §10.2's
  sugar writes; any other root is a `nix file` (box 2.17). Formal defaults,
  `@`, `rec`, `inherit`, `let … in`, `or` defaults, `assert`, `?`,
  negation, `/`, `->`, floats, URIs, `<…>` paths and a selection by a key
  that is not a name are named errors. Nothing is rewritten into another
  shape of Nix, such as a hand-written `let … in` into Galileo `let`s,
  because the import would then emit something other than its input. Box
  2.17 has landed, so 4.16 does not wait for it.
- **Parse identity is `nix-instantiate --parse` equality.** Both texts are
  read from standard input in one directory, so relative paths resolve
  alike, and the printed trees must match byte for byte. That already
  ignores quoting of static keys (`"config"`, `lib."mkIf"`), how attribute
  paths nest, the order of bindings and formals, parentheses, and plain
  against indented strings: every normalization 4.6 asked about. Nothing
  else is normalized, so the lift transcribes Clause's output as it is: the
  `__clause_argument_N` parameters of 18 modules, and the left-nested `+`
  text chains of 27 modules (322 `+`), written as `++` with Clause's
  parentheses. Renaming those parameters or writing the chains as
  interpolation is ordinary editing after the migration.
- **184 modules from 185 sources.** `native/nix/nixpkgs.clause` holds the
  foreign declarations that the 184 modules import and emits nothing. 183
  outputs hold `config` and `options`, and one adds `imports`. A3 and WP4's
  Done when count the 184 outputs.
- **What WP4 needs from the language owner, not ruled here.** K7: an
  `enable` option whose description is a `let` name, for capacity-watchdog,
  lane-gc, proton-log-watchdog, skill-review-queue and vast-reaper. Until it
  lands they import as `nix file`, byte-identical, and box 4.3b re-imports
  them. WP4 needs no explicit `lib.mkOption({…})` call: all 20 Beagle
  `lib.mkOption` calls write `type`, `default`, `description` in the
  sugar's order with table types, and Clause's 8 are compared by parse
  identity, where order does not count. Box 4.5 needs brace parameters
  with defaults (`flake.nix:56` and `:228`, 8 formals). For reading only: a
  multi-line body for a lambda that returns a record (Clause's 18 lambdas
  import as one-line records), a `nix module` whose entries sit at the root
  (the host files import as `nix file`, unchecked), and a local `let` in
  expressions (north's flake outputs).
- **A flake's top level is a literal set.** Nix reads `flake.nix` without
  evaluating its top level, which must be an attribute set, so no lowering
  of a root `let` works there. REVIEWED 2.17 measured it: `nix flake
  metadata` rejects the applied lambdas, and a hand-written set passes. So
  `galileo build` refuses a root `let` in a `nix file` that emits
  `flake.nix`, naming its line and asking for the binding inside `outputs`
  (box 2.17b), and every other `nix file` keeps L2's lowering. The meaning
  of `let` does not change, so the owner has nothing to confirm.
- **North's flake is proven by evaluation (8.10).** Its `description` and
  `inputs` must equal the pin's, and so must the drvPath of
  `devShells.x86_64-linux.default`, its only output. The three bindings in
  its `let` each name only earlier ones, which is when Nix's recursive
  `let` and Galileo's sequential lambdas agree (SPEC §10.2, D-37), and
  `inherit system` is `system = system` in a set that is not recursive.
  D-8 rejected the drvPath alone because a module no host evaluates is
  invisible to it; this check evaluates every output the flake has. Since
  box 2.4c, `import` is bound, so the source drops `with builtins:`.
- **K11.** D-20's rule stands, and its list was stale: rnix parses Nix in
  `galileo-import`, which `galileo import nix` ships, and in the dev tools.
  Like lsp-server in `galileo-lsp`, it replaces an external format; the core
  crates still take no dependency.

Rejected:

- **A `.bnix` reader and a Clause parser with its elaboration (4.6's
  `f244c20`, 903 lines):** Clause inlines its functions when it compiles,
  so the importer would reimplement a frozen language to reach the parse
  that its committed output already has.
- **Normalizing `+` chains, parameter names, `let` or `inherit` before
  comparing:** the migration proof would rest on a comparator of our own,
  where `--parse` is Nix's.
- **Parse identity after a `let` and `inherit` normalization for north's
  flake:** the same comparator, for one 25-line file whose whole output an
  evaluation covers.
- **The importer writing Galileo text itself (4.16's `ab83c76`):** a second
  printer beside galileo-syntax's parser would drift from it, and T5's audit
  is about to change the spelling.
- **Lifting a hand-written root `let … in` into Galileo `let`s:** the import
  would emit other Nix, though equivalent, and `beagle-import-nix` has no
  caller to serve (4.9).
- **A covering set of 20 Clause modules first:** one lift handles the
  corpus, so the test runs all 184 and lists failures by construct.
- **Moving a flake's root `let` into `outputs`:** `description` and
  `inputs` must be literals, so the move changes what they may read.
- **`-o OUTPUT` and directory arguments:** standard output and a shell loop
  serve box 4.13.

### D-40. One scope function, value-free counts, record rows, modes checked per stratum, and one expression evaluator

The language owner's first batch (D-36). It writes the scope rulings for
box 3.2 into SPEC (notebook L3 to L5, ruled at 19:05 and never persisted),
and rules on the open questions of boxes 2.9, 2.11, 3.2, 3.4, 3.5 and 3.7,
reconciling the reference-semantics and engine planners' files. Line
numbers are at main `f0be292`, whose code `b92dc07` keeps.

- **Box 3.2: one scope function.** A rule, a handler clause, a claim, a
  view and a definition each own their variables. Each aggregate, argmin
  and query opens a nested scope, and a name that also appears at the top
  level of its owner, or of any enclosing scope, is outside: shared, and
  grounded before the nested scope runs. Every other name is local, so
  sibling aggregates keep their own locals, and a declared variable may not
  reuse an outside name. Only shared variables give SPEC §9.7's empty count
  of 0, which `exactly one count per north` needs, and Clause, which north
  is imported from, keeps every name local unless `given` lists it
  (`north.clause:1168` and `:1171`, `:2677` and `:2679`). In a recursive
  rule, `?y = ?x` gives `?y` the safety of `?x`, and anything else computed
  from a variable that only the head's own component grounds is value
  invention, a membership in `[?x]` and a field read included, until a
  program needs one. A stratum is one strongly connected group of
  relations, numbered by longest path, so `r(f(?x)) when r(?y),
  static-input(?x)` is accepted.
- **Box 3.2: definitions and views are their own owners.** The grounding
  check covers every definition and view, used or not, with only its
  parameters grounded, which is how both interpreters already run them. At
  `f0be292`, `check` accepted a definition whose body is
  `all ?x where ?x = ?y and ?y = ?x`, and the reference then stopped with
  "?y is not bound". `ir::Definition` gains a source location for file:line
  (G6). Box 3.2b.
- **Box 2.9: `count` takes no value, and `sum`, `min` and `max` need one.**
  An aggregate ranges over the distinct bindings of its local variables,
  and `count` counts them, so a value could only mislead:
  `?n = count ?i where ?shop sells ?i for ?c` reads as the number of items
  but counts item-and-cost pairs, and 3.4's reference already ignored it.
  Soufflé draws the same line, `count : { … }` beside `sum z : { … }`
  (A-Souffle). All 19 aggregates in north's Clause are counts, written
  `sum 1.0 given … where { … }`, and firn's Clause has none. A count of
  distinct items names them with a query:
  `?n = count where ?i in all ?j where ?shop sells ?j for ?_`. SPEC §9.4's
  `[ expr ]` and §9.7's `count ?row` change. Box 2.9b.
- **Box 2.11: an aggregate's value belongs to its operation.**
  `AggregateOp` is `Count`, `Sum(Expr)`, `Min(Expr)` or `Max(Expr)`, with no
  separate value, so a count with a value or a sum without one cannot be
  built and a diagnostic prints `?n = count where …`, not `count null`. The
  Core IR is the reference semantics and the API a host embeds (T4), so it
  holds exactly the language's forms. Box 3.4b, which lands before 3.6 so
  the engine's aggregates are written once.
- **Box 3.4: `min`, `max` and argmin keys are `Int` or `Text`,** the types
  `<` orders, in its order. Anything else is a type error, which the
  interpreters guard as they guard `sum`, and box 2.5f checks before
  evaluation. When `<` orders more types, such as fractions (D-12), they
  follow. SPEC §4.5 now gives the canonical order of every value a row can
  hold, which ties and default query order use. Box 3.4b.
- **Box 3.4: a query that names several variables returns records named
  after them.** `all ?p, ?v where …` returns `{p, v}` rows, read as
  `?row.p`; one variable still gives plain values, which §9.6's
  `{?module}` and §9.7's `rows` need. Rows are distinct and come in the
  canonical order of their variables, compared in the order the query names
  them, so renaming a variable never reorders a result. Records already
  exist (§3.1); a positional row is not a `List T`, and Galileo has no
  indexing to take one apart. `galileo query --json` gives firn's TypeScript
  host objects and `bindgen` gives struct fields. A variable such a query
  names is observable, because it names a field. A `for` with several
  variables takes a query written in place and binds its variables in that
  order. Box 3.4c.
- **Box 3.4: a sort key reads only the query's variables and outside ones
  (notebook K1).** L3 stands, and the 3.4 review finding becomes a fix. A
  key read only by the condition gives one row several keys: in
  `all ?x where ?x costs ?a, sorted by ?a`, an item with two costs has two
  places, and the one the interpreter keeps is invisible in the source.
  PostgreSQL refuses the same query ("for SELECT DISTINCT, ORDER BY
  expressions must appear in select list"), and §9.7's `sorted by
  ?row.order` reads the named `?row`. An argmin's key may still read its
  condition, since it returns one binding. Box 3.4c.
- **Boxes 3.4 and 3.5: modes are checked when a relation is complete, so
  `the` never meets a broken mode (notebook K9).** `the ?x` returns its one
  binding or `null`. It type-checks only when a chain of modes fixes `?x`
  from constants and outside variables, and it is `maybe T` unless every
  mode in the chain is `exactly one` (box 2.5e, in the front end, where the
  same chain picks the type). A relation's modes are checked as soon as it
  is complete: stated facts when they load, which the reader does, state
  facts at each step (7.2), and a derived relation when its stratum is done
  (3.17), by one galileo-core function over the model so far that both
  evaluators call. Evaluation stops at the first stratum that fails, so no
  `the`, `not`, aggregate or query reads a relation whose mode Fails. Each
  conflicting key Fails with its two smallest tuples and a proof of each,
  each live key of an `exactly one` mode with no value Fails, and
  `galileo check` answers its claims `Unknown`, naming the conflict. A
  completed stratum is not the inside of a fixed point: both evaluators
  complete the same strata with the same tuples, so they report the same
  conflicts, and each tuple's proof lies in its own and lower strata. The
  interpreters keep `EvalError::Type` for two bindings, as a guard for an IR
  that skipped both checks. firn's tag overrides and flake inputs and
  north's menu count need the check, and no box built it.
- **Box 3.5: one expression evaluator and one meter.** Expressions, the
  reductions behind aggregates, argmin and queries, and the work meter are
  written once in galileo-core, and the reference interpreter, the engine,
  `why`, `why not` and every later consumer call them. The two evaluators
  keep apart only how they find bindings, which is what the differential
  tests compare (G7); expressions are checked against named references,
  SPEC's cases and Clause's recorded text functions (8.4). The engine's
  copy of the evaluator (`d5eb676`) drifted within twelve minutes: the 3.3
  review fix (`75fafa1`) changed the reference's charges only, and then one
  zero-budget program gave `Unknown(work)` on the reference and `Overflow`
  on the engine, unseen by generators that never produce text built-ins,
  records, `++`, `//`, `if` or calls. Box 3.6 moves the reductions, and box
  3.3b, which already owned the per-call unit (D-37), also moves the
  evaluator and the meter; the engine planner's box 3.16 is folded into it.
  D-37's work unit and its costs (tags 179, dungeon-static 687) stand.
- **Box 3.7: an `Unknown` never counts as agreement.** A generated program
  counts toward the 10,000 only when the reference reaches a model or an
  error, and the engine must then reach the same model or an error of the
  same kind. An `Unknown` on either side is reported apart (D-4). The two
  evaluators' costs differ by design, so one budget can stop one and not
  the other. Today `differential.rs` counts a paired `Unknown` as agreement
  and two errors of one kind as a disagreement; the fixed seed has no
  `Unknown`, so 3.7's result stands, and box 3.8 brings the comparison to
  this rule.

Rejected:

- **A value on all four aggregates, or one that `count` ignores (§9.4 and
  3.4 as written):** `sum where …` means nothing, and an ignored value
  misleads.
- **`count <expr>` counting distinct values of `<expr>`:** `count ?c` and
  `sum ?c` would range over different things. Adding it later changes no
  program; taking it out would (D-35).
- **`value: Option<Expr>`, or `null` for a count (2.11 as landed):** either
  can still hold a form the language lacks, and every consumer would have
  to reject it.
- **Canonical order for any `min`, `max` or argmin key (3.4 as landed):** it
  orders entities and booleans that `<` refuses, and a `min` over `nix`
  payloads breaks §3.2. **`Int` only:** `<` orders text.
- **Positional rows, a tuple type, or records for one variable too:** no
  rule can read a list apart, records already fit (G1), and §9.6 and §9.7
  need plain values. **Ordering rows by field name:** renaming `?p` to `?z`
  would reorder the output.
- **Each row at its smallest key (3.4 as landed):** a hidden choice; the
  largest would be as defensible. **A condition variable that a mode fixes
  from the named ones, now:** unambiguous, but no program needs it.
- **`Unknown` for a `the` with two bindings:** the answer is known and
  failing. **`Fails` from the `the`:** a broken mode fails whether or not a
  query reads it, and its proofs belong to the relation.
- **One mode check over the finished model (the engine planner's):** a
  `the` in a higher stratum would raise a type error before the conflict
  is reported with its proofs. **Checking inside a fixed point and stopping
  at the first conflict:** the conflict reported would depend on evaluation
  order.
- **Keeping the engine's copy so the differential tests also check
  expressions:** a copy carries its original's bugs, and every built-in and
  T4 extension would be written twice. **The reference importing the
  evaluator from `engine.rs`:** the oracle would import the module under
  test (R14). **Compiled expressions now:** no measured budget needs them.
- **Checking only the definitions some rule reaches:** an unused definition
  is still wrong. **A second grounding schedule in the front end:** one
  scope function serves the checker and both evaluators.
- **A paired `Unknown` as agreement:** `Unknown` never approves (D-4).

### D-41. The A4 path and the 2.12 review: firn's platform gate, entity relations, linked modules, vocabularies that never change a line, questions as arguments, clause and view scopes

The language owner's second batch, on the next win's path (5.1 to 5.3).
It rules DECISION 5.1, DECISION 5.2's two blockers, notebook K3, K4, K5
and O3, the open questions of boxes 2.10, 2.11, 2.12, 3.9, 3.11, 3.15 and
5.4, from the declarations, printer-lowering, IR-lowering, sentence-syntax
and explanation planners' files, and the three flaws REVIEWED 2.12
accepted at `51b31fa`, judged against T5. Line numbers are at
`FIRN_ROOT` and at main `b92dc07`.

- **Box 5.1: `supports` gates enabling, at the rule that already vetoes.**
  On a darwin host firn keeps only the 34 modules of
  `config/darwin-modules.json`, for default and `+` memberships alike
  (`allowed-module?`, `native/tag_resolve.bjs:121`), and only darwin
  restricts (`native/tag_inputs.bjs:798`). So SPEC §9.6's rules gave ashashi
  48 modules where firn gives 33, while whiterabbit matched (122). The
  vocabulary gains `{platform} allows every module` and the derived
  `{module} is supported on {host}`, by two rules: a module is supported
  on a host whose platform allows every module, or whose platform it
  supports. `enabled` requires it beside `not ?host disables ?module`.
  firn's policy becomes facts: `linux allows every module`, and one
  `X supports darwin` per allowlist entry. The enabled set and the
  overrides equal firn's filtered result, since filtering by module and
  host commutes with the union of memberships, while the membership stays
  visible, so `why not bat is enabled on ashashi` names the missing
  support where firn drops the module without a word (G8). The old second
  requirement, every enabled module supports its host's platform, would
  have failed on every linux module, and the rules now guarantee it; its
  replacement catches the one case the drop hides, a host that adds a
  module its platform does not support. A positive gate reads as a reason
  in the proof (T5's visible structure), where an `unsupported` relation
  under `not` would read as an absence.
- **Box 5.1: one name may name entities of several types.** An entity is
  its type and its name, and a fact introduces each entity it names in a
  typed slot, as SPEC §9.6's example always did (`firefox is tagged
  browsers` declares neither). So firn's `python`, `racket`, `doom-emacs`
  and `printing`, each a module and a tag, are fine: the slot's type picks
  the entity, and only a position with no type makes such a name an error
  that lists both.
- **K3: `X is a T` is the sentence of a built-in relation.** Each entity
  type `T` has the shape `{entity: T} is a T`, named `is-a-T` and exported
  with `T`, the template hover already shows (`afd89d1`). Its facts are the
  declarations and the entities facts introduce, each cited at its line;
  for a `state entity` it is a state relation of the live entities, which
  `create` includes and `delete` withdraws. In a rule it is a positive
  sentence, so `?north is a North` grounds `?north`, `not ?x is a T` reads,
  and the evaluators, static checks and `why` need nothing new. Types
  become relations, T5's one uniform term. Measured at `1961391`: the
  landed type test read `?x is a Dog` as the author's `{x} is a {kind}`
  template until `use b` brought `type Dog`, and then as the test, a
  re-reading G5 forbids; and the 2.12 lowering, `entities("Hero")`, made
  the reference answer `Unknown(unsupported)` for every rule that used it
  (DECISION 2.12 review). Firn's `supported-everywhere` rule needs it,
  since no module is declared.
- **O3, box 5.2: a `for` block is one more enclosing scope.** A query
  inside it reads the `for`'s variables as outside variables, already
  grounded (L3), as §9.6's inner `for` reads the outer `?module`, which is
  how 5.2 built whiterabbit's file (`bd5c704`).
- **Boxes 2.11, 3.11 and 5.4, and DECISION 5.2: modules are linked, never
  concatenated.** Each file is one module, read against its own vocabulary,
  and lowering links them into one Core IR, where everything a module
  declares is `module/name` (`firn/tags/is-tagged`), so adding a module
  renames nothing that review, checkpoints or bindings key on. A shape is
  one relation, lowered by its own module, and a relation keeps every mode
  line, each checked on its own. A call resolves to a definition of its
  module or of a module it uses, and two in scope are an error. A module is
  named by its path from the project root, the nearest ancestor of the
  working directory holding `.git` or else the working directory, so a
  name never depends on where `galileo` runs. A command reads the modules
  under its paths, the whole project by default, and every module they
  `use`. `use firn/<name>` names Galileo's library under `lib/firn/`, which
  the binary carries and which joins a program only through `use`. So
  `galileo check` reads source only, its fixture names go, and `lower_files`,
  which read several files as one module, goes. Measured at `f2802f2`: an
  unrelated file declaring the same `{hero} is in {room}` broke
  `galileo check` (R16), any `use` failed with "there is no module", and
  5.2's second file could not `use firn/tags`.
- **K4, box 2.10: a `use` brings vocabulary and never changes a line.** A
  module's scope is its own declarations, the exports of the modules it
  uses (not transitively) and the prelude. It exports types, shapes and
  definitions, never entity declarations, facts, rules, handlers,
  requirements, views or Nix outputs, which belong to the program. A name
  means the module's own declaration, else the one export or prelude name
  that has it, and two are an error naming both in either `use` order; a
  public declaration may not mention a private type. This is T5's guiding
  star: a module carries its own sentence vocabulary with exact meanings,
  and importing it can make an existing line an error but never re-read it
  (G5, R16). Measured at `b92dc07`: prepending `use story` re-pointed
  `Ash is in Hall` to `story`, appending `use b` re-pointed `X runs` through
  an exported entity declaration, and the same export broke
  `whiterabbit enables racket` once a used module declared
  `racket is a Module`. The MVP has no qualified names.
- **Box 2.10: a question is a command's argument, not a declaration.**
  `galileo why`, `why not` and `trace` take a ground sentence, `prevent` and
  `achieve` a claim, and `query` a query, each read as a line of a module
  that uses every module. `via`, `at revision` and `using:` become the
  flags `--via`, `--revision` and `--using`, so no question keyword is read
  inside a sentence. No command answered a question written in a file;
  box 2.10's top-level questions cost `why`, `trace`, `prevent` and
  `achieve` the right to start a fact, and the first-`via` split failed on
  firn's `via` module. §5.6's Silver-Key row needs a claim:
  `achieve possible eventually Ash is in Shrine --using lies-in`.
- **Box 3.15: `achieve acceptable` minimizes among acceptable sets.** It
  finds the minimal addition sets after which the claim and every
  requirement Hold, so a larger acceptable set is found when every smaller
  one breaks a requirement. Filtering `possible`'s minimal sets would
  answer "none" while an acceptable intervention exists.
- **Box 3.9: an absence leaf names where its sentence would come from.** A
  static relation's leaf names the files that state its facts about the
  same first argument, else every stated fact; a derived relation's says
  "nothing derives" and names its rules; a state relation's names the
  state. Facts are filed by subject (§9.6), so those files hold every
  candidate, while 3.9's every-file scope listed ashashi's file for a
  whiterabbit leaf and would list about 206 files for `is tagged`.
- **REVIEWED 2.12: a clause is its own scope, a view reads its slot as
  `?name`, and `requires` takes one sentence a side.**
  - Clauses of one event were refused unless they spelled their binders
    alike, though §5.2 groups them and §9.4 asks nothing of the names. A
    clause is its own scope, as a rule is (L3), so each `on` line names
    each argument with its own distinct variable, two clauses may choose
    different names, and lowering maps each clause's names onto the
    event's arguments. Constants, repeats, `?_` and expressions stay
    errors on an `on` line, and values are tested under `when`, as all 150
    of north's `on` lines and SPEC's six already do.
  - A view's `for {north: North}` is a slot, read as `?north` in its body
    as a handler reads its event's slots, so in a sentence a variable is
    always `?name` and a bare name always an entity (K5). A `define`'s
    parameters are names, read bare like `let` and lambda parameters, and
    a `?` variable may not reuse one. The review's bare Text parameter
    `needle` was read as a name and lowered to `null` after its
    diagnostic, and §9.7's bare `north` would break once a used module
    declared an entity `north` (G5).
  - `S requires C` and `… among P` take one sentence for `S`, `C` and `P`.
    "S requires A and B" reads, in English and in logic, as two necessary
    conditions, so it is two requirements, and the conjunction is an error
    that shows them. Lowering kept a dummy atom in its place.

Rejected:

- **Gating each membership rule, as firn filters:** the same enabled set,
  but `why not` would lose the membership that a gate at `enabled` keeps.
  **An `unsupported` relation under `not`:** the proof reads an absence
  where a reason exists. **A platform that restricts once any module
  supports it:** the first `supports linux` fact would silently shrink
  every linux host.
- **Importer-written drops for the filtered modules:** the import would
  differ from its source and lose the reason.
- **An IR literal over `Program.entities`, or an `entities(T)` call:** a new
  literal kind in both evaluators, three checks, claims and `why`, each a
  place to disagree (G7), and its proof leaf is no stated fact. **A type
  test decided before templates:** an import re-reads the line.
- **Several files read as one module (`lower_files`):** an unrelated file
  breaks a program (R16), `use` cannot resolve and `private` means nothing.
  **Bare IR names, or names qualified only on a clash:** linked modules
  collide, or a new module renames an existing relation. **Module names
  from the working directory:** the same program gets another identity in
  a subdirectory. **Library modules in every program:** firn's requirements
  would reach the dungeon.
- **The first `use` keeping a clashing name (2.10), or the last:** a
  reordered or appended `use` re-reads a line. **Exporting entity
  declarations:** the re-pointing and the firn break above. **Transitive
  `use`:** an edit to a used module's own uses would change its importers.
- **Questions as top-level declarations, or a `questions` block:** no
  command answers them. **Positional `via` and `at revision` inside the
  sentence:** a special reading rule for what a flag says plainly (T5).
- **`acceptable` as a filter on `possible`'s minimal sets:** it misses the
  answer it exists to find.
- **Every file that states the relation as a leaf's scope (3.9):** other
  subjects' files, hundreds for firn.
- **One spelling of an event's binders across its clauses (the IR-lowering
  ruling's Q2):** a naming rule no other scope has, and renaming one
  clause's binder, which changes no meaning, broke the program. **Bare
  view parameters, with an error on a clash:** an import could break an
  existing view (G5). **`requires` distributing over a conjunction:** a
  second spelling of two requirements, whose shared pattern variables would
  silently come apart.

### D-42. The T5 audit: earned syntax, types as contracts, and steps stated as relations between before and after

The language owner's third batch: the earned-syntax audit of SPEC §9.4 and
every §9 keyword against T5, which WP4's importers (4.1, 4.6, 4.16, 8.6)
waited on, and notebook K7, K10 and O1. Counts are at main `70f79f7`,
`FIRN_ROOT` and `NORTH_ROOT`; §9.4 is counted as `help_syntax.rs` counts it.

**Measured.** Production names 48 → 46, word terminals 89 → 72, symbol
terminals 25 → 25. Of the 214 comment lines in firn's sources (138 in 15
`.bnix` files, 76 in 9 Clause files, counted outside text blocks), a Galileo
rendering makes 71 redundant (41 and 30): the generated-file markers and
"do not edit" headers that a query in place replaces (16), the tag
vocabulary's prose that its facts, rules and requirements state (18), 7
lines that name what a value or module is, which a relation, an option or a
responsibility fact names, and the 30 lines of `machine-capacity.clause` that restate nested `if` chains
which Galileo writes as one named rule per deferral reason. The other 143 say
why (hardware forensics, ownership, measurements, decisions): no construct
can say them, and none should try. Galileo's 7 programs have no comment.

- **Principle 15: a handler clause is one transition, stated as a relation
  between before and after.** `when` says what holds before and `then` what
  holds after, as a rule relates its body to its head (Dedalus's `p@next`).
  The six action verbs go: a state sentence holds afterwards, `not S`
  withdraws, `?x is a new T {…}` creates, `not ?x is a T` deletes (D-41's
  entity relation), and an effect sentence is a request. A sentence of a
  moded relation replaces the value at its key, because persistence stops
  where the step contradicts a fact under the mode: that is what `set` did,
  and `then ?hero is in ?there` now reads as the post-state it makes. The IR
  keeps its six actions, so box 7.1 proceeds as staffed and 2.19d only
  parses. Source order becomes unobservable: `E` is a set delivered in
  canonical value order (two equal requests are one), and creation ids follow
  the canonical order of the entities' types and field values, then their
  clauses' normalized text and substitutions. DECISION 2.12's "clauses in
  source order" is a lowering detail, and the IR-lowering ruling's "every
  clause of one event in one module" needs no rule. Verdicts on the other
  imperative-looking constructs, all kept: a `for` block is a comprehension
  with no state; a sequential `let` is nested immutable bindings, each bound
  once; `run [e1 … en]` is the relational composition of n steps, its order
  being input data; step 5's "check, in order" only picks which failure is
  reported; stopping at the first failing stratum is how definedness is
  evaluated; `initially` is the initial state as facts; `may skip` puts the
  identity step in the relation when nothing matches. Clauses and the lines
  of a `then` have no order.
- **Principle 8: four words renamed, scored by how easily they are
  misread.** `static` becomes `given`: `static` has five programming meanings
  (C linkage, Java members, typing, sites, SPEC's own "static error"), and
  it differs from `state` by two letters in a column that alternates them
  (§5.6). `checkpoint` becomes `milestone`, the word §4.6 already used,
  because a checkpoint is also a saved runtime state (§10.4). `first ?x by`
  becomes `min ?x by` and `max ?x by`: Python's `min(xs, key=…)`, Kotlin's
  `minBy` and Rust's `min_by_key` return the element, while `first` is
  also the list function. `every trace to S passes C` becomes `S only after
  C`, since "passes" reads as a test verdict, and `every E happens when C`
  becomes `E only when C`, since the old form reads as a trigger, "whenever
  C, E happens". The rest score low and stay: `given`, `state`, `derived`,
  `event` and `effect`; logic's `not`, `and`, `or`, `in`, `some`, `every`,
  `no` and `the` (Russell's definite description); LTL's `eventually`,
  `always` and `never`; `exactly one … per`, `at most one`, `count`, `sum`,
  `min`, `max`, `all … where` (set-builder) and `sorted by`; law's
  `settled by`; `invariant`, `ensures` (Dafny's postcondition), `requires`,
  `has no single point of failure among`, `initially`, `may skip`, `on`,
  `when` (a condition in all four places), `then`, `new`, `rule`, `define`,
  `view`, `use`, `type`, `maybe`; and the Nix target's words, each of which
  a byte-identical file needs.
- **Principle 14: types are contracts.** A slot's type states that every
  value of the slot is a T, which since D-41 is the relation `?x is a T`; a
  mode states a functional dependency; a requirement states what
  evaluation must find. One kind of statement, checked at three times, with
  no hierarchy and no subtyping (SPEC §3.3). What merges: the option-type
  grammar into the value types, cutting ten words (`bool`, `int`, `text`,
  `lines`, `path`, `port`, `package`, `list of`, `attrs of`, `one of`), so an
  agent never meets `Text` beside `text`; `Port` stays as the Nix target's
  one contract, an `Int` from 0 to 65535; the table shrinks to the seven
  types firn's 28 `lib.mkOption` calls use (G1). `static entity`, a second
  spelling of `type` that no program gives fields, goes. What does not
  merge: slot types stay written, because a slot's type picks the entity a
  name means and the template a line matches before any model exists (G5),
  so a requirement cannot stand in for one; modes stay shape lines, which
  `the` typing and the replacing sentence read. The Nix lowering needs types
  for §10.2's table, for `++` against `+` (2.6) and for the `nix T` stage,
  and the one grammar serves all three. D-35 rejected one grammar for
  option and value types because NixOS merges by type; the merge keeps one
  Galileo type per NixOS type, so each emits exactly what it did.
- **K7: `enable` takes an expression.** `option P: enable e` takes any
  `Text` expression and emits `lib.mkEnableOption e`, a `let` name as that
  name, which capacity-watchdog, lane-gc, proton-log-watchdog,
  skill-review-queue and vast-reaper need (2.19e; 4.3b follows). §10.2's
  explicit `lib.mkOption({…})` call goes: every firn call writes the sugar's
  order (D-39).
- **Brace parameters with defaults, for 4.5.** A formal is `name = expr`,
  emitted as Nix's `name ? expr`, as Python writes a default and JavaScript
  a destructuring default (2.19f).
- **O1 and K10: keywords never shadow templates.** A line is read every way
  the grammar's keywords and the templates in scope split it, and exactly
  one complete typed reading must remain (G5). So templates may use any
  word, `and` and `requires` included, and `backup requires capability gpu`
  reads as firn's template inside a `requirements` block (2.19g). No
  declaration-time check is needed, since a misreading cannot happen; a
  line that two readings type-check is an error showing both.
- **`body` goes,** an alias of `conj`.
- **Principle 16: what Galileo takes from Clause and Beagle.** It has:
  Clause's deletion test ("if we remove it, what meaning becomes
  inexpressible or ambiguous?"), which this audit applied; structural
  participation with no registry (D-41's entities named by slots); role
  contracts with cardinality (modes); one atomic change from one pre-state;
  exhaustion is not absence (`Unknown`); domain readings (templates and
  `use`); focus blocks; and Beagle store's separation of a term from its
  assertion and its occurrence, with logical order intrinsic and wall-clock
  time as data (events, revisions, no clock). It lacks, until a program
  needs them: Clause's `some` cardinality (at least one), checked live
  edits, hypothetical queries beyond `achieve` and review, and Store's
  assertion multiplicity. It rejects Clause's drift: the formation,
  judgment and authorization taxonomy and the Lean-checked kernel (T5's
  principle 11), and `##` documentation comments (principle 10).
- **DECISION 4.11 confirmed.** `galileo query PATH... QUESTION --json` takes
  source paths until program images exist (§10.4), and the argv walk
  (aliases, optional-leaf ambiguity, diagnostics) stays in
  `native/firn_host.mjs`, which T1 puts in TypeScript, until Galileo can walk
  a list left to right with state, which recursion (SPEC §12.1) brings.
  Galileo holds the command table, leaf requiredness and each default plan.

Rejected:

- **Keeping the six verbs and only documenting their declarative reading:**
  each reads as a command, `set` as an assignment, and a clause would stay
  the one construct that does not read like a rule (principle 15).
- **Pure Dedalus `then`, where a moded sentence never replaces:** every
  move would need `not ?hero is in ?here` beside `?hero is in ?there`, and
  forgetting it rejects the step; agents write the replacement first.
- **`then not X` beside `then X` as a conflict:** Clause's imported
  withdraw-and-include of an unchanged value, such as a selection that
  wraps to itself, would start rejecting.
- **`E` in clause order:** source order would become observable, and moving
  a clause would change what the host does. **Creation ids in clause
  order:** the same.
- **`depends on` or `needs` for `requires`:** firn's own vocabulary already
  declares `{responsibility} depends on {dependency}` and `{module} needs
  flake input …`; O1's reading rule, not a rarer word, is what stops a
  collision. **`is necessary for`:** it reverses the sides that claims and
  review already name.
- **`fixed` for `static`:** SPEC already says a review fixes a failure and
  modes fix a variable. **No kind word for given shapes:** `{door: Door}
  is unlocked` would read as a fact whose first slot is a record.
  **`entity Hero` for `type Hero`:** it reads as declaring one entity.
- **`argmin`:** jargon that no English sentence uses, where `min … by` has
  the priors.
- **Requirements in place of slot types:** a line could not be read before
  its model exists (G5). **Inline text unions in types now:** no option or
  slot needs one (G1).
- **A `because` annotation for rationale comments:** untrusted text like
  `says:`, and a comment already carries it.
- **Inferring `state entity` from a `new` somewhere:** adding a handler
  would make an existing fact an error elsewhere.

### D-43. A replacing sentence names one key; handler clauses under D-42

The language owner's fourth batch opens with the declarations planner's
Q5, checked against D-42 before box 2.19d starts. D-42 let a `then`
sentence of a moded relation replace "the value at its key", and box 7.1
read that as withdrawing what the sentence contradicts under every mode.
On a relation with two modes that silently drops another key's fact.
Counts are at main `d7df3ff`, `FIRN_ROOT` and `NORTH_ROOT`.

- **A sentence replaces along a relation's one mode, and only then.** With
  exactly one mode the line names the key it replaces, as `then ?hero is
  in ?there` names the hero. With two, such as a board's `exactly one tile
  per unit` and `at most one unit per tile`, `then ?u is on ?t` would also
  withdraw the fact of whatever unit stood on `?t`, a fact the line never
  mentions, turning the rejection Clause gives into a silent eviction. So a
  sentence of a relation with two or more modes adds its fact and step 5
  checks every mode: a move writes `not ?u is on ?here` beside it, and a
  move onto an occupied tile rejects with both facts. PostgreSQL makes the
  same cut: `ON CONFLICT (key) DO UPDATE` names one key and rejects a
  conflict on any other, while SQLite's and MySQL's `REPLACE`, which
  deletes every row that conflicts on any key, is the known trap (SPEC
  §5.2, §5.3, A-Upsert). No program has a state relation with two modes
  yet: north names no relation in two of its 152 `mode` lines, and SPEC,
  `lib/` and the tests have none. 2.19d lowers such a sentence to
  `Include`, and 7.1's `Set` withdraws under the relation's one mode.
- **Clause variables, as the planner ruled, in D-42's words.** A `then`
  line reads only variables that the `on` line binds, that a `when`
  literal grounds (a sentence, `?x = e`, `?x in e`, an aggregate, `min` or
  `max … by`, `?x is a T`) or that a `new` line of the clause binds, in any
  order, since lines have none. `?x is a new T` takes a variable nothing
  else in the clause binds and a `state entity`; `not ?x is a T` takes one
  that the `on` line or `when` binds, never one the clause creates
  (§5.3's static error).
- **`then` has at least one line.** `may skip` admits the empty step when
  no clause matches, so it never licensed an empty clause: on one the
  `when` was dead. All 150 north handlers write something.
- **`may skip` belongs to the event.** Every clause of an event says it or
  none does, and a mismatch is an error naming both clauses, so removing
  an imported event's markers removes them together under one replay check
  (§5.3). north's `command-goal` has 7 clauses, and OR-ing the marker, as
  `lower.rs` does today, would let one clause's marker cover another's
  absence. Box 2.19h carries these three with the variable rule.

Rejected:

- **Replacing along every mode (D-42 as 7.1 read it):** the silent drop
  above, SQLite's `REPLACE`.
- **A static error for any `then` sentence of a two-mode relation:** it
  would leave such a relation no way to gain a fact, and games need one
  (T4).
- **Replacing along the `exactly one` mode and checking the `at most one`:**
  a word in the shape's mode line would silently change what every `then`
  line means, and two `at most one` modes would still need a rule.
- **Replacing only when one pre-state fact is contradicted:** a unit
  created onto an occupied tile would still evict the unit there.
- **A clause naming its key (`then … replacing ?u`):** a new keyword no
  corpus program needs (G1); a two-mode relation writes its `not`.

### D-44. Precedence claims bind their left side once, a default never names a formal, and why-comments as facts

Three questions from boxes in flight (DECISION 2.19c twice, DECISION
2.19f) and Tom's proxy's request to classify the comment lines D-42 left
as "why" prose (T5 principles 9 and 10). Read at main `67314dd` and
`FIRN_ROOT`.

- **`S only after C` and `E only when C` bind their left variables once,
  around the whole claim.** `?hero is in Vault only after ?hero is in
  Library` means: for every hero, every path to a state where that hero is
  in the Vault visits one where that hero is in the Library. The IR closed
  each side as its own `Some`, which loses the shared `?hero`, so the
  trace claim gains its variables (`EveryTracePasses { variables, target,
  checkpoint }`) and the checkpoint closes only its own other variables.
  The event claim already keeps its variables in the event atom, and its
  condition reads them free. Evaluating both is 7.5's, which renames to
  the new forms and needs 2.19c: `core/claims.rs` never evaluated the old
  forms either, so 2.19c's Done when inspects the lowered claims and
  expects `Unknown`, not §5.6's verdicts (SPEC §6.2's table).
- **A default may not mention a name spelled like a formal of its
  lambda,** even one an outer binding also has. In
  `let x = "outer"; ({x, y = x}) => y`, Galileo's rule reads the outer `x`
  and Nix's `y ? x` reads the formal, so the lowering would change the
  meaning; the error names `x`. The worker's narrowest reading, confirmed:
  no renaming or capture analysis (SPEC §9.4, box 2.19f).
- **Why-comments: 75 can be facts today, 8 need an addition, 60 stay
  prose.** Of the 143 lines, ownership, provenance, decisions,
  measurements, defect status and command usage (75) are `given` facts
  under a library vocabulary, with a requirement where they constrain and
  the value generated from the facts where they justify one: firn and
  north owning disjoint paths under `at most one owner per path`
  (`codex/default.bnix:41`), a mitigation whose defect is marked fixed
  failing the check (`boot.clause:21–25`), `cores * 3 == 24`
  (`nix-settings.clause:12–14`). Eight state the purpose of one literal
  setting no fact generates; *settings as entities*, a fact naming a
  module's attrpath that `galileo check` fails when no module sets it, would
  hold them, and is not earned by eight lines (notebook O6). The 60 left
  are causal mechanism and rejected alternatives, which no construct
  should state. This needs no language change: the facts wait for the
  lift's idiomatic pass (T2), since A3 compares emitted bytes.

Rejected:

- **2.19c evaluating the precedence forms:** a syntax box would build
  7.4's search and 7.5's claims; the IR shape is what 2.19c owns.
- **Closing the trace claim's target as `Some`:** "some hero reaches the
  Vault on a path through some hero's Library visit" is weaker than SPEC's
  claim and Holds where a second hero sneaks in.
- **Renaming the formal or the outer name in the emitted Nix (2.19f):**
  capture machinery for a case no corpus lambda has; firn's two defaulted
  lambdas read only literals.
- **Decimals, a `Date` contract or version contracts for comments:** an
  `Int` in a smaller unit states a measurement, provenance only quotes a
  date, and NixOS `assertions` already check a version at evaluation.

### D-45. The Nix stage left open by boxes 2.5 and 2.6

The stages planner's six questions, judged against T5. Counts are over the
234 modules firn generates at `FIRN_ROOT` (50 Beagle, 184 Clause), probed
at `f0be292` against scratch copies of `tests/external/kanata`. Each
ruling below is the planner's; the owner checked it against D-42's
contracts and adds only `Any`'s reading.

- **Signatures are `define`s without a body, only in `firn/nix-lib`.**
  `define builtins.readFile(file: Path): Text` is a contract on a host
  function (principle 14), with no new keyword, as TypeScript's `declare
  function` and Gleam's externals declare one. The name is rooted at `lib`
  or `builtins`, or is a bare global the Nix target binds (`import`,
  `toString`), never a prelude name, so a signature never writes `nix`. A
  `Path` parameter also takes `Text`, as Nix reads a string as a path
  (`modules/styling` passes an interpolated string to `readFile`).
  Anywhere else a body-less `define` is a forgotten body: today `define
  wrap(i: Int): Int` alone in a file builds. `Any` is the contract that
  states nothing, written only of Nix values.
- **`firn/nix-lib` comes with the Nix target.** The checker embeds it, and
  every `nix module` and `nix file` sees it without a `use`, as it sees
  option declarations and as TypeScript ships `lib.d.ts`; the CLI, the LSP
  and the tests stop passing it.
- **A call into `lib` or `builtins` needs a signature,** else an error with
  a did-you-mean: today `lib.mkForse(false)` builds and emits
  `lib.mkForse false`. firn calls 20 such functions in 36 files, and the
  import boxes add their signatures; a selected member such as
  `lib.licenses.mit` stays unchecked, and `pkgs` keeps its index warning.
- **A fact's Nix roots are SPEC's module arguments and the Nix target's
  names.** A `nix T` payload is checked over exactly those; any other slot
  gives the stage error; `flakeRoot` is a root only where an `arguments:`
  lists it (G5). A dotted name fills only an `OptionPath` slot: today
  `flakeRoot.x is tagged browsers` makes an entity named `flakeRoot.x`.
- **`!= null` narrows, and a `for` source is known.** Values never change
  and `lib.mkIf` evaluates its value only when the condition holds, so
  narrowing in `if`, `when` and `and` is sound in both stages; with no
  `match` in the MVP it is the only way to use a `maybe T` as a `T`
  (styling's `builtins.match`). A `for` unrolls when Galileo builds, so a
  Nix-stage source is a stage error.
- **K8: ordering takes two `Int`s or two `Text`s,** as §4.3 and both
  evaluators already do; Nix's byte order on UTF-8 is the same. Comparing
  a never-null value with `null` stays an error.

Boxes 2.5b, 2.5c and 2.5d carry these, in parallel; 4.2 and 4.6 add the
signatures their files call.

Rejected:

- **A `foreign` signature form, or `nix` in signatures:** a second keyword
  or a second spelling for one contract (D-35).
- **An explicit `use firn/nix-lib`:** 36 files would carry a line that
  tells the reader nothing. **A nix-lib inside nixos-config:** a program
  could silence the check by declaring the misspelt name (D-18).
- **Unchecked `Nix Any` for unsigned calls, or a warning as for `pkgs`:**
  `lib` and `builtins` are a small API Galileo curates, so a missing
  signature is Galileo's to fix, not version skew.
- **Every module argument in the program as a fact root:** one file's
  `arguments:` would change how another file's fact reads (R16).
- **Narrowing through `or`, `not` or a `let`:** no program needs it, and
  adding it later rejects nothing. **Lowering a `for` over a Nix list:** a
  second lowering of one construct.
- **`Int`-only ordering (2.5's reading):** `a < b` on text would hold in a
  rule and fail in a module, against §4.3.

### D-45 addendum. The box that makes a check strict adds the signatures its landed gates call

A one-shot ruling on DECISION 2.5b, 2026-10-11. Lane `nix-signatures-2-5b`
(`09ba1a2`, replayed as `690e1be` on `3606747`) rejects the imported
`modules/beagle/default.nix` and `flake.nix`, so `firn_import` fails. D-45
had 4.2 add the signatures its files call, but 4.2 landed while an unsigned
call still built, so nothing measured that clause. Probed with the CLI at
`690e1be` over the 235 committed `.nix` files at `FIRN_ROOT`: 5 files call
13 members with no signature. Two of those files are in a landed gate, and
the 184 Clause outputs (4.6) call none.

- **2.5b declares the six members 4.2's files call, in
  `lib/firn/nix-lib.gal`:** `builtins.attrNames(set: Attrs Any): List
  Text`, `builtins.fromJSON(text: Text): Any`, `builtins.map(function: Any,
  list: List Any): List Any`, `builtins.readDir(path: Path): Attrs Text`,
  `builtins.throw(message: Text): Any` and `builtins.tryEval(value: Any):
  Attrs Any`. Each is the narrowest contract §9.4 can write that the corpus
  meets: there is no function type or generic type, so `map`'s function is
  `Any` and its lists `List Any`, and `tryEval`'s `{success, value}` reads as
  `Attrs Any`. Measured with the pre-2.5b CLI (`83b94ff`): with the six
  declared, both files build with 0 diagnostics and unchanged bytes.
  Mis-declaring `map`, `fromJSON` and `readDir` gives 4 diagnostics.
- **`lib/firn/nix-lib.gal` is the catalog's one home** (SPEC §3.2, D-45,
  4.26a).
- **The box that makes a check strict adds what its landed gates call; a
  later box adds what its own files call.** 4.3, the first to build the
  remaining three files, declares the other seven: `builtins.getAttr`
  (`modules/containers/devshell-flake.nix`), `lib.concatStringsSep`
  (`modules/glide/glide.nix`), and `builtins.head`, `builtins.match`,
  `lib.findFirst`, `lib.hasPrefix` and `lib.splitString`
  (`modules/styling/default.nix`).

Rejected:

- **An unsigned call as `Nix Any`, a warning or an exempt list:** D-45
  rejected the first two, and an exempt list is a second catalog with no
  contracts.
- **Dropping beagle or `flake.nix` from the gate, or a test-only signature
  file:** the gate is A2's evidence, and a test that passes signatures in is
  the caller injection 2.5b removes.
- **A Rust table in `check.rs`:** a second spelling of a signature (D-35).
- **All 13 now:** seven would be declared with no landed gate building a
  call.
- **A text union for `readDir` or a record type for `tryEval`:** syntax
  §9.4 lacks, for values no program decides on (§3.2).

### D-46. Slot tokens, list slots by declared type, and literal kinds as readings

The sentence-syntax planner's Q1, Q2 and Q4 to Q6 (Q3 and Q7 became
D-41's entity relations), notebook K6 and DECISION 4.11's list slot,
judged against T5 and D-42's reading rule. Probed at main `1961391`.

- **A slot token is one postfix expression, parsed when the line is
  read.** A variable, name, literal, bracketed list or record, or
  parenthesized expression, then `.name` and `(…)` suffixes, without
  spaces; an expression with an operator is parenthesized. Today `has
  (1 +)`, `has 1.5`, `Ash visits +` and `Ash visits Hall+Cellar` all read
  as raw text, and a `nix` payload is never parsed (box 2.8b).
- **K6: the slot's declared type decides what a list means.** In a `nix T`
  or `List T` slot a bracketed list is one value; elsewhere in a fact it is
  one fact per element, one level deep, and `[]` is an error. §3.2's own
  `[pkgs.firefox]` payload could not be stated, and a list-valued override
  broke `at most one value per host, path`. Only facts take the sugar: in
  a rule body a list could mean "all" or "any", and no program needs
  either. Since D-42 a slot takes §9.4's one type grammar, so `List Text`
  is a slot type, and DECISION 4.11's plans leave `nix Any` for it (boxes
  2.8c, 4.11b): a contract that says what the slot holds, where `nix Any`
  said nothing (principle 14).
- **Literal kinds are readings, not a lexical precedence.** The planner
  ordered binding, membership, `not`, symbol and expression before any
  template and reserved `and`, a leading `not` and a leading `{x} in` in
  templates. D-42 already rules that every split is read and exactly one
  typed reading must remain, so the kinds join the readings and the
  template restrictions go; what stays is the planner's point that no
  word's presence in some template decides a line's kind, which the landed
  `is_sentence` did, so an import could move an expression into the
  sentence path. With no reading, a line with a non-keyword bare word gets
  the closest shapes, not "unexpected `enabls`" (box 2.9c).
- **A `when` line holds one literal.** A depth-0 `and` outside a `where`
  and outside a template reading would separate two, and is an error
  asking for one per line or `?x = (?a and ?b)`: today `?x = ?a and ?b`
  binds a conjunction on a rule line and splits in a query, one text with
  two meanings. An aggregate's `where` still takes the rest of its line,
  so north's nine-literal aggregates import as one line.
- **`not` before an expression is boolean negation** (`not contains(…)`),
  and before a sentence negation over `M(S)`.

Rejected:

- **Always expanding a list (2.8 as landed), or a second spelling such as
  `([pkgs.firefox])`:** the declared type already decides.
- **Templates that may not use `and`, `not` first or `{x} in`:** D-42's
  reading rule makes each collision one error at the line that meets it,
  and §5.4's `Codex finished {turn} with {status}` shows keyword-like
  words belong in templates.
- **Checking slot syntax later, in the checker:** lowering and the checker
  each re-parse, a payload is never checked, and the error comes after the
  line was accepted. **Letting an expression's type choose a template:**
  the reader would need signatures that `use` brings, so an import could
  move a line (G5).

### D-47. Fields fixed at creation, typed views, claims that bind in one place, sufficient supports, L1, and the intervention and recursion boxes

The rest of batch (c)'s planner questions: IR-lowering Q1 and Q3,
printer-lowering Q4's lowering, claims Q1 to Q5, explanation Q2 to Q4,
L1, and two boxes the notebook named. Each ruling is the planner's unless
it says otherwise; the owner checked each against T5 and D-42 to D-46.

- **An entity's fields are state relations, fixed for its life.** Each
  field `f` of `T` is the state relation `T.f` (IR `<module>/T.f`), with
  `exactly one f per T`, or `at most one` for a `maybe` field, as §4.1's
  sugar already said; `?x is a new T {…}` includes the field facts and
  `?x.f` reads them, `null` for a `maybe` field with none (boxes 2.12c,
  3.18). Changed from the planner: no `then` line writes a field, because a
  field has no sentence. A value that changes is a state shape, which
  §9.7 already chose for a row's visibility and position; 8.6 imports a
  Clause relation as a field only when nothing writes it after creation.
  So a record is a value fixed at creation and every change is a sentence
  (principle 15), and no field-write syntax is needed (G1).
- **Views carry their fields' types** (2.12e), so bindgen and JSON read
  types instead of inferring them. **A requirement's name is its claim as
  `galileo fmt` prints it, a label;** review pairs requirements by lowered
  claim up to variable names, since one text can be two claims (R16), and a
  repeated one is a warning (2.12g, with 6.1's identity).
- **O4: `galileo check` exits 2 whenever it reaches no verdict,** a static
  error included, as review's table already sends "anything else" to 2;
  a warning never stops it (5.4b).
- **Claims bind their variables in one place.** One core function decides
  which forms bind which variables, for the checker and for lowering: the
  quantifiers, the left side of the four binary forms (D-44), `never`
  existentially, and definitions (`?x = e`, aggregates, `min`/`max … by`),
  which give at most one value and so need no quantifier. A membership
  binds nothing, or `?x in [a, b]` would be R15's implicit `some`, which
  core accepts today (3.11b, 3.11d).
- **Each claim's verdict is its own.** Each claim is evaluated on the
  program plus its own auxiliary rules: today one claim reading an
  unsupported construct turns every claim of `gate` Unknown, and a wider
  claim moves another's deciding budget from 80 to 641. A well-formed form
  the engine cannot evaluate yet is `Unknown(unsupported)`; a form SPEC
  does not define is a static error (3.11c). Box A, counterfactuals
  drawing on the claim's one budget, landed as the 3.12 fix.
- **A support under negation includes its absence leaves,** minimal under
  inclusion over both kinds, so it is sufficient (A-KR2026, Proposition
  1): a support of positive facts alone stops enabling firefox once
  `whiterabbit disables firefox` is stated (3.13b). §6.4's hitting sets
  are not even the right candidates under negation, as 3.14 already
  searches (SPEC now says so). **`galileo fmt`** refuses what the lexer
  rejects and changes no token, adjacency or nesting (§11.1; 2.13 landed
  it).
- **L1 enters §9.1:** a text block's closer sits exactly 2 spaces deeper
  than its opening line, as the lexer already enforces.
- **`prevent` and `achieve` take a claim** (§7.4 since D-41) while
  `supports.rs` takes one atom: a candidate counts when the claim, re-run
  under its one budget, Fails or Holds (3.15d), and `galileo prevent` and
  `galileo achieve` read the claim as a command argument (3.15e).
- **Recursive definitions are rejected** (§3.5): `define loop(n: Int):
  Int` with body `loop(n)` lowers as total today (3.19).

Rejected:

- **Field writes after creation (`then ?row.label is …`):** a second way to
  change state beside sentences, and a production no program needs; north
  changes only `position` and `visible` after creation, which §9.7
  derives. **Fields only on `EntityType`:** the state hash, checkpoints,
  replay and search would each need a second store.
- **Requirement identity by text or `module/text`:** the R16 pair and `/`
  in text literals break both.
- **Exit 1 on a static error:** 1 means a counterexample, which review
  compares by instance (D-29).
- **Membership as a claim definition (3.11's reading):** it keeps R15's
  hole open. **A static error for every unsupported form:** programs that
  7.4 and 7.5 will answer would stop compiling.
- **Expanding each absence into the facts that block it:** exponential and
  NP-hard to minimise; `why not` answers that question.
- **fmt repairing indentation:** a guess where the language errs (L1).

### D-48. The peer file's open DECISION lines

The language `DECISION` lines no landed ruling answered (notebook O5),
judged against SPEC, T5 and D-41 to D-47. Most were overtaken by later
rulings; the rest are ruled here.

- **Answered by later rulings.** DECISION 2.6 (nix-lib embedded): D-45.
  DECISION 3.9 twice: D-41 (absence scope) and D-47 (supports include
  absence leaves). DECISION 3.13 (Unknown under negation): 3.14's verified
  search, which SPEC §6.4 now states. DECISION 2.12, six lines: `?x is a
  T` is D-41's built-in relation (2.11e), entity records D-47 (2.12c),
  relation names and modes D-41 (2.11b, landed), handler order D-42 and
  D-43, views D-41 and D-47, `use` visibility D-41 (2.11d); the 2.12
  review's `entities("Hero")` goes with 2.11e. DECISION 4.11 twice: D-42
  and D-46.
- **Confirmed as landed.** DECISION 5.4: state-claim requirements and the
  derived `must realize` stand, and 5.7 replaces the local `type Host,
  Module` with `use firn/tags`. DECISION 2.11-fix: a variable-free
  parenthesized filler is computed at lowering and the fact states its
  value (§4.2's ground sentence, D-46's slot tokens). DECISION 6.8's hover
  levels and DECISION 3.14's typed-achievement oracle reading stand.
  DECISION 2.13: superseded by the explanation ruling 2.13 landed (fmt
  refuses rather than re-derives nesting; D-47).
- **Known computation (DECISION 2.6, 2.6-fix).** A prelude function takes
  known values and Galileo computes it, so `length("abc")` emits `3`; a Nix
  argument is a stage error naming the Nix call, where today a bare
  `length` reaches Nix. Interpolation takes `Text`, an `Int` in decimal or
  a Nix value; a known `Bool`, list, record or `null` is an error, as Nix
  makes it, only earlier. `++` with neither operand typed is an error,
  since emitting `++` or `+` would be a guess, and a Nix keyword as a
  Galileo name is an error, never a rename. A block binds a `let` name once
  (D-37), so the lowerer's hoisting of a second `let a` was a bug (box
  2.6c).
- **Reads walk the option tree (DECISION 2.15).** SPEC §3.2 already walks a
  read and an entry the same way; 2.15 checked entries only (2.15c). The
  `declarations` field the extractor adds is the lead's tooling.
- **A decided minimum stands (DECISION 3.14 twice).** A verified set at a
  size whose smaller candidates all failed decides the minimum, as a
  witness decides an existential claim; running out afterwards reports
  the verified sets marked incomplete, not `Unknown` (3.14b).
- **Patches use three words (DECISION 6.1).** `add`, `remove`, `replace`:
  `withdraw` left the handler vocabulary in D-42, and two words for one
  removal fail principle 2 (6.1c). The identity normalizes every
  scope-local variable and ignores sentence-register declaration order,
  which D-42 made unobservable; order inside Nix bodies reaches the
  emitted bytes and counts (2.12g).
- **The card's Nix examples build (DECISION 2.16)** now that `for` lowers
  (2.16b).

Rejected:

- **Renaming a Nix keyword on lowering:** two spellings of one name
  (compass 2). **Guessing `++` from the operands' shapes:** a wrong guess
  emits Nix that fails only at evaluation.
- **Implicit text for `Bool` in interpolation:** Nix rejects it, and
  `"{flag}"` reads as a bug more often than as intent.
- **Reporting `Unknown` after a minimum is witnessed:** it hides an answer
  the search already proved.

### D-48 addendum. An interpolated `Path` is Nix's store copy

A one-shot language ruling on DECISION 2.6c, 2026-10-11. Batch `b65a185`
fails `firn_import` at `flake.nix` after its six byte comparisons pass:
2.6c's checker accepts only `Text`, `Int` and Nix values in `{…}`, and the
imported line 41 applies `(firnModules) => …` to `./modules`, so the
directly applied lambda types `firnModules` as a known `Path`, and then
writes `"{firnModules}/{m}"`. SPEC §9.1 listed `Path` neither as allowed
nor as an error. In the firn pin this is the only interpolated path: no
`.nix` file writes `${./…}`, and the six byte-compared modules interpolate
none.

- **A `Path` interpolates, and its text is `Nix Text` (box 2.6c).** Nix
  coerces a path in `"${…}"` by copying it into the store and interpolating
  the store path. Only Nix evaluation knows that path, so Galileo neither
  computes nor substitutes it: the interpolation emits the expression as
  written inside `${…}`, whether the `Path` is known or not, and the text
  is Nix-stage (SPEC §3.2). A prelude call on it, or a non-`nix` fact slot
  given it, is the stage error. `"{./x}/y"` builds to `"${./x}/y"`, `let p
  = ./x` with `"{p}"` builds to `"${p}"`, and line 41 builds to
  `"${firnModules}/${m}"`, as committed.
- **Nothing else widens.** A `Path` fits no `Text` slot and stays distinct
  from `Text` everywhere else (§3.1, F4), and a known `./x` stays a known
  value; only the text that interpolates it is Nix-stage, and SPEC states
  that meaning rather than reading it off a `Nix` type.

Rejected:

- **The importer lifts the path differently:** Galileo has no cast to a
  `nix` type, `toString(firnModules)` gives the unstored absolute path and
  `firnModules + "/{m}"` a path, so each changes what Nix computes, and an
  importer workaround is a language bug (notebook compass 13).
- **Galileo computes the text** as `./modules/…` or an absolute path: the
  value Nix interpolates is a store path, and no Galileo value may depend
  on Nix evaluation (§3.2's trust boundary).
- **Counting a `Path` as a Nix value:** a known `./x` would turn Nix-stage
  wherever it stands, the silent widening the question ruled out.

### D-49. A misspelled name is an error, and `?_` under `not` means no value exists

The language owner's ruling on two questions from the lead before the
zero-shot benchmark's second run, judged against T5's principles 8 and 12
(words that cannot be misread; succeed or fail early): 2.11e's open risk
that a misspelled name in an entity slot silently becomes an entity, and
REVIEW bea430f's finding that the bare `_` message suggests `?_`, which
then fails in its most common use, `not ?p borrowed ?_`. Measured at main
`bea430f` with a template matcher over the `.gal` sources and firn's tag
facts as `galileo_import::bnix_tags` writes them from `FIRN_ROOT`; no cargo
runs.

- **An entity exists only by its declaration (box 2.11f).** `X is a T` is
  the only way a name becomes an entity of a `type`, and `new` the only way
  for a `state entity`. Every name in an entity slot, in a fact, rule,
  claim, handler, view, query, Nix body or question, names a declared
  entity of the slot's type, or the line is an error that names it, the
  type and the nearest declared entity of that type. A declaration in any
  module counts (SPEC §2). One name may be declared under several types
  (firn's `python`, `racket`, `doom-emacs` and `printing`), so the "is
  already a" error goes; declaring one entity twice is a warning. This
  supersedes D-41's "a fact introduces each entity it names": `is-a-T`
  holds exactly the declarations, and 2.11e's implied facts go.
  - The corpus's facts name 1,728 entities, counted per program. 1,535
    are declared: every one in the dungeon (19), `lib/firn/dispatch.gal`
    (47), the responsibility facts (12), the 4.12 lane's `host-list` and
    `prewarm` (2), the benchmark's data (196 in the public examples, 1,257
    in the hidden tests; the card already teaches "every entity is
    declared once") and firn's 2 hosts. The other 193 are all in firn's
    tag facts as the 5.1 importer writes them: 166 modules, 25 tags and 2
    platforms. Rules and claims name 0 undeclared entities (the dungeon's
    claims name its declared hero and rooms; the benchmark references
    name none).
  - firn needs one declaration per module file (214), 25 tag lines in
    `config/tags.gal` and 2 platform lines, all written by the importer.
    Every host-enabled tag is carried by some module, and every
    host-edited or disabled module exists (`glide` has no tags but has its
    file), so the declared program lowers cleanly. firn's own validator
    already rejects a host edit or `:disabled` entry that names no module,
    but not a misspelled tag on either side; now every slot is checked.
  - Over firn's 584 name occurrences, a typo at any one of them is an error
    under this rule unless it spells another declared entity of the same
    type, which no name rule can catch. A command that reads part of a
    project must read the files that declare what it names; with 2.11e
    that partial read gave silently wrong answers instead.
- **Each `?_` is existential where it stands, inside a `not` too (box
  2.11g).** SPEC §6.2 already says so of claims; rule bodies, handler
  `when`, views and queries now read it the same way. `not ?p borrowed ?_`
  holds when no value makes `?p borrowed ?_` true, as Datalog reads
  `!borrowed(p, _)` and Prolog `\+ borrowed(P, _)`, and each `?_` is its
  own variable, so `not ?_ connects ?_` holds when the relation is empty.
  The `not`'s named variables must still be grounded elsewhere. An absence
  leaf prints the `?_`, "nothing states `ben has read ?_`", and `why not`
  names a fact that blocks it. The bare `_` message keeps offering `?_`,
  which is now right everywhere a `?_` can stand. Of the 14 `not` literals
  in the 8 benchmark references, 4 negate a helper relation that exists
  only to drop one variable (`has owners`, `is busy`, `is locked`, `is
  trapped`), each a shape and a four-line rule that one `?_` replaces, and
  card v1b tells authors to write such helpers.

Rejected:

- **A name that appears exactly once is an error:** it rejects 83 correct
  firn modules, each tagged once and named nowhere else; a typo repeated
  twice passes; and whether a line is legal depends on lines elsewhere.
- **The subject slot introduces an entity, other slots must name one:** it
  rejects 25 correct firn names (23 tags, `darwin` and `glide`), still
  misses a typo in 282 of 584 occurrences (every subject), and no reader
  can see which slot introduces.
- **Per-type opt-in, such as an open `type`:** a keyword and two kinds of
  type for one meaning (T5's principle 2), and the open type keeps the
  hazard.
- **Facts introduce entities, with only rules and claims checked:** a
  misspelled tag in a module file or a host's `enables` line stays a new
  tag, the case the lead's question names.
- **The `_` message stops suggesting `?_` under `not`:** one helper
  relation per negated existence, against the priors of Datalog and Prolog
  (T5's principle 9) and the first-try bar (principle 12).
- **Any variable seen only under `not` is existential:** a misspelled
  named variable silently means "anything".
- **Lowering `not S` with a `?_` to `count where S` equal to 0:** the same
  truth, but `why` would show a generated count and `_#` names where the
  author wrote `not`.

### D-50. A fact changes origin only when its shape changes kind, and `?_` stands only as a whole argument

The language owner's ruling on the two `DECISION` lines left open after
D-49 (peer file, 6.2 and 2.11g), judged against SPEC §4.1, §8.2 and §8.4,
T5's principles 1, 2 and 12 and the notebook's compass. Read at main
`2c9a13a`; no cargo runs. No other `DECISION` line after D-48's batch is
unanswered.

- **R11's journey is a derived patch that changes a shape's kind (DECISION
  6.2, boxes 6.2, 6.6, 6.7 and 2.8d).** A shape's kind fixes where its
  facts come from: a `given` relation's from source lines, a `derived`
  one's from rules only (§4.1). So within one program no sentence is both
  stated and derived, and the prototype's R11 scenario, a revision that
  `admit`s `Goal holds` while a rule still derives it, has no Galileo
  spelling. A fact changes origin only when its shape changes kind, from
  `derived` to `given` or back, and only a derived patch, two program
  images such as a git base and the working tree, can change a shape: a
  written patch cannot redeclare one (§8.2's second restriction). That is
  the case R11 guards in Galileo: a refactor that replaces a rule by the
  facts it derived would otherwise show every one of them lost. So 6.2
  checks R11 on R9's baseline frozen as one image and, as the other, the
  same program with `holds` declared `given`, its three rules gone and
  `Seed holds`, `Gate holds` and `Goal holds` stated: the truth view
  reports each "now stated", none gained or lost, and with the images
  swapped "now derived". Both images are frozen from source by the public
  `Baseline::freeze` the R9 scenario already uses, so no core image is
  built by hand. Under a written patch R11's lesson still holds: a derived
  fact whose support changes is neither gained nor lost. On R9's baseline,
  `add Goal is seeded` with `remove make-gate` loses `Gate holds` and
  keeps `Goal holds`, now derived through `make-seed`.
  - The written patch `add Goal holds` stays an error, but its message
    changes. Today it reads "only static facts are stated here; state facts
    go under `initially`", advice that fails again for a derived shape. It
    becomes `` `holds` is derived, so only rules give its facts ``, at the
    top level, under `initially` and in a patch alike (box 2.8d, after
    2.19a renames `static`).
  - A derived patch's edits as sentences (stated facts added and removed,
    declarations added, removed or replaced, a shape's changed kind
    included) arrive with git mode, 6.7, the first command that builds
    two images; 6.2 lists a written patch's edits.
- **A `?_` stands only as a whole argument of a sentence (DECISION 2.11g,
  box 2.11h).** The worker's narrow reading is confirmed: a `?_` inside an
  expression, as in `not ?p has (?_ + 1)` or `?n = ?_ + 1`, is an error,
  under a `not` or not. Each `?_` is its own variable, so nothing else can
  ground it, and an expression is computed from grounded values, never
  solved. The error prints the variable as written, `?_`: today its list
  shows the internal `?_#1`, the leak D-49 rejected for `why`.

Rejected:

- **A patch may state a sentence of a derived shape, and review reports
  "now stated":** the patched program is one no source file can write, a
  special case for patches only (T5's principle 1).
- **Relations fed by both facts and rules, as in Datalog and Prolog:**
  reopens §4.1's kinds for one review label, while no corpus program needs
  it (compass 3).
- **Rewriting R11 over a stated shape whose kind stays:** its facts never
  change origin, so the scenario could never report "now stated".
- **Checking the truth view on a hand-built core image:** a program no
  source writes, where the derived patch reaches the same two images from
  source.
- **A `?_` inside an expression as existential:** for `+` it means the
  bare `?_` again, and in general it hides an equation solved per row.
  Datalog's and Prolog's anonymous variable is an argument too.

### D-51. Rules and clauses read as one sentence, no output record outside Nix, template words are names, and `galileo run` steps a source program

The language owner's ruling on the lead's batch D-51: Tom's length finding
(through the proxy), DECISIONS 5.5 and 5.7 in the peer file, and, added
mid-batch as benchmark-critical, DECISION 7.3. Judged against T5's
principles 1, 2, 4, 9, 12 to 14, SPEC §4, §5, §9 and §10.4 and the
notebook's compass. Read at main `fa31751`; no cargo runs.

- **The length finding, measured.** Over the 8 benchmark references,
  counting non-blank lines, Galileo has 405 and Python 170 (2.4x); Soufflé,
  which also declares its 7 to 9 input relations, has 243. Galileo's lines
  are: derived declarations 42 (10%), rule framing 108 (27%: the
  `rule NAME` line and the lone `when` line, 54 each), rule heads 54
  (13%), rule bodies 117 (29%), requirements 18 (4%) and the `nix file`
  record 66 (16%: 16 header, 25 `for` and 25 entry lines). Python's are 16
  of input, 8 `print` and 146 of logic. So declarations and the output
  record are half of the excess, and the other half is rule framing, which
  the finding did not name. Lines did not slow the authors: in the dev
  cells of `results.jsonl` the authored solutions have a median of 46
  lines in Galileo and 45 in Python, and 734 and 796 output tokens. They do
  cost the reader, and Tom's readability pair waits on this. After this
  ruling the references come to 231 lines, 1.4x Python's and below
  Soufflé's, and every line left is a declaration, a head, a literal or a
  requirement.
- **(b) No output record outside Nix (SPEC §4.5, §11.1).** A program
  declares output only for a target that writes a file, `nix file` and
  `nix module` for Nix. Otherwise its relations are its answers, and
  `galileo query … --json`, a command since D-41, asks for them. The
  benchmark's `nix file` records restate each answer relation in Nix's
  attribute shape, 8 lines a program and all of them mechanical, which
  fails principle 2. Saves 66 lines (16%). It loses no check and touches
  no reading, since the record only emits. The answer interface moves into
  the task contract, which names each answer by its sentence, as Python's
  names its JSON keys. That is the lead's harness, not a box.
- **(a) Derived shapes stay declared, one line each.** Inferring a shape
  from rule heads saves 42 lines (10%) and breaks compass 10, D-49's rule
  for names: a misspelled head silently declares a second relation and
  takes a rule from the first, where today it is an error that names the
  closest shape. Under (b) nothing in the program reads an answer
  relation, so no "unread relation" warning could catch it. It also
  breaks R16: a `use` that exports a matching shape turns a declaring
  head into one deriving another module's relation. Declaring the shape
  inline in its first rule saves the same 42 lines, but makes one line
  both a declaration and a use, picks a "first" rule that source order
  must not decide (compass 8), and scatters the program's vocabulary,
  which as one block reads like a statute's definitions section
  (principle 9) and tells the reader what the program computes. Slot
  types, modes and `milestone` need the declaration anyway.
- **(c) A rule reads as one sentence, and so does a handler clause (SPEC
  §4.3, §5.2, §9.4; boxes 2.20a, 2.20b).** `rule <name>: <head>` takes one
  line, and each body literal one line under it, the first opening with
  `when` and every other with `and`:
  `rule nested-member: ?u is in ?p` / `when ?u is in ?g` /
  `and ?g is nested in ?p`. A handler clause lays out `when` and `then`
  the same way. The lone `when` and the separate head line carry no
  structure (principle 2), while a rule body is a conjunction like every
  query's and claim's condition, now spelled like one (principle 1) and
  read as English (principle 4). One literal per line stays (D-46), so a
  `where` still ends at its line's end, and rule names stay for `why`
  labels and patches. Saves 108 lines (27%). The old layout is an error
  showing the new lines, with no compatibility. The SPEC examples are
  rewritten here; the card changes in 2.20a and 2.20b, so the medium
  benchmark's card should take them before its dev runs.
- **DECISION 5.5, all four narrow readings confirmed (SPEC §9.6).**
  (a) A flake input's name is quoted `Text`: only the module that needs an
  input names it, so a `FlakeInput` declaration would repeat that line.
  §9.6 now quotes `"nur"` and lists the landed shape, the derived URL
  relation with its mode, and the rule. (b) The platform-support
  requirement is 5.1's second tags requirement; 5.5's row now says so.
  (c) Only `:url` is a fact; 5.6 adds facts for the other attributes it
  emits. (d) `galileo build` refuses while a derived mode Fails, as §4.4
  says. Nothing landed changes.
- **DECISION 5.7: (a) and (c) confirmed, (b) and (d) amended (SPEC §9.1;
  boxes 2.8e, 5.7b).** (a) `firn/responsibilities` uses `firn/tags`, the
  importer strips `-module` as firn does, and the host file uses both,
  since `use` is not transitive. (c) The completeness requirement checks
  the module too, as D-34 lists it. (b) The library declares `boot`,
  `environment` and `long-lived-system-service` because its rules name
  them; a host that restates one gets §4.2's warning, not an error. The
  workaround, templates saying "startup" and "user session" so that facts
  can write `boot` and `environment` bare, exposes a language defect.
  §9.1 made every word of every template in scope a reserved entity name,
  so adding a library shape could break facts in other modules. That
  rule predates D-42, one typed reading per line, and D-49, every slot
  token a declared entity of its slot's type, which together make it
  redundant. A template word is now a name in a slot, and backticks
  settle only a line that two readings fit (2.8e). The templates then say
  `boot` and `environment` again, one word per concept (5.7b). (d)
  Matching existentially is confirmed, since sets have no first element,
  and the single value that the .bjs assumes becomes a contract: `at most
  one` domain, criticality and lifecycle per responsibility, so a second
  value is an error showing both lines (5.7b).
- **DECISION 7.3: `galileo run` steps a source program through a file of
  event sentences (SPEC §10.4, §11.1; boxes 7.3, 7.3b).**
  `galileo run [PATH…] --events FILE` reads source paths, as `check`,
  `build` and `query` do (D-42's ruling on 4.11). FILE holds one ground
  event sentence per line, each read as a question is: one typed reading,
  declared entities and closed domains, so a misspelled event is an
  error at FILE:line and nothing runs. Each line is its own occurrence,
  not an atomic `run [...]`. The command prints `{"revision": 0, "hash":
  H}` and then one JSON object per line: an admitted step's revision, hash
  and effect sentences in canonical order, or a rejected step's kind,
  explanation and the unchanged previous revision and hash. Later lines
  still run. It exits 0 when every event is admitted, 1 when any is
  rejected and 2 when nothing runs. The same input prints the same bytes,
  which is the replay check. `--events FILE` also sets the state that
  `galileo query … --json` and `galileo check` answer in (7.3b), so the grader
  reads final answers with `query`. Sentences, not JSON lines: agents
  write Galileo, and an event read through the program's own templates gets
  D-42's and D-49's checks, where JSON needs a second encoding of event
  and slot names. The benchmark already renders facts per language. A
  `state entity` has no name to write in FILE, so a task keys what its
  events create by a field value or a declared entity (notebook O7).

Rejected:

- **A `show` declaration for answers:** a second spelling of a question,
  which D-41 made a command argument (compass 2).
- **Derived shapes inferred from heads, or declared in the first rule:**
  see (a).
- **Only merging the head into the `rule` line, with `when` kept as a
  block header:** saves 54 lines where the conjunction layout saves 108,
  and keeps rule bodies the one condition not spelled as a conjunction.
- **One-line rules for one-literal bodies:** a second layout (compass 2).
- **Keeping §9.1's template-word reservation and renaming the domains in
  templates:** a sentence-matching workaround that grows with every
  library.
- **An image prerequisite for `galileo run`:** `galileo compile` does not
  exist, and every command reads source today; an image takes the paths'
  place when WP8 builds one.
- **JSON-lines events and a `--json` switch:** no host needs JSON input
  yet (compass 3), and §10.4 already fixes JSON lines as the output.
- **One atomic sequence per file:** a grader needs each rejection
  reported, with the steps after it still run, which is how a host sees a
  session.

### D-52. Nothing is overwritten: a session's history, as-of, since and the chain of title

Tom's decision, relayed by the proxy as his "yes" (T5's principle 18),
written up by the language owner. It confirms D-47's option A (a record's
fields are fixed at creation, and anything that changes is its own
relation) and extends it: a step never overwrites anything. Read at main
`d8a6824`, after 7.1, 7.2, 7.3 and 2.12c landed and with 7.3b in progress.
Judged against T5's principles 1, 2, 8, 9, 12, 15, 17 and 18, SPEC §4.1,
§5, §7 and §10.4, and the notebook's compass. No cargo was run. Prior art
was checked against Datomic's filters page and SQL Server's temporal
tables.

- **D-47's option A is confirmed.** A field is fixed for its entity's
  life, and a value that changes is a state relation that a `then`
  sentence writes. This closes the direction question D-47 left with Tom.
  Nothing in the landed 2.12c changes: a field fact gets a span like any
  other fact, one that runs from its entity's creation to its deletion.
- **A session keeps every state it admits (SPEC §5.7).** The history is a
  span `[from, until)` per fact and per live entity, plus each revision's
  allocator. The state at revision `r` is every fact and entity whose span
  contains `r`. An admitted step closes the span of each fact in `S \ S'`
  and opens one for each fact in `S' \ S`, both at `r+1`. A rejected step
  records nothing, and a sequence `run [...]` is one revision. A span is
  SQL:2011's system-time period: a row is current at `t` when `from ≤ t <
  until` (A-Temporal). It is also Dedalus with its timestamps kept
  (A-Dedalus).
- **"The latest fact per key", by mode.** A relation with one mode (one
  mode line, `exactly one` or `at most one`) keys a fact by that mode's key
  slots. Its sentence adds the new fact and supersedes the fact at that
  key. The old fact's span ends where the new one's starts, and both stay
  in the history. Step 5 already holds every key to at most one fact per
  state, so a key's open span is its value now and its closed spans are its
  past values, in revision order. Under `exactly one`, a live key's latest
  record is always a value. Under `at most one`, `then not` can make it a
  withdrawal, and then the key has no value. A relation with no mode, or
  with two or more (D-43), keys a fact by the whole fact: a sentence opens a
  span when the fact did not hold, and `then not S` closes one when it did.
  The key is how changes are told: "superseded by" names the new value at
  the same key. It is not a second store.
- **Withdrawal, supersession and deletion.** `then not S` closes `S`'s span
  at `r+1` when `S` held, and does nothing when it did not. A superseding
  sentence closes the old value's span. `then not ?x is a T` closes the
  entity's live span and the span of every state fact that mentions it,
  fields included. Ids are never reused (D-31), so the entity and its
  fields can still be asked about as of any revision in its life. Pending
  effect requests are state (§5.4), so they get spans the same way.
- **A sentence whose fact already holds records nothing.** The same goes
  for `then not S` beside `then S`: the state did not change, so the open
  span goes on. Tom left this to the language's design ("whatever the
  proper answer is consistent with the language's design", 2026-10-11), so
  option A below is final.
- **The log and the revision hash (§5.7, §10.4).** The history is derived
  from the event log. Steps are deterministic, so replaying the log from
  `initially` builds the same history. The revision hash stays `h(S)`,
  computed over the state alone. That lets search merge two paths that reach
  one state (§5.5), and lets a restored checkpoint continue with the same
  hashes. The history's integrity check is that its state at each revision
  hashes to the hash the log recorded there. A checkpoint holds the history,
  and a session converted from a Clause checkpoint has a history that begins
  at that checkpoint's revision.
- **A program reads one state, now. Only questions read history.** A rule,
  a query and a requirement read the state being evaluated. A handler's
  `when` reads its pre-state. An invariant or `ensures` reads the post-state,
  where `before` reads the pre-state. No construct names a revision.
  `galileo check --events FILE --as-of R` evaluates state claims in the state
  at `R`, and temporal claims stay claims about the whole transition system.
  So §5.1's rule (no construct joins two states, which is R1's fix) holds
  over time as well. A program means the same in every run, and search
  states carry no history. A program that needs its own past states it, as
  Dedalus states persistence: `{hero} has held {key}`, written by the
  clause that makes it true and never withdrawn.
- **Surface: two flags and no new word (§9.4, §10.4, §11.1).** `then` reads
  as before.
  - `--as-of R` answers `query`, `check`, `why` and `why not` in a run's
    state at revision `R`. It replaces D-41's `--revision`, which no code
    parses yet. "Revision" alone collides with git revisions in the same
    CLI (`galileo review --base REV`) and does not say which state. "As of"
    has one established meaning wherever it is used: Datomic's `as-of`,
    SQL:2011's `FOR SYSTEM_TIME AS OF`, Delta Lake's `VERSION AS OF`,
    legal and financial English (A-Datomic, A-Temporal; principles 8
    and 9).
  - `R` is a revision as `galileo run` prints it, not a line of the events
    file. A host session has no file, and a rejected line makes no
    revision.
  - `--since R` makes `galileo query` list how its answer changed after `R`.
    The first JSON line is the answer at `R`. Each later revision whose
    answer differs gets one line, with its events-file line, its event
    sentence, and the rows added and removed. "Since" is read the way
    `git log --since` and "changes since" are read.
  - The query language gains nothing. A question's time is a property of
    its command, as `--via` and `--using` are (D-41 rejected positional
    `at revision`), and the three uses above keep programs to one state.
- **`why` over time is a chain of title (§7.3).** `galileo why S --events
  FILE [--as-of R]` prints `S`'s proof at `R`, with each state leaf ending
  `since revision k`. It then prints one block for each occurrence the
  proof reaches, latest first, each block once: `revision k: <event>
  (FILE:N), by the clause at <file:line>, when at revision k-1:`, followed by
  the clause's `when` proof in the state before it. The state leaves of that
  proof say since when they held, all the way back to `initially`'s lines.
  A rejected event never appears in a chain. Blocks are flat, not nested, so
  the output grows linearly with the run and reads like a ledger: each
  conveyance with its warrant (principle 9). A chain that the work budget
  cuts short says so (G9). The block names the clause because a seeded bug
  lives in a clause, and the medium benchmark's Dim 1 measures how fast it
  is located.
- **`why not` over time.** For a state sentence that held before `R`, it
  prints `held from revision a until revision k`, the cause at `k`
  (`superseded by S2`, `withdrawn`, or `deleted with E`) and that
  occurrence. For a sentence that never held it prints `never held in
  revisions 0 to R`. Near misses are computed in the state at `R`.
- **Engine.** The state is the materialized current view. The step core
  reads and writes only `State`, and its `Transition` gains its net change
  (facts added and withdrawn, entities created and deleted), built from its
  own `W`, `I`, `C` and `D`. So the run appends history in proportion to
  the change, never by diffing whole states. A question about now never
  reads a span. An as-of question selects spans and evaluates rules on that
  one state. A since question evaluates only at the revisions where a span
  of a relation it depends on opened or closed. `why` re-matches the one
  occurrence it explains on its stored pre-state, through a step-core entry
  that returns the clause and substitution behind each added fact. It
  rebuilds that on demand, as §7.1 rebuilds proofs (A-Souffle). No question
  re-runs events from revision 0. Compaction is planned and not built: it
  will drop spans that closed before a horizon revision and make that
  revision the earliest one an as-of can answer (§12.1).
- **Boxes, in order (PLAN WP7).** 7.10, the history in the core, can start
  now beside 7.3b, since they share no file. Next comes 7.11 (`--as-of`).
  Then 7.8, amended to the chain of title, and 7.12 (`--since`) run in
  parallel. 7.8b (`why not` over time) comes last. That is 90 minutes in
  all, and all of it lands before 7.4. The medium benchmark's dev runs
  start after 7.3b and 2.19h and do not wait. Dims 1 and 5 (seeded bugs and
  routine questions) run on held-out tasks after Dim 4, so the transition
  card should take `--as-of`, `--since` and `why --events` once they land.
  Dim 5's Q3 asks after which events a sentence's truth flipped, which
  `--since 0` answers in one call. Its gap note about 7.8 closes when 7.8
  lands. These rows are amended:
  - 7.4: search nodes are states without history;
  - 7.7: `trace` prints an events file;
  - 7.9: pending requests get spans;
  - 8.1: a checkpoint holds the history, and `as_of` is in the runtime
    API;
  - 8.11: a converted session's history begins at its checkpoint.
- **Landed code that changes, by file.**
  - `crates/galileo-core/src/state.rs`: `Transition` gains the net change,
    `Run` gains `history` and appends to it on admission, `Run::replay`
    rebuilds it (7.10), and there is a step-core entry for the causes of
    added facts (7.8). `State` keeps `facts`, `live` and `next`.
  - New: `crates/galileo-core/src/history.rs` (7.10, 7.12), with
    `crates/galileo-core/src/lib.rs` exporting it.
  - `crates/galileo-core/src/why.rs` (7.8) and `why_not.rs` (7.8b).
  - `crates/galileo-cli/src/main.rs` (7.11, 7.8, 7.8b, 7.12): the flags and
    USAGE.
  - `crates/galileo-cli/tests/run.rs`.

  Nothing in `galileo-syntax`, `engine.rs`, `eval.rs`, `check.rs` or
  `claims.rs` changes, and 7.3b stands as staffed.

**Restated facts: option A, final (2026-10-11).** Tom answered "whatever the proper answer is consistent with the language's design". A is the answer that fits it: the history is the sequence of admitted states, so each revision's state hashes to its logged hash and replay rebuilds it; programs read states, never statements; and the step core already folds a restated fact into no change (§5.3 step 4). B would add records that no as-of or since answer can see. The question as it was put:
tension: Tom's words, "a `then` sentence adds a fact at the next revision
and never replaces the old one", read literally, record a fact for every
`then` sentence, including one whose fact already holds. D-52 records only
changes of state.

- **Option A (ruled):** record each change of state. A span opens when a
  fact starts holding and closes when it stops. A re-stated fact records
  nothing.
  - The history is exactly the sequence of admitted states.
  - Its state at each revision hashes to the logged hash.
  - Memory grows with change.
  - `why` names the occurrence that made the fact true. Datomic's history
    view likewise shows each value change as a retraction and an assertion.
- **Option B:** record every `then` sentence's fact at its revision, held
  or not, as SQL Server adds a history row for every update "even if no
  column values change" (A-Temporal).
  - `why` could name the latest occurrence that re-stated a fact.
  - A game that restates every unit's position each tick grows its history
    every step while nothing moves.
  - As-of answers stay the same, because the records collapse to the same
    spans.

Recommendation: A. A history should keep what happened to the state, and
B's extra records change no answer except which occurrence `why` names.

Rejected:

- **The history inside `State`:** search could never merge two paths to one
  state, every hash would differ, and every step would copy the history.
- **A log of deltas without spans (Datomic's log alone):** an as-of answer
  would fold every delta from revision 0, which is replaying history.
- **Derived relations materialized at every revision:** memory per revision
  spent on answers an as-of computes from one state.
- **`as of revision N`, `since` or `ever` inside rules, claims or the text
  of a question:** these join two states (R1), make a program's meaning
  depend on a run, and give search states a history. D-41 rejected
  positional `at revision` for the same reason.
- **Keeping `--revision`:** it collides with git revisions and names no
  state. **`--as-of` taking events-file lines:** hosts have revisions, not
  files, and `--since`'s lines already carry both.
- **Storing each added fact's clause and substitution at every step:** that
  spends memory on every step for an answer `why` rebuilds from the stored
  pre-state when asked.
- **A nested causal tree:** its depth grows with the run. **One link per
  `why` call:** the chain is what was asked for, and the budget already
  bounds it.
- **A history hash chained through revisions:** the log's pairs of event
  and hash already identify a history, and replay checks each one.
- **`--since` on `why` and `check`:** that would be a second way to list
  changes. `query` lists them for any answer.
- **Compaction now:** Tom asked for it planned, not built (§12.1).

### D-53. The language is Galileo: `galileo`, alias `gal`, files `.gal`

Tom renamed the language on 2026-10-11. In his words: "Well, it's called
Galileo. It's called that I like calling it that" and ".gal works". The
old name, Muove, kept being mangled by voice transcription.

- The command is `galileo`. The short alias `gal` is a symlink installed
  beside it, not a second build target.
- Source files end in `.gal`. Directory loading reads `*.gal`, and the
  language server claims `.gal` with language id `galileo`.
- The crates are `galileo-syntax`, `galileo-core`, `galileo-cli`,
  `galileo-import`, `galileo-lsp`, `galileo-nix` and `galileo-review`. The
  repository is tompassarelli/galileo, and the local container is
  `~/code/galileo`.
- The rename is mechanical: no behavior changes, and the workspace suite
  passes unchanged at 157/0. SPEC, PLAN, DECISIONS and the notebook use the
  new name throughout, including earlier entries.
- Kept as history: the external reviews under `inputs/` keep their original
  text and paths, and git history keeps the old name. Earlier benchmark
  results recorded under `muove` stay as they were. Results from the medium
  benchmark on are reported as Galileo.

### D-54. Runtime images contain linked source and identity

The Galileo lead ruled box 8.2 on 2026-10-11, signing as the language
owner, and the owner confirmed it in D-55: no IR codec. Core stays
dependency-free, and a handwritten IR format would have to track every IR
change. A program image contains the linked modules' source and the
program's §8.1 identity. Its plain-text format starts with
`galileo-image 1 <identity hex>`, then, for each module in link order,
`module <name> <byte length>` followed by exactly that many source bytes.

`galileo-runtime` owns `Program`, holding the lowered Core program and its
identity. `Program::load(bytes)` parses the image, lowers it with
`galileo_syntax::lower::lower_program`, recomputes the identity and returns
`Error::WrongProgram` on a mismatch. Session, Event and Projection take
this runtime Program; dereferencing it to the Core program is allowed.
`galileo compile <module> -o <image>` writes this format, and `galileo run`
accepts images or source paths.

Rejected:

- **A codec for the Core IR:** the core crates take no dependency (D-20),
  and a handwritten format would have to follow every IR change.
- **A free `load` that returns the core `Program`:** the runtime re-exports
  that type, so the host would hold no identity to check a checkpoint
  against.

### D-55. The lead's rulings of 2026-10-11, audited: receipts settle by slot name, pending requests are state, a type is `module/Type`, and a slice replays its projection

The language owner's audit of the nine `RULED` lines that the Galileo lead
posted on 2026-10-11 under the owner's name, which AGENTS.md and D-36
reserve to the owner (peer file lines 1127, 1144, 1147, 1187, 1231, 1256,
1257, 1277 and 1460), and of the SPEC §5.4 sentence that 7.9's review cut
changed (`6323367`). Read at main `c80182c` and judged against SPEC's
intent, T5's rubric, the notebook's compass and D-15, D-28, D-41, D-42 and
D-52. No cargo was run. Seven rulings stand as ruled, one stands with
SPEC's spelling, and one SPEC sentence is replaced.

- **A warning never stops a check (5.7b, line 1127; `22b6309`).
  Confirmed.** SPEC §6.1 and §4.2 said it since D-47 and D-49: a warning,
  such as a fact stated or an entity declared twice, prints and the check
  goes on. The CLI's exit 2 on a warning was a bug against SPEC, and
  keeping the lowerer's warnings apart from its errors is the fix, not a
  new rule.
- **Which requests are pending (7.9, line 1144; `6323367`). Confirmed.**
  Only a shape with `settled by` has pending requests, since nothing could
  settle another (D-15: `settled by` defines which requests are pending).
  A sequence's requests are one set, the union of its occurrences', so
  equal requests in one admission are one (D-42 made `E` a set, and a
  sequence is one admission). §5.3 and §5.4 now say both.
- **A receipt settles by slot name (7.9's review cut). Replaced.** The cut
  settled a request when the receipt's "entities of the request's entity
  types equal the request's entities in order". It closed a real gap, a
  receipt that carries an entity the request lacks, as a `status` of an
  entity type would. But positions across two templates are English word
  order: `copied {dst: File} from {src: File}` never settles `copy {src:
  File} to {dst: File}`, and a receipt with a second `Turn` never settles
  anything, both silently. A receipt now settles the oldest pending
  request that agrees with it on every slot both shapes name, as mode and
  domain lines already name slots (T5's principle 1; compass 8 and 10).
  The receipt must name every entity slot of the effect, and a shared name
  has one type in both, or the `settled by` line is an error naming the
  slot (compass 3 and 7). Every `settled by` in the repository already
  agrees: `send` and `finished` share `turn`, the runtime test's `arrival`
  and `acknowledged` share `hero` and `room`, and north's
  `menu-host-effect` and `clear-host-effect` share `north`. Box 7.9b.
- **Pending requests are part of the state (K16, found in this audit).**
  §5.4 said so, while §5.1's state and §10.4's hash listed only the facts,
  the live entities and the allocator. The code already hashes the pending
  requests oldest first, the order "the oldest" needs. §5.1 and §10.4 now
  list them.
- **A type's identity is module-qualified (2.10c, lines 1147 and 1187;
  `c9cb3b7`). Confirmed, in SPEC's spelling.** D-41 lets two modules each
  declare `Hero`, so the linked IR must tell the two apart. Messages print
  the written name, and D-41's ambiguity error names both modules. The
  test helper that compares the linked and single-module models by
  written type name states §9.6's claim, the same model up to module
  names, as `by_template` already did for relations. But the landed key
  is `dungeon.Hero`, while §2 names everything a module declares
  `module/name` and §4.1 names a field `<module>/T.f`. One type has two
  spellings today, and `Program::field_relation` converts between them.
  The key is `dungeon/Hero`, so its field `dungeon/Hero.f` is the type's
  name plus `.f` (compass 2). §2 now says so. Box 2.10e.
- **Typed host calls (8.3, line 1231; `0f9ae4c`). Confirmed.** `run` and
  `run_all` take `E: Event`, `view` takes `V: Projection`, and bindgen
  implements both. That keeps §9.7's and §10.4's `session.run(event)` and
  `session.view::<Menu>(…)` instead of moving them onto generated types. A
  tuple stays an `Event`, and the untyped view is `project`, the word
  §10.4's observation contract already uses. §9.7 and §10.4 now pass the
  view's arguments: `view::<Menu>(north)`.
- **Rows stay state shapes in the mechanical import (8.6 (1), line 1256).
  Confirmed.** `clear-menu-rows` withdraws the row relations outside the
  creating clause, the case PLAN 8.6's rule names, so they become fields
  only once 8.8's `delete` replaces the teardown. A row relation that the
  clear withdraws while the row stays live is `at most one`, since
  `exactly one` would reject steps Clause admitted (line 1277 (3)).
- **Images are linked source plus identity (8.2, line 1257; `497e41b`).
  Confirmed** as D-54, whose attribution is corrected.
- **The mechanical menu keeps its mailbox and also emits the effect (8.6,
  line 1277; `9fe1864`). Confirmed.** D-28 makes the first import keep
  Clause's admission, and three accept guards read the mailbox, which no
  Galileo condition can read from pending requests. The carrying effect,
  `at most one per step` and `settled by clear-host-effect`, gives §10.4's
  observation contract its ordered effects and makes `pending()` mirror
  the mailbox. D-15's rejection of a single-slot mailbox is about the end
  state; whether 8.8 reaches it is O8.
- **A slice replays its projection (8.7, line 1460; `4315225`).
  Confirmed.** The menu slice has 13 events (§9.7, N10), and 132 of the
  recording's 185 occurrences belong to other slices. Replaying the
  recording projected onto the slice, one atomic sequence per transaction,
  with every slice state relation compared at each boundary (the review
  cut's widening), catches any dropped occurrence that touches the slice.
  Importing outside handlers would replay a different slice. The other
  slices replay theirs (8.13), so A11 covers every occurrence. §10.4 now
  states the rule.

Rejected:

- **Settling by entity type and position (the review cut):** silent when
  a receipt orders same-typed slots differently or carries another entity
  of a request's type.
- **Settling when every entity argument is equal (the wording before the
  cut):** a receipt that carries an entity of its own never settles.
- **Matching shared slots without the entity check:** a receipt that drops
  or renames the effect's entity slot would settle the oldest request of
  any entity, silently.
- **Dropping the mailbox in the mechanical import:** admission would change
  while a request is pending, against D-28.
- **Importing out-of-slice handlers for 8.7:** that replays another slice
  and hides the interference the boundary comparison shows.
- **Keeping `dungeon.Hero`:** two spellings of one name, against §2 and
  compass 2.

### D-56. The lead's four remaining rulings, audited: a no-op takes its revision, a block groups its clauses, a quoted key is a text literal, and a keyword reading can meet a template reading

The language owner's audit of the four `RULED` lines that the lead signed
before D-55's batch and that notebook O5 listed: 7.10's no-op revision
(peer file lines 753 and 759), 7.8's multi-clause blocks (815), 2.19g's
keyword exclusion (899) and 4.16's quoted keys (1093, refined by 4.1 at
`0592d9b`). Read at main `fcbb8fa` and judged against SPEC's intent, T5's
rubric, the notebook's compass and D-35, D-42, D-51 and D-52. Probes ran
the CLI built at `b39203a`, which is `c8e490f` plus 7.9b's settlement and
touches none of the paths below; no cargo was run. One ruling stands as ruled,
one stands with a layout, one finding is withdrawn, and one is replaced.

- **An admitted no-op takes its revision (7.10, lines 753 and 759;
  `15aa455`). Confirmed.** §5.3 step 6 gives every admitted step revision
  `r+1`, and §5.7 records nothing for a sentence whose fact already holds;
  the PLAN row that read "no revision" was the outlier. A revision counts
  admissions, as Datomic's basis does, so `galileo run`'s revisions stay one
  per admitted line, the log's hash at each revision checks the history,
  and an admitted step's `E` has a revision to go out with. The history
  still changes only where the state does (D-52's option A), and `--since`
  and the chain of title never name such a revision, since no span opens
  there. §5.7 now says so.
- **A block groups its clauses (7.8, line 815; `0aa02f3`). Confirmed, with
  a layout.** One block per occurrence, every distinct clause and
  substitution the proof reaches, each `when` proof on the stored
  pre-state, and one step-core call per block all stand: N2 makes the
  clauses of one event one occurrence, and §7.3 prints each occurrence
  once. But the landed block names its clauses on one line and runs their
  proofs together, so a reader cannot tell which condition held for which
  clause, and one clause under two substitutions prints its location
  twice: measured, `by the clause at subst.gal:13, subst.gal:13`. A block
  with two or more parts now gives each its own `by the clause at
  <file:line>:` line with its proof beneath it, and a block with one keeps
  the clause on the occurrence's line, so no line is added where nothing
  needs separating (compass 7 and 11; T5's principle 10). A fact two parts
  added is credited to the first in canonical order, as `step_causes`
  already does, so source order stays unobservable (compass 8). §7.3 now
  says so. Box 7.8c.
- **A keyword reading can meet a template reading (2.19g, line 899).
  Finding withdrawn.** The lead found that a claim's `requires` and a
  `{a} requires {b}` template could never both type-check on one line,
  given one-token slots and the template-word filler check, and let 2.19g
  test two same-shaped templates instead. D-51 had already ruled that a
  template word is a name in a slot, and 2.8e (`133a601`) removed that
  check. A one-word template is a sentence (a template needs only one
  word), so `build requires test`, with `given build`, `given test`,
  `given {a: Task} requires {b: Task}` and two `Task`s named `build` and
  `test`, has both readings. Measured on main, which lacks 2.19g, the
  line inside `requirements` silently reads as the claim and Fails, while
  the same line at the top level states the template's fact: the silent
  misreading K10 and O1 exist to stop. D-42's rule is unchanged, every
  reading is tried and exactly one must remain, and 2.19g's Done when gets
  this line as its keyword-against-template case. The unlanded 2.19g lane
  (`25979cb`) sits on `2a20336`, before 2.8e, so it must rebase onto main.
  §9.4 now gives the example.
- **A quoted key is a text literal (4.16, line 1093; 4.1). Replaced in
  part.** Mapping a key that is not a name, rather than rejecting it,
  stands: §9.4's `seg` already takes a text literal and `{expr}`, and
  firefox's `"text/html"` needs it. Routing a static key through a computed
  segment does not. The record register keeps a quoted segment's raw
  source text, while the Nix printer escapes what it gets, so one literal
  means two things. Measured: `"a\"b": 1` and `"x\\y": 2` in a
  `nix file` build keys whose `attrNames` are `[ "a\\\"b" "x\\\\y" ]`,
  while `k: "a\"b"` builds the value `"a\"b"`; and `"a{x}": 1` builds the
  key `a{x}` while `{"c{x}"}` builds `cb`. The importer dodges this by
  writing `{"a\"b"}`, a second spelling (4.1 moved escape-free keys to the
  quoted form). A quoted segment now means what its literal means as a
  value, escapes decoded and `{…}` interpolated; braces hold any other
  expression, and `{"…"}` is an error offering the quoted form (compass 1,
  2 and 7). A quoted segment emits a quoted Nix key and `{expr}` emits
  `${…}`, so an imported `"a${x}"` key builds back as written. Firn's pin
  has no static key with an escape, and its only interpolated keys are
  flake.nix's three `"${config.myConfig.modules.users.username}"`. The
  ruling's other half stands: selections by a key that is not a name, and
  `rec`, `inherit` and the rest, stay rejected (D-35). §3.1 now says so.
  Box 4.16b.

Rejected:

- **No revision for an admitted no-op (the PLAN row before `15aa455`):**
  revisions would stop counting admissions, and `galileo run`'s output and
  the log's hash check would need a second numbering.
- **One block per clause and substitution:** prints an occurrence more than
  once, against §7.3's one block per occurrence.
- **A separate line for the clause in every block:** adds a line to every
  single-clause block, which the dungeon's chain shows needs none.
- **Testing only two same-shaped templates for 2.19g:** never exercises the
  keyword reading, which is the box's point.
- **Keeping `{"…"}` for static keys with escapes:** two spellings of one
  key, and it leaves `"a\"b"` meaning another key when written by hand.
- **Decoding quoted keys without interpolating them:** `"a{x}"` would still
  mean one thing as a key and another as a value.

### D-57. North's menu view carries what its screen reads, and a slice is never a host authority by itself

A one-shot Opus xhigh planner's ruling on box 8.9's two `DECISION` lines
(peer file lines 1516 and 1521), read at main `0bc86fc` and north
`d781814`. Neither changes the language.

- **The view carries what the screen reads (box 8.9a).** North's
  `MenuState` holds kind, title, query, help, empty, selection and rows
  of key, label, description, annotation and position, and
  `render_menu`, `render_usage` and the tests read all of them. The
  program already states the title, help and empty texts and derives each
  row's position, so its `Menu` view returns them, each row a record with
  the row entity, and bindgen's `Menu` then has north's shape. Probed with
  the CLI built at `cbe8a26`: `galileo check` exits 0 and bindgen emits
  `Menu {empty, help, kind, query, rows, selection, title}` and
  `MenuRows {annotation, description, key, label, position, row}`. The
  replay compares relations, not views (D-55), so it must still report 0
  mismatches; any other count means the change touched more than the view.
- **A slice is never a host authority by itself (boxes 8.9, 8.14).** At
  `d781814`, 24 handlers outside the slice write its state in their
  `withdraw` and `include` blocks: 21 command and input handlers the
  mailbox, `command-usage`, `command-compact` and `count-usage` the menu's
  kind, title, count and selection, and `settle-new-conversation` and
  `settle-switch-conversation` the active conversation. A Galileo menu
  Session in north's live path would be a second owner of those facts, so
  north moves to Galileo once, for the whole program, at 8.14, and no
  bridge shares them between runtimes. 8.9 measures the replacement on the
  menu's own surface: the lane writes north's menu calls as an adapter
  over the generated API beside the unchanged Clause path, counts it
  against the 116 hand-written lines it replaces, and times one step.

Rejected:

- **Host defaults for the texts, or each row's position from its index in
  the list:** a second copy of the program's rules in Rust, the mirror N4
  removes.
- **An ownership bridge for the mailbox, the shared menu fields and the
  active conversation, persisted across restart:** two owners of one fact,
  and synchronization that 8.14 deletes.
- **A Galileo session fed from Clause's transitions:** the same, hidden.
- **`projected_menu` filling the generated `Menu` from Clause's tables:**
  keeps the hand-written decoding the bindings exist to replace.
- **Timing on hand-built facts and a forged checkpoint:** measures a state
  the program never reached, and restates its row count and allocator.

### D-58. A8's step runs on the engine over a kept model; a field read is one probe, and a ranked aggregate sorts once

A one-shot Opus xhigh planner's ruling on box 8.9's `DECISION` (peer file
line 1581): north's real setup, one `Session::run_all` of 1,003 events,
ran out of the default 10,000,000 work at `menu-row-position` after
122.9 s (north `79d3818`, main `2816fb5`). Read at main `209fa44`,
measured at `5a4f1e3`, whose engine and step code are the same, with a
scratch probe in a release build and real events through `Run::run_all`.
It changes no language.

- **Cause.** Three costs multiply, and a fourth sits under them.
  - `step_outcome` evaluates M(S) and M(S′) for every occurrence with the
    reference interpreter (`crate::evaluate` is `eval::evaluate`), so the
    setup makes 2,006 naive evaluations of growing states.
  - A field read `?x.f` visits every tuple of the field relation, one unit
    each (`expr.rs`). `menu-row-position` reads `?prior.order` for every
    prior row of every row, so one evaluation is Θ(k³): engine work
    20,591, 143,666, 1,072,316 and 8,284,616 at 25, 50, 100 and 200 rows,
    and 1,007,023,016 (15.5 s) at 1,000. The reference also scans
    `?prior menu row visible true` for every prior, so it runs out first:
    the 148th offer exhausts the budget, 160.7 s into the probe's setup.
  - With the reads as one probe each (simulated as joins), the rank sum is
    still Θ(k²): 4,029,014 work and 5.3 s per engine evaluation at 1,000
    rows, so a setup would take about 30 minutes even at one evaluation
    per occurrence.
  - Every step copies whole states. At 1,000 rows (7,010 facts) a step of
    a copy of the program without rules costs 33–40 ms: the `State` clone
    2.1 ms, the hash, which prints the whole state, 6.6 ms, and the
    `Program` and facts copy 3.0 ms, twice per occurrence. A view
    evaluates the model again and projects with the reference's full
    scans (1.3 s), and one clone of the projected 1,000-row value costs
    1.46 ms.

  So no single change reaches 1 ms; each of the six below removes a
  measured cost, and A8 needs all of them.
- **A field read is one probe.** `Queries` answers a field read once: the
  engine from its lazy index on column 0, the reference from its ordered
  set. It charges one unit, since it scans one tuple (SPEC §6.1). `check.rs`
  records a field read as a dependency on every `T.f` relation, which
  `--since` also lacked.
- **A ranked aggregate sorts once, in the engine.** A `count` or `sum`
  whose body holds exactly one ordering comparison between body-local
  variables and outer variables that nothing else in the body mentions is
  computed per group, the binding of the body's other outer variables: its
  rows are produced once, sorted with running sums, and each outer binding
  is a binary search. Keys that are not all `Int` or all `Text` take the
  general path. The reference stays naive, and the differential generator
  learns these aggregates first (G7).
- **The step core keeps its model and runs on the engine.** `Run` keeps
  M(S) of its current state beside it, outside `State`, the checkpoint, the
  hash and equality, and builds it in full at first use after `new`,
  `restore` or `replay`, under its own default budget. Each occurrence
  matches against it. M(S′) is M(S) with only its changed state relations
  replaced when no relation that a rule reads changed, and a full engine
  evaluation otherwise, in which each relation equal to M(S)'s keeps its
  handle; it is the next occurrence's M(S). A step therefore charges only
  what it evaluates, which depends on S and the step alone, so a restored
  session continues with the same results. `state::step` keeps the
  reference interpreter, and a property test checks `Run` against it on
  the recorded sequences (D-16).
- **A step costs its change.** `run_all` applies each occurrence in place
  with an undo log instead of cloning the state, the step clones no
  `Program`, state modes are checked on the relations the step changed,
  and a replacement finds its old value by a range over its relation.
  Amended on 2026-10-11 by a one-shot Opus xhigh planner, who timed 8.9j's
  candidate (`bfe62bb`) section by section under one exclusive lease. Of
  an instrumented 12.7 ms `query-menu` step at 1,000 rows (9.964 ms
  uninstrumented), the full state mode check took 10.1 ms, the two
  `Program` copies with their drops 1.3 ms, three scans of every fact
  0.8 ms, the live-set copy 0.3 ms, and the apply, history and log
  0.04 ms. The mode check alone exceeds the 5 ms that 8.9j was given, so
  that target belonged to 8.9j and 8.9k together. 8.9j holds once
  `overhead_step_ms` falls below 8.9i's 34.735 ms. 8.9k keeps the
  ≤ 0.2 ms end target and now also finds the withdrawn facts, the
  replaced relations and the live set without a scan or copy of the whole
  state, which no box had named.
- **The state hash is a sum of fact hashes.** h(S) is FNV-1a 64 over the
  wrapping sum of each state fact's FNV-1a 64 (of its relation name and
  values as printed today), the wrapping sum of each live entity's, the
  allocator and the pending requests in order. A step updates the sums
  from its change; `State::hash` computes the same from scratch, and the
  history check compares them at every revision. Printed hashes change
  once. No test or recording asserts a hash value, and a replay matches
  entities, never hashes (D-55).
- **A view recomputes only the fields whose relations changed.** A view
  reads the kept model, the session keeps each view's last record with
  the handle of every relation each field reads, and
  `Projection::decode` borrows the record. A step that changes only the
  selection rebuilds only `selection`, and decoding clones each row's
  strings once.
- **A view hands out its unchanged fields shared.** Amended on 2026-10-11
  by a one-shot Opus xhigh planner's ruling on box 8.9l's exclusive result
  (6e9478a on `ea23cc4`: `view_median_ns` 600,006 against 150,000),
  measured with one paired probe under one exclusive lease in a scratch
  copy of 6e9478a. The memo works: 1.00 field is recomputed per view, the
  `selection`, and the memoized projection costs 23,383 ns at 200 rows and
  55,484 ns at 1,000; without it a projection on the kept model costs
  33.2 ms and 697 ms. The time goes to the owned decode. At 200 rows the
  view after a move took 719,416 ns: the decode 384,319, of which the
  allocator's deferred work for the memory the move freed is about 300,000
  (a forced consolidation after the move took 300,122, and the view after
  it 211,235), the rest cold caches (100,508 with no move, 462,866 after a
  cache scrub). At 1,000 rows the decode alone is 1,752,188 ns with no
  move and 3,886,190 after one, and a clone of the record is 1,562,573,
  so no owned copy of 1,000 rows fits A8's 1 ms. The session therefore
  also keeps each field's decoded typed value, dropped when that field is
  recomputed, and a generated view's top-level list fields are
  `Arc<[T]>`: a step that changes only the selection decodes only
  `selection`, and the view after a move cost 58,139 ns at 1,000 rows in
  the probe. 8.9l keeps its memo and lands with its 600,006; 8.9n shares
  the fields. North's adapter and `menu_step.rs` read `rows` through
  `Deref` and stay unchanged.

What stays fixed: SPEC's step semantics, the reference interpreter as the
oracle (D-16), the default budget of 10,000,000 work, the replay at 0
mismatches, and 8.9's measurement as written. PLAN boxes 8.9b to 8.9n do
this, and 8.9 ticks with 8.9m.

Rejected:

- **A construct such as `position of ?row among … by ?row.order`:** the
  program already states rank correctly, and finding it fast is the
  engine's job (SPEC §10.1). It would take the language owner, SPEC text,
  syntax and both interpreters, and A8's timed step reads rank only from
  the kept model.
- **Tuple-level incremental maintenance (DRed, counting, DBSP):** a menu
  step changes no relation a rule reads, so relation-level reuse costs the
  same there for a fraction of the code. It waits for a step that changes
  a relation a large derivation reads (SPEC §12.1).
- **A larger budget, or fewer units per scanned tuple:** the charges were
  right for the algorithms; the algorithms were wrong.
- **A hash computed only when read:** the log records each revision's hash
  (SPEC §10.4), so that changes the log's contract instead of its cost.
- **Caching the typed `Menu` in the session:** it needs `Clone` on every
  generated view and still copies 1,000 rows per step. Keeping each
  field's typed value with list fields shared copies none (amended above).
- **Decoding rows on demand behind accessors:** every generated list
  becomes methods and decode errors move to reads, for no gain over shared
  typed rows in a menu step.
- **North's adapter reading only the changed field:** the adapter cannot
  see which fields changed without a new session API, and 8.9's adapter
  and measurement stay as written.
- **`Arc<str>` text in `Value`:** it touches every crate, and a decode
  still visits and frees 1,000 rows per step.
- **Another allocator:** a dependency that replaces no external format
  (D-20), and it hides the move's garbage, which 8.9j and 8.9k remove.
- **Only lowering 8.9l's target:** the owned decode of 1,000 rows alone,
  1.75 ms with no move, exceeds A8's whole budget.
- **Re-deriving only the affected relations inside an evaluation:** the
  engine would need seeded stores; a menu step re-derives nothing, and an
  offer changes every relation the rules read.

### D-59. A text block's closer continues its line, and a `lines` option is a `nix file`'s `lib.mkOption` call

The language owner's ruling on box 4.2's closing marker (peer file lines
1634 and 1639; lane `firn-constructs-4-2` at `a98acb9`) and box 4.26's
`lib.types.lines` scaffold (1638), with the build failure 4.26 met at
`use firn/tags` (1650). Read at main `209fa44`, rebased onto `14f5bdf`,
whose D-58 touches none of it, and judged against SPEC §2, §3.1, §9.1,
§9.4 and §10.2, T5's rubric, the notebook's compass and D-38, D-39, D-42
and D-45. Probes ran the CLI built at `a98acb9`, which is `209fa44` plus
importer and test changes only; no cargo was run.

- **A text block's closer continues its line (4.2). The lexer and §9.1
  change; the printer is right.** §9.4 makes a text block a
  `value-literal`, an expression anywhere, a call's arguments and an
  inline record included, while §9.1 put its closer alone on its line, so
  only a block that ended its line could close. Firn needs the other kind.
  Imported, `modules/beagle/default.nix` gives `let mkDev = (name) =>
  pkgs.writeShellApplication({…, text: '''` whose closer is `'''})`
  (line 14), and `flake.nix`'s overlays give `'''), extraInstallCommands:
  '''` and `'''})(final.runCommand(…)({})('''`, closers that open the
  next block. A rule that cannot write a Beagle module is wrong (compass
  4, K7). The closer now starts its line after nothing but spaces, still
  2 spaces deeper than the line that opened the block, and what follows it
  continues that line, another block included, which closes at the same
  column. Swift's multi-line strings (SE-0168) close the same way: the
  closing delimiter begins its line, its column sets the dedent, and the
  expression goes on after it, as in `print("""` … `""")`; Java's text
  blocks (JEP 378) also continue after the closer. L1's guard holds: a
  forgotten closer meets the next line of its own depth, which is content
  left of the closer's column, an error at that line. Given up: a content
  line whose first non-space characters are `'''`, which neither firn's
  pin nor any `.gal` in the repository writes (`rg "^\s*'''\S"` finds none), and which a text
  literal still writes. The record printer already puts the closer 2
  spaces under the start of its logical line with the rest after it, so
  `record.rs` does not change; `lexer.rs` tokenizes the rest of the
  closer's line into the same logical line, and `fmt.rs`, which scans
  blocks itself, keeps the rest and prints it after `'''`. §9.1 and §9.4
  now say so (K20). Box 4.2a.
- **4.2's gate compares `flake.nix` by parse identity.** Its two
  one-operand `++` calls have no byte-identical spelling in Nix or Galileo
  (D-38), so §10.2 and box 4.5 already compare it by parse identity. The
  strict gate keeps all seven files and all eight constructs: six
  byte-identical, `flake.nix` parse-identical, nothing skipped.
- **A `lines` option is a `nix file`'s `lib.mkOption` call (4.26). The
  narrow reading stands.** firn's `template submodule` declares
  `extraConfig` with `lib.types.lines`. `lines` holds exactly the strings
  `str` holds and differs only in how NixOS merges two definitions:
  `lines` joins them with newlines, `str` rejects two that differ. A type
  is a contract on values (T5's principle 14, D-42), so a `Lines` type
  would be a second name for `Text`'s contract (compass 2) and would carry
  a NixOS merge policy into the types that T4 keeps open to general
  programs. D-42 cut `lines` from the option grammar with nine other
  words, and no firn module declares one (G1, compass 3). So the scaffold
  is a `nix file` whose value declares `enable` with
  `lib.mkEnableOption(…)` and `extraConfig` with `lib.mkOption({type:
  lib.types.lines, default: "", description: "Extra config text."})`,
  laid out as `galileo import nix` prints that Nix: the entry block
  `options.myConfig.modules.<name>`, then `config when …`, never a written
  `lib.mkIf`, which would be a second spelling of `when` (compass 2).
  Measured: importing the scaffold's Nix prints exactly that `nix file`,
  `galileo check` exits 0, and `galileo build` gives back the input's
  bytes. The other three module kinds stay `nix module`. §10.2 listed
  `lines` among the types to come; it now says why it is not one (K21).
  SPEC §3.2 makes a call to an undeclared `lib` member an error once 2.5b
  lands, so `firn/nix-lib` declares the two calls (D-45). 4.26's row says
  so, and box 4.26a adds the signatures.
- **`galileo build` reads the linked program (4.26, line 1650). Not a
  language question.** §2 has a command read the modules under its paths
  and every module they `use`, §10.2 has `build` compile its program in
  one process, and 2.11c's row says `check`, `build` and `why` all do.
  Measured: a module holding `use firn/tags`, `sample-tool is a Module`
  and a `nix module` checks with exit 0, while `galileo build` fails with
  ``there is no module `firn/tags` to use``, because `emit` parses each
  file alone (`load` in `crates/galileo-cli/src/main.rs`). Box 4.26a.

Rejected:

- **`'''` alone, with the delimiters on the next line (4.2's option a):**
  that line would begin with `})` at its parent's depth, a layout rule
  under which a line starts in the middle of an expression, and the lexer
  would change anyway.
- **Hoisting an inline block into a `let`:** a Galileo `let` emits an
  applied lambda, so the Nix stops being byte-identical, and beagle's
  block reads the lambda parameter `name`, which no outer `let` sees.
- **A `"…"` literal for the text:** emits a Nix `"…"` string, not
  byte-identical for a Beagle output, and puts a shell script on one line.
- **A multi-line layout for records and calls:** new syntax for what one
  lexical rule settles.
- **Only closing brackets and commas after a closer:** `flake.nix` opens
  the next block on a closer line, and a list of allowed tokens is a
  special case the general rule does not need.
- **A `Lines` type, or `option P: Text` with a merge field:** the first is
  a second name for one contract; the second is machinery for one
  scaffold.
- **An `option` type written as Nix (`option P: lib.types.lines`):** the
  second type grammar that D-42 merged away.
- **`Text` (`lib.types.str`) in the scaffold:** changes what firn's new
  modules mean, since two `extraConfig` definitions would then fail
  instead of joining.

### D-60. A single point of failure binds its sentence's variables, as `requires` does

The language owner's ruling on the 6.3 worker's finding (peer file lines
1768 and 1772, `galileo-6.3-result.md`): the requirement `?n holds has no
single point of failure among ?n has ?b` stops before lowering with
"`?n` isn't bound: say which with `some ?n: …`, `every ?n: …` or
`no ?n: …`". Read at main `8819f89` and judged against SPEC §5.6, §6.1 to
§6.4, §8.4, §8.5 and §9.4, D-29, D-44, D-47, box 3.11b's row, T5's rubric
and the notebook's compass. Probes ran the CLI built at `7c63fb7`, whose
reader (`crates/galileo-syntax/src`) is main's; no cargo was run.

- **The error is wrong: SPEC already binds the left side.** §6.2 counts
  the form itself among the binders: in `S requires C`, `S has no single
  point of failure among P`, `S only after C` and `E only when C`, the
  left side's variables are universal, and variables only in `C` or `P`
  are pattern variables. D-47 says the same of the one binding function,
  and 3.11b's row names the left side of `has no single point of
  failure`. §5.6's `?trap strikes ?hero only when ?trap warned ?hero` is a
  SPEC example of a left side read universally, and its single point of
  failure is ground on the left with `?key` and `?room` as pattern
  variables. §8.4's failing instance, "the claim under one binding of its
  universally quantified variables", counts exactly these bindings, so R6's
  comparison needs them. The requirement means: every node that holds
  still holds after removing any one stated fact `?n has ?b` for that
  node.
- **3.11b's ruling agrees.** Its `?x in [Seed, Nowhere] and ?x holds` is a
  bare conjunction, a state claim that no form binds, and a membership
  binds nothing (D-47), so `?x` would be R15's implicit `some`. A binary
  form's left side is not implicit: the form's meaning quantifies it
  universally, as `requires` already does.
- **The error's advice has no correct spelling.** §9.4 gives a quantified
  claim a `conj` body, so `every ?n where ?n is a Node: ?n holds has no
  single point of failure among ?n has ?b` reads only as a single point
  of failure over a quantified claim, a static error by §6.2 (box 3.11c).
  Measured on a scratch module with `A has A1`, `A has A2`, `B has B1` and
  the rule `?n holds when ?n has ?b`: the free form is the static error
  (exit 2), `?n holds requires ?n has ?b` holds for all 2 instances
  (exit 0), `B holds has no single point of failure among B has ?b` fails
  with `?b = B1` (exit 1), and the `every` form is `Unknown(unsupported)`
  (exit 2).
- **The cause is the reader.** Its single-point-of-failure reading in
  `handlers.rs` reads the left side as a claim of its own, so a sentence
  there meets the bare-claim scan for unbound variables, while `requires`
  reads both sides as patterns. The typed IR and `claims.rs` already plan
  the subject's variables universally, and 6.3's typed scenario checks two
  universal subjects. Box 3.11d removes that scan and routes the reader
  through `claims::unbound` (3.11b), so the fix belongs there; a patch now
  would edit code 3.11d deletes (compass 5). Both rows' Done when now
  measure it, and 3.11b's guards against the same mistake one level
  down: `unbound` must not judge the left sentence as a claim of its own.
- **A temporal claim on the left binds as it does alone.** §6.2's table
  reads the sentence form "for every binding of `S` that holds" and the
  temporal form as "the claim must still hold", and `eventually S` and
  `always S` bind nothing. So `eventually ?h is in Vault has no single
  point of failure among ?k lies in ?r` is an error naming `?h` and
  `eventually`, and `never S` on the left keeps its existential
  variables. In a program without events `eventually S` is `S`, so the
  sentence form already says it; over events, robustness per binding of a
  reachability question is an extension no program needs (compass 3).
  Today the reader accepts the temporal case and lowering wraps it in an
  implicit `some`, R15's hole that 3.11d closes.
- **`requires` needs nothing.** The reader already binds its left side,
  as measured above.

SPEC §6.2 now says that a temporal claim on the left binds as it does
alone, and shows the universal requirement beside the `every` spelling,
which is an error. Boxes 3.11b and 3.11d are amended; no new box.

Rejected:

- **Keeping the error and asking for `every ?n …`:** that spelling is a
  static error, so the universal requirement would have none, and this
  form alone of the four would not bind its left side.
- **A box that patches the reader now:** it edits the scan 3.11d removes,
  and 6.3 landed with a requirement that needs no change.
- **Binding a temporal left side's variables:** new semantics no program
  asks for, under which `eventually ?h …` is a claim on the left of this
  form and an error alone.
- **A quantified body that admits a single point of failure:** a second
  spelling of the free form (compass 2).

### D-61. The menu without workarounds: fields for one entity's values, slots named by their types, `in` as a comparison, and cases as `if` chains and tables

The language owner's batch from Tom's direction of 2026-10-11, given after
he saw `examples/north-menu.gal` (198 lines), verbatim: "don't let the
language be ugly. Fix those issues fix the gaps." His proxy showed four gaps
in that file: no case expression, so nested `if … else (if …)` chains; no
membership test, so `(if ?kind == "history" then true else ?kind ==
"archived-history") == true`; fifteen one-value state declarations for what
is one record; and three near-identical `accept-menu` clauses. Tom then
added, through the proxy: `{north: North}` in a template is a smell, so a
slot takes its name from its type (`state Hero is in Room`) and braces stay
only where two slots share a type or need another name; no `{value: T}`
slots, since such a slot means the shape is a field, so the menu's
one-value shapes become fields of a North state record; and the braces in
every `.gal` file are counted before and after. Read at main `1ee816a` and
judged against T4, T5's rubric, the notebook's compass, D-28, D-42, D-46,
D-47 and D-55 to D-60. Probes ran the CLI built in lane
`codex-8.9j-in-place-step` (`0dfa3f8`), whose reader is main's apart from
2.9c's diagnostics; no cargo was run.

**What caused each gap.**

- **Membership already worked.** Replacing `north-menu.gal:157` with `and
  ?kind in ["history", "archived-history"]` checks with exit 0: a
  membership literal whose variable is already grounded tests it, in the
  reader, the reference and the engine alike (`bind` and `extend` compare an
  existing binding). The file does not use it because of the importer.
  Clause conditions have no `or`, so `north.clause` L787 writes `if(?kind =
  "history", true, ?kind = "archived-history") = true`, `clause_menu.rs`
  copies it token for token, and none of 8.8's fifteen moves reached it.
- **So did unparenthesized chains.** `if` reaches as far right as it can
  (§9.4), so `if a then x else if b then y else z` reads, and §9.7's `wrap`
  writes one. Every parenthesis in the file's chains is the importer's: its
  `expression()` wraps each Clause `if(…)` in parentheses. A `define` body
  is already one expression over its indented lines: a three-line `else if`
  definition returns `a`, `b` and `c` for its three cases.
- **The record was missing.** D-47 gave fields only to created entities,
  fixed at creation, so a declared entity's changing values had to be
  shapes, and with no word for the value the template became `{north:
  North} menu kind {value: Text}`. That is the fourteen `{value: T}` slots,
  the `exactly one value per north` lines, ten `initially` sentences, and a
  view that spells each read as `the ?kind where ?north menu kind ?kind`.
- **The three clauses** are what the first three gaps cost: each repeats
  the selected-row join, the mailbox guard and a kind test, and two repeat
  their effect name in two `then` lines.

**Rulings.**

- **A value an entity has one of is a field, and a block says where it
  comes from (SPEC §4.1, §4.2, §4.3, §5.2).** `state for T` declares state
  fields and `derived for T` derived fields, for a `type` or a `state
  entity`. Each is the relation `T.f` with `exactly one f per T`, or `at
  most one` for `maybe`, exactly as a creation field already was, so the
  Core IR, the step, the history, the hash, the engine and the reference do
  not change. A declared entity's state field starts under `initially` as
  `north-main.menu-kind = ""`, a created entity's in its `new` record; a
  handler states the next value with `then ?x.f = e`, which is the
  sentence `T.f(?x, e)` and supersedes under the mode, so the old value
  stays in the history; `= null` withdraws a `maybe` field. A derived field
  is a rule's head `?x.f = e`. `?x.f` reads all three alike, and field reads
  already cost one probe (D-58) and already count as dependencies (8.9h).
  The right side of `=` is a whole expression, not a one-token slot, so the
  importer's outer parentheses go too. The prior is TLA+'s `x' = x + 1`,
  where `then` plays the prime and the right side reads the pre-state
  (A-TLA), and Datomic's cardinality-one attribute (A-Datomic-attr).
- **Creation fields stay fixed (T5's principle 18).** A field under `state
  entity T` is given at `new` and never stated again; `then ?row.label = …`
  is an error that names it and offers a state field. Tom's "fields fixed,
  history permanent" was about created records, and both halves hold: a
  creation field never changes, and a state field changes only as a moded
  state relation always could, by a new fact whose predecessor stays in the
  history. What is new is that the attribute is declared as a field, as Tom
  asked, rather than as a sentence with no words for its value. This is the
  direction's one tension, and the lead may take it to Tom: the other
  option is state fields only on declared types, with a created entity's
  changing values kept as shapes.
- **A `{value: T}` field-shape is an error.** A `state` or `derived` shape
  whose mode keys one entity slot and whose other slot is named `value`
  offers the field. A functional shape whose words say something (`Hero is
  in Room`, `Host runs Platform`) stays a sentence, since the sentence is
  what `why` prints. `tags.gal`'s two `{value: nix Any}` slots are keyed by
  more than one slot, so they stand.
- **A slot is named by its type (SPEC §4.1, §9.1).** In a template, a word
  that begins with a capital is a type the program declares, and declares
  the slot named by the type in lower case with a hyphen before each later
  capital (`MenuRow` gives `menu-row`). Braces stay for a prelude type
  (`{query: Text}`), a type with arguments, a type two slots share (both
  braced), and any other name. A type's name begins with a capital (every
  type in the repository already does), and a template word that must be
  capitalized takes backticks; the corpus has one, SPEC §5.4's `Codex`,
  which becomes the word `codex`. So capitalization alone decides, and a
  `use` can only supply a type a template already names (G5). Slot names
  are unchanged wherever the old name was the type's (550 of the 609 braced
  slots of a declared type in the repository at `c05549c`), so modes, `settled by`, bindings and
  hashes do not move, and the importer and bindgen name slots as before.
  `{room: Room}`, where `Room` gives that name, becomes an error offering
  `Room` once 2.21g has rewritten the corpus, so a slot has one spelling.
- **`in` is a comparison (SPEC §4.3, §9.4).** `e in list` is `true` when the
  list holds `e`'s value, at the precedence of `==` and not chaining, as in
  Python (A-Python-in), so `?north.menu-kind in ["history",
  "archived-history"]`, `not ?k in […]` and `if ?k in […] then …` read with
  no binding and no parentheses. A line whose literal is `?x in e` stays
  membership: the comparison when `?x` is grounded, a generator otherwise,
  one meaning either way. It is `CompareOp::In` in the Core IR, evaluated by
  the one expression evaluator, so the engine and bindgen's view inference
  need no change; in a Nix body a Nix-stage operand is a stage error naming
  `builtins.elem`.
- **Cases are `if` chains and tables (SPEC §9.4).** No `match`: over `Text`,
  `Int` and `Bool` it is a second spelling of an `else if` chain, and T4
  keeps it for sum types, whose arms bind fields. A total case analysis is
  a `define` whose body reads one case per line (`menu-help(kind)`,
  `work-annotation(state, blocked, holder)`, §9.7's `wrap`); a mapping that
  holds only for some keys is a given relation joined under `when`, so a key
  with no fact matches no clause: `accepting a "models" menu requests
  "select-model"` collapses the three `accept-menu` clauses into one (probe:
  a text-keyed given table joins and the clause steps). A table needs no
  construct and `why` cites its line.
- **G10 (SPEC §1.4).** Every example and everything an importer writes
  reads idiomatically, and a construct that forces a workaround is a
  language bug.
- **The importer writes the idioms (boxes 8.6b, 8.6c).** On `north.clause`
  today it writes 263 lines with 73 braces, 19 `{value: T}` slots, nested
  `else (if …)` chains on six lines and three `… == true` tests
  (measured). After: a Clause `one` or `maybe` relation keyed by one entity
  becomes a field named by the relation less the key type's prefix
  (`menu-row-key` of a MenuRow is `key`), a creation field when only its
  entity's creating clause writes it besides one teardown that withdraws
  every fact of the entity, which becomes `not ?row is a MenuRow`, and a
  state field otherwise; a read `?k R ?v` puts `?k.f` in place of `?v`, and
  a second read of the same value becomes `?k1.f1 == ?k2.f2`; a
  withdraw-and-include pair becomes `then ?k.f = e`; `if(c1, true, c2) =
  true` becomes `c1 or c2`, and equalities of one operand to literals
  become `e in [...]`; `x = true` becomes `x`; nested `if(…)` becomes an
  unparenthesized chain; a `++` chain of text and values becomes one
  interpolated literal; a `when` literal that only bound an `exactly one`
  key's old value for its withdrawal goes, and a value compared to one
  literal is that literal; an effect value that two `then` lines repeat is
  bound once under `when`; and a slot named by its type loses its braces,
  as does a shape's `name:` when the template gives the same name. The
  rest stays D-28's mechanical import: every handler `may skip`, the
  maintenance handlers kept, prefix event templates (`event accept-menu
  North`), and the mailbox.

**Audit of every `.gal` file and of the importers' output** (braces are `{`
characters, before and after; "after" is the sketch for north-menu, SPEC
§9.7 for its fixture, and the slot rule elsewhere):

| File | Braces | Finding and ruling |
| --- | --- | --- |
| `examples/north-menu.gal` | 66 → 35 | The four gaps, plus `?effect` bound to compare with `""`, `sum 1` for `count`, `++` for interpolation and a repeated effect name. Rewritten in 8.8b: 198 → 164 lines (`notes/north-menu-sketch.md`) |
| `crates/galileo-syntax/tests/menu.gal` | 19 → 4 | SPEC §9.7's text; it becomes §9.7's new form (2.21h) |
| `crates/galileo-syntax/tests/dungeon.gal` | 27 → 2 | Default slots only; Tom's example is its first `state` line |
| `lib/firn/responsibilities.gal` | 48 → 6 | Default slots only |
| `lib/firn/tags.gal` | 42 → 8 | Default slots; `{value: nix Any}` stands (not a field) |
| `lib/firn/dispatch.gal` | 26 → 15 | 318 backticks are notebook K15's workaround, which D-51 made needless: removing them leaves every answer byte-identical (probe: two `galileo query` answers hash equal). One fact per line with a repeated subject is fine; focus blocks are available |
| `lib/firn/prewarm.gal`, `host-list.gal` | 18 → 13, 3 → 2 | The same backticks; `{policy: Prewarm}` becomes `Prewarm`. A singleton policy entity is fine |
| `tests/external/firn-tags/flake.gal` | 5 → 5 | Fine: `Text` slots and a `{?name}` key |
| `tests/external/kanata/*.gal`, `lib/firn/nix-lib.gal` | 2 → 2, 0 → 0 | Fine: the record register is Nix-shaped by D-38 and D-39 |
| `galileo import clause` (menu) | 73 → about 32 | The importer rules above (8.6b, 8.6c) |
| `bnix_tags.rs` output | — | Backticks every name that equals a template word, K15's rule; it backticks only keyword names now (2.21i) |
| `galileo import nix` output | — | Fine: Nix layout by D-38, D-39 and D-56 |
| All 12 files | 256 → 92 | |

**What stays open.** Notebook O8, the mailbox: the guard "no request
pending" still needs the mirror fields `host-effect` and `effect-payload`,
but 21 handlers outside the slice write the mailbox (D-57), so the
pending-request reading is ruled with the slice that owns it (8.13), and
its machinery would sit in the engine's model, an A8 file. `given for T`,
fields of given data, waits for a program that needs one; dispatch.gal's
47 records are its likely first. `match` waits for sum types (T4).

Rejected:

- **`match` or `case` now:** over text it repeats the `else if` chain with
  a new keyword and arm syntax; with sum types it binds fields and earns
  them (T4).
- **`else` lines that continue a clause's line:** a `define` already holds a
  chain one case per line and names it.
- **Grouped shapes, one header over several `menu kind: Text` sentences:**
  keeps a sentence per attribute and three spellings of each read, and Tom
  asked for fields.
- **A record-valued shape (`North has menu {menu: …}`):** record types are not
  in the MVP, each change rewrites the record with `//`, history and `why`
  lose the attribute, and the replay compares attributes.
- **A Menu entity replaced by `new` each step:** copies every field to change
  one.
- **Field writes on creation fields:** reverses principle 18 for no program.
- **Initial values in the field block (`menu-kind: Text = ""`):** a second
  place that states initial state beside `initially`.
- **`:=`, `set` or `is` for a field's next value:** D-42 removed assignment
  words; `=` already defines a value, and `then` says when.
- **A sentence to write a field and `?x.f` to read it:** two spellings of
  one relation.
- **A slot by type only when a type of that name is in scope:** a `use`
  would turn a template word into a slot (G5).
- **Prelude types as slot names (`Text` as `text`):** the name says nothing
  of the slot's role.
- **One bare and one braced slot of a shared type:** reads lopsided; strict
  first (compass 3).
- **Keeping `{room: Room}` legal beside `Room`:** two spellings of one slot.
- **`in` only as the membership literal:** the importer would bind `?kind =
  ?north.menu-kind` only to reach it, the workaround G10 names.
- **A prelude `elem(x, list)`, or `in` lowered to `count(filter(…)) > 0`:**
  Python's operator is the prior, and `why not` would print the lowering.
- **A derived relation for the selected row:** a relation Clause's tables
  cannot compare, for three body lines that read well.
- **Ruling O8 in this batch:** the menu slice is not the mailbox's only
  writer (D-57).

Boxes, in order: 2.21a (default slots), 2.21i (firn library and the tags
importer), 2.21b (field blocks), 2.21c (stating fields), 2.21h (the §9.7
fixture), 8.6b (importer fields); after 8.9m: 2.21d (`in`), 8.3b (bindgen
reads a field's relation), 2.21e (field facts print as `X.f = v`), 2.21f
(the card), 8.6c (importer expressions), 8.8b (north-menu rewritten, 8.9's
bindings regenerated), 2.21g (the corpus migrated and `{room: Room}` an
error). 13 boxes, 225 minutes.

### D-61 addendum. One notation for a blank, and it keeps its `?`; fact tables, list declarations, `every` posts, and unbound clause variables as check errors

The language owner's ruling on Tom's follow-ups to D-61 of 2026-10-11,
sent through his proxy: whether the accept-menu clause's `?row` is
unbound; whether `?` can go where a handler's head already binds the
variable; five smells in `crates/galileo-syntax/tests/dungeon.gal` (copied
preconditions, type guards, the entity roll-call, names beside templates,
the two registers); a table form for many facts of one shape, and filler
articles; and his question of what the braces in `accepting {kind} menu
requests {effect}` are for. Probes ran the CLI built in lane `owner-sigil`
at `bc78fec`.

**Measured.**

- **The sketch's `?row` is bound** by `and ?north lists ?row`
  (`notes/north-menu-sketch.md`, the `accept-menu` clause); the proxy's
  summary had cut that line. The checker catches half the failure. A `then`
  line that reads an unbound variable errors at its line: ``north-menu.gal:154:3:
  `?row` isn't bound: it must appear in the event sentence, in `when`, or in
  an `is a new` line``. A `when` comparison, boolean expression or binding
  that needs one does not: `galileo check` exits 0 on `when ?row.order >
  0`, and on the work `accept-menu` clause with its row join cut, and
  `galileo run` then stops at the event with `Type { at: "handler clause",
  message: "?row is not bound" }` and no line. A rule names the variable
  but gives its first line: `north-menu.gal:51:1: menu-row-position:
  nothing grounds ?ghost, which ?ghost > 0 needs`. Box 2.21j.
- **`?` counts.** The D-61 sketch has 158, today's `examples/north-menu.gal`
  248, the dungeon 61. Dropping `?` from head-bound variables after the
  `on` line would leave the sketch 85 (73 fewer), and from the `on` line
  too, 55.
- **A list does not yet declare entities:** `[Entrance, Hall] is a Room`
  errors that `Entrance` isn't a declared Room. **Event names are already
  optional:** the dungeon with `move:` cut from its event line runs with
  the same hashes. **The missing domain line** errors with `nothing
  grounds ?hero, which ?door opens for ?hero needs` and offers no fix.
- **Braces.** The 256 `{` in the repository's twelve `.gal` files are 245
  template slots and 11 records, interpolations and Nix keys; the menu
  importer's 73 are all slots.

**Rulings.**

- **Every variable keeps its `?` (SPEC §9.1).** The head's variables
  included, at every occurrence. In a sentence a bare token is an entity or
  a template word, so `?` is what shows, on the line itself, which slots
  are fixed and which are open. Dropping it for head-bound names breaks
  that. The same text would mean two things: `north.menu-kind =
  ""` names the entity under `initially` and the variable inside a clause,
  and the menu's singleton could as well be named `north`; K5 and D-41
  reversed exactly this for views, because an entity `north` would then be
  shadowed. A clause would spell its variables two ways, `north lists
  ?row`, so a reader tracks where each name is bound, and moving a `when`
  line into a rule, a claim or `galileo query`, which have no head, changes
  what it means. Body literals have no order (compass 8), so no occurrence
  of a `when` variable can be marked as the binder: the head is the only
  place the rule could apply, so the two spellings are unavoidable. And
  slot names follow types
  (D-61) while templates hold the words that name their slots, so bare
  variables double words: `north requests host effect effect with payload
  payload`. `?north.menu-kind` costs what Python's `self.menu_kind` costs,
  an explicit receiver. The sketch stays at 158 from this ruling; D-61
  already cut 248 to 158. A `define`'s parameters stay bare, since they are
  names in an expression, not blanks in a sentence (K5).
- **A blank is `?name` in one notation, and braces outside text mean only
  records (SPEC §4.1, §9.4).** A template slot is the bare type that names
  it, as D-61 ruled, or `?name: Type`: `given Door leads from ?here: Room
  to ?there: Room`, `event move-menu North ?delta: Int`. The type reads
  `maybe` and `nix` and one type name with the arguments it takes, so the
  next word is the template's; a declaration always gives the type, so a
  bare `?name` there is an error, and `?room: Room` offers `Room`. A
  template's `?` cannot be read as anything else, because only a shape line
  holds a template; the `name:` before it is a word and a colon. The
  importer writes the same (8.6b). The repository's `{` go from 256 to 92
  under D-61 alone and to 19 with this: the three `new` records and the
  view record in north-menu, Nix's two `{}`, flake.gal's two computed keys,
  and eleven interpolations in text. Slot names, modes, `settled by`,
  bindings and hashes do not move.
- **Fact tables (SPEC §4.2).** A sentence whose every slot is a distinct
  `?name`, with rows indented under it, states one fact per row, the header
  with the row's tokens put in place (A-Gherkin's Examples tables). A slot
  token holds no space outside quotes and brackets (§9.2), so a row splits
  one way; how many spaces separate tokens means nothing, `galileo fmt`
  aligns the columns, and only indentation is read. Constants, repeats,
  `?_` and expressions in the header, rowless headers, wrong-width rows and
  text blocks in rows are errors. The menu's `accepting` block goes from 8
  lines to 9, a header plus six rows under the shape's two, and its facts
  from 324 characters to 274: a table pays from about three rows of two
  columns, and a one-column run is a list (below).
- **Filler articles (SPEC §9.4).** A template's words are its author's; the
  language neither drops nor ignores one, since `a` is also the keyword in
  `is a` and two spellings of one template break compass 2. The idiom is
  G10's: leave out a word no reading needs, so the menu's shape is
  `accepting ?kind menu requests ?effect`.
- **A shared precondition is an `every` post, not a copied clause (SPEC
  §5.2).** `then … and every ?v where C: P` holds `P` for each binding of
  `C`, the claim quantifier `every … where …:` read over the post-state,
  with `C` read on the pre-state as every `then` expression is. The
  dungeon's two move clauses become one: `then ?hero is in ?there` and
  `and every ?trap where ?trap is armed in ?there and ?trap is telegraphed:
  ?trap warned ?hero`. It lowers to one more clause of the event, so the
  step, the engine and the reference do not change. North's
  `clear-menu-rows` becomes `then every ?row where ?north lists ?row: not
  ?row is a MenuRow`, which matches with no rows, so its `may skip` goes.
- **`?hero is a Hero` stays (SPEC §4.3).** It is the domain of the rule's
  universal, as `hero(h)` is in Datalog, not a guard. A head slot's type
  grounding the variable would let a misspelled body variable leave the
  head's ranging silently over every Hero, which compass 10 forbids. The
  missing-domain error offers the line (2.21j).
- **A roll-call is a list (SPEC §4.2).** The list sugar declares one entity
  per element: `[Entrance, Hall, Library, Armory, Crypt, Vault, Shrine] is
  a Room`. The dungeon's 19 declaration lines become 5.
- **A shape's name is written only when needed (SPEC §4.1).** It was
  already optional; it stays where the derived name would collide or an
  outside reader keeps another (north's events, `menu-host-effect`). The
  dungeon's three event names go.
- **The two registers are acceptable as they are (SPEC §9.2).** They are
  two frames over one expression grammar: a sentence of a relation, and a
  Nix entry whose layout NixOS fixes (D-38, D-39). The words they share
  mean the same, `when` a condition and `.f` a field read. A binding is a
  `?x = e` literal in a sentence owner and a `let` in a Nix body, because
  one is found by matching and the other is computed once in order.

| File | Lines | `?` | `{` |
| --- | --- | --- | --- |
| `crates/galileo-syntax/tests/dungeon.gal` | 112 → 91 | 61 → 55 | 27 → 0 |
| `examples/north-menu.gal`, then D-61's sketch, then this addendum's | 198 → 164 → 163 | 248 → 158 → 184 | 66 → 35 → 12 |
| All twelve `.gal` files | | | 256 → 92 → 19 |

The sketch's 26 new `?` replace its 23 braced slots, name the table's two
columns and the `every` post's `?row`.

Rejected:

- **Bare head-bound variables:** the failures above; K5 already ruled
  it for views.
- **`?` only at a variable's first occurrence:** body order is not
  observable (compass 8).
- **An implicit receiver, `.menu-kind` or `its menu-kind`:** a second
  spelling of a field read, and an event with two entity slots has none.
- **Prolog's capitalized variables:** D-61 gave capitals to types.
- **Marking entities instead of variables:** firn's 1,535 declared entities
  are named bare in thousands of facts.
- **Untyped `?name` slots inferred from facts:** a declaration's types
  would depend on facts in other modules; D-51 kept shapes declared.
- **Rows under the shape's declaration:** shapes are exported and facts are
  not (§2), and firn states a library shape's facts in host files.
- **Pluralized roll-calls, `… are Rooms`:** `Rooms` would be a capitalized
  word that names no type (D-61), and English plurals (`Heroes`) are what
  agents misremember.
- **`also when … then …`:** a new keyword and three lines where `every`,
  already a claim word, takes one.
- **Optional articles in matching:** two spellings of every template, and
  `a` is half of the keyword `is a`.
- **A `let` in the sentence register:** `?x = e` is already the binding, and
  a sentence owner's bindings have no order.

Boxes, in order: 2.21k (`?name: Type` slots) after 2.21a, then 2.21l (fact
tables) and 2.21m (list declarations); 2.21n (`every` posts) after 2.21c;
2.21o (the dungeon) after those; after 8.9m, 2.21j (unbound clause
variables are check errors, at the literal's line). Amended: 2.21a, 2.21f,
2.21g, 2.21h, 8.6b, 8.8b. 6 boxes, 100 minutes.

### D-61 addendum 2. `all` takes an expression

The language owner's ruling on the G10 smell its D-61 addendum reported,
which Tom answered through his proxy with "fix it": the sketch's `Menu`
view binds `?entry = {…}` only because `all` takes variables, not an
expression. Read at `812fe17` in lane `owner-all`; the probes were `rg`
over the repository and the firn and north pins.

**Measured.** `all ?x where C` returns one row per distinct binding of
`?x`, the condition's other variables existential, and its sort key may
read only `?x` and outside variables (D-40, K1). So the view names a record
in order to return it, and sorts by the record's field:
`all ?entry where ?north lists ?row and ?row.visible and ?entry = {row:
?row, …, position: ?row.position}, sorted by ?entry.position`. An
aggregate already reads an expression up to its first top-level `where`
(`sum ?c where …`, `rules.rs`'s `binding`). Nothing in the repository,
firn's pin or north's Clause names anything `all` in an expression (`rg`
finds `all-zero`, template words and text). The other `all` queries
(§9.7's `rows`, `flake.gal`'s two `for` sources, firn_dispatch's
question and the tests') name variables and keep their meaning.

**Rulings.**

- **`all` takes an expression (SPEC §4.5, §9.4).** In `all e where C [,
  sorted by k]` the head `e` is computed once for each distinct binding of
  its local variables, those it reads that are not outside the query
  (§4.3's scope function). `all ?x where C` therefore means what it did,
  and a list of two or more variables still gives D-40's records. The view
  becomes `rows: all {row: ?row, key: ?row.key, …, position:
  ?row.position} where ?north lists ?row and ?row.visible, sorted by
  ?row.position`. Python's comprehension `[e for … if c]` is the prior
  (A-Python-comprehension).
- **A row is a binding, not a value.** Two bindings with one value give two
  rows, so `all ?row.label where ?north lists ?row` gives a label per row,
  as a view's rows need, since two rows that print alike are still two
  rows. That keeps K1: a sort key reads only the head's local variables and
  outside ones, so each row has one key. Distinct values are asked for by
  naming them: `all ?label where ?north lists ?row and ?label =
  ?row.label`. Without `sorted by`, an expression head's rows come in the
  canonical order of their values, which for one variable is the order
  they had.
- **A head reads at least one local variable,** so `all ?x where …` with
  `?x` outside stays an error (K2), and `all "x" where …` is one. `the`
  keeps its one variable.
- **One spelling.** A record head whose two or more fields are each `name:
  ?name` is an error that offers the variable list in field-name order,
  which gives the same rows in the same order, as `{room: Room}` offers
  `Room` (D-61).
- **No line gets two readings, and `all` is a keyword wherever an
  expression is due (G5, SPEC §9.1, §9.4).** With the head ending at the
  first top-level `where` (below), every form probed keeps one reading:
  `all ?x where …` gives the same rows read as a variable list or as an
  expression, `all ?p, ?v` is a variable list because a lone variable's
  head would end at the comma with no `where`, and a query in a head is
  parenthesized. The near miss is `all (e)`. Today `all` opens a query
  only before a variable, so `all(xs)` calls a function named `all`, and
  in a Nix body `with lib:` supplies nixpkgs' `lib.all` at the Nix stage,
  where Galileo cannot see it. Deciding by a `where` further along would
  make one token's kind depend on distant ones, so `all` always opens a
  query: `all(xs)` without `where` is an error, and a name `all` is
  written `` `all` ``, as §9.1 writes every keyword used as a name.
  Template words stay words, so `derived all set` still reads.
- **A head ends at the first top-level `where`,** as an aggregate's value
  does, since `where` is no operator. An `if`, `with` or lambda in a head
  stops there, and a query in a head is parenthesized, as a query inside
  any larger expression is (§9.4).
- **In a Nix body** a head is computed by Galileo with its query, so a
  Nix-stage operand in it is a stage error naming the operand, as for `in`
  (D-61).
- **Types and the Core IR.** `all e where …` is a `List` of `e`'s type; a
  record head gives the record type the binding gave, and bindgen names the
  row struct by view and field as before, so 8.9's generated API does not
  move. `ir::Query` carries the head, and lowering records its local
  variables as the query's declared ones, so the checker, the reference
  interpreter and the engine share one reduction (D-40), and every reader
  of `QueryKind::All` keeps working.

| `notes/north-menu-sketch.md`'s block | Lines | `?` | `{` | `rows` line |
| --- | --- | --- | --- | --- |
| After the D-61 addendum, then this one | 163 → 163 | 184 → 182 | 12 → 12 | 230 → 208 characters |

**What stays open.** The sketch's `?visible` and `?rank` bind an aggregate
only because `count where` is a literal, not an expression: the same smell
in rule bodies, left for its own ruling (notebook O10).

Rejected:

- **Distinct head values (strict set-builder):** a sort key would need the
  head to determine its binding, which no check can see, and two rows that
  print alike would merge in a view.
- **Every local variable distinct, as `sum` ranges:** `all ?x where ?x
  costs ?a` would list `?x` once per cost, changing every existing query.
- **A head after the condition (`all ?row where …, as {…}`, SQL's
  `select`):** a second place and a new keyword for what is now the first
  thing a reader sees.
- **Sort keys that read any condition variable:** K1's invisible choice
  returns.
- **Dropping the variable list:** `for ?a, ?b in` binds its query's
  variables in order, and `galileo query` questions name their columns by
  it.
- **Deciding `all (` by a later `where` instead of reserving `all`:** a
  token's kind would hang on distant tokens, for no program that names
  anything `all`.
- **`the` with an expression:** no program asks for one, and a `maybe` head
  would nest `maybe` in `maybe`.

Box 2.21p after 8.9m; 8.8b now waits for it. Amended: 8.8b. 1 box, 20
minutes.

### D-61 addendum 3. `count where` is an expression

The language owner's ruling on notebook O10, which Tom ruled gets fixed
(through his proxy, 2026-10-11), with his standing rule that a binding that
exists only because a construct is not an expression is fixed without
asking. Read at `6c39c53` in lane `owner-count`; the probes ran the CLI
built in lane `owner-all` at `8882f02`, and `rg` over the repository and
the firn and north pins.

**Measured.**

- The sketch's `menu-count` and `menu-row-position` bind `?visible` and
  `?rank` only to carry a `count where` into the head: an aggregate is a
  literal, `?n = count where …` (SPEC §4.3, §9.4's `literal`), and the head
  `?x.f = e` reads an expression. §9.7's `menu-count` binds `?count` the
  same way, and the menu importer's output binds `?matched` and `?rank` for
  its two `then` lines, written `sum 1 where` (Clause's `sum 1.0 given … as
  ?v`), though D-40 made `count` the word.
- `count`, `sum`, `min` and `max` are also names. The prelude lists them
  (§3.5, `check.rs`'s `PRELUDE`); SPEC writes `count(all ?m where ?m is on,
  sorted by ?m)` and `count - 1` (§9.4); the sketch, §9.7 and the card
  declare `define wrap(index: Int, count: Int)`; firn's Clause calls
  `count(…)` on lists 15 times (`native/prewarm.clause`, `firn.clause`,
  `firn-host-list.clause`, the machine-capacity skill), which their ports
  carry over. North's Clause writes the words only as variables, template
  words and its 19 `sum` aggregates.
- Today `?n = max (?a.x) where …` reads as an aggregate whose value is
  parenthesized, while elsewhere `max(` is a call. No `.gal` file, test or
  card writes an aggregate value that starts with `(`.
- A head slot already takes a list: `rule default-plan: ?command plans
  ?executable with [?node, ?edge, ?argument]` checks and answers as
  `lib/firn/dispatch.gal`'s `?arguments` binding does (`galileo query`).
- `for ?name, ?url in all ?name, ?url where …` builds what flake.gal's
  `all ?n, ?u` builds (`galileo build --stdout`).

**Rulings.**

- **An aggregate is an expression (SPEC §4.3, §9.4).** `count where C`,
  `sum e where C`, `min e where C`, `max e where C` and the argmin `min ?x
  by e where C` (and `max ?x by`) stand wherever an expression is due: a
  rule's head, a binding, a comparison, a `then` line, a view field, a
  `define` body, a Nix body. The aggregate literal goes, and `?n = count
  where …` is the binding of that expression, meaning what it did. The
  sketch's rules become `rule menu-row-position: ?row.position = count
  where ?north lists ?prior and ?prior.visible and ?prior.order <
  ?row.order` under `when ?north lists ?row`, and `rule menu-count:
  ?north.menu-count = count where ?north lists ?row and ?row.visible` under
  `when ?north is a North`. Soufflé computes `count : { … }` inside
  arithmetic, and Python writes `sum(… for … if …)` in place (A-Souffle,
  A-Python-comprehension).
- **Its variables scope as a query's do (SPEC §4.3).** Its condition and
  value open a nested scope wherever it stands. A rule's head, and a
  clause's `on` and `then` lines, are their owner's top level outside
  their nested scopes, so a name that also appears there or in the body is
  shared and grounded first, and every other name is local. In
  `menu-row-position`, `?north` and `?row` are shared and `?prior` is local,
  the scopes the bound form had. An aggregate in a `then` line reads the
  pre-state, as every `then` expression does. The scope function does not
  change.
- **One reading, by the next token (G5, SPEC §2, §9.4).** `count` opens an
  aggregate only before `where`, and `sum`, `min` and `max` only before a
  token that begins an expression other than `(`; otherwise each is a name.
  After a name come an operator, `.`, `(`, a closing bracket, `,`, `:`,
  `then`, `else`, `when`, `where` or the line's end, and of those only `(`
  also begins an expression, so `(` is given to the call. The deciding token is adjacent,
  unlike the `where` addendum 2 refused to wait for after `all (`, and
  programs do name things `count`, unlike `all`. An aggregate's value ends
  at its first `where`, so it never needs outer parentheses: `max (?a.x)
  where …`, an aggregate today, becomes the call and a `where` that ends
  nothing, an error offering `max ?a.x where …`, and no program writes it.
  An aggregate in an `all` head, a sort key or another aggregate's value is
  parenthesized, as a query is.
- **An empty `min` or `max` is `null` (SPEC §3.5, §4.3).** An expression
  has a value, so `min`, `max` and the argmin are `maybe T`, `null` when
  nothing matches, as `the` is with no binding and as §3.5 makes every
  partial operation. D-40's "binds nothing, so the literal fails" belonged
  to the literal; keeping it would give `min` one meaning in a rule and
  another in a view. `count` and `sum` stay `Int`, 0 when nothing matches.
  No program writes `min` or `max` (D-40: north's Clause aggregates are 19
  counts, firn's none), so only tests and the card's `favourite` change.
- **A `null` head derives no fact (SPEC §4.3).** A rule head `?x.f = e`
  whose `e` is `null` derives no fact, so a `maybe` field reads `null`, as
  `then ?x.f = null` withdraws one (§5.2). It also answers what `?x.f =
  the ?y where …` already asks of 2.21c. North's FIFO head (N3) is one
  line, `rule queue-head: ?queue.head = min ?item by ?item.order where
  ?queue holds ?item`, under `derived for Queue` with `head: maybe Item`. A
  field that is not `maybe`, or a sentence slot, given a `maybe` expression
  is an error offering a `maybe` field.
- **Lowering keeps the IR (D-58).** An aggregate in a rule or a clause
  lowers to the IR's aggregate literal binding a fresh variable no source
  can spell, in the innermost condition that holds it, so the menu's IR is
  today's with `?rank` renamed, and the engine's ranked count and the
  replay see what they see now. Elsewhere it lowers to `the` over that
  literal. `why` prints it where it was written.
- **`the` shares no gap.** It is a query, already an expression (§4.5),
  and a `maybe` one in a field head now has its meaning (above).

**The scan (G10).** Every binding in the repository's twelve `.gal` files,
the sketch, SPEC's examples and the importers' documented output:

- Fixed: the sketch's `?visible` and `?rank`, §9.7's `?count` and §4.3's
  example. The menu importer's `?matched` and `?rank` become `count where`
  in their `then` lines (8.6c).
- Fixed as a missed idiom: `lib/firn/dispatch.gal`'s three `?arguments =
  [...]`. The head slot already takes the list, so nothing forced the
  binding (2.21q writes the list in the head).
- Kept: the sketch's `?description` (`offer-work-row`) and `?annotation`
  (`offer-history-row`). `if` is already an expression and fits the `new`
  record; the binding names a long value so the record, which stays on one
  line (§9.4), stays readable. Nothing forces it.
- Replaced by other boxes: `examples/north-menu.gal`'s `?order`,
  `?reference`, `?conversation`, `?key`, `?visible`, `?matched`, `?rank`
  and `the` view fields (8.8b copies the sketch), and
  `crates/galileo-syntax/tests/menu.gal`'s `?count` (2.21h copies §9.7).
- Not this smell: kanata's `let cfg = config.myConfig.modules.kanata`, a
  Nix body's name for an option path read eight times, emitted
  byte-identically; the Nix lift's `let`s, which mirror Nix's own (D-8);
  the tags importer, which writes facts. `flake.gal`'s two `for ?name, ?url
  in all ?n, ?u where …` and §9.6's inner loop bind no expression: the
  `for` destructures its query's rows, and the same names work in both
  places, so the renaming is a missed idiom, and the repetition left
  (`for ?name, ?url in all ?name, ?url where`) is notebook O11, for its own
  ruling.

| `notes/north-menu-sketch.md`'s block | Lines | `?` | `{` | `menu-count` and `menu-row-position` |
| --- | --- | --- | --- | --- |
| After D-61 addendum 2, then this one | 163 → 161 | 182 → 178 | 12 → 12 | 6 → 4 lines |

Rejected:

- **Reserving `count`, `sum`, `min` and `max` wherever an expression is
  due, as `all` is:** it breaks `wrap(index, count)` in the sketch, §9.7
  and the card, SPEC's `count(…)` and the ports of firn's `count(list)`,
  and agents name parameters `min` and `max`.
- **Deciding `sum (` by a `where` after its closing parenthesis:** a
  token's kind would hang on distant tokens (addendum 2's reason).
- **Cutting the prelude's four functions:** firn's ports call `count(list)`,
  and an aggregate over `?x in xs` merges equal elements, so it is no
  list's sum.
- **A `let` in rules, or `where ?rank = …` after a head:** a second place
  for a binding, and the binding is the smell.
- **Keeping "an empty `min` fails its literal":** one meaning in a rule,
  another in a view, and an expression without a value breaks §3.5.
- **An `Expr::Aggregate` in the IR:** the engine's ranked count reads the
  aggregate literal (D-58), and A8's 1 ms rests on it.
- **Narrowing in rule bodies (`and ?best != null`):** no program needs it
  yet, and a `maybe` field takes the value (compass 3).

Boxes: 2.21q after 2.21p and 2.21c; 2.21r after 2.21q and 2.19b. 8.8b and
8.6c wait for 2.21q. Amended: 8.6c, 8.8b. 2 boxes, 40 minutes.

### D-61 addendum 4. Write it once

The language owner's ruling on notebook O11, which Tom ruled gets fixed
(through his proxy, 2026-10-11), and on his standing rule given with it:
a smell that makes the author write something twice, or write ceremony
(repeated names, helper bindings, filler syntax), is fixed by the language
owner without asking, and only a fix that would change the language's
direction or break programs beyond an in-repo rewrite is escalated. Read at
`8f70152` in lane `owner-once`: every `.gal` file, SPEC's examples, the
card and the importers' documented output (`bnix_tags.rs`, D-61's menu
importer rules). No cargo was run; the counts are `grep` and a throwaway
script over the files and the sketch's block.

**Measured.**

- `for ?name, ?url in all ?n, ?u where ?n flake input url ?u` declares two
  names for each column and binds them by position (`flake.gal` twice,
  §9.6's two loops, one of them a single variable: `for ?module in all ?m
  where ?m is enabled on whiterabbit`). A `for` with several variables
  already takes only a query written in place (§9.6), so the second list
  is pure repetition.
- Records repeat names: a field given the variable of its own name (`key:
  ?key`, eight in the sketch's two `offer-*-row` records) or a field read of
  its own name (`key: ?row.key`, five in the view's `rows`, and §4.5's
  example). 18 such fields in the sketch.
- Repeated subjects: `north-main.` 10 times in the sketch's `initially`,
  `whiterabbit` 9 times in §9.6's host file, `firefox` 3 times in its module
  file, each of `dispatch.gal`'s 47 commands 6 or 7 times (308 lines),
  `` `prewarm` `` 11 times, `` `host-list` `` 3 times, and the tags
  importer writes a host's and a module's facts one subject per line.
  `prewarm.gal`'s 4 `chooses lease` facts, 2 `targets` and 2 `recognizes
  rebuild`, `dispatch.gal`'s 10 `is common command` facts and `flake.gal`'s 3
  `follows` facts repeat one template per row. Roll-calls one line per
  entity remain in §9.6 (`config/tags.gal`, `config/platforms.gal`),
  `responsibilities.gal` (`boot`, `environment`) and the tags importer's
  tag and platform files, though the D-61 addendum made a roll-call a list.
- §9.7's `clear-menu-rows` still copies the clause with `may skip`, which
  the D-61 addendum replaced by an `every` post in the sketch only.

**Rulings.**

- **A `for` binds its query's variables in place (SPEC §4.3, §9.4, §9.6;
  notebook O11).** `for ?name, ?url where C [, sorted by k]` iterates the
  rows `all ?name, ?url where C [, sorted by k]` returns, one per distinct
  binding of the listed variables in the query's canonical order, and binds
  each listed variable directly; `C`'s other variables are existential, as
  in `all`. One variable is the same form: `for ?module where ?module is
  enabled on whiterabbit`. `for ?x in e` keeps iterating a list value, one
  variable, a query with an expression head included (`for ?label in all
  ?row.label where …`, a label per row). The listed variables are declared
  variables (§4.3): each must be grounded by `C` and may not reuse an
  outside name. Errors at their line: a `for` over an `all` whose head is a
  variable or a variable list, which offers the `where` form with the
  query's names (`for ?name, ?url in all ?n, ?u where …` offers `for ?n,
  ?u where …`), and `for ?a, ?b in e` over anything else, which offers one
  variable and its field reads. G5: the token after the variable list,
  `in` or `where`, decides the form, and nothing later changes it; inside
  `C`, a membership `?x in e` is a literal as anywhere. `galileo query`
  keeps `all`: its question `all ?name, ?url where …` prints, column for
  column and in order, the rows the `for` iterates, so a `for` header
  becomes a question by putting `all` where `for` stood. Lowering builds the
  query it built before with the listed names as its variables, so the
  record printer, the Nix it emits and `galileo build --stdout` do not move.
  Prior: Python's `for name, url in …` binds once (A-Python-comprehension),
  and set-builder notation names its variables once.
- **A record field is written once (SPEC §4.5, §9.4).** In an inline record
  an element `?x` is the field `x: ?x`, and `?x.f` is the field `f: ?x.f`:
  `?row is a new MenuRow {?key, ?label, ?description, ?annotation, order:
  ?north.menu-row-count}`, and the view's `all {?row, ?row.key, ?row.label,
  ?row.description, ?row.annotation, ?row.position} where …`. The priors
  are SQL's select list, where `r.key` names its column `key`, Nix's
  `inherit key;` and `inherit (row) key;`, and JavaScript's and Rust's field
  shorthand. Written out, `x: ?x` and `f: ?x.f` are errors offering the
  short element, so a field has one spelling (as `{room: Room}` offers
  `Room`). It reads only `?` variables: a bare name stays `name: e`, since
  `({self, nixpkgs, ...}) => …` is a lambda's formals and would otherwise be
  told from a record only by a later `=>`. Entries (`attrpath: expr` lines
  in Nix bodies) are not records and do not change. G5: inside braces an
  element begins with a name or a quoted key (`name: e`), `{` (a computed
  key), or `?` (the short element), so one token decides; an entry line's
  `{?name}` is a key segment and an expression's `{?name}` a record, two
  grammar positions that never meet. In an `all` head, a record of two or
  more lone variables (`{?p, ?v}`) is D-61 addendum 2's error offering the
  variable list, which it extends to the short spelling.
- **A subject is written once (SPEC §4.2).** A focus block's lines are its
  subject's facts with the subject left out: a predicate (`leads from
  Entrance to Hall`), `is a T`, which declares the subject, a field fact `f
  = e` under `initially` (`north-main` above `menu-kind = ""`), and a fact
  table whose header is a predicate with the subject left out and whose rows
  sit under it. Two or more facts in a run that share a subject are written
  as a focus block (G10), so the sketch's `initially` states `north-main`
  once, §9.6's host file `whiterabbit` once, and each of `dispatch.gal`'s
  commands its name once. G5: a focus line is read with the subject put
  first, as today; `f = e` has one reading, since `=` is no template word,
  and a predicate with variables is a table header when rows follow it and
  an error otherwise, as a header with no rows already is. D-61's audit line
  that one fact per line with a repeated subject "is fine" is withdrawn
  under Tom's rule.
- **A template is written once (SPEC §4.2).** Three or more facts of one
  template in a run are a fact table, and a one-column run a list (the D-61
  addendum's idiom, now applied everywhere): `flake.gal`'s `follows`
  facts, `dispatch.gal`'s `is common command` facts, `prewarm.gal`'s
  `chooses lease` facts under its focus block, and a roll-call is one
  `[…] is a T` line per type.
- **§9.7 empties the menu with the `every` post** the D-61 addendum ruled,
  so its `clear-menu-rows` loses its `when` line and `may skip`.

**The sweep.** Fixed, by the rulings above: `flake.gal`'s two loops and its
three facts; §9.6's two loops, its firefox and whiterabbit files, its tag
and platform roll-calls and the `why` tree's line numbers that follow; the
sketch's 18 repeated record names and 10 `north-main.`; §4.5's record
example; §9.7's `clear-menu-rows`; `dispatch.gal`'s 47 subjects and its 10
common commands; `prewarm.gal` and `host-list.gal`'s subjects and
`prewarm.gal`'s 8 repeated templates; `responsibilities.gal`'s two domain
declarations; the tags importer's per-line subjects and roll-calls; the menu
importer's records (8.6c).

Kept, each with its reason:

- **Quantifier variable lists** (`every ?host, ?software where …:`, `no ?h,
  ?s, ?m: S`, the `every` post): the list is the one place each name is
  declared, the form Tom's own example keeps for `for`, logic's ∀x and ∃x
  always name their variable, and without it `every ?host adds ?module to
  ?tag: …` reads as a claim about every host.
- **Two requirements sharing one condition** (`tags.gal`, §9.6): each line
  is one requirement with its own verdict and failing instances (D-29), and
  box 5.5 names the second; `every … where C: A and B` would merge two
  verdicts into one.
- **Rules sharing a head**, and **rule names** (the dungeon's two `opens
  for` rules, `tags.gal`'s `supported-*`, `responsibilities.gal`'s four
  `incomplete-*`, `dispatch.gal`'s three `*-plan` and two `*-leaf`;
  `rule menu-count: ?north.menu-count = …`): each case is a named rule,
  and the name is the label `why` prints and `--via` takes (D-51); a body
  has no `or` (§4.3), Datalog's prior, and a name derived from the head
  (`is-enabled-on-by-default-through`) labels a proof worse than the
  author's (`enabled-by-default`).
- **A shape declared and then used** (`event Hero moves through Door` and
  `on ?hero moves through ?door`; a `derived` shape and its rule's head):
  D-51 keeps shapes declared.
- **Explicit receivers in `then` lines** (`?north.menu-kind = …`): the D-61
  addendum's ruling.
- **North's `menu-` field prefixes and the view's `kind:
  ?north.menu-kind` lines:** the field names are the relations the replay
  maps to Clause's (`North.menu-kind` to `menu-kind`) and that 21 handlers
  outside the slice write (D-57), and the view's names are 8.9's generated
  API. A Menu entity is the call of the slice that owns them (8.13).
- **The mailbox mirror** (`host-effect`, `effect-payload` beside the effect
  with the same values): notebook O8, by D-57.
- **An `option` line's whole path** (`myConfig.modules.kanata.` five times
  in kanata's `default.gal`): each line mirrors the Nix line it emits,
  `options.myConfig.modules.kanata.devices = …` (D-38, D-39), and 6 of
  firn's 213 module `default.nix` files declare more than one option.
- **The sketch's `?description` and `?annotation`** and kanata's `let cfg`:
  D-61 addendum 3 kept them; each is now read once, as a short element.
- **Mode lines that name slots** (`exactly one room per hero`): a
  functional dependency is a statement of its own, not a second spelling.

Escalated, not ruled:

- **Posts shared across events.** The sketch's three `offer-*-row` clauses
  each state `?north.menu-row-count = ?north.menu-row-count + 1`, `?north
  lists ?row` and `order: ?north.menu-row-count`. Removing the copies needs
  either a construct by which one event's step states another event's posts,
  which changes D-42's one atomic transition per event (direction), or one
  `offer-row` event with north computing the description and annotation,
  which changes 8.9's generated API and north's adapter outside this
  repository. Notebook O12.

| File | Lines | `?` | `{` |
| --- | --- | --- | --- |
| `tests/external/firn-tags/flake.gal`, today, after 2.21g's slots, after this addendum | 14 → 14 → 15 | 17 → 20 → 19 | 5 → 2 → 2 |
| `crates/galileo-syntax/tests/dungeon.gal`, today, after 2.21o (SPEC §5.6), after this addendum | 112 → 91 → 91 | 61 → 55 → 55 | 2 → 0 → 0 |
| `notes/north-menu-sketch.md`'s block, after addendum 3, after this one | 161 → 162 | 178 → 178 | 12 → 12 |

`flake.gal`'s two `for` lines go from 13 `?` to 9 and from 133 characters
to 119; its table adds a line and its header's three `?`, and writes
`flake input … follows … from` once instead of three times. The sketch
keeps its `?` and `{`: the short elements keep every variable, and its
characters go from 5,871 to 5,655, `rows` from 208 to 156, its `new`
records from 191, 136 and 136 to 178, 99 and 99, its `initially` block from
343 to 266, its lines over 120 characters from 7 to 5, and its repeated
record names from 18 to 0. The dungeon has nothing this addendum changes.

Rejected:

- **`for ?name, ?url in all ?name, ?url where …`, the same names twice:**
  it builds the same file today (addendum 3's probe), and it is the
  repetition.
- **`for where C` with no variable list, the condition's variables bound
  implicitly:** which variables a row distinguishes would be invisible, and
  `galileo query` names its columns by the list.
- **Positional destructuring kept for other lists:** a record's fields have
  no order but their names' (§4.5), so position would bind by spelling.
- **Short elements for bare names (`{name}`):** a lambda's formals would be
  told from a record by a later `=>` (addendum 2's reason).
- **Implicit receivers or a `with ?north:` block in clauses:** the D-61
  addendum's ruling.
- **A grouped `option P` block:** a second layout for 6 of 213 modules,
  unlike the Nix each line emits.
- **Dropping quantifier lists:** above.
- **`or` in rule bodies to merge cases:** loses the case's name in `why`.
- **Rule names derived from the head:** above.
- **`given for Command` records for `dispatch.gal` (notebook O9):** the
  focus block already writes each name once; O9 stays open for a program
  where fields beat sentences.

Boxes: 2.21s (`for … where`) after 2.21p; 2.21t (short record elements)
after 2.21q; 2.21u (focus blocks take declarations, field facts and tables)
after 2.21l and 2.21c; 2.21v (the firn library and `flake.gal`) after
2.21s, 2.21u, 2.21m and 2.21g; 2.21w (the tags importer) after 2.21u and
2.21m. Amended: 2.21h (§9.7's `every` post), 8.6c (short elements) and
8.8b (the sketch's counts, and 2.21t and 2.21u). 5 boxes, 100 minutes.

### D-61 addendum 5. One row offer

The language owner's ruling on notebook O12, after Tom's proxy ruled it
on 2026-10-11 as an engineering call: D-42's one step per event stays, so
no event states another event's posts; the three `offer-*-row` handlers
become one `offer-row` event, with every caller updated in the same
change, north's included, since north is Tom's repository; and A8's step
target stays, measured on the merged event. Read at `2cf9917` in lane
`owner-offer`, with north's lane `galileo-8-9-measure` at `6ffa8c6` read in
place. No cargo was run; the counts are a throwaway script over the
sketch's block.

**Measured.** The sketch's three offer clauses, 15 lines and 1,015
characters, each state the counter's increment, `?north lists ?row` and
`order: ?north.menu-row-count`, and differ only in the values their `new
MenuRow` record is given. Those values come from three places: a setting's
four texts arrive as shown; a work item's label, description and
annotation are functions of its seven arguments alone, which north's
`work_items.rs` already half formats (it sends `clock` as `3m` and `eta`
as `5m` or `?`); and a conversation's annotation reads
`?north.active-conversation`, which only the program holds (D-57). The
callers are `examples/north-menu.gal`, `menu_scale.rs`, `runtime.rs`,
`step_agreement.rs` and `north_menu_replay.rs` in galileo, and north's lane
adapter `src/galileo_menu.rs` (64 non-blank lines, three row loops) with
its regenerated `src/menu_bindings.rs`; at 8.14, `clause_state.rs`'s three
offer transitions (lines 464, 487 and 506) follow. The two recorded
sequences hold 6 `offer-history-row` occurrences, 1 `offer-settings-row`
and no `offer-work-row`.

**Rulings.**

- **One `offer-row` event carries a row's four texts (SPEC §5.2, §9.7).**
  `event offer-row North ?key: Text ?label: Text ?description: Text
  ?annotation: Text`, with one clause:

  ```text
  on offer-row ?north ?key ?label ?description ?annotation
    then ?north.menu-row-count = ?north.menu-row-count + 1
    and ?north lists ?row
    and ?row is a new MenuRow {?key, ?label, ?description, annotation: if ?north.menu-kind in ["history", "archived-history"] and ?key == ?north.active-conversation then "current" else ?annotation, order: ?north.menu-row-count}
  ```

  What the host knows arrives as an argument, and what the state holds is
  read in the clause. North composes a work item's label
  (`"{reference}  {title}"`, two spaces), description (`No holder`, or
  `{holder} · {clock} held / {eta} ETA`) and annotation (`closed`, else
  `blocked`, else `ready` with no holder, else `held`) beside the clock
  and eta it already formats; it sends a conversation's annotation as `""`
  and a setting's texts as before. The program marks the active
  conversation's row `current` in a conversation menu, as
  `offer-history-row` did. The clause always matches and no recorded offer
  skipped, so it has no `may skip` (8.8's rule); the work offer's `may
  skip` came from the import, with no recording to remove it. Clause's copy of the work formatting (`north.clause`
  L652–670) serves the frozen Clause path until 8.15 removes it.
- **The event does not carry the row's kind.** The proxy's ruling had
  `offer-row` carry it. The kind is already state: `?north.menu-kind`, set
  by the `open-*-menu` that north sends before every offer, so carrying it
  would write it twice, once per row of a 1,000-row menu, and let an offer
  contradict its menu. Its one use, choosing the `current` mark, reads the
  menu's kind instead. North fills a menu only by `clear-menu-rows`, its
  `open-*-menu` and then its offers (`galileo_menu.rs`; `clause_state.rs`
  457–510), so on every sequence north sends, `offer-row` posts what the
  old offers posted, and the replay checks the recorded ones.
- **No posts shared across events, and no named post blocks (SPEC
  §5.2).** Events whose clauses make the same posts, differing only in the
  values posted, are one event. No event states another's posts (D-42),
  and no block of posts is named for clauses to call. That block is the
  smaller form the proxy allowed, and it is not smaller: it is a new
  construct and a second place where a transition is written, for a
  repetition the merge removes with none, and each call would list the
  event's arguments again.
- **The importer keeps Clause's three handlers.** An import mirrors its
  source's events (D-28), and the merge needs north to compose the work
  row, which only a rewrite with its host change can do. The replay and the
  step-agreement test translate a recorded `offer-history-row` into
  `offer-row` with annotation `""` and an `offer-settings-row` as it is,
  through one test module.
- **The merge lands before 8.8b,** on `examples/north-menu.gal` in its
  present forms (8.8c), with north's bindings and adapter following in
  north's lane (8.8d), so 8.8b's stop on a non-empty bindings diff keeps
  its meaning.

Kept:

- **`open-history-menu` beside `open-settings-menu`:** they share two
  posts but not their posts: a history menu keeps the help and empty texts
  it finds, as Clause's does (`north.clause` L618–630), and the replay
  holds that behaviour.

Rejected:

- **One event stating another event's posts** (O12's first option): the
  proxy rejected it, since D-42 keeps one step per event.
- **A named block of posts that the handlers call** (TLA+'s action
  operators): above.
- **`offer-row` with every kind's arguments** (`?state`, `?holder`,
  `?clock`, `?eta` and `?blocked` beside the four texts): each call would
  pass five or six filler values, the ceremony Tom's rule removes, and the
  clause would dispatch on the kind.
- **North marking `current`:** a second owner of the active conversation
  (D-57).
- **A row's order from its creation, dropping the counter:** it makes the
  allocator's order observable, a change of direction, and drops
  `menu-row-count`, which the replay compares.
- **A derived `annotation`:** the mark would follow the menu's kind and
  the active conversation after the menu closes, where Clause's is fixed
  when the row is offered.
- **A `when` binding for the mark:** the event's slot already has the
  name `?annotation`, so unlike addendum 3's binding, read as the short
  element of its own field, it would add a name read once (`annotation:
  ?mark`), the helper binding Tom's rule removes. The record's line is 225
  characters, the sketch's longest, and the sketch has 4 lines over 120
  where it had 5.

| File | Lines | Non-blank | `?` | `{` | Characters | Over 120 |
| --- | --- | --- | --- | --- | --- | --- |
| `notes/north-menu-sketch.md`'s block, after addendum 4, after this one | 162 → 141 | 134 → 116 | 178 → 128 | 12 → 5 | 5,655 → 4,605 | 5 → 4 |

The three offer clauses' 15 lines and 1,015 characters become 4 lines and
364, and the three event lines' 288 characters 83; `work-annotation`'s
five lines and the work row's two interpolated texts go to north.

Boxes: 8.8c (galileo: the example in its present forms, its four callers
and the replay's translation, with A8 measured on the merged event), then
8.8d (cross-repo, north's lane: bindings, image and adapter). 8.8b now
needs both, and 8.14 and 9.13 are amended. 2 boxes, 40 minutes.

### D-62. A step maintains its model by deltas (Tom: go, 11 Oct 2026, kept only if 9.20 passes)

Tom: "go on the faster engine if its better". The gate is 9.20: WP9 stays only if it measures a 1,000-row setup under 1 s, a 10,000-row setup under 10 s and a query keystroke under 50 ms, with 0 disagreements against the reference on every change shape. If 9.20 misses, revert 9.14's commit, which removes the new path, and report the numbers.

A one-shot Opus planner at max effort, on Tom's direction through his
proxy ("OK, we'll just make it speed up certainly fast"): make the engine
fast by construction, so that every step costs what it changes, instead of
fixing speed case by case as box 8.9 does. Read at main `781f5a3` with
SPEC §1, §4, §5, §6.1, §7, §10.1, §10.4 and §12.1, D-16, D-37, D-40, D-58
with its amendments, WP3 and boxes 8.9 to 8.9n, and `engine.rs`,
`eval.rs`, `state.rs`, `expr.rs`, `modes.rs` and `claims.rs`. Differential
dataflow (A-DD, MIT licensed) and the DBSP paper (A-DBSP) were read for
ideas only; no code is copied. One scratch probe built the menu's state
directly from the setup's per-row facts, checked it equal to the real
setup's at 200 rows (1,410 facts), and timed `engine::evaluate` in a
release build under one exclusive lease. It changes no language: SPEC's
semantics, the Core IR, the reference interpreter, checkpoints, the event
log and the state hash stay as they are.

Revised once, on 2026-10-11, by a second one-shot Opus planner at max
effort, after a Codex Sol xhigh reviewer tried to break the first draft
and accepted five flaws: D-61's mutable fields make a field read a
source; reordered tests and eagerly kept aggregate groups raise errors
the reference never reaches; a restored session's rebuild can exceed the
default budget; argmin loses a pair that two rows share; and the checks
compared error kinds only. Each answer stands below where it applies,
with the review's three open choices (the ranked sweep's direction,
DRed's re-derivation and the half rule's measure) settled, its narrow
cases folded into the WP9 rows, and two new boxes, 9.5b and 9.18b. Tom's
proxy added one goal, a 1,000-row setup in under 1 s, since 8.9m measured
35 s and a host shows the menu only after its last offer. Read at main
`ff9e23c` with D-61, SPEC §4.1, §5.2, §5.3 and §6.2, `check.rs`'s
schedule, `expr.rs`, `engine.rs`'s ranked path, `state.rs`'s `Run`, the
differential and `step_agreement.rs`.

- **Why: what a step costs today.** D-58 keeps M(S) and reuses it when a
  step changes no relation a rule reads; any other step evaluates M(S′) in
  full. North's menu changes such a relation at every row offer and every
  query keystroke, and one evaluation of its state costs 34,016 units and
  31.2 ms at 1,000 rows and 340,016 units and 437.9 ms at 10,000 rows
  (medians at `781f5a3`; 28,682 units and 25.4 ms when the query leaves
  111 rows visible). So after all of 8.9 a keystroke at 1,000 rows misses
  A8's 1 ms by 31 times, and a 10,000-row setup, one evaluation per offer,
  takes about 36 minutes by extrapolation (43.8 µs per row per
  evaluation). Three more costs grow with the state rather than the
  change: a view after a rows change recomputes its `rows` field on the
  reference's full scans (697 ms at 1,000 rows, D-58's 8.9l amendment);
  clause matching runs on the same scans (`eval::matches`), so
  `accept-menu`'s clause visits about two million tuples at 1,000 rows,
  counted from its nested scans; and a program with an invariant evaluates
  every claim on the reference at every step (`check_claims_spending`).
  Each 8.9 box removed one measured cost, and the next program shape would
  need its own.
- **What a session keeps.** `Maintained`, in a new
  `crates/galileo-core/src/incremental/`, takes the place of D-58's kept
  `Model`: each relation's tuples with a derivation count (1 for stated
  and state facts) in canonical order, with ordered indexes by key
  columns; hidden relations and summaries for aggregates (below); mode
  state by key; and a version per relation that changes when its presence
  does. It stays outside `State`, the checkpoint, the hash and `Run`'s
  equality, and it is built in full at first use after `new`, `restore` or
  `replay`, charged to no occurrence.
- **A rebuild after revision 0 has no cap.** At revision 0 the build has
  the default budget, as D-58's has. After `restore` or `replay` at a
  later revision, admitted steps produced the state and the session that
  took them held its model, so the rebuild runs to its end: M(S) is finite
  (§4.3), and its units, which `Run::build_work` reports, are a function
  of S. A capped rebuild breaks restore. In the review's trace, offers
  that each add one fact reach 10,000,001 facts at a few units a step, and
  a rebuild under the default 10,000,000 cannot even load them, so the
  restored session answers `Unknown` where the uninterrupted one admits.
  The price is the time and memory of about one evaluation of the state,
  paid once at restore.
- **The delta rule.** A step's change (D-58's W, I, C and D, as added and
  withdrawn facts and created and deleted entities) is a Z-set of base
  tuples (A-DBSP). Strata run in order, and a relation's readers see only
  its presence change, +1 when its count leaves 0 and −1 when it returns
  to 0, so M(S′) is still the set of SPEC §4.4. Within a stratum, rules
  run in program order whatever their relation's strategy, and each rule's
  count change takes two passes over its changed sources: positive
  sentences, `not` patterns whose match flipped, field reads whose
  entity's value changed, and aggregate literals whose group value
  changed. The retraction pass enumerates in the pre-state the
  derivations the step removes, and the insertion pass enumerates in the
  post-state the derivations it adds. Each derivation counts once, at its
  first changed source in body order, because the sources before it may
  match only what the step left unchanged. This is the counting algorithm
  (Gupta, Mumick and Subrahmanian, SIGMOD 1993) run as delta queries over
  shared indexes (A-DD's `dogsdogsdogs`), so no join result is stored.
  Lower strata are updated in place, and the pre-state is read through
  each relation's step delta: the store minus this step's insertions plus
  its deletions.
- **A field read is a keyed source.** D-61 makes each field the relation
  `T.f`, and a state field changes while its entity lives (`then ?x.f =
  e`), so the first draft's argument, that a field changes only with its
  entity's lifecycle, holds for creation fields alone. A read `?x.f` whose
  argument is a variable is a source keyed by the entity: its tuple is
  `(x, v)`, or `(x, null)` when a `maybe` field has no fact, and its
  changed keys are the entities whose `T.f` presence changed. It counts at
  its literal's position, reads within one literal left to right, and a
  delta query from it binds `?x` to each changed key, reading the old
  value in the retraction pass and the new one in the insertion pass. A
  read whose argument is not a variable, such as `?x.g.f`, keeps its rule
  on Recompute when its relation changes. In the review's trace, five live
  counters and `H(?c, ?n) when IsCounter(?c) and ?n = ?c.value`, a step
  that sets `c0.value` from 0 to 1 changes 2 of `Counter.value`'s 5
  tuples, below the half rule, and the delta from the read retracts
  `H(c0, 0)` and inserts `H(c0, 1)`. Each pass still reads one state: with
  `L1(e, 1)` inserted for a new `e` and `L2(1)` deleted in the same step,
  old values for retractions and new ones for insertions inside one mixed
  pass leave the mixed terms of `H(?x.f) when L1(?x, ?y) and L2(?y)`
  uncancelled.
- **Errors arise where the reference raises them.** The reference tests
  each body literal in its schedule, the first ready literal in body order
  (`check.rs`), and a literal it never reaches raises nothing: an event
  that binds `?n` to the largest `Int`, with `Ready(?n)` then
  `?m = ?n + 1` and `Ready` empty, admits the empty `may skip` step. So in
  the executor every literal other than a plain sentence, a positive
  sentence or `not` whose arguments are variables, constants or `?_`,
  keeps its scheduled place: the literals scheduled before it hold first,
  and none scheduled after it runs before it. Between two such literals
  the executor takes the plain sentence whose index bucket is smallest. A
  delta query binds its changed source first, which only narrows the
  bindings, and still tests the literal that holds that source at its
  scheduled place. A binding that reaches a literal in S′ but not in S has
  a first changed source at or before that literal in the schedule, so
  the insertion pass reaches it, and a binding that reached it in S raised
  nothing then, since S was admitted; so an occurrence raises an error on
  the engine exactly when it does on the reference. Aggregate groups are
  kept for every group an outer binding might read (below), so they hold
  errors as data, raised only when an outer binding reads the group.
- **Three strategies, chosen per derived relation and step.** *Delta*, as
  above, for rules made of positive and negative sentences, comparisons,
  boolean literals, bindings, memberships, field reads, calls to
  definitions without queries, top-level aggregates, argmins and queries
  whose bodies are plain sentences holding every shared variable the body
  or value reads, and 8.9e's ranked shape, whose body is plain sentences
  apart from its comparison. *DRed* (the same paper) for a recursive
  stratum, which keeps set semantics (below). *Recompute* for every other
  rule, such as one with a query inside an expression or a called
  definition or another correlated aggregate, and for a relation one of
  whose sources changed at least half by the half rule: the relation's
  rules are evaluated in full on the post-state, by the executor below or,
  for a shape it does not cover, by engine.rs over the current tuples of
  what they read, and its presence change is the difference, so no step
  costs much more than evaluating what it touches. A recursive stratum
  takes one strategy for all its relations. Recompute is the permanent
  fallback, and the interim one until each Delta shape lands, so an
  unsupported shape is slow, never wrong, and `Maintained::strategies`
  names each relation's strategy so that a slow program can be read.
- **The half rule counts net presence.** A source's change is what is
  present in exactly one of the pre- and post-state: a relation's tuples,
  a field read's entities whose value differs, and an aggregate's groups
  whose value or existence differs. A fact withdrawn and included in one
  step, or a derivation count that moves without reaching 0, changes
  nothing. A relation recomputes when, for some source of one of its
  rules, twice that change is at least the source's size in the
  pre-state, so a source that was empty recomputes its readers whenever it
  gains. Like every choice here, it depends on S and the step only.
- **DRed re-derives from the remainder.** A recursive stratum
  over-deletes, in the pre-state, every tuple with a derivation through a
  deleted or over-deleted tuple, to a fixpoint, and removes the whole
  over-deleted set. Then one semi-naive least fixpoint, seeded with that
  remainder and reading lower strata in the post-state, re-derives
  over-deleted tuples and inserts new ones, so a tuple returns only
  through tuples already back, and over-deleted tuples never certify each
  other: with `Reach(?x) when Seed(?x)` and `Reach(?y) when Reach(?x) and
  Edge(?x, ?y)` over the cycle `a → b → a`, deleting the only seed `a`
  removes `Reach(a)` and `Reach(b)`.
- **Aggregates.** Inside the engine only, so that the Core IR and the
  reference stay as SPEC defines them, a top-level aggregate, argmin or
  query whose body is plain sentences gets a hidden derived relation of
  its body's distinct rows, keyed by the shared variables its body and
  value read, and maintained like any relation. Any other body keeps its
  rule on Recompute, since the reference tests such a literal only for
  the groups an outer binding reaches. Per group a summary answers the
  literal: the row count; an exact `i128` sum, range-checked when read
  (§4.3); an ordered multiset of values for `min` and `max`; for argmin,
  an ordered multiset of `(key, candidate)` pairs, each counted by the
  distinct rows that project to it and gone only with its last row, so
  rows `(A, 0, w1)`, `(A, 0, w2)` and `(B, 1, w3)` still choose `A` once
  `w1` goes; a tally of `Int`, `Text` and other values; and the rows
  whose value, key or candidate raised. A group with a raising row, or a
  tally its reduction rejects, answers by the shared reduction over its
  rows, which gives the reference's error, and only when an outer binding
  reads it. For `all` and `the`, a changed group keeps the shared
  `expr::query`'s result over its rows, value or error. An empty `count`
  or `sum` is 0, and an empty `min`, `max` or argmin binds nothing. D-40
  wrote the reductions once; the summaries maintain count, sum, min, max
  and argmin a second time, as 8.9e's running sums already do, and the
  differential compares them with the reference after every batch.
- **A ranked aggregate sweeps by direction.** A ranked `count` or `sum`
  (8.9e) keeps its group's keys in an order-statistic multiset with
  `i128` sums, and the rule's bindings before it as a hidden relation
  ordered by group and threshold. Read as `key op threshold`, with the
  operands reversed as engine.rs's ranked path does, a value is one prefix
  query: the weight of the keys below `t` for `<`, at or below `t` for
  `<=`, the total less the weight at or below `t` for `>`, and the total
  less the weight below `t` for `>=`. A key `k` whose weight changes
  therefore changes the value exactly at the thresholds `t > k` for `<`,
  `t ≥ k` for `<=`, `t < k` for `>` and `t ≤ k` for `>=`. Per group and
  pass the changed keys are sorted and the outer bindings swept once over
  that range, ascending from the smallest changed key for `<` and `<=`
  and descending from the largest for `>` and `>=`, with an `i128`
  running change that takes each key's weight change as the sweep passes
  it; a binding whose running change is 0 keeps its derivation. In the
  review's case, `count where Key(?k) and ?k > ?t` over the keys 2, 4, 6
  and 8, deleting 8 changes the values at the thresholds 1 and 5 and not
  at 10.
- **Modes are checked where the step touched them.** State modes at the
  keys of the facts a step adds and withdraws and of the entities it
  creates; derived modes, per stratum, at the keys its presence change
  touched and at newly live keys of `exactly one` modes; one per-key
  function serves this and `modes::check_stratum` (D-40). A pre-state that
  holds its modes cannot fail at an untouched key, so the failures, in
  relation, mode and key order, are the full check's, and §5.3 step 5's
  order and every rejection read as today.
- **A step is one meter and one transaction.** Matching, `then` values,
  the delta, mode checks, invariants and `ensures` charge one `Meter`
  sized by the step's budget; today each part gets a fresh budget, so a
  step has no single cost. Matching, values, `ensures` and views use one
  executor over the maintained indexes, slot-based like engine.rs's
  search and ordered as above. `before` claims are decided before the
  change is applied. Each invariant's auxiliary relations (3.11c's rules
  for that claim) are maintained above every stratum of the program,
  claim by claim in program order, and a step decides each claim from
  them, a sentence or `some` invariant holding when its witnesses are not
  empty and a `no` or `every` invariant when its counterexamples are
  empty, and stops at the first that fails, as the reference does. A
  rejected or `Unknown` occurrence, or a rejected sequence, undoes the
  maintained model with the state through 8.9j's undo log.
- **Work stays a function of the state and the step.** Every structure is
  ordered, never a hash container, and processed in canonical order:
  strata ascending, rules in program order, sources in body order, tuples
  in value order. Every maintained quantity is a function of the state
  alone. So a step's units, its first error with its message, and where a
  budget stops it depend on S and the step only, and a restored session,
  which rebuilds the model, matches the uninterrupted one in all of them
  at every later step (D-58). Building an index is never charged, as
  engine.rs's lazy indexes are not, since when an index is first built
  depends on history.
- **Diagnostics agree as far as one order allows.** `Run` and
  `state::step` agree on every occurrence's admission, effects, revision
  and hash, and on a rejection's kind and every explanation line, since
  the same checks and the same why-not produce them on the same state. An
  evaluation error agrees on its location (`at`, the rule or clause) when
  the first stratum that raises is not recursive: both paths run strata in
  order and rules and clauses in program order, and both stop in the
  first that raises. Its kind and message agree when that rule raises one
  text in the occurrence; when it raises two, such as `the` in two groups
  with different counts, or in a recursive stratum's rounds, each side
  names its own first, as engine.rs and the reference can today. Engine
  and reference work stay independent; only a restored session's must
  equal the uninterrupted one's.
- **Questions keep their paths.** `why` and `why not` rebuild provenance
  from scratch with engine.rs on the state they explain, as they do today
  on stored states (§5.7); minimal proof heights are not maintained.
  `as_of`, `--since`, claims, interventions and state search keep their
  evaluators; each could later fold its history or its candidates through
  `Maintained`, by its own decision.
- **The reference stays the oracle (D-16).** Three checks extend to
  deltas. *Folding deltas*: each of the differential's 10,000 programs,
  extended with a state entity whose creation and state fields a rule
  reads, gets 8 random batches of insertions, deletions, entity creations
  and deletions and replacements of a live entity's state field, and after
  each batch the maintained model equals the reference evaluated from
  scratch; a batch the reference answers with an error gives one of the
  same kind and location, a mode error the same failures, and after the
  undo the model before it. Its generators gain the review's narrow cases:
  a guard before an overflowing binding, unread groups that overflow,
  argmin witnesses that share a pair, every ranked operator below the half
  rule, and an old cycle. *Steps*: 8.9g's recorded sequences, and
  generated programs whose handlers set, include, withdraw, create and
  delete, step through `Run` and `state::step` with equal admissions,
  effects, revisions, hashes and explanations. *Restore*: a checkpoint
  restored at a random revision matches the uninterrupted session in full
  at every later step, also under a small occurrence budget, so that
  rebuilds larger than the budget and budget cuts are among them. An
  `Unknown` on either side is reported apart and never agrees (D-40).
- **What 8.9 becomes.** 8.9c's one-probe field read is how every lookup
  works. 8.9e's sort-once stays in engine.rs for questions and for
  relations that recompute through it, and the order-statistic summary is
  its maintained form. 8.9f's kept model becomes `Maintained`, and 8.9g's
  property extends. 8.9h's rule-read skipping is subsumed, since a change
  no rule reads produces empty deltas, and its branch goes. 8.9i's summed
  hash is already a delta, linear in the state's Z-set. 8.9j's in-place
  step and undo log carry the model too. 8.9k's state mode check per
  changed relation narrows to touched keys, which a 10,000-row offer
  needs. 8.9l's memo stays, keyed by relation versions instead of `Arc`
  identity because the model now changes in place, and its projection
  moves from the reference's scans to the executor. 8.9n's shared fields
  stay. No 8.9 box stops: A8 finishes as planned, and WP9 starts beside it
  on new files.
- **Goals, which box 9.20 measures once under an exclusive lease;**
  nothing before 9.20 shows them. One row offer costs at most 500 units
  at both 1,000 and 10,000 rows, within 10% of each other, and under 1 ms
  at 10,000 rows (today a full evaluation, 340,016 units and 437.9 ms);
  `accept-menu` costs at most 200 units; a 1,000-row setup takes under
  1 s, where 8.9m measured 34,963 ms at `45aa8d9` for one `run_all` of
  clear, open, 1,000 offers and a query, each offer evaluating the whole
  menu, so the cost grows with the square of the rows and a host shows
  the menu only after its last offer; a 10,000-row setup takes under 10 s
  (about 36 minutes after 8.9, by extrapolation, never measured); a
  query keystroke with its view takes under 50 ms at 1,000 rows and at
  most a full evaluation's units (today 31 ms plus the view's 697 ms); A8's
  move and view stay under 1 ms; and the three checks above report 0
  disagreements. The first draft let a keystroke charge 1.5 times a full
  evaluation, 51,024 units, which the review priced at today's measured
  0.92 and 1.29 µs per unit: with 9.16's 10 ms view, 56.8 to 75.7 ms, so
  that gate and the 50 ms one contradicted each other unless the executor
  beat engine.rs per unit. A keystroke recomputes every row's visibility
  and position but loads none of the state's 7,010 facts, a unit each of
  a full evaluation's 34,016, so its gate is now one full evaluation,
  which with the view fits 50 ms at the 0.92 µs per unit measured at
  1,000 rows; 50 ms stays the goal, and only 9.20 shows whether it holds.
  A unit is a scale, not a speed: a Delta step charges units in
  proportion to the tuples and derivations it changes, each O(log n) time
  in ordered indexes, and a Recompute step about one evaluation of the
  relations it touches.

What stays fixed: SPEC's semantics and every rejection's kind and text,
the reference interpreter as the oracle (D-16), the default budget of
10,000,000 work for each occurrence and for the build at revision 0, the
checkpoint, log and hash formats, the `Session` API apart from a new
`last_work`, and A8's measurement. PLAN boxes 9.1 to 9.21, with 9.5b and
9.18b, do this. SPEC §10.1 and §12.1 describe the engine's strategy, not
semantics, and 9.21 rewrites them once 9.20's numbers hold.

Rejected:

- **A dataflow runtime: DBSP circuits, or differential dataflow with
  nested time:** iteration-indexed traces keep every round's delta, an
  operator graph is a second IR, and positive-only recursion (§4.3) needs
  only DRed; A-DDlog, a Datalog compiler to A-DD, is archived.
- **Depending on `differential-dataflow`, `timely` or `dbsp`:** the core
  crates take no dependencies (D-20), and timely's workers and threads sit
  behind an operating-system boundary that only the CLI and the LSP cross
  (§10.4).
- **Keeping D-58's relation-level reuse and adding cases:** every
  keystroke and offer stays a full evaluation, 31 ms at 1,000 rows, and
  each new shape needs its own box.
- **Recomputing only the strata whose inputs changed:** the menu's
  visibility and position strata are most of an evaluation, so a keystroke
  still costs nearly all of one.
- **Insert-only semi-naive continuation across steps:** every moded `then`
  line withdraws a fact, so deletions are routine.
- **Counting in recursive strata:** a cyclic derivation has no finite
  count.
- **Backward/Forward or FBF for recursion (Motik et al., 2019):** less
  over-deletion than DRed for more code; it waits for a measured program
  whose DRed over-deletes.
- **Maintaining proof heights:** minimal heights under deletion are
  shortest paths under deletion, and `why` is a question that rebuilds on
  its own state.
- **Old values for retractions and new ones for insertions within one
  pass:** not exact, by the example above; two single-state passes are.
- **Copy-on-write structures for rollback:** a dependency or a
  hand-written persistent B-tree, where 8.9j's undo log already exists.
- **An overlay for the post-state instead of updating in place:** every
  insertion probe would pay it; in place, only the retraction pass reads
  through a delta, and appends stay cheap.
- **Hash indexes:** after deletions their order depends on history, and
  with it a step's first error and where a budget stops it.
- **Maintaining view lists:** the host's owned decode of a changed list
  dominates (8.9n), so views evaluate on the executor under 8.9l's memo.
- **Rewriting aggregates in the Core IR:** the hidden relations exist
  inside the engine only, so the IR and the reference stay what SPEC
  defines.
- **Compiled rules (§12.1) first:** a constant factor, not the
  asymptotics.
- **Keeping field reads out of the delta by their entity's lifecycle:**
  D-61's state fields change while their entity lives, so the argument
  holds only for creation fields, and one rule for every read is simpler.
- **Recomputing every rule that reads a changed state field:** a game
  that changes `?unit.hp` each step would recompute every reader.
- **A ready test first, or tests reordered freely:** an overflow behind
  an empty guard, which the reference never reaches, would reject the
  step.
- **Raising a hidden group's error when the group changes:** the
  reference evaluates a group only when an outer binding reaches it.
- **A set of `(key, candidate)` pairs for argmin:** a pair that two rows
  share would go with the first of them.
- **Capping the rebuild at the default budget, or any fixed budget:** a
  restored session would answer `Unknown` where the uninterrupted one
  admits.
- **Gating admission on the model's build cost:** that cost follows the
  executor's choice at every binding, which cannot be maintained without
  storing join results.
- **Keeping the model or its build cost in the checkpoint:** the
  checkpoint format stays fixed.
- **Re-deriving against the pre-state or the whole post-state:** an old
  cycle would certify itself.
- **Counting derivation changes, or a fact withdrawn and included, in the
  half rule:** the strategy would change where the model does not.
- **Re-running the reference to report its first error:** its full
  evaluation of a large state exceeds the budget, turning a rejection into
  `Unknown`.

Risks, largest first:

1. **Exactness.** A double count or a missed cancellation in the passes,
   the field sources, the hidden relations or the ranked sweep corrupts
   models only for change shapes the generators may never produce. Folding
   deltas with deletions, field replacements, entity lifecycles and
   rollbacks, a seeded fault per box, the review's narrow cases, the
   recorded north sequences and generated steps guard it.
2. **Determinism.** Any order that depends on history, such as a lazily
   built structure, a hash container or an insertion order, would move a
   restored session's budget boundaries and first errors. Ordered
   structures, canonical processing, uncharged index builds and the
   restore check guard it. When one rule raises two different texts in one
   occurrence, engine and reference can name different bindings, as
   engine.rs and the reference can today; a warm and a restored session
   cannot.
3. **Constant factors.** Units cost 0.9 to 1.3 µs in engine.rs and every
   `Value` clone copies its strings, so steps that touch everything gain
   little from deltas, the keystroke goal rests on the executor's cost per
   unit, the indexes, hidden relations and the ranked outer index keep
   several copies of each row, and a test between two selective sentences
   keeps the reference's join order around it.
4. **Restore time.** A rebuild after `restore` costs about one evaluation
   of the state, in time and memory, before the first step it serves.

Also: DRed can over-delete far beyond the net change; folding history,
claims or search through `Maintained` is scope that waits for its own
decision; and box 3.3c's per-byte charges will move the unit counts the
targets name.

If it fails midway: nothing durable changes, no checkpoint, log, hash or
IR format, so stopping costs only the boxes spent, at most about seven
worker-hours. Before 9.14 nothing a host sees changes, and an abandoned
`incremental/` module goes in one commit. After 9.14, each shape not yet
maintained recomputes per relation, about as fast as D-58's full
evaluation, and 9.14 is one revertible commit back to D-58's kept model.

### D-62 addendum. A step's `last_work` counts what its one meter charged

A one-shot Opus planner's ruling on DECISION 9.13, 2026-10-11, read at
WIP `fcf94f2` with `state.rs`, `engine.rs`, `claims.rs`, `eval.rs`,
`why_not.rs` and `modes.rs`. `engine::evaluate`, the reference's blocking
evaluator, `check_claims_spending` and `why_not` report their units only
on success or exhaustion, and engine.rs's derived mode check charges
nothing, so on those paths one meter cannot count the whole of a step
without changing their APIs.

- **`last_work` is what the meter charged.** Matching, `then` values,
  `ensures` and `Run`'s own mode checks charge the occurrence's meter
  directly, a mode check one unit per tuple it reads; the evaluation and
  the invariant check charge the units their APIs report. When the budget
  stops the occurrence, `last_work` is the budget. Otherwise it is exact
  for an admitted occurrence: under a budget of `last_work` it admits, and
  under one unit less it ends `Unknown(work)`. An evaluation or invariant
  check that ends in an error or a mode failure reports nothing, so there,
  and under a derived-mode rejection, `last_work` is a lower bound. A
  later box that charges the meter directly narrows that.
- **The no-match explanation stays outside the meter.** It renders a
  decided rejection under the occurrence's budget, as today, and 9.18
  compares it with the reference line for line.
- **engine.rs's derived mode check stays uncharged,** as in every
  question, so the unit counts D-58 and this decision quote keep their
  meaning; state search keeps today's charges.

Rejected:

- **Returning the units spent with every error from `engine::evaluate`,
  `derive`, `evaluate_blocking`, `check_claims_spending` and `why_not`:**
  five files across lanes for a total no box measures (9.13, 9.17, 9.19
  and 9.20 read admitted steps; 9.18b compares two sessions, which needs
  only determinism), on step paths 9.14 and 9.17 replace.
- **`last_work` absent on errors:** an `Option` in the `Session` API with
  no reader.
- **Estimating the units a failed part hid:** not what was charged.
- **Charging the explanation to the meter:** a rejection whose matching
  spent most of the budget would end `Unknown`, where today it rejects.
- **Charging every entity an `exactly one` check scans:** `move-menu` at
  200 rows would charge all 200 `MenuRow`s for `menu-selection`'s one
  North key.

### D-62 addendum 2. A unit ceiling taken from engine.rs is a ratio at the same commit

A one-shot Opus planner's ruling on DECISION 9.10, 2026-10-12, read at
main `df69ed3` and lane `70e13d7`, whose `engine.rs`, `expr.rs`,
`examples/north-menu.gal` and `menu_scale.rs` equal main's.

- **D-62's engine numbers predate 3.3c.** They were measured at
  `781f5a3`; box 3.3c (`44b9cc7`) then charged each byte of a `++` result
  in the shared `expr.rs` (D-37, D-40). `menu-row-visible` concatenates
  `?row.label ++ " " ++ ?row.description ++ " " ++ ?row.key`, 68 + 5d bytes
  for a row whose index has d digits, so one evaluation of the menu now
  costs 116,466 units at 1,000 rows (34,016 + 82,450) and 1,214,466 at
  10,000 (340,016 + 874,450), as this decision foresaw.
- **A ceiling derived from engine.rs's units is a ratio to
  `engine::evaluate`'s work at the same commit,** never a number quoted
  from `781f5a3`, and it counts every unit the one meter charges, bytes
  included: a build runs under the default budget against all of them,
  and a value grows only by units charged (D-37). 9.10's maintained build
  at 10,000 rows is at most 1.5 times it: at `70e13d7`, 1,244,477 against
  1,214,466.
- **The absolute goals stand.** The 500-unit offer, the 200-unit accept,
  the 100-unit move and 9.11, 9.19 and 9.20 name targets, not multiples of
  340,016. An offer's own row adds 88 bytes at 1,000 rows and 93 at 10,000,
  which neither the 500 nor the 10% spread contradicts; `query_work` at
  most `query_full` is already a same-run ratio.

Rejected:

- **Excluding Concat bytes from a ceiling:** a second meter that no budget
  or `last_work` reads.
- **A re-derived fixed constant:** stale again at the next charge change.
- **A fixed allowance over the current evaluation:** the same carve-out by
  subtraction; D-62 prices a build as a multiple of an evaluation.

### D-62 addendum 3. The ranked outer index covers a prefix that stands before its aggregate

A one-shot Opus planner's ruling on box 9.11's split, 2026-10-12, read at
main `dc1f52e` with `incremental/{mod,aggregate,ranked,execute}.rs`, the
differential and the handoff of 9.11's first attempt, which measured
`menu_scale -- --steps 1000` at `1a5edd7`: `offer_work` 22,210 against the
500-unit goal, and `setup_ms` 20,399.

- **An offer grows with the rows for three reasons, and the index removes
  one.** A changed ranked group visits every outer binding;
  `aggregate::update` re-evaluates every row of a summary when a step
  creates an entity or changes a field its value or key reads, and the
  menu's position key reads `?prior.order`; and a creation touches every
  stratum. PLAN 9.11a to 9.11f take them in turn: a generator case first,
  since 8.9d's ranked programs bind their outer variables from constant
  lists and never change an index, then the index, keyed refresh with
  liveness scoped to `exactly one` domains, the sweep and one probe.
- **The index covers a ranked plan whose threshold is a variable and
  whose literals scheduled before the aggregate all stand before it in the
  body.** The menu's rule and every differential ranked program have that
  shape. Another ranked plan still takes both passes and visits every
  binding of a changed group, as 9.10 does: slow, never wrong.
- **An error the index raises is the rule's.** The index is filled and
  maintained at its rule's turn, from that rule's own prefix in its
  schedule, so a binding it raises on is one the rule's insertion pass
  reaches at the same literal, and the error carries the rule's location
  as D-62's error rule requires.

Rejected:

- **Dropping the index on an error and visiting every binding:** a second
  path that no generated program reaches, for an error the rule raises
  anyway.
- **The prefix as a hidden rule in the program, as an aggregate's rows
  are:** its literals would raise under another rule's name and stratum.
- **A threshold expression kept as an evaluated column:** no program has
  one.

### D-63. The firn path's one-shot rulings: selection by a segment, no `null` facts, `builtins.match`, grouped selections and opaque imports

A one-shot Opus planner on DECISION 4.3, 4.5 (twice), 2.8c and 4.7 (twice),
2026-10-11, at main `e98e4c9` and the firn pin `10740a8`.

- **A Nix selection takes an attrpath segment (4.3).** D-35 kept `x."key"`
  and `x.{e}` out because no firn expression selected by one; the pin's
  quickshell and zen-browser write
  `(inputs.X.packages."${pkgs.stdenv.hostPlatform.system}").default` three
  times, and byte identity is WP4's gate (compass 4). Field access takes
  §9.4's `seg` as an entry does, a quoted segment emitting a quoted key and
  `{e}` emitting `${e}` (D-56). Only a Nix-stage value is selected so, the
  result is `Nix Any` and the key ends the option-tree walk; on a known value
  it is an error offering `lookup`, which keeps D-35's partiality point, and
  under `lib` or `builtins` an error, since signatures are found by name
  (D-45). This replaces D-35's rejection of both forms and the selection
  half of D-56's last ruling (boxes 2.3e, 4.3c).
- **A parenthesized selection is a Nix selection of its own (4.7).** Nix's
  parser keeps `(a.b).c` apart from `a.b.c`, and parse identity compares its
  trees (D-39), so the group survives import, fmt and lowering; Clause
  writes `(((a.config).lib).file).f` and Beagle `(x."${s}").default`.
- **A fact never writes `null` (2.8c).** `null` is how a missing fact reads
  (§4.1, §4.3, §5.2), so a fact with `null` in a `maybe T` slot would either
  store a second spelling of absence or state nothing, which §4.2 already
  makes an error for `[]`. No program needs one (compass 3). A `null`
  reaching a slot through a head, a `then` line or an event argument is
  ruled when a program first does.
- **`builtins.match` returns `maybe List (maybe Text)`,** Nix's contract,
  once 2.5c's `!= null` narrowing lands; styling's applied lambda is 2.5c's
  own Done-when case. `Any` is its stopgap until then.
- **An `imports` value that is not a path is a source of foreign options
  (4.7).** Without a schema, each node in which the importing module
  declares an option is open, program-wide; with a schema nothing changes
  (box 2.4d). The landed file-wide exemption unchecked every read in the
  file, schema or not, and left other files' reads of the same options
  failing.
- **`++` with a `Text` operand is `Text` (4.7).**
- **Plan facts, not rules (4.5).** The pin's flake has no `providerAdapter`;
  it is a `let` of `modules/codex`, imported by 4.3, and SPEC §10.2 and F4
  drop it. 4.5's input section alone stands; the other five sections need a
  construct that emits a binder and a reference named by a known `Text`,
  which goes to the language owner with box 4.5b, and 4.28 waits for it.

Rejected:

- **The two files as `nix file`, or `builtins.getAttr` in the import:** the
  first still writes the selection; the second emits other bytes, and an
  importer workaround is a language bug (compass 13).
- **Only the quoted form:** a second key grammar beside `seg`.
- **A printer rule that ends a select path at every non-name key:**
  grouping already says where a path ends, and both spellings round-trip.
- **A stored `null` in facts:** two spellings of absence.
- **Keeping `Any` for `builtins.match`:** a contract that states nothing
  where §9.4 can state Nix's.
- **Reading option declarations out of an imported Nix record:** a second
  way to declare an option.

### D-63 addendum. An unknown option with no near name lists its node's names

A one-shot Opus ruling on DECISION 2.4d, 2026-10-12, at main `739b9ed` and
the firn pin `10740a8`. 2.4d's Done when said that, with module A's opaque
`imports` removed, module B's read of `config.myConfig.modules.t.zone`
names `enable`. The landed checker says only "`zone` isn't an option of
`myConfig.modules.t`.", because `closest` offers only a name within a third
of the misspelled name's length, and `enable` is far more edits than that
from `zone`. SPEC §3.2 promised a did-you-mean and the declaring line under
every closed node, which a name with no near neighbour cannot have.

- **With no near name, the error lists the node's names (box 2.4e).**
  "`zone` isn't an option of `myConfig.modules.t`, which has `enable`."
  It gives up to six distinct child names in byte order and counts the
  rest: "…, `port` and 1 more." Six is the largest firn module node (glide's
  and users' six options), so every Galileo-declared node in firn lists in
  full, and a schema node such as `services` stays on one line. Reads and
  entries share the sentence, since both come from `unknown`.
- **A near name keeps its did-you-mean alone,** and a node with no names,
  such as an empty schema's `config`, keeps today's sentence.
- **2.4d's Done when narrows** to the near-miss read its commit measured,
  `enabel` naming `enable`, and keeps its tick.

Rejected:

- **Only rewording 2.4d's Done when:** the case an opaque import's removal
  produces, a read of a name the module never declared, would keep an
  error that names nothing (principle 7).
- **The nearest names by distance:** with nothing near, distance order is
  noise to the reader; byte order is stable and easy to scan.
- **Every name, uncapped:** a schema node such as `services` has hundreds.
- **Each name with its declaring line:** six sources make the sentence
  unreadable, and the names alone answer what the node holds.
- **Widening `closest`'s limit:** it would offer `enable` for `zone` as a
  fix, which it is not.

### D-64. The outside review's triage: `possibly` for reachability, `eventually` reserved, a failed `requires` names its bypass, and review names the definitions behind a requirement

An Opus planner ruling as language owner, for Tom's request through Plato,
2026-10-12, at main `6812a15`, on ChatGPT's source-only review of Galileo
at `e98e4c9` (`~/.local/state/agents/handoffs/galileo-external-review/`,
whose `triage.md` gives the verdict on each claim). Probes ran the CLI
built at `739b9ed`.

- **`possibly S` is reachability (SPEC §6.2, §9.4).** `eventually S` meant
  that some reachable state satisfies `S`, and one witness settles it. Every
  source Galileo cites for temporal claims (A-Alloy6, A-TLA, A-Apalache,
  A-telingo) uses "eventually" for LTL's F, which a requirement asserts of
  every run, and UPPAAL calls `E<>` "possibly" and `A<>` "eventually". A
  requirement is read in order to be approved, so the word failed T5's
  principle 8 and compass 9: a reader with those priors sees progress where
  Galileo checks possibility. The meaning is unchanged; only the spelling
  moves. The corpus holds two dungeon requirements, one §7.4 example,
  goldens and tests; firn and north use none (box 2.19i).
- **`eventually` is reserved:** a static error that offers `possibly S`, so
  that "every run reaches `S`" can take it when a program's requirement is
  progress, with the fairness assumption that needs (§12.1). Strict until a
  program needs more (compass 3).
- **A failed `requires` names its bypass (§6.1).** Its counterexample is the
  binding and the proof of `S` that holds with `C` blocked, each rule at
  file:line. Today `galileo check` prints "fails: it is false.", which names
  no rule (box 3.12b).
- **Review names the definitions behind a requirement (§8.4, item 8).** D-50
  reports a fact whose support changes as neither gained nor lost, so a
  patch that redefines a word a requirement reads, such as `approved`, while
  the requirement keeps holding, showed only as an edit. Review now lists
  each such edit beside every requirement and milestone that reads it,
  before and after, whatever the verdicts. It informs and does not gate
  (box 6.11).
- **A verdict on a program with events names its scope** (G9, §5.5): no
  SPEC change. Box 3.11e prints it, and box 7.4b decides `possibly`,
  `always` and `never` over events through 7.4's search, which `galileo
  check` never called: all five dungeon requirements answered Unknown.
- **Open (notebook O13):** on a program with events, `S requires C` and a
  single point of failure over a sentence are judged in one state, the
  initial one or the one `--events` reaches. The review's own door-and-key
  bypass, firing only once Ash is in the Crypt, holds vacuously in the
  initial state and fails only with events that reach the Crypt. It is
  ruled when the first event program states a dependency requirement.

Rejected:

- **Keeping `eventually` and printing "possibly" in answers:** the line is
  what a reviewer approves, and one reading per line (G5) is about the line.
- **`reachable S` or `S is reachable`:** as a suffix it collides with a
  domain's own `Room is reachable`, and as a prefix it is not an adverb, as
  `always` and `never` are.
- **Liveness now:** no program needs it (G1), and it needs fairness, which
  no program states.
- **Making item 8 gate approval, as requirement edits do (§8.5):** most firn
  edits touch rules that the tags requirements read, so the flag would be
  routine and then ignored.
- **Renaming `achieve`'s `possible` mode to avoid `achieve possible
  possibly`:** the mode is a command's word, not a program's line.

### D-65. A computed name: Nix binders and references named by a known `Text`, and `for` elements in braces

The language owner's ruling on box 4.5b's question, which the firn
rulings planner routed to the owner (D-63). The question: how does a Nix
body emit, once per element of a known list of `Text`, a lambda formal
and a reference whose name is that text? Read at main `359d05f` and the
firn pin `10740a8`. The probes ran the CLI built at `da5ec7c` and
`nix-instantiate --parse`. No cargo was run. The counts come from a
throwaway script run over a scratch copy of `flake.gal`.

**Measured.**

- The pin's `flake.nix` names its five module inputs 45 times outside
  the input section: once each as an `outputs` formal, and twice each
  as `name = name;` in four `inputs` records (NixOS, home-manager,
  darwin, darwin home-manager). 4.5's `flake.gal` writes all 45 by
  hand.
- firn's `docs/flake-inputs.md` promises that adding a module's flake
  input needs "no flake.bnix edits". With hand-written lists it needs
  five. 4.28's input scenarios add inputs, and a missed edit fails only
  when Nix evaluates.
- No construct emits an identifier from a known `Text`. `{?k}` makes
  only keys, and an entry value `?n` emits `"elephant"`. Nix's
  `{ inherit a; }` parses differently from `{ a = a; }`, so the records
  must emit `name = name;`.
- The records sit under 19 distinct hand-written binders: the outputs
  body's nine `let` lambdas, `mkSystem`'s formals, and the module
  lambdas' `config`, `pkgs` and `lib`. Any of them would capture a
  reference of the same name. firn's splicer checks only URL conflicts.
- Backticks accept any text: `` ({`a b`, ...}) => `a b` `` reaches
  Nix's parse gate, which names the emitted line (K24). `` `{?n}` ``
  reads today as a name spelled `{?n}`, and no `.gal` file writes it.

**Rulings.**

- **A computed name (SPEC §9.1, §9.4, §10.3).** `` `{e}` `` is the name
  that the known `Text` `e` spells. As a brace formal it binds that
  name, and as an expression in a Nix body it reads it. Galileo decides
  it when the `for` unrolls and emits a plain identifier. Backticks
  already mark a name (D-34), and `{e}` already computes one from a
  value as a key's segment (§3.1, D-56). The composition means a name
  computed from a value, with no new word, and one token decides it
  (G5). A name and a key stay two notions with one spelling each:
  `` `{e}` `` as a key is an error offering `{e}`. Prior art: Template
  Haskell's `mkName`, Rust's `format_ident!` and Racket's `format-id`,
  each a name built from a string. Nix itself has no computed variable,
  which is why Galileo decides the name when it builds.
- **`for` elements in braces (SPEC §9.4, §10.3).** `for H: f` in an
  inline record and `for H: x` in brace formals unroll once per row of
  `H`, a `for` block's header. A `for` block is already a comprehension
  with no state (D-42), so this is the same construct in one more
  position, not a new one. The header ends at the first `:` outside its
  brackets, as an `every` post's condition does. `for` opens an element
  only before `?`. The four records are inline because `//` follows
  them, and an inline record takes no indented `for` block.
- **A computed name never changes another name's meaning (SPEC §2,
  §10.3).** It is an error at its line when:
  - its text is not a Nix identifier, or is a Nix keyword;
  - as a formal, another formal of its lambda has the name, or it would
    shadow a name in scope, which includes the prelude, the names the
    Nix target binds, and `true`, `false` and `null`, which Galileo's
    literals emit;
  - as a reference, no Nix binding has the name (it offers the nearest),
    or two enclosing bindings have it (it names the nearer one, which
    would capture it).

  A bare name never reads a computed formal, so `galileo check` reads
  every bare name before any `for` unrolls. A computed name anywhere
  else is an error. The name comes from data, so nothing in the source
  shows the collision; the error is the only place the author can see
  it, and with the splicer a collision passed silently.
- **K24: backticks hold one bare name or one `{e}`.** Any other text is
  an error at its line, as §9.1 already said.

Kept:

- **`{?n}: `{?n}`` written out in a record.** A short element
  `` `{?n}` `` would make braces before `=>` read as formals, which is
  addendum 4's reason for bare names.
- **A `for` header at each of the five places.** Each names the derived
  relation once. A `let` holding the list would emit a Nix binding, and
  a flake's top level takes none (D-39).

Rejected:

- **Striking or narrowing 4.5b:** five edits per input, against firn's
  one-fact contract and Tom's write-once rule.
- **`outputs = inputs@{…}` with `lib.getAttrs`:** other Nix, so parse
  identity fails, and rewriting firn's flake around a missing construct
  is a language bug (compass 13).
- **`inherit` records:** they parse differently, and the formals would
  still need a construct.
- **`{e}` as binder and reference:** in an expression it is addendum 4's
  record.
- **`?n` itself as a binder:** `?n` reads its value everywhere else
  (compass 1).
- **A call such as `name(?n)`:** a binder cannot be a call, and `name`
  is the commonest Nix formal.
- **A word such as `named ?n`:** a new word, where two existing
  notations already say it (principle 2).
- **Splicing a `List Text` into formals by its type:** a meaning that
  depends on a value's type.
- **A `nix file` that knows flake inputs:** one file format's semantics
  built into the language.
- **Text around `{e}` in backticks, entities as `e`, computed `let` or
  parameter names:** no program needs them, and allowing them later
  breaks nothing (compass 3).
- **Leaving the error to Nix:** a bad identifier fails at the parse gate
  naming Nix's line, and a captured reference fails nowhere.

| `tests/external/firn-tags/flake.gal` at `359d05f`, before and after 4.5b | Lines | Input names outside the facts | `?` | `{` | Characters |
| --- | --- | --- | --- | --- | --- |
| | 72 → 72 | 45 → 0 | 17 → 41 | 95 → 104 | 10,736 → 10,548 |

The formals run goes from 48 characters to 44, and each record from 96
to 50. A `for` header carries three `?`, which is where the 24 new `?`
come from.

Boxes: 2.22a (`for` elements and keying by offset, after 2.21s), 2.22b
(computed names and K24) and 2.22c (their errors, after 2.22b). 4.5b now
needs 2.22a and 2.22c, and 4.28 still waits for 4.5b. 3 boxes, 60
minutes.

### D-66. Prefix `-`, the brace parameter `{...}`, and a `let` name interpolates as itself

The language owner's rulings on the language half of DECISION 4.13 (peer file 2686). These are
the three constructs in nixos-config main `4bcfad4c` that no Galileo form lowers to. Read at main
`78b7827` and the firn pin `4bcfad4c`. The probes ran the CLI built at `4243032` over scratch
imports of main's 69 Beagle outputs. No cargo was run.

**Measured.**

- plato's `OOMScoreAdjust = -500;` is the first negative number in firn's Nix; D-35 counted none
  at `10740a8`. In a scratch copy with `-500` patched to `500`, plato imports and builds
  byte-identical, so negation is its only gap. Nix parses `-500` as `(__sub 0 500)`.
- nexus-alerts and socrates-chat write `home-manager.users.${username} = ({ ... }: { … })`.
  §9.4's brace parameter needs a formal, so `({...}) => …` is a parse error: "expected a formal
  before `...`". Patched to `{ config, ... }:`, both import, and their builds stop only at
  unsigned `lib` calls (box 4.13b).
- nexus-checks and nexus-hardening import and build with 0 diagnostics, yet differ in 1 and 4
  lines. `"${stateDir}/gh"` emits `"/var/lib/nexus-checks/gh"`, and `"-w /etc/ssh${creds}"`
  emits the text. Box 2.6c computes every known interpolation, even though the `let` that binds
  the name is emitted and a bare reference stays a name (`CHECKS_STATE = stateDir;`) (K25).
- These are not language questions:
  - github-pat's guarded indented string already has a Galileo form, since D-59 lets
    `''' when c` follow the closer, and it builds `x = lib.mkIf c ''…'';`.
  - The record printer puts an entry's `when` after the first line of a multi-line value.
  - Eleven members have no signature.

  Boxes 4.13a and 4.13b (D-67) handle them.

**Rulings.**

- **Prefix `-` (SPEC §9.4), as D-35 set it aside for the first program that writes a negative
  number.**
  - Where an operand is due, `-` negates an `Int`, known or Nix. It binds tighter than every
    binary operator and looser than field access and calls, which is where Nix's own negation
    sits.
  - Before an integer literal it is part of the literal, so `-9223372036854775808` can be
    written.
  - After an operand, `-` subtracts, so `a -1` is `a - 1`, and T4's spaced minus stands.
  - Outside a Nix body, `-e` is `0 - e`, overflow included. In a Nix body it emits Nix's `-e`,
    which Nix reads as `0 - e`. A known negative `Int` emits as `-` and its magnitude, in
    parentheses where Nix would read a binary `-`.
  - The importer lifts every Nix negation, and `negation` leaves D-39's named errors.
  - Principle 9: `-500` is the spelling every reader already has.
- **`{...}` is the brace parameter with no formals (SPEC §9.4).** It emits `{ ... }:`, a module
  lambda that takes the module arguments and binds none. It is the empty case of D-42's brace
  parameter, not a new construct. `{}` stays an error, since no program needs a lambda that takes
  only the empty record. A root's `arguments:` still needs a name, since no firn root binds
  nothing.
- **A `let` name interpolates as itself (SPEC §9.1, K25).** In a Nix body, `{name}`, where a
  `let` binds `name` to a known `Text`, emits `${name}`, as `name` emits anywhere else there.
  The `let` is emitted, so Nix computes the same text, and the emitted Nix stays the image of its
  source (D-39). Galileo computes the text only where Nix's interpolation would not give it or
  cannot be written: an `Int`, which Nix refuses to interpolate, and a prelude call, a `?`
  variable, an aggregate or a query, which have no Nix spelling. Every other known interpolation
  is computed, as 2.6c landed.

Kept:

- **One token per slot.** A sentence slot still holds one token, so `-5` fills no slot. North's
  `-1` deltas arrive as event data (D-35), and allowing it later breaks nothing (compass 3).
- **D-35's rejection of negative literals in the lexer:** `a -1` still subtracts.

Rejected:

- **`0 - 500` in plato's `.gal`, or plato rewritten to `"-500"`:** both emit other Nix, and an
  importer workaround or a firn rewrite around a missing construct is a language bug
  (compass 13).
- **A negative literal token without prefix `-`:** for D-35's reasons; `-x` would still need the
  operator.
- **`(_) => …` or a bare record for `({ ... }: …)`:** both emit other Nix.
- **`{}` beside `{...}`:** no program needs it.
- **Inlining the `let` in the importer:** the import would emit other Nix (D-39).
- **Never computing interpolation:** `"{n}"` of an `Int` would emit Nix that fails to evaluate,
  and a `?` variable has no Nix name.

Boxes 2.23a, 2.23b and 2.23c: 3 boxes, 50 minutes.

### D-66 addendum. A group of option declarations

The language owner's ruling on DECISION 2.23b (peer file 2688). Once
`{...}` landed (`f042842`), socrates-chat still differed from `$P` in lines
4–28. Its options are one grouped block, and a `nix module`'s `option`
lines emit only flat bindings. D-66's probes missed this, because the
module stopped at `({ ... }:` first. Read at main `8b52d04` and the firn pin
`4bcfad4c`. The probes ran the CLI built at `6b70273`. No cargo was run.

**Measured.**

- The 66 Beagle outputs at the pin hold 91 flat declarations in 50 files,
  and 6 grouped declarations in 1 file (socrates-chat). 15 of the flat
  files have two or more options under one parent (52 declarations), and
  34 have one. The 185 Clause outputs need only parse identity, which
  ignores how attribute paths nest.
- socrates-chat written as a `nix file`, with its options as a nested entry
  of `lib.mkOption` calls, is `cmp` 0. With `cfg.homeservr` and
  `cfg.wathces` written into it, `galileo check` finds neither. As a `nix
  module` it finds both, each with a did-you-mean naming the declaring
  line.

**Rulings.**

- **`option P` with declarations under it is a group (SPEC §9.4,
  §10.2).**
  - Each line under it is an `option` line without the word and without
    the prefix: `Q: T` or `Q: enable e`, with its fields under it. It
    declares `P.Q` at its own line.
  - The group emits one binding, `options.P = { Q = …; … };`, with its
    declarations in source order. Flat lines emit one binding each.
  - Grouping is layout alone. The checker, `refs` and every message read
    each declaration by its full path through one function, so nothing
    can tell how it was written (compass 5).
  - This is the nesting rule `config` entries already follow, and the
    shape a focus block has: a shared subject written once on the line
    that opens the block (SPEC §4.2).
- **The source's layout is the Nix layout.**
  - Flat Nix has flat lines and grouped Nix has a group, so each Nix
    text has one spelling (compass 2). The emitted Nix stays the image of
    its source (D-39), as with D-63's parenthesized selection.
  - `galileo fmt` keeps the form written.
  - `galileo import nix` lifts a module-body binding `options.P = { … }`,
    with `P` non-empty, as one group when every binding in it is a
    declaration Galileo spells. Every other shape lifts as before: flat
    lines, or a `nix file` when a declaration has no spelling. Clause's
    one-line `options = { … }` therefore stays flat.
- **Strict until a program needs more (compass 3).** `option P` with
  nothing under it is an error, and so are a group inside a group and a
  `for` inside a group, each at its line.

Rejected:

- **Grouping whenever every option shares one prefix:** it changes the
  bytes of 15 Beagle outputs (49 if a lone option counts), among them
  kanata (§9.5) and the gates of 2.23b, 2.23c, 4.13a and 4.13b. A rule that
  cannot write a Beagle module is wrong (compass 4).
- **A marker word (`options P`, an annotation):** `options` beside `option`
  is a second keyword for one concept, one letter apart, which is what
  D-42 cut from `text` beside `Text` (T5's principle 8). The group's
  indentation already says it.
- **socrates-chat as a `nix file`:** its six options leave the option tree,
  and 2 of 2 seeded misspellings pass. An importer workaround is a
  language bug (compass 13), as D-63 ruled for selections. D-59's `nix
  file` is for a type that states no contract, and all six of these types
  state one.
- **Rewriting socrates-chat's `.bnix` flat in nixos-config:** that is a firn
  rewrite around a missing construct, which D-66 rejected for plato, and
  it would move the pin. The grouped block is also the usual way NixOS
  modules declare options, so the next module would hit the same gap.
- **Parse identity for socrates-chat alone:** 4.13e counts every Beagle
  output byte-identical with no file excluded, and compass 4 needs the
  language to write it.
- **A printer heuristic:** nothing in the meaning separates socrates-chat
  from kanata, so the bytes would rest on a guess.
- **`option` repeated on each line under the group:** it repeats the word
  the opening line carries (D-61 addendum 4).

Kept: flat `option` lines, which 50 Beagle outputs need. Whether flat
lines give way to groups once T2's idiomatic layout frees the bytes is
notebook O14.

Box 2.23d: 1 box, 20 minutes. Amended: 2.23b, 4.13e, and the firn critical
path.

### D-67. firn's drift past the pin: the lane measures its merge base, the tests re-pin, and sources sit beside their outputs

A one-shot Opus planner on DECISION 4.13 (peer file 2686), 2026-10-12. Read at main `78b7827`,
nixos-config main `4bcfad4c` and lane `farm/galileo-4-13` at `f595ace7`.

**Measured.**

- nixos-config main is 214 commits past the pin `10740a8`. Since then it has:
  - added 22 Beagle modules;
  - retired the hosts ashashi and thinkpad-x1e and `darwinConfigurations` (`7d7a8c9d`);
  - rewritten `flake.nix`, which now sets `system.configurationRevision` from `self.rev`
    (`1ee5dc59`);
  - gained a Clause module, for 185 outputs;
  - added `host-deploy` to `native/firn.clause`;
  - deleted box 4.21's eight validator fixtures as orphan scaffolding (`5bf48226`).
- In the lane, 185/185 Clause outputs are parse-identical and 57/69 Beagle outputs are
  byte-identical. Scratch imports of all 69 at main find **eleven** `lib` and `builtins` members
  with no signature, in 9 files: `builtins.hasAttr`, `builtins.path`, `lib.concatLists`,
  `lib.escapeShellArg`, `lib.escapeShellArgs`, `lib.mkOptionDefault`, `lib.optional`,
  `lib.types.enum`, `lib.types.listOf`, `lib.types.nonEmptyListOf` and `lib.types.nullOr`.
  Three hid behind import errors and six behind the printer's lexer error.
- Nexus job 64 evaluated whiterabbit's drvPath both ways:
  - As `path:` over `git archive`, base and lane both give `/nix/store/pdfdnsb9…`. The lane adds
    254 `.gal` files and rebuilds 185 `.nix`, and neither changes it.
  - As git flakes they differ (`h5wjkzrm…` against `y6d9hnhk…`), which `self.rev` explains.

  Statically, every relative path in a module names one file, copied by its content, and nothing
  turns `firnModules` or `self` into a string.

**Rulings.**

- **WP4's Done when measures the migrated lane against its merge base on nixos-config main.**
  That is what 4.14 lands, and PLAN already says a lane compares against its own merge base
  (D-32). The drvPath comparison evaluates `path:` copies (`git archive`) of the two trees, where
  `self` carries no revision. A comparison of git flakes can never pass, since
  `configurationRevision` changes with every commit. SPEC §1.3's "no host embeds the flake
  revision" (F15) was true at `10740a8` and is restated.
- **galileo's tests re-pin to `4bcfad4c`,** the lane's merge base, so its gates cover the corpus
  the lane migrates (4.13e, 4.13f and 4.13g). `10740a8` retires once nothing reads it.
  - When main moves again before 4.14, the lane rebases and re-imports by script.
  - An output that then fails gets a box.
  - The pin moves only when a gate needs that output.
- **ashashi and darwin are reported as gone.** A3's darwin drvPath has no host at the merge
  base, so the report says "no darwin host since `7d7a8c9d`". A4's ashashi result stands as
  measured at `10740a8`. The tests' ashashi and thinkpad-x1e cases go with their input.
  `lib.mkDarwinSystem`, which `flake.nix` still defines, is covered by `flake.nix`'s parse
  identity. `config/darwin-modules.json`, which `flake.nix` still reads, keeps its
  `X supports darwin` facts.
- **Sources sit beside their outputs, with no filter.** `modules/X/default.gal` sits beside
  `default.nix`, which is where `galileo build` writes and where firn's readers look (D-9,
  SPEC §2). Job 64 shows the drvPath does not see the sources.
- **Each of the twelve has an owner.** Language: D-66 (2.23a, 2.23b, 2.23c). Printer and
  importer: an entry's `when` follows its value's last line, which also lifts github-pat's
  guarded text block (4.13a). nix-lib: the eleven signatures, which the D-45 addendum assigns to
  the box that builds those files (4.13b).
- **Lane choice: `darwin is a Platform` in `config/tags.gal` stays.**
  - The platform facts mirror a file main still ships.
  - With no darwin host, nothing else declares the entity, and an undeclared name is an error
    (D-49).
  - `platform_facts` declares the platform it names, so the tests' program matches the lane.
  - The declaration goes when `darwin-modules.json` goes, which no WP4 box does.
- **Lane choice: `config/hardcoded-repo-paths.tsv` lists `theme-switcher/default.gal`.** The
  `.gal` is the source that holds the path, the file's own rule is that a coupling is declared by
  hand, and its hook required the entry. The `default.bnix` line goes with that file (4.34).
- **Drift outside the twelve.**
  - `dispatch.gal` gains `host-deploy` (4.13g).
  - 4.21's eight-fixture check, whose oracle main deleted, becomes `firn repo validate` on the
    lane and on a misspelt scratch copy.
  - A D-33 row whose Done when names a firn test means that test at the lane's merge base.
  - 4.13 regenerates its `.gal` by script, so syntax boxes such as 2.21s cost a rerun, not
    hand edits.

Rejected:

- **Keeping `10740a8` for galileo's tests and measuring main only in the lane:** the gates would
  keep testing two retired hosts, and eight of the eleven unsigned calls would surface only at
  4.13.
- **Freezing nixos-config's modules until 4.14 lands:** it stops Tom's system work for a
  migration step.
- **Git-flake drvPaths with `configurationRevision` overridden:** that changes the flake under
  test.
- **A source filter on `./modules`:** it guards nothing measured, it changes `flake.nix`, and
  the rebuilt Clause `.nix` move that store path anyway.
- **Deleting `darwin-modules.json` and the darwin facts now:** `flake.nix` reads the file, and
  removing it is nixos-config's call.
- **One box per module:** the twelve share four causes.

Boxes 4.13a, 4.13b, 4.13e, 4.13f and 4.13g, 5 boxes and 90 minutes. Amended: 4.13, 4.5b, 4.20,
4.21, 4.23, 4.34 and 4.37.

### D-67 addendum. firn_responsibilities re-pins too, and the farm's pin moves with the last reader

A one-shot Opus planner on DECISION 4.13f, 2026-10-12, at main `dae0239` and the firn pins
`10740a8` and `4bcfad4c`. No cargo was run.

**Measured.**

- `crates/galileo-cli/tests/firn_responsibilities.rs` reads the pin through
  `tests/external/firn-tags/fixture.rs`, so D-67's list of gates missed it. 4.13f moved
  `fixture::firn_root` to `4bcfad4c`, and its worker gave the test its own `firn_root` at
  `10740a8` (`4f049af`).
- At `4bcfad4c`, whiterabbit's `enabled-tags.bnix` adds `blender` and `obsidian` to `:disabled`.
  Both are still tag-enabled (`creative`, `notes`), and `responsibilities.bnix` is unchanged, so
  the whiterabbit budget check exits 0 where the test expects 1. firn's own test at that commit
  requires `live-budget-passes?` for whiterabbit, so the merge base's calculation passes too.
- The same commit rewrites `native/responsibility_projection_test.bjs`: `(Vec t/Triple)` and
  `(t/triple …)` become `(Vec projection/Triple)` and `(projection/triple …)`, and hyphenated
  keywords become `(keyword "project-build")`. The test's scenario reader splits on the old
  spellings, so past the whiterabbit assertion it would find one scenario of seven and panic.
  The seven scenarios' triples and boot lists are otherwise unchanged.
- `.github/workflows/farm.yml` runs every test with `FIRN_PIN: 10740a8…`, and 4.13's
  `rg -l 10740a8 crates tools tests` does not search `.github`.

**Rulings.**

- **The worker's reading stands.** 4.13f's Done when names neither the test nor its
  expectations, and keeping the test at `10740a8` kept main green without guessing new ones.
  D-67 still governs: galileo's tests re-pin to `4bcfad4c`, and `10740a8` retires only once
  nothing reads it, so the test gets its own box before 4.13 (4.13h).
- **At `4bcfad4c` whiterabbit fails no budget part,** the merge base's verdict. The scenario
  reader reads `$P`'s spellings only, and the seven scenarios keep their expectations.
- **4.33's Done when follows the merge base:** the budget passes at `$P`, and a scratch copy that
  takes `blender` and `obsidian` off whiterabbit's `:disabled` list fails naming both, the inverse
  of the scratch case it named at `10740a8`.
- **The farm's `FIRN_PIN` moves to `4bcfad4c` in 4.13's retirement step,** once no test defaults
  to `10740a8`, and 4.13's search adds `.github`.

Rejected:

- **Re-opening 4.13f for it:** 4.13f landed and ticked, and its Done when never named the test.
- **Keeping the test on `10740a8`:** it would gate a tree no lane migrates, and the pin could never
  retire (D-67).
- **Reading both `.bjs` spellings:** a compatibility path for a tree that retires with 4.13.
- **A scratch whiterabbit that re-enables both, inside the test:** `propositions-blender` already
  fails the environment-module requirement, and the real host passing is what tests direct
  disables. The scratch case is 4.33's, which runs the budget on the lane.
- **Moving `FIRN_PIN` in 4.13h:** 4.13e's gates still default to `10740a8`, so the farm would fail
  them until 4.13e lands.

Boxes: 4.13h, 1 box and 20 minutes. Amended: 4.13 and 4.33.

### D-68. `galileo check` reports what lowering finds, and every located error prints `FILE:LINE:COLUMN:`

A one-shot Opus planner on the gap the 2.22c tick recorded (`a6e5ccc`), 2026-10-12, at main
`dae0239`. The probes ran the CLI built at `631b538`, which contains `650ddb5`. No cargo was run.

**Measured.**

- Eight diagnostics goldens print an error under `galileo build --check` and `exit 0` under
  `galileo check`: the seven `computed-*` classes captured, duplicate-formal, keyword,
  not-identifier, shadows-literal, shadows-prelude and unbound, and `keyword-lambda`. No other
  golden differs so.
- One cause: `galileo check` runs `analyze` (check.rs) and then the claims, and never lowers a
  Nix body. The D-65 scope walk and the keyword-binder error sit in `nix.rs`'s lowering, which
  only `emit` calls. The four D-65 errors that the parser or `analyze` raises (after-dot,
  bare-name, let, nix-stage) appear under both commands.
- Those eight print `FILE:line N:C: message` and no source line: `emit` prints a lowering error as
  `"{file}:{e}"`, and the lexer's `Error` displays as `line N:C: message`. `analyze`'s
  `Diagnostic` and main.rs's `located` print `FILE:N:C: message` and the source line indented four
  spaces, as 179 lines in 77 goldens do. `galileo fmt` (`"{path}:{e}"`) and the flake root-`let`
  error (`"{}:line {}:"`) print the `line` form too, and `fmt_check.rs` and `nix_lowering.rs`
  assert it.
- The scope walk stops at its first error. A scratch `nix file` whose records `h:` and `k:`, on
  lines 5 and 6, each capture `pkgs` gets one diagnostic, at line 5, from `build --check`, and
  `exit 0` from `check`. SPEC §10.3 makes a flake input named `pkgs` an error at both
  home-manager records, and 4.5b's Done when counts exactly two.

**Rulings.**

- **`galileo check` reports every error `galileo build` finds before it prints Nix (SPEC §6.1).**
  Static checks cover the whole program before any claim is evaluated, and a static error exits
  2 with no verdict. A computed name's error is static: §10.3 makes it an error at its line,
  decided when the `for` unrolls, with no Nix evaluated. A check that exits 0 on a program that
  `build` refuses approves what cannot be built. So `check` runs the lowering `emit` runs, after
  `analyze` finds nothing and before any claim, and a lowering error prints and exits 2. It
  does not run the `nix-instantiate --parse` gate, which tests emitted text, not the program;
  `build --check` keeps it, and `firn repo validate` runs both (§11.3).
- **One location format: `FILE:LINE:COLUMN: message`, then the source line (SPEC §6.1, G6).**
  It is what the parser, `analyze` and `located` already print, and what editors and agents
  jump from. Every located error the CLI prints goes through one renderer, and an error with no
  line keeps `FILE: message`. The library's `Error` display, which knows no file, keeps
  `line N:C:`.
- **Each scope error is its own diagnostic (SPEC §10.3).** The walk records an error and goes on,
  and a computed formal that errs still binds its name in its body, so one bad name gives one
  diagnostic, not a cascade. Every other lowering error still stops its file.

Rejected:

- **Moving the scope checks into `analyze`:** the checker would need the `for` unroll and its
  query answers, a second unroll beside lowering's, where 2.22c asked for one scope (compass 5).
- **Leaving them to `build --check`, since firn runs both:** §6.1 makes `check`'s exit 0 mean no
  static error, and north's CI runs `check` without `build` (§11.3).
- **`check` calling `build --check`:** drift and the parse gate would decide a requirement
  verdict, and `check` would need `nix-instantiate`.
- **`FILE:line N:C` everywhere:** 8 goldens print it against 77, and no tool reads it.
- **Collecting every lowering error:** only the scope walk has a program that needs two
  (compass 3).

Boxes: 2.22d (`check` lowers, one format) and 2.22e (every scope error, after 2.22d), 2 boxes and
40 minutes. Amended: SPEC §6.1 and 4.5b.

## 2. Decisions Tom made

The orchestrator relayed Tom's choices on 2026-10-10. On T1 to T3 he took
the recommendation; T4 and T5 are his own direction. T6 is his pick of
2026-10-12, relayed by his proxy, and T7 to T9 are his picks of the same day
on the best-case triage, each the recommendation.

### T1. firn's mechanics move to TypeScript on Bun

firn's 59 `.bjs` files hold 23,268 LOC (F12). When Beagle's JavaScript
retires, they become TypeScript on Bun and call `galileo` through its CLI and
JSON output. Decision logic moves into Galileo rules either way (D-2).

Rejected:

- **Rust with the Galileo crates embedded:** firn already runs on Bun
  (`native/firn_host.mjs`), Bun is Tom's default for JS and TS, and porting
  Beagle's emitted JS to TypeScript is mechanical.

### T2. Idiomatic Nix layout once identity is proven

The emitter keeps Beagle's byte layout through WP4, because byte identity is
the migration proof. After WP4 it switches to idiomatic `let … in` in one
commit, verified by whiterabbit's drvPath and by the capture check of PLAN
box 4.15.

Rejected:

- **Beagle's layout permanently:** nothing depends on the bytes once the
  migration is proven, and `let` written as an immediately applied lambda
  makes the generated Nix hard to read.

### T3. Clause is frozen now

Clause gets no new features from today. nixos-config and north stay on their
current Clause pins until acceptance A3 (firn) and A11 (north replaced)
pass, and then Clause is archived. Tom's choice named A3 and A8. Review
finding M01 then split the old A8 into the menu slice (A8) and north
replaced (A11), and north uses Clause until A11, so the archive waits for
A11.

Rejected:

- **Keeping Clause alive in parallel:** north's adoption ratchet repinned
  Clause 29 times in 5 days (N8), and Galileo takes over its role.
- **Retiring it only after both migrations, with no freeze:** new Clause
  features would widen what Galileo must import.

### T4. Galileo aims to be a general-purpose language

Tom, 2026-10-10, close to verbatim: "I don't see a reason why this can't be
a general purpose language. I have specific use cases, like a layer for Nix
and handling certain logical concerns in north, but I am aiming at a general
purpose language. Over time it could also handle concerns like game
development if it's in my game development stack. Aim that high rather than
become something overly narrow."

So the destination is a general-purpose language, and firn and north stay
the first two targets because they prove it on real work. D-2 and D-12 are
rewritten to match, SPEC §1 states the direction, and SPEC §12 turns each MVP
cut into "not yet" with what brings it in. A few things stay out by design
(SPEC §12.2), such as I/O inside Galileo code, evaluating Nix and free
English, each with how its need is met instead. PLAN.md sketches the first
general-purpose milestone after A11. The acceptance scenarios, the MVP's
size and the firn-first critical path do not change.

The planner went through the SPEC and the open boxes for choices that would
block a general-purpose future. Three change now:

- **Only the CLI and the LSP touch the operating system** (SPEC §10.4,
  AGENTS.md). Every other crate takes and returns data, so a browser or
  C-ABI host can embed the runtime without a port. It costs nothing while
  those crates are being written and a refactor once they are. Box 8.1's
  Done when gains one `rg` check, and every crate on main already passes it.
- **`1.5` is reserved** for fractional numbers and is an error in the MVP
  (SPEC §9.1), so no other construct takes that syntax first. No box
  changes: 2.1's lexer splits it into `1`, `.` and `5`, 2.8 hands that on as
  an expression, and SPEC §9.4's `expr` rule has no form that accepts it.
- **G1** admits a milestone program as the runnable example after the MVP
  (SPEC §1.4), so a general-purpose construct has a way in.

The rest stays compatible as specified:

- **Numbers.** `Int` never converts implicitly, so a fractional type changes
  no program (D-12). Wrapping and bitwise operations come as named
  built-ins.
- **Functions.** The Core IR calls definitions by name and has lambdas and
  application (box 3.1). Recursion needs only a new effect level, `partial`,
  under a budget, and rules and requirements keep calling total definitions.
- **Rule recursion.** Stratified negation and no value invention in
  recursion stay. Lattice-valued relations and well-founded negation are
  conservative: they give meaning only to programs that today's checks
  reject.
- **Data types.** `type Name = …` and definition bodies leave room for sum
  types, `match`, local `let` and generics (SPEC §9.4).
- **Effects.** The pure core with requests and receipts (G4, D-15) is the
  general-purpose I/O model, not a limit: Elm and Roc run applications the
  same way (A-Elm, A-Roc). A step that waits on a read is a later extension,
  with the host's answer logged like a receipt.
- **Performance.** `M(S)` is the semantics, so incremental evaluation and
  compiled programs are fast paths under G7. A8 already budgets one step at
  1,000 rows under 1 ms, well inside a 60 Hz frame.
- **The runtime** fits any host. It takes bytes and events and returns
  effects and views, through a Rust API or JSON lines (SPEC §10.4). Other
  bindings come with their host.
- **Syntax.** The sentence register stays for relations, and computation
  lives in the record register's expressions, which grow by block bodies.
  Hyphenated names stay, so binary minus needs spaces (`count - 1`), as in
  Clojure and the other Lisps.
- **Determinism.** Galileo code has no clock and no randomness (SPEC §5.3).
  That is what gives a game lockstep and replays; time and seeds arrive as
  event arguments.
- **State is data.** Functions stay out of facts and state, which keeps
  checkpoints, hashes and explanations possible.
- **The Nix stage.** `Nix T` is the first target stage. A second
  code-generation target generalizes it, and no MVP program changes.
- **T1 stands:** firn's `.bjs` mechanics go to TypeScript on Bun now.
- **D-33's machine-capacity ruling stands:** its hook must not depend on a
  second binary, though F64 alone would no longer rule it out.

Rejected:

- **Keeping D-2 as written:** Tom's direction replaces it.
- **Adding floats, recursion and sum types to the MVP now, to be safe:** none
  is expensive to add later (above), and each would grow the MVP for no
  target that needs it.

### T5. The design direction: earned syntax over one relational term

Tom gave Galileo's design direction on 2026-10-10, relayed by his proxy in
several parts. It is the rubric every language ruling is judged against
(`notes/language.md`). Tom approves direction, not syntax, and is never
asked about syntax (D-36). He called it "not a table flip": it sets the
shape, so landed work is steered rather than restarted.

1. **Turtles all the way down.** One uniform relational term unit that
   composes, as in Beagle store's term design. Minimize special cases.
2. **Every piece of grammar, syntax and symbol is deeply earned.** The test:
   does it objectively make code faster to scan, faster for an agent to
   understand, and faster for an agent and a human to share understanding?
   If not, cut it. Mainstream languages still carry more syntax and rules
   than they need, so Galileo carries fewer.
3. **Relational structure stays visible.** Lisp throws structure away for
   uniform parameter handling, which is lower information density and
   slower to parse, and that is why Beagle was abandoned.
4. **A precise subset of English, not literal English,** with mathematical
   terminology where it earns its place: neither bespoke half-English nor a
   wall of symbols.
5. **The best version of Clause.** Clause holds most of the intent
   (relational, process-first design), but its implementation drifted into
   correctness rabbit holes and lost the goal: readable code that structural
   questions can be asked of.
6. **Types and semantics follow from a few principles and constraints,** and
   relational graph semantics strong enough to support physics, that is,
   general-purpose and game use (T4).
7. **From here the design is mechanical:** derived from Clause, Beagle store
   and these principles.
8. **Words that are hard to hallucinate.** The vocabulary combines a precise
   subset of English with the terms, semantics and notation of formal logic,
   mathematics and philosophy. A word is a strong candidate exactly when it
   has one established, precise meaning that no reader, human or agent, can
   plausibly misread; such a term beats a coined keyword or a casual English
   word whose meaning drifts.
9. **The priors models already carry.** Models are trained mostly on
   English, and their most advanced training uses the established notation
   of philosophy, mathematics and law. Law is especially strong on
   ownership, provenance and relationships, which is why it shaped Clause's
   prose. Reuse those priors rather than inventing new ones.
10. **Code that needs no comments.** The code is semantically rich enough to
    be its own explanation, and a comment that restates code marks a
    language-design failure. A construct is tested by whether a reader still
    needs a comment to know what it means.
11. **The guiding star: "Imagine Lean, but it reads a little more like
    English and supports importing your own domain semantics."** From Lean:
    every statement has one checked meaning, and domain notation and
    vocabulary are imported as libraries, as in mathlib. Not from Lean: a
    proof-driven core or any proof burden on the writer; the checker does
    the work and the writer only states things. The core stays relational,
    nearer Datalog and Prolog than dependent types. In short: Lean's checked
    meaning, Inform 7's readability, Datalog's relational core, with
    importable domain vocabularies.
12. **Succeed or fail early.** With no training on Galileo, an agent given
    only the compressed language card writes non-trivial, useful programs in
    a whole class (rules, relationships, domain logic) at least 2x faster
    than in Python, with the goal 3–5x, and a first-try pass rate at least
    Python's; otherwise the language is useless to Tom. A zero-shot
    authoring benchmark measures it, with dev and held-out task sets, a
    token-capped card tuned to a plateau, and failures classified as
    harness, misread, expressiveness or tooling, so harness failures never
    count against the language. It runs again at the terminal-dungeon
    milestone for general-purpose programs.
13. **Done means two things together:** agents write the target class of
    programs 2–5x faster than in Python (principle 12), and Tom, shown the
    same program side by side in Galileo and in Python, finds the Galileo one
    more readable. The benchmark report ends with one such pair.
14. **Contracts, not types.** What must hold of values, relations and
    behaviour is stated as plain checked statements, consistent with
    requirements, claims and verdicts, in an expressive contract system
    rather than a type hierarchy.
15. **Immutable and declarative at its soul.** A program states what is
    wanted, as facts, rules and constraints, never steps. State change and
    effects (WP7, general-purpose and game programs) are rules about what
    follows from events, never step-by-step mutation, so every sequenced
    step, assignment or loop either has a declarative reading or goes.
16. **Take the best ideas from projects Tom agrees with,** as Clause and
    Beagle did, mining their design notes and decision records first. He
    welcomes direction questions, above all on mutability against
    immutability, brought as the tension, its two best options and a
    recommendation, never as syntax.
17. **The survival bar, reset (2026-10-11, via the proxy).** TypeScript is
    the primary comparator, because it has a static checker. On medium and
    complex programs with state logic, Galileo's median time-to-correct must
    be 0.50 to 0.67 of TypeScript's, with a first-try rate at least
    TypeScript's; on small programs the bar is parity. Python stays a
    reference row, and Soufflé the closest-paradigm check, which Galileo
    should not lose to. This replaces principle 13's 2 to 5x over Python
    for the benchmark. Tom's words: "i guess than we need to be 1.5-2x
    faster than typescript"; "realistically on small programs it's a
    wash".
    Addendum (2026-10-12, via the proxy, after the external-review triage):
    Tom answered "yes" to keeping this bar on behaviour alone. "Behaviour
    plus review guarantees" is reported beside it, with no pass line until
    one measured run exists. He also answered "yes" to SPEC §1.1 stating
    that review judges a change against the frozen facts, the recorded runs
    and a bounded search, not every possible input.
18. **Fields fixed, history permanent (2026-10-11, via the proxy).** Tom
    answered "yes" to "fields fixed, history permanent, as-of and
    why-over-time queries". It confirms D-47's option A and extends it:
    nothing is ever overwritten. A `then` sentence adds a fact at the next
    revision and never replaces the old one; the current state is the
    latest fact per key, and history is kept. Questions gain "as of
    revision N", "since", and `why` across revisions, Datomic- or
    Dedalus-style, on 7.3's event log, replay and revision hash. "Now"
    stays fast through a materialized current view, and no query replays
    history. Compaction is planned, not built. `then` reads the same
    (D-52).

Tom's words, from voice transcripts, verbatim:

> "I don't wanna get too much into the implementation details other than
> like a couple of different manifesto ideas turtles all the way down which
> comes from beagle store in the term design right I generally like
> minimizing special casing, anything unnecessary grammar, and syntax that
> isn't like deeply fucking earned ... lisp is just harder to ... parse
> because it collapses relational structure in the way that it formats
> function calls ... lisp throws away structure to make handling parameter
> simpler and so that's why I basically punted on beagle ... I shifted more
> towards maintaining a kind of relational term unit that composes and
> reads like English, but it can't just read directly like English.
> Obviously we have to choose a subset of English that's precise enough
> and then also lean into mathematical terminology where it's valuable ...
> is that really making the code faster to scan in an objective manner is
> this really going to make an agent be able to ... understand the code
> faster and coordinate with the human on that understanding faster or we
> now back to like having to learn a whole bespoke half English way of
> talking that just sucks ... I do think we need some symbols in
> terminology obviously but I find that in most languages it's still too
> much ... a lot of my thinking about what I wanted this to be is already
> existing in clause. It just started to drift to become this kind of
> leviathan that escaped from the original objective of having the code be
> more readable and being able to ask certain structural questions about
> it ... everything should basically follow from a couple of key
> principles and constraints in terms of at the end of the day having the
> language support physics, you know that falls out of a kind of graph of
> kind of like a relational semantics ... It should be pretty mechanical
> after that ... I just look at those two languages, especially clause,
> which is more recent on the relational process first Design and then
> just think what's the best version of this right"

> "This is some kind of combination of a subset of English with like
> formal like logic and math and philosophy, kind of verbiage and semantics
> and notation wherever the meaning is like very difficult to hallucinate
> makes it a very strong candidate for a semantic word that we will use"

> "AI is trained predominantly on English sentences, and some of the more
> advanced training ... is trained on sentences that use very well
> established notation in philosophy in mathematics in law; laws are very
> good reference because law has to be very specific ... which is why I
> think it ended up in the prose of clause because law has already a lot
> of priors that can be useful for being extremely clear about ownership
> and provenance and relationships. I'm not trying to flip over the table
> here, but I'm just trying to set the shape of like what I think should
> be possible ... the goal should be at the end of the day that we don't
> need code comments anymore. the code is the comment the code is
> semantically rich. We don't need a separate like comment header to
> describe what the code is doing that to me as a failure in programming
> language design where that work really should be in the compiler level
> but because people don't write very good compilers, they make human
> beings suffer the consequences."

> "imagine Lean, but it reads a little more like English and supports
> importing your own domain semantics"

> "there's no reason why we can't have a very expressive contract system,
> I prefer to call it a contract system rather than a type system. A type
> system is just a very stupid antiquated way of thinking about what type
> types are actually trying to do."

> "it's like immutable and declarative. We're not really telling the
> computer what to do. We're telling you what we want or we're setting
> constraints ... it's more about setting constraints and describing
> rules."

### T6. A dependency requirement on a program with events is judged in every reachable state

Tom picked this in his own picker on 2026-10-12, relayed by his proxy, from
an Opus planner's recommendation at main `465284f`
(`~/.local/state/agents/handoffs/galileo-requires-scope-recommendation.md`).
It settles notebook O13, which D-64 left open. The question: "Should a
dependency requirement (`S requires C`) on a program with events be judged
in every reachable state? Then a bypass that only fires later fails
`galileo check`, and when the search cannot finish the claim reports
Unknown instead of passing."

- **The rule (SPEC §6.2).** On a program with events, `S requires C` is
  judged in every reachable state, through 7.4's search. The claim Holds
  only if the search covers every state. It Fails as soon as one state
  fails, with the steps that reach it and the rule that bypasses (D-64's
  surviving proof). It is Unknown when the search cannot finish. A single
  point of failure over a sentence is judged the same way. `--events`
  changes neither, as it changes no temporal claim (7.3b), and a program
  without events, all of firn, has one state, so nothing changes there.
- **Why.** The claim is judged in every state its rule can actually run in
  (the executable rule); a failure names how to reach the bypass and which
  rule bypasses (the explanation); and the review's own door-and-key
  bypass, which passes `check` today, fails it (the enforceable
  requirement). It matches SPEC §6.2's first sentence and the review
  record's S3, and `only after` and `only when`, the other forms whose left
  side is universal, already range over every path. It is the strict
  reading (compass 3): no program states such a claim yet, so loosening it
  later breaks nothing, while tightening it later could.
- **Cost, measured** on a release build of galileo-core at `465284f`, whose
  `claims.rs` and `search.rs` main still has, on scratch copies of the
  dungeon, 3 runs at load about 20 on 24 cores. The base dungeon has 19
  reachable states, enumerated in 34 ms (complete). Judging `requires` in
  all 19 takes 7 ms against 0.3 ms in the initial state alone: about 20x,
  linear in the number of states. `Armory-Gate opens for Ash requires Ash
  holds Bronze-Key` goes from vacuous to Holds, with instances in 8 of the
  19 states. With the bypass, 27 states in 13 ms, `Shrine-Door opens for
  Ash requires Ash holds Silver-Key` Fails at depth 3, after Front-Door,
  Library-Arch and Crypt-Stair, where the initial-state check Holds
  vacuously. Firn costs nothing: it has one state. On north's menu slice
  every such claim that should Hold is Unknown (open domain), as `always`
  and `never` are there under 7.4b: 5 of its 9 events have open `Text` or
  `Int` slots (`examples/north-menu.gal:71-81`) and `MenuRow` creation is
  unbounded. A bypass the search finds there still Fails, and the claims
  can Hold once those slots are closed with `only in`.
- **No requirement changes.** Firn's 8 requirements are `every` and `no`
  state claims (`lib/firn/tags.gal`, `lib/firn/responsibilities.gal`, whose
  `requires` is a template word), north states none, and the dungeon's five
  use no `requires`.
- **Build:** box 7.4c, 20 minutes, after 7.4b and before 3.11e, which would
  otherwise print `in the initial state` on these verdicts.

Rejected:

- **The initial state, with `always (S requires C)` for every state:** the
  bare line reads as a guarantee and passes vacuously (both probe claims
  are vacuous initially), and `always` would have to take a claim that is
  not a state claim.
- **`requires` as an `invariant`, checked at each step:** it catches only
  runs that actually happen, after shipping, costs a counterfactual
  re-evaluation on every step, and D-30 admits only state claims.

### T7. Galileo stays the program that runs; a backend comes only when a program needs it

Tom answered "yes" in his own picker on 2026-10-12, relayed by his proxy,
to question 1 of the best-case triage
(`~/.local/state/agents/handoffs/galileo-external-review/best-case-triage.md`,
main `68af1be`): "Keep Galileo the program that runs, not a specification
beside one, and add a new backend only when a program needs it, so
'primary authored representation compiled to TypeScript, Rust or Python'
stays out of the plan?" It confirms SPEC §1.1 (no second model beside the
implementation), §12.1, G1 and T4, and rejects the review's claim 3.

### T8. Galileo stays private until firn and north run on it and the review-effort test has run once

Tom answered "yes" to question 2: "Keep Galileo private, with no ecosystem,
commercial, packaging or outside-user work, until firn and north run on it
and the review-effort test has run once?" No PLAN box adds an ecosystem,
commercial, packaging or outside-user deliverable before both conditions
hold: firn and north run on Galileo (4.14 and WP8), and AF.49 and AF.50
have run once. It rejects the review's claim 5 for now.

### T9. The review-effort test uses AI reviewers only

Tom answered "yes" to question 3: "Run the review-effort test with agent
reviewers only, and no review session for Tom?" AF.49 and AF.50 use two cold
agent reviewers per card, one reading the intents, requirements and
`galileo review` output, the other the intents and the source diff; no box
asks Tom to review.

## 3. Process notes

- **Where the inputs went.** They were moved from the container root into
  `main/` through an orphan lane. Both archives were extracted into fresh
  scratch directories, read, scanned for secrets, and imported as source
  (27 of 27 and 6 of 6 files).
- **What each reviewer saw.** Only `SPEC.md`, `PLAN.md` (the
  migration-engineer lens) and Galileo's own `inputs/`. No secrets,
  nixos-config or north files, transcripts, or `~/code/clients`.
- **Gemini terms.** Google may use Gemini prompts for training unless that is
  turned off in Antigravity's settings. Gemini received the SPEC.md text only,
  on Tom's named request.
- **Capacity.** The workers' capacity hook refused two of the five research
  workers at first (24–32% desktop pressure). Both ran a few minutes later.

## 4. Review record

Three reviews were run against the SPEC as it stood at 17:58, the
migration lens with PLAN.md as well:

- **Astra xhigh** through `codex exec` (`gpt-6-astra`), lens "language
  designer: soundness and expressiveness".
- **Astra xhigh**, lens "firn/north migration engineer: can this replace
  today's `.bnix` and Clause programs".
- **Gemini 3.1 Pro (high)** through `agy`, lens "adversarial: find the
  semantics holes".

Each finding was weighed by its evidence first: a failing scenario counts
whoever raised it. Every finding came with a scenario that fails against the
reviewed text, so all 45 are accepted. Gemini's fixes and the self-review
fixes below landed while the Astra reviews ran. So LD-13 was already
fixed, and LD-06, LD-14, M05 and M06 were fixed in part.

| Reviewer | Findings | Severity | Accepted | Rejected |
| --- | --- | --- | --- | --- |
| Gemini, adversarial | 9 | 2 critical, 3 high, 3 medium, 1 low | 9 | 0 |
| Astra, language designer | 18 | 2 critical, 10 high, 5 medium, 1 low | 18 | 0 |
| Astra, migration engineer | 18 | 1 critical, 12 high, 5 medium | 18 | 0 |

### 4.1 Gemini, adversarial

| ID | Finding | Change |
| --- | --- | --- |
| BooleanLiterals | The menu rule filters on `contains(…)`, but no rule literal accepts a boolean expression | SPEC §4.3 and §9.4: a boolean expression is a literal that holds when it is `true` |
| NixInFacts | A tag override's `[pkgs.firefox]` is a `Nix` value, which could not enter a fact | SPEC §3.2: opaque `nix T` payloads that can be stored, bound and emitted, never decided on |
| UndefinedTagRelation | `_generated-enables` read `gets … set to … by`, which `firn/tags` never declared | SPEC §9.6: the shapes and the `tag-overrides` rule |
| MissingQuerySyntax | `all … where …` was missing from the grammar | SPEC §9.4: `query` |
| MissingAggregateSyntax | `aggregate` was referenced but not defined | SPEC §9.4: `aggregate`, including argmin |
| NestedForSyntax | A `for` block could not nest inside an entry | SPEC §9.4: `for-block` is an `entry` |
| UnquantifiedAmong | `among ?key lies in ?room` broke the explicit-quantifier rule | SPEC §6.2: `requires` and `among` bind their own variables |
| MenuEmptyReject | Moving the selection in an empty menu rejected, where Clause admits an empty step | SPEC §9.7: `may skip`; §5.3: the importer marks every handler `may skip` |
| MissingArgsParamSyntax | `args` and `param` were not defined | SPEC §9.4: both rules |

### 4.2 Astra, language designer

| ID | Finding | Change |
| --- | --- | --- |
| LD-01 | A membership `?t in [?s ++ "x"]` in a recursive rule grows text forever | SPEC §4.3: no value invention covers memberships, calls and head expressions; PLAN 3.2 tests it |
| LD-02 | `invariant every Clock ticks happens when Clock is ready` makes admission depend on itself | SPEC §6.3: invariants and `ensures` take state claims only (D-30); PLAN 7.2 |
| LD-03 | A query inside a definition hides a negative cycle (`chosen` from `not chosen`) | SPEC §4.3: a query in a rule counts as an aggregate over what it reads; PLAN 3.2 |
| LD-04 | `?x = ?y` with `?y = ?x` passed the safety check | SPEC §4.3: safety by a grounding schedule; PLAN 3.2 |
| LD-05 | Searching only `"ok"` proved `never compromised` while the runtime admitted `"boom"` | SPEC §5.5: closed and open slots, the event alphabet in every answer (D-26); PLAN 7.4 |
| LD-06 | Views and handlers queried `M(static)` | Views already read `M(S)`. Added: SPEC §4.5 gives every query context its model, and "known" means evaluated by Galileo (§3.2) |
| LD-07 | `delete` left the entity live, and an include could resurrect its facts | SPEC §5.3: `(live \ D) ∪ C`, and an include of a non-live entity rejects; §4.1: static facts cannot mention state entities |
| LD-08 | The allocator was outside the state, so equal states could allocate differently | SPEC §5.1: the allocator is in the state, its hash and checkpoints (D-31); PLAN 7.3 |
| LD-09 | Nothing could emit the flake root, which is not a module | SPEC §9.4 and §10.2: `nix file` (D-25); PLAN 4.5, 8.10 |
| LD-10 | Under negation, a hitting set of current supports need not break the claim | SPEC §6.4: hitting sets for monotone cones, verified search otherwise, and the edge cases; PLAN 3.14 |
| LD-11 | §8.5 forbade the ordinary approval that §5.6 gave `library-ladder` | SPEC §8.5: a patch that fixes every failure is approved normally |
| LD-12 | A requirement that keeps failing can hide a new failing host | SPEC §8.4: comparison by failing instance (D-29); PLAN 6.3 |
| LD-13 | Two equal `set`s conflicted only when already satisfied | Already fixed before the review returned (S1): withdrawals apply before inclusions, so both starting values admit |
| LD-14 | Argmin ties were undefined | Ties already fixed (S5). Added: with no candidate, argmin binds nothing |
| LD-15 | `sum` could overflow or not depending on order | SPEC §4.3: exact sums with one range check; aggregates range over distinct bindings; PLAN 3.4 |
| LD-16 | Rename-insensitive identity hid renames that change emitted Nix | SPEC §8.1: only unobservable names are normalized away; PLAN 6.1 |
| LD-17 | `matches the menu query` dropped its north, so one north's query matched another's rows | SPEC §9.7: `matches the menu query of ?north` |
| LD-18 | The Dedalus correspondence ignored rejection | SPEC §5.3: it covers the post-state only; admission is Galileo's own |

The designer also confirmed the dungeon by its own search over 19 states:
the room set, the Library checkpoint, the warning requirement, the Iron-Key
failure, the ladder repair and the four Silver-Key placements all agree.
Its shortest witness takes 5 steps, not the 6 the SPEC listed, so §5.6 now
gives the 5-step witness, which the ChatGPT oracle's room path confirms.

### 4.3 Astra, migration engineer

| ID | Finding | Change |
| --- | --- | --- |
| M01 | firn's three Clause programs, north's `flake.clause`, the rest of north and Clause's removal had no boxes; A8 meant two things | PLAN 4.10–4.12, 8.10, 8.12–8.15; SPEC §13: A8 is the menu slice and A11 is north replaced; §10.4: `galileo run` and `galileo query` for firn's TypeScript host |
| M02 | The Nix front-end covered only kanata's subset | SPEC §10.2: `nix file`, `with`, `//`, nested modules; PLAN 2.2, 2.3, 4.2 |
| M03 | Attribute names and attribute paths were conflated | SPEC §3.1: `OptionPath`, and the attribute-name rule for text and entities |
| M04 | `contains` and `lowercase` had no signatures, and Clause parity was unchecked | SPEC §3.5: signatures, with Clause as the reference; PLAN 8.4, 8.6 |
| M05 | The menu view read the static model | As LD-06 |
| M06 | An empty move rejected where Clause skips | `may skip` was already added (MenuEmptyReject). Added: replay compares admission results (SPEC §10.4) |
| M07 | Derived relations change when maintained values become visible | SPEC §9.7 and D-28: import with maintenance handlers in order, then refactor under replay at transaction boundaries |
| M08 | Deletion and empty creation were underspecified | As LD-07. SPEC §4.1: `create` sets every non-`maybe` field, and the importer's rule for Clause `one` fields |
| M09 | F64 to Int and the FIFO rewrite had no preservation contract | SPEC §10.4: the 2^53 integer check, and FIFO rules imported as written |
| M10 | An effect list could admit what the mailbox rejected | SPEC §5.4: `at most one per step` |
| M11 | No path for existing north checkpoints, the allocator or pending effects | SPEC §5.4 and §10.4: `settled by`, pending effects, checkpoint contents, `galileo import clause-checkpoint`; PLAN 8.11 |
| M12 | Comparing `Menu` views alone misses admissions, effects and restarts | SPEC §10.4: the observation contract, the entity correspondence and coverage; PLAN 8.5, 8.7 |
| M13 | The drvPath cannot certify modules no host enables | SPEC §10.2 and A3: per-module parse identity, listed with drvPath coverage; PLAN 4.7, 4.13 |
| M14 | The baseline was mutable, and the 63-to-50 mapping was implicit | PLAN: pinned worktrees (D-32), and the manifest box 1.5; Appendix A re-checked at `10740a8a` |
| M15 | `fmt`, the schema and Beagle's tools had no boxes | SPEC §2 and §9.4: `foreign`; PLAN 2.13–2.15, 4.9, 4.16; D-27 |
| M16 | Packages missed dependency edges | PLAN: a Needs column on every box, checked for cycles |
| M17 | Several Done whens did not measure their claim | PLAN: the firefox review replaces the no-op mako change; contracts in 6.5; replay, not handler counts, in 8.7; named rejection scenarios in 7.2; latency in 6.7 and 8.9; A9 names `why --all` |
| M18 | Boxes averaged 25.8 minutes | PLAN: 92 boxes of 20 minutes or less (19.5 on average), plus the north slices that 8.12 sizes |

### 4.4 Revisions made before the reviews returned

The lead's own pass, while the reviews ran:

- **S1.** The step's conflict rule became Dedalus's withdraw-before-include,
  with conflicts found by modes. The old rule rejected `withdraw X` with
  `include X` and a `set` to the current value.
- **S2.** `exactly one` takes a single entity key.
- **S3.** Every claim is evaluated over the transition system; a program
  without events has one state.
- **S4.** `requires` and `among` quantify their own variables, and vacuous
  claims are marked.
- **S5.** Argmin syntax, with a canonical tie-break.
- **S6.** `galileo build` refuses to emit while a derived mode Fails.
- **S7.** Opaque `nix T` payloads (with NixInFacts).
- **S8.** The tags vocabulary was completed: `supports`, `sets … to … for`,
  `gets … set to … by`.
- **S9.** The claim grammar.
- **S10.** `the ?x where …` in views.
- **S11.** The text built-ins.
- **S12.** `--allow-requirement-changes` for derived patches.
- **S13.** Entity field access, `?x.field`.
- **S14.** The canonical value order.
- **S15.** The slot-token rule.

Also: empty `min` and `max` bind nothing; `delete` and `create` of one
entity in one step is a static error; two tag overrides of one path refuse
to emit; the `why` example uses firefox, because kanata is set in its host
module rather than enabled by a tag; and the drvPath gate covers whiterabbit
only, as nixos-config's AGENTS.md requires.
