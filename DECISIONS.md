# Muove decisions

This file records the decisions made for the MVP, the four that Tom made,
and how each review finding was resolved.

## 1. Decisions made

Each decision gives its evidence and, under "Rejected", the alternatives
considered. Evidence IDs refer to SPEC.md Appendix A, and `A-` IDs to its
Appendix B.

### D-1. A new Rust workspace in `~/code/muove/main`

Muove is built fresh, porting the prototype pieces listed in P6.

Rejected:

- **Grow Clause.** It is about 120k Rust LOC, stalled after 29 repins in 5
  days, and carries five P0 audit findings (N8).
- **Grow Beagle.** Racket costs 1.8 s per file to start, it has 17 recorded
  silent miscompiles, and its surface has been migrated repeatedly (F11,
  F13).
- **Grow the prototype in place.** It is positive-only Datalog with a naive
  engine, three critical review bugs and no value language (P2, P4, P5).

### D-2. Scope: a general-purpose language, proven first on firn and north

Muove is meant to be a general-purpose language (T4). firn and north are its
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
  it. It would leave every program's meaning split between Muove and a host
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

### D-7. Two stages; Muove never evaluates Nix

Muove values are either known at Muove compile time or `Nix` terms, which are
type-checked and emitted unevaluated (SPEC §3.2).

Rejected:

- **Embedding a Nix evaluator** such as snix or tvix: very large, and it
  would make Muove answerable for Nix semantics.
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

A handler opts into the empty step with `may skip`. `muove import clause`
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
- **Effectful Muove code:** it would break replay.

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

`rnix` parses Nix in dev tools and tests, and `lsp-server`/`lsp-types` serve
the LSP. The core crates stay dependency-free.

Rejected:

- **Zero dependencies everywhere:** that means writing a Nix parser.

### D-21. The first rewrite is nixos-config `modules/kanata`

The pair is `default.bnix` (28 LOC) and `kanata.bnix` (29 LOC), and Beagle
reproduces both byte-identically today (F14). Runners-up: `modules/users`
and `modules/firefox`.

### D-22. The first north slice is the menu, `north.clause` L447–827

Fallback: the references slice (N10).

### D-23. File extension `.muove`

Rejected:

- **`.mv`:** the Move language already uses it.

### D-24. Container layout

`main/inputs/` holds the imported inputs as source next to the original zips.
The prototype's two-commit history stays inside `claude-muove.zip`.

Rejected:

- **Nesting the prototype's `.git`:** it would create an embedded repository.

### D-25. `nix file` is the general output form; `nix module` is its special case

The flake root, `providerAdapter` and overlays are Nix values, not module
items (SPEC §10.2).

Rejected:

- **Keeping `flake.nix` outside Muove:** north's `flake.clause` and
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

`muove import nix` replaces `beagle-import-nix`, `muove rename option`
replaces `beagle-rename`, and `muove refs`, `muove build --stdout`,
`muove review` and `muove schema explain` replace the compiler queries.
The evidence-ranked repair pipeline is dropped. PLAN box 4.9 checks for
callers before Beagle leaves the build.

Rejected:

- **Porting the repair pipeline:** ranking repairs for a failed compile is
  what Muove's did-you-mean, closest-shape diagnostics and the review gate
  already do, and no caller is known.

### D-28. North migrates mechanically first, then refactors under replay

The importer keeps Clause's behaviour: every handler `may skip`,
maintenance handlers kept in order, FIFO rules as written, and the mailbox
as `at most one per step`. Each move to SPEC §9.7's end state is a separate
change that must replay identically.

Rejected:

- **Importing straight to the end state:** maintenance handlers run at
  different points from derived relations, so a sequence can read a stale
  value in Clause and a current one in Muove, and an empty move rejects
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
  `muove check` and `muove build` diagnostics with file:line (box 2.2).
  None replaces `beagle-provides`: a `.muove` file exports every
  declaration not marked `private` (SPEC §2), and `muove refs` finds its
  users. Their lines in `docs/compiler-queries.md` and
  `docs/fixtures/tags-example.bnix` go in box 4.22.
