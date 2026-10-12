# After firn: Galileo against other configuration languages

Tom said yes (through Plato, 2026-10-11) to measuring Galileo against the
alternatives on firn's real configuration. This note is the plan. The run
starts after boxes 4.13 and 4.14 land, and galileo#1 tracks it. Numbers alone
decide, and the report states every loss.

- **Goal:** four numbers for each language, all on the same firn
  configuration:
  - size: the source it takes;
  - mistakes: how many real and seeded mistakes its checker rejects before a
    rebuild;
  - explanation: whether its answer to "why is X on?" or "why isn't Y on?"
    points at the right source;
  - cost: how long its check takes after a one-module change.
- **Columns:** Galileo; today's Clause and Beagle; Nix, the baseline every
  column still compiles to; Nickel, CUE, Dhall and KCL.
- **Facets:** Category research. Spec: judged. Scope: cross-repo. Surface:
  rust (`tools/compare`), nix, and the outside languages under `bench/`.
  Verify: measurement (the export gates, the replay and two timing sessions).
- **Files:** `bench/**`, `tools/compare/**`, this note's Results section and
  PLAN.md's After firn section. Nothing lands in nixos-config.
- **Done when** (galileo#1):
  1. Size: every column has its line and file counts for the sample, and
     Galileo, Clause and Beagle, and generated Nix also have counts for full
     firn. Every outside port has passed its gate.
  2. Mistakes: each column has its count of rejected cases, out of N real
     cases and 40 seeded mutants, shown beside the drvPath-eval floor.
  3. Explanation: each tool has a score out of 20 on the ten questions,
     from a judge who saw the answers blind.
  4. Cost: each column has its median check seconds on whiterabbit, under an
     exclusive lease.
  5. The Results section gives each axis's rank, and every axis where
     Galileo does not rank first, with the gap.
- **Not required:**
  - porting all of firn to the outside languages;
  - replaying the corpus on full firn;
  - the dispatcher (`lib/firn/dispatch.gal`, notes/4.10), which is about
    plans rather than modules, tags and hosts;
  - a second judge;
  - any change that lands in firn.

## Fixed inputs

- **The sample's firn** is the pin that every Galileo oracle reads:
  `FIRN_ROOT=~/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9`.
  It holds both hosts: whiterabbit (linux) and ashashi (darwin). firn's
  `main` has since retired ashashi (`7d7a8c9d`).
- **Full firn** is counted at two commits. L is the firn commit where 4.14 lands, and it counts for Galileo. C (`cbb7c069`) counts for Clause and Beagle: it is the last first-parent ancestor of L whose committed Nix is the output of its own `firn repo build` through Beagle and Clause; its child `d07b5b9e` rewrote those outputs with `galileo build`. L's first parent no longer does: its `firn repo build` runs `galileo build`, and it holds no `.clause` file. Both are extracted with `git archive` into scratch. C is 34 first-parent commits before L, with 237 module directories to L's 236 and whiterabbit as the only host at both, so the two counts cover nearly the same configuration, and the report says so beside them.
- **Tools** run from `bench/flake.nix`'s shell and are never vendored.
  Licenses were checked on 2026-10-11 against each upstream LICENSE file
  (GitHub's license API) and against nixpkgs' metadata:

| Tool | nixpkgs version | License | Upstream |
| --- | --- | --- | --- |
| nickel | 1.16.0 | MIT | tweag/nickel |
| cue | 0.16.1 | Apache-2.0 | cue-lang/cue |
| dhall, dhall-json | 1.42.3, 1.7.12 | BSD-3-Clause | dhall-lang/dhall-haskell |
| kcl | 0.12.3 | Apache-2.0 | kcl-lang/kcl, kcl-lang/cli |
| nixfmt | the bench lock's | MPL-2.0 | NixOS/nixfmt |
| hyperfine | 1.20.0 | Apache-2.0 or MIT | sharkdp/hyperfine |

All of these run as programs. Nothing from them is copied into `bench/`, so
no attribution is owed. MPL-2.0 binds only modified nixfmt files, and none are
modified.

## Fairness rules

1. Every column expresses the same sample and the same five rules. Before it
   is measured, each column passes the same gate: its export equals the
   reference for both hosts, and through the shim it gives the pin's
   whiterabbit drvPath.
2. Every column gets the same NixOS option schema: `galileo schema extract`
   output, restricted to the options the sample sets, printed mechanically
   into each language's types. Generated schema lines are reported but not
   counted.
3. Galileo's sample is the source firn ships: 4.13's import of the pin's
   sample, unpolished. The outside ports are written by hand to be short and
   idiomatic, so any Galileo win on size is conservative.
4. A port lands before its porter opens `bench/corpus/` or
   `bench/questions/`, and it does not change afterwards. A changed port
   reruns its replay and its answers.
5. Each tool is scored as it ships, using the nearest command its own
   documentation offers. A port does not record reasons as data in order to
   answer the questions.
6. The outside-language sample stays out of firn. The ports are new
   translations kept in `bench/`, not copies of fixtures (D-19). The Galileo
   sample is generated at run time. firn is touched only through scratch
   worktrees, which are never pushed and are removed when their box ends.

## The sample

The sample is ten modules, chosen to cover both source languages and the
feature classes of firn's sources that notes/4.17's readers serve: tags,
opt-ins, overrides, flake inputs, options and schema, platform, inventory
and responsibilities. Line numbers are at the pin.

| # | Module | Source | Feature classes |
| --- | --- | --- | --- |
| 1 | firefox | `modules/firefox/{default,firefox}.bnix` | Beagle; tag `browsers` (`default.bnix:6`); tag override `browsers` → `firefox.default true` (:7); flake input `nur`; an enable and a typed option with a default (:12–16); imports |
| 2 | zen-browser | `modules/zen-browser/{default,zen-browser}.bnix` | Beagle; opts in to `browsers` (:6), which whiterabbit takes with `+zen-browser` |
| 3 | kanata | `modules/kanata/{default,kanata}.bnix` | Beagle; typed options (path, port, lists) that the host sets (`hosts/whiterabbit/configuration.bnix:30–38`); direct enable; `''` text; the first win |
| 4 | users | `modules/users/{default,users}.bnix` | Beagle; options that other modules read (`username`), set by the host (:7–10) |
| 5 | lane-gc | `modules/lane-gc/default.bnix` | Beagle; a K7 module whose enable reads a `let` name; direct host enable (:63) |
| 6 | atuin | `native/nix/atuin.clause`, `modules/atuin/tags.clause` | Clause; tag `terminal`; supported on darwin (`config/darwin-modules.json:2`); a home-manager key computed from `users.username` |
| 7 | chrome | `native/nix/chrome.clause`, `modules/chrome/tags.clause` | Clause; opts in to `browsers`, which whiterabbit takes with `+chrome` |
| 8 | godot | `native/nix/godot.clause`, `modules/godot/tags.clause` | Clause; tag `creative`, excluded by whiterabbit's `[creative -godot]` (`enabled-tags.bnix:22`) |
| 9 | bun | `native/nix/bun.clause`, `modules/bun/tags.clause` | Clause; tag `javascript`; listed in whiterabbit's `:disabled` (:29) |
| 10 | obsidian | `native/nix/obsidian.clause`, `modules/obsidian/tags.clause` | Clause; tag `notes`; realizes `obsidian` in `hosts/whiterabbit/responsibilities.bnix` (:26, :28, :38, :40, :42) |

The host parts are:

- both hosts' `enabled-tags.bnix`, in full;
- whiterabbit's `configuration.bnix` entries for users, kanata and lane-gc;
- the responsibility triples that name obsidian.

The modules come to 240 source lines, and their `tags.clause` files to 20
more. The host parts name 74 modules outside the sample; every column receives their names, tags and opt-ins as context, which is reported beside the size score and not counted.

**Every column implements the same five rules,** as `lib/firn` does:

1. **Tag resolution.** A host enables the modules of the tags it selects,
   adds its opt-ins and direct enables, and removes its exclusions and its
   `:disabled` list (SPEC §9.6).
2. **The platform gate.** On darwin, a host enables only the modules that
   `config/darwin-modules.json` lists (5.1).
3. **Tag overrides.** A selected tag sets the option values that its modules
   declare for it, as with firefox.
4. **Realization.** An enabled module realizes every required software (5.4;
   this holds on whiterabbit).
5. **Flake inputs.** Each input name has at most one URL (5.5).

**Rule units.** Each rule is one language-neutral unit in `sample.tsv`, kind `rule`, with no pin lines: `rule.tag-resolution`, `rule.platform-gate`, `rule.tag-override`, `rule.realization` and `rule.flake-input`. Each column's `units.tsv` maps every rule unit to ranges of its own rule sources: `lib/firn` for Galileo, the five pin `.bjs` files above for Clause and Beagle, and the port's own files for the others. A range may be a whole file and may serve more than one rule. Only axis 5's cards and the key's A column cite rule units, and axis 1 still counts the rules row by file.

5.7's boot-coupling budget is left out. It fails at the pin, naming blender
and obsidian, so no column's unmutated sample would pass its check.

## 1. Size

- **Measured:** files, and lines that are neither blank nor comment-only,
  counted after each language's own formatter (`galileo fmt`,
  `nickel format`, `cue fmt`, `dhall format`, `kcl fmt`, `nixfmt`). Clause
  and Beagle have no formatter, so they count as committed.
  `tools/compare size` prints one row for each column and part: modules,
  hosts and rules.
- **The sample, in seven columns:**
  - **Galileo:** the generated sample, plus the `lib/firn` files that
    implement the five rules.
  - **Clause and Beagle:** the pin files above. Their rules row counts the
    pin's `native/tag_resolve.bjs`, `tag_resolve_family.bjs`,
    `tag_inputs.bjs`, `responsibility_projection.bjs` and `flake_input.bjs`.
    Those files also hold parsing and I/O, and the report says so beside the
    number.
  - **Nix:** the pin's emitted `.nix` for the sample, run through `nixfmt`.
    Its hosts row is the sample's lines of `_generated-enables.nix`. It has
    no rules row, since Nix has no tags.
  - **The outside ports:** their sources under `bench/<lang>/`. Each also
    reports its escapes: values passed through to Nix as `$ref` or `$call`,
    which its checker cannot see.
- **Full firn, in three columns:**
  - **Galileo at L:** `modules/**/*.gal`, `hosts/**/*.gal` and `flake.gal`,
    plus the rules files.
  - **Clause and Beagle at C:** `modules/**/*.bnix`, `native/nix/*.clause`,
    `modules/*/tags.clause`, `hosts/**/*.bnix` except
    `_generated-enables.bnix`, `flake.bnix` and `hardware-configuration.bnix`.
  - **Generated Nix at C.**

  For scale, the pin has 63 `.bnix` files (2,179 lines), 185 Clause files in
  `native/nix` (2,274), 184 `tags.clause` files (736), and 231 emitted
  `.nix` files under `modules/` and `hosts/` (1,955 lines, unformatted).
- **Scoring rule:** a column's score is the sum of its modules, hosts and
  rules rows. Fewer lines rank higher, and scores within 5% of each other
  tie. File counts are reported beside the score.

## 2. Mistakes caught before a rebuild

- **Corpus source:** firn's own history in `~/code/nixos-config/main` and
  its CI, measured on 2026-10-11:
  - The Build workflow on `main` has 31 red streaks (1,646 runs since
    2026-08-20, 496 of them failed). Each streak contributes its first
    failing run's error (`gh run view --log-failed`) and the commit that
    turned it green.
  - 101 commits since 2026-03-12 touch `*.bnix`, `*.clause` or `*.nix` and
    match `git log -i -E --grep='\bfix|revert|broke|regress'`. All of
    history holds 8 reverts.
- **Selection:**
  - A candidate is kept when its mistake is in the configuration: an
    evaluation error, an unknown or mistyped option, a wrong value, a
    module, tag or host selection, a platform, a flake input, or a package
    reference.
  - Dropped candidates are counted by reason: runner, cache, network, disk,
    secret and timeout failures, and fixes to a running service's behavior.
  - Duplicates found by both sources merge by fix commit.
  - At most 30 real cases are kept, the most recent first. If fewer than 10
    survive, the git-log search extends to all of history.
- **Seeded mutants:** 40, drawn with seed 4114 from every site in the sample
  where one of these operators applies:

| Op | Mistake |
| --- | --- |
| M1 | A NixOS option path misspelled where a module sets it |
| M2 | A module's own option misspelled where it is read or set (`cfg.devices` → `cfg.devics`) |
| M3 | A value of the wrong type: a bool as text, a port as text, a list as one value |
| M4 | A value out of its type's range (kanata's port 70700) |
| M5 | A host enables a module that does not exist |
| M6 | A tag misspelled in a module or in a host selection |
| M7 | An opt-in or exclusion that names no module of its tag (`+zen-browsr`) |
| M8 | A realization withdrawn (`obsidian is realized by obsidian-module`) |
| M9 | A darwin host selects or enables a module that darwin does not support |
| M10 | A second URL for the flake input `nur` |
| M11 | A misspelled package name (`pkgs.atuin` → `pkgs.atuinn`) |

