# Galileo language notebook

The language owner's working memory (DECISIONS D-36). SPEC.md states each
rule and DECISIONS.md says why it was chosen. This file holds what the
owner needs between sessions: how rulings are made, the principles they
share, where each language ruling lives, the contradictions found and their
status, the open questions, and who writes Galileo. A new owner reads it
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
13. Done means that, plus Tom finding the Galileo version of one program more
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
17. The survival bar is TypeScript: on medium and complex programs with
    state logic, Galileo's median time-to-correct is 0.50 to 0.67 of
    TypeScript's, with a first-try rate at least TypeScript's; on small
    programs, parity. Python is a reference row, and Soufflé a warning if
    Galileo loses to it. This replaces 13's bar for the benchmark. The bar is on
    behaviour alone; behaviour plus review guarantees is reported beside
    it with no pass line until one measured run exists (Tom, 2026-10-12).
18. Fields fixed, history permanent: nothing is ever overwritten. A `then`
    sentence adds a fact at the next revision, the current state is the
    latest fact per key, and questions ask as of a revision, since a
    revision, and why across revisions. "Now" stays a materialized view,
    no query replays history, and compaction comes later (D-52).

## Where the work stands

Checkpoint of an Opus planner ruling as owner, 2026-10-12, at main
`8daa31d`, for DECISION 17:52 8.8b and DECISION 09:49 8.13.22. **Done:
D-85 addendum.** 8.8b's `accept-menu` costs six work per row (6,027 at
1,000) because a step's clause is matched by `query::Reader::matches` on
the source body, which 9.11r's rewrite never sees: `?row.visible` and
`?row.position == ?north.menu-selection` are filters, so the reader walks
`?north lists ?row`. The given the lead named costs ten at any width
(probe). The reader plans a `Bool` field condition and `?x.f == e` as the
field's atoms (SPEC §10.1). 8.8b lands now; 9.11t lands after it and
9.20 needs it. 8.13.22: `menu-count` is one derived field with a rule per
row kind (the A8 menu excludes `"usage"`, s3 counts usage rows); its
other choices stand, the table renamed `DERIVED_HANDLERS`. K28 widened.