- **G2.** `beagle-daemon` is dropped, and Muove gets no query cache:
  `muove lsp` holds the memo (SPEC §11.2), and a full build is budgeted at
  2 s (§10.5). The PostToolUse hook its doc names does not exist in the
  pin. The doc lines go in 4.22.
- **G3.** `clause-workbench`, `FIRN_CLAUSE_WORKBENCH` and
  `config/clause-revision` are deleted with every caller in box 4.19: F1,
  F4, F9's Clause branch, F14 to F16, F19, and `firn-runtime-update`'s
  other Clause lines (7, 37–43, 256, 291–325, 542–546). firn gets `muove`
  the way it got Clause. `config/muove-revision` names a muove commit,
  `firn-runtime-update` builds it from `~/code/muove/main` (or
  `FIRN_MUOVE_REPO`) into the runtime's `bin/muove`, and the launchers
  refuse a checkout that declares another revision. There is no override
  variable; a test builds a scratch runtime. SPEC §11.3 records this.
- **G4.** `prewarm.clause` and `firn-host-list.clause` go to box 4.12. It
  also deletes F3's compile-js branch, F11's workbench lines, the committed
  `native/firn-host-list.{js,d.ts}`, `native/firn-host-list-consumer.ts`,
  `native/firn-host-list-clause.test.sh` and `FIRN_CLAUSE_HOST_LIST_MODULE`
  (`dotfiles/bin/firn:32`, `native/firn_io.mjs:3`). `machine-capacity.clause`
  moves to TypeScript in the skill's own host (box 4.18), not to Muove: it
  computes in F64 (D-12), it is neither firn nor north (SPEC §1.3,
  guardrail G1), and its bundle runs in spawn hooks that must not depend
  on a second binary. Its fixture assertions and
  `charged-resources.property.ts` are the oracle.
- **G5.** After box 4.8 the tags are facts. firn's tag loader reads them
  with one `muove query --json`, and box 4.20 deletes the Clause bridge
  (`native/clause_source_host.mjs`, its four importers, F5's staging,
  `load-clause-tags`), the `.bnix` tag parser `load-beagle-module`,
  `native/module_metadata.clause`, and `native/nix/` with its README and
  two tests.
- **G6.** `muove check` replaces `check-source`, and SPEC §11.1's row says
  so. F13 goes with `native/nix/` (4.20), F18 with 4.18, and F17 with the
  skill (G7).
- **G7.** `muove help [TOPIC]` replaces `authoring-card`: the commands,
  plus a language card with one example per SPEC §9.4 production, each
  compiled by the tests (a new §11.1 row, box 2.16). AI writes most Muove,
  and Beagle's 70 recorded hallucinations (F13) show what remembered
  syntax costs. `nixos-config:AGENTS.md:6-7` and its Checks name `.muove`,
  `firn repo build` and the runtime's `muove help`, `muove refs` and
  `muove schema explain` (4.22). The clause-authoring skill stays while
  north writes Clause, and box 8.15 deletes it.
- **Outside 4.9's list, named there.** `muove schema extract` (§11.1, box
  2.14) replaces `beagle-extract-schema`. `muove check`, with 2.15's schema
  checks, and `muove build --check` replace `beagle-validate` and
  `beagle validate` (§11.3). `firn repo validate`, `repo doctor`,
  `repo watch` and `repo upgrade` run those, and box 4.21 deletes firn's
  schema executable (`native/schema_transaction*.bjs` with its host, tests
  and runtime component). `beagle doctor` is dropped, because G3's
  revision check catches toolchain skew. Box 4.22 deletes `modules/beagle`
  and its ten wrappers with their callers: whiterabbit's `-beagle` tag
  edit and the `beagle` entry in `config/darwin-modules.json`. firn runs
  Beagle through `BEAGLE_PATH` directly, whiterabbit already excludes the
  module, and `muove lsp` serves `.muove`.
- **Outside 4.9's list, found here.** 21 files under `native/` read or
  write `.bnix`, `enabled-tags.bnix`, `configuration.bnix`, `flake.bnix`
  or Clause source: tags, inventory, authoring templates, flake inputs,
  views, repo quality and the system-policy text. So do the checkout tests
  at `dotfiles/bin/firn:7` and `native/prewarm_host.mjs:63`. They fail
  silently once `.bnix` goes, because `load-beagle-module` returns an empty
  module when its file is missing. Readers take facts through
  `muove query --json`, writers write `.muove`, and a command whose work
  Muove now does runs that `muove` command. No `.bnix` parser survives.
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
  maps `beagle-provides` to `muove refs` and `beagle-extract-schema` to
  `muove check`, and it gives firn no pinned `muove`.