- **Cards:** each case is one language-neutral row in
  `bench/corpus/cases.tsv`. The row gives the case's evidence (a run URL or
  commit, and the error line), its class, the sample unit it edits and the
  edit itself. A real case on a module outside the sample is transplanted
  onto the sample unit of the same class, and its row says so. There are
  two kinds of card:
  - A token card is a rename or a literal swap. It gives an anchor and its
    replacement, and `tools/compare replay` applies it through a language's
    `units.tsv` wherever the anchor matches exactly one site in the card's
    unit. Where it does not, that language's replay box writes
    `bench/corpus/<id>/<lang>.patch`. Every patch changes exactly the places
    where the column writes what the pin’s edit changes, wherever those
    places are, and nothing else. The card’s unit names the mistake; it does
    not limit where the edit may go. It makes the card’s edit to that
    language’s spelling of the same element: the same new name, the same
    literal in its syntax, or the same target, and nothing else. Two
    spelling rules apply to every column: (i) when the column writes one pin
    element in several places, edit every place; (ii) when a renamed field
    is written in the column as a list entry or a fact, remove the entry and
    set the misspelled field where the column sets that module’s options. A
    patch never edits rule code that other modules share. Each such patch is
    committed before the column's first checked replay and is applied as a
    structural patch. The `mistakes.tsv` note lists each changed file:line.
  - A structural card has one patch per language,
    `bench/corpus/<id>/<lang>.patch`, written in that language's replay box.
  - A card is `not held` by a column when it edits configuration that the
    contract export does not carry and the column's sources do not hold,
    so the column leaves it in the pin's Nix. Galileo's sample and the four
    ports leave flake-input `follows` edges there. Such a card gets no patch
    in that column. It stays in the shared N and counts as not rejected.
    The three `follows` cards are `not held` in all four ports.