Checkpoint of an Opus planner ruling as owner, 2026-10-12, at main
`144166b`, for DECISION 08:39 8.8b (peer file line 2810). **Done: D-85.**
8.8b's D-61 forms raised `menu_cost`'s offer from 214 to 46,180 work at
2,000 rows, because `?prior.visible` is no plain atom and `?row.order` no
variable threshold for the engine's plans. Variant f, a `shown-menu-row`
relation plus `?order = ?row.order`, is G10's named workaround and is
dropped. The language and the IR stay; the engine's copy plans an `exactly
one` `Bool` field condition inside an aggregate as its atom with `true`,
and a compared field of an outer variable as a value bound before the
aggregate (SPEC §10.1). Found K28. Box 9.11r (20 minutes) before 8.8b; 9.20
needs both.

Checkpoint of an Opus planner ruling as owner, 2026-10-12, at main
`c2ae815`, for DECISION 6.5 (peer file line 2715). **Done: D-70.**
`before` marks one sentence, as `before S` or `before not S`, and reads
the earlier state: the pre-state in `ensures` and the baseline in a patch
contract. An unmarked sentence reads the later state, and `after` is gone.
`ensures` reads one state per claim (§5.1). A contract binds as a
requirement's state claim does. `says` and `intends` are patch sections
that join neither image. `becomes` and `stays` are marked pairs, `only P
changes` is judged per `P`'s shape, and a derived patch carries no
contract. Contracts are checked by the one claim checker over the
baseline's facts copied in. Found K27. Boxes 6.5a to 6.5e.

Checkpoint of an Opus planner ruling as owner, 2026-10-12, at main
`8b52d04`, for DECISION 2.23b (peer file 2688). **Done: D-66
addendum.** `option P` with declarations indented under it is a group
that emits `options.P = { … };`. The source's layout is the Nix layout,
as with entries, so socrates-chat's one grouped block is written and 50
flat modules keep their bytes. Found K26 and opened O14. Box 2.23d. The
probes ran the CLI built at `6b70273`.

Checkpoint of an Opus planner ruling as owner, 2026-10-12, at main
`78b7827`, for DECISION 4.13: firn's main drifted past the pin.

**Done: D-66.**

- Prefix `-` comes in as D-35 set aside, since plato's `-500` is firn's
  first negative number. It binds tighter than every binary operator and
  looser than field access and calls, folds into an integer literal, and
  means `0 - e` outside Nix.
- `{...}` is the brace parameter with no formals.
- A `let` name bound to a known `Text` interpolates as `${name}` (K25).

Kept: one-token slots and D-35's lexer rejection. Boxes 2.23a–2.23c.
D-67, the planner's half, re-pins the tests to `4bcfad4c`. The probes
ran the CLI built at `4243032`.

Checkpoint of the eighteenth owner, 2026-10-12, restarted from this file
at main `359d05f` for one question: box 4.5b, which the firn rulings
planner routed here (D-63).

**Done: D-65.** A computed name, `` `{e}` ``, is the Nix binder or
reference that the known `Text` `e` spells. Backticks already mark a
name and `{e}` already computes one, so no word is added. A `for`
element, `for H: x`, unrolls inside an inline record or a lambda's brace
formals, as a `for` block unrolls entries. A computed name never shadows
and is never shadowed: a text that is not a Nix identifier, a Nix
keyword, a formal that another formal or a name in scope already has,
and a reference that no Nix binding has or that two enclosing bindings
have are each an error at its line. A bare name never reads a computed
formal. `flake.gal` goes from 45 hand-written input names to 0, `?`
from 17 to 41, `{` from 95 to 104, and 10,736 characters to 10,548. A
new module input is one fact again, as firn's `docs/flake-inputs.md`
promises. Found K24: backticks took any text. Kept: `{?n}: `{?n}``
written out (addendum 4's formals reason). Boxes 2.22a (after 2.21s),
2.22b and 2.22c, then 4.5b. Probes ran the CLI built at `da5ec7c`.

Checkpoint of an Opus planner ruling as owner, 2026-10-12, at main
`6812a15` in lane `review-triage`, for the triage of ChatGPT's outside
review (`~/.local/state/agents/handoffs/galileo-external-review/`).

**Done: D-64.** `possibly S` replaces `eventually S` with the same meaning,
and `eventually` is a reserved error, since temporal logic and model
checkers read it as every run reaching `S` (compass 9, T5's principle 8);
a failed `requires` prints the proof that survives with its condition
blocked; review lists the edited definitions each requirement reads
(SPEC §8.4 item 8). Boxes 2.19i, 3.11e, 3.12b, 7.4b, 6.11 and 6.12.
Opened O13. Probes ran the CLI built at `739b9ed`: all five dungeon
requirements answer Unknown, and the review's door-and-key bypass holds
vacuously without `--events`.

Checkpoint of the seventeenth owner, 2026-10-11, restarted from this file
at `2cf9917` in lane `owner-offer`, built on `owner-once`, for O12, which
Tom's proxy ruled as an engineering call: no event states another's posts,
the three offers merge into one `offer-row`, and every caller moves in the
same change, north's included.

**Done: D-61 addendum 5.** `offer-row North ?key ?label ?description
?annotation`, one clause: north composes a work item's texts, the program
marks the active conversation's row `current` from the menu's kind, and the
event carries no kind, since the menu's kind is state. SPEC §5.2: events
whose clauses make the same posts are one event, what the host knows an
argument and what the state holds read in the clause; no named post block
(a new construct the merge makes needless). Kept: the two `open-*-menu`
events, whose posts differ. Sketch 162 → 141 lines, `?` 178 → 128, `{` 12
→ 5, 5,655 → 4,605 characters. Boxes 8.8c (galileo, present forms) and
8.8d (north's lane, cross-repo); 8.8b, 8.14 and 9.13 amended.

Checkpoint of the sixteenth owner, 2026-10-11, restarted from this file at
main `8f70152` in lane `owner-once` for Tom's "O11 gets fixed" and his wider
standing rule: a smell that makes the author write something twice, or
write ceremony (repeated names, helper bindings, filler syntax), is fixed
without asking; only a fix that changes direction or breaks programs beyond
an in-repo rewrite is escalated.

**Done: D-61 addendum 4.** A `for` binds its query's variables in place,
`for ?name, ?url where C`, the rows `all ?name, ?url where C` gives; `for
?x in e` iterates a list, and a `for` over a variable-head `all` or a
positional `for ?a, ?b in e` is an error offering the `where` form. A
record element `?x` is `x: ?x` and `?x.f` is `f: ?x.f` (SQL's select list,
Nix's `inherit`), the long spelling an error. A focus block holds `is a T`,
field facts `f = e` and tables, and two or more facts in a run with one
subject are one focus block; three or more of one template a table, a
one-column run a list. §9.6 and §9.7 rewritten; dispatch.gal, prewarm.gal,
host-list.gal, responsibilities.gal, flake.gal and the tags importer follow
in boxes. Kept: quantifier variable lists (the binder Tom's own `for`
example keeps), shared-condition requirements (one verdict each), named
rule cases, shape declarations, explicit receivers, north's `menu-`
prefixes, the mailbox, `option` paths. Escalated: posts shared across
events (O12). Sketch 161 → 162 lines, `?` 178, `{` 12, 5,871 → 5,655
characters, 18 → 0 repeated record names. Boxes 2.21s–2.21w.

Checkpoint of the fifteenth owner, 2026-10-11, restarted from this file at
main `6c39c53` in lane `owner-count` for Tom's "O10 gets fixed" and his
standing rule: a binding that exists only because a construct is not an
expression is fixed without asking.

**Done: D-61 addendum 3.** An aggregate is an expression, wherever one is
due; the aggregate literal goes, and `?n = count where …` is the binding
of one. `count` opens an aggregate only before `where`, and `sum`, `min`
and `max` only before a token that begins an expression other than `(`,
so `count(xs)`, `min(a, b)` and `define wrap(index: Int, count: Int)` keep
reading (reserving the words, as `all` was, breaks the sketch, §9.7, the
card and firn's 15 `count(list)` calls). Its variables scope as a query's;
a rule's head is its owner's top level. `min`, `max` and the argmin are
`maybe T`, `null` when nothing matches, and a `null` field head derives no
fact. Lowering hoists into the IR's aggregate literal, so D-58's ranked
count holds. The scan fixed dispatch.gal's three `?arguments` (a head slot
already takes a list) and the importer's `sum 1` bindings, and opened O11.
Sketch 163 → 161 lines, 182 → 178 `?`. Boxes 2.21q (8.8b's path) and 2.21r.

Checkpoint of the fourteenth owner, 2026-10-11, restarted from this file
at main `812fe17` in lane `owner-all` for Tom's "fix it" on the
addendum's G10 smell.

**Done: D-61 addendum 2.** `all` takes an expression: its head is computed
once per distinct binding of its local variables, so a row is a binding,
K1 holds, and `all ?x` means what it did. No line gets two readings; the
near miss, `all (e)` as a call (a Nix `with lib:` can supply `lib.all`),
is closed by reading `all` as a keyword wherever an expression is due. The sketch's `?entry` binding goes: 163
lines, 184 → 182 `?`. Box 2.21p after 8.9m; 8.8b waits for it.

The thirteenth owner's checkpoint, 2026-10-11, restarted from this file
at main `bc78fec` in lane `owner-sigil` for Tom's follow-ups to D-61.

**Done: the D-61 addendum.** `?` stays on every variable, the head's
included: a bare token in a sentence is an entity or a word, the same
text would otherwise mean two things in `initially` and in a clause, a
`when` line would change meaning when moved into a rule, and K5 already
ruled it for views (sketch 158 `?`; dropping head-bound ones would give
85). One notation for a blank: a slot is its bare type or `?name: Type`,
and braces outside text mean only records (256 → 92 → 19 `{`). Fact
tables state many facts of one shape under a header of variables, and
spaces between cells mean nothing. Articles are the author's words; the
idiom drops filler. A shared precondition is an `every ?v where C: P`
post, which also empties the menu with no `may skip`. `?hero is a Hero`
stays, the domain of the universal (compass 10). A list declares one
entity per element. A shape's name is written only when needed. The two
registers are two frames over one expression grammar. Probe:
`galileo check` accepts a `when` variable nothing grounds, and `run`
fails without a line (K23, box 2.21j). Dungeon 112 → 91 lines, 61 → 55
`?`, 27 → 0 `{`; the sketch 164 → 163 lines, 158 → 184 `?`, 35 → 12 `{`.
Boxes 2.21j–2.21o.

The twelfth owner's checkpoint, 2026-10-11, restarted from this file at
main `1ee816a` for Tom's batch, and rebased onto main before landing.

**Done: D-61,** Tom's "don't let the language be ugly. Fix those issues fix
the gaps", given after `examples/north-menu.gal`, and his additions through
the proxy (slots named by their types, no `{value: T}` slots, the braces
counted). Measured causes: membership already tested a grounded variable
and `else if` already needed no parentheses, so those two workarounds were
the importer's; the record was missing, since D-47 gave fields only to
created entities. Ruled: `state for T` and `derived for T` declare fields
that `then ?x.f = e`, `initially name.f = e` and a rule head `?x.f = e`
state, each the relation `T.f` the IR already had; creation fields stay
fixed; a `{value: T}` field-shape is an error; a capitalized template word
is a type and names its slot (`state Hero is in Room`), and `{room: Room}`
becomes an error once 2.21g rewrites the corpus; `e in list` is a
comparison; no `match`, since a `define` holds an `else if` chain one case
per line and a partial mapping is a given table; G10 makes a forced
workaround a language bug. The importer writes all of it (8.6b, 8.6c).
Sketch: `notes/north-menu-sketch.md`, 198 → 164 lines, 66 → 35 braces; all
twelve `.gal` files 256 → 92 braces. The tension for Tom, if the lead takes
it: a created entity's state fields change, while T5's 18 fixed fields; the
creation fields stay fixed. Probes ran the CLI built in lane
`codex-8.9j-in-place-step` (`0dfa3f8`).

The eleventh owner's checkpoint, restarted from this file at main
`8819f89` for one question:

**Done: D-60,** 6.3's finding (peer file 1768, 1772). `?n holds has no
single point of failure among ?n has ?b` is well-formed: §6.2 and D-47
bind the left side of all four binary forms universally, and the error
is the reader's, whose single-point-of-failure reading judges its left
sentence as a claim of its own (K22). The `every ?n: …` the error offers
reads as a single point of failure over a quantified claim, a static
error, so the universal requirement had no spelling. A temporal claim on
the left binds as it does alone, so `eventually ?h …` there errors naming
`?h`. No new box: 3.11b and 3.11d, which own the binding function and
remove the reader's scan, now measure it. Probes ran the CLI built at
`7c63fb7`, whose reader is main's.

The tenth owner's checkpoint, restarted from this file at main `209fa44`
and rebased onto `14f5bdf`, whose D-58 is a one-shot planner's engine
ruling:

**Done: D-59,** two DECISION lines from WP4 (peer file 1634, 1638, 1639)
and 4.26's build failure (1650). A text block's closer starts its line
after nothing but spaces, 2 spaces deeper than the line that opened the
block, and the rest of that line continues the logical line, another block
included, which closes at the same column; §9.4 already made a text block
an expression anywhere, and firn's beagle module (`'''})`) and flake
overlays (`'''), extraInstallCommands: '''`) need it (K20, box 4.2a:
`lexer.rs` and `fmt.rs`; the record printer is right). 4.2's gate keeps
seven files with `flake.nix` parse-identical (D-38). `lib.types.lines` is
no contract, only a NixOS merge policy over `Text`'s values, so no
`option` type emits it, and the submodule scaffold is a `nix file` calling
`lib.mkOption` (K21; §10.2). `galileo build` failing at `use firn/tags`
is a CLI bug against §2 and 2.11c, not a language question (box 4.26a,
with the scaffold's two `firn/nix-lib` signatures). Probes ran the CLI
built at `a98acb9`.

**Done: D-56,** the audit of the four lead-signed rulings O5 listed.
Confirmed: 7.10 (an admitted no-op takes revision `r+1` with an empty
change; a rejected step records nothing; now in §5.7). Confirmed with a
layout: 7.8 (one block per occurrence with every reached clause and
substitution; two or more parts each get a `by the clause at` line with
their proof beneath; box 7.8c, §7.3). Withdrawn: 2.19g's finding that a
keyword reading and a template reading never both type-check; since 2.8e
a one-word template's word can fill a slot, so `build requires test`
has both, and 2.19g's Done when now tests it (§9.4; the lane predates
2.8e). Replaced in part: 4.16's static keys through a computed segment; a
quoted segment is a text literal, decoded and interpolated as a value is,
and `{"…"}` is an error (K19, box 4.16b, §3.1). Probes ran the CLI built
at `b39203a` (`c8e490f` plus 7.9b's settlement).

**Done: D-55,** the audit of nine `RULED` lines the lead posted under the
owner's name (peer file 1127 to 1460) and of 7.9's review-cut sentence in
§5.4. Confirmed as ruled: 5.7b (warnings never stop a check, already
§6.1), 7.9 (only `settled by` shapes have pending requests; a sequence's
`E` is one set), 8.3 (`run<E: Event>`, `view<V: Projection>`, untyped
`project`), 8.6 (rows stay state shapes until 8.8; `at most one` for rows
the clear withdraws), 8.2 (D-54, attribution corrected), 8.6's mailbox
(kept with its guards, plus the carrying effect; O8) and 8.7 (a slice
replays its projection, now §10.4). Confirmed in SPEC's spelling: 2.10c's
module-qualified type identity is `dungeon/Hero`, not the landed
`dungeon.Hero` (K17, box 2.10e). Replaced: a receipt settles the oldest
pending request that agrees with it on every slot both shapes name, the
receipt names every entity slot of the effect, and a mismatch is an error
at the `settled by` line (K18, box 7.9b). Found: §5.1 and §10.4's hash
left out the pending requests that §5.4 and the code put in the state
(K16, closed by the SPEC text). D-53 and D-54 sat after the review record;
they now close section 1 with D-55.

D-52 landed after D-51, and the peer file has its `RULED D-52` line.

**Done: D-52,** Tom's principle 18. D-47's option A is confirmed, and
nothing is overwritten. A session keeps a span `[from, until)` per fact and
per live entity (SPEC §5.7). An admitted step closes the spans of
`S \ S'` and opens spans for `S' \ S` at `r+1`, so the history changes
exactly where the state does. Under one mode, "the latest fact per key" is
the key's open span, and "superseded by" names it. Under no mode or two
modes, the key is the whole fact. A re-stated fact records nothing (final:
Tom left it to the design, 2026-10-11). Deletion
closes the entity's spans and those of its facts; ids are never reused, so
they stay answerable. `h(S)` stays over the state alone, so search merges
states and replay rebuilds the history. A program reads one state: no
construct names a revision. Surface: `--as-of R` (replacing D-41's
`--revision`, which collides with git revisions) on `query`, `check`, `why`
and `why not`; `--since R` on `query`, which prints JSON lines of rows
added and removed per revision; and `why … --events` printing a chain of
title, one block per occurrence, latest first, naming the clause. Boxes in
order: 7.10, 7.11, then 7.8 and 7.12, then 7.8b (90 minutes). Amended:
7.4, 7.7, 7.9, 8.1, 8.11. Prior art checked: Datomic's filters page, and
SQL Server's temporal tables, which record a history row for every update
even when nothing changes (option B).

**Done: D-51,** the lead's batch. The length finding, measured over the 8
benchmark references in non-blank lines: Galileo 405, Python 170, Soufflé
243. Galileo's split: derived declarations 42, rule framing (`rule NAME` and
a lone `when`) 108, heads 54, bodies 117, requirements 18, `nix file`
record 66. Rulings: no output record outside Nix, since `galileo query` reads
the answers (−66); derived shapes stay declared, because inferring them
breaks compass 10 and R16; a rule is `rule name: head` with `when L` and
`and L` lines, and a handler clause lays out `when` and `then` the same way
(−108, boxes 2.20a, 2.20b). The result is 231 lines, 1.4x Python. DECISION
5.5's four readings are confirmed and §9.6 quotes `"nur"`. DECISION 5.7(b)
exposed K15: §9.1 reserved every template word as an entity name; now a
template word is a name in a slot (2.8e), and the templates say `boot` and
`environment` again, with `at most one` modes for domain, criticality and
lifecycle (5.7b). DECISION 7.3: `galileo run [PATH…] --events FILE` reads one
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
the `.gal` sources and firn's facts from `FIRN_ROOT`.

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

- 8.8b now, then 9.11t (D-85 addendum): check that 8.8b's example holds
  no `shown-menu-row`, no `?order = ?row.order` and no sentence atom
  written for speed, and that `menu_cost` measures accept after 9.11t;
- 8.13.22: check that `menu-count` has exactly the two disjoint rules and
  that `DERIVED_HANDLERS` lists only handlers Galileo derives;
- D-64's boxes as they land: 2.19i, then 7.4b, 7.4c (T6), 3.11e and
  3.12b, then 6.11; check each verdict line against G9 and the `possibly`
  wording, and that `requires` on an event program never says `in the
  initial state`;
- D-61's boxes as they land: 2.21a, 2.21b, 2.21c and 8.6b before 8.9m,
  then 2.21d, 8.3b, 2.21p, 2.21q, 2.21r, 2.21s, 2.21t, 2.21u and 8.8b, then
  2.21v and 2.21w; check each landed form against SPEC §4.1 and
  G10, and audit the rewritten `examples/north-menu.gal` against the sketch;
- D-65's boxes as they land: 2.22b and 2.22c now, 2.22a after 2.21s,
  then 4.5b; check `flake.gal`'s five sections against SPEC §10.3 and
  G10;
- new `DECISION` lines in the peer file, newest first, and any `RULED`
  line a lead signs as language owner, audited as D-55 and D-56 did (O5);
- boxes 4.2a and 4.26a (D-59), which close K20 and unblock 4.2 and 4.26;
- boxes 3.11b then 3.11d, which close K22 (D-60);
- the readability pair (T5's principle 13) once 2.20a and 2.20b land: check
  that it shows the D-51 layout and no output record;
- the zero-shot benchmark's failure transcripts, when they arrive: each
  misread or tooling failure names a construct for the next audit;
- O6's *settings as entities*, if the lift's idiomatic pass (T2) meets a
  reason it cannot attach to a generated value;
- K-items close as their boxes land: K4 (2.10d), K6 (2.8c), K8 (2.5c),
  K14 (2.8d), K20 (4.2a), K22 (3.11d), K24 (2.22b); K3, K5, K7, K9, K10,
  K12, K13, K15 to K19 and K21 are closed.

**Direction questions.** D-47's question is closed: Tom chose fixed fields
and a permanent history (principle 18, D-52). D-52's restated-facts question is closed:
option A, recording only changes of state, is final (Tom: "whatever the
proper answer is consistent with the language's design"). D-42 makes effects a set; ask Tom only when a
program needs effects in source order, such as printed lines, with the set
against an ordered list as the two options.

## How a ruling is made

- **One owner at a time.** The Galileo lead keeps one Opus xhigh planner (max
  only when Tom asks by name), `galileo-language-owner`, and sends it each
  tick's language `DECISION` lines with SendMessage. It rules on what a
  Galileo program means and how it is spelled: lexical rules, both registers,
  names and `use`, types and stages, rules, aggregates and queries, claims
  and verdicts, steps and effects, the Core IR, diagnostics and `galileo fmt`.
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
6. Galileo keeps adding keywords (T4), so where a keyword and a template word
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
12. A program's meaning never depends on a run. Every construct reads one
    state, and history is reached only through a question's flags; a
    program that needs its past states it as a relation (D-52).
13. A workaround in an example or in importer output is a language bug: find
    whether the construct is missing or only unused, fix the language or
    the importer, then rewrite the example (G10, D-61).

## Where the language rulings are

| Area | Rulings | SPEC |
| --- | --- | --- |
| Lexical | D-34 backticks; D-35 reserved characters and a leading `-`; T4 `1.5` reserved and spaced minus; L1 (§9.1 since D-47); D-42 keywords never shadow templates (O1); D-47 fmt's canonical layout; D-51 a template word is a name in a slot, so backticks settle only a two-reading line; D-56 nothing keeps a keyword reading from a template reading, so such a line is an error showing both; D-59 a text block's closer starts its line and the rest of that line continues the block's line, so a block stands inside a call, list or record; D-61 a capitalized template word is a type, a type's name begins with a capital, and a template word that must be capitalized takes backticks; D-61 addendum: `?` marks every blank, a template slot (`?name: Type`) and every variable at every occurrence, the head's included; a `define`'s parameters stay bare; D-61 addendum 2: `all` is a keyword wherever an expression is due, and a name `all` takes backticks; DECISION 2.21i: fourteen keywords are read wherever an expression is due (`all and else false if in not null or then true when where with`), and one list, galileo-syntax's `expr::KEYWORDS`, serves the parser, the record printer and the tags importer; every other grammar word needs backticks only on a two-reading line; D-61 addendum 3: `count` opens an aggregate only before `where`, and `sum`, `min` and `max` only before a token that begins an expression other than `(`, so otherwise each is a name and `count(xs)` a call; D-61 addendum 4: after a `for`'s variable list, `in` or `where` decides the form; D-65 backticks hold one bare name or one `{e}`, a computed name, and other text between them is an error (K24); D-66 prefix `-` | §2, §9.1, §9.2, §9.4, §11.1 |
| Record register | D-25 `nix file`; D-35 records, precedence, `where`, field access and types; L2; D-44 a default never names a formal; D-56 a quoted key is a text literal, decoded and interpolated as a value is, `{"…"}` an error, quoted keys emit quoted Nix keys; D-61 addendum 4: `for ?a, ?b where C` binds the query's variables in place, `for ?x in e` a list, no positional destructuring; a record element `?x` or `?x.f` names its field, the long spelling an error, bare names and entries unchanged; D-63 a Nix selection takes a quoted or computed segment, only on a Nix-stage value, and a parenthesized selection is a selection of its own; D-65 a `for` element `for H: f` unrolls inside an inline record or a lambda's brace formals, its header ending at the first `:` outside its brackets and `for` opening one only before `?`; a computed name is a brace formal or a reference in a Nix body, never a key, where it offers `{e}`; D-66 `{...}`, a `let` name interpolates as itself ; D-66 addendum: `option P` with declarations indented under it is a group, emitting `options.P = { … };`, flat lines emit one binding each, and the source's layout is the Nix layout | §3.1, §4.3, §9.4, §9.6, §10.2, §10.3 |
| Sentence register | D-10 declared templates and two registers; D-11 scoped vocabularies and explicit quantifiers; D-46 slot tokens, list slots by type, literal kinds as readings, one literal per `when` line; D-61 a slot named by its type (`state Hero is in Room`), braces only for another name, a prelude type, a type with arguments or a shared type, and `{room: Room}` an error once 2.21g lands; D-61 addendum: a slot is its bare type or `?name: Type`, braces outside text mean only records, fact tables under a header of variables, a list declares one entity per element, a template's words are its author's and filler articles go, a shape's `name:` only when needed, two registers over one expression grammar; D-61 addendum 4: a focus block holds `is a T`, field facts `f = e` and tables, two or more facts with one subject in a run are a focus block, three or more of one template a table, a one-column run a list | §4.1, §4.2, §4.3, §9.2, §9.3, §9.4 |
| Names, scope and `use` | D-11; D-37 option tree, sequential `let`, prelude and the Nix target's names; L3; D-41 linked modules, project root, library, vocabulary scope and exports, questions as command arguments, view and clause scopes; D-55 a type's identity is `module/Type` inside, its written name in messages; D-63 an `imports` value that is not a path opens, without a schema, each node where its module declares an option; D-63 addendum: an unknown option with no near name lists its node's names, at most six; D-65 a computed name neither shadows nor is shadowed, reads only a Nix binding, and is never read by a bare name | §2, §3.2, §4.3, §4.5, §5.2, §9.4, §10.3 |
| Values, types and stages | D-7 two stages; D-12 `Int` only; D-41 entity relations (`X is a T`), one name for entities of several types; D-42 types are contracts, one type grammar for options, `given`, `state entity` only; D-45 nix-lib signatures, unsigned calls, fact roots, narrowing, `for` sources, `Text` ordering; D-49 an entity exists only by its declaration, the nearest-name error, one name declared under several types; D-50 a derived shape's sentence is never a fact, and the error says so; D-59 `lib.types.lines` is no contract, so no `option` type emits it and a `lines` option is a `nix file`'s `lib.mkOption` call; D-61 `state for T` and `derived for T` fields, creation fields fixed, a `{value: T}` field-shape an error, G10; D-48 addendum (DECISION 2.6c): an interpolated `Path`, known or not, is emitted as written inside `${…}`, never computed, and makes its text `Nix Text`; D-63 a fact never writes `null`, `builtins.match` is `maybe List (maybe Text)`, `++` with a `Text` operand is `Text` | §1.4, §3, §4.1, §4.2, §9.1, §9.4, §9.6, §10.2, §10.3 |
| Rules, aggregates and queries | D-51 `rule name: head`, then `when L` and `and L` lines, the body a conjunction; derived shapes stay declared; D-40 scopes (L3 to L5), value-free `count`, record rows, sort keys, `min`/`max` types, `the`; D-41 `for` as an enclosing scope; D-42 `min`/`max ?x by`; D-49 `?_` existential where it stands, inside a `not` too; D-50 `?_` only as a whole argument, printed as `?_`; D-61 `e in list` a comparison and `?x in e` membership, a rule head `?x.f = e`, cases as a `define`'s `else if` chain or a given table, no `match` before sum types; D-61 addendum: a handler clause's `when` is safety-checked like a rule's, errors at the literal's line, `?x is a T` is the domain of a universal and stays; D-61 addendum 2: `all e where …` computes `e` once per distinct binding of its local variables, a row is a binding, a sort key reads only the head's variables, a variable list of two or more stays, `{p: ?p, v: ?v}` offers it, `the` keeps one variable; D-61 addendum 3: an aggregate is an expression wherever one is due and the aggregate literal goes, its condition and value a nested scope, a rule's head its owner's top level, `min`/`max`/argmin `maybe T` and `null` when nothing matches, a `null` field head derives no fact, lowering hoists into the IR's aggregate literal | §3.5, §4.3 to §4.5, §5.2, §6.2, §7.1, §9.1, §9.4 |
| Claims and verdicts | D-4, D-5, D-6, D-26, D-29, D-30; D-37 work unit; D-40 mode checks per stratum; D-41 one sentence a side of `requires`, absence scope by kind, `acceptable` minimality; D-42 `only after`, `only when`, `milestone`; D-44 their left variables bound once around the claim; D-47 exit 2 without a verdict, one binding function, a verdict per claim, sufficient supports, interventions over claims, requirement names; D-55 confirms a warning never stops a check; D-60 a single point of failure binds its sentence's variables universally, as `requires` does, and a temporal claim on its left binds as it does alone; D-64 `possibly S` is reachability, `eventually` is reserved for every run reaching `S` and is an error offering `possibly`, and a failed `requires` prints the proof of `S` that survives with `C` blocked; T6 (Tom) on a program with events, `S requires C` and a single point of failure over a sentence are judged in every reachable state: Holds only on a complete search, Fails at the first failing state with the trace that reaches it and the surviving proof, otherwise Unknown, and `--events` changes neither; D-70 `before` marks one sentence, `before S` or `before not S`, and reads the earlier state, an unmarked sentence the later one, no `after`, `not before S` and a mark on a non-sentence errors, a mark binds nothing, and each `ensures` claim reads one state | §4.4, §4.6, §5.1, §5.2, §6, §7, §8.1, §10.4, §12.1 |
| Steps and effects | D-13, D-14, D-15, D-31; D-41 a clause is its own scope; D-42 `then` states the post-state, replacing sentences, `E` a set, canonical creation ids; D-43 a sentence replaces along one mode only, clause variables, `then` non-empty, `may skip` per event; D-47 fields are relations fixed at creation, typed views (confirmed by T5's 18); D-51 a clause's `when` and `then` lines laid out as a rule's; D-52 a superseded fact stays in the history; D-55 a receipt settles by slot name and must name every entity slot of the effect, only a `settled by` shape has pending requests, a sequence's `E` is one set, pending requests are part of the state and its hash; D-61 `then ?x.f = e` states a state field, `= null` withdraws a `maybe` one, `initially name.f = e`; D-61 addendum: a post `every ?v where C: P`, one more clause of the event; D-61 addendum 5: events whose clauses make the same posts, differing only in values, are one event, what the host knows an argument and what the state holds read in the clause; no event states another's posts and no post block is named for clauses to share | §4.1, §4.5, §5 |
| History and time | D-52 spans per fact and live entity, the latest fact per key by mode, re-statements record nothing, `h(S)` over the state alone, programs read one state, the chain of title, compaction later; D-56 an admitted no-op takes its revision with an empty change; a chain block gives each of two or more clause-and-substitution parts its own `by the clause at` line | §5.7, §7.3, §10.4, §12.1 |
| Answers and the step CLI | D-51 no output record outside Nix (`galileo query --json` reads relations); `galileo run [PATH…] --events FILE`, one event sentence per line, JSON lines with revision and hash; `--events` for `query` and `check`; D-52 `--as-of R` for `query`, `check`, `why` and `why not`, `--since R` for `query`, `why … --events` | §4.5, §10.4, §11.1 |
| Change review | D-18 frozen baselines; D-29 failing instances; D-48 three patch words, identity normalization; D-50 a fact changes origin only with its shape's kind, so "now stated" needs a derived patch; a derived patch's edits; D-64 review lists each edited rule, definition or derived shape beside every requirement and milestone that reads it, informing, never gating; D-70 `says` and `intends` are patch sections that join neither image, `intends` takes state claims, `only P changes` (per `P`'s shape) and `requirements unchanged`, binds as a requirement's state claim, `becomes true`, `becomes false` and `stays` stand for marked pairs and the long form is an error, a contract is checked on the patched program plus the baseline's facts of each shape a `before` sentence reads, both sides' initial states on a program with events, and a derived patch carries no contract | §8, §9.4, §12.1 |
| Evaluators | D-16; D-40 one expression evaluator and meter, differential comparison; D-85 a spelling never changes what a step costs: the engine's copy plans an `exactly one` `Bool` field condition inside an aggregate as its atom with `true` and a ranked comparison's field of an outer variable as a value bound before the aggregate, the IR unchanged; its addendum plans a step clause's `Bool` field condition and `?x.f == e` as the field's atoms | §10.1 |
| Runtime and migration | D-28 mechanical first; D-54 an image is linked source plus identity; D-55 typed `run` and `view` through `Event` and `Projection`, untyped `project`; the mechanical menu keeps its mailbox and emits the carrying effect; a slice replays the recording projected onto its events | §9.7, §10.4 |

**L1 to L5.** An Opus max planner ruled these at 19:05 on 2026-10-10
(`~/.local/state/agents/handoffs/galileo-semantics-rulings.md`). They reached
worker briefs but never SPEC or DECISIONS. L2 is in SPEC §2 and §10.2 since
D-37, L3 to L5 in §4.3 since D-40, and L1 in §9.1 since D-47. (`0a5bd23`
landed D-40 as D-38, a number 1.7's printer entry, `cc1dc93`, also took; it
became D-40 after WP4's D-39, `f50ab7d`, and peer lines citing D-38 for
scopes, counts or modes mean D-40.)

- **L1 (box 2.1).** A text block's closing `'''` starts its line,
  exactly 2 spaces deeper than the line that opens the block, and content
  is dedented to it. Since D-59 the rest of the closer's line continues
  the line that opened the block (box 4.2a).
- **L2 (boxes 1.4, 2.6).** Galileo's `let` is sequential and non-recursive,
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
  one claim says `every ?h where ?h is a Hero: …`, `galileo check` answers
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
  transitive (SPEC §2). 2.10c landed (`c9cb3b7`); open until box 2.10d
  lands.
- **K5. View parameters.** SPEC §9.7's view writes its parameter bare
  (`north has menu kind ?k`), while every other variable in a sentence is
  `?name` (§9.1). Box 2.12 (`51b31fa`) reads a bare name equal to a
  parameter as the parameter, so an entity of that name cannot be named in
  the view. Ruled in D-41: a view's slot is `?north` in its body, and a
  `define`'s parameters stay bare names (SPEC §4.5, §9.7). Closed by 2.12d
  (`3553b2d`).
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
  Closed by 3.17 (`e4a64fa`); 2.5e landed in `caad196`.
- **K10. Template words that are keywords.** `lib/firn/responsibilities.gal`
  declares `{responsibility} requires capability {capability}`, while
  §9.4's claims include `conj "requires" conj`. Measured at `f2802f2`:
  `backup requires capability gpu` reads as a fact at the top level but
  fails inside a `requirements` block, and the closest-shape list offers
  the very template it should have matched. D-35 says a template may use
  any word, yet `and` separates a condition's literals (D-35), and 2.9 lets
  `?x in` win over a template that starts `{x} in`. Box 5.7 writes the
  boot-coupling requirements over `requires capability`. See O1. Ruled in
  D-42 with O1: every reading is tried and exactly one must remain. D-56
  withdrew the lead's finding that a keyword reading and a template reading
  never both type-check: since 2.8e, `build requires test` has both, and at
  on main it silently reads as the claim. Closed by 2.19g (`406110b`,
  `dceed07`).
- **K11 (not a language question).** D-20 says `rnix` parses Nix "in dev
  tools and tests", while D-27 and box 4.16 put an rnix importer, `galileo
  import nix`, in the shipped binary. D-20's rule, a dependency that
  replaces an external format, allows it in `galileo-import`, so its wording
  is stale. For the planner of DECISION 4.6/4.16. Resolved by D-39, which
  corrects D-20's wording.
- **K12. The CLI reads one module.** `lower_files` joins every path that
  `galileo check` gets into one module, so a file with a `use` fails ("there
  is no module `firn/tags`"), and an unrelated file's template breaks
  another file's line (R16; measured at `f2802f2`). Ruled in D-41: modules
  are linked under `module/name`, named from the project root, and the
  library is embedded (SPEC §2). Closed by 2.11b and 2.11c.
- **K13. REVIEWED 2.12's flaws.** At `51b31fa`, clauses of one event had to
  spell their binders alike, a view's bare Text parameter lowered to
  `null`, and `S requires A and B` lowered to a dummy atom. Ruled in D-41: a
  clause is its own scope, a view reads its slot as `?name`, and each side
  of `requires` is one sentence (SPEC §5.2, §4.5, §6.2). Closed by 2.12d
  (`3553b2d`) and 2.12f (`5e5842b`).
- **K14. R11 against the kinds.** SPEC §8.4 asks review to report a
  derived fact that becomes stated as "now stated" (R11), while §4.1 lets
  no line state a derived shape's sentence; DECISION 6.2 found the written
  patch `add Goal holds` rejected with advice to use `initially`. Ruled in
  D-50: a fact changes origin only when its shape changes kind, which only
  a derived patch can do, and 6.2 checks R11 on two images frozen from
  source; the message says rules give a derived shape's facts. 6.2 landed
  (`5e5842b`); open until box 2.8d lands.

- **K15. Template words reserved as entity names.** §9.1 required
  backticks for a name that any template in scope uses as a word, and
  `read` rejected such a filler, so a library shape that says `boot`
  broke every host fact naming the entity `boot`. DECISION 5.7(b) found
  it: 5.7's templates said "startup" and "user session" to dodge it. D-42's
  one typed reading and D-49's declared entities make the rule redundant.
  Ruled in D-51: a template word is a name in a slot. Closed by 2.8e
  (`cd333fe`) and 5.7b (`22b6309`).
- **K16. Pending requests outside the state's definition.** §5.4 makes
  pending requests part of the state and its checkpoint, and 7.9's code
  hashes them oldest first, while §5.1 defined a state as facts, live
  entities and the allocator, and §10.4's hash listed only those. Closed
  in D-55: §5.1 and §10.4 list the pending requests.
- **K17. Two spellings of a type's identity.** §2 names everything a module
  declares `module/name`, and §4.1 a field `<module>/T.f`, but 2.10c landed
  the key `dungeon.Hero`, so `Program::field_relation` converts it to
  `dungeon/Hero.f`. Ruled in D-55: the key is `dungeon/Hero`. Closed by
  2.10e (`0bc86fc`).
- **K18. Settlement by position.** 7.9's review cut (`6323367`) settled a
  request when the receipt's entities of the request's types equal the
  request's in order, so a receipt that orders same-typed slots differently,
  or carries a second entity of a request's type, never settles anything.
  Ruled in D-55: slots match by name, and the receipt names every entity
  slot of the effect or the `settled by` line is an error. Closed by 7.9b
  (`916b4e7`).
- **K19. A quoted key means its raw text.** The record register keeps a
  quoted segment's source text and the Nix printer escapes it again, so
  `"a\"b": 1` builds the key `a\"b`, and `"a{x}": 1` the key `a{x}`, while
  as values both literals decode and interpolate. 4.16's ruling routed
  static keys with escapes through `{"…"}`, a second spelling. Ruled in
  D-56: a quoted segment is a text literal, `{"…"}` is an error, and a
  quoted key emits a quoted Nix key. Closed by 4.16b (`e0aa8e8`).
- **K20. A text block that cannot close inside an expression.** §9.4
  makes a text block a `value-literal`, so it may stand in a call's
  arguments or an inline record, but §9.1 put its closer alone on its
  line, so only a block that ended its line could close. 4.2's lift
  printed firn's beagle module as `text: '''` … `'''})` and its flake
  overlays as `'''), extraInstallCommands: '''`, and the build failed with
  `unterminated text block`. Ruled in D-59: the rest of the closer's line
  continues the block's line. Open until box 4.2a lands.
- **K21. `lines` promised as a type.** §10.2 said `lines` would come as a
  type with the first program that declared one, while D-42 had cut it
  from the option grammar, and as a contract it equals `Text`. Closed in
  D-59: §10.2 says why it is no type, and a `lines` option is a
  `nix file`'s `lib.mkOption` call.
- **K22. A universal single point of failure rejected.** §6.2 makes the
  left side of `S has no single point of failure among P` universal, and
  core plans it so, but the reader reads that left side as a claim of its
  own, whose scan rejects `?n` in `?n holds has no single point of
  failure among ?n has ?b` and offers `every ?n: …`, which §6.2 makes a
  static error. Found by 6.3 (peer file 1768). Ruled in D-60: the error is
  wrong. Open until box 3.11d lands, with 3.11b's guard.
- **K23. A clause's `when` is never safety-checked.** §4.3 and §5.2 make a
  variable that a comparison, boolean expression or binding needs, and
  that nothing grounds, a static error, but core `check` schedules only
  rules, definitions and views: `galileo check` exits 0 on `when ?row.order
  > 0`, and `galileo run` fails at the event with `Type { at: "handler
  clause", message: "?row is not bound" }`. A rule's grounding error gives
  the rule's first line, not the literal's. Found by the owner's probe for
  the D-61 addendum. Open until box 2.21j lands.
- **K24. A backticked name holds any text.** §9.1 says backticks hold
  one bare name, but the lexer takes everything up to the next backtick.
  So `` ({`a b`, ...}) => `a b` `` builds to `{ a b, ... }: a b`, which
  only Nix's parse gate rejects, naming the emitted line, and
  `` `{?n}` `` reads as a name spelled `{?n}`. Found by the owner's
  probe for D-65 (CLI built at `da5ec7c`). Ruled in D-65: anything
  other than one bare name or one `{e}` between backticks is an error at
  its line. Open until box 2.22b lands.
- **K25. A `let` name's interpolation is computed though the `let` is
  emitted.** 2.6c computed every known interpolation. So
  `"${stateDir}/gh"` imported and built as `"/var/lib/nexus-checks/gh"`,
  while `stateDir` alone emitted as a name, against D-39's image of the
  lowering. Closed by D-66 (box 2.23c).

- **K26. `option` lines cannot write grouped declarations.** A `nix
  module`'s `option` lines emitted only `options.P.Q = …;`, yet
  socrates-chat at `4bcfad4c` declares its six options as one
  `options.P = { … };` block, the only grouped block among 66 Beagle
  outputs. It was found by 2.23b's worker once `{...}` landed. Ruled in
  the D-66 addendum: `option P` with declarations under it is a group.
  Open until box 2.23d lands.

- **K27. Two-state claims: whole claim or one sentence.** §6.2 named `before
  S` and `after S` as whole claims, the parser read `before` over a whole
  condition and only in `ensures`, nothing parsed `after`, and §8.3's
  example read one binding in two states, which only a sentence mark can
  write. Found by 6.5's worker (DECISION 6.5). Ruled in D-70: `before`
  marks one sentence, there is no `after`, and each `ensures` claim reads
  one state. Open until boxes 6.5a and 6.5e land.

- **K28. A field form cost more than the sentence it replaced.** D-61
  addendum 3 promised that lowering keeps the menu's IR, so the engine's
  ranked count would see what it saw. It held for the aggregate, but the
  field reads around it lower as `Expr::Field`: `?prior.visible` is no plain
  atom, so `aggregate::expand` plans neither the position nor the count, and
  `?row.order` is no variable threshold for the ranked index. 8.8b measured
  `menu_cost`'s offer at 4,775 and 46,180 work at 200 and 2,000 rows (main
  214 and 219). Ruled in D-85: the engine plans both as the forms the
  bindings spelled, and the IR keeps what was written. 9.11r landed. The
  same held in a step's clause: 8.8b's `accept-menu` walked every row
  (6,027 at 1,000 rows, main 39), ruled in D-85's addendum. Open until box
  9.11t lands.

## Open questions

- **O1. Keywords against templates.** Decide where each keyword is read,
  and check a shape's template against that list when the shape is
  declared, so a collision is one error at the declaration rather than a
  misreading at each use. Start from `and`, `or`, `not`, `in`, `where`,
  `is a`, `requires`, `passes`, `happens when`, `has no single point of
  failure among`. Since D-41 a question is a command's argument, so `why`,
  `trace`, `prevent` and `achieve` are never read inside a module, and
  `via`, `at revision` and `using:` became flags (`--revision` is
  `--as-of` since D-52). Ruled in D-42: a line is
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
- **O4. `galileo check` on a program that does not compile.** Box 3.11 exits
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
    `galileo check` fails when a fact names a path no module sets, so a
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
  and 7.3, so none is open at fa31751. At d8a6824 the lead had settled
  the bench-H3 lines (events files are `*.events`), and D-52 found no
  language line open. A planner that stops without landing its ruling leaves its
  questions here. The lead signed nine rulings as language owner on
  2026-10-11 (peer file 1127 to 1460); D-55 audited them, and D-56 the
  four before them: 7.10 (753, 759), 7.8 (815), 2.19g (899) and 4.16
  (1093). The lead's 7.3 nearest-name ruling (675), a Damerau distance so
  `Ahs` offers `Ash`, is a diagnostic's metric that SPEC leaves open, and
  it stands. No lead-signed language ruling awaits audit through peer
  line 1505. D-59 answers 1634, 1638, 1639 and 1650's build question; the
  other `DECISION` lines through 1657 (8.9's setup cost, which D-58 took,
  5.3's scratch route, the landing protocol and 4.26's commit hook) are
  not language questions. D-60 answers 6.3's finding (1768, 1772).

- **O7. Naming created entities outside a program.** A `state entity`
  has no name, so `galileo run`'s events file cannot name one that an
  earlier event created (D-51). Tasks key created things by a field value
  or a declared entity until a program needs more. Then rule a spelling
  that names an entity by its type and a key field, never by a raw id
  (§10.4 matches entities by creation order, never ids).

- **O8. The menu's mailbox after the mechanical import (8.6, 8.8).** The
  lead ruled, and D-55 confirmed, that the first import keeps the
  `host-effect` and `effect-payload` state shapes, their three accept
  guards and `clear-host-effect` as Clause wrote them, and also emits a
  carrying effect, `at most one per step` and `settled by
  clear-host-effect`, so `pending()` mirrors the mailbox. The end state
  needs a guard that no request is pending, and no Galileo condition reads
  pending requests. The likely answer, not ruled: pending requests are
  already state (§5.1), so a `settled by` shape's sentence read under
  `when` holds for each pending request, and the guard becomes `not ?north
  requests host effect ?_ with payload ?_`, with no new word. Rule it when
  8.8 reaches the mailbox; until then the mailbox and its guards stay.
  D-61 found it again as a workaround G10 names: two mirror fields, a
  receipt handler and two `then` lines where the effect alone is wanted. It
  stays open by D-57: 21 handlers outside the menu slice write the mailbox,
  so it is ruled with the slice that owns the mailbox (8.13), and reading
  pending requests under `when` puts them in the engine's model.

- **O9. Fields of given data (D-61).** `state for T` and `derived for T`
  exist; `given for T`, fields stated as facts (`flake-inputs.order = 1`),
  waits for a program that needs it. `lib/firn/dispatch.gal`'s 47 commands
  of seven facts each are the likely first, as one record per command; rule
  it with that program, with the fact form and the record form as the
  options. D-61 addendum 4 gives each command one focus block, so the
  record form must beat a form that already writes each name once.
- **O10. Aggregates as expressions (D-61 addendum 2).** The sketch's
  `menu-count` and `menu-row-position` bind `?visible` and `?rank` only
  because `count where` is a literal, so a rule head cannot read it: the
  binding G10 names. Ruled in D-61 addendum 3: an aggregate is an
  expression, read by the token after its word. Open until 2.21q and 2.21r
  land.

- **O11. A `for` repeats its query's variables.** A `for` with several
  variables takes a query and names each column twice, `for ?name, ?url in
  all ?n, ?u where …` (flake.gal twice, §9.6's inner loop), or with the
  same names, which already builds the same file (D-61 addendum 3's probe).
  No binding of an expression is forced, so Tom's standing rule does not
  reach it, but G10's idiom scan will. The likely answer, not ruled: `for
  ?name, ?url where C` iterates the bindings of `C` in the variables'
  canonical order, with no `all` and no second list, while `for ?x in e`
  keeps iterating a list. Ruled in D-61 addendum 4 as that answer, the
  rows `all ?name, ?url where C` gives. Open until 2.21s lands.

- **O12. Posts shared across events (D-61 addendum 4, escalated).** The
  sketch's three `offer-*-row` clauses each state the row counter's
  increment, `?north lists ?row` and `order: ?north.menu-row-count`. A
  construct by which one event's step states another's posts changes
  D-42's one atomic transition per event, and one `offer-row` event with
  north computing description and annotation changes 8.9's API and north's
  adapter outside this repository. Tom's proxy ruled the second on
  2026-10-11, and D-61 addendum 5 ruled its form: `offer-row` carries a
  row's four texts and not its kind, which the menu's kind already states;
  events whose clauses make the same posts are one event, and no post
  block is shared (SPEC §5.2). Open until 8.8c and 8.8d land.

- **O13. Dependency requirements over events (D-64).** SPEC §6.2 opens with
  every claim judged over the transition system, but its table gives
  `S requires C` and a single point of failure over a sentence no state,
  and core judges them in one: the initial state, or the one `--events`
  reaches. Measured at `739b9ed`: the dungeon with `Shrine-Door opens for
  Ash requires Ash holds Silver-Key` and a rule opening the door for a
  hero in the Crypt holds vacuously and exits 0, and fails only with
  events that walk Ash to the Crypt. `requires` cannot be an `invariant`
  (D-30). The options: (a) judged in every reachable state through
  search, a universal claim that needs a complete search, as S3 of the
  review record reads §6.2; (b) the initial state, with `always` taking
  a `requires` for every reachable state; (c) `requires` admitted as an
  `invariant`, checked at each step. Likely (a): no event program states
  one today, so nothing breaks, and the bare sentence keeps `always S` as
  its every-state form. Until then 3.11e prints `in the initial state` on
  these verdicts. Rule it with the first event program that states a
  dependency requirement, likely north's or the terminal dungeon's.
  Ruled by Tom in T6 (2026-10-12): (a), the single point of failure the
  same way, SPEC §6.2. Open until 7.4c lands, before 3.11e.

- **O14. Flat option lines after T2.** 15 Beagle modules repeat one
  prefix over 52 flat `option` lines (kanata 5, users 10), which D-61
  addendum 4's rule would write once. Byte identity keeps them flat
  through WP4. When T2's idiomatic layout lands (4.15), decide whether
  flat lines sharing a node become an error offering the group, or stay.

## Who writes Galileo

Agents write Galileo, not Tom. On 2026-10-10, shown the kanata module, the
dependent-responsibility rule and the dungeon facts, Tom ruled through his
proxy: "its whatever the agent wants looks fine to me. im not writing the
config the agent is"; on sentence style, "whatever the agent thinks... PL
design is trending towards"; and on the general-purpose direction (T4),
"look good" (D-36, amended). So the owner decides syntax by what agents
write correctly and what current PL design favours, and Tom approves
direction, not syntax. This is Tom's decision, not open for a ruling, and
there are no exercises for Tom.