- **Muove subcommands for syntax, provides, daemon, doctor and
  check-source:** `check`, `refs`, `lsp` and the revision check already
  meet each need.
- **machine-capacity in Muove:** it needs F64, it is not a target, and a
  launch-critical hook would depend on a second binary.
- **A Muove authoring skill:** feature detail goes in CLI help.
- **Running `muove` live from `~/code/muove/main`, as firn runs Beagle:**
  a muove change would move nixos-config's drift check (D-32's reason).
- **`muove` as a NixOS package:** fast-changing projects stay outside the
  system closure.
- **Renaming `.beagle-cache` now:** about 50 references change, and no
  check needs it; the rename goes with Beagle.

### D-34. The boot-coupling budget becomes Muove requirements, and backticks quote a name anywhere

Line numbers are at `FIRN_ROOT`.

- **Box 4.33: Muove owns the budget's verdict.** firn's boot-coupling
  budget (`native/responsibility_projection.bjs:416–433`) is decision logic
  over responsibility facts, and D-2 and T1 put decision logic in Muove
  rules. Box 5.7 writes its five parts as requirements beside 5.4's
  realization requirement: no module of environment-domain software is
  enabled on the host; no boot-domain responsibility reaches an
  environment-domain one through `depends on`; every required software has
  a module, a domain, a criticality and, in the boot domain, a lifecycle;
  every required capability is provided; and every boot-domain software is
  a long-lived system service whose module is enabled. firn then reads no
  responsibility facts. The calculation's only caller is the test that
  `native/repo_build_native.test.sh` runs, so box 4.33 makes that step run
  `muove check` and deletes the calculation and its test without porting
  them.
- **The oracle fails at the pin.** whiterabbit enables `obsidian` and
  `blender` through its `notes` and `creative` tags (nixos-config
  `61d004a6` and `2e0393a8`), while
  `hosts/whiterabbit/responsibilities.bnix:43–44` puts their workflows in
  the environment domain. No check noticed: CI has not passed since
  2026-08-16, before the budget landed (`ea236556`), and the live check
  loads module tags through the Clause workbench
  (`native/tag_resolve_family.bjs:171`), which CI does not set up. Until
  Tom changes the enables or the responsibilities, `muove check`, and with
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

- **Feeding the existing calculation through `muove query --json` (the
  4.17 census):** the logic stays in `.bjs` for T1 to port to TypeScript,
  realization is checked twice (5.4 and `incomplete-software`), and a
  failure gets no `why not`, review label or LSP diagnostic. The unseen
  failure above is what a gate outside Muove costs.
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

- **Nix's `{ a = 1; }`:** Muove writes `name: value` everywhere else, and
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

### D-36. One owner for the language, the next win before breadth, and Tom writes Muove

Tom's assistant raised four concerns about how Muove is being built,
measured on 2026-10-10 at 20:00 (29 of 111 boxes, about 12,800 lines of
Rust, 61 DECISION lines). `notes/language.md` holds the working detail.

- **Coherence: accepted, with the owner's memory in the repository.**
  Fourteen Opus max planners have taken Muove questions since 18:29, up to
  nine at once, and three of them held questions from box 2.11. The five
  rulings of 19:05 (`notes/language.md` L1–L5) reached worker briefs but
  never SPEC or DECISIONS, and two boxes then built their opposite: 3.4
  sorts by a body-local variable, and 2.11 accepted a declared variable
  that clashes with an outside one (fixed in `afd89d1`). Sequential `let`
  was asked again for 2.4 after L2 ruled it, and `?x is a T` went to two
  planners. The first pass over every ruling found eleven contradictions
  (K1–K11), one already fixed. So one Opus max planner, the language
  owner, now takes every question about what a Muove program means and
  how it is spelled. The lead keeps it between batches with SendMessage
  and restarts it from the notebook, the only memory that counts. A ruling
  binds once SPEC states it, and lands
  with one DECISIONS entry per batch and the notebook's index lines.
  Workers keep writing `DECISION` lines, take the narrowest reading SPEC
  allows, and land; a box stays unticked only when its Done when depends
  on the answer. Other questions still go to one-shot planners.