- **Truth:** each card is applied to the pin and evaluated: whiterabbit's
  drvPath, or ashashi's enabled set for darwin cases. Each card gets one
  label:
  - `eval-error`: evaluation fails;
  - `silent`: evaluation passes but the result changes;
  - `equivalent`: nothing changes, and the case is dropped.

  This evaluation is also Nix's column, the floor every column pays when it
  rebuilds.
- **Replay per column:** apply the card, run the column's check and record
  three things: the exit status, the first diagnostic line, and whether that
  line names the edited unit's file. Each `mistakes.tsv` row says how the
  card was applied: `token`, `translated`, `structural` or `not held`. Then
  restore the source.

| Column | Check (must exit 0 on the unmutated sample) |
| --- | --- |
| Galileo | `galileo check && galileo build` on a scratch copy of the generated sample; the build regenerates its Nix, as `firn repo build` does for Clause and Beagle |
| Clause and Beagle | the pin's `firn repo build && firn repo validate` (the pin's AGENTS.md check after `.bnix` edits; its CI Tests job adds a drift diff that every `.bnix` card would trip), in a scratch worktree of the pin |
| Nix | `nix eval --raw .#nixosConfigurations.whiterabbit.config.system.build.toplevel.drvPath` at the pin |
| Nickel | `nickel export --format json main.ncl` |
| CUE | `cue vet ./... && cue export --out json` |
| Dhall | `dhall-to-json --file main.dhall` |
| KCL | `kcl run main.k --format json` |

Replay runs only these commands. `galileo build --check`, and each
`check.sh`'s comparisons against the contract export, the pin's Nix or its
`drvPath`, compare a column against the expected output, so they would reject
every card that changes the output.

- **Scoring rule:** a column's score is the number of kept cases its check
  rejects. More rejections rank higher, and scores within one case tie.
  Reported beside the score:
  - the real and seeded counts;
  - the silent cases caught;
  - the share of rejections that name the edited file;
  - the `not held` cards, which count as not rejected because the column's
    check never sees that configuration, each with where the column leaves
    it; for Galileo, that firn at L holds `follows` edges in `flake.gal`,
    which the sample does not import.

  Cases that no column rejects are listed.

## 3. Explanation

The ten questions are about the sample, and each one asks about a real
selection in the pin's host files. Each question has a key:

- D, the source units an answer must cite;
- A, units it may also cite without penalty: absence conditions, the option
  default and the rules library, which is all five rule units in every row.

The key is frozen in `bench/questions/key.tsv` before any tool is asked.
In the D column, file names are short: `tags` is the host's
`enabled-tags.bnix` and `config` is whiterabbit's `configuration.bnix`.

| # | Question | D at the pin | Origin |
| --- | --- | --- | --- |
| Q1 | Why is firefox enabled on whiterabbit? | `browsers` (tags :20); firefox's `:tags [browsers]` (default.bnix :6) | SPEC §9.6's proof; tag activation `098c5b6c` |
| Q2 | Why is obsidian enabled on whiterabbit? | `notes` (tags :26); obsidian's tag `notes` (tags.clause :4) | `9995c602` later moved it to `:disabled` |
| Q3 | Why is zen-browser enabled on whiterabbit? | `+zen-browser` in `[browsers …]` (tags :20); its opt-in (default.bnix :6) | `098c5b6c` |
| Q4 | Why is chrome enabled on whiterabbit? | `+chrome` (tags :20); chrome's opt-in (tags.clause :4) | `098c5b6c` |
| Q5 | Why is `myConfig.modules.firefox.default` true on whiterabbit? | the override (default.bnix :7); `browsers` (tags :20). A also holds the option default (:15) | the override shadows the default |
| Q6 | Why is lane-gc enabled on whiterabbit? | the direct enable (config :63) | `05e0bc46` |
| Q7 | Why is atuin enabled on ashashi? | `terminal` (ashashi tags :7); atuin's tag `terminal`; `"atuin"` (darwin-modules.json :2) | platform filter `d2b2289e` |
| Q8 | Why isn't godot enabled on whiterabbit? | `-godot` in `[creative -godot]` (tags :22) | `098c5b6c` |
| Q9 | Why isn't bun enabled on whiterabbit? | `bun` in `:disabled` (tags :29) | AF.33 cites the commit that added it |
| Q10 | Why isn't chrome enabled on ashashi? | ashashi has no `browsers` selection and no `+chrome` (ashashi tags :6–9); chrome is opt-in only (tags.clause :4); chrome is missing from darwin-modules.json | three independent blockers; `d2b2289e` |

**Each tool's nearest command** is named before the questions are asked:

- **Galileo:** `galileo why '<m> is enabled on <host>'`, and
  `galileo why not … --via <rule>`.
- **Clause and Beagle:** the attribution from `firn tag resolve <host>`,
  plus `firn module status <m>`, both run at the pin.
- **Nix:** `nix eval --json` of
  `options.myConfig.modules.<m>.enable.definitionsWithLocations` on the
  pin's host (`.default` for Q5).
- **Nickel, CUE, Dhall and KCL:** `nickel query`, `cue eval -e`, nothing
  known for Dhall, and `kcl run -S`. Each porter confirms or replaces these
  from the tool's documentation in its port box. A tool with no such
  command answers "no equivalent".

**Rubric,** for each answer:

- **Right source (2):** the answer cites every unit of D, and nothing
  outside D and A.
- **Partially right (1):** the answer cites part of D, or a file generated
  from D (such as `_generated-enables.nix`), or all of D plus a cause
  outside D and A.
