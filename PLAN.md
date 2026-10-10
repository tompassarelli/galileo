# Muove implementation plan

This plan builds the MVP in SPEC.md. It is ordered toward the first win:
nixos-config's `modules/kanata`, rewritten in Muove, compiling to byte-identical
Nix. The rest of firn follows, then north. Decisions and review findings are in
DECISIONS.md.

## Start state

Checked 2026-10-10 18:27:

- **muove** `main` is `6df2bc26`: the imported inputs, with this plan landing
  on top. There is no remote and no CI.
- **nixos-config**: the pin is `10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9`.
  `main` has since moved to `b261e5bc`, with no `.bnix`, `.clause` or `.nix`
  changes in between.
- **north** `main` is `eaee1838d1968b7caba65195866e5d52fd30b2e1`.

**Before wave 1** (the lead, 5 minutes): pin both fixtures, so that no check
loses its oracle while the migration lands (repo-safety pins).

```bash
git -C ~/code/nixos-config/main worktree add --detach \
  ~/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9 \
  10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9
git -C ~/code/north/main worktree add --detach \
  ~/code/north/pins/eaee1838d1968b7caba65195866e5d52fd30b2e1 \
  eaee1838d1968b7caba65195866e5d52fd30b2e1
```

Each pin gets a `.pin` sidecar with `consumer-main: ~/code/muove/main`.
`FIRN_ROOT` and `NORTH_ROOT` name these pins in every brief.

## Staffing rules

- **Boxes.** Every box is 20 minutes or less. A box is ready once every box
  in its **Needs** column has landed. It inherits its package's facets and
  files unless its row says otherwise. *mechanical* marks a box that may go
  to `worker-haiku` with `Arm: haiku-trial`.
- **Lanes.** Each package names the lanes that can run at the same time. One
  worker takes one lane, box by box.
- **Local workers only.** Muove has no remote, so cloud workers cannot take
  it. Take a capacity lease for builds. On 2026-10-10 the spawn hook refused
  two workers at 24–32% desktop pressure.
- **Tiers.** Start every box at `worker` (Opus medium). Codex SOL high is
  equally fine for the Rust boxes.
- **Briefs.** Each brief gives the box's Done when verbatim, the package
  facets, the start state and the pins.
- **Landing.** Each lane works in `~/code/muove/worktrees/<lane>` and lands
  a box with `git -C ~/code/muove/main merge --ff-only <branch>` once its
  Done when passes. There is no `safe-push`, because there is no remote.
  nixos-config and north are tooling-profile repositories: changes to them
  run their named check and land through their own worktrees and
  `safe-push`.
- **Fixtures stay where they are.** Tests read them through `FIRN_ROOT` and
  `NORTH_ROOT` and never copy them into this repository (DECISIONS D-19).
  A nixos-config or north lane compares against its own merge base, so other
  work landing on their `main` does not move the oracle.
- **Tests.** Every kept test is one of the five kinds in the testing skill:
  - the external references are the kanata and other Beagle `.nix` bytes,
    `_generated-enables`, the dungeon state-search results, the ChatGPT
    regression scenarios, recorded Clause sequences and Clause's text
    functions;
  - the properties are printer round-trips, `fmt` idempotence and
    reference-against-engine agreement;
  - the recorded scenarios are replay logs;
  - the integration checks are `nix-instantiate --parse`, the drvPath eval,
    the JSON CLI and the north embedding.

  Delete scaffolding tests before landing. Cost ceilings are counted in engine
  work units. Wall time appears only in the one-off measurements named in a
  Done when.

## Workspace layout (created by box 1.1)

```text
Cargo.toml                 workspace
flake.nix                  devshell: rustc, cargo, gcc, nix tools
crates/muove-syntax        lexer, blocks, parser, AST, formatter
crates/muove-check         names, vocabularies, types, stages, effects, diagnostics
crates/muove-core          Core IR, reference interpreter, Verdict, Budget
crates/muove-engine        semi-naive evaluation, provenance, explain, state search
crates/muove-review        baseline freeze, patches, review views and verdicts
crates/muove-nix           Nix AST and printer (beagle, idiomatic layouts)
crates/muove-import        bnix, clause, nix and clause-checkpoint importers
crates/muove-runtime       sessions, steps, checkpoints, views, JSON CLI
crates/muove-bindgen       Rust bindings generator
crates/muove-lsp           language server
crates/muove-cli           the `muove` binary
lib/firn/                  tags, responsibilities, flake, dispatch, nix-lib (.muove)
examples/dungeon/          dungeon.muove and its five patches
tests/                     scenarios/, properties/, external/ (fixtures read in place)
tools/nix-roundtrip        dev only: parse .nix with rnix, print with muove-nix
tools/firn-manifest        dev only: source-to-output manifest of FIRN_ROOT
```

## Order and parallelism

**Ready at the start** (after the pins): 1.1, 1.5, 4.9, 4.10, 8.4 and 8.5.
Once 1.1 lands, 2.1 and 3.1 are ready too, so seven workers can start within
20 minutes.

```text
wave 1:  WP1 printer   WP2 record register (first win)   WP2 sentence register
         WP3 engine    inventories: 1.5, 4.9, 4.10, 8.4, 8.5
wave 2:  WP4 firn modules   WP5 firn rules   WP7 state and events   WP3 explanation
wave 3:  WP6 review and LSP   WP8 north
```

**Critical path to the first win** (about 140 minutes):
1.1 → 2.1 → 2.2 → 2.3 → 2.4 → 2.5 → 2.6, while 1.2–1.4 run beside it.

With a worker free for every ready box, the Needs graph gives these
earliest landings: the first win at about 140 minutes, A6 and A7 at about
240, firn's integration (4.14) at about 255, and the menu slice (8.9, A8) at
about 280. A11 then waits on the slice boxes that 8.12 sizes.

**Wins, in order:**

1. **First win:** box 2.6 (acceptance A1).
2. **firn replaced:** WP4 and WP5 (A2–A5).
3. **Change review:** WP6 (A6).
4. **The thesis demo:** WP7 (A7), with WP3's A9 and A10.
5. **North's menu:** WP8 to box 8.9 (A8).
6. **North replaced:** the rest of WP8 (A11).

**Totals.** 110 boxes and about 36 worker-hours, plus about 30 north-slice
boxes that box 8.12 sizes. The average box is 19.5 minutes.

