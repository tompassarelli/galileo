# Muove MVP specification

Status: MVP specification, 2026-10-10. It supersedes
`inputs/claude-muove-plan/claude-muove-first-draft-spec.md` wherever they
differ. PLAN.md sequences the work; DECISIONS.md records the choices,
including Tom's (T1–T4), and the review findings.

## 0. How to read this

Each section ends with a **Traces to** line. `F` items are measured firn and
nixos-config usage, `N` items are measured north usage, `P` items come from
building and auditing the Claude prototype, `R` items from re-verifying the
ChatGPT review, and `A` items are references. The evidence ledger is
Appendix A, the references Appendix B, the section-to-evidence matrix
Appendix C. A construct with no trace is not in the MVP (guardrail G1).

## 1. Thesis, direction, targets and guardrails

### 1.1 Thesis

Muove is a language for software that AI writes and people approve, and it
is meant to be general-purpose (§1.2). A program is rules over relations,
state that events change, and requirements. The same executable rules
determine behavior, explain behavior and constrain changes to behavior.

- **Determine.** A Muove program is what runs. In the MVP it emits the Nix
  that NixOS evaluates, and it is the state machine north's host drives.
  There is no second model beside the implementation.
- **Explain.** `why`, `why not` and `trace` are answered from the same
  evaluation, as sentences with source locations, and every answer says how
  complete it is.
- **Constrain.** Requirements are part of the program. A change is reviewed
  against a frozen baseline: what it changes, what follows, which requirements
  it introduces, fixes or leaves broken, and whether it does what its author
  said it would.

Muove is for people who read meaning: AI writes most of the code, and people
approve consequences. So the language is built to be checked rather than
trusted. A line has exactly one reading, or it is an error that names the
closest shapes (G5). An author's description of a change is shown and never
trusted, while its `intends` claims are checked (§8.3). The authoring
reference is `muove help`, whose examples the tests compile (§11.1), because
Beagle's 70 recorded hallucinations show what remembered syntax costs (F13).

### 1.2 Direction

The destination is a general-purpose language (DECISIONS T4). Muove starts
where its ideas are needed today, firn's Nix layer and north's state
machine, and is meant to grow from there into command-line tools, game
logic and other programs, rather than stay a rules layer inside other
languages. Three parts of the MVP already carry over to any program:

- **I/O goes through the host.** The core is pure (G4). A program asks its
  host for effects and gets their outcomes back as events (§5.4). The host
  is a platform in Roc's sense: it decides which effects exist and when the
  program runs (A-Roc). Elm runs whole applications the same way (A-Elm).
  north's Rust host and firn's TypeScript host are the first two platforms.
- **Steps are deterministic.** The same state and event always give the same
  result (§5.3), so any run can be replayed, explained and reviewed, whether
  it configures a machine or plays a game.
- **One reference semantics.** Every faster engine is tested against the
  reference interpreter (G7), so speed never changes meaning.

What the MVP leaves out is not yet, not never. §12 lists each item with what
brings it in, and PLAN.md sketches the first general-purpose milestone after
A11. Constructs still enter one at a time, each with a program that needs it
(G1).

### 1.3 First targets, in order

firn and north come first because they are real systems with measured
baselines, a drvPath and recorded event sequences. Replacing them proves the
language on work that already runs.

1. **firn** (nixos-config). Today two compilers emit the committed `.nix`:
   Beagle (63 `.bnix`, 2,178 LOC, 50 generated `.nix`) and Clause (185 Nix
   modules, 184 tag files and 3 firn programs). Around them sit tag resolution,
   responsibilities, flake-input collection, schema checks and drift checks
   (F1 to F12).
2. **north**. A 4,064-line Clause state machine embedded in a Rust host, and
   `flake.clause` (N1 to N10).

Replaced means, measured:

- **firn.** `firn repo build` invokes neither Beagle nor Clause.
  whiterabbit's `config.system.build.toplevel.drvPath` is unchanged by the
  migration. nixos-config verifies only whiterabbit, and F15 shows no host
  embeds the flake revision, so the drvPath is a sound identity check. For
  ashashi (nix-darwin) and thinkpad-x1e, every generated file is identical
  and their system drvPaths are compared where they evaluate on Linux; that
  comparison is reported but does not gate.
- **north.** Recorded event sequences give the same admissions, effects and
  projections on the Muove runtime as on Clause (§10.4), and north's tests
  pass with the Clause crates removed from `Cargo.toml` (acceptance A11).

### 1.4 Guardrails

Each guardrail is enforced by a named check in PLAN.md.

- **G1.** No construct without a runnable program that needs it: in the MVP
  a firn, north or acceptance example, and after it a milestone program
  (first draft, "no new concept without a runnable example").
- **G2.** Every check answers `Holds(evidence)`, `Fails(counterexample)` or
  `Unknown(reason)`, never a boolean (R5 to R8).
- **G3.** A conclusion only combines facts from one state (R1).
- **G4.** The core is pure. Effects are typed requests that the host performs,
  and their outcomes come back as events (N6, F12).
- **G5.** Every line has exactly one reading against the vocabulary in scope,
  and imports never change the reading of an existing line (P3, R16).
- **G6.** Answers print as sentences with file:line, never as hashes or
  internal ids (first draft).
- **G7.** One reference semantics. Every fast path is differential-tested
  against it (R14, A-Cedar).
- **G8.** Nothing is discarded silently. Overridden, losing or blocked
  definitions stay in provenance (A-Nix priorities, A-Souffle `.override`).
- **G9.** Every bounded answer states its bound and its scope (R12, R13,
  A-Apalache).

Traces to: F1, F2, F12, F13, F15, N1, N4, N6, P3, R1, R5–R8, R12–R16,
A-Cedar, A-Apalache, A-Nix, A-Roc, A-Elm.

## 2. Program structure

- **Files and modules.** A program is a set of `.muove` files under a project
  root: the nearest ancestor of the working directory that holds `.git`, or
  the working directory when none does. Each file is one module, named by
  its path from the root without the extension:
  `modules/kanata/kanata.muove` is `modules/kanata/kanata`, wherever
  `muove` runs, so names and the identities built from them never move. A
  command reads the modules under the paths it is given, the whole project
  by default, and every module they `use`. Each module is read against its
  own vocabulary, and the modules are linked into one Core IR, never
  concatenated. There, everything a module declares is named
  `module/name`, such as `firn/tags/is-tagged`, so adding a module never
  renames anything.
- **Libraries.** `use firn/<name>` names a module of Muove's own library,
  `lib/firn/`, which the `muove` binary carries. A library module joins a
  program only through `use`, so the dungeon never gets firn's
  requirements, and a project file with a library module's name is an
  error.
- **Vocabulary scope.** `use <module>` makes the types, shapes and
  definitions that module exports visible, and `use` is not transitive. A
  sentence is read only against the vocabulary in scope: its own module,
  what it `use`s, and the prelude. A name means the module's own
  declaration of it, and otherwise the one export or prelude name that has
  it; when two have it, using the name is an error that names both,
  whatever the order of the `use` lines. Two shapes in scope that give a
  line two readings make that line an error. So a module carries its own
  sentence vocabulary with exact meanings, and a `use` never changes how an
  existing line reads: the line reads as before, or it is an error that
  names both modules (R16).
- **Names in expressions.** Scopes nest, and an inner name shadows an outer
  one. Outermost is the prelude: §3.5's functions. `sort … by`, `min … by`
  and the `… where` aggregates are syntax, not names. Inside a `nix module`
  or `nix file` body come the names Nix binds in every file: `builtins`,
  and the bare globals that `firn/nix-lib` types, `import` and `toString`.
  Every other Nix global is written `builtins.<name>`, and no bare one takes
  a prelude function's name, so `map` is Muove's. Outside Nix bodies no Nix
  name is in scope. Next come a block's `arguments:` and `let`s, then
  lambda parameters, `for` variables and the arguments of a nested
  `nix module (…)`. A `let` is sequential: the items after it see it, and
  its own value and earlier items do not, so recursion never comes through
  `let` (§3.5). A block binds a name once, its arguments included
  (DECISIONS D-37).
- **Exports.** A module exports its types, shapes and definitions unless
  they are marked `private`, and one that is not `private` may not mention
  a `private` type. A type that an exported shape mentions but another
  module declares is known where the shape is used, and is named there only
  by using its module. Entity declarations, facts, rules, handlers,
  requirements, views and Nix outputs are not exported: they belong to the
  program, whatever uses exist. The MVP has no qualified names.
- **Imports outside the project.** These are pinned by a hash of their
  normalized content in `muove.lock` (A-Dhall). The MVP has none: firn and
  north are each one project.
- **Foreign facts.** Data produced outside Muove, such as the NixOS option
  schema, enters only through a declared source:
  `foreign nixos-options from ./.beagle-cache/schema.json` (§9.4). Each fact
  it provides records where it came from (§3.2, §7.1). The MVP reads one
  format, the option schema JSON that firn extracts with `nix eval` today
  (F9).

Traces to: R16, P3, F9, A-Dhall, A-Unison.

## 3. Values, types, stages and effects

### 3.1 Values

| Value | Notes |
| --- | --- |
| `Bool` | `true`, `false` |
| `Int` | Signed 64-bit. Overflow fails the step or check that hit it, with its location. It never wraps. |
| `Text` | UTF-8. Literals take `{expr}` interpolation; text blocks are raw (§9.1). |
| `Path` | `./x`, `../x`, `/x`. Distinct from `Text`, because Nix treats them differently (F4). |
| `null`, `maybe T` | `maybe T` is `T` or `null` |
| `List T` | ordered |
| records | closed, with named fields |
| `Attrs T` | open maps from text keys to `T` (Nix attribute sets) |
| text unions | `"queued" \| "paused" \| …`, Muove's enums (N3: north encodes enums and booleans as text) |
| `OptionPath` | A sequence of attribute names, written as a dotted literal such as `myConfig.modules.firefox.default` in a slot typed `OptionPath`. It is never split out of a `Text` value. |
| entities | references to nominal entity types: declared, or created by events |
| functions | first-class in expressions; never stored in facts or state |

There are no fractional numbers in the MVP. North's `F64` only stands in for
counters and positions (N3), and firn uses none (F3). They are not yet
(§12): `Int` never converts implicitly, so adding them changes no MVP
program (DECISIONS D-12).

**Attribute names.** In an attrpath segment `{expr}`, a `Text` value is
exactly one attribute name, quoted in the emitted Nix when it needs quotes,
and an entity contributes its name as one attribute name. An `OptionPath`
value contributes all of its segments. So a username key such as `"a.b"`
stays one name (F4), while a tag override's path splices into several (F6).

### 3.2 Two stages: known and Nix

- **Known values** are computed by Muove itself: at build time for firn
  (literals, given facts, rule results over the given model, definitions),
  or at run time inside a session for north (anything read from the current
  state, §4.5). "Known" never means evaluated by Nix.
- **`Nix T` values** are Nix expressions that Muove type-checks and emits but
  never evaluates. This covers anything rooted at a module argument (`config`,
  `lib`, `pkgs`, `inputs` and so on) or at a name the Nix target binds
  (`builtins`, `import`, `toString`; §2), and any call into those.
- Known values lift into `Nix T` implicitly and are emitted as literals.
- A prelude function (§3.5) takes known values and is computed by Muove, so
  `length("abc")` emits `3`. A Nix-stage argument is a stage error that
  names the Nix-side call, such as `builtins.length`; a prelude name is
  never emitted as a bare Nix name.
- **Opaque payloads.** A `Nix T` value may be stored in a fact slot typed
  `nix T`, such as a tag override's value `[pkgs.firefox]`. Rules can bind it
  and pass it on, and emission writes it back out. Nothing can decide on it:
  conditions, comparisons, arithmetic, aggregates and event guards on a
  `nix T` value are type errors that read "this depends on Nix evaluation;
  Muove can't decide it". The one exception is equality of the normalized
  syntax tree, which mode checks and deduplication use.
- So no Muove conclusion, verdict or step ever depends on how Nix evaluates.
- This boundary is the trust boundary. Muove explains what Muove determines,
  and what Nix decides is shown as the Nix that was emitted. Evaluated Nix
  data, such as the option schema, enters only as foreign facts with
  recorded provenance.

`Nix`-stage types come from three places:

1. option declarations in Muove sources;
2. the NixOS option schema that firn already extracts with `nix eval`
   (`.beagle-cache/schema.json`, F9), declared as a foreign source;
3. `firn/nix-lib`, a curated signature module for the `lib` and `builtins`
   functions firn uses (F3 lists them) and for the bare globals the Nix
   target binds (§2). A signature is a `define` without a body, such as
   `define builtins.readFile(file: Path): Text`. Its name is rooted at
   `lib` or `builtins`, or is one bare global that no prelude function
   shares, so a call is always a Nix term and its types are written without
   `nix`. A parameter takes a known or a Nix value of its type, a `Path`
   parameter also takes `Text`, because Nix reads a string as a path, and
   `Any`, the contract that states nothing, takes any value and is written
   only of Nix values (here and in `nix Any`). The checker carries
   `firn/nix-lib`, every `nix module` and `nix file` sees it without a
   `use`, and no other source declares a `define` without a body, which
   would be a forgotten body. Each name is declared once.

A call to a `lib` or `builtins` member that `firn/nix-lib` does not declare
is an error with a did-you-mean, directly or through a `let` alias. A
member that is selected but not called, such as `lib.licenses.mit`, is
unchecked `Nix Any`.

**Nix roots in facts.** A fact is outside every Nix body (§2). In one,
`config`, `lib`, `pkgs`, `inputs` and the names the Nix target binds are
Nix roots. A `nix T` slot reads its payload over exactly those names, with
its names and calls checked, and any other slot rejects an expression
rooted at one as a stage error. A name such as `flakeRoot` is a root only
inside the declaration whose `arguments:` lists it, so no file changes how
another file's fact reads. A dotted name fills only an `OptionPath` slot.

**The option tree.** `config` in a `nix module` has its module system's
option tree: the options that Muove sources declare, together with the
declared foreign schema. Where both declare a path, Muove's declaration
wins. A read such as `cfg.devics` and an entry such as
`myConfig.modules.kanata.enabel: true` walk the tree the same way. Under a
closed node, an unknown name is an error with a did-you-mean and the
declaring line; under an open node, names pass unchecked.

- With a schema, every node down to a declared option is closed.
- Without one, a node is closed only when a Muove source declares an option
  directly in it, such as `myConfig.modules.kanata`. So
  `config.services.kanata.enable` passes until a schema is declared, and a
  node that also holds foreign options, such as `services.foo` once Muove
  adds `services.foo.extra`, needs the schema.
- Past a declared option, the rest of a path is a key into its value, such
  as `main` in `services.kanata.keyboards.main`. A computed segment `{…}`
  ends the walk, and the entries of a `nix file` value are attribute keys,
  not option paths.
- A nested `nix module (…)` has its own module system, so Muove's
  declarations do not apply to its `config` (§10.2).

A `pkgs` attribute missing from the schema's package index is a warning that
names the index's revision.

### 3.3 Types are contracts