- **Wrong (0):** the answer cites nothing from D or generated from D, or
  there is no answer.

**The judge** is one cold Codex session with no repository access and no
Galileo context. It gets only the rubric, the key and the blinded answers.
`tools/compare blind` prepares those answers in three steps:

1. It rewrites each cited path and line as that unit's description from the
   column's `units.tsv`, such as "host whiterabbit selects tag browsers",
   "a generated enables file" or "another file".
2. It strips tool names, command names and file extensions.
3. It shuffles each question's seven answers under letters, and seals the
   mapping in `bench/questions/seal.tsv` until scoring ends.

A mechanical score, computed from the cited unit sets, is printed beside the
judge's score, and every difference is listed.

**Scoring rule:** a tool's score is the sum over the ten questions, out of
20. Higher ranks higher, and equal sums tie. The counts of right, partial and
wrong answers are reported beside it.

## 4. Cost

- **Host:** whiterabbit (24 threads, 94 GiB), in two exclusive
  `machine-capacity` sessions of at most 15 minutes each. All builds are
  release builds.
- **The change:** the same one-unit edit in every column of (b) and (c), which
  flips firefox's override value (`firefox.default` under `browsers`) from
  true to false; (a) uses the edit named there. Each run uses
  `hyperfine --warmup 2 --runs 10 --export-json`, with
  `--prepare` applying and reverting the edit alternately, so every run
  checks a changed source.
- **Inputs:**
  - **(a) Full firn:** Galileo at L (`galileo check && galileo build --layout idiomatic`, the build `firn repo build` runs at L), and Clause and Beagle at C (`firn repo build` in a detached scratch worktree `~/code/nixos-config/worktrees/bench-c`, with C's runtime staged as AF.10 stages the pin's; about 90 s per run). Neither commit holds firefox's override, so the edit flips firefox's declared default from false to true: `modules/firefox/default.bnix:14` at C and `modules/firefox/default.gal:11` at L, each changing only `modules/firefox/default.nix:7` to `default = true;`. `--prepare` alternates the flip with `git checkout -- modules/firefox`. The floor is whiterabbit's drvPath eval with `--option eval-cache false`, run 3 times.
  - **(b) The sample:** Galileo and the four outside languages. Clause and
    Beagle cannot check the sample apart from the whole repository.
  - **(c) A scaled corpus:** the same five columns, on the sample cloned
    21 times with renamed modules. That makes 210 modules; firn has 214.
- **Scoring rule:** a column's score is the median wall time of its check on
  each input. Lower ranks higher. Two times tie when they are within 10% or
  within 50 ms, whichever is larger. The minimum and maximum are reported.

## Rejected alternatives

- Port all of firn to each outside language: that is about 4,000 source
  lines per language, while ten modules covering every feature class answer
  the same question.
- Compare outside ports by JSON alone, with no drvPath gate: a JSON contract
  can drift from what firn builds. The drvPath is firn's own integration
  check (4.13).
- Have the outside ports emit Nix text: none of the four languages has a Nix
  printer, and string templates would hide values from their checkers.
- Hand-polish Galileo's sample: Galileo should be judged on the source firn
  ships.
- Give the outside languages no NixOS schema: that would measure their
  ecosystems, not the languages, and Galileo also reads an extracted schema.
- Let the ports record reasons as data: that answers by porter effort what
  Galileo answers from its rules, and the size axis would then have to count
  it.
- Seed mutants by hand in each language: shared cards make every column face
  the same mistake.
- Score explanations by unit sets alone: the comparison asks for a judge, so
  the mechanical score stays only as a cross-check.
- Two judges and a tie-break: the mechanical score already exposes any
  disagreement.

## Layout of `bench/`

```text
bench/flake.nix, flake.lock    the tool shell; LICENSES.tsv beside it
bench/firn-commits.tsv         L and C
bench/sample.tsv               the sample's source units: id, kind, pin file and lines; the five rule units have none
bench/schema/                  the extracted option subset and its printed types per language
bench/contract/                shim.nix, gate.sh, reference.nix, make-reference.sh; out/ (git-ignored) holds both hosts' reference exports and private.json, written at run time from the pin
bench/galileo/                 make.sh (generates the sample from FIRN_ROOT, D-19), check.sh, units.tsv
bench/clause-beagle/           check.sh and units.tsv; the sources stay in the pin
bench/{nickel,cue,dhall,kcl}/  each port, with its check.sh and units.tsv; it reads contract/out/private.json and writes its exports to its git-ignored out/
bench/corpus/                  candidates.tsv, cases.tsv and per-language patches
bench/questions/               key.tsv, answers/, blinded/, seal.tsv
bench/results/                 size.tsv, mistakes.tsv, explanation.tsv, cost/*.json
tools/compare/                 size, replay, blind, scale, types (std only, dev only)
```

## Boxes

An export is the JSON that a column hands to `bench/contract/shim.nix` for one host. It holds the enabled sample modules, the host settings, and each module's options and config as that host evaluates them: every `mkIf` is decided (its body when true, its attribute left out when false), and reads of the sample's own options are the host's values. Only what JSON cannot hold stays symbolic: reads of `pkgs`, `inputs`, `flakeRoot` and `lib.types` appear as `$ref` values; calls of `lib` and `builtins` functions (option declarations, priorities, `makeBinPath`, `readFile`, and `lib.concatStrings` for a string that embeds one of these) appear as `$call` values; and a module function that ignores its arguments appears as its body.

Both hosts' reference exports, `bench/contract/out/private.json` and every column's exports are written at run time into git-ignored `bench/**/out/` directories and are never committed, because they hold the pin's private host settings. `private.json` holds only those settings, each host's `users.email` and whiterabbit's `kanata.devices`, as `make-reference.sh` reads them from the pin. Each port reads it through its language's own JSON import (Dhall through `json-to-dhall` in `check.sh`), types it with `bench/schema/<lang>/`, and writes every other setting, kanata's port included, in its own source. The lines that read it count toward the port's size. The file itself counts in no column and is reported like `context.json`. Galileo's generated host source holds the same two values, taken from the pin. No card edits them.