| Package | Boxes | Minutes | Lanes |
| --- | --- | --- | --- |
| WP1 | 7 | 130 | 1 |
| WP2 | 16 | 315 | 2 |
| WP3 | 15 | 300 | 3 |
| WP4 | 32 | 620 | 4 |
| WP5 | 7 | 140 | 1 |
| WP6 | 10 | 200 | 2 |
| WP7 | 9 | 175 | 2 |
| WP8 | 14, plus slices | 265, plus slices | 3 |

## WP1. Nix AST and the Beagle-compatible printer

- **Goal:** a Nix AST and a printer that reproduces Beagle's output byte for
  byte.
- **First rewrite target:** nixos-config `modules/kanata`, from
  `default.bnix` (28 LOC) and `kanata.bnix` (29 LOC). WP1 prints its two
  `.nix` files (boxes 1.1–1.4), and WP2 compiles them from Muove (box 2.6).
- **Facets:** Category feature. Spec: exact. Scope: module. Surface: rust and
  nix. Verify: local-test.
- **Files:** `Cargo.toml`, `flake.nix`, `crates/muove-nix/**`,
  `tools/nix-roundtrip/**`, `tools/firn-manifest/**`.
- **Lane:** one, box by box; 1.5 can go to a second worker at once.
- **Done when:**
  1. The printer round-trips all 50 Beagle-generated `.nix` under
     `$FIRN_ROOT` byte-identically (50/50; acceptance A2).
  2. The manifest accounts for every `.bnix`, Clause module and tag file in
     Appendix A's counts (63, 185, 184).

| Box | Work | Needs | Min | Done when |
| --- | --- | --- | --- | --- |
| 1.1 [x] | Workspace, devshell and Nix AST types: attrsets with attrpath keys, lists, strings with parts, indented strings, paths, literals, select, apply, lambdas with formals and `...`, let, if, with, binops, not. A plain printer | — | 20 | `nix develop -c cargo test -p muove-nix` builds the AST of `$FIRN_ROOT/modules/kanata/default.nix` by hand, and the printed text passes `nix-instantiate --parse` |
| 1.2 [x] | Beagle layout, part 1: attrsets, lists, strings, calls, imports | 1.1 | 15 | The printed text for that AST matches `$FIRN_ROOT/modules/kanata/default.nix` (`cmp` exits 0) |
| 1.3 [x] | `tools/nix-roundtrip`: rnix concrete syntax to the muove-nix AST, for every node kind the two kanata files use | 1.1 | 20 | `nix-roundtrip --ast` prints both kanata files' ASTs with no unsupported-node error |
| 1.4 [x] | Beagle layout, part 2: let-as-lambda, `mkIf` chains, `''` strings | 1.2, 1.3 | 20 | `nix-roundtrip $FIRN_ROOT/modules/kanata/kanata.nix \| cmp - $FIRN_ROOT/modules/kanata/kanata.nix` exits 0 |
| 1.5 [x] | `tools/firn-manifest`: every `.bnix` with its emitted file or the reason it emits none, every `native/nix/*.clause` with its output, every `tags.clause` with its module *(mechanical)* | — | 15 | Prints 63 `.bnix` rows (50 with an output), 185 module rows and 184 tag rows for `$FIRN_ROOT` |
| 1.6 [x] | Round-trip every `.nix` that has a `.bnix` source, and fix the layout cases that fail *(mechanical)* | 1.4, 1.5 | 20 | At least 40/50 identical; each failure is listed with its first differing line |
| 1.7 | The remaining cases: `''${` escapes, long lists, nested modules, `with`, `//`, symbol keys, `flake.nix` | 1.6 | 20 | 50/50 identical (A2) |

## WP2. Front-end and the first win

- **Goal:** the parser, checker and lowering that make `modules/kanata`
  compile from Muove, then the sentence register, the formatter and the
  foreign option schema.
- **Facets:** Category feature. Spec: exact. Scope: cross-module. Surface:
  rust. Verify: local-test.
- **Files:** `crates/muove-syntax/**`, `crates/muove-check/**`,
  `crates/muove-cli/**`, `lib/firn/nix-lib.muove`,
  `tests/external/kanata/{default,kanata}.muove`.
- **First rewrite:** nixos-config `modules/kanata`. Its sources are
  `default.bnix` (28 LOC) and `kanata.bnix` (29 LOC), and they emit
  `modules/kanata/default.nix` and `modules/kanata/kanata.nix`. SPEC §9.5
  gives the target Muove text. Runners-up are `modules/users` and
  `modules/firefox`.
- **Lanes:** the record register (2.1–2.7, 2.14, 2.15) and the sentence
  register (2.8–2.13), which starts once 2.1 lands.
- **Done when:**
  1. `muove build tests/external/kanata` writes `default.nix` and
     `kanata.nix`, and `cmp` against `$FIRN_ROOT/modules/kanata/` exits 0 for
     both (acceptance A1).
  2. Changing `cfg.devices` to `cfg.devics` gives the did-you-mean error that
     cites `default.muove`'s declaring line.
  3. The dungeon, `lib/firn/tags.muove` and the menu slice lower to Core IR
     with no diagnostics.
  4. `muove fmt` is idempotent on every `.muove` in the repository.

