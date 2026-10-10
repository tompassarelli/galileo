# Muove language notebook

The language owner's working memory (DECISIONS D-36). SPEC.md states each
rule and DECISIONS.md says why it was chosen. This file holds what the
owner needs between sessions: how rulings are made, the principles they
share, where each language ruling lives, the contradictions found and their
status, the open questions, and Tom's own exercises. A new owner reads it
first.

## How a ruling is made

- **One owner at a time.** The Muove lead keeps one Opus max planner,
  `muove-language-owner`, and sends it each tick's language `DECISION`
  lines with SendMessage. It rules on what a Muove program means and how it
  is spelled: lexical rules, both registers, names and `use`, types and
  stages, rules, aggregates and queries, claims and verdicts, steps and
  effects, the Core IR, diagnostics and `muove fmt`. Nix layout for byte
  identity, migration, crate boundaries and process go to one-shot
  planners. A question that is both goes to the owner.
- **Its memory is this file.** When the owner's context passes 350k tokens,
  or the lead restarts, the lead starts a new owner from this file, SPEC §2
  to §9 and the DECISIONS entries indexed below. Nothing the owner knows
  only in its context binds anyone.
- **Before ruling,** it checks the answer against the compass, the index
  and the corpus the answer touches: firn at `FIRN_ROOT`, north at
  `NORTH_ROOT`, and SPEC's examples.
- **A ruling binds once SPEC states it.** One commit carries the SPEC text,
  one DECISIONS entry per batch in D-35's form, and this file's index and
  contradiction lines. The owner then posts `RULED <box>: …` in the peer
  file and names each box whose landed code must change, for the lead to
  staff.
- **Workers don't wait.** A worker writes `DECISION <box>: …`, takes the
  narrowest reading SPEC allows, says so in that line, and lands. Its box
  stays unticked only when its Done when depends on the answer.

## Compass

The principles the rulings so far share. A ruling that breaks one says why.