| Box | Work | Needs | Min | Done when |
| --- | --- | --- | --- | --- |
| AF.1 | Set up the bench shell and `LICENSES.tsv`; record L and C from firn's log *(mechanical)* | 4.14 | 10 | `nix develop ./bench -c` prints the versions of the seven tools above; `LICENSES.tsv` has seven rows; `firn-commits.tsv` names L and C; C is an ancestor of L that holds `native/nix/*.clause` and whose `native/repo_build.bjs` selects `.bnix` sources, its first-parent child is `d07b5b9e`, and L holds `flake.gal` and no `.bnix` |
| AF.1b | Apply edits 1 to 6 and the TSV row above *(mechanical)* | AF.1 | 10 | `rg -n 'at [P]\b\|L and [P]' notes/after-firn-comparison.md` prints nothing; `git -C ~/code/nixos-config/main ls-tree -r --name-only $(awk '$1=="C"{print $2}' bench/firn-commits.tsv) native/nix \| wc -l` prints 188; `git -C ~/code/nixos-config/main ls-tree -r --name-only 79becb6a native/nix` prints nothing; `git -C ~/code/nixos-config/main rev-parse 79becb6a^1` equals C |
| AF.1c | Correct C per the final AF.37 ruling *(mechanical)* | AF.1b | 10 | `rg -c 'cbb7c0[6]9' bench/firn-commits.tsv notes/after-firn-comparison.md` prints 1 for each file; `rg '2f05331[d]' notes/after-firn-comparison.md` prints nothing; `git -C ~/code/nixos-config/main rev-parse d07b5b9e^1` equals C |
| AF.2 | Write `bench/sample.tsv`: every source unit of the sample and its host parts at the pin, with id, kind and pin lines | — | 15 | A script prints 0 unassigned and 0 doubly assigned non-blank lines across the sample's pin files, and prints counts per kind |
| AF.2b | Add the five language-neutral rule units to `sample.tsv` and its checker *(mechanical)* | AF.2 | 10 | The checker exits 0 and prints `unassigned: 0`, `doubly assigned: 0` and `rule<TAB>5<TAB>0`; it exits non-zero on a scratch copy with `rule.realization` removed, and on one where `rule.platform-gate` is given `native/tag_resolve.bjs` lines 1 to 10 |
| AF.3 | Run `galileo schema extract --host whiterabbit --flake $FIRN_ROOT` and keep only the options the sample declares or sets | AF.2 | 15 | Every option path that `sample.tsv` sets is present with its type, and no path falls outside the sample's prefixes |
| AF.4 | `tools/compare types`: print the subset as Nickel contracts, CUE definitions, Dhall types and KCL schemas | AF.3 | 20 | Each printed file passes its tool (`nickel typecheck`, `cue vet`, `dhall type`, `kcl run`), and a scratch value of the wrong type fails in all four for one option of each type |
| AF.5 | `bench/contract/shim.nix` and `gate.sh`: in a scratch, detached worktree of nixos-config at the pin, the export replaces the sample modules it names, and `gate.sh` prints whiterabbit's drvPath | AF.2 | 20 | A hand-written export of kanata alone gives the pin's drvPath; with kanata's port set to 7071, it gives another; the scratch worktree is gone after `gate.sh` exits |
| AF.6 | Generate the reference exports for both hosts from the pin's sample Nix, by evaluating each module against that host's evaluated `config`, with `mkIf` decided and `pkgs`, `inputs`, `flakeRoot`, `lib` and `builtins` kept as `$ref` and `$call` values | AF.5 | 20 | `make-reference.sh` writes `bench/contract/out/{whiterabbit,ashashi}.json` and `private.json`, all git-ignored; `gate.sh bench/contract/out/whiterabbit.json`, with all ten modules replaced, prints the pin's drvPath; the ashashi reference's enabled set equals the sample modules in the pin's ashashi `_generated-enables.nix`; `git log -p main..HEAD | grep -qFf <(jq -r '..|strings' bench/contract/out/private.json)` exits 1 |
| AF.7 | `tools/compare size` and `replay` (std only) | AF.2 | 20 | `size --physical` on the Clause and Beagle module files prints 240 lines and on their `tags.clause` files 20, as `wc -l` does; `replay --dry-run` applies two token cards through a scratch `units.tsv`, prints the edited lines and leaves `git status` clean |
| AF.8 | `tools/compare blind` and `scale` | AF.2 | 20 | `blind` on two hand-written answers prints no path, extension or tool name (`rg` finds none of the seven) and seals its letter map; `scale 21` on a stub port writes 210 renamed modules |
| AF.9 | Write `bench/galileo/make.sh`, which runs 4.13's import commands on the pin's sample into an uncommitted `out/` (D-19), with `check.sh` and `units.tsv` | 4.14, AF.2, AF.3 | 20 | `galileo build` of the sample emits the ten modules' Nix byte- or parse-identical to the pin's, as 4.13 requires, plus the sample's lines of both `_generated-enables.nix`; `check.sh` exits 0 |
| AF.10 | Check Clause and Beagle at the pin: a scratch, detached worktree `~/code/nixos-config/worktrees/bench-pin`, with the pin's runtime staged under its own revision; `bench/clause-beagle/check.sh` records the exact commands | AF.2 | 20 | `check.sh` exits 0 on the pin and leaves `git status --short` empty, so the committed `.nix` reproduce; it exits non-zero with `:hardware.uinput.enabl` at `modules/kanata/kanata.bnix:9`, and its exit 0 with `cfg.devics` at `kanata.bnix:20` is recorded as Clause and Beagle's measured miss; it prints its wall time, says whether `firn repo build` is incremental, and prints the commands that create, stage and remove the scratch |
| AF.11–AF.14 | Port part 1, one box each for Nickel, CUE, Dhall and KCL: the five rules, both hosts' selections and settings (the private ones read from `bench/contract/out/private.json`), and firefox, zen-browser, kanata, users and lane-gc, typed by `bench/schema/<lang>/` | AF.4, AF.6 | 20 | For both hosts, the export of those five modules equals `bench/contract/out/<host>.json`'s (`jq -S` shows no difference), and `gate.sh` on that partial export prints the pin's drvPath; `git log -p main..HEAD | grep -qFf <(jq -r '..|strings' bench/contract/out/private.json)` exits 1 |
| AF.15–AF.18 | Port part 2, in the same four lanes: atuin, chrome, godot, bun, obsidian and the responsibility facts; `check.sh`; `units.tsv`; and the tool's nearest "why" command, taken from its documentation | AF.11–AF.14, one each | 20 | For both hosts, the whole export equals `bench/contract/out/<host>.json` (`jq -S` shows no difference); `gate.sh` prints the pin's drvPath; `check.sh` exits 0; `git log -p main..HEAD | grep -qFf <(jq -r '..|strings' bench/contract/out/private.json)` exits 1 |
| AF.19–AF.22 | Replay, in the same four lanes: write the language's structural patches from the cards, then run `tools/compare replay --lang <lang>` | AF.15–AF.18, one each; AF.7, AF.25 | 20 | `bench/results/mistakes.tsv` has one row per kept case for the language, each with exit status, first diagnostic line and wall time; no case is skipped; every `translated` patch changes exactly the places where the column writes what the pin’s edit changes, wherever those places are, and nothing else, and was committed before the column's first checked replay |
| AF.23 | Gather the Build-workflow candidates: the first error line and green commit of each of the 31 red streaks, classified | — | 20 | `bench/corpus/candidates.tsv` has 31 Build rows, each with run URL, error line, fix commit and class; every dropped row names its reason |
| AF.24 | Gather the git-log candidates: the 101 commits, deduplicated against AF.23 and classified | — | 20 | All 101 rows are kept or dropped with a reason, and the kept count is printed |
| AF.25 | Write the case cards: the kept real cases (at most 30, most recent first) and the 40 seeded mutants (seed 4114) | AF.2, AF.23, AF.24 | 20 | `cases.tsv` holds every kept real case and 40 seeded rows, each naming its unit, its class or operator, and its edit; a real case outside the sample names the unit it is transplanted to |
| AF.26–AF.28 | Truth: evaluate each card at the pin in the gate worktree, at most 25 cards per box, two boxes at a time under heavy leases | AF.5, AF.25 | 20 | Every card in the box is labelled `eval-error`, `silent` or `equivalent`, with its drvPath or its error's first line; equivalent cards are marked dropped |
| AF.29 | Replay for Galileo | AF.7, AF.9, AF.25 | 15 | `mistakes.tsv` has Galileo's row for every kept case; every `translated` patch changes exactly the places where the column writes what the pin’s edit changes, wherever those places are, and nothing else, and was committed before the column's first checked replay; after the replay, `bench/galileo/check.sh` exits 0 |
| AF.30–AF.32 | Replay for Clause and Beagle in the pin worktree, a third of the cases per box. If AF.10 measured more than 45 s per case, add boxes so that each box stays under 20 minutes. AF.10 measured about 90 s per case, so the column runs in up to six boxes of at most 12 cases, one after another, because `check.sh` uses the single `bench-pin` scratch: the first box stages the runtime and keeps the scratch, the last removes it. | AF.7, AF.10, AF.25 | 20 | `mistakes.tsv` has the column's row for every case in the box |
| AF.33 | Freeze the key: the ten questions' D and A units from `sample.tsv`, with each selection's commit | AF.2, AF.2b | 15 | `bench/questions/key.tsv` has ten rows whose units all exist in `sample.tsv`, and it is committed before any `answers/` file |
| AF.34 | Collect the answers: run each column's named command for all ten questions | AF.9, AF.10, AF.15–AF.18, AF.33 | 20 | `bench/questions/answers/<column>/Q<n>.txt` holds 70 files, raw output or "no equivalent" |
| AF.35 | Blind and judge: `tools/compare blind`, then the cold Codex session scores each answer 2, 1 or 0 with a one-line reason; unseal and tabulate | AF.8, AF.34 | 20 | `bench/results/explanation.tsv` has 70 scored rows, each column's total out of 20 and the mechanical score beside it |
| AF.36a | Replace the port copier with the shared scaled oracle: `tools/compare scale 21 --out bench/scaled` | AF.8, AF.9, AF.15–AF.18 | 20 | The ignored oracle holds scaled context and private settings, both contracts with 21 times each host's sample module count, and `galileo-expected/` with 210 module directories, 294 module Nix artifacts and three combined host artifacts; only module identities and their option paths change |
| AF.36b | Generate one scaled Galileo program with `bench/galileo/scale.py`; add `check.sh --scaled` | AF.36a | 20 | The program has 210 module directories; `check.sh --scaled` exits 0, both host exports equal the shared contracts, and `diff -r` of emitted Nix against `galileo-expected/` exits 0; only pin-byte and `units.tsv` checks are skipped; `git status --short` lists nothing under `bench/scaled` |
| AF.36c | Generate one scaled Nickel program with `bench/nickel/scale.py`; add `check.sh --scaled` | AF.36a | 20 | The program has 210 module definitions; `check.sh --scaled` exits 0 and both exports equal the shared contracts; only the pin-specific drvPath gate is skipped; `git status --short` lists nothing under `bench/scaled` |
| AF.36d | Generate one scaled CUE program with `bench/cue/scale.py`; add `check.sh --scaled` | AF.36a | 20 | The program has 210 module definitions; `check.sh --scaled` exits 0 and both exports equal the shared contracts; `git status --short` lists nothing under `bench/scaled` |
| AF.36e | Generate one scaled Dhall program with `bench/dhall/scale.py`; add `check.sh --scaled` | AF.36a | 20 | The program has 210 module definitions; `check.sh --scaled` exits 0 and both exports equal the shared contracts; `git status --short` lists nothing under `bench/scaled` |
| AF.36f | Generate one scaled KCL program with `bench/kcl/scale.py`; add `check.sh --scaled` | AF.36a | 20 | The program has 210 module definitions; `check.sh --scaled` exits 0 and both exports equal the shared contracts; `git status --short` lists nothing under `bench/scaled` |
| AF.37 | Time full firn: Clause and Beagle at C in the `bench-c` scratch in one exclusive session, then Galileo at L and the drvPath floor in a second *(mechanical)* | AF.1b, AF.9, AF.10 | 20 | `bench/results/cost/full.json` holds hyperfine's 10 runs for each of the two columns and 3 eval runs, with the medians printed; before timing, one `firn repo build` at C exits 0 and leaves `git status --short` empty; if 12 Clause and Beagle runs would pass 15 minutes, they run as two exports of 1 warmup and 5 runs, both kept; the `bench-c` scratch is gone; after one flipped run, `git status --short` lists only `modules/firefox/default.bnix` and `default.nix` at C, and only `modules/firefox/default.gal` and `default.nix` at L, and `default.nix:7` reads `default = true;` at both |
| AF.38 | Time the sample and the scaled corpus in one exclusive session, for five columns *(mechanical)* | AF.36 | 20 | `bench/results/cost/sample.json` and `scaled.json` hold 10 runs for each of the five columns, with the medians printed |
| AF.39 | Write the Results section here: the four tables, each axis's rank, and the losses with their gaps; write PLAN's outcome line; close galileo#1 | all above | 20 | Every number in the section matches `bench/results/`, and every axis where Galileo does not rank first is listed with its gap |