| Box | Work | Needs | Min | Done when |
| --- | --- | --- | --- | --- |
| 2.1 [x] | Lexer and indentation blocks, ported from the prototype's `syntax::blocks`. Comments that respect strings and blocks. `'''` text blocks with dedent | 1.1 | 20 | Golden block tests pass for both kanata `.muove` files, and `says: "Fixes issue #12"` keeps its full text |
| 2.2 [x] | Record register, declarations: `nix module`, `nix file`, `arguments`, `option` with fields, `let`, `imports`, `config when`, entries, nested blocks, `when` suffixes, `for` blocks | 2.1 | 20 | Both kanata files parse. A malformed entry gives file:line and the expected form |
| 2.3 [x] | Record register, expressions: lists, records, calls, field access, operators including `//` and `++`, paths, text blocks with `{{…}}`, lambdas with formals, `with`, `if` | 2.2 | 20 | Every expression in both kanata files parses, and each form in SPEC §9.4's `expr` rule parses from a one-line example |
| 2.4 [x] | Checker, names: options typed as `config.myConfig.modules.kanata`, `let` scope, did-you-mean | 2.3 | 20 | kanata checks with no diagnostics. `cfg.devics` names `devices` and `default.muove`'s declaring line |
| 2.5 [x] | Checker, stages: the known/Nix rule, `lib/firn/nix-lib.muove` signatures for the `lib` and `builtins` calls kanata makes, `maybe` | 2.4 | 20 | A `Nix` value in a fact slot is a stage error. `cfg.port != null` checks and `cfg.port > 1` does not |
| 2.6 | Lowering to the muove-nix AST (option sugar, SPEC §10.2's type table, `when` to `lib.mkIf`, Beagle's let layout) and `muove build` | 2.5, 1.4 | 20 | **First win:** `cmp` exits 0 for both kanata files (A1) |
| 2.7 | `muove build --check`, `--stdout FILE`, and the `nix-instantiate --parse` gate | 2.6 | 15 | `--check` exits 0 on kanata, and exits 1 with a diff when a temporary copy of the expected `.nix` has one byte changed. `--stdout` prints the same bytes |
| 2.8 [x] | Sentence register, part 1: shapes with kinds, modes, domain and effect lines; facts, focus blocks, list slots; the slot-token rule. Port the prototype's `read_atom`, `splits` and closest-shape ranking | 2.1 | 20 | The prototype's three ambiguity and closest-shape messages reproduce. `?m is enabled on ?h by default through ?t` reads only as the longer template |
| 2.9 [x] | Sentence register, part 2: rules with every SPEC §4.3 literal, and queries | 2.8, 2.3 | 20 | `lib/firn/tags.muove` (SPEC §9.6) and the menu rules (§9.7) parse |
| 2.10 [x] | Sentence register, part 3: handlers, requirements and claims, questions, `use` scoping and `private` | 2.9 | 20 | The dungeon (§5.6) parses. Importing an unrelated module with the same template leaves an existing line's reading unchanged (R16) |
| 2.11 [x] | Lowering to Core IR: facts, rules, queries | 2.9, 3.1 | 20 | `lib/firn/tags.muove` lowers with no diagnostics, and hover data (the structured reading) exists for every sentence |
| 2.12 | Lowering to Core IR: events, handlers, effects, requirements, views | 2.10, 2.11 | 20 | The dungeon and the menu slice lower with no diagnostics |
| 2.13 | `muove fmt [--check]` for both registers | 2.10 | 20 | On every `.muove` in the repository, formatting twice equals formatting once and keeps the parse tree. `--check` exits 1 on a misindented copy |
| 2.14 | `muove schema extract`: run the `nix eval` expression firn's extractor runs today, and write the schema JSON | 2.7 | 20 | On `$FIRN_ROOT`, the output has the same option paths and types as the JSON firn's current extractor writes |
| 2.15 | `foreign nixos-options from …`, `muove schema explain PATH`, and checking option paths against the schema | 2.14 | 20 | `muove schema explain services.kanata.enable` prints its type and declaring module. `services.kanata.enabel` in a scratch copy gets a did-you-mean |
| 2.16 | `muove help [TOPIC]`: the commands, and the language card that replaces `clause-workbench authoring-card` (DECISIONS D-33) | 2.12, 2.13 | 20 | `muove help syntax` prints one example for each production of SPEC §9.4's grammar, and a test compiles every printed example with no diagnostics |

## WP3. Engine core: evaluation, verdicts, explanation

- **Goal:** the Core IR, the static checks, the reference interpreter, the
  fast engine, three-valued verdicts, lazy proofs and the static requirement
  forms.
- **Facets:** Category feature. Spec: exact. Scope: cross-module. Surface:
  rust. Verify: local-test.
- **Files:** `crates/muove-core/**`, `crates/muove-engine/**`,
  `tests/properties/**`, `tests/scenarios/**`.
- **Lanes:** after 3.3, the engine (3.5–3.8), explanation and strength
  (3.9, 3.10, 3.13–3.15) and claims (3.4, 3.11, 3.12).
- **Done when:**
  1. 10,000 generated programs show 0 reference-against-engine disagreements
     (A10).
  2. A9 holds as SPEC §13 words it.

| Box | Work | Needs | Min | Done when |
| --- | --- | --- | --- | --- |
| 3.1 [x] | Core IR types and Rust builders: relations with kinds, modes and slots; facts; rules with every SPEC §4.3 literal; queries; events, handlers, actions, effects and claims | 1.1 | 20 | dungeon-static, the tags program and the R13 program are built with the builders and print their relation and rule counts |
| 3.2 [x] | Static checks: the grounding schedule, stratification with queries counted as aggregates, and no value invention in recursion | 3.1 | 20 | Four scenarios give SPEC §4.3's messages: a membership that grows text, a query that hides a negative cycle, `?x = ?y` with `?y = ?x`, and a plain negation cycle |
| 3.3 | Reference interpreter: naive stratified evaluation with negation, comparisons, bindings and membership. `Verdict` and `Budget`, charged inside every loop before intermediates are built | 3.2 | 20 | The dungeon-static and tags models are computed. A budget of 1,000 work units on the tags program gives `Unknown(work)`, never a partial model |
| 3.4 [x] | Reference interpreter: aggregates over distinct bindings with exact sums, argmin with its tie-break, queries in rules | 3.3 | 20 | Six named cases give SPEC §4.3's results: empty `sum`, empty `count`, empty `min`, an argmin tie, an argmin with no candidate, and `max + 1 - 1` summed in both orders |
| 3.5 [x] | Semi-naive engine with per-relation indexes, positive rules and stratified negation | 3.3 | 20 | The dungeon-static and tags models equal the reference's |
| 3.6 | Engine: aggregates, argmin, queries and work-unit accounting | 3.4, 3.5 | 20 | The six cases of 3.4 match the reference. On P7's 101-room chain, the engine uses at least 10× fewer work units than the reference |
| 3.7 | Differential property, part 1: a generator of stratified programs with negation; a fixed seed locally | 3.5 | 20 | 10,000 programs, 0 disagreements. A seeded fault in the engine's negation makes the test fail |
| 3.8 | Differential property, part 2: add aggregates, argmin, bindings and grounded membership to the generator | 3.6, 3.7 | 20 | 10,000 programs, 0 disagreements (A10). A seeded aggregate fault makes the test fail |
| 3.9 | Lazy provenance in the engine (producing rule and height per tuple), `why` reconstruction under the "stated means stated" invariant, absence leaves for `not` | 3.5 | 20 | R10's scenario shows no invented leaf. A `not` proof shows its absence leaf and scope. On the R13 program, `why` returns one proof and `why --all` returns `Unknown(work)` in under 1 s (A9) |
| 3.10 | `why not`: near misses ported from `NearMiss`, plus `via <rule>` | 3.9 | 20 | `why not Ash holds Silver-Key` on dungeon-static prints the missing and blocked conditions the way the prototype's README example does |
| 3.11 [x] | Static claims: quantifiers, vacuity, and `muove check` exit codes 0, 1 and 2 | 3.3 | 20 | The R5 and R15 scenarios assert correctly. The exit codes are right on a holding, a failing and an unknown fixture. On P7's chain, a budget too small to finish gives `Unknown`, never `Fails` (A9) |
| 3.12 | `requires` as counterfactual dependence, with budget propagation, and no single point of failure | 3.11 | 20 | The R8 and R11 scenarios assert correctly. The results agree with `inputs/chatgpt-muove-feedback/muove_review/counterfactual_budget_model_results.json` |
| 3.13 | Minimal supports (port `insert_minimal`, bounded and lazy) and Berge hitting sets for monotone cones | 3.9 | 20 | The prototype's hospital and impact oracle cases give the answers `tests/oracle.rs` expects |
| 3.14 | Verified search for non-monotone cones, and the strength edge cases | 3.13 | 20 | `s when a` with `s when not a` is reported unbreakable within pattern `a`. An already-failing claim reports size 0. The typed-achievement oracle case passes |
| 3.15 | `prevent` and `achieve`, in `possible` and `acceptable` modes | 3.14, 3.11 | 20 | In the static dungeon, `achieve acceptable` excludes the placements that break `never` (R17) |