1. Every line has one reading, and an import never changes it (G5, R16).
2. Every value has one spelling (L1, D-35's operators).
3. Strict until a program needs more: allowing more later breaks no
   program, and allowing less would (G1, D-34, D-35).
4. The corpus decides. Every Beagle module must still emit byte-identically
   and every Clause module parse-identically (D-8), so a rule that cannot
   write one of them is wrong (K7).
5. One meaning in one place: the checker, the reference interpreter and the
   engine share one scope function (L3) and one semantics (G7).
6. Muove keeps adding keywords (T4), so where a keyword and a template word
   can meet is decided once, for every keyword (O1).
7. A diagnostic is a sentence with file:line that names the offending
   literal, and a closest-shape list contains the fix (G6, P3).

## Where the language rulings are

| Area | Rulings | SPEC |
| --- | --- | --- |
| Lexical | D-34 backticks; D-35 reserved characters and a leading `-`; T4 `1.5` reserved and spaced minus; L1 | §9.1, §9.4 |
| Record register | D-25 `nix file`; D-35 records, precedence, `where`, field access and types; L2 | §9.4, §10.2 |
| Sentence register | D-10 declared templates and two registers; D-11 scoped vocabularies and explicit quantifiers | §9.2, §9.3 |
| Names, scope and `use` | D-11; L3 | §2, §4.3 |
| Values, types and stages | D-7 two stages; D-12 `Int` only | §3 |
| Rules, aggregates and queries | L3, L4, L5 | §4.3 to §4.5 |
| Claims and verdicts | D-4, D-5, D-6, D-26, D-29, D-30 | §6, §7 |
| Steps and effects | D-13, D-14, D-15, D-31 | §5 |

**Not yet in SPEC.** An Opus max planner ruled these at 19:05 on
2026-10-10 (`~/.local/state/agents/handoffs/muove-semantics-rulings.md`).
They reached worker briefs but never SPEC or DECISIONS, and the owner's
first batch writes them in.

- **L1 (box 2.1).** A text block's closing `'''` sits alone on its line,
  exactly 2 spaces deeper than the line that opens the block, and content
  is dedented to it.
- **L2 (boxes 1.4, 2.6).** Muove's `let` is sequential and non-recursive,
  and lowers to nested `Apply(Lambda)` in source order. A Nix `let … in`
  stays recursive and is never rewritten as lambdas. Box 4.15's idiomatic
  printer refuses a binding that would capture its own name.
- **L3 (box 3.2).** One scope function. A rule, a handler clause, a claim
  or a view owns its variables, and a definition sees its callers only
  through its parameters. Each aggregate, argmin and query opens a scope. A
  name that also appears at the top level of the owner, or of any scope
  enclosing it, is outside and shared, and must be grounded first. A
  declared variable (`all ?x`, `the ?x`, `first ?x`) may not reuse an
  outside name. A query returns only its declared variables, and its sort
  key reads only declared and outside variables.
- **L4 (box 3.2).** In a recursive rule, `?y = ?x` gives `?y` the safety of
  `?x`. Anything else computed from a variable that only the head's own
  component grounds is value invention.
- **L5 (box 3.2).** A stratum is one strongly connected group of relations.
  Strata are numbered by longest path, and a `not`, aggregate, argmin or
  query inside one stratum is an error that prints the cycle.

## Contradictions

The first pass, on 2026-10-10, read D-1 to D-35, T1 to T4, L1 to L5, the
DECISION lines in the peer file and the landed code they name. Each entry
stays open until its fix lands.

- **K1. Sort keys.** L3 lets a sort key read only declared and outside
  variables. Box 3.4 (`5662557`) sorts by a body-local variable, its test
  expects that (REVIEWED 3.4), and DECISION 3.4 asked the question again.
  The reference planner holds it. Under G1, L3 stands unless a program
  needs more, and SPEC's one sorted query sorts by `?row.order`, a declared
  variable.
- **K2. Declared variables that clash.** L3 rejects `all ?x` when `?x` is
  outside, and box 2.11 (`cc356d6`) accepted it. Fixed in `afd89d1`.
- **K3. `?x is a T` in a rule body.** SPEC §9.7's `menu-count` grounds
  `?north` with `?north is a North`, and L3 cites it, but §4.3 lists no such
  literal and 2.11 lowers it to a diagnostic. Measured at `f2802f2`: once
  one claim says `every ?h where ?h is a Hero: …`, `muove check` answers
  every claim in the file `Unknown`, citing "the built-in entities is not
  supported yet". The sentence planner and the 2.11 planner each hold half.
  The likely answer is a positive literal over `T`'s entities, the declared
  ones for a static type and the live ones for a `state entity`, which
  grounds `?x` and joins §4.3's list.
- **K4. Two used modules that declare one type name.** Box 2.10
  (`35c9f83`) lets the first `use` keep the bare name, so prepending
  `use story` re-points an existing `Ash is a Hero`. That breaks G5 and §2.
  The declarations planner holds it, and G5 gives the answer the DECISION
  line offers as its alternative: the bare name is an error that names
  both modules.
- **K5. View parameters.** SPEC §9.7's view writes its parameter bare
  (`north has menu kind ?k`), while every other variable in a sentence is
  `?name` (§9.1). Box 2.12 (`51b31fa`) reads a bare name equal to a
  parameter as the parameter, so an entity of that name cannot be named in
  the view. No planner holds it. Pick one spelling for a variable in a
  sentence.
- **K6. A bracketed list in a `nix` slot.** §4.2 makes `[a, b]` in a slot
  sugar for one fact per element, and §3.2 stores `[pkgs.firefox]` in a
  `nix T` slot as one payload. Box 2.8 (`ba4bd21`) expands it whatever the
  slot's type. The sentence planner holds it. The likely answer is that the
  slot's declared type decides, and the sugar applies only where that type
  is neither a list nor `nix`.
- **K7. `option` lines the grammar cannot write.** D-35 gives
  `option-type = "enable" text-literal`, and box 2.3c (`a82e0c6`) enforces
  it, yet five firn modules pass a `let` name to `lib.mkEnableOption`:
  capacity-watchdog, lane-gc, proton-log-watchdog, skill-review-queue and
  vast-reaper. All five must emit byte-identically (A3). Measured at
  `a82e0c6`: `option myConfig.modules.lane-gc.enable: enable description`
  fails with "`enable` needs a description text". SPEC §10.2 also has the
  importer write an explicit `lib.mkOption({…})` call, which no `option`
  form accepts. Nothing needs either before box 4.1; settle both together.
- **K8. Ordering comparisons on `Text`.** §4.3 allows `< <= > >=` on `Int`
  and `Text`, and box 2.5 (`532eff9`) accepts `Int` only. The stages
  planner holds it.
- **K9. A `the` query with two answers.** §4.4 makes a broken mode `Fails`
  with both proofs, and box 3.4 returns `EvalError::Type`. The reference
  planner holds it.
- **K10. Template words that are keywords.** `lib/firn/responsibilities.muove`
  declares `{responsibility} requires capability {capability}`, while
  §9.4's claims include `conj "requires" conj`. Measured at `f2802f2`:
  `backup requires capability gpu` reads as a fact at the top level but
  fails inside a `requirements` block, and the closest-shape list offers
  the very template it should have matched. D-35 says a template may use
  any word, yet `and` separates a condition's literals (D-35), and 2.9 lets
  `?x in` win over a template that starts `{x} in`. Box 5.7 writes the
  boot-coupling requirements over `requires capability`. See O1.
- **K11 (not a language question).** D-20 says `rnix` parses Nix "in dev
  tools and tests", while D-27 and box 4.16 put an rnix importer, `muove
  import nix`, in the shipped binary. D-20's rule, a dependency that
  replaces an external format, allows it in `muove-import`, so its wording
  is stale. For the planner of DECISION 4.6/4.16.

## Open questions

- **O1. Keywords against templates.** Decide where each keyword is read,
  and check a shape's template against that list when the shape is
  declared, so a collision is one error at the declaration rather than a
  misreading at each use. Start from `and`, `or`, `not`, `in`, `where`,
  `is a`, `requires`, `passes`, `happens when`, `has no single point of
  failure among`, and 2.10's line-initial `why`, `trace`, `prevent` and
  `achieve`.
- **O2. What `count` counts.** `count ?x where …` ranges over the distinct
  bindings of every local variable of its body (§4.3, L3), not over
  distinct `?x`. Tom's exercise B is the first reader to check.
- **O3. `for` blocks as scope owners.** L3 names rules, handler clauses,
  claims and views, but SPEC §9.6's generated enables reads the outer
  `for`'s `?module` inside its inner query. Box 5.2 needs the answer.
- **O4. `muove check` on a program that does not compile.** Box 3.11 exits
  2, the code that means `Unknown`.
- **O5. Questions in flight.** Every language `DECISION` line in the peer
  file that no landed ruling answers. A planner that stops without landing
  its ruling leaves its questions here.

## Tom's exercises

Tom writes Muove himself, so that its feel is tested before box 4.14 makes
`.muove` firn's source (PLAN T.1 and T.2; 4.14 waits for both). Each takes
about 15 minutes with no agent help while he writes. The lead hands him a
`muove` built from main and an empty directory, and afterwards writes his
notes below, in his words.

### A. A firn module in the record register (T.1)

1. Write `default.muove` for nixos-config's `modules/update-notify` from
   its 13-line `default.bnix` at `FIRN_ROOT`, with SPEC §9.5's kanata
   example as the only reference. Leave out `:tags-opt-in`, which becomes a
   fact once `firn/tags` exists (box 5.1) and never reaches the Nix.
2. The module reads options that `modules/users` declares, so the lead puts
   this stub beside it as `users.muove`:

   ```text
   nix module
     arguments: config, lib, pkgs, ...
     option myConfig.modules.users.username: text
     option myConfig.modules.users.homeDir: text
   ```

3. Run `muove build .`, then
   `diff default.nix $FIRN_ROOT/modules/update-notify/default.nix`. A
   version written to check this exercise builds byte-identically at
   `a82e0c6`, so a difference is either Tom's choice or a gap to name.
   (lane-gc was the first choice, and K7 now stops it at its `enable`
   line.)

Watch for:

- the first thing Tom typed that failed, and whether the message told him
  the fix;
- where he reached for Nix or Beagle: `=`, `;`, `${…}`, `(s …)`, `inherit`;
- whether `let`, `config when`, `nix module(config, ...)`, the
  `{username}` and `"update-notify@"` keys and `"{homeDir}"`
  interpolation read as he expected;
- whether the Nix matched, and if not, whose choice the difference was;
- in one sentence, whether he would rather write this than the `.bnix`.

### B. A small program in the sentence register (T.2)

`muove check` reads source paths since box 5.4 (`f2802f2`).

1. Read `lib/firn/responsibilities.muove` (44 lines), and say in one
   sentence each what `rule dependent-responsibility` and its requirement
   mean. A wrong paraphrase is a finding.
2. In a new `machines.muove`, write about something Tom knows, such as his
   machines and the agents and services each one runs: two or three types,
   three or four shapes, six to ten facts, one rule, and two requirements,
   one he expects to hold and one he expects to fail. Run
   `muove check machines.muove` after each change. Once box 5.3 lands,
   `muove why` shows how a verdict came about.
3. For each error or surprising verdict, note what he wrote, what he meant
   and what Muove said.

Watch for:

- lines Muove would not read, and whether the closest shapes showed the
  fix;
- words he reached for that the register lacks: articles, plurals,
  `isn't`, an `and` inside one sentence;
- a template word that Muove also reads as a keyword (K10): `requires`,
  `is a`, `in`, `and`, `where`;
- a verdict that surprised him, which means a line read differently from
  what he meant (O2);
- whether `?variables` and `every ?x where …:` read naturally;
- in one sentence, whether the sentences say what he meant.

### Tom's notes

None yet.
