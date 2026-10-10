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

Checkpoint of the sixth owner, 2026-10-10, restarted from this file.
D-51 landed after D-50, and the peer file has its `RULED` lines.

**Done: D-51,** the lead's batch. The length finding, measured over the 8
benchmark references in non-blank lines: Muove 405, Python 170, Soufflé
243. Muove's split: derived declarations 42, rule framing (`rule NAME` and
a lone `when`) 108, heads 54, bodies 117, requirements 18, `nix file`
record 66. Rulings: no output record outside Nix, since `muove query` reads
the answers (−66); derived shapes stay declared, because inferring them
breaks compass 10 and R16; a rule is `rule name: head` with `when L` and
`and L` lines, and a handler clause lays out `when` and `then` the same way
(−108, boxes 2.20a, 2.20b). The result is 231 lines, 1.4x Python. DECISION
5.5's four readings are confirmed and §9.6 quotes `"nur"`. DECISION 5.7(b)
exposed K15: §9.1 reserved every template word as an entity name; now a
template word is a name in a slot (2.8e), and the templates say `boot` and
`environment` again, with `at most one` modes for domain, criticality and
lifecycle (5.7b). DECISION 7.3: `muove run [PATH…] --events FILE` reads one
event sentence per line and prints JSON lines with hashes (7.3), and
`--events` gives `query` and `check` the final state (7.3b). The measuring
script was throwaway: it classified lines by indentation and leading word.

**Done: D-50,** the two `DECISION` lines open after D-49. R11 (6.2): a
shape's kind fixes where its facts come from, so a fact changes origin
only when its shape changes kind, which only a derived patch can do; 6.2
checks "now stated" on R9's baseline against a second frozen image with
`holds` declared `given`, and a written patch's `add Goal holds` stays an
error whose message now says rules give a derived shape's facts (2.8d).
`?_` (2.11g): the narrow reading stands, a `?_` inside an expression is
an error under a `not` too, and diagnostics print `?_`, not `?_#1`
(2.11h). No other line after D-48's batch was open.