## WP4. firn module migration

- **Goal:** move every firn module, host, flake source and Clause program to
  Muove or TypeScript, with identical results, and retire Beagle and Clause
  from the build.
- **Facets:** Category feature. Spec: exact. Scope: cross-repo. Surface: rust,
  nix and ts. Verify: local-test, plus the drvPath eval, which is the Nix
  integration check.
- **Profile:** nixos-config is a tooling repository. Run the named check and
  land through its worktree and `safe-push`.
- **Files:** `crates/muove-import/**`, `lib/firn/dispatch.muove`, and in a
  nixos-config lane: `modules/**/*.muove`, `hosts/**/*.muove`,
  `flake.muove`, `native/firn_host.mjs`, `native/prewarm_host.mjs`,
  `native/repo_build.bjs` and the docs that name `.bnix`. Boxes 4.17 to
  4.22 add the callers DECISIONS D-33 names: `config/muove-revision`,
  `dotfiles/bin/{firn,firn-runtime-update}`, firn's `native/` readers and
  writers of module source, `modules/beagle/**`, `docs/**`, `AGENTS.md`
  and `dotfiles/agents/skills/machine-capacity/**`.
- **Lanes:** Beagle imports (4.1–4.5), Clause imports (4.6–4.8), firn's
  Clause programs (4.10–4.12), and inventories and tools (4.9, 4.16). 4.13 to
  4.15 join them. The D-33 callers (4.17–4.22, and the boxes 4.17 adds)
  share 4.13's nixos-config lane and land with 4.14, except 4.18, which
  lands in nixos-config on its own.
- **Done when:**
  1. `nix eval --raw .#nixosConfigurations.whiterabbit.config.system.build.toplevel.drvPath`
     is the same in the migrated lane as at its merge base, because
     nixos-config verifies only whiterabbit. The ashashi darwin drvPath is
     reported where it evaluates on Linux (A3).
  2. Every Beagle-output file is byte-identical, and every one of the 185
     Clause modules is parse-identical, listed with whether whiterabbit's
     drvPath covers it (A3).
  3. `rg -c 'beagle-build|compile-nix' native/repo_build*.bjs` finds no
     compile calls, and firn's dispatcher gives `firn.clause`'s plan for
     every command in its table (A3).
  4. A full `muove build` takes under 2 s, measured once (today about 90 s).

