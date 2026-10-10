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