- **The sentence register: accepted, and the measured risk is Muove's own
  keywords, not English.** Declared templates already rule out reading
  English (D-10). At `f2802f2`, `backup requires capability gpu` reads as a
  fact at the top level but fails inside a `requirements` block, because
  `requires` is also a claim keyword, and firn's own vocabulary uses it
  (`lib/firn/responsibilities.muove`). A claim that writes `?h is a Hero`
  makes every claim in its file `Unknown`, and 2.10 lets a prepended `use`
  re-point an existing line (G5). Since 5.4, `muove check` reads source, so
  Tom writes a small sentence-register program now (exercise B), and the
  owner settles keyword collisions with one rule (notebook O1).
- **End to end before breadth: accepted as a standing order, and its first
  case is done.** A1 landed (2.6 `fc36795` and 2.7 `bb8a95f`; both kanata
  files byte-identical, 989 and 1,019 bytes), and A5 passes through
  `muove check` on whiterabbit's real responsibility facts (5.4,
  `f2802f2`). No command yet explains. So the next win is A4 from source:
  `muove build` emits both hosts' `_generated-enables.nix` byte-identically,
  `muove why` prints the firefox proof, and `muove check` runs
  `requirements for tags`. Box 5.1 now needs the reference interpreter
  (3.4) rather than the engine (3.6), which waits on a ruling, and 5.3 owns
  the `muove why` command. When capacity limits staffing, the next win's
  boxes go first. New box 2.17 lowers `nix file`, which 4.5 and 8.10 need
  and no box owned.
- **Tom's taste: accepted, now, and before firn's cutover.** Today's
  toolchain builds a real firn module: a version of `modules/update-notify`
  written to check the exercise builds byte-identically beside a four-line
  users stub. Tom writes that module himself (exercise A) and the small
  sentence-register program (exercise B), about 15 minutes each, and the
  lead records his notes in the notebook. Box 4.14, which makes `.muove`
  firn's source, waits for both (PLAN T.1 and T.2).

Rejected:

- **One-shot planners per batch, as before:** five rulings never reached
  SPEC, two boxes built their opposite, and questions were asked twice.
- **One-shot planners that each read and update the notebook:** that fixes
  persistence, but planners running at once in one area still rule
  without seeing each other.
- **A resident owner whose context is its only memory:** a context fills
  and a restart loses it, and the lead's planners leave no transcript on
  disk.
- **Tom as the language owner:** his time goes to the two exercises, where
  no agent can stand in for him.
- **The owner also ruling on Nix layout, migration and crate boundaries:**
  WP1 and WP4 would queue behind questions that do not change what a
  program means.
- **Holding every other box until A4 lands:** its path is at most two
  boxes wide, so workers would idle.
- **A dungeon tweak for exercise A:** the dungeon's events do not run until
  WP7.
- **Gating every box on Tom's exercises:** only 4.14 is hard to undo.

### D-37. Names follow one option tree, a sequential `let` and a target-free prelude; a work unit is one step

These rule on the open questions of boxes 2.4 and 3.3. The name probes ran
on main `35c9f83`, and again on `62a8257` after 2.15, against scratch copies
of `tests/external/kanata`, and nixos-config line numbers are at
`FIRN_ROOT`.