| Box | Work | Needs | Min | Done when |
| --- | --- | --- | --- | --- |
| 4.1 | `muove import bnix` for the F3 construct table | 2.6, 1.7 | 20 | The imported `kanata`, `users` and `firefox` emit byte-identical `.nix` |
| 4.2 | Construct coverage: map every F3 and F4 construct to its Muove form and emitted Nix, with one real file each (`with`, `//`, lambdas with formals, a nested home-manager module, `mkOutOfStoreSymlink`, symbol keys, `''${`, `import`) | 4.1 | 20 | The coverage table names a real file for each construct, and each of those files imports and emits byte-identically |
| 4.3 | Import every module `.bnix` in the manifest *(mechanical)* | 4.2 | 20 | Every module row of the 1.5 manifest emits byte-identically |
| 4.4 | Import the host files and `hardware-configuration` *(mechanical)* | 4.3 | 15 | Every host row of the manifest emits byte-identically |
| 4.5 | `flake.bnix` to `flake.muove` through `nix file`: `providerAdapter`, overlays, and the input section from 5.6 | 4.2, 5.6 | 20 | `flake.nix` is byte- or parse-identical |
| 4.6 | `muove import clause` for the Clause Nix-module subset. Run a construct census over the 185 modules and import a covering set of 20 | 2.6 | 20 | The census lists every construct the 185 use, and the 20 modules that cover them emit parse-identical Nix |
| 4.7 | Import the other 165 Clause modules *(mechanical)* | 4.6 | 20 | 185/185 are parse-identical, each listed with whether whiterabbit enables it |
| 4.8 | Move the 184 `tags.clause` files into facts in each module's `.muove` | 4.7, 5.1 | 20 | `all ?m where ?m is enabled on <host>` is unchanged for both hosts |
| 4.9 [x] | Caller matrix for retiring Beagle and Clause from the build: every caller in nixos-config of `beagle-build`, `beagle-rename`, `beagle-import-nix`, the compiler queries, the repair pipeline and `clause-workbench` *(research)* | — | 15 | Each caller is listed with its Muove replacement (SPEC §11.1) or DECISIONS D-27's drop. A caller of a dropped tool is named for the lead |
| 4.10 [x] | Classify every definition and handler of `native/firn.clause`, `prewarm.clause` and `firn-host-list.clause` as decision (to Muove) or mechanics (to TypeScript, DECISIONS T1) *(research)* | — | 15 | The table covers all 893 lines, with counts per destination and the dispatcher's command table extracted |
| 4.11 | `lib/firn/dispatch.muove`: the command table as facts and plans as a query; `muove query --json`; `native/firn_host.mjs` calls it | 4.10, 2.12, 3.6 | 20 | For every command in the table, the JSON plan equals the plan `firn.clause` produces |
| 4.12 | `prewarm.clause` and `firn-host-list.clause`: decisions to Muove queries, mechanics to TypeScript modules behind the existing bridges. Delete their Clause callers (DECISIONS D-33, G4) | 4.11 | 20 | `native/prewarm_native.test.sh` and `native/firn_dispatch.test.sh` pass without a workbench, `firn tag resolve`'s host list is unchanged, and `native/prewarm.clause`, `native/firn-host-list.{clause,js,d.ts}` and `FIRN_CLAUSE_HOST_LIST_MODULE` are gone |
| 4.13 | In a nixos-config lane, swap in all Muove output and run the drvPath eval | 4.4, 4.5, 4.7, 4.8 | 20 | Done when 1 and 2 above |
| 4.14 | firn integration: `firn repo build` and `firn repo diff` call `muove build [--check]`; the Beagle and Clause compile calls go; the dispatcher runs on Muove. Land through `safe-push` | 4.12, 4.13, 4.19, 4.21, 4.22, 4.23, 4.24, 4.25, 4.26, 4.28, 4.29, 4.31, 4.32, 4.33, 4.34 | 20 | Done when 1 to 4 above |
| 4.15 | Switch to the idiomatic layout (Tom chose it, DECISIONS T2), with the printer's capture check (SPEC §10.2) | 4.14 | 20 | whiterabbit's drvPath is unchanged, the capture check reports 0 bindings across all modules, and `muove build --check` is clean |
| 4.16 | `muove import nix`, which replaces `beagle-import-nix`: parse a hand-written module with rnix and lift it into the record register | 1.3, 2.6 | 20 | Importing `$FIRN_ROOT/modules/kanata/kanata.nix` as if hand-written gives a `.muove` that builds back to the same bytes |
| 4.17 [x] | Census of firn's readers and writers of module source (DECISIONS D-33): each with its commands and its destination, a `muove query --json` read, a `.muove` writer or a route to a `muove` command, sized into boxes added here *(research)* | — | 15 | Every file that `rg -l '\.bnix\|\.clause' native dotfiles/bin` lists at `$FIRN_ROOT`, other than the sources themselves, has a row naming a box of 20 minutes or less |
| 4.18 [x] | `machine-capacity.clause` to TypeScript in the skill's own host, keeping its F64 arithmetic; delete its Clause build steps (D-33, G4). Land in nixos-config through `safe-push`, then `agents sync` | — | 20 | With `FIRN_CLAUSE_WORKBENCH` unset, `machine-capacity.test.sh` passes against the rebuilt `machine-capacity.mjs`, and `rg -ci clause dotfiles/agents/skills/machine-capacity` finds nothing |
| 4.19 | firn's runtime pins Muove: `config/muove-revision`, `firn-runtime-update` builds and stages `bin/muove` and the dispatcher's Muove, and the Clause staging and `FIRN_CLAUSE_WORKBENCH` contract go (D-33, G3) | 4.11, 4.12, 4.18, 4.20 | 20 | `firn-runtime-update` installs `bin/muove` at the declared revision, a checkout that declares another revision is refused, `config/clause-revision` is gone, and `rg -l 'clause-workbench\|FIRN_CLAUSE\|clause-revision' native dotfiles/bin` finds nothing |
| 4.20 | firn's tag loader reads module tags through `muove query --json`; delete the Clause bridge, `load-beagle-module`, `native/module_metadata.clause` and `native/nix/` (D-33, G5) | 4.8, 4.11, 4.13, 5.1, 4.34 | 20 | `firn tag resolve whiterabbit`, `firn tag resolve ashashi` and `firn module list` give the same module and tag sets in the migrated lane as at its merge base, and `rg -l 'clause_source_host\|readModuleMetadataSource\|load-beagle-module\|module_metadata' native dotfiles/bin` finds nothing |
| 4.21 | firn's validators run `muove check` and `muove build --check`, and its schema extraction runs `muove schema extract`; delete firn's schema executable, `beagle doctor` and the validator fixtures (D-33) | 2.15, 4.1, 4.11, 4.13 | 20 | Each of the eight `$FIRN_ROOT/tests/fixtures/*.bnix`, imported, gets the accept or reject and the option path from `muove check` that `beagle-validate` gives it. `firn repo validate` exits 0 in the migrated lane, and `rg -l 'beagle-validate\|beagle-extract-schema\|beagle validate\|beagle doctor' native dotfiles/bin` finds nothing |
| 4.22 | Delete `modules/beagle` with its callers and `docs/repair-pipeline.md`; rewrite the compiler-query, schema, rename and import docs, the firn skill's routes and `AGENTS.md` for `muove` (D-33, G1, G2, G7) | 4.13 | 20 | `rg -n 'beagle-(syntax\|sig\|fields\|provides\|callers\|impact\|expand\|daemon\|fix\|repair\|check\|validate\|doctor\|lsp\|schema\|rename\|import-nix)\|clause-revision\|repair-pipeline' docs modules AGENTS.md` finds nothing, and whiterabbit's drvPath equals the lane's before the deletion |
| 4.23 | Tag resolution's host and generated-enable readers take facts through `muove query --json`; `+emit` routes to `muove build` (5.2). Delete the host/generated `.bnix` parsers and `emit-host-enables-bnix`; update the resolve driver fixtures | 4.20, 5.2 | 20 | `firn tag resolve whiterabbit` and `firn tag resolve ashashi` return their merge-base active-module sets; `+emit` gives 5.2's identical `_generated-enables.nix`; the old host/generated parsers and `.bnix` emitter have 0 callers |
| 4.24 | Tag commands read facts through `muove query --json` and edit host `.muove` facts for tag and module mutations; move the command/driver/family fixtures and retire their source parsers. Keep the tag index cache path (D-33) | 4.23 | 20 | The six mutation scenarios in `native/tag_family.test.sh` give the same resolved sets after the first and repeated calls, its refusal scenarios preserve the source bytes, and `firn tag list/show/filter/index/status` matches the merge-base facts |
| 4.25 | Inventory reads direct module references, explicit enables/disables and tag resolution through `muove query --json`; delete `host-config-from-source`, update configuration filenames and inventory fixtures | 4.20, 4.23 | 20 | `native/inventory_native.test.sh`'s host list/status and module all/used/unused/refs/status scenarios give the same sets using `.muove`, and `host-config-from-source` has 0 callers |
| 4.26 | Module and host authoring: `module add` and `template service/submodule/home/host` write `.muove` (host configuration and tag facts included) and generate Nix through `muove build`; update the authoring fixtures | 4.1, 2.15, 5.1 | 20 | All four module scaffold kinds and the sample host pass `muove check` and build their expected Nix; the duplicate-name, git-failure and collision-host scenarios keep their refusal and staging behavior |
| 4.28 | Flake-input resolution reads the derived input facts through `muove query --json`; emit routes to `muove build` of `flake.muove`. Delete the module `.bnix` reader and marker splicer; update flake-input driver/family fixtures | 4.5, 5.5, 5.6 | 20 | The existing flake-input scenarios give the same input set, emit produces 5.6's byte-identical six sections, and the old module parser and marker splicer have 0 callers |
| 4.29 | Platform and schema views: list/show/safelist read module options and platform-support facts through `muove query --json`, safelist emits Muove syntax, and schema explain/extract route to `muove schema explain/extract`. Delete views' `.bnix` scanning and `beagle.nix-schema-path` source parsing; update the view fixtures | 4.13, 5.5, 2.14, 2.15, 4.21 | 20 | The both/linux/darwin/neither/wildcard/freeform fixture cases keep their platform verdicts, the safelist snippet passes `muove check`, `firn schema explain services.kanata.enable` names the same declaring option and type, and views have 0 `beagle.nix-schema-path` imports |
| 4.31 | Repo doctor/watch/upgrade and quality readers use `.muove` source discovery, Muove validation and query facts for references/impact; remove owned `.bnix` metadata stripping and update repo quality/workflow fixtures | 4.13, 4.21, 4.29 | 20 | Doctor detects a deliberately stale generated output, watch invokes validation for a `.muove` change, upgrade's dry-run retains its phase order, and `strip-owned-authoring` has 0 callers |
| 4.32 | Checkout probes in `firn` and `prewarm_host.mjs` recognize migrated sources; system-policy digest names `.muove` and runtime Muove help. Update launcher/prewarm/policy fixtures | 4.11, 4.12, 4.19 | 20 | A scratch migrated checkout is accepted by the launcher and prewarm probe, a directory missing `flake.muove` is rejected, and the system-policy fixture matches the `.muove` instruction text |
| 4.33 | The boot-coupling budget runs as `muove check` (DECISIONS D-34): the budget step of `native/repo_build_native.test.sh` runs it on the migrated checkout, and `native/responsibility_projection.bjs`, its test, the test host's import and their build-family entries are deleted, not ported | 5.7, 4.21 | 20 | The step's budget verdict matches the merge-base calculation's (at the pin it fails, naming `blender` and `obsidian`), `muove check` reports no budget failure on a scratch copy that disables both on whiterabbit, and `rg -l 'responsibility.projection' native` finds nothing |
| 4.34 | Ancillary source references: `machine-update` stages `.muove`; `hardcoded-repo-path-check` recognizes `.muove` siblings; `myfunctions` names the migrated bash source; convo's interpreter-source assertion, which reads `native/nix/bash.clause`, uses the migrated source/output before 4.20 deletes it | 4.13 | 20 | The update staging selector accepts `.muove` and rejects an unexpected path, the path checker skips generated Nix with a `.muove` sibling, `myfunctions` names `default.muove`, and convo's empty-PATH interpreter ownership check passes |

