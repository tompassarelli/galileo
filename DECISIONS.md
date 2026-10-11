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

## 2. Decisions Tom made

The orchestrator relayed Tom's choices on 2026-10-10. On T1 to T3 he took
the recommendation; T4 and T5 are his own direction.

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