**Done: D-49,** two benchmark-critical questions from the lead. An entity
exists only by its declaration: a name in an entity slot anywhere that no
module declares for that type is an error naming the nearest declared
entity (2.11f; supersedes D-41's "a fact introduces each entity it
names"). Measured: 1,535 of the corpus's 1,728 fact-named entities are
declared; the 193 that are not are all in firn's importer-written tag
facts (166 modules, 25 tags, 2 platforms), which the importer now
declares. Each `?_` is existential where it stands, inside a `not` too:
`not ?p borrowed ?_` means `?p` borrowed nothing (2.11g); 4 of the 14
`not` literals in the benchmark references negate a helper that one `?_`
replaces. The measuring scripts were throwaway: a template matcher over
the `.muove` sources and firn's facts from `FIRN_ROOT`.

**Done: batch (c).** D-43: a replacing `then` sentence names one key, so
a relation with two modes adds and step 5 checks both; clause variables,
a non-empty `then`, `may skip` per event (2.19d amended, 2.19h). D-44:
`only after` and `only when` bind their left variables once (2.19c lowers,
7.5 evaluates); a brace default never names a formal (2.19f); the
why-comment census (O6). D-45: the stages file (2.5b to 2.5d; K8). D-46:
sentence syntax (2.8b, 2.8c, 2.9c, 4.11b; K6). D-47: IR-lowering,
printer-lowering Q4, claims, explanation Q2 to Q4, L1, interventions over
claims (3.15d, 3.15e), no recursive defines (3.19); fields are fixed at
creation. D-48: every open peer `DECISION` line (2.6c, 2.15c, 2.16b,
3.14b, 6.1c; 2.12g widened). All nine planner files are now persisted;
explanation Q5 (2.14b) and DECISION 2.15's extractor field are the lead's
tooling.

**Next, in this order:**

- new `DECISION` lines in the peer file, newest first;
- the readability pair (T5's principle 13) once 2.20a and 2.20b land: check
  that it shows the D-51 layout and no output record;
- the zero-shot benchmark's failure transcripts, when they arrive: each
  misread or tooling failure names a construct for the next audit;
- O6's *settings as entities*, if the lift's idiomatic pass (T2) meets a
  reason it cannot attach to a generated value;
- K-items close as their boxes land: K4 (2.10c, 2.10d), K5 and K13
  (2.12d, 2.12f), K6 (2.8c), K8 (2.5c), K9 (3.17 landed), K10 (2.19g),
  K14 (6.2, 2.8d); K3, K7 and K12 are closed.

**Direction questions.** One is with Tom through the lead (D-47): an
entity's fields are fixed at creation, and anything that changes, a game
unit's hit points included, is a state relation written by a `then`
sentence; the other option is fields a `then` line rewrites. The owner
recommends fixed fields. D-42 makes effects a set; ask Tom only when a
program needs effects in source order, such as printed lines, with the set
against an ordered list as the two options.

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
10. A name means only what a declaration gives it: a misspelling is an
    error that names the nearest declaration, never a new thing, and a
    named variable never silently means "anything" (D-49; T5's principle
    12).
11. A line earns its place as a word does. A line break carries structure,
    so a keyword alone on a line goes, and output is declared only for a
    target that writes a file (D-51).

## Where the language rulings are

| Area | Rulings | SPEC |
| --- | --- | --- |
| Lexical | D-34 backticks; D-35 reserved characters and a leading `-`; T4 `1.5` reserved and spaced minus; L1 (§9.1 since D-47); D-42 keywords never shadow templates (O1); D-47 fmt's canonical layout; D-51 a template word is a name in a slot, so backticks settle only a two-reading line | §9.1, §9.2, §9.4, §11.1 |
| Record register | D-25 `nix file`; D-35 records, precedence, `where`, field access and types; L2; D-44 a default never names a formal | §9.4, §10.2 |
| Sentence register | D-10 declared templates and two registers; D-11 scoped vocabularies and explicit quantifiers; D-46 slot tokens, list slots by type, literal kinds as readings, one literal per `when` line | §4.2, §4.3, §9.2, §9.3, §9.4 |
| Names, scope and `use` | D-11; D-37 option tree, sequential `let`, prelude and the Nix target's names; L3; D-41 linked modules, project root, library, vocabulary scope and exports, questions as command arguments, view and clause scopes | §2, §3.2, §4.3, §4.5, §5.2, §9.4 |
| Values, types and stages | D-7 two stages; D-12 `Int` only; D-41 entity relations (`X is a T`), one name for entities of several types; D-42 types are contracts, one type grammar for options, `given`, `state entity` only; D-45 nix-lib signatures, unsigned calls, fact roots, narrowing, `for` sources, `Text` ordering; D-49 an entity exists only by its declaration, the nearest-name error, one name declared under several types; D-50 a derived shape's sentence is never a fact, and the error says so | §3, §4.1, §4.2, §9.4, §9.6, §10.2, §10.3 |
| Rules, aggregates and queries | D-51 `rule name: head`, then `when L` and `and L` lines, the body a conjunction; derived shapes stay declared; D-40 scopes (L3 to L5), value-free `count`, record rows, sort keys, `min`/`max` types, `the`; D-41 `for` as an enclosing scope; D-42 `min`/`max ?x by`; D-49 `?_` existential where it stands, inside a `not` too; D-50 `?_` only as a whole argument, printed as `?_` | §4.3 to §4.5, §7.1, §9.1, §9.4 |
| Claims and verdicts | D-4, D-5, D-6, D-26, D-29, D-30; D-37 work unit; D-40 mode checks per stratum; D-41 one sentence a side of `requires`, absence scope by kind, `acceptable` minimality; D-42 `only after`, `only when`, `milestone`; D-44 their left variables bound once around the claim; D-47 exit 2 without a verdict, one binding function, a verdict per claim, sufficient supports, interventions over claims, requirement names | §4.4, §4.6, §6, §7, §8.1 |
| Steps and effects | D-13, D-14, D-15, D-31; D-41 a clause is its own scope; D-42 `then` states the post-state, replacing sentences, `E` a set, canonical creation ids; D-43 a sentence replaces along one mode only, clause variables, `then` non-empty, `may skip` per event; D-47 fields are relations fixed at creation, typed views; D-51 a clause's `when` and `then` lines laid out as a rule's | §4.1, §4.5, §5 |
| Answers and the step CLI | D-51 no output record outside Nix (`muove query --json` reads relations); `muove run [PATH…] --events FILE`, one event sentence per line, JSON lines with revision and hash; `--events` for `query` and `check` | §4.5, §10.4, §11.1 |
| Change review | D-18 frozen baselines; D-29 failing instances; D-48 three patch words, identity normalization; D-50 a fact changes origin only with its shape's kind, so "now stated" needs a derived patch; a derived patch's edits | §8 |
| Evaluators | D-16; D-40 one expression evaluator and meter, differential comparison | §10.1 |

**L1 to L5.** An Opus max planner ruled these at 19:05 on 2026-10-10
(`~/.local/state/agents/handoffs/muove-semantics-rulings.md`). They reached
worker briefs but never SPEC or DECISIONS. L2 is in SPEC §2 and §10.2 since
D-37, L3 to L5 in §4.3 since D-40, and L1 in §9.1 since D-47. (`0a5bd23`
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
  §4.1 to §4.3). Fixed in `d00f678` (2.11e). D-49 then dropped the
  half where a fact introduces the entities it names: `is-a-T` holds only
  declarations, and an undeclared name is an error. Closed by 2.11f
  (`ff55705`).
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
  call goes. Closed by 2.19e (`84a0e22`).
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
  library is embedded (SPEC §2). Closed by 2.11b and 2.11c.
- **K13. REVIEWED 2.12's flaws.** At `51b31fa`, clauses of one event had to
  spell their binders alike, a view's bare Text parameter lowered to
  `null`, and `S requires A and B` lowered to a dummy atom. Ruled in D-41: a
  clause is its own scope, a view reads its slot as `?name`, and each side
  of `requires` is one sentence (SPEC §5.2, §4.5, §6.2). Open until boxes
  2.12d and 2.12f land.
- **K14. R11 against the kinds.** SPEC §8.4 asks review to report a
  derived fact that becomes stated as "now stated" (R11), while §4.1 lets
  no line state a derived shape's sentence; DECISION 6.2 found the written
  patch `add Goal holds` rejected with advice to use `initially`. Ruled in
  D-50: a fact changes origin only when its shape changes kind, which only
  a derived patch can do, and 6.2 checks R11 on two images frozen from
  source; the message says rules give a derived shape's facts. Open until
  boxes 6.2 and 2.8d land.

- **K15. Template words reserved as entity names.** §9.1 required
  backticks for a name that any template in scope uses as a word, and
  `read` rejected such a filler, so a library shape that says `boot`
  broke every host fact naming the entity `boot`. DECISION 5.7(b) found
  it: 5.7's templates said "startup" and "user session" to dodge it. D-42's
  one typed reading and D-49's declared entities make the rule redundant.
  Ruled in D-51: a template word is a name in a slot. Open until boxes 2.8e
  and 5.7b land.

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
  2, the code that means `Unknown`. Ruled in D-47: 2 whenever no verdict is
  reached, a static error included.
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
  file that no landed ruling answers. D-48 cleared every line up to the
  2.19 batch's, D-50 the two after it (6.2 and 2.11g), and D-51 5.5, 5.7
  and 7.3, so none is open at fa31751. A planner that stops without landing its ruling leaves its
  questions here.

- **O7. Naming created entities outside a program.** A `state entity`
  has no name, so `muove run`'s events file cannot name one that an
  earlier event created (D-51). Tasks key created things by a field value
  or a declared entity until a program needs more. Then rule a spelling
  that names an entity by its type and a key field, never by a raw id
  (§10.4 matches entities by creation order, never ids).

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