## WP5. firn rules: tags, enables, responsibilities, flake inputs

- **Goal:** express tag resolution, responsibilities and flake inputs as Muove
  rules, so the same rules generate, explain and check them.
- **Facets:** Category feature. Spec: exact. Scope: cross-module. Surface:
  rust and muove. Verify: local-test.
- **Files:** `lib/firn/{tags,responsibilities,flake}.muove`,
  `crates/muove-import/src/bnix_tags.rs`, `tests/external/firn-tags/`.
- **Lane:** one.
- **Done when:**
  1. `_generated-enables.nix` is byte-identical for whiterabbit and ashashi
     (A4).
  2. `muove why firefox is enabled on whiterabbit` names `browsers` and both
     absence leaves (A4).
  3. The responsibilities requirement Holds on whiterabbit, and Fails with a
     proof when `obsidian is realized by obsidian-module` is withdrawn (A5).

| Box | Work | Needs | Min | Done when |
| --- | --- | --- | --- | --- |
| 5.1 | The `firn/tags` vocabulary and rules (SPEC §9.6), and importers for `enabled-tags.bnix`, `:tags`, `:tags-opt-in` and `:tag-overrides` | 2.11, 3.6 | 20 | `all ?m where ?m is enabled on <host>` equals the module set in today's `_generated-enables.bnix`, for both hosts |
| 5.2 | Generated enables, with tag overrides spliced as `OptionPath`s, through the nested comprehension | 5.1, 2.6 | 20 | Byte-identical `_generated-enables.nix` for both hosts |
| 5.3 | `why` and `why not` for enables | 5.1, 3.9, 3.10 | 20 | The firefox proof as in SPEC §9.6. A script shows that every module's tag attribution matches `firn tag resolve <host>` for both hosts |
| 5.4 | Responsibility triples as facts, plus the realization requirement | 2.11, 3.12 | 20 | Holds on whiterabbit; Fails with a proof after the withdrawal (A5) |
| 5.5 | Flake inputs: a derived URL relation with `at most one url per name`, and the platform-support requirement | 5.1 | 20 | The input set equals the content of today's 6 marker sections. An injected second URL for one name Fails, naming both modules, and `muove build` refuses to emit |
| 5.6 | The flake's input section, emitted from a query inside `flake.muove`'s `nix file` | 5.5, 2.6 | 20 | The emitted input section is byte-identical to today's marker-section output |
| 5.7 | The boot-coupling budget as requirements beside 5.4's (DECISIONS D-34): no module of environment-domain software enabled on the host; no boot-domain responsibility that reaches an environment-domain one through `depends on`; every required software with a module, a domain, a criticality and, in the boot domain, a lifecycle; every required capability provided; every boot-domain software a long-lived system service whose module is enabled. A host's modules are those `active-boot-modules` counts (`$FIRN_ROOT/native/responsibility_projection_test.bjs:135–143`): tag-enabled, plus direct enables, minus direct disables | 5.1, 5.4 | 20 | whiterabbit's facts at `$FIRN_ROOT` and the seven scenarios of `$FIRN_ROOT/native/responsibility_projection_test.bjs:57–124` fail exactly the requirements whose `assess-boot-coupling` part is non-empty at the merge base, naming the same items. On whiterabbit that is the environment-module requirement alone, naming `blender` and `obsidian` |

## WP6. Change review and the LSP

- **Goal:** the review gate of SPEC §8, its firn mode, and the LSP of §11.2.
- **Facets:** Category feature. Spec: exact. Scope: cross-module. Surface:
  rust. Verify: local-test.
- **Files:** `crates/muove-review/**`, `crates/muove-lsp/**`,
  `tests/scenarios/chatgpt-regressions/**`.