## Order, staffing and ETA

- **Lanes:**

| Lane | Boxes |
| --- | --- |
| Setup | AF.1–AF.8 (two workers once AF.2 lands: AF.3 then AF.4, and AF.5 then AF.6, with AF.7 and AF.8 after) |
| Galileo | AF.9, AF.29 |
| Clause and Beagle | AF.10, AF.30–AF.32 |
| Nickel, CUE, Dhall, KCL | one lane each, three boxes |
| Corpus | AF.23–AF.28 |
| Explanation | AF.33–AF.35 |
| Cost | AF.36–AF.38 |
| Report | AF.39 |

- **Early starts:** AF.2, AF.10, AF.23, AF.24 and AF.33 read only the pin
  and firn's history, so the lead can start them before 4.14 lands.
  AF.10 should run before 4.19, because 4.19 retires the Clause staging in
  firn's runtime and is one of 4.14's Needs.
- **Critical path:** AF.2, AF.5, AF.6, a language's three port and replay
  boxes, then AF.36, AF.38 and AF.39. That is about 155 minutes of work.
- **Staffing,** following AGENTS.md:
  - **Codex Sol high:** AF.2–AF.22, AF.29–AF.32 and AF.36.
  - **Codex Sol medium:** the mechanical boxes, AF.1, AF.26–AF.28, AF.37
    and AF.38.
  - **Claude Opus medium:** the quality-critical boxes: the corpus triage
    (AF.23–AF.25), the key (AF.33), the judge's setup (AF.35) and the report
    (AF.39).
  - **Landing and review:** Claude Opus lands every box, and a Codex
    reviewer tries to break each box that Claude wrote.