A type is a contract the checker proves before anything runs, the same kind
of plain checked statement as a mode (§4.1) or a requirement (§6), checked
earlier (DECISIONS T5's principle 14, D-42). `{room: Room}` states that every
value of that slot is a Room, which is the relation `?r is a Room` (§4.1);
`exactly one room per hero` states a functional dependency; a requirement
states what evaluation must find. There is no hierarchy and no subtyping.

- **Structural** for records, lists and attribute sets. **Nominal** for
  entity types: a type is a kind of entity, and its members are a relation.
- **Text unions** for closed choices. `maybe T` for optional values.
- **One spelling.** An option is declared with the same types as a slot
  (`List Text`, `maybe Port`), and §10.2's table says what each emits. The
  Nix target adds one contract, `Port`, an `Int` between 0 and 65535
  (A-Pkl, A-KCL).
- **Inference** is local and bidirectional. Definition parameters and shape
  slots are annotated; everything else is inferred.
- **Null checks narrow.** In `if`, in an entry's or a block's `when`, and in
  `config when`, a conjunct `E != null` of the condition gives `E` its
  non-null type in the guarded part, and `E == null` does so in the `else`
  branch, where `E` is a name or a field path such as `cfg.port`. The right
  operand of `and` sees its left operand's narrowing; `or`, `not` and a
  `let`-bound condition narrow nothing. Values never change, and `when`
  lowers to `lib.mkIf`, which evaluates its value only when the condition
  holds, so this holds in both stages. Comparing a value that can never be
  null with `null` is an error, because the answer is fixed.
- **Why slots stay annotated.** A slot's type picks the entity a name means
  and which template a line matches (§4.2, §9.2), so it must be known before
  any model exists; a requirement cannot stand in for it. The Nix target
  reads types to choose `++` or `+` (§9.4), to print option types (§10.2),
  and to keep `nix T` values out of decisions (§3.2).

### 3.4 Effects (inferred, never written)

Every definition, rule and handler has one inferred effect level:

- `total`: computation on known values;
- `staged`: builds Nix terms;
- `reads state`: rules over state relations;
- `transition`: event handlers;
- `emits`: handlers that emit effect requests.

Hover shows the level, and review uses it. For example: "this patch changes
only total definitions, and they are used by 3 rules." No Muove code performs
I/O. Files are read only at compile time, for imports and declared foreign
sources, and those reads are recorded in provenance (A-Koka: effect rows,
inferred without annotations).

### 3.5 Totality

- **No recursion.** Definitions may not be recursive, directly or mutually.
- **Iteration** goes through built-ins: `map`, `filter`, `fold`, `sort by`,
  `join`, `count`, `sum`, `min`, `max`, `min … by`, `max … by`.
- **Text built-ins** cover what north's laws and handlers call (N1, N3):
  `lowercase(Text): Text`, `contains(Text, Text): Bool`,
  `starts-with(Text, Text): Bool`, `trim`, `words(Text): List Text`,
  `first-word`, `rest-words`, `split(Text, Text): List Text` and
  `length(Text): Int`, plus interpolation and `++`. They work on Unicode
  code points. Case mapping and word splitting behave exactly as Clause's
  functions do today, so north's menu matches do not change at cutover. PLAN
  box 8.4 records Clause's behaviour on empty, ASCII and non-ASCII text as
  the reference.
- **Partial operations return `maybe`:** `first`, `lookup` and division.
  Integer overflow is the only runtime error.

So every expression terminates, rules terminate (§4.3), and every step
terminates (§5.3) (A-Dhall: totality is safe, but keep programs small and
fast; A-Formulog: a typed functional sublanguage called from rules).

Traces to: F3, F4, F6, F9, N1, N3, P5, A-Koka, A-Formulog, A-Dhall,
A-Pkl, A-KCL, A-Nix.

## 4. Relations, facts and rules

### 4.1 Shapes

A shape declares a relation. It has:

- a kind: `given`, `state`, `derived`, `event` or `effect`;
- a sentence template with typed slots;
- a stable name, written as `name:` before the template, or derived from the
  template's words (`{hero} is in {room}` is named `is-in`);
- optional modes and an optional `milestone` flag.

```text
state {hero: Hero} is in {room: Room}
  exactly one room per hero
```

**Modes:**

- `exactly one X per K`: a total function. Every live `K` has one `X` in
  every state. `K` must be a single entity slot. A multi-slot key takes
  `at most one`, because "every combination of live entities" is almost
  never what an author means.
- `at most one X per K`: a partial function.
- No mode means a many-valued set.

A shape may carry several mode lines, and each is checked on its own. These
are north's 120 `one`, 15 `maybe` and 17 `many` relations (N1).

**Kinds:**

- `given` facts are stated in source and never change.
- `state` facts start under `initially` and change only through events.
- `derived` relations are defined only by rules. They are recomputed for
  every state, and are never stated or written by events.

**Entity relations.** Each entity type `T` has a built-in shape
`{entity: T} is a T`, named `is-a-T` and exported with `T`. For a `type` it
is given, and its facts are the entities of `T`: each `X is a T`, and each
entity a fact names in a slot of type `T`, cited at that fact's line. For a
`state entity` it is a state relation that holds the live entities: a
handler's `?x is a new T` includes it, and its `not ?x is a T` withdraws it
with the entity's other facts (§5.2).

**Entity records** are sugar for one functional state relation per field.
`?x is a new T {field: value, …}` must set every field whose type is not
`maybe`, and `not ?x is a T` removes them all. Only `state entity` types
are created and deleted, with or without fields. Given facts cannot mention
them, so a deletion never leaves a given fact dangling:

```text
state entity MenuRow
  key: Text
  label: Text
  order: Int
```

Fields are read in expressions as `?row.label`. This replaces north's 8
hand-maintained MenuRow relations and its hand-written teardown (N3).

Each field `f` of `T` is the state relation `T.f` (IR name
`<module>/T.f`), whose two slots are the entity and the value, with mode
`exactly one f per T`, or `at most one` when the field's type is `maybe`.
`?x is a new T {…}` includes one fact per field it gives (a `maybe` field
given `null` or left out has none), and `?x.f` reads the fact in the state
being evaluated, `null` when a `maybe` field has none. A field is fixed for
the entity's life: no sentence names `T.f`, so no `then` line writes it,
and a value that changes is a state shape of its own, as §9.7 makes a
row's visibility and position derived relations (T5's principle 15). `X is
a T` for a `state entity` is an error, since its entities come only from
`new`.

`muove import clause` makes a Clause `one` field `exactly one` only when
every site that creates the entity sets it in the same step. Otherwise the field becomes
`maybe`: north creates about 20 Command referents with no data (N3).

### 4.2 Facts

- **A fact** is a ground sentence of a `given` shape, or of a `state` shape
  under `initially`.
- **Entities.** `X is a T` declares one, and a fact also introduces each
  entity it names in a slot of an entity type: `firefox is tagged browsers`
  makes `firefox` a Module and `browsers` a Tag. An entity is its type and
  its name, so one name may name entities of several types, as firn's
  `python` names a module and a tag. The slot's type picks one, and where
  nothing gives the type, a name of two types is an error that lists
  them.
- **Focus blocks** (a subject line with its predicates indented under it) and
  **list slots** are sugar. `whiterabbit enables [terminal, rust]` states one
  fact per element, one level deep, and an empty list there is an error,
  since the line would state nothing. The slot's declared type decides: in a
  slot typed `nix T` or `List T` the list is one value, so
  `[pkgs.firefox]` is one payload (§3.2) and a `List Text` slot holds a
  plan's arguments as one list. Only facts take the sugar; in a rule, a
  query, a `then` line or a claim a list is one value and fits only a slot
  whose type admits it.
- **Duplicates.** Stating the same fact twice is a warning. Stating facts
  that break a mode is an error that shows both lines.

### 4.3 Rules

A rule has a name, a head sentence of a derived shape and a `when` body. The
body literals are:

- a positive sentence, `?x is a T` included (§4.1);
- `not` followed by a sentence; `not` followed by an expression is a boolean
  expression literal, holding when the expression is `false`;
- a comparison: `== != < <= > >=` on `Int` and `Text`, and `== !=` on
  `Bool`, text unions and entities;
- a boolean expression, such as `contains(lowercase(?label), ?query)`. The
  literal holds when the expression is `true`;
- a binding `?x = expr`, where `expr` is total;
- membership: `?x in ?list`;
- an aggregate: `?n = count where <body>`, or `?n = sum|min|max <expr> where
  <body>`. It ranges over the distinct bindings of its body's local
  variables (see scopes below): `count` counts them, and `sum`, `min` and
  `max` combine `<expr>` over them, so two rows with equal costs both
  count. `sum` adds `Int` values exactly and checks the `Int` range once, at
  the end, so the result never depends on order. `min` and `max` take `Int`
  or `Text` and order them as `<` does. An empty `sum` or `count` is 0; an
  empty `min` or `max` binds nothing, so the literal fails;
- an argmin or argmax: `?r = min ?x by <expr> where <body>` binds `?r` to
  the `?x` with the smallest `<expr>`, an `Int` or `Text` ordered as `<`
  orders them, and `max ?x by` to the one with the largest, as Python's
  `min(xs, key=…)` does. Ties go to the smaller `?x` in canonical value
  order, so the result is deterministic. With no candidate it binds nothing, so the literal
  fails. North writes this today as `sum … = 0.0` FIFO-head workarounds
  (N3).

**Scopes.** A rule, a handler clause, a claim, a view and a definition each
own their variables, and a definition or view sees its callers only through
its parameters. Each aggregate, argmin and query opens a nested scope. A
name that also appears at the top level of its owner, or of any scope
enclosing it, is outside the nested scope: it is shared, and must be
grounded before the nested scope runs. Every other name is local, so two
sibling aggregates can each have their own `?prior`. A declared variable
(`all ?x`, `the ?x`, `min ?x by`) may not reuse an outside name. In the
record register a `for` block is one more enclosing scope: a query inside
it reads the `for`'s variables as outside variables, already grounded. The
checker, the reference interpreter and the engine use this one scope
function.

Three static checks. Each failure is an error that names the offending
literal or cycle.

- **Safety, by a grounding schedule.** A positive sentence grounds its
  variables. A binding `?x = e`, a membership `?x in e`, an aggregate or an
  argmin grounds `?x` once every outside variable it reads is grounded.
  Every variable must end up grounded, and the variables of a `not`, a
  comparison or a boolean expression must be grounded before it is tested.
  Body order does not matter, but a cycle such as `?x = ?y` with
  `?y = ?x` never grounds, so it is an error. Every definition and view is
  checked as its own owner, whether or not anything calls it: only its
  parameters are grounded when its body starts.
- **Stratification.** A stratum is one strongly connected group of
  relations, counting everything a rule reads, through definitions, named
  queries and head expressions included. Strata are numbered by longest
  path, so every dependency between strata points down. `not`, aggregates
  and queries refer only to relations in lower strata. A query used in a
  rule, directly or through a definition, counts as an aggregate over every
  relation it reads. A cycle through any of them is an error that prints the
  cycle.
- **No value invention in recursion.** In a rule whose head shares a
  strongly connected component with a positive body sentence, every head
  slot must be a constant, a variable grounded by a body sentence (directly
  or through `?y = ?x`, which gives `?y` the safety of `?x`), or a value
  computed only from variables grounded by relations of lower strata. This
  applies to bindings, memberships, aggregates, calls, field reads and head
  expressions alike, and keeps every model finite.

### 4.4 Semantics

- **The model.** Static facts plus a state `S` mean the perfect (stratified
  least) model `M(S)` of the rules. It is unique and finite (A-Souffle,
  A-Flix, A-Ascent).
- **Mode checks.** Modes on derived relations are checked on `M(S)`. Two
  values for one key give `Fails`, carrying both proofs: the conflict is
  reported, never kept as two facts (A-egglog). firn's flake-input URL
  conflict is exactly this: `derived {name: Text} flake input url {url:
  Text}` with `at most one url per name` (F8). `muove build` refuses to emit
  anything while a derived mode Fails.
- **When modes are checked.** A relation's modes are checked as soon as the
  relation is complete: stated facts when they load (§4.2), state facts at
  each step (§5.3), and a derived relation when its stratum is done.
  Evaluation stops at the first stratum that fails a mode, so nothing reads
  a relation whose mode Fails. Each conflicting key of that stratum Fails
  with its two smallest tuples and a proof of each, and so does each live
  key of an `exactly one` mode that has no value. `muove check` reports them
  and answers its claims `Unknown`, naming the conflict.

### 4.5 Queries as values

`all <vars> where <body> [, sorted by <expr>]` returns the distinct bindings
of its variables: a list of values when it names one variable, and
otherwise a list of records whose fields are the variables' names
(`all ?p, ?v` gives `{p, v}` rows). Without `sorted by`, rows come in the
canonical order of their variables, compared in the order the query names
them. A sort key reads only the query's variables and the variables it
shares with its owner, never one that only its condition binds, and ties
keep the canonical order. `the ?x where <body>` returns the one binding, or
`null` when there is none. It type-checks only when modes fix `?x` from
constants and outside variables: a sentence whose relation has a mode fixes
its value slot once its key slots are fixed, and a binding, aggregate or
argmin fixes its variable once what it reads is fixed. Its type is `T` when
every mode in that chain is `exactly one`, and `maybe T` otherwise.

A query is evaluated where it appears:

- **In a `nix module`, a `nix file`, or a definition they call:** over
  `M(given)`. Such a query may read only given relations and derived
  relations that do not depend on state; anything else is a stage error.
  This is how rule results feed emitted Nix: generated enables and flake
  inputs (§10.3).
- **In a rule, handler, `view` or requirement:** over `M(S)` for the current
  state. A query that reads state relations has effect level `reads state`.
  A view's parameters are bound before its queries run.