- **Lanes:** review (6.1–6.7, 6.10) and the LSP (6.8, 6.9).
- **Done when:**
  1. All 9 ChatGPT regression scenarios, written in Muove, give the verdicts
     their tests assert, and the three `intends` contracts give approve,
     reject and unknown (A6).
  2. Reviewing a scratch commit that removes firefox's `browsers` tag reports
     that firefox is no longer enabled on whiterabbit, matching
     `firn tag resolve` before and after, in under 5 s, measured once.

| Box | Work | Needs | Min | Done when |
| --- | --- | --- | --- | --- |
| 6.1 | Frozen baseline with SPEC §8.1's identity, and the restricted patch parser | 2.10, 2.12 | 20 | R9's rule placed outside the revision is a patch error naming its line. Renaming an entity changes the identity; renaming a rule variable does not |
| 6.2 | The edits as sentences, and the truth view: gained, lost, now stated, now derived | 6.1, 3.6 | 20 | R11's scenario reports "now stated", not "no longer true" |
| 6.3 | Requirement comparison by failing instance; introduced, fixed and pre-existing; the verdict table and both flags | 6.2, 3.11 | 20 | Removing B's backup while A's is already missing reports an introduced failure. R6's scenario is not approved. A patch that fixes every failure is approved without flags |
| 6.4 | The support view (minimum failure sets before and after) and the checkpoint view | 6.2, 3.14 | 20 | R4 prints "The smallest failure set fell from 2 facts to 1". R3's checkpoint bypass is reported |
| 6.5 | `intends` contracts, including `before` and `after` | 6.3 | 20 | Three contracts give approve, reject (an intended claim Fails) and unknown (its budget runs out) |
| 6.6 | The 9 ChatGPT regression scenarios in Muove | 6.4, 6.5 | 20 | 9 of 9 give their asserted verdicts (A6) |
| 6.7 | Git mode: `muove review --base REV`, with per-host consequences | 6.3, 5.2 | 20 | Done when 2 above |
| 6.8 | LSP: diagnostics, hover (reading, type, effect level), definition, references; `muove refs` | 2.5, 2.12 | 20 | A scripted JSON-RPC run over stdio gets the `devics` diagnostic and a hover on `kanata.muove`. `muove refs myConfig.modules.kanata.port` lists its uses in both kanata sources |
| 6.9 | LSP code lens "enabled on …", and `muove rename option` | 6.8, 5.3 | 20 | Renaming `myConfig.modules.kanata.port` updates both kanata sources, and the rebuilt `.nix` differs only in that name |
| 6.10 | `firn rebuild` runs `muove review --base <last switched revision>` before it switches. Lands in a nixos-config lane | 6.7, 4.14 | 20 | A dry run prints the per-host consequences, and exits non-zero on an introduced `Fails` |

## WP7. State, events, effects and temporal checks

- **Goal:** the step semantics, event log, effects, state search and temporal
  claims of SPEC §5, demonstrated on the dungeon.
- **Facets:** Category feature. Spec: exact. Scope: cross-module. Surface:
  rust. Verify: local-test.
- **Files:** `crates/muove-core/src/state.rs`,
  `crates/muove-engine/src/search.rs`, `crates/muove-runtime/**`,
  `examples/dungeon/**`, `tests/external/dungeon-oracle/`.
- **Lanes:** steps and search (7.1–7.7) and provenance and effects (7.8,
  7.9).
- **Done when:**
  1. The dungeon gives SPEC §5.6's answers and its five review verdicts
     through the full review path, and every reported witness replays (A7).
  2. The reachable-room set equals the independent state search in
     `inputs/chatgpt-muove-feedback/muove_review/dungeon_state_oracle_results.json`.

| Box | Work | Needs | Min | Done when |
| --- | --- | --- | --- | --- |
| 7.1 | The step core of SPEC §5.3: clauses, `set`, `include`, `withdraw`, `create`, `delete`, `emit`, the live set and the allocator, canonical order | 3.6 | 20 | Four scenarios give the specified post-state and effects: a dungeon move; `withdraw X` with `include X`, a no-op; a create that sets every field; a delete that removes every field and the entity |
| 7.2 | Step checks and rejections, `may skip`, and `at most one per step` effects | 7.1 | 20 | One scenario per rejection kind rejects with the specified sentence and leaves the state unchanged: no match, conflicting write, unfilled `exactly one`, derived mode, invariant, `ensures`, non-live include, and two competing effects (8). A temporal claim marked `invariant` is a static error |
| 7.3 | Sequences, revisions, the event log and replay, with the allocator in the state hash | 7.2 | 20 | Replaying a recorded log gives identical state hashes at every revision. A create followed by a delete hashes differently from the empty state |
| 7.4 | Breadth-first search for `eventually`, `always` and `never`, with scope labels, budgets, argument domains and the recorded event alphabet | 7.3 | 20 | The shortest witness to the Vault takes 5 steps. With `code sampled from ["ok"]`, `never compromised` is `Unknown(open domain)`; with `only in ["ok"]` it Holds and the runtime rejects `"boom"` |
| 7.5 | `every trace to S passes C`, `every E happens when C`, and temporal single points of failure | 7.4 | 20 | The rest of SPEC §5.6's answer table. An incomplete search never Holds a universal claim |
| 7.6 | The dungeon's five patches through the WP6 review | 7.5, 6.3 | 20 | The five verdicts of §5.6 (A7) |
| 7.7 | `trace` and `replay` | 7.4 | 15 | Every witness printed by 7.4 to 7.6 replays to its claimed state |
| 7.8 | Event provenance: `why S at revision r` | 7.3, 3.9 | 20 | `why Ash holds Iron-Key at revision 4` names the pick-up and "not withdrawn since" |
| 7.9 | `settled by` and pending effects, in the state and in checkpoints | 7.3 | 20 | Two requests are emitted and one is settled. After a checkpoint and restore, `pending()` lists exactly the other |

## WP8. North: runtime, bindings, the menu slice, then the rest

- **Goal:** embed the runtime in Rust, generate typed bindings, prove the
  menu slice equivalent to Clause by replay, then move every slice and remove
  Clause.
- **Facets:** Category feature. Spec: exact. Scope: cross-repo. Surface:
  rust. Verify: local-test, plus north's own `cargo test`.
- **Profile:** north is a tooling repository. Run its named check and land
  through its worktree and `safe-push`.
- **Files:** `crates/muove-runtime/**`, `crates/muove-bindgen/**`,
  `crates/muove-import/src/clause*.rs`, and in a north lane:
  `clause/*.muove`, `flake.muove`, a transition recorder in
  `src/clause_state.rs`, and `Cargo.toml`.