- **ETA:** 39 boxes, about 12.5 worker-hours. With six or seven workers,
  the run takes about 4.5 hours of wall time from 4.14's landing. That
  includes staffing, review, landing and the two 15-minute exclusive timing
  sessions. Starting the five early boxes first brings it to about
  3.5 hours.
- **Risk:** the Clause and Beagle column depends on the pin's toolchain
  still running once 4.19 has landed.

## Addendum: 5. Review (DECISIONS D-64)

Added 2026-10-12 from the triage of ChatGPT's outside review
(`~/.local/state/agents/handoffs/galileo-external-review/triage.md`).
Axes 1 to 4 measure behaviour. This axis measures what Galileo's thesis
claims: a patch that keeps every output can still break what the outputs
were meant to depend on, and review should say so in readable terms.

This axis follows from Galileo's design. A value-level checker can state
"if enabled, then supported", but a patch that keeps every output never
breaks such a statement. So the axis cannot show whether Galileo wins, only
by how much and at what cost. The falsifiable parts are the false alarms on
safe refactors, the lines each column needs, and whether the first
diagnostic names the intent. The report states that limit beside the score.

- **Intents:** `bench/review/intents.md` states four of them in plain words
  before any guarantee is written:
  - a module is enabled on a host only through a tag that the host enables, unless the host's own configuration enables it directly;
  - only on a platform that supports it and, unless the host's own configuration enables it directly, only because that platform supports it;
  - a tag override applies only to a module that its tag enables;
  - a responsibility is realized only by an enabled module.
- **Cards:** `bench/review/cases.tsv` holds nine language-neutral rows on
  the sample's five rules:
  - six breaking cards, at least one per intent, each a believable patch,
    such as a convenience rule that pins a module on and skips the platform
    gate, with pins only on modules already enabled;
  - three control cards: refactors that keep every intent.

  Every card must leave whiterabbit's drvPath at the pin's value, or it is
  replaced. These are the `equivalent` cases that axis 2 drops.
- **Two measurements per column,** following the review's split:
  - **(a) Behaviour only:** replay each card against the column's port from
    axes 1 to 4, unchanged, and count the cards its check or export diff
    rejects.
  - **(b) Behaviour plus guarantees:** the porter writes the four intents in
    the column's own checked form, then replays the cards. The forms are
    Galileo `requires` requirements, Nickel contracts, CUE constraints,
    Dhall assertions, KCL `check` blocks, and Clause's checks in the pin's
    `firn repo validate`. Galileo runs `galileo review --base` (6.7, 6.11,
    3.12b); the other columns run their check and a `jq -S` diff of
    exports.
- **Scoring:**
  - breaking cards rejected, out of 6;
  - false alarms on control cards, out of 3;
  - the guarantee lines each column needed, reported beside axis 1;
  - the share of rejections whose first line names the intent and the
    edited unit.

  A column that cannot state an intent is scored as it ships, and the report
  says which intents it could not state.
  On the sample, I4 binds no host: whiterabbit's responsibility assignment (responsibilities.bnix lines 32 and 34) is outside sample.tsv, so B5 and B6 change no derived fact in any column. The report states this above the review table and scores them as every other card.

| Box | Work | Needs | Min | Done when |
| --- | --- | --- | --- | --- |
| AF.40 | Write `bench/review/intents.md` and `cases.tsv`: the four intents and nine cards, each naming the `rule.*` unit it edits, its edit in words, and its change to the pin's emitted Nix that AF.41 applies (`none` when the emitted Nix is unchanged) | AF.2, AF.2b | 20 | 4 intents and 9 rows (6 breaking, at least one per intent, and 3 control), each unit present in `sample.tsv`; both files are committed before any `guarantees` file exists |
| AF.40b | Apply the exact text above to intents.md and to the note: the I1 and I2 intent lines and sample paragraph, the note's intent bullets and I4 scoring sentence, AF.42's Work and Done when, the Needs cells, and this row after AF.40 | `bench/review/intents.md`, `notes/after-firn-comparison.md` | AF.41 | 10 | `rg -c '^1\. \*\*I1 — Enabled tag:.*unless the host.s own configuration enables it direct[l]y\.$' bench/review/intents.md` and `rg -c '^  - a module is enabled on a host only through a tag that the host enables, unless the host.s own configuration enables it direct[l]y;$' notes/after-firn-comparison.md` each print 1; `rg -c 'only because that platfor[m] supports it' bench/review/intents.md notes/after-firn-comparison.md` prints 1 for each file; `git diff --quiet df50eb9 -- bench/review/cases.tsv bench/review/truth.tsv` exits 0; `fd guarantees bench` prints nothing at the landed commit |
| AF.41 | Truth: apply each card's Nix-level edit to the pin in the gate worktree | AF.5, AF.40 | 20 | All 9 cards give the pin's whiterabbit drvPath, and any card that does not is replaced and re-run; the scratch worktree is gone |
| AF.42 | Galileo: `bench/galileo/guarantees.gal`, the nine cards as derived patches `bench/galileo/review/<id>.diff` to a project-local copy of `lib/firn`, and both replays through `galileo review --base HEAD` in a scratch git repository under `target/tmp` built from `bench/galileo/out`, every `use firn/` becoming `use lib/firn/`, as `thesis_demo.rs` does | AF.9, AF.40b, AF.41, 6.12 | 20 | `bench/results/review.tsv` has Galileo's 18 rows (9 cards × 2 measurements), each with exit status, first line and whether it names the intent; the guarantee lines are counted; the scratch base's `galileo build` output equals `bench/galileo/out`'s (`diff -r` exits 0); each diff touches only the `lib/firn` file `units.tsv` maps its unit to; the scratch repository is gone |
| AF.43 | Clause and Beagle in the pin worktree: the intents that Clause's checks can state, then both replays | AF.10, AF.40b, AF.41 | 20 | 18 rows in `review.tsv`; every intent Clause cannot state is listed with the reason |
| AF.44–AF.47 | One box per outside language, Nickel, CUE, Dhall and KCL: guarantees, nine structural patches and both replays | AF.15–AF.18, one each; AF.40b, AF.41 | 20 | 18 rows per language in `review.tsv`; with the guarantees, `check.sh` still exits 0 on the unpatched sample |
| AF.48 | Add the review table to Results: rank by breaking cards rejected under (b), ties broken by fewer false alarms, then fewer lines; then the effort table below: (R) and (I) at both sizes, with the three registered losses | AF.42–AF.47, AF.49, AF.50 | 20 | Every number matches `review.tsv` and `review-effort.tsv`; this axis's limit and the effort limits are stated above their tables; every intent a column could not state is listed; each registered loss is marked tripped, not tripped, or not run, the third until the benchmark reports |