- **Box 2.4: one option tree, checked only where it is complete.** `config`
  in a `nix module` has its module system's option tree: the options that
  Muove sources declare, together with the declared foreign schema. Where
  both declare a path, Muove's declaration wins, because Muove emits it. A
  name is an error only under a closed node. With a schema, every node down
  to a declared option is closed. Without one, Muove sees only its own
  declarations, so a node is closed only when it directly holds a Muove
  option, such as `myConfig.modules.kanata`. A node that also holds foreign
  options, such as `services.foo` once Muove adds `services.foo.extra`,
  needs the schema. firn never meets that case: all 61 option declarations
  in its `.bnix` sources sit under `myConfig`. Past a declared option, the
  rest of a path is a key into its value, such as `main` in
  `services.kanata.keyboards.main`. A nested `nix module (…)` has its own
  module system, and Muove's declarations do not apply to its `config`.
  The 2.4 checker closes every node above a Muove option, the root
  included, so `config.services.kanata.enable` is an error today ("`services`
  isn't an option of `config`"), not a pass, and so is
  `config.home.username` in a nested home-manager module. The Beagle
  sources read foreign options 36 times (`config.home` 16, `config.lib` 17,
  `config.hardware`, `config.sops`, `config.stylix`), and 18 of the 63
  nest a module, so box 4.2 would fail on them. Box 2.15 (`62a8257`, which
  landed during this ruling) joins the schema with Muove's declarations,
  Muove's winning, but walks only entries, and only when a schema is
  declared. Box 2.4b makes reads walk the same tree and gives the tree its
  rule without a schema.
- **Box 2.4: entry paths are option paths, checked now.** An entry under
  `config` or `config when`, or in a module-level `for` block, walks the
  same tree as a `config.` read, so `myConfig.modules.kanata.enabel: true`
  will name `enable` and `default.muove:3`. Since 2.15 it does when a
  schema is declared, and without one it passes. A computed segment `{…}`
  ends the walk, and the entries of a `nix file` value are attribute keys,
  not option paths. Box 2.4b checks entries against Muove's declarations
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
- **Box 2.4: the prelude holds Muove's functions, and the Nix target binds
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
  `map` is Muove's and Nix's is `builtins.map`, as the Beagle sources write
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

None of this needs breaking for a general-purpose Muove (T4). A foreign
namespace stays open until a declaration closes it, which a later target's
foreign sources follow too. The scope rules carry over to definition bodies
unchanged. Target names stay out of the prelude. A budget counts the same
kinds of step whatever runs them, and no value outgrows it.

Rejected:

- **Closing every node above a Muove option (the 2.4 checker):** it
  rejects every foreign read, such as `config.services.kanata.enable`, and
  every read in a nested home-manager module.
- **Closing those nodes except the root:** it still catches
  `config.myConfig.modulez`, but it rejects every foreign option in a
  namespace that holds a Muove one, such as `config.services.kanata.enable`
  once Muove declares `services.foo.extra`, the case 2.4 asked about. The
  schema catches that typo instead.
- **Muove's declarations replacing the schema under any node they reach:**
  the same rejection, with a schema present.
- **Checking entry paths only when a schema is declared, as 2.15 does:** a
  typo under `myConfig` is then emitted silently by every file without a
  schema, though Muove's own declarations catch it.
- **A recursive `let`, as in Nix:** it would bring back the recursion SPEC
  §3.5 rules out, Beagle's lambda layout cannot print it, and no firn file
  needs it.
- **Rebinding a name in its own block, as Rust allows:** one `let … in`
  cannot hold it. Allowing it later breaks no program; forbidding it later
  would.
- **Nix's globals in the prelude:** every target's names would reach every
  program, and Nix's `map` would collide with Muove's.
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

## 2. Decisions Tom made

The orchestrator relayed Tom's choices on 2026-10-10. On T1 to T3 he took
the recommendation; T4 is his own direction.

### T1. firn's mechanics move to TypeScript on Bun

firn's 59 `.bjs` files hold 23,268 LOC (F12). When Beagle's JavaScript
retires, they become TypeScript on Bun and call `muove` through its CLI and
JSON output. Decision logic moves into Muove rules either way (D-2).

Rejected:

- **Rust with the Muove crates embedded:** firn already runs on Bun
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
  Clause 29 times in 5 days (N8), and Muove takes over its role.
- **Retiring it only after both migrations, with no freeze:** new Clause
  features would widen what Muove must import.

### T4. Muove aims to be a general-purpose language

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
(SPEC §12.2), such as I/O inside Muove code, evaluating Nix and free
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
- **Determinism.** Muove code has no clock and no randomness (SPEC §5.3).
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

### D-38. The Beagle layout's parentheses are printer rules over a semantic Nix AST

muove-nix has no parenthesis node and no layout metadata. The `beagle`
layout prints every binary operation inside one pair of parentheses, and
parenthesizes a selection whose base is a selection, as Beagle's emitter
does (`emit-nix.bclj:1390–1398`, `:2165–2172`). With these two rules the
printer round-trips 49 of the 50 Beagle-generated files byte for byte.
`flake.nix` is parse-identical: its two one-operand
`(++ (builtins.map …))` calls (`flake.bnix:136`, `:304`) are Beagle
no-ops, and neither Nix nor Muove can express their extra parentheses.
So A2 counts 49/50 plus `flake.nix` parse-identical, which box 4.5
already allows. This refines D-8 for `flake.nix`.

Rejected:

- **Keeping rnix's parentheses in the AST:** the round trip would pass
  with no parenthesis rules at all, and Muove's lowering, which the
  migration depends on, could never supply them.
- **A rule for `builtins.map` with a lambda argument:** it fits the corpus
  by accident (`modules/beagle/default.nix:8` has none).
- **Removing the no-op from firn's source and re-pinning:** it changes the
  oracle to pass the check (D-32), and `flake.muove` drops it anyway.

## 3. Process notes

- **Where the inputs went.** They were moved from the container root into
  `main/` through an orphan lane. Both archives were extracted into fresh
  scratch directories, read, scanned for secrets, and imported as source
  (27 of 27 and 6 of 6 files).
- **What each reviewer saw.** Only `SPEC.md`, `PLAN.md` (the
  migration-engineer lens) and Muove's own `inputs/`. No secrets,
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
| LD-06 | Views and handlers queried `M(static)` | Views already read `M(S)`. Added: SPEC §4.5 gives every query context its model, and "known" means evaluated by Muove (§3.2) |
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
| LD-18 | The Dedalus correspondence ignored rejection | SPEC §5.3: it covers the post-state only; admission is Muove's own |

The designer also confirmed the dungeon by its own search over 19 states:
the room set, the Library checkpoint, the warning requirement, the Iron-Key
failure, the ladder repair and the four Silver-Key placements all agree.
Its shortest witness takes 5 steps, not the 6 the SPEC listed, so §5.6 now
gives the 5-step witness, which the ChatGPT oracle's room path confirms.

### 4.3 Astra, migration engineer

| ID | Finding | Change |
| --- | --- | --- |
| M01 | firn's three Clause programs, north's `flake.clause`, the rest of north and Clause's removal had no boxes; A8 meant two things | PLAN 4.10–4.12, 8.10, 8.12–8.15; SPEC §13: A8 is the menu slice and A11 is north replaced; §10.4: `muove run` and `muove query` for firn's TypeScript host |
| M02 | The Nix front-end covered only kanata's subset | SPEC §10.2: `nix file`, `with`, `//`, nested modules; PLAN 2.2, 2.3, 4.2 |
| M03 | Attribute names and attribute paths were conflated | SPEC §3.1: `OptionPath`, and the attribute-name rule for text and entities |
| M04 | `contains` and `lowercase` had no signatures, and Clause parity was unchecked | SPEC §3.5: signatures, with Clause as the reference; PLAN 8.4, 8.6 |
| M05 | The menu view read the static model | As LD-06 |
| M06 | An empty move rejected where Clause skips | `may skip` was already added (MenuEmptyReject). Added: replay compares admission results (SPEC §10.4) |
| M07 | Derived relations change when maintained values become visible | SPEC §9.7 and D-28: import with maintenance handlers in order, then refactor under replay at transaction boundaries |
| M08 | Deletion and empty creation were underspecified | As LD-07. SPEC §4.1: `create` sets every non-`maybe` field, and the importer's rule for Clause `one` fields |
| M09 | F64 to Int and the FIFO rewrite had no preservation contract | SPEC §10.4: the 2^53 integer check, and FIFO rules imported as written |
| M10 | An effect list could admit what the mailbox rejected | SPEC §5.4: `at most one per step` |
| M11 | No path for existing north checkpoints, the allocator or pending effects | SPEC §5.4 and §10.4: `settled by`, pending effects, checkpoint contents, `muove import clause-checkpoint`; PLAN 8.11 |
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
- **S6.** `muove build` refuses to emit while a derived mode Fails.
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
