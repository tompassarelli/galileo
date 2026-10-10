# Muove language notebook

The language owner's working memory (DECISIONS D-36). SPEC.md states each
rule and DECISIONS.md says why it was chosen. This file holds what the
owner needs between sessions: how rulings are made, the principles they
share, where each language ruling lives, the contradictions found and their
status, the open questions, and who writes Muove. A new owner reads it
first.

## The rubric: Tom's direction (T5)

Every ruling is judged against T5 (DECISIONS T5 has Tom's words). Tom
approves direction, not syntax, and is never asked about syntax.

1. One uniform relational term that composes, turtles all the way down
   (Beagle store's term design); minimize special cases.
2. Every rule, keyword and symbol is deeply earned: it makes code faster to
   scan, faster for an agent to understand, and faster for an agent and a
   human to share understanding. Otherwise cut it.
3. Relational structure stays visible; Lisp's uniform calls throw it away.
4. A precise subset of English, with mathematical terms where they earn it:
   neither bespoke half-English nor a wall of symbols.
5. The best version of Clause, without its drift into correctness rabbit
   holes.
6. Types and semantics follow from a few principles, strong enough for
   physics and games (T4).
7. The design is mechanical from here: derive it from Clause, Beagle store
   and these principles.
8. A word earns its place when its meaning is hard to hallucinate: one
   established, precise meaning from English, logic, mathematics or
   philosophy that nobody can plausibly misread. Prefer such a term to a
   coined keyword or drifting English.
9. Reuse the priors models already carry: English, and the notation of
   philosophy, mathematics and law, which is exact about ownership,
   provenance and relationships.
10. Code needs no comments. If a reader still needs a comment to know what a
    construct means, the construct is wrong.
11. The guiding star: Lean's one checked meaning per statement, Inform 7's
    readability, Datalog's relational core, and domain vocabularies a
    program imports, with no proof burden on the writer.
12. Succeed or fail early: zero-shot, from the card alone, agents write the
    target class of programs at least 2x faster than in Python (goal 3–5x)
    and pass first try at least as often, measured by the authoring
    benchmark, whose failure transcripts feed the audits.
13. Done means that, plus Tom finding the Muove version of one program more
    readable than the Python version side by side.
14. Contracts, not types: what must hold of values, relations and behaviour
    is a plain checked statement, one mechanism with requirements, claims
    and verdicts, not a type hierarchy.
15. Immutable and declarative: a program states facts, rules and
    constraints, never steps. A step is one atomic transition stated as a
    relation between before and after; a sequenced step, an assignment or a
    loop has a declarative reading or goes.
16. Take the best ideas of projects Tom agrees with, Clause and Beagle
    first, from their design notes and decision records. Direction
    questions, above all mutability against immutability, go to the lead as
    the tension, its two best options and a recommendation, never syntax.

## Where the work stands

Checkpoint of the second owner (xhigh), 2026-10-10. D-40, D-41 and D-42
landed its predecessor's two batches and the T5 audit, and the peer file
has their `RULED` lines.

**Done: the T5 audit (D-42).** §9.4 went from 48 productions, 89 word
terminals and 25 symbols to 46, 72 and 25. Handler clauses state the
post-state under `then`; `static` is `given`, `checkpoint` is `milestone`,
`first ?x by` is `min`/`max ?x by`; `only after` and `only when` replaced
the trace and event claims; option types are the value types; K7, brace
defaults and O1/K10 are ruled. Boxes 2.19a to 2.19g carry it, 7.1 proceeds
on the IR, and 4.1, 4.3b, 4.5, 4.36 and 8.6 now need them. 71 of firn's 214
comment lines are made redundant; the 143 left say why. The re-count is
§9.4's quoted terminals split into words and symbols, with productions
named as `help_syntax.rs` names them.

**Next, in this order,** from the planners' files in
`~/.local/state/agents/handoffs/muove-rulings-*.md`:

- ~~**stages** Q1 to Q6~~: done in D-45 (2.5b, 2.5c, 2.5d; K8);
- ~~**sentence-syntax** Q1, Q2, Q4 to Q6~~: done in D-46 (2.8b, 2.8c,
  2.9c, 4.11b; K6; DECISION 4.11's list slot);
- ~~**declarations** Q5~~: done in D-43. A sentence replaces only along a
  relation's one mode; with two it adds and step 5 checks both (2.19d
  lowers it to `Include`). Clause variables, at least one `then` line and
  `may skip` per event are box 2.19h;
- **IR-lowering** Q1 (2.12c, 3.18: fields as functional state relations,
  `?x.f`), Q3's typed view fields (2.12e) and requirement names (the claim
  as `muove fmt` prints it; review pairs requirements by lowered claim; a
  6.1 identity follow-up); **printer-lowering** Q4 (2.12b) is answered in
  spelling by D-42's `?x is a new T {field: value, …}`, so rule only its
  lowering;
- **claims** Q1 to Q5, boxes B to F: exit 2 on a static error (O4), local
  definitions in claims, one `claims::unbound` for which forms bind which
  variables (now with `only after` and `only when`), and one model per
  claim. Box A landed as the 3.12 fix (`034d379`);
- **explanation** Q2 (3.13b), Q3 (§6.4; DECISION 3.14 twice) and Q4
  (§11.1's `fmt` row); Q5 (2.14b) is the lead's;
- L1 into §9.1;
- `muove prevent` and `muove achieve` have no command box: each takes a
  claim and `--using` (SPEC §7.4), while `supports.rs` takes one atom;
- REVIEWED 2.12's open scope question: `define loop(n: Int): Int` with
  body `loop(n)` lowers as `EffectLevel::Total` though §3.5 forbids
  recursion, so a checker box should reject it;
- the peer file's unruled `DECISION` lines: 2.6, 2.6-fix, 2.11-fix, 6.1
  (two), 6.8, 2.15 (two), 3.14, 2.13 and 2.16, and confirmations of 3.9,
  3.13, 2.12 (six) and 5.4. D-39 answered 4.6, 4.16 and 8.10; D-42
  answered 4.11;
- the zero-shot benchmark's failure transcripts, when they arrive: each
  misread or tooling failure names a construct for the next audit.

**Direction questions.** None is open. D-42 makes effects a set; ask Tom,
through the lead, only when a program needs effects in source order, such
as printed lines, with the set against an ordered list as the two options.

## How a ruling is made

- **One owner at a time.** The Muove lead keeps one Opus xhigh planner (max
  only when Tom asks by name), `muove-language-owner`, and sends it each
  tick's language `DECISION` lines with SendMessage. It rules on what a
  Muove program means and how it is spelled: lexical rules, both registers,
  names and `use`, types and stages, rules, aggregates and queries, claims
  and verdicts, steps and effects, the Core IR, diagnostics and `muove fmt`.
  Nix layout for byte identity, migration, crate boundaries and process go
  to one-shot planners. A question that is both goes to the owner.
- **Its memory is this file.** When the owner's context passes 350k tokens,
  or the lead restarts, the lead starts a new owner from this file, SPEC §2
  to §9 and the DECISIONS entries indexed below. Nothing the owner knows
  only in its context binds anyone.
- **Before ruling,** it checks the answer against the compass, the index and
  the corpus the answer touches: firn at `FIRN_ROOT`, north at `NORTH_ROOT`,
  and SPEC's examples.
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
8. Source order is never observable: clauses, `then` lines and effects
   have no order, and what needs one carries it as a value (D-42).
9. A word stays only if no reader can misread it and removing it would
   lose a meaning (T5's principles 2 and 8; Clause's deletion test).

## Where the language rulings are

| Area | Rulings | SPEC |
| --- | --- | --- |
| Lexical | D-34 backticks; D-35 reserved characters and a leading `-`; T4 `1.5` reserved and spaced minus; L1; D-42 keywords never shadow templates (O1) | §9.1, §9.2, §9.4 |
| Record register | D-25 `nix file`; D-35 records, precedence, `where`, field access and types; L2; D-44 a default never names a formal | §9.4, §10.2 |
| Sentence register | D-10 declared templates and two registers; D-11 scoped vocabularies and explicit quantifiers; D-46 slot tokens, list slots by type, literal kinds as readings, one literal per `when` line | §4.2, §4.3, §9.2, §9.3, §9.4 |
| Names, scope and `use` | D-11; D-37 option tree, sequential `let`, prelude and the Nix target's names; L3; D-41 linked modules, project root, library, vocabulary scope and exports, questions as command arguments, view and clause scopes | §2, §3.2, §4.3, §4.5, §5.2, §9.4 |
| Values, types and stages | D-7 two stages; D-12 `Int` only; D-41 entity relations (`X is a T`), one name for entities of several types; D-42 types are contracts, one type grammar for options, `given`, `state entity` only; D-45 nix-lib signatures, unsigned calls, fact roots, narrowing, `for` sources, `Text` ordering | §3, §4.1, §4.2, §9.4, §10.2, §10.3 |
| Rules, aggregates and queries | D-40 scopes (L3 to L5), value-free `count`, record rows, sort keys, `min`/`max` types, `the`; D-41 `for` as an enclosing scope; D-42 `min`/`max ?x by` | §4.3 to §4.5, §9.4 |
| Claims and verdicts | D-4, D-5, D-6, D-26, D-29, D-30; D-37 work unit; D-40 mode checks per stratum; D-41 one sentence a side of `requires`, absence scope by kind, `acceptable` minimality; D-42 `only after`, `only when`, `milestone`; D-44 their left variables bound once around the claim | §4.4, §4.6, §6, §7 |
| Steps and effects | D-13, D-14, D-15, D-31; D-41 a clause is its own scope; D-42 `then` states the post-state, replacing sentences, `E` a set, canonical creation ids; D-43 a sentence replaces along one mode only, clause variables, `then` non-empty, `may skip` per event | §5 |
| Evaluators | D-16; D-40 one expression evaluator and meter, differential comparison | §10.1 |

**L1 to L5.** An Opus max planner ruled these at 19:05 on 2026-10-10
(`~/.local/state/agents/handoffs/muove-semantics-rulings.md`). They reached
worker briefs but never SPEC or DECISIONS. L2 is in SPEC §2 and §10.2 since
D-37, and L3 to L5 in §4.3 since D-40; L1 is not yet in SPEC. (`0a5bd23`
landed D-40 as D-38, a number 1.7's printer entry, `cc1dc93`, also took; it
became D-40 after WP4's D-39, `f50ab7d`, and peer lines citing D-38 for
scopes, counts or modes mean D-40.)

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
  Ruled in D-40: L3 stands (SPEC §4.5). Fixed in `cc66e30` (3.4c).
- **K2. Declared variables that clash.** L3 rejects `all ?x` when `?x` is
  outside, and box 2.11 (`cc356d6`) accepted it. Fixed in `afd89d1`.
- **K3. `?x is a T` in a rule body.** SPEC §9.7's `menu-count` grounds
  `?north` with `?north is a North`, and L3 cites it, but §4.3 lists no such
  literal and 2.11 lowers it to a diagnostic. Measured at `f2802f2`: once
  one claim says `every ?h where ?h is a Hero: …`, `muove check` answers
  every claim in the file `Unknown`, citing "the built-in entities is not
  supported yet". The sentence planner and the 2.11 planner each hold half.
  Ruled in D-41: `X is a T` is the sentence of a built-in relation
  `is-a-T`, whose facts are the declarations and every entity a fact
  names in a `T` slot, and for a `state entity` the live entities (SPEC
  §4.1 to §4.3). Open until box 2.11e lands.
- **K4. Two used modules that declare one type name.** Box 2.10
  (`35c9f83`) lets the first `use` keep the bare name, so prepending
  `use story` re-points an existing `Ash is a Hero`. That breaks G5 and §2.
  The declarations planner holds it, and G5 gives the answer the DECISION
  line offers as its alternative: the bare name is an error that names
  both modules. Ruled in D-41, with no entity export and `use` not
  transitive (SPEC §2). Open until boxes 2.10c and 2.10d land.
- **K5. View parameters.** SPEC §9.7's view writes its parameter bare
  (`north has menu kind ?k`), while every other variable in a sentence is
  `?name` (§9.1). Box 2.12 (`51b31fa`) reads a bare name equal to a
  parameter as the parameter, so an entity of that name cannot be named in
  the view. Ruled in D-41: a view's slot is `?north` in its body, and a
  `define`'s parameters stay bare names (SPEC §4.5, §9.7). Open until box
  2.12d lands.
- **K6. A bracketed list in a `nix` slot.** §4.2 makes `[a, b]` in a slot
  sugar for one fact per element, and §3.2 stores `[pkgs.firefox]` in a
  `nix T` slot as one payload. Box 2.8 (`ba4bd21`) expands it whatever the
  slot's type. The sentence planner holds it. The likely answer is that the
  slot's declared type decides, and the sugar applies only where that type
  is neither a list nor `nix`. Ruled in D-46 as that answer, with `List T`
  slots. Open until box 2.8c lands.
- **K7. `option` lines the grammar cannot write.** D-35 gives
  `option-type = "enable" text-literal`, and box 2.3c (`a82e0c6`) enforces
  it, yet five firn modules pass a `let` name to `lib.mkEnableOption`:
  capacity-watchdog, lane-gc, proton-log-watchdog, skill-review-queue and
  vast-reaper. All five must emit byte-identically (A3). Measured at
  `a82e0c6`: `option myConfig.modules.lane-gc.enable: enable description`
  fails with "`enable` needs a description text". SPEC §10.2 also has the
  importer write an explicit `lib.mkOption({…})` call, which no `option`
  form accepts. Nothing needs either before box 4.1; settle both together.
  Ruled in D-42: `enable` takes any `Text` expression, and the explicit
  call goes. Open until box 2.19e lands.
- **K8. Ordering comparisons on `Text`.** §4.3 allows `< <= > >=` on `Int`
  and `Text`, and box 2.5 (`532eff9`) accepts `Int` only. The stages
  planner holds it. Ruled in D-45: two `Int`s or two `Text`s. Open until
  box 2.5c lands.
- **K9. A `the` query with two answers.** §4.4 makes a broken mode `Fails`
  with both proofs, and box 3.4 returns `EvalError::Type`. Ruled in D-40:
  modes are checked as each relation completes and evaluation stops at the
  first stratum that fails, so a checked program never reaches the guard;
  `the` type-checks only when modes fix its variable (SPEC §4.4, §4.5).
  Open until box 3.17 lands; 2.5e landed in `caad196`.
- **K10. Template words that are keywords.** `lib/firn/responsibilities.muove`
  declares `{responsibility} requires capability {capability}`, while
  §9.4's claims include `conj "requires" conj`. Measured at `f2802f2`:
  `backup requires capability gpu` reads as a fact at the top level but
  fails inside a `requirements` block, and the closest-shape list offers
  the very template it should have matched. D-35 says a template may use
  any word, yet `and` separates a condition's literals (D-35), and 2.9 lets
  `?x in` win over a template that starts `{x} in`. Box 5.7 writes the
  boot-coupling requirements over `requires capability`. See O1. Ruled in
  D-42 with O1: every reading is tried and exactly one must remain. Open
  until box 2.19g lands.
- **K11 (not a language question).** D-20 says `rnix` parses Nix "in dev
  tools and tests", while D-27 and box 4.16 put an rnix importer, `muove
  import nix`, in the shipped binary. D-20's rule, a dependency that
  replaces an external format, allows it in `muove-import`, so its wording
  is stale. For the planner of DECISION 4.6/4.16. Resolved by D-39, which
  corrects D-20's wording.
- **K12. The CLI reads one module.** `lower_files` joins every path that
  `muove check` gets into one module, so a file with a `use` fails ("there
  is no module `firn/tags`"), and an unrelated file's template breaks
  another file's line (R16; measured at `f2802f2`). Ruled in D-41: modules
  are linked under `module/name`, named from the project root, and the
  library is embedded (SPEC §2). Open until boxes 2.11b and 2.11c land.
- **K13. REVIEWED 2.12's flaws.** At `51b31fa`, clauses of one event had to
  spell their binders alike, a view's bare Text parameter lowered to
  `null`, and `S requires A and B` lowered to a dummy atom. Ruled in D-41: a
  clause is its own scope, a view reads its slot as `?name`, and each side
  of `requires` is one sentence (SPEC §5.2, §4.5, §6.2). Open until boxes
  2.12d and 2.12f land.

## Open questions

- **O1. Keywords against templates.** Decide where each keyword is read,
  and check a shape's template against that list when the shape is
  declared, so a collision is one error at the declaration rather than a
  misreading at each use. Start from `and`, `or`, `not`, `in`, `where`,
  `is a`, `requires`, `passes`, `happens when`, `has no single point of
  failure among`. Since D-41 a question is a command's argument, so `why`,
  `trace`, `prevent` and `achieve` are never read inside a module, and
  `via`, `at revision` and `using:` became flags. Ruled in D-42: a line is
  read every way keywords and templates split it, and exactly one typed
  reading must remain (SPEC §9.4), so no declaration-time check is needed
  (box 2.19g).
- **O2. What `count` counts.** `count ?x where …` ranges over the distinct
  bindings of every local variable of its body (§4.3, L3), not over
  distinct `?x`.
- **O3. `for` blocks as scope owners.** L3 names rules, handler clauses,
  claims and views, but SPEC §9.6's generated enables reads the outer
  `for`'s `?module` inside its inner query. Ruled in D-41: a `for` block is
  one more enclosing scope (SPEC §4.3), as 5.2 built it (`bd5c704`).
- **O4. `muove check` on a program that does not compile.** Box 3.11 exits
  2, the code that means `Unknown`.
- **O6. Why-comments as facts (T5 principles 9 and 10).** D-42 left 143
  of firn's 214 comment lines as "why" prose. Classified at `FIRN_ROOT`
  (the 41 `.bnix` and 30 Clause lines D-42 made redundant excluded; Clause
  counted outside text blocks): **75** can be checked facts or claims
  today, **8** need a small addition, **60** stay prose.
  - (a) 75: ownership, provenance, decisions, measurements, defect status
    and command usage, written as `given` facts under a library vocabulary
    (`{project} owns {path: Text}`, `{setting} mitigates {defect}`,
    `{defect} is reported at {url: Text}`, `{person} decided {decision} on
    {date: Text}`), with a requirement where they constrain, and the value
    generated from the facts by a query or `for` block where they justify
    one. Examples: `boot.clause:21–25` (`cwsr_enable=0` mitigates the
    gfx1150 MES hang, open upstream on 6.18 and 6.19, reported at
    frame.work/t/79221; a requirement fails once the defect is marked
    fixed; "do not bump the kernel" is a NixOS `assertions` entry);
    `codex/default.bnix:41–42` (firn and north own disjoint paths, `at
    most one owner per path`, the `home.file` entries generated from
    firn's facts); `nix-settings.clause:12–14` (24 build jobs, and the
    claim `cores * 3 == 24`); `rebuild-nopasswd.clause:3` (Tom decided it
    on 2026-07-09, with a requirement that the host has one user).
  - (b) 8: a purpose stated of one literal setting that no fact
    generates: `codex-runtime:33–34` (`StartLimitBurst`, fail visibly),
    `whiterabbit/configuration:65–66` (`TimeoutStartSec`), `niri:28–29`
    (out-of-store `config.kdl`), `swayidle:21–22`. The addition would be
    *settings as entities*: a fact may name a module's attrpath, and
    `muove check` fails when a fact names a path no module sets, so a
    stale reason becomes an error. Not earned yet: 8 lines, and generating
    values from facts covers the other cases. Revisit when the lift's
    idiomatic pass (T2) meets a reason it cannot attach.
  - (c) 60: causal mechanism and rejected alternatives, the 143's true
    "why": `boot.clause:22` (broken CWSR saturates MES, causing page
    faults), `codex/default.bnix:84–90` (the keyring unlock sequence),
    `beagle/default.bnix:13–20` (the stale-bytecode skew a packaged racket
    would reintroduce).
  - Rejected as additions: decimals for measurements (an `Int` in a
    smaller unit states them), a `Date` contract (provenance only quotes
    dates), version contracts on `nix` values (NixOS `assertions` check
    them at evaluation), and a `because` annotation (D-42).
- **O5. Questions in flight.** Every language `DECISION` line in the peer
  file that no landed ruling answers. A planner that stops without landing
  its ruling leaves its questions here.

## Who writes Muove

Agents write Muove, not Tom. On 2026-10-10, shown the kanata module, the
dependent-responsibility rule and the dungeon facts, Tom ruled through his
proxy: "its whatever the agent wants looks fine to me. im not writing the
config the agent is"; on sentence style, "whatever the agent thinks... PL
design is trending towards"; and on the general-purpose direction (T4),
"look good" (D-36, amended). So the owner decides syntax by what agents
write correctly and what current PL design favours, and Tom approves
direction, not syntax. This is Tom's decision, not open for a ruling, and
there are no exercises for Tom.