AF.48 prints the I4 sentence above the review table and also states that Galileo's rules are reviewed as a project-local copy of `lib/firn`, since review refuses edits to the built-in library (D-73).

- **Order:** AF.40 starts early, since it reads only the pin, with AF.2,
  AF.10, AF.23, AF.24 and AF.33. AF.42 waits for 6.12, which finishes
  `galileo review`. AF.49 follows AF.42, beside AF.43 to AF.47, and AF.50
  follows AF.49 and AF.36. The axis adds 11 boxes, about 3.7 worker-hours,
  and about an hour of wall time after AF.42. AF.39 waits for AF.48.
  AF.49 and AF.50 are quality-critical, as AF.35's judge setup is, so
  Claude Opus medium takes them.

### Review effort: the bet's test (AF.49, AF.50)

Added 2026-10-12 from the triage of ChatGPT's best-case text
(`~/.local/state/agents/handoffs/galileo-external-review/best-case-triage.md`).
That text names Galileo's whole bet: reviewing what a change must keep true
is "dramatically easier" than reviewing its code, and the software is not
much harder to build. AF.40 to AF.48 score what each column's check
rejects. None of them measures what a reviewer must read to decide, or
whether a reviewer who reads only the requirements and review's output
decides right. These two boxes measure that inside Galileo, on axis 5's
cards.

- **Two packets per card:**
  - **(I) implementation review:** the four intents in words and the
    unified diff (`diff -ru`) of the card's base and patched Galileo
    sources;
  - **(R) requirement review:** the four intents in words,
    `bench/galileo/guarantees.gal`, and `galileo review`'s whole output on
    the card, with no source diff.

  Neither packet carries the card's words or an author's description. One
  cold reviewer session reads each packet, set up as AF.35's judge is, with
  no repository access, and answers approve or reject, naming the intent a
  rejected card breaks.
- **Two sizes:** the nine cards on the sample (AF.49), and three of them on
  AF.36's 210-module corpus, each joined to one sweep that edits every clone
  and keeps every output (AF.50). The bet is about the second size: a diff
  grows with the change, while the requirements and the verdict should not.
- **Losses, registered before any packet is built.** The bet fails on firn
  if any of these holds:
  1. on the sample, (R) gets fewer cards right than (I);
  2. at 210 modules, (R) gets fewer cards right than (I), or its median
     reviewer tokens are not below (I)'s;
  3. Galileo misses T5's principle 17 bar, on behaviour alone, in the
     authoring benchmark: the bet's "not much harder to build" half.

  Reported beside them with no line: each condition's median tokens, bytes
  and wall time at both sizes, and its growth from the sample to 210
  modules; Galileo's guarantee lines (AF.42) and the benchmark's second
  number. A line for "dramatically" waits for one run, as Tom ruled for the
  second number (principle 17 addendum).
- **Limits, stated above the effort table:**
  - agent reviewers stand in for people, as agents already review agents'
    work in Tom's organization;
  - (R) sees only what the requirements state, so a patch that breaks an
    unstated intent is approved by design (DECISIONS D-50); every card here
    breaks a stated one;
  - firn has one state, so each requirement is decided completely, while on
    north's menu a universal claim answers Unknown today (DECISIONS T6). This
    is the bet's most favourable case;
  - this is the within-language half. The cross-language half, reviewing
    TypeScript against reviewing Galileo, is Dim 7 below, which the medium
    benchmark has not adopted;
  - one run per packet, on 9 and 3 cards.

| Box | Work | Needs | Min | Done when |
| --- | --- | --- | --- | --- |
| AF.49 | Review effort on the sample: `tools/compare packets` (std only) writes each card's (I) and (R) packets into `bench/review/effort/` from AF.42's base and patched sources and its `galileo review` outputs; one cold reviewer session per packet; `bench/results/review-effort.tsv` gets one row per packet with card, condition, size, lines, bytes, verdict, the intent named, right or wrong, and the session's tokens and wall time | AF.42 | 20 | 18 packets exist, and `rg -c '^@@'` finds no diff hunk in any (R) packet and at least one in every (I) packet; `review-effort.tsv` has 18 rows with every field filled; the right verdicts per condition, out of 9, and the median tokens and bytes per condition are printed, with loss 1 printed as tripped or not |
| AF.50 | Review effort at 210 modules: `bench/review/scale.md`, written before any patch, names three AF.40 cards (two breaking, taken from B1 to B4, and one control) and one sweep over every clone of AF.36's Galileo corpus that keeps every output and that `galileo review` lists as edits, not a reformat that SPEC §8.1's normalization drops; each card joined to the sweep is one large patch, kept as base and patched sources; AF.49's packets and reviewers then run on the three | AF.36, AF.49 | 20 | Each large patch leaves the corpus's `galileo build` output byte-identical (`diff -r` exits 0), and its (I) packet holds at least 210 diff hunks; `galileo review` takes under 5 s on each, measured once; `review-effort.tsv` gains 6 rows; the right verdicts per condition, out of 3, the median tokens per condition and each condition's growth from AF.49's medians are printed, with loss 2 printed as tripped or not |

### For the authoring benchmark (T5's principle 17)

The review proposes judging Galileo over a full change cycle: build, an
unexpected change, then review. The medium benchmark
(`~/.local/state/agents/muove-bench/MEDIUM-PLAN.md`, owned by its lead)
measures the first two (Dim 4, and Dim 2 "breakage from change"), but not
the third. It also measures behaviour only. The recommendations for its
lead:

1. Keep the pre-registered pass line, 0.67 of TypeScript's time-to-correct,
   on behaviour alone.
2. Report a second number beside it, behaviour plus guarantees: after a
   Dim 4 cell passes, the agent is given one intent in words and must make
   its program reject a held-out patch that keeps every visible answer and
   breaks that intent. Time-to-correct covers both stages.
3. Add Dim 7, review: for each of 5 held-out tasks, 2 patches (one breaking
   an intent while keeping the visible answers, one safe) in 3 languages,
   so 30 cells. The agent answers "approve" or "reject" with its reason,
   and Galileo agents may run `galileo review`. Measure accuracy, time, and
   whether the reason names the intent.

Tom answered (2026-10-12): the pass line stays on behaviour alone, and the
second number is reported beside it with no pass line until one measured
run exists (DECISIONS, principle 17 addendum).