- **Lanes:** runtime and bindings (8.1–8.3), the north recorder and text
  references (8.4, 8.5, which start at once), and the slice work (8.6 on).
- **Done when:**
  1. The sequences recorded from `main.rs:3547` and
     `restart_storage_tests.rs:388` replay on `muove-runtime` under SPEC
     §10.4's observation contract, and one menu step at 1,000 rows takes
     under 1 ms, measured once (A8).
  2. Every slice of `north.clause` replays under the same contract with its
     coverage reported, and north's tests pass with the Clause crates removed
     from `Cargo.toml` (A11).

| Box | Work | Needs | Min | Done when |
| --- | --- | --- | --- | --- |
| 8.1 | The runtime API of SPEC §10.4, with checkpoint, restore and `pending()` | 7.3, 7.9 | 20 | On the dungeon, restoring a checkpoint and continuing gives the same state hash as running straight through, and `rg -l 'std::(env\|fs\|net\|process\|thread\|time)' crates --glob '**/src/**' --glob '!**/muove-cli/**' --glob '!**/muove-lsp/**'` finds nothing (SPEC §10.4) |
| 8.2 | `muove compile` images, and `muove run` over JSON lines | 8.1 | 15 | The dungeon's witness, run as JSON lines through `muove run`, ends in the same view as through the Rust API |
| 8.3 | `muove bindgen rust` for entities, text unions, events, effects and views | 8.1, 2.12 | 20 | The generated menu bindings compile, and a test drives `MoveMenu` through the typed API |
| 8.4 [x] | In a north lane, record Clause's `lowercase`, `contains` and word functions on empty, ASCII and non-ASCII inputs *(research)* | — | 20 | At least 30 recorded cases, kept as the external reference for SPEC §3.5's built-ins |
| 8.5 [x] | In a north lane, a transition recorder: every occurrence with its admission result, ordered effects, transaction boundaries and projections, and handler and clause coverage counted at the dispatcher | — | 20 | Recording the two named tests produces logs with all four observation kinds, and a coverage report |
| 8.6 | Import the menu slice mechanically (SPEC §9.7): `north.clause` L447–827, `host-effect` and `active-conversation`; every handler `may skip`; maintenance handlers kept; the mailbox effect `at most one per step`; `one` fields mapped by SPEC §4.1; FIFO rules as written | 2.12, 7.2, 8.4 | 20 | `muove check` passes on the imported slice, and the built-ins match all of 8.4's cases |
| 8.7 | Replay the two recorded sequences on Muove, with the entity correspondence | 8.5, 8.6, 8.1 | 20 | Admissions, effects, projections and restart continuation are identical at every step. The coverage report is printed |
| 8.8 | Refactor to SPEC §9.7's end state: derived count, visibility and position; `delete` for the teardown; `may skip` removed where no recorded occurrence skipped | 8.7 | 20 | The replay is still identical after each change. Report the handler count before and after |
| 8.9 | Replace the menu mirror types and decoding in the north lane with the generated bindings, and measure | 8.3, 8.8 | 20 | Report the line count before and after. The north lane's `cargo test` passes. One menu step at 1,000 rows takes under 1 ms (A8) |
| 8.10 | North's `flake.clause` to `flake.muove` through `nix file` | 2.6 | 15 | north's `flake.nix` is byte- or parse-identical |
| 8.11 | `muove import clause-checkpoint`, and a restart from a real north checkpoint | 8.1, 8.6 | 20 | A checkpoint taken during the recorded run converts. Restoring it and continuing gives identical observations, and the allocator is past every id in use |
| 8.12 | Slice the rest of `north.clause` by line range, with handlers, relations, Rust call sites and recorded tests; size each slice into 20-minute boxes *(research)* | 8.7 | 20 | Every line of `north.clause` belongs to exactly one slice, and each slice's boxes are appended to this table with Needs and Done when |
| 8.13 | The slice boxes from 8.12, each imported and replayed by the method of 8.6 to 8.8 (about 30 boxes) | 8.12 | — | Each slice replays under the observation contract, with its coverage reported |
| 8.14 | Host integration: north runs on `muove-runtime` behind a build feature | 8.13 | 20 | north's `cargo test` passes with the feature on and off |
| 8.15 | Remove the Clause crates from `Cargo.toml` and the feature, and delete the clause-authoring skill from `nixos-config:dotfiles/agents/skills/` (DECISIONS D-33, G7) | 8.14 | 15 | north's `cargo test` passes, `rg -c clause Cargo.toml` finds 0 (A11), and `agents sync` no longer lists clause-authoring |

## After the MVP

Porting firn's 59 `.bjs` files (23,268 LOC) to TypeScript on Bun (DECISIONS
T1) is a separate effort. Until then, the Beagle compiler builds only those
files, never Nix.

### The first general-purpose milestone (a sketch, not boxes)

Muove is meant to be a general-purpose language (DECISIONS T4). After A11,
its next proof is a program that is neither configuration nor north: the
dungeon of SPEC §5.6, made playable. It reuses A7's program, so the
milestone measures what is new, a host platform and the language's answers
used live, rather than a new domain.

- **The program.** A turn-based game on an `Int` grid: the hero, doors, keys
  and traps of §5.6, plus a guard that steps toward the hero each turn,
  picking its step with `first … by` distance. The host reads a `Screen`
  view after each step and draws it.
- **The host.** A small terminal platform on `muove-runtime`. Key presses
  come in as events, `exit` and `bell` go out as effects, and the event log
  is written for replay. The same platform runs command-line tools, with
  arguments, standard input and file contents in as events, and output and
  exit codes out as effects.
- **Answers in play.** Asking the guard why the gate stays shut answers with
  `why not`, built from the proof: the first draft's honest dialogue.
- **What it measures.**
  1. A recorded session replays to identical state hashes.
  2. One turn on a generated level with 1,000 entities takes under 1 ms,
     A8's budget.
  3. `muove review` of a level change, such as the ladder patch, names the
     change in reachable rooms before anyone plays it.
- **What it may bring in from SPEC §12.** A guard that plans more than one
  greedy step needs values computed in recursion. Each such item arrives
  with this program as its example (G1); what the game does not need stays
  out.
- **Then, in order.** A command-line tool on the same platform, such as one
  of firn's TypeScript mechanics rewritten with identical output. It tests
  sequential I/O and decides whether steps should wait on reads. Then a
  Greywrought encounter in the browser, through a wasm32 build of the
  runtime.