A view's `for {north: North}` declares a slot, which its body reads as
`?north`, as a handler reads its event's slots, so in a sentence a variable
is always `?name` and a bare name is always an entity. A `define`'s
parameters are names, read bare in its expression body like `let` and
lambda parameters; a `?` variable there may not reuse one, and a sentence
slot reaches a parameter as a parenthesized expression, `(index)`. Each
view field carries the type the checker infers (a `the` query by §4.4's
rule, `all ?x` as `List T`, `?x.f` by the field's type), so bindings and
JSON output infer nothing, and a field that cannot be typed is an error
naming it.

The default order is the canonical value order:

- `false` before `true`, and `null` before any other value;
- `Int` numerically;
- `Text` and paths by code point;
- entities by name, then by creation order;
- option paths and lists element by element, a prefix first;
- records field by field, in field-name order;
- `nix` payloads by their normalized text.

So emission is deterministic.

### 4.6 Milestones

A derived shape marked `milestone` is a stable domain milestone.
Derivation-dependence changes are reported only for milestones, so
refactoring internal rules makes no review noise (R3 and the review's advice
to "expose stable domain checkpoints"). The word is not `checkpoint`, which
names a saved runtime state (§10.4).

Traces to: F6, F7, F8, N1, N2, N3, P2, R3, A-Souffle, A-Flix, A-Ascent,
A-egglog, A-Datafun.

## 5. State, events and effects

### 5.1 States

A state `S` is a finite set of state facts, the set of live entities, and
the entity allocator (the next fresh id). Ids are never reused, so the
allocator is part of the state's identity, its hash and its checkpoint.
Derived facts are always relative to one state, in `M(S)`.

No construct joins facts from two different states. This is the fix for the
dungeon flaw (R1): "Ash can reach Hall" and "Ash holds Bronze-Key" combine
only if both hold in the same state.

### 5.2 Events and handlers

An event shape declares a transition with typed arguments and a stable name:

```text
event move: {hero: Hero} moves through {door: Door}
```

A handler `on <event sentence>` has one or more clauses. Same-named handlers
are clauses of one event, run as one occurrence (N2). Each clause is its own
scope, as a rule is (§4.3): its `on` line names each of the event's
arguments with its own variable, distinct within the line, and two clauses
may name them differently. A constant, a repeated variable, `?_` or an
expression on an `on` line is an error, and a value is tested under `when`.

A clause is one transition stated as a relation between before and after,
as a rule relates its body to its head: `when` says what holds before, and
`then` what holds after. Nothing in a clause is a command, and neither its
lines nor the clauses of an event have an order (DECISIONS D-42). Each
clause has:

- **`when`**: rule-body literals, evaluated against `S` and `M(S)` (the
  pre-state).
- **`then`**: what the step makes true, at least one line, one line each,
  with every expression evaluated on the pre-state:
  - a sentence of a state relation holds afterwards. When the relation has
    exactly one mode, the sentence replaces the value at that mode's key:
    each pre-state fact that the sentence and the mode together contradict
    is withdrawn, so `then ?hero is in ?there` moves the hero under
    `exactly one room per hero`. Without a mode the fact is added. With two
    or more modes the line does not say which key it replaces, so the fact
    is added and step 5 checks every mode: under `exactly one tile per
    unit` and `at most one unit per tile`, a move says `not ?u is on ?here`
    beside `?u is on ?there`, and a move onto an occupied tile rejects
    rather than silently taking the tile from its unit;
  - `not S`, for a state sentence `S`, withdraws `S`;
  - `?x is a new T` creates one fresh entity of the `state entity` `T` per
    substitution, written `?x is a new T {field: value, …}` when `T` has
    fields, and gives every field whose type is not `maybe`. `?x` is bound
    by nothing else in the clause;
  - `not ?x is a T` deletes the entity and every state fact that mentions
    it. `?x` is bound by the `on` line or `when`, and `T` is a `state
    entity`;
  - a sentence of an effect shape is a request to the host (§5.4).
  A `then` line reads only variables that the `on` line binds, that a
  `when` literal grounds (a sentence, `?x = e`, `?x in e`, an aggregate,
  `min`/`max … by` or `?x is a T`; §4.3), or that a `new` line of its
  clause binds, in any order. `not`, comparisons and boolean expressions
  ground nothing.
- **`ensures`** (optional): state claims (§6.3) checked on the post-state,
  where `before S` reads the pre-state. North checks these by hand today, in
  12 Rust `Err(Protocol)` postconditions (N4).
- **`may skip`** (optional): admits the empty step when no clause matches.
  It belongs to the event: every clause of the event says it, or none
  does, and a mismatch is an error naming both clauses.

### 5.3 Step semantics

For one occurrence `e(args)` on state `S`:

1. **Match.** Evaluate every clause's `when` on the pre-state and collect
   every substitution: all matches contribute (N2). Action expressions are
   also evaluated on the pre-state.
2. **No match.** If no clause matched, the step is `Rejected(no match)`, with
   a why-not for each clause. With `may skip`, the empty step is admitted
   instead.

   Clause admits the empty step silently today (N2), which is why north
   re-checks postconditions in Rust. `muove import clause` therefore marks
   every imported handler `may skip`, so the import keeps Clause's
   behaviour. The LSP lists those markers so they can be removed one event
   at a time, each with a replay check.
3. **Collect.** Gather, over every clause and substitution at once:
   - `W`: withdrawn facts: each `not S`, and each pre-state fact that a
     sentence of a relation with one mode replaces;
   - `I`: included facts;
   - `C`: created entities. Ids come from a per-state counter in the
     canonical order of the entities' types and field values, then of their
     clauses' normalized text (§8.1) and substitutions, so moving or
     reordering clauses never renumbers an entity;
   - `D`: deleted entities;
   - `E`: effect requests, a set: two equal requests are one, and the host
     receives them in canonical value order (§4.5), never in clause order.
     An effect whose order matters carries that order as a value, as Elm's
     `Cmd.batch` promises no order (A-Elm).
4. **Post-state.** `S' = ((S \ W) \ facts(D)) ∪ I`. The live entities become
   `(live(S) \ D) ∪ C`, and the allocator moves past `C`. Withdrawals apply
   before inclusions, so a fact that is both withdrawn and included is
   present afterwards. This is Dedalus's rule, and it is what makes
   `then not X` beside `then X`, or a sentence that replaces a value with
   itself, a no-op rather than a conflict.

   Two exceptions are errors, not silent wins:
   - Including a fact that mentions an entity that is not live in `S'`
     rejects the step. That covers an entity deleted in the same step.
   - Deleting an entity that is also created in the same step is a static
     error.
5. **Check, in order.** Any failure rejects the whole step, leaves `S`
   unchanged, and comes back with an explanation in sentences.
   1. State modes hold on `S'`. Two different values for one functional key
      reject, which is N2's "overlapping writes reject the step". Every
      `exactly one` slot of every live entity must be filled.
   2. Derived modes hold on `M(S')`.
   3. Program invariants hold (requirements marked `invariant`, §6.3).
   4. The clause's `ensures` holds.
6. **Admit.** Otherwise the step is admitted. It gets revision `r+1` and state
   hash `h(S')`, and `E` goes to the host.

A sequence `run [e1 … en]` is one atomic admission. Intermediate states stay
local, and any rejection rejects the whole sequence (N2).

**Determinism.** The same `S` and the same occurrence always give the same
`S'` and the same `E`. There is no clock and no randomness: the host passes
time and outside identifiers as event arguments.

**Formal anchor.** The post-state is Dedalus's: a `then` sentence is
`p@next`, `not` is the exception to persistence, and positive rules apply
inside one timestep (A-Dedalus, A-Statelog). A moded sentence's replacement
is persistence limited by the relation's one mode: a fact persists unless
the step contradicts it under that mode. This is PostgreSQL's `INSERT … ON
CONFLICT (key) DO UPDATE`, which names one key and rejects a conflict on
any other, and not SQLite's `REPLACE`, which deletes every row that
conflicts on any key (A-Upsert). The correspondence covers the
post-state only. Admission (the no-match rule and the checks of step 5) is
Muove's own, and a rejected step has no successor. Users never write
`@next`.

### 5.4 Effects

An effect shape declares a request the host performs:

```text
effect send: send {prompt: Text} to Codex for {turn: Turn}
```

1. A handler's `then` names effect sentences.
2. The runtime returns them with the admitted step.
3. The host performs them and reports each outcome as an ordinary event (a
   receipt), such as `event finished: Codex finished {turn: Turn} with
   {status: TurnStatus}`.

This replaces north's single-slot host-effect mailbox (N3) and matches its
contract: transition, authorized effect, receipt, settlement (N6). Replay
never performs an effect again, because receipts are in the log. firn's
command plans, today `Plan: steps: Sequence<Execution>` in
`native/firn.clause` (F12), become effect requests on the same terms.

An effect shape can add two lines:

```text
effect send: send {prompt: Text} to Codex for {turn: Turn}
  at most one per step
  settled by finished
```

- **`at most one per step`**: two different requests of this shape in one
  step reject the step, as conflicting writes do. `muove import clause`
  declares north's mailbox effect this way, so a step that Clause rejected
  for two competing effects still rejects (N2, N3).
- **`settled by <event>`**: an admitted occurrence of that receipt event,
  whose entity arguments equal the request's, settles the oldest pending
  request. Requests not yet settled are pending. They are part of the state
  and its checkpoint, and after a restore `Session::pending()` lists them,
  so the host can reconcile them under its own contract (N6). Muove never
  performs an effect itself.

### 5.5 Transition systems and temporal questions

A program with events defines a transition system: the states reachable from
the initial state by admitted steps.

**Argument domain.** An entity slot ranges over the live entities of its
type, and a text-union slot over its values. An `Int` or `Text` slot is open
unless its event closes it:

```text
event switch-mode: {north: North} switches to {mode: Text}
  mode only in ["normal", "insert"]
event type-query: {north: North} types {query: Text}
  query sampled from ["", "fi", "zz"]
```

- **`only in`** closes the slot. The runtime rejects any other value, so a
  search over it can be complete.
- **`sampled from`** leaves the slot open, and the search tries only those
  values. A witness found that way still proves an existential claim. While
  any event has an open slot, a universal or negative claim is
  `Unknown(open domain: <slot>)`, with the sampled result attached as
  evidence. A slot with neither line is open with no samples.

Every search answer records the event alphabet it used. A program that
creates entities can have unboundedly many states, because ids are never
reused (§5.1); its searches then end on a budget and say so.

**Search.** Temporal requirements (§6.2) and questions are answered by
explicit-state breadth-first search with canonical state hashing, under
budgets for states, depth and work.

**Scope.** Every answer carries one (G9):

- "in this state";
- "along these N traces";
- "across all N reachable states (complete)";
- "explored N states to depth D (incomplete)".

**Completeness.**

- A witness trace proves an existential claim even when the search is
  incomplete.
- Universal and negative claims need a complete search. A bounded search
  that finds nothing is `Unknown`, not `Holds` (A-Alloy6, A-Apalache).

### 5.6 The dungeon, restated (acceptance scenario S1)

The dungeon is rewritten with explicit state. Moving and picking up are
events, so no conclusion can combine incompatible journeys.

```text
type Hero, Room, Door, Key, Trap

given {door: Door} leads from {here: Room} to {there: Room}
given {door: Door} is unlocked
given {door: Door} is locked by {key: Key}
  at most one key per door
given {trap: Trap} is armed in {room: Room}
given {trap: Trap} is telegraphed

state {hero: Hero} is in {room: Room}
  exactly one room per hero
state {key: Key} lies in {room: Room}
  at most one room per key
state {hero: Hero} holds {key: Key}
state {trap: Trap} warned {hero: Hero}
state {trap: Trap} hurt {hero: Hero}

derived {door: Door} opens for {hero: Hero}

event move: {hero: Hero} moves through {door: Door}
event pick-up: {hero: Hero} picks up {key: Key}
event strike: {trap: Trap} strikes {hero: Hero}

rule unlocked-doors-open
  ?door opens for ?hero
  when
    ?door is unlocked
    ?hero is a Hero

rule held-keys-open
  ?door opens for ?hero
  when
    ?door is locked by ?key
    ?hero holds ?key

on ?hero moves through ?door
  when
    ?hero is in ?here
    ?door leads from ?here to ?there
    ?door opens for ?hero
  then
    ?hero is in ?there

on ?hero moves through ?door
  when
    ?hero is in ?here
    ?door leads from ?here to ?there
    ?door opens for ?hero
    ?trap is armed in ?there
    ?trap is telegraphed
  then
    ?trap warned ?hero

on ?hero picks up ?key
  when
    ?hero is in ?room
    ?key lies in ?room
  then
    not ?key lies in ?room
    ?hero holds ?key

on ?trap strikes ?hero
  when
    ?trap is armed in ?room
    ?hero is in ?room
  then
    ?trap hurt ?hero

requirements for the level
  eventually Ash is in Vault
  Ash is in Vault only after Ash is in Library
  eventually Ash is in Vault has no single point of failure among ?key lies in ?room
  ?trap strikes ?hero only when ?trap warned ?hero
  never Ash is in Shrine
```

(Entity declarations, doors and `initially` are as in the prototype's
`dungeon.clause`. Ash starts in Entrance, the Bronze-Key lies in Library and
the Iron-Key in Crypt.)

**Expected answers.** These match an independent `(room, keys held)` search
(R1). The prototype got the third and the Silver-Key placements wrong.

| Requirement or question | Answer |
| --- | --- |
| `eventually Ash is in Vault` | Holds. Shortest witness, 5 steps: through Front-Door, through Library-Arch, through Crypt-Stair, picks up Iron-Key, through Crypt-Door |
| Rooms in some reachable state | Entrance, Hall, Library, Crypt, Vault. The Armory is unreachable: its gate needs the Bronze-Key, and there is no way back from the Library to the Hall |
| `… only after Ash is in Library` | Holds (complete) |
| `… has no single point of failure among ?key lies in ?room` | **Fails.** Without "Iron-Key lies in Crypt", no reachable state has Ash in Vault. The prototype showed ✓ |
| `?trap strikes ?hero only when ?trap warned ?hero` | Holds |
| `never Ash is in Shrine` | Holds |
| Silver-Key placements that open the Shrine | Entrance, Hall, Library, Crypt. The prototype's 6 included the Armory and the Vault |

**Reviews** (§8):

| Proposal | Verdict and reason |
| --- | --- |
| `secret-passage` | Reject: "only after Library" fails |
| `pitfall` | Reject: a strike happens without a warning |
| `crowbar` | Reject: "never Shrine" and "only after Library" fail |
| `library-ladder` | Approve, and it **fixes** the single-point-of-failure requirement |
| `armory-collapse` | Changes no reachable behaviour, but is not approved, because the baseline failure persists. With `--allow-baseline-failures` it gets "approve: no new failures" |

Traces to: R1, R2, R17, N2, N3, N4, N6, F12, A-Dedalus, A-Statelog,
A-Upsert, A-EventCalculus, A-Alloy6, A-Apalache, A-telingo.

## 6. Requirements and verdicts

### 6.1 Verdicts

A verdict is one of three values:

- **`Holds(evidence)`**: the evidence is a proof tree, a witness trace or a
  completeness record of what was searched.
- **`Fails(counterexample)`**: a proof of a forbidden fact, a trace, or the
  binding that fails.
- **`Unknown(reason)`**: which budget ran out and where, or which construct
  this question does not support yet.

How a verdict is decided:

- An existential claim may Hold on a witness before its search finishes.
- A universal or negative claim Holds only after a complete search.
- A counterexample Fails at once.
- Every inner computation's completeness flows into its verdict, including
  counterfactual re-evaluations (R8).
- Each claim's verdict depends only on the program and that claim: it is
  evaluated on the program plus its own auxiliary rules, never on a model
  augmented for every claim. Its counterfactual re-evaluations draw from
  the same work budget as the model it reads, and the first that runs out
  makes it `Unknown`, naming the counterfactual.
- A well-formed claim of a form the engine does not evaluate yet is
  `Unknown(unsupported)`, and stays with its claim. A form SPEC does not
  define is a static error.
- Budgets are charged inside every loop, before intermediates are built
  (R12).
- A work unit is one elementary step, charged before the step builds
  anything: a fact loaded, a tuple scanned, a literal tested, a call made,
  a value built into a list, record, tuple or call, an element that a
  membership or a built-in visits or that `++` or `//` copies, or a row or
  head tuple produced. A value grows only by units charged, so real cost
  stays proportional to the units. The reference interpreter and the
  engine charge the same kinds of step, so their costs compare, and a
  budget gives the same verdict on every machine (DECISIONS D-37).

`muove check` exits 0 when every claim Holds and 1 when any Fails. It exits
2 otherwise: when none Fails but some are Unknown, and when it reaches no
verdict because of a static error, unreadable input or bad usage. A static
error prints to stderr with file:line and no verdict is printed; static
checks cover the whole program, requirements included, before any claim is
evaluated. A warning, such as a fact stated twice (§4.2), prints and the
check goes on.

### 6.2 Requirement forms

Every claim is evaluated over the program's transition system. A program
without events, such as all of firn, has exactly one state, so there `S`,
`always S` and `eventually S` coincide, and `never S` means `S` is false.

**Quantifiers (R15).** Every variable a claim reads must be bound, by one
of:

- `some`, `every` or `no`;
- the form itself. In `S requires C`, `S has no single point of failure
  among P`, `S only after C` and `E only when C`, the variables of the left
  side are universal: the claim is checked for every binding under which it
  holds or occurs. Variables that appear only in `C` or `P` are pattern
  variables. In `never S` they are existential; `eventually S` and
  `always S` bind nothing; in `ensures` and `before`, the clause's
  variables are bound (§5.2);
- a definition: `?x = e`, an aggregate or `min`/`max … by`, each of which
  gives at most one value, so it needs no quantifier and is visible to the
  rest of its claim. A membership `?x in e` gives one value per element, so
  it binds nothing here: a bare `?x in [a, b]` would be R15's implicit
  `some`.

A variable that appears only inside an aggregate or query is local to it
(§4.3), and each `?_` is a fresh variable, existential where it stands. Any
other variable is an error: in a sentence or a quantified claim it offers
`some`, `every` or `no`, and elsewhere it names the form, such as
"`eventually` takes a sentence without variables". One core function
decides this for the checker and for lowering.

**One sentence a side.** The `S` and `C` of `S requires C`, and the `P` of
`… among P`, are each one sentence. `S requires A and B` reads as two
necessary conditions, so it is written as two requirements, `S requires A`
and `S requires B`, and the conjunction is an error that shows them.

**Vacuity.** A universal claim with no instances Holds, and its evidence is
marked vacuous. Review reports when a claim becomes vacuous (ChatGPT's
"making an obligation vacuous").

| Form | Meaning |
| --- | --- |
| `S` | the ground sentence holds in the initial state |
| `some ?x…: S` / `no ?x…: S` | some or no binding holds in the initial state |
| `every ?x… where A: B` | `B` holds for every binding of `A`, in the initial state |
| `eventually S` | some reachable state satisfies `S` |
| `always S` | every reachable state satisfies `S` |
| `never S` | no reachable state satisfies `S` (with `S`'s variables existential) |
| `S requires C` | Counterfactual dependence. For every binding under which `S` holds, blocking every fact that matches `C` under that binding (removed, and not derivable) makes `S` false. For monotone cones this equals "every derivation passes through `C`" (`requires_iff`, P1). Under negation it is counterfactual. |
| `S has no single point of failure among P` | For every binding of `S` that holds, `S` still holds after removing any one stated fact that matches `P` |
| `S only after C` | for every binding of `S`'s variables, every path from the initial state to a state where `S` holds under it visits a state where `C` holds under it, at or before. `C`'s other variables are existential in that state |
| `E only when C` | every admitted occurrence matching event pattern `E` has `C` in its pre-state, under the occurrence's binding of `E`'s variables. This replaces the stated "trap is telegraphed" fact (R2) |
| `<temporal claim> has no single point of failure among P` | `P` ranges over initial facts, and the claim must still hold with any one of them removed |

`has no single point of failure` applies to a sentence or a temporal claim;
over a quantified claim, `requires` or another such claim it is a static
error.

**Two-state claims** (`before S`, `after S`) appear only in patch contracts
(§8.3, A-Dafny) and `ensures`; in a requirement they are a static error.

### 6.3 Where requirements live

- **Placement.** `requirements` blocks can appear in any module. They are
  part of the program, so they are part of its baseline.
- **Checking.** `muove check` checks them, and review checks them on both
  sides of a change.
- **Invariants.** A requirement marked `invariant` is also checked at every
  runtime step (§5.3). Only **state claims** can be invariants or `ensures`:
  a sentence, or a `some`, `no` or `every` claim, evaluated on one state. A
  temporal claim depends on which steps are admitted, so using it to decide
  admission could contradict itself. Marking one `invariant` is an error
  that points to the handler's `when`, which is where event preconditions
  go.

### 6.4 Strength

When review says a claim got weaker or stronger, it measures the **minimum
failure set**: the smallest set of stated facts matching the declared
pattern whose removal makes the claim fail. It never measures the number of
supports.

- **Monotone cones.** The minimum failure set is a minimum hitting set of
  the claim's minimal supports (`minimal_prevention_iff`, P1).
- **Under negation**, removing a fact can enable another derivation or let
  a blocked `not` through, so hitting sets are not even the right
  candidates: with `s when x(?n)`, `x(?n) when a(?n), not b`, `b when not c`
  and the facts `a(1)`, `a(2)` and `c`, removing `c` alone breaks `s`. A
  cone is non-monotone when the target reaches a `not`, aggregate or query
  edge through any rule, fired or not. Every subset of the pattern's
  stated facts is a candidate, smallest first, verified by recomputing the
  model under the claim's one work budget; the first size with a verified
  set is the minimum, and every verified set of that size is reported. A
  verified set at a size whose smaller candidates all failed decides the
  minimum; a search that then runs out reports the sets it verified, marked
  incomplete. A search that runs out before deciding is `Unknown`.
- **Edge cases.** A claim that already fails has a minimum failure set of
  size 0. A claim that no removal within the pattern can break is reported
  as unbreakable within that pattern.

R4 is why. Supports went from {A}, {B} to {A,C}, {B,C}, {D,C}: the support
count rose from 2 to 3 while C became a single point of failure. Review
prints: "Three supports remain. Every one now depends on C. The smallest
failure set fell from 2 facts to 1."

Traces to: R2, R4–R8, R12, R15, R17, P1, P3, P4, F7, F8, A-Alloy6,
A-Apalache, A-MOLLY, A-Dafny.

## 7. Explanation

### 7.1 `why S`

- **One proof tree**, of minimal height first. During evaluation, each derived
  tuple records the rule that produced it and its height, and the tree is
  rebuilt only when asked (A-Souffle lazy provenance; R18: the prototype
  enumerated every support for every fact).
- **Leaves** are of four kinds:
  - stated facts, with file:line;
  - absence checks for `not`, with their scope, which names where the
    sentence would have to come from. For a given relation, foreign ones
    included, it is the files that state that relation's facts about the
    same first argument: "nothing states `whiterabbit disables kanata`
    (checked: `hosts/whiterabbit/tags.muove`)", and with no such file,
    every stated fact of the program. For a derived relation it is
    "nothing derives" and the rules that conclude it, with file:line; for
    a state relation, the current state. The scan reads only that
    relation's rows and charges each one;
  - computed comparisons and bindings;
  - foreign facts, with their source. For example: "from `schema.json`,
    extracted at <revision>".
- **Invariant.** A leaf shown as stated is stated. If reconstruction runs out
  of budget, the answer is `Unknown`, never an invented leaf (R10).
- **`--all`** enumerates the minimal supports, lazily and bounded. A
  support is the stated facts and absence checks of one derivation, minimal
  under inclusion over both, so the fact holds wherever they all hold
  (A-KR2026's Proposition 1), and each leaf prints as `why` prints it. A
  derived relation's absence stays one leaf, which `why not` explains. A
  cone that reaches an aggregate or query is `Unknown(unsupported)` until
  firings record what those read. Deciding whether a fact belongs to
  why-provenance is NP-complete (A-KR2026), so this is never the default.
- **`--precise`** adds source spans (A-why-depends).
- **Overridden definitions** stay in the tree, marked "overridden by
  <definition> at file:line" (G8).

### 7.2 `why not S`

- For each rule that could conclude `S`, it shows the closest binding: the one
  with the fewest failing conditions.
- Each condition is marked met, missing or blocked. A failing `not` names the
  fact that blocks it.
- Missing derived conditions are explained recursively, to depth 2. This is
  the prototype's `NearMiss` (P6).
- `muove why not S --via <rule>` fixes the rule (A-Souffle
  `explainnegation`).

### 7.3 State facts and traces

- **`muove why S --revision r`** explains a state fact by the event
  occurrence that included it and that event's `when` proof in its
  pre-state, then states that nothing withdrew it since (A-EventCalculus
  inertia).
- **`trace S`** prints a witness trace as event sentences.
- **`replay <trace>`** runs a trace through the runtime and confirms `S`, so
  every reported witness is executable (the ChatGPT review's "make the proof
  do something").

### 7.4 `prevent` and `achieve`

Both are interventions over a declared vocabulary, the relations that
`--using` names. Each takes a claim (§6.2), so §5.6's Silver-Key placements
are `muove achieve possible eventually Ash is in Shrine --using lies-in`:

- **`prevent C`** finds minimal withdrawal sets after which `C` fails.
- **`achieve C`** finds minimal addition sets after which `C` holds.

A candidate counts when `C`, evaluated on the changed facts under the
claim's one work budget (§6.1), Fails for `prevent` or Holds for
`achieve`; a candidate that comes out `Unknown` makes the answer `Unknown`
if a smaller answer could depend on it.

How they are computed depends on the cone:

- **Monotone cones** use the exact characterisations the Lean file proves:
  `prevent_iff`, `minimal_prevention_iff` and `achieve_iff`. They are
  machine-checked on Lean 4.29.1 with no `sorry` (P1).
- **Non-monotone cones** use verified search: each candidate is applied and
  the program recomputed before it is reported. The search is bounded, and
  hitting the bound gives `Unknown`.

`achieve` has two modes:

- **`possible`** is diagnostic and ignores requirements.
- **`acceptable`** finds the minimal addition sets after which `C` and every
  requirement Hold (R17). Minimality is taken among those sets, so when
  `{a}` achieves `C` but breaks a requirement and `{a, b}` achieves it
  acceptably, `{a, b}` is an answer.

### 7.5 Output

Answers are sentences rendered from the declared templates, with file:line
(G6). Every bounded answer says so (G9).

Traces to: R3, R10, R17, R18, P1, P6, F6 ("firn tag resolve is the
debugger"), A-Souffle, A-KR2026, A-why-depends, A-EventCalculus, A-OPA
(residual "what must hold" is post-MVP).

## 8. Change review

### 8.1 Frozen baseline

- **Freezing.** The baseline's program image is parsed once and frozen. It is
  a git revision for firn and north, or a named model.
- **Identity.** Its identity is a hash of its normalized definitions.
  Normalization removes only what cannot be observed: formatting, comments,
  the names of every scope-local variable (of a rule, clause, claim, view,
  query or aggregate) and the order of sentence-register declarations
  (D-42). Order inside a `nix module` or `nix file` body reaches the
  emitted Nix, so it counts. Names of entities, shapes,
  definitions and exports, and every label that reaches emitted Nix or a
  view, are part of the identity (A-Dhall, A-Unison).
- **Hashes stay internal.** They appear only in machine-readable review
  records, never in rendered output (G6).
- **Requirements.** A requirement's name is its claim as `muove fmt`
  prints it, without `invariant`: a label, not an identity. Review pairs
  requirements by their lowered claims up to variable names, with the
  `invariant` flag, since one text can be two claims against two
  vocabularies (R16). A requirement repeated with the same lowered claim is
  a warning and is lowered once, as a repeated fact is (§4.2).

### 8.2 Patches

A patch is one of two things:

- **Derived**, from two program images, such as the git base and the working
  tree.
- **Written** as a `patch` block in a restricted delta language, with three
  section words, each naming what it touches:
  - `add` facts and entity declarations, and `add rule <name>`;
  - `remove` facts (each a baseline-stated fact) and `remove <name>` for a
    rule or definition;
  - `replace rule|definition|handler <name>` and `replace requirements
    [for …]`, each with the whole new block beneath.
  One word per edit: a patch removes a fact as it removes a rule.

Three restrictions keep the baseline fixed (R9: today a proposal can install
a rule in its own baseline):

1. A patch cannot contain declarations outside those sections.
2. It cannot redefine a baseline name except through `replace`.
3. It is parsed against the frozen baseline's namespace.

### 8.3 Contracts

**`says:`** text is shown and never trusted. This idea comes from the
prototype.

**`intends:`** lists checked claims about the change:

- `S becomes true`, `S becomes false`, `S stays`;
- `only <pattern> changes`: every change in truth matches the pattern;
- `requirements unchanged`;
- two-state claims using `before` and `after` (A-Dafny). For example: "every
  module enabled before on ?host and needed by a responsibility is enabled
  after".

A failed intended claim rejects the patch. This catches a change that keeps
every requirement but does not do what it set out to do.

### 8.4 What review computes

1. **The edits**, as sentences.
2. **Truth view.** Derived facts gained and lost. A fact that moved between
   stated and derived is reported as "now stated" or "now derived", not as
   lost (R11).
3. **Support view.** The minimum failure set before and after (§6.4), for
   each requirement's subject and each milestone.
4. **Milestone view.** Derived facts that newly bypass a declared
   milestone. R3's case had equal fact support but a different derivation
   dependence.
5. **Requirement verdicts** on the baseline and on the patched program,
   compared by **failing instance**: the claim under one binding of its
   universally quantified variables (or the claim itself when it has none),
   identified by its sentences. Each instance is introduced, fixed or
   pre-existing. A requirement that keeps failing but gains a failing
   instance has an introduced failure. If either side's failing instances
   were not enumerated completely, the comparison is `Unknown`.
6. **Programs with events** also get:
   - replay-corpus differences: the projections of recorded sequences before
     and after;
   - a bounded product search for the shortest event sequence after which the
     two programs differ in a declared view (a behavioural diff witness;
     A-Cedar, A-semver-checks).
7. **firn** also gets per-host consequences: modules turned on or off,
   option values set in Muove, generated files and flake inputs.

### 8.5 Verdict

| Verdict | When |
| --- | --- |
| **approve** | every requirement of the patched program Holds, every intended claim Holds, and every requirement edit is declared |
| **reject** | any requirement or intended claim Fails on the patched program. Each failure is labelled introduced or pre-existing. |
| **unknown** | anything else. It never approves (exit 2). |

Approval needs every requirement of the patched program to Hold, so a patch
that fixes every baseline failure and introduces none is approved normally.
`library-ladder` is the example (§5.6). When failures remain after the
patch, the best verdict is "approve: no new failures". It needs
`--allow-baseline-failures`, and every failing instance of the patched
program must already fail in the baseline. The output lists those
instances (R6).

A derived patch, such as a git diff, has no declarations. Any requirement
change in it is listed first, and approval then needs
`--allow-requirement-changes`. So a change can never redefine success
silently.

Traces to: R3, R4, R6, R7, R9, R11, R20, P4, F6, F13, A-Cedar,
A-semver-checks, A-Dafny, A-Dhall, A-Unison.

## 9. Syntax

### 9.1 Lexical rules

- **Encoding and layout.** UTF-8 with LF line ends. Each indentation level is
  2 spaces, and a tab is an error.
- **Comments.** `#` starts a comment at the start of a line or after
  whitespace. It never does so inside a text literal or a text block (P4: in
  the prototype, `"Fixes issue #12"` displayed as "Fixes issue").
- **Variables** are `?name`; `?_` is anonymous.
- **Names.** Entity names are tokens such as `Front-Door`, `whiterabbit`,
  `kanata` or `MenuRow`. A name that collides with a template word or a
  keyword in scope is written in backticks. Backticks work in every position
  that takes a name, binders included, and hold one bare name:
  `` let `when` = true `` binds `when` (DECISIONS D-34).
- **Text literals** are `"…"`. Escapes are `\" \\ \n \t \{`, and `{expr}`
  interpolates a `Text`, an `Int` (in decimal, computed by Muove) or a Nix
  value, which Nix coerces as it does in `"${…}"`. A known `Bool`, list,
  record, `null` or `maybe` value is an error, as in Nix.
- **Text blocks** open with `'''` at the end of a line and close with `'''`
  alone on its own line, exactly 2 spaces deeper than the line that opens
  the block (L1); a closer at any other depth, or content left of it, is an
  error, so a forgotten closer never swallows the lines after it. Content is
  dedented to the closing marker's column and is raw: no escapes and no comments. `{{expr}}` interpolates. The value is
  every content line followed by a newline, which matches Nix `''` strings
  (F4).
- **Paths** are `./x`, `../x` and `/x`.
- **Integers** are written in decimal. A literal such as `1.5` is reserved
  for fractional numbers (§12) and is an error in the MVP.

### 9.2 Two registers, fixed by block headers

- **Sentence register:** shapes, facts, rules, handlers, requirements,
  patches, and the sentences and claims that commands ask about.
- **Record register:** `nix module` bodies, `view` bodies and inline record
  values.

The enclosing header decides the register, so a line's meaning never depends
on guessing it.

In the sentence register, each slot is filled by exactly one token: a
variable, a name, a literal, a bracketed list or record, or a parenthesized
expression, followed by any `.name` and `(…)` suffixes, written without
spaces, as in `lib.mkDefault(true)` or `?row.label` (balanced brackets
count as one token). The token is parsed with §9.4's `expr` when the line is
read, so a malformed one is an error at its line, and an expression with an
operator is parenthesized: `(?a + 1)`. So `?m is enabled on ?h by
default through ?t` can never be read as the shorter template `… by {tag}`
with a three-word tag.

A line must have exactly one typed reading against the vocabulary in scope,
keywords included (§9.4). If it has several, the error lists them, each
qualified by its module. If it has none, the error lists the closest shapes
(P3).

### 9.3 Muove is not English

Templates are declared mixfix relations, in the style of `_leads from_to_`.
The author declares every phrase, and a line that does not match exactly one
template is an error, not a guess (A-Inform7: "naturalness is an illusion";
A-Attempto: one deterministic reading). Explanations are rendered from the
same templates, which is why the output reads as sentences. Hover always
shows the structured reading, such as `leads(door: Front-Door, here:
Entrance, there: Hall)`.

### 9.4 Grammar summary

```text
file        = { use | decl }
use         = "use" module-path
decl        = type-decl | shape | entity-decl | fact | focus-block | initially
            | rule | handler | define | view | requirements
            | foreign | nix-module | nix-file | patch
foreign     = "foreign" "nixos-options" "from" path
type-decl   = "type" Name { "," Name }
            | "type" Name "=" text-literal { "|" text-literal }
shape       = kind [ "milestone" ] [ name ":" ] template
              [ INDENT { shape-line } DEDENT ]
kind        = "given" | "state" | "derived" | "event" | "effect"
template    = { word | "{" slot ":" type "}" }
shape-line  = mode-line | domain-line | "at most one per step" | "settled by" name
mode-line   = ("exactly one" | "at most one") slot "per" slot { "," slot }
domain-line = slot ("only in" | "sampled from") list
entity-decl = "state" "entity" Name [ INDENT { field ":" type } DEDENT ]
initially   = "initially" INDENT { sentence | focus-block } DEDENT
rule        = "rule" name INDENT sentence "when" INDENT { literal } DEDENT DEDENT
literal     = sentence | "not" sentence | expr cmp expr | expr
            | "?" var "=" expr | "?" var "in" expr | "?" var "=" aggregate
aggregate   = "count" "where" conj
            | ("sum" | "min" | "max") expr "where" conj
            | ("min" | "max") "?" var "by" expr "where" conj
conj        = literal { "and" literal }
claim       = [ quant vars [ "where" conj ] ":" ] conj
            | ("eventually" | "always" | "never") conj
            | sentence "requires" sentence
            | claim "has no single point of failure among" sentence
            | conj "only after" conj
            | sentence "only when" conj
quant       = "some" | "every" | "no"
vars        = "?" var { "," "?" var }
handler     = "on" sentence INDENT [ "when" INDENT { literal } DEDENT ]
              "then" INDENT post { post } DEDENT
              [ "ensures" INDENT { claim } DEDENT ] [ "may skip" ] DEDENT
post        = sentence | "not" sentence | "?" var "is a new" Name [ record ]
define      = "define" name "(" params ")" ":" type [ INDENT expr DEDENT ]
view        = "view" Name "for" slot-list INDENT { field ":" expr } DEDENT
requirements= "requirements" [ "for" words ] INDENT { ["invariant"] claim } DEDENT
nix-module  = "nix module" INDENT "arguments:" args { module-item } DEDENT
nix-file    = "nix file" INDENT [ "arguments:" args ] { "let" name "=" expr }
              "value" ( ":" expr | INDENT { entry } DEDENT ) DEDENT
args        = name { "," name } [ "," "..." ]
module-item = "let" name "=" expr
            | "option" attrpath ":" ( type | "enable" expr ) [ INDENT { opt-field } DEDENT ]
            | "imports:" expr
            | "config" [ "when" expr ] INDENT { entry } DEDENT
            | for-block
for-block   = "for" vars "in" expr INDENT { entry } DEDENT
entry       = attrpath ":" expr [ "when" expr ]
            | attrpath [ "when" expr ] INDENT { entry } DEDENT
            | for-block
attrpath    = seg { "." seg }      seg = ident | text-literal | "{" expr "}"
expr        = value-literal | path | ident | "?" var | list | record
            | "(" expr ")" | expr "." ident | call | lambda
            | "if" expr "then" expr "else" expr | expr binop expr
            | "not" expr | "with" expr ":" expr | query
            | "nix module" "(" args ")" INDENT { entry } DEDENT
value-literal = integer | text-literal | text-block | "true" | "false" | "null"
binop       = "or" | "and" | cmp | "//" | "+" | "-" | "*" | "++"
cmp         = "==" | "!=" | "<" | "<=" | ">" | ">="
list        = "[" [ expr { "," expr } ] "]"
record      = "{" [ attrpath ":" expr { "," attrpath ":" expr } ] "}"
query       = "all" vars "where" conj [ "," "sorted by" expr ]
            | "the" "?" var "where" conj
call        = expr "(" [ expr { "," expr } ] ")"
lambda      = "(" params ")" "=>" expr
params      = param { "," param }
param       = name [ ":" type ]
            | "{" name [ "=" expr ] { "," name [ "=" expr ] } [ "," "..." ] "}"
type        = "maybe" type | "nix" type | Name { type-arg }
type-arg    = Name | "(" type ")"
```

A `define` without a body is a signature, and only `firn/nix-lib` declares
one (§3.2).

A question is not a declaration. `muove why`, `why not` and `trace` take a
ground sentence, `prevent` and `achieve` a claim, and `query` a query, each
read as a line of a module that uses every module of the program, with no
private declaration in view. Their options are flags: `--via`, `--revision`
and `--using`.

A `nix module` is a `nix file` whose value is a NixOS module (§10.2). A
lambda with one brace parameter emits a Nix lambda with formals, such as
`{ self, nixpkgs, ... }: …`, and a formal written `name = expr` is one with
a default: `{hostname, system = "x86_64-linux", extraModules = [], ...}`
emits `{ hostname, system ? "x86_64-linux", extraModules ? [ ], ... }`
(`flake.nix:56`, `:228`). A default reads only names outside the lambda,
and may not mention a name spelled like any formal of its lambda, even one
an outer binding also has, because Nix reads the formal there:
`let x = "outer"; ({x, y = x}) => y` is an error naming `x`. DECISIONS
D-35 records the rulings below, and D-42 the T5 audit's cuts.

**Keywords never shadow templates.** A line is read every way the grammar's
keywords and the templates in scope can split it, and exactly one complete
typed reading must remain (G5, §9.2). The literal kinds are readings like
any other: `?x = …`, `?x in …`, `not …` and a boolean expression are tried
beside every template, and no word's presence in some template decides a
line's kind. When no reading remains, a line with a bare word that is not a
keyword gets §9.2's closest shapes. A `when` line holds one literal: a
depth-0 `and` that no `where` precedes and no template reading uses would
separate two, which is an error asking for one literal per line, or for
`?x = (?a and ?b)` when the boolean is meant. So a template may use any word,
`and`, `requires` and `only when` included: `backup requires capability gpu`
in a `requirements` block claims a sentence of firn's
`{responsibility} requires capability {capability}`, because no split at
`requires` leaves a sentence on each side, and a line that two readings
type-check is an error that shows both (notebook K10, O1).

**Operators**, from loosest to tightest: `or`; `and`; prefix `not`; the
comparisons; `//`; `+ -`; `*`; `++`; then field access and calls. `and`,
`or`, `+`, `-` and `*` group to the left, and `//` and `++` to the right.
Comparisons don't chain, so `a < b < c` is an error. `< <= > >=` compare
two `Int`s or two `Text`s, `Text` by code point (§4.3, §4.5); in a Nix body
they lower to Nix's, whose byte order on UTF-8 is the same. `not a == b` is
`not (a == b)`, and `not` is never the bare operand of a tighter operator:
`a == not b` is an error that asks for `a == (not b)`.
`if`, `with`, lambdas and queries reach as far right as they can, so
`a + if c then 1 else 2 + 3` adds `a` to the whole `if`. `++` joins both
lists and text; the emitter picks Nix `++` or `+` by type, and when neither
operand's type is known it is an error asking for a typed operand, since
either Nix operator would be a guess. A Muove name that is a Nix keyword
(`if`, `then`, `else`, `let`, `in`, `with`, `rec`, `inherit`, `assert`,
`or`) cannot name a Nix binding or reference: it is an error at its line,
never renamed.

**Reserved characters.** A `-` inside a word belongs to the name
(`kanata-main`), so subtraction is written `count - 1`. A `-` that begins
a token is always the operator, and where an operand is due it is an error,
because negation is not in the MVP. Outside text, paths and comments,
`! ? % & ^ ~ @ $ ; \` are errors, except in `!=` and as a variable's
leading `?`. They are kept for later operators.

**Inline records** are written `{a: 1, b.c: "x", {?k}: v}`. Their keys are
attrpaths, which nest and merge as entry keys do. Like a list or a call, an
inline record stays on one line and takes no `when`; a conditional field is
written as an indented entry.

**Field access** takes a name, bare or in backticks: `cfg.port`,
`` x.`when` ``. A key that is not a name (`x."a b"`) and a computed key
(`x.{e}`) are not in the MVP. A known map is read with `lookup`, which
returns `maybe` (§3.5).

**Where a condition ends.** A `where` reaches as far right as it can: to
the end of the line, to the first `,` outside the brackets it opened, or to
a closing bracket it did not open. That holds for queries, aggregates and
`min … by` alike, and an entry's `when` suffix is split off before its value
is read. So `count(all ?m where ?m is on, sorted by ?m)` needs no
parentheses, and a query inside a larger expression does:
`(all ?x where …) ++ ys`. Inside a condition, `and` separates literals, and
a nested `where` takes the rest of its condition. An `or` beside a
separating `and` must be parenthesized, as `(a or b) and c` or
`a or (b and c)`, because in an expression `and` binds tighter than `or`.

**Types.** `type` is written in slots, entity fields, `define` and lambda
parameters, and after `option`, where §10.2's table says what each emits;
there is one type grammar and one spelling of each type (`Text`, never
`text`). `option P: enable e` takes any `Text` expression, a literal or a
`let` name. A written type outside this grammar is a parse error, and a
name that is not a type is a checker error; neither becomes an unchecked
type. Function types and record types are not in the MVP, and the grammar
leaves `(…) => T` and `{…}` free for them.

### 9.5 First win: `modules/kanata` in Muove

`modules/kanata/default.muove` emits today's `modules/kanata/default.nix`
byte for byte:

```text
nix module
  arguments: config, lib, pkgs, ...
  option myConfig.modules.kanata.enable: enable "Kanata keyboard remapping"
  option myConfig.modules.kanata.devices: List Text
    default: []
    description: "Input device paths for kanata to capture. Find yours with: ls /dev/input/by-id/"
  option myConfig.modules.kanata.configFile: Path
    description: "Path to kanata .kbd config file"
  option myConfig.modules.kanata.port: maybe Port
    default: null
    description: "TCP port for kanata server (e.g. for glide integration)"
  option myConfig.modules.kanata.extraArgs: List Text
    default: []
    description: "Extra command-line args for kanata (e.g. --debug, --log-layer-changes for diagnosing layer/spacebar seizures)"
  imports: [./kanata.nix]
```

`modules/kanata/kanata.muove` emits today's `modules/kanata/kanata.nix`,
including Beagle's `((cfg: { … }) config.myConfig.modules.kanata)` encoding
of `let`, under the `beagle` layout (§10.2):

```text
nix module
  arguments: config, lib, pkgs, ...
  let cfg = config.myConfig.modules.kanata
  config when cfg.enable
    hardware.uinput.enable: true
    services.udev.extraRules: '''
      KERNEL=="uinput", MODE="0660", GROUP="uinput", OPTIONS+="static_node=uinput"

      '''
    users.groups.uinput: {}
    users.users.kanata
      isSystemUser: true
      group: "kanata"
      extraGroups: ["input", "uinput"]
    users.groups.kanata: {}
    services.kanata
      enable: true
      package: pkgs.unstable.kanata
      keyboards when cfg.devices != []
        main
          devices: cfg.devices
          port: cfg.port when cfg.port != null
          extraDefCfg: "process-unmapped-keys yes"
          extraArgs: cfg.extraArgs
          config: builtins.readFile(cfg.configFile)
    systemd.services.kanata-main.serviceConfig when cfg.devices != []
      DynamicUser: lib.mkForce(false)
      User: "kanata"
```

What Muove checks that Beagle did not:

- **Field names.** `cfg` has type `config.myConfig.modules.kanata`, so
  `cfg.devics` is an error: "`devics` isn't an option of
  `myConfig.modules.kanata`. Did you mean `devices`? (declared at
  default.muove:4)".
- **Types.** `cfg.port != null` type-checks because `port` is `maybe Port`.
- **Staging.** `builtins.readFile(cfg.configFile)` is `Nix Text`, so it can be
  emitted but never turned into a fact.

### 9.6 Module metadata and host tags as facts

Metadata sits in the same file as the module as plain facts, so it never
reaches Nix. Today it is removed by text surgery (F5).

```text
# modules/firefox/default.muove
use firn/tags

firefox is tagged browsers
browsers sets myConfig.modules.firefox.default to true for firefox
firefox needs flake input nur from "github:nix-community/NUR"

nix module
  arguments: config, lib, pkgs, ...
  option myConfig.modules.firefox.enable: enable "Enable Firefox browser"
  option myConfig.modules.firefox.default: Bool
    default: true
    description: "Set Firefox as the default browser via MIME types"
  imports: [./firefox.nix]
```

```text
# hosts/whiterabbit/tags.muove
use firn/tags

whiterabbit is a Host
whiterabbit runs linux
whiterabbit enables [terminal, cli-tools, theming, auth, javascript, python, rust]
whiterabbit enables browsers
whiterabbit adds [chrome, qutebrowser, zen-browser] to browsers
whiterabbit enables desktop
whiterabbit drops mako from desktop
whiterabbit adds river to desktop
whiterabbit disables [anytype, bun, cargo, clang]
```

The vocabulary and the resolution algorithm of `docs/tags-composition.md`,
as rules (F6):

```text
# firn/tags.muove
type Module, Tag, Host, Platform

given {module: Module} is tagged {tag: Tag}
given {module: Module} can opt into {tag: Tag}
given {host: Host} enables {tag: Tag}
given {host: Host} drops {module: Module} from {tag: Tag}
given {host: Host} adds {module: Module} to {tag: Tag}
given {host: Host} disables {module: Module}
given {host: Host} runs {platform: Platform}
  exactly one platform per host
given {module: Module} supports {platform: Platform}
given {platform: Platform} allows every module
given {tag: Tag} sets {path: OptionPath} to {value: nix Any} for {module: Module}
derived {module: Module} is supported on {host: Host}
derived {module: Module} is enabled on {host: Host} by {tag: Tag}
derived {module: Module} is enabled on {host: Host} by default through {tag: Tag}
derived milestone {module: Module} is enabled on {host: Host}
derived {host: Host} gets {path: OptionPath} set to {value: nix Any} by {module: Module}
  at most one value per host, path

rule enabled-by-default
  ?module is enabled on ?host by default through ?tag
  when
    ?host enables ?tag
    ?module is tagged ?tag
    not ?host drops ?module from ?tag

rule default-membership-enables
  ?module is enabled on ?host by ?tag
  when
    ?module is enabled on ?host by default through ?tag

rule enabled-by-opt-in
  ?module is enabled on ?host by ?tag
  when
    ?host enables ?tag
    ?host adds ?module to ?tag
    ?module can opt into ?tag

rule supported-everywhere
  ?module is supported on ?host
  when
    ?module is a Module
    ?host runs ?platform
    ?platform allows every module

rule supported-by-platform
  ?module is supported on ?host
  when
    ?host runs ?platform
    ?module supports ?platform

rule enabled
  ?module is enabled on ?host
  when
    ?module is enabled on ?host by ?tag
    ?module is supported on ?host
    not ?host disables ?module

rule tag-overrides
  ?host gets ?path set to ?value by ?module
  when
    ?module is enabled on ?host
    ?module is enabled on ?host by default through ?tag
    ?tag sets ?path to ?value for ?module

requirements for tags
  every ?host, ?module, ?tag where ?host adds ?module to ?tag: ?module can opt into ?tag
  every ?host, ?module, ?tag where ?host adds ?module to ?tag: ?module is supported on ?host
```

firn's platform policy is facts too: `linux allows every module`, and
`X supports darwin` for each of the 34 modules in today's
`config/darwin-modules.json`. So on ashashi, which runs darwin, a tag
enables only the modules that support darwin, as `native/tag_resolve.bjs`
filters today, and `why not` names the missing support where firn drops
the module without a word (G8). The second requirement catches the one
case where that drop hides a mistake: a host that adds a module its
platform does not support.

The generated enables module comes straight from the rules. Today there is
an intermediate generated source and a second compile (F6).

```text
# hosts/whiterabbit/_generated-enables.muove
use firn/tags

nix module
  arguments: config, lib, pkgs, ...
  for ?module in all ?m where ?m is enabled on whiterabbit
    myConfig.modules.{?module}.enable: lib.mkDefault(true)
    for ?path, ?value in all ?p, ?v where whiterabbit gets ?p set to ?v by ?module
      {?path}: lib.mkDefault(?value)
```

The inner loop emits tag overrides, such as `firefox.default`, right after
their module's `enable`, in the order today's file uses. `{?path}` splices
several attribute names because `?path` is an `OptionPath`, and
`{?module}` is one name because `?module` is an entity (§3.1). A `for` with
several variables takes a query written in place and binds the query's
variables in the order the query names them, here `?path` to `?p` and
`?value` to `?v`. Any other list is iterated with one variable.

Overrides apply only through default memberships, as `docs/tags-composition.md`
specifies. Two overrides of one path on one host break the `at most one`
mode, so `muove build` refuses to emit and shows both proofs. Today the
first override silently wins (G8).

`muove why firefox is enabled on whiterabbit` prints:

```text
firefox is enabled on whiterabbit                                    ← enabled
├─ firefox is enabled on whiterabbit by browsers                     ← default-membership-enables
│  └─ firefox is enabled on whiterabbit by default through browsers  ← enabled-by-default
│     ├─ whiterabbit enables browsers        hosts/whiterabbit/tags.muove:7
│     ├─ firefox is tagged browsers          modules/firefox/default.muove:4
│     └─ nothing states: whiterabbit drops firefox from browsers   (checked: hosts/whiterabbit/tags.muove)
├─ firefox is supported on whiterabbit                               ← supported-everywhere
│  ├─ firefox is a Module                    modules/firefox/default.muove:4
│  ├─ whiterabbit runs linux                 hosts/whiterabbit/tags.muove:5
│  └─ linux allows every module              config/platforms.muove:3
└─ nothing states: whiterabbit disables firefox                      (checked: hosts/whiterabbit/tags.muove)
```

kanata, by contrast, is not tag-enabled. whiterabbit sets it in its host
module (`configuration.bnix:30`). For such cases, `why` answers from the
option assignment that Muove emitted: "`myConfig.modules.kanata.enable` is
set at hosts/whiterabbit/configuration.muove:30, unconditionally". Any
`mkIf` condition is Nix-stage, so it is shown as emitted Nix and never
decided.

### 9.7 North: the menu slice in Muove

North's menu is `north.clause` L447–827: 13 handlers, 8 north-scoped
relations and 8 MenuRow relations (N10). In Muove, the menu count, row
visibility and row position become derived relations. That removes the
`count-menu` and `filter-menu` maintenance handlers that today run after
every transition (N3). The row teardown becomes `not ?row is a MenuRow`, and the
`F64` arithmetic becomes `Int`:

```text
use north/core

state entity MenuRow
  key: Text
  label: Text
  description: Text
  annotation: Text
  order: Int

state {north: North} has menu kind {kind: Text}
  exactly one kind per north
state {north: North} has menu query {query: Text}
  exactly one query per north
state {north: North} has menu selection {index: Int}
  exactly one index per north
state {north: North} lists {row: MenuRow}

derived {row: MenuRow} matches the menu query of {north: North}
derived {north: North} has menu count {count: Int}
  exactly one count per north

rule matching-rows
  ?row matches the menu query of ?north
  when
    ?north lists ?row
    ?north has menu query ?query
    contains(lowercase("{?row.label} {?row.description} {?row.key}"), lowercase(?query))

rule menu-count
  ?north has menu count ?count
  when
    ?north is a North
    ?count = count where ?row matches the menu query of ?north

define wrap(index: Int, count: Int): Int
  if index < 0 then count - 1 else if index >= count then 0 else index

event move-menu: {north: North} moves menu selection by {delta: Int}
event clear-menu-rows: {north: North} clears menu rows

on ?north moves menu selection by ?delta
  when
    ?north has menu count ?count
    ?count > 0
    ?north has menu selection ?selected
  then
    ?north has menu selection wrap(?selected + ?delta, ?count)
  may skip

on ?north clears menu rows
  when
    ?north lists ?row
  then
    not ?row is a MenuRow
  may skip

view Menu for {north: North}
  kind: the ?k where ?north has menu kind ?k
  query: the ?q where ?north has menu query ?q
  selection: the ?i where ?north has menu selection ?i
  rows: all ?row where ?row matches the menu query of ?north, sorted by ?row.order
```

`may skip` keeps Clause's behaviour on an empty menu, where moving the
selection is a no-op rather than a rejection (§5.3).

This is the end state, not the first import. `muove import clause` first
imports the slice mechanically: every handler is marked `may skip`, and the
`count-menu` and `filter-menu` maintenance handlers stay handlers that run
in Clause's order inside the same atomic sequence. Replacing each one with a
derived relation, and removing each `may skip`, is then a separate change
that must pass the replay check of §10.4. Clause and Muove can disagree in
between: a handler that ran before maintenance read a stale count in Clause
but reads the current one in Muove (N2, N3).

From this file, `muove bindgen rust` generates the following. They replace
the hand-written `MenuState` mirror, its decoding in `decode_projection` and
the per-event wrappers in `clause_state.rs` (N4):

- `struct Menu`;
- `enum Event { MoveMenu { north, delta }, ClearMenuRows { north }, … }`;
- typed `Session::run` and `Session::view::<Menu>()`.

Traces to: F3, F4, F5, F6, F8, F9, F14, N3, N4, N10, P3, P4, R16,
A-Inform7, A-Attempto, A-Nix.

## 10. Compilation and evaluation

### 10.1 Pipeline

1. Parse.
2. Resolve names and vocabularies.
3. Check types and stages.
4. Lower to the Core IR: relations, rules, handlers and values.
5. Hand the IR to a target.

The Core IR is the reference semantics. Two things run it:

- **The reference interpreter** is naive and written to be obviously right.
- **The engine** uses semi-naive evaluation with indexes. It also does lazy
  provenance and state search.

Both call one expression evaluator and charge one work meter (§6.1), so
they differ only in how they find bindings: the reference by a naive fixed
point over full scans, the engine by join plans, indexes, semi-naive deltas
and provenance. Expressions are checked against named references instead:
this specification's cases and Clause's recorded text functions (§3.5). A
faster expression path, such as compiled expressions (§12.1), comes in as a
second implementation with its own differential test against this one.

Differential tests compare the two on generated programs (G7). A program
counts as compared only when the reference reaches a model or an error, and
the engine must then reach the same model or an error of the same kind. An
`Unknown` on either side is reported apart and never counts as agreement
(G2). Cedar's model of the same practice, an executable Lean model tested
against its Rust implementation, found about two dozen bugs (A-Cedar). The
prototype's oracle tests did not do this: they called the engine they
claimed to check (R14).

### 10.2 Nix target (firn)

**`muove build [paths]`:**

- compiles its program (§2) in one process, so a query in one module reads
  the facts of every module, as the generated enables do;
- writes each `nix module` to the `.nix` beside its source, atomically, and
  only when the bytes change (F1);
- runs every output through `nix-instantiate --parse` (F1's syntax gate);
- with `--check`, exits 1 and prints a diff when a committed `.nix` differs
  from what its source emits (F10). This replaces hand-edit reconciliation
  commits such as c739a062 (F13).

**Layouts:**

- `beagle` reproduces Beagle's emitter byte for byte: `let` as an
  immediately applied lambda, 2-space indent, `[ ]` and `{ }`, `''` strings
  with `''${` escaping. The migration uses it, because byte identity is its
  proof.
- `idiomatic` emits `let … in` in nixfmt style. Tom chose to switch to it
  once WP4 has proven identity (DECISIONS T2). Muove's `let` is sequential
  (§2) and Nix's is recursive, so `let x = v; … in e` agrees with the
  nested lambdas `((x: …) v)` unless a binding's value mentions its own
  name or a later binding's. The printer therefore refuses that case (a
  capture error naming the binding), and whiterabbit's drvPath confirms the
  switch.

**Lowering:**

- `when` lowers to `lib.mkIf`, the push-down form, never to `if`. That is
  what lets NixOS evaluate `config` conditions without infinite recursion
  (A-Nix, "Delaying conditionals").
- Metadata facts never reach Nix (F5).
- `nix file` emits any Nix value: the flake root's
  `{ inputs = …; outputs = …; }`, its `providerAdapter` and its overlays
  (F4). `nix module` is the same form, specialized to a module. A
  `nix file` that emits `flake.nix` has no root `let`: Nix reads a flake's
  top level without evaluating it, so it must be an attribute set, and the
  bindings go inside `outputs` (DECISIONS D-39).
- `with E: body` lowers to Nix `with`. Inside it, a name that is not a Muove
  binding resolves to `E.<name>` at the Nix stage, and Muove bindings always
  win, as Nix's do. When `E` is `pkgs`, such names are checked against the
  schema's package index (F4: `with pkgs;`).
- `a // b` is a right-biased merge. On known records and attribute sets
  Muove computes it; otherwise it lowers to Nix `//` (F3).
- A nested `nix module (args)` value, such as a home-manager user module,
  has its own argument scope and its own module system: Muove's option
  declarations do not apply to its `config` (§3.2). Its entries and its
  `config` reads are checked against the foreign schema of that module
  system when one is declared, and otherwise stay unchecked `Nix` with one
  warning per module (F3: 81 `nix/module` forms, nested ones included).

**Option types:**

| Muove | NixOS |
| --- | --- |
| `Bool` | `lib.types.bool` |
| `Int` | `lib.types.int` |
| `Text` | `lib.types.str` |
| `Path` | `lib.types.path` |
| `Port` | `lib.types.port` |
| `List T` | `lib.types.listOf T` |
| `maybe T` | `lib.types.nullOr T` |

These are every option type firn's 28 `lib.mkOption` calls use. Others,
such as `lines`, `package`, `attrsOf` and `enum`, come with the first
program that declares one (G1), each as a type that §3.3's grammar already
spells.

`option P: enable e` emits `lib.mkEnableOption e`, where `e` is any `Text`
expression: a literal, or a `let` name, which emits as that name, as five
firn modules write `lib.mkEnableOption description` (notebook K7).
`option P: T` with fields emits `lib.mkOption { type …; default …;
description …; }`, with the fields in that order, which every firn
`lib.mkOption` call already uses (DECISIONS D-39).

**Migration proof.** Compare
`nix eval --raw .#nixosConfigurations.whiterabbit.config.system.build.toplevel.drvPath`
from two worktrees that differ only in their sources. If they differ,
`nix-diff` on the two `.drv` files localizes the cause (F15). For the darwin
host ashashi, compare `darwinConfigurations.ashashi.system.drvPath` the same
way if it evaluates on Linux; it is reported, not gating. Under the `beagle`
layout every generated file must also be identical: byte-identical for
Beagle sources (`flake.nix` as box 4.5 says), parse-identical for Clause sources.

The drvPath covers only modules that whiterabbit enables. A module that no
evaluated host enables is still checked by byte identity (Beagle output) or
by parse identity (Clause output), and is listed as "not covered by
drvPath" in the migration report. Parse identity means that
`nix-instantiate --parse`, reading the committed file and the emitted text
from standard input in one directory, prints the same bytes for both. That
ignores what Nix's own printer ignores (quoting of static keys, how
attribute paths nest, the order of bindings and formals, parentheses, and
plain against indented strings) and nothing else (DECISIONS D-39).

### 10.3 Rules that feed Nix

Known-stage queries (§4.5) inside `nix module` comprehensions emit generated
modules directly. The generated enables and the flake's input section are
two examples. This removes the intermediate generated `.bnix` and the marker
splicing in `flake.bnix` (F6, F8).

A `for` block unrolls when Muove builds, so its source must be known: a
source that depends on Nix evaluation is a stage error, and Nix-side
iteration is a call such as `builtins.map(…)`.

### 10.4 Runtime target (north)

`muove compile <module> -o <image>` produces a program image: the Core IR,
the vocabulary and a content hash.

The Rust crate `muove-runtime` provides:

- `Program::load(bytes)`;
- `Session::new(&program)` and `Session::restore(&program, checkpoint)`;
- `session.run(event) -> Result<Step { revision, effects }, Rejection>`;
- `session.run_all(&[event])`, which is atomic;
- `session.view::<V>()`, `session.checkpoint()` and `session.why(sentence)`.

`muove bindgen rust` generates typed Rust for entities, text unions, events,
effects and views. North writes about 1,100 lines of mirror types and
marshaling by hand today (N4).

**Checkpoints** hold the program identity (§8.1), the revision, the state
facts, the live entities, the allocator and the pending effects (§5.4).
Restoring under a different program identity is an error.

**Other hosts.** `muove run <image> --events <file.jsonl>` and
`muove query <image> <question> --json` expose the same runtime as JSON
lines. firn's TypeScript host (DECISIONS T1) calls them for its command
plans, so the MVP needs no JavaScript target (§12).

**Only the CLI and the LSP touch the operating system.** `muove-cli` and
`muove-lsp` read and write files, run processes, start threads, and read the
clock and the environment. Every other crate, the runtime, engine and core
included, takes and returns data: source text, program images and
checkpoints come in as bytes, a host's inputs come in as events, and the
event log goes out as records that the host writes. Budgets count work,
never time (§5.5). So any host can embed the runtime, a browser included
(§12). Box 8.1 checks this.

The event log is the replay corpus. It records each occurrence, its effects
and the resulting revision hash. North has no replay tests today (N5).

**Migrating north.** The proof is replay under an explicit observation
contract. For each recorded Clause sequence, Muove must give the same:

1. admission result: admitted, or rejected with the same kind (no match,
   conflicting write, mode, postcondition);
2. ordered effects;
3. projections, compared only at transaction boundaries, after Clause's
   appended maintenance handlers have run;
4. continuation: restoring a checkpoint taken after any step and running the
   rest gives the same results.

Entities are matched through a correspondence built from creation order,
never through raw ids or state hashes. The recorder reports handler and
clause coverage, counted at the dispatcher, so the 26 events that are
dispatched by text name count too (N3). The equivalence claim is bounded by
that corpus and its coverage, and it says so (G9).

Two conversions run once, at cutover:

- **State.** `muove import clause-checkpoint` converts an existing
  content-addressed Clause checkpoint (N4) into a Muove checkpoint: its
  facts, its live entities, an allocator past every id in use, and the
  host's opaque-payload handles kept as the same opaque keys (N6).
- **Numbers.** An `F64` relation becomes `Int` only when every literal, and
  every value in the corpus and the checkpoints, is an integer of magnitude
  at most 2^53. Otherwise the import stops and names the value. Clause's
  `sum … = 0.0` FIFO-head rules are imported as written, and become
  `min … by` only after replays with tied and empty queues agree (N3).

### 10.5 Performance targets

| Target | Budget | Today |
| --- | --- | --- |
| firn: compile all 63 + 185 module files | under 2 s in total | about 1.8 s per file and about 90 s for 50 files (F11) |
| firn: review of a change, both hosts | under 5 s | — |
| north: one menu step at 1,000 rows | under 1 ms | — |

Each target is a measured check in PLAN.md.

Traces to: F1, F3, F4, F5, F6, F8, F10, F11, F12, F13, F15, N2, N3, N4,
N5, N6, N7, R14, A-Cedar, A-Nix, A-Ascent.

## 11. Tooling

### 11.1 CLI

| Command | Does | Replaces or traces to |
| --- | --- | --- |
| `muove build [--check] [--layout beagle\|idiomatic]` | emit Nix; drift check | `firn repo build`, `firn repo diff` (F1, F10) |
| `muove check [PATH…]` | all requirements of the modules under the paths, exit 0/1/2 | prototype `check`, now three-valued (R5); `beagle-validate`, `clause-workbench check-source` (DECISIONS D-33) |
| `muove why S [--all] [--precise]` | proof tree | `firn tag resolve` as "the debugger" (F6) |
| `muove why not S [--via R]` | near misses | prototype `why not` (P6) |
| `muove why S --revision R` / `muove trace S` / `muove replay T` | state provenance; executable witnesses | R1 |
| `muove prevent C --using …` / `muove achieve possible\|acceptable C --using …` | interventions | P6, R17 |
| `muove review [--base REV] [PATCH]` | §8 | R3–R11 |
| `muove fmt [--check]` | canonical layout: refuses what the lexer rejects (exit 2, file unchanged), keeps every token and whether tokens touch, and normalises spaces, comments and blank lines, one file at a time | F13 (surface churn) |
| `muove import nix FILE` | prints the Muove for a Nix file: hand-written, or the committed output of a `.bnix` or Clause module, which is how firn migrates (DECISIONS D-39) | `beagle-import-nix` (F1, F2, F10) |
| `muove import clause` | north's relations and handlers, mechanically (DECISIONS D-28) | N1 |
| `muove import clause-checkpoint` | one-time state conversion | N4, N6 |
| `muove run IMAGE` / `muove query IMAGE Q --json` | the runtime over JSON lines | firn's command plans (F12, DECISIONS T1) |
| `muove refs NAME` / `muove build --stdout FILE` | callers; the Nix a file emits | the compiler queries `callers` and `expand` (F10) |
| `muove schema extract\|explain` | foreign schema facts | `firn schema explain`, `beagle-schema` (F9) |
| `muove rename option OLD NEW` | rename by resolved references | `beagle-rename` (F10) |
| `muove compile`, `muove bindgen rust` | runtime image and bindings | N4 |
| `muove lsp` | language server | F10, beagle issue #13 (F13) |
| `muove help [TOPIC]` | the commands, and the language card: one example per §9.4 production, each compiled by the tests | `clause-workbench authoring-card` (DECISIONS D-33) |

### 11.2 LSP

- **Pipeline.** Salsa-style queries: parse, resolve, type, then solve, per
  file. Results are memoized, and diagnostics accumulate (A-salsa).
- **Source spans.** Every Core node and every proof node carries one
  (A-Lean, info trees).
- **MVP features:**
  - diagnostics with did-you-mean and closest shapes;
  - hover: the structured reading, the type, the effect level, and for a
    derived fact "why";
  - go to definition, and references (what `firn module refs` does today);
  - rename option;
  - a code lens on each module: "enabled on whiterabbit (via desktop),
    ashashi (via desktop)".
- **Options are static.** Muove declares options itself, so the LSP knows
  them without evaluating the flake. nixd needs an evaluated
  `nixosConfigurations` to know them (A-nixd).

### 11.3 Change-constraint checks in use

- **firn.** `firn repo validate` runs `muove check` and
  `muove build --check`. `firn rebuild` runs
  `muove review --base <last switched revision>` before it switches, and
  prints the per-host consequences. firn runs the `muove` that
  `firn-runtime-update` builds from the commit named in
  `nixos-config:config/muove-revision`, and its launchers refuse a
  checkout that declares another revision (DECISIONS D-33).
- **north.** CI runs `muove check`, the replay corpus and
  `muove review --base origin/main` on changes to `.muove` files.
- **Turtles.** Every diagnostic carries a "because" chain, such as why
  `cfg` has type `config.myConfig.modules.kanata`. Compiler checks written as
  Muove rules, so that `why` works on an error, come after the MVP (first
  draft, "the last turtle").

Traces to: F1, F6, F9, F10, F13, N4, N5, P6, R5, R17, A-salsa, A-Lean,
A-nixd, A-why-depends.

## 12. Not in the MVP

Muove is meant to be general-purpose (§1.2), so almost everything the MVP
leaves out is not yet rather than never. §12.1 lists those items with what
brings each one in: a program that needs it, which then becomes its runnable
example (G1). §12.2 lists the few things that stay out by design, with how
the need behind each one is met.

### 12.1 Not yet

| Not yet | Brought in by |
| --- | --- |
| **General-purpose programs**, such as command-line tools and game loops. Until then firn's mechanics go to TypeScript on Bun (DECISIONS T1), and north's terminal, process and transport code stays in Rust (F12, N6, DECISIONS D-2). | The first general-purpose milestone after A11 (PLAN.md, After the MVP), with a host platform for terminals and command lines. |
| **Fractional numbers**, and wrapping or bitwise `Int` operations. | For fractions, a program whose quantities do not fit a scaled `Int`, such as continuous physics (DECISIONS D-12). For wrapping and bitwise operations, one that hashes or draws pseudo-random numbers from a seed. A fractional type keeps §4.5's canonical order and §5.3's determinism: no implicit conversion from `Int`, a total order, the same result on every platform, and sums taken in canonical order. |
| **Recursive definitions** (§3.5). | A program that walks a recursive structure, such as a syntax tree. A recursive definition gets its own effect level, `partial`, and runs under a budget. Rules and requirements keep calling only total definitions, so §4.3's models stay finite. |
| **Sum types with payloads, `match`, local `let` in definitions, and user-defined generic types.** Text unions, records, `maybe`, `List` and `Attrs` cover firn and north. | A program with a value that takes one of several shapes, such as a parsed command line. They extend `type Name = …` and definition bodies, where the MVP grammar leaves room (§9.4). |
| **Recursion through negation, and values computed in recursion** (§4.3). | Rules that need them. Shortest paths need values computed in recursion, which lattice-valued relations give (A-Flix, A-Ascent). A game's won and lost positions need recursion through negation, and under the well-founded semantics the positions it leaves undecided are `Unknown`. |
| **Effects a step waits on**, such as a handler that reads a file within its step. Today every outcome comes back as a later event (§5.4). | The command-line milestone, if request and receipt make sequential I/O hard to read. Only reads qualify, and the host's answer is logged like a receipt, so replay stays exact. |
| **Incremental evaluation and compiled programs.** Semi-naive evaluation per state is enough at firn and north scale (A-DD, A-DBSP), and the runtime interprets the Core IR. The salsa memo covers the LSP. | The first program that misses its step budget, likely a real-time game loop. Both are fast paths, differential-tested against the reference (G7). A8's budget, one step at 1,000 rows under 1 ms, is already well inside a 60 Hz frame (16.7 ms). |
| **Embedding:** Wasm, a C ABI, a JavaScript target, a browser playground, and bindings other than `bindgen rust`. north links the Rust crate, and firn's TypeScript host runs the CLI (§10.4). | The first host that can do neither, such as a browser game. Only the CLI and the LSP touch the operating system (§10.4), so a wasm32 build of the runtime is packaging rather than a port. |
| **Package imports** pinned in `muove.lock` (§2). firn and north are each one project. | The first code that two projects share, such as a library that firn and a game both use. |
| **Greywrought game rules.** | The game milestone and the browser host above. Greywrought runs in the browser today, on TypeScript and Three.js. |
| **Symbolic or SMT analysis, ASP planning, partial-order reduction and symmetry reduction.** Explicit-state search with stated bounds comes first (A-telingo, A-Cedar). | A requirement that matters and stays `Unknown` because explicit search runs out of budget. |
| **Timed requirements**, such as "warned at least N steps before". | A program whose events carry time, such as a game's ticks or north's timeouts, with a requirement about that time. |
| **Failure groups** (`among P in groups G`) and **mutation probing of requirements** (R: weak support). | A failing example: facts that fail together, which one-fact removal misses, or a requirement that kept holding while the behaviour it guards broke. |
| **The compiler's checks as Muove rules**, the last turtle. | After the MVP, the first diagnostic that people cannot act on without its derivation. §4.3's stratification and grounding checks are the likely first, since their errors already print cycles and schedules. |
| **Porting firn's `.bjs` mechanics.** | T1's port to TypeScript on Bun, after the MVP. Until then the Beagle compiler builds only those files, never Nix. |

### 12.2 Out by design

| Out | Why, and what meets the need |
| --- | --- |
| **I/O, clocks and randomness inside Muove code** | They would break replay and determinism (G4, §5.3). The host performs effects, and time, outside identifiers and random seeds arrive as event arguments. |
| **Mutable variables, and functions in facts or state** | State changes only through events, so every change has a step that explains it (§5.3). State stays data, so it can be compared, hashed, checkpointed and explained (§3.1). Iteration goes through built-ins, and later through recursion (§12.1). |
| **Evaluating Nix** | Muove never evaluates `config`, `pkgs` or `lib`, so no verdict depends on Nix semantics (§3.2, DECISIONS D-7). Evaluated Nix data enters as foreign facts with provenance, or as a receipt when a host runs Nix. |
| **English authoring** | No free phrasing, plurals or paraphrase variants (§9.3, A-Inform7). Declared templates give the readability, with one reading per line. |
| **Exhaustive provenance by default** | Deciding why-provenance is NP-complete (A-KR2026). `why --all` gives it on request, bounded. |
| **An AI inside the toolchain** | Any AI is an author. It reads `muove help` and the diagnostics, writes patches, and gets review's verdict and sentences back (§8). The review gate is the interface. |
| **Beagle's evidence-ranked repair pipeline** | Dropped (DECISIONS D-27, D-33). Muove's diagnostics (did-you-mean, closest shapes) and the review gate do its job. |

Traces to: F10, F12, N6, P2, R (non-adopted recommendations), A-Inform7,
A-KR2026, A-DD, A-DBSP, A-telingo, A-Cedar, A-Flix, A-Ascent.

## 13. Acceptance scenarios

The MVP is done when all of these pass. PLAN.md maps each to a work package.

| # | Scenario | Measured by |
| --- | --- | --- |
| A1 | kanata: `default.muove` and `kanata.muove` emit `default.nix` and `kanata.nix` byte-identical to nixos-config's | `cmp` exits 0 for both |
| A2 | The printer round-trips all 50 Beagle-generated `.nix` | 49/50 byte-identical; `flake.nix` parse-identical, differing only at its two one-operand `++` calls |
| A3 | Every module, host and flake source is migrated, and firn's three Clause programs are replaced | whiterabbit's toplevel drvPath is unchanged. Every Beagle-output file is byte-identical (`flake.nix` as box 4.5 says). Every module no host enables is parse-identical and listed. `rg -c beagle-build\|compile-nix native/repo_build*.bjs` finds 0 compile calls. firn's dispatcher gives the same plan as `firn.clause` for every command in its table |
| A4 | `_generated-enables.nix` comes from rules | Identical for whiterabbit and ashashi. `why firefox is enabled on whiterabbit` names the `browsers` tag and both absence checks |
| A5 | The responsibilities requirement on whiterabbit | Holds. Withdrawing "obsidian is realized by obsidian-module" makes it Fail with a proof |
| A6 | ChatGPT's 9 regression scenarios, in Muove syntax, plus three `intends` contracts | Each scenario gives the verdict its test asserts. The contracts give approve, reject and unknown as written |
| A7 | The dungeon | §5.6's answers and review verdicts, and `replay` confirms every witness |
| A8 | North's menu slice | The sequences recorded from `main.rs:3547` and `restart_storage_tests.rs:388` replay under §10.4's observation contract (admissions, effects, projections, restart). One menu step at 1,000 rows takes under 1 ms |
| A9 | Budget honesty | On the 20-stage program (R13), `why` returns one proof and `why --all` returns `Unknown(work)` in under 1 s. On the 101-room chain (P7), a budget too small to finish gives `Unknown`, never `Fails`, and the default budget gives the correct answer or `Unknown` |
| A10 | Differential test | 10,000 generated programs, reference against engine, 0 disagreements |
| A11 | North runs on Muove | Every slice replays under §10.4's contract with its coverage reported. North's tests pass with the Clause crates removed from `Cargo.toml`. `flake.muove` emits north's `flake.nix` |

## Appendix A. Evidence ledger

Measured 2026-10-10. nixos-config is at `main` (`b3a904d3`) and north at
`main` (`eaee1838`). The prototype is from
`inputs/claude-muove-plan/claude-muove.zip`. Five read-only investigations
and the lead's own probes produced the numbers below. Re-checked at
nixos-config `10740a8a`, PLAN's start state: the file counts are unchanged
(63 `.bnix`, 185 Clause sources, 184 tag files), and the 25 commits in
between touch 4 Nix or Clause files, among them `native/nix/ssh.clause`.

### firn and nixos-config

- **F1. Beagle pipeline.**
  - 63 `.bnix` files, 2,178 LOC. 30 modules hold 42 files and 1,160 LOC;
    `flake.bnix` is 409 LOC; there are 8 host files.
  - 50 `.nix` files are generated from them, 2,013 LOC.
  - `beagle-build` is Racket: `parse.rkt` 6,825 LOC, `check.rkt` 10,676,
    `emit-nix.rkt` 2,492.
  - `nix-instantiate --parse` is the syntax gate.
  - Generated `.nix` is committed, because the flake reads the git tree.
  - Output is published only when its bytes change.
- **F2. Clause in nixos-config.** 374 `.clause` files:
  - `native/nix/*.clause`: 185 sources, 2,272 LOC: 184 modules, emitted
    as one-line quoted Nix, and `nixpkgs.clause`, the foreign declarations
    they share, which emits nothing (DECISIONS D-39);
  - `modules/*/tags.clause`: 184 metadata files;
  - `native/firn.clause` (589 LOC), `prewarm.clause` (224) and
    `firn-host-list.clause` (80).

  184 of 214 modules (86%) are authored in Clause. Beagle-to-Clause
  migration commits are dated 2026-09-07/08.
- **F3. Construct counts** across all `.bnix` (`count / files`):

  | Construct | Count / files |
  | --- | --- |
  | `lib.mkDefault` | 160 / 3 (157 in generated enables) |
  | `s` (interpolation) | 119 / 23 |
  | `nix/module` | 81 / 59 (including nested home-manager modules) |
  | `lib.mkIf` | 42 / 36 |
  | `lib.mkEnableOption` | 32 / 31 |
  | `let` | 30 / 26 |
  | `p` (path) | 27 / 11 |
  | `lib.mkOption` | 21 / 8 |
  | `fn` | 16 / 4 |
  | `mkOutOfStoreSymlink` | 16 / 5 |
  | `ms` (multiline string) | 13 / 8 |
  | `nix/with` | 8 / 7 |
  | `++` | 6 |
  | `//` | 4 |
  | `import` | 4 |
  | `builtins.*` | 17 / 6 |
  | `if` | 3 / 2 |
  | `lib.mkForce` | 2 |

  Literals: 877 strings, 429 maps, 341 vectors. Absent: `rec`,
  `or`-defaults, `inherit`.
- **F4. Hard cases.**
  - `''` strings that need `''${` escaping;
  - embedded scripts of more than 100 lines (`niri`, `theme-switcher`);
  - paths versus strings;
  - dotted attribute paths;
  - symbol-valued keys (`${username}`);
  - `nil` → `null`;
  - `with pkgs;`;
  - `let` emitted as lambda application;
  - in `flake.bnix`: `providerAdapter`, overlays and marker sections.
- **F5. Metadata.** `:tags`, `:tags-opt-in`, `:tag-overrides` and
  `:flake-inputs` are stripped from emitted Nix by line- and brace-based text
  surgery (`strip-authoring-attributes` in `native/repo_build.bjs`).
- **F6. Tag resolution** (`docs/tags-composition.md`):
  - Tag defaults minus `-` edits, plus opt-in `+` edits, minus `:disabled`.
  - Tag overrides are applied as `mkDefault` through default memberships.
  - `_generated-enables` is generated (whiterabbit 134 lines, ashashi 43).
  - The docs call `firn tag resolve` "the debugger" for "why is module X
    enabled".
- **F7. Responsibility triples** (`hosts/whiterabbit/responsibilities.bnix`):
  `member_of`, `has_responsibility`, `depends_on`, `requires_capability`,
  `provides_capability`, `requires_software`, `is_realized_by`,
  `is_realized_in`, `has_criticality` and `must_realize`.
- **F8. Flake inputs.** Modules declare `:flake-inputs`. These are spliced
  into 6 marker sections of `flake.bnix`, and two different URLs for one name
  are a hard error (`docs/flake-inputs.md`).
- **F9. Schema validation.**
  - It catches unknown paths, and type, enum, `listOf`, `nullOr` and
    `attrsOf` mismatches, at file:line:col with did-you-mean.
  - It checks against `.beagle-cache/schema.json`, produced by `nix eval`
    (`validate-nix.rkt`, 1,238 LOC).
  - `firn schema explain` reports a path's type and declarations.
- **F10. Tooling to keep.**
  - `firn repo diff`, the drift check;
  - compiler queries: syntax, sig, fields, callers, impact, expand;
  - the evidence-ranked repair pipeline;
  - `beagle-rename` for option paths;
  - `beagle-import-nix`;
  - the darwin/linux platform matrix.
- **F11. Build cost.** `beagle-build` takes 1.80, 1.87 and 1.78 s per file,
  mostly Racket startup. Files compile one after another, so a full build is
  about 90 s.
- **F12. The firn CLI.**
  - `~/.local/bin/firn` is a bash launcher for
    `bun native/firn_host.mjs`.
  - 59 `.bjs` files, 23,268 LOC of Beagle/JS.
  - The command table is in `native/firn.clause`, including
    `Plan: steps: Sequence<Execution>`.
- **F13. Pain points.**
  - 428 of 2,059 commits mention beagle, bnix or firn.
  - The surface was migrated again and again: tilde strings, colon-keyed
    maps, quote prefixes.
  - Generated `.nix` was edited by hand and later reconciled (c739a062,
    c6716e1f).
  - Beagle's `hallucinations.jsonl` has 70 entries, 17 of them silent
    miscompiles. 5 are Nix-specific, among them `:flake false` emitted as
    `"false"` and an `s`/`ms` part mis-tagging.
  - Beagle issues #13 (the LSP checks against the wrong profile), #14
    (quoted data typed `Any` bypasses checks) and #15 (self-host disagrees
    with the oracle).
- **F14. The first-rewrite pick: `modules/kanata`.**
  - Sources: `default.bnix` (28 LOC) and `kanata.bnix` (29 LOC).
  - Today's compiler reproduces both `.nix` byte-identically.
  - It exercises:
    - `mkOption` with `listOf`, `str`, `path`, `port` and `nullOr`;
    - defaults `[]` and `null`;
    - `mkEnableOption`, an `imports` path and `let cfg`;
    - nested `mkIf`, `mkForce` and `builtins.readFile`;
    - `''` text with escaped quotes;
    - `pkgs.unstable.*` and dotted paths.
- **F15. drvPath identity is sound.** No host config references
  `configurationRevision`, `self.rev`, `self.outPath` or `inputs.self`
  (`rg`, lead's probe).

### north

- **N1. north.clause**, 4,064 lines:
  - 152 relation shapes with modes: 120 `one`, 15 `maybe`, 17 `many`;
  - 135 events across 150 `on` clauses: 150 `when`, 145 `withdraw`, 144
    `include`;
  - 20 `create` blocks, 4 `law`/`derive` pairs, 19 `sum` aggregates, 66
    comparisons and 17 types.
- **N2. Handler semantics.**
  - `when` is a join over the pre-state, and every substitution contributes.
  - Expressions are evaluated on the pre-state.
  - Same-named clauses run as one occurrence.
  - A `one` relation is written by `Put`; conflicting writes reject the step.
  - A step with no match admits an empty delta.
  - A transition sequence is one atomic admission.
- **N3. Workarounds for missing features.**
  - No `or`, `not` or `!=`: 32 lines end in `= false` or `= 0.0`.
  - `sum … = 0.0` stands in for negation and FIFO-head selection.
  - `F64` counters stand in for integers.
  - Enums and booleans are encoded as text.
  - Dispatch goes through text handler names: 26 of 135 events are never
    named in Rust.
  - Host effects use a single-slot mailbox.
  - 6 maintenance handlers are appended to every transition.
  - There is no entity delete: `clear-menu-rows` withdraws 8 relations by
    hand.
  - Created entities get no initial data: `initialize` creates about 20
    Command referents.
  - Per-attachment loops live in Rust.
- **N4. Host boundary.**
  - The source is embedded with `include_bytes!` and run through
    `ResidentSourceWorkbenchV1`.
  - `NorthState` has 140 wrapper functions.
  - After each admission, the projection is decoded into about 40 mirrored
    fields.
  - About 2,200 of `clause_state.rs`'s 2,228 non-test lines serve the
    boundary: about 1,100 are mirror types and marshaling, and there are 12
    `Err(Protocol)` postcondition re-checks.
  - State persists as content-addressed checkpoint segments.
- **N5. Tests.** 118 `#[test]`. 21 drive the workbench and about 60 drive
  `NorthState`. None replays a recorded Clause event sequence.
- **N6. The architecture contract** (`docs/architecture.md`):
  - "Clause owns North semantics … Rust owns irreducible mechanics".
  - Each effect goes from transition, to authorized effect, to receipt, to a
    settlement transition.
  - The host holds opaque payloads that Clause cannot represent.
- **N7. `flake.clause`** (21 lines) maps to `flake.nix` (25 lines) through
  `clause-workbench project-nix`. No check or script runs that, so the pair is
  kept in lockstep by hand.
- **N8. Clause.**
  - 5 crates, about 120k Rust LOC.
  - North repinned Clause 29 times in 5 days under its adoption ratchet.
  - Work then moved to compiler-genesis trust (P2).
  - The audit's P0 findings: identity aliasing, a magic Atom, the process-v2
    contract, a stale status authority, and tests that do not cover the new
    crates.
  - The last substantive commit is from 2026-09-08.
- **N9. work-ownership.bjs** was deleted on 2026-10-10 (eaee1838). North
  `main` has no Beagle usage left.
- **N10. The first north slice: the menu.**
  - Source: `north.clause` L447–827: 8 north-scoped relations, 8 MenuRow
    relations and 13 handlers, plus `host-effect` (L1982–1999) and
    `active-conversation` (L1757–1764).
  - Rust drivers: `clause_state.rs` L457–530. Projection: L1827.
  - Tests: `main.rs:3547` and `restart_storage_tests.rs:388`.

### Prototype

- **P1. Build and proofs.**
  - It builds with rustc 1.95 and gcc, and 25 of 25 tests pass.
  - `MuoveProofs.lean` checks on Lean 4.29.1 with no `sorry`. Lean 4.33.1
    is not in nixpkgs.
  - The theorems are about abstract monotone entailment, not the Rust
    code, and `requires_iff` covers only stated targets.
- **P2. Architecture.**
  - 3,584 source lines: `syntax` 140, `program` 1,314, `engine` 422, `ask`
    976, `render` 493, `lib` 108, `main` 131.
  - The fixpoint is naive, not semi-naive.
  - Datalog is positive only.
  - Modes are parsed but never used.
- **P3. What works.**
  - Typed sentence shapes with the one-reading rule and did-you-mean errors.
  - `why` with proof trees.
  - `prevent` and `achieve`, verified by recomputation and against brute
    force on small worlds.
  - Why-not near misses, diff, and all four must-sentence forms.
- **P4. What is broken.**
  - Review approves when out of budget (`lib.rs:112`).
  - It approves while a must-sentence is already broken (`ask.rs:968`,
    `render.rs:491`).
  - A proposal can slip a rule into the base (`lib.rs:93`,
    `program.rs:348–353`).
  - `check`'s intent report ignores completeness.
  - `" #"` truncates `says:` text.
- **P5.** It has no expression language, attribute sets, functions or code
  generation.
- **P6. Worth porting.**
  - `reasons` with `insert_minimal`, and `minimal_hitting_sets`;
  - verify-by-recompute (`ask.rs:316`, `:402`);
  - the diff's `Changed` type, and the review layout;
  - `says:` shown but never trusted;
  - `NearMiss`, and `build_proof`'s rank-based proofs;
  - `syntax::blocks`, and `Error` with its line;
  - `suggest`/`edit_distance`, and the closest-shape ranking.
- **P7. Scaling.** A 101-room chain (5,050 facts) exhausts the 20M work
  budget in 14 s on a debug build. It then reports a broken must-sentence
  with no "Incomplete" line.

### Review verification

All rows below were reproduced. 8 of the 9 regression tests fail on the
prototype, both Python oracles reproduce their bundled JSON exactly, and an
independent state search confirms the dungeon flaw.

- **R1. Dungeon.** In play, the reachable rooms are Entrance, Hall, Library,
  Crypt and Vault, but the prototype derives the Armory too. The Iron-Key is
  a real single point of failure. Only 4 of the 6 `achieve` placements work.
- **R2.** The trap requirement has no notion of order.
- **R3.** Minimal fact support is not derivation dependence, and `diff`
  misses a bypass.
- **R4.** "More reasons" is a misleading measure of strength: review approved
  while C became a single point of failure.
- **R5.** `check` succeeds despite incomplete analysis (critical).
- **R6.** Review approves a result that was already invalid (critical).
- **R7.** Review approves while incomplete (critical).
- **R8.** Counterfactual checks discard their completion status.
- **R9.** A proposal rewrites its own baseline (critical).
- **R10.** `build_proof` invents a "stated" leaf.
- **R11.** A derived fact that becomes stated is shown as "no longer true".
- **R12.** Budget checks sit outside the expansion loops. Real cost is far
  above the work counter.
- **R13.** The 2^20 blowup: one `why` ran past 120 s without reporting
  Incomplete.
- **R14.** The oracle tests share `engine::close` with production.
- **R15.** Bare variables in must-sentences mean "some".
- **R16.** The global namespace makes sentences ambiguous across models.
- **R17.** `achieve` offers changes that break `never` requirements.
- **R18.** Provenance is eager.
- **R19.** The Lean sketch is abstract. It is now machine-checked; see P1.
- **R20.** `says:` is never checked.

## Appendix B. References

All URLs were opened during the prior-art investigation, except where marked.

- **A-Souffle**: provenance, https://souffle-lang.github.io/provenance ; components, https://souffle-lang.github.io/components
- **A-Flix**: first-class Datalog, https://doc.flix.dev/fixpoints.html
- **A-Ascent**: lattices in Rust Datalog, https://github.com/s-arash/ascent
- **A-egglog**: functions with merge, https://arxiv.org/abs/2304.04332
- **A-Datafun**: monotonicity and semi-naive evaluation, https://www.rntz.net/datafun/
- **A-Formulog**: https://people.seas.harvard.edu/~chong/abstracts/BembenekGC2020.html
- **A-DDlog**: archived typed Datalog, https://github.com/vmware-archive/differential-datalog
- **A-Dedalus**: Datalog in time and space, https://www2.eecs.berkeley.edu/Pubs/TechRpts/2009/EECS-2009-173.html
- **A-Statelog**: https://users.sdsc.edu/~ludaesch/Paper/moc98.html
- **A-Upsert**: PostgreSQL's `ON CONFLICT` names one conflict target, https://www.postgresql.org/docs/current/sql-insert.html#SQL-ON-CONFLICT ; SQLite's `REPLACE` deletes every conflicting row, https://www.sqlite.org/lang_conflict.html
- **A-EventCalculus**: https://en.wikipedia.org/wiki/Event_calculus
- **A-telingo**: temporal ASP, https://arxiv.org/abs/1705.09811 , https://github.com/potassco/telingo
- **A-Semiring**: provenance semirings, https://web.cs.ucdavis.edu/~green/papers/pods07.pdf
- **A-KR2026**: why-provenance with negation, https://proceedings.kr.org/2026/19/kr2026-0019-bogaerts-et-al.pdf
- **A-MOLLY**: lineage-driven fault injection, https://people.ucsc.edu/~palvaro/molly.pdf
- **A-DD**: differential dataflow, https://github.com/TimelyDataflow/differential-dataflow
- **A-DBSP**: https://arxiv.org/abs/2203.16684
- **A-salsa**: https://salsa-rs.github.io/salsa/overview.html
- **A-Nix**: NixOS manual, "Delaying conditionals" and "Setting priorities", https://nixos.org/manual/nixos/stable/ ; module system deep dive, https://nix.dev/tutorials/module-system/deep-dive
- **A-nixd**: https://raw.githubusercontent.com/nix-community/nixd/main/nixd/docs/configuration.md
- **A-why-depends**: https://nix.dev/manual/nix/2.24/command-ref/new-cli/nix3-why-depends
- **A-Dhall**: https://docs.dhall-lang.org/discussions/Safety-guarantees.html
- **A-Nickel**: https://nickel-lang.org/user-manual/merging
- **A-CUE**: https://cuelang.org/docs/concept/the-logic-of-cue/
- **A-Pkl**: https://pkl-lang.org/main/current/language-reference/index.html
- **A-KCL**: https://www.kcl-lang.io/docs/reference/lang/spec/schema
- **A-Jsonnet**: https://jsonnet.org/articles/design.html
- **A-Lean**: elaboration and info trees, https://lean-lang.org/doc/reference/latest/Elaboration-and-Compilation/
- **A-Alloy6**: https://alloytools.org/alloy6.html
- **A-Apalache**: https://apalache-mc.org/docs/apalache/running.html
- **A-Dafny**: two-state lemmas, https://dafny.org/latest/DafnyRef/DafnyRef
- **A-OPA**: https://www.openpolicyagent.org/docs/cli
- **A-Cedar**: https://arxiv.org/html/2403.04651v2
- **A-Unison**: https://www.unison-lang.org/docs/the-big-idea/
- **A-Koka**: https://arxiv.org/abs/1406.2061
- **A-Verse**: https://www.cs.tufts.edu/~nr/cs257/archive/simon-peyton-jones/verse-icfp23.pdf
- **A-Roc**: platforms and applications, https://www.roc-lang.org/faq
- **A-Elm**: commands to the runtime, https://guide.elm-lang.org/effects/
- **A-semver-checks**: https://github.com/obi1kenobi/cargo-semver-checks
- **A-Attempto**: https://attempto.ifi.uzh.ch/site/description/description.html
- **A-Inform7**: https://inform-7-handbook.readthedocs.io/en/latest/chapter_10_advanced_topics/so,_is_inform_7_the_ultimate_if_programming_langua/

## Appendix C. Traceability matrix

| Section | Inventory and audit evidence | References |
| --- | --- | --- |
| 1 Thesis, direction, targets, guardrails | F1, F2, F12, F13, F15, N1, N4, N6, P3, R1, R5–R8, R12–R16 | A-Cedar, A-Apalache, A-Nix, A-Roc, A-Elm |
| 2 Program structure | F9, P3, R16 | A-Dhall, A-Unison |
| 3 Values, types, stages, effects | F3, F4, F6, F9, N1, N3, P5 | A-Koka, A-Formulog, A-Dhall, A-Pkl, A-KCL, A-Nix |
| 4 Relations, facts, rules | F6, F7, F8, N1, N2, N3, P2, R3 | A-Souffle, A-Flix, A-Ascent, A-egglog, A-Datafun |
| 5 State, events, effects | F12, N2, N3, N4, N6, R1, R2, R17 | A-Dedalus, A-Statelog, A-Upsert, A-EventCalculus, A-Alloy6, A-Apalache, A-telingo |
| 6 Requirements, verdicts | F7, F8, P1, P3, P4, R2, R4–R8, R12, R15, R17 | A-Alloy6, A-Apalache, A-MOLLY, A-Dafny |
| 7 Explanation | F6, P1, P6, R3, R10, R17, R18 | A-Souffle, A-KR2026, A-why-depends, A-EventCalculus, A-OPA |
| 8 Change review | F6, F13, P4, R3, R4, R6, R7, R9, R11, R20 | A-Cedar, A-semver-checks, A-Dafny, A-Dhall, A-Unison |
| 9 Syntax | F3, F4, F5, F6, F8, F9, F14, N3, N4, N10, P3, P4, R16 | A-Inform7, A-Attempto, A-Nix |
| 10 Compilation, evaluation | F1, F3, F4, F5, F6, F8, F10, F11, F12, F13, F15, N2, N3, N4, N5, N6, N7, R14 | A-Cedar, A-Nix, A-Ascent |
| 11 Tooling | F1, F6, F9, F10, F13, N4, N5, P6, R5, R17 | A-salsa, A-Lean, A-nixd, A-why-depends |
| 12 Not in the MVP | F10, F12, N6, P2 | A-Inform7, A-KR2026, A-DD, A-DBSP, A-telingo, A-Cedar, A-Flix, A-Ascent |
| 13 Acceptance | every item above | — |
