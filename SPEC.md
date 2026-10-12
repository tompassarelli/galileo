# Galileo MVP specification

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

Galileo is a language for software that AI writes and people approve, and it
is meant to be general-purpose (§1.2). A program is rules over relations,
state that events change, and requirements. The same executable rules
determine behavior, explain behavior and constrain changes to behavior.

- **Determine.** A Galileo program is what runs. In the MVP it emits the Nix
  that NixOS evaluates, and it is the state machine north's host drives.
  There is no second model beside the implementation.
- **Explain.** `why`, `why not` and `trace` are answered from the same
  evaluation, as sentences with source locations, and every answer says how
  complete it is.
- **Constrain.** Requirements are part of the program. A change is reviewed
  against a frozen baseline: what it changes, what follows, which requirements
  it introduces, fixes or leaves broken, and whether it does what its author
  said it would. Review judges a change against the frozen facts, the
  recorded runs and a bounded search, not every possible input; symbolic
  analysis stays future work (§12.1).

Galileo is for people who read meaning: AI writes most of the code, and people
approve consequences. So the language is built to be checked rather than
trusted. A line has exactly one reading, or it is an error that names the
closest shapes (G5). An author's description of a change is shown and never
trusted, while its `intends` claims are checked (§8.3). The authoring
reference is `galileo help`, whose examples the tests compile (§11.1), because
Beagle's 70 recorded hallucinations show what remembered syntax costs (F13).

### 1.2 Direction

The destination is a general-purpose language (DECISIONS T4). Galileo starts
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
  migration. nixos-config verifies only whiterabbit. Since nixos-config
  `1ee5dc59`, whiterabbit embeds the flake revision
  (`system.configurationRevision` reads `self.rev`). So the check evaluates
  `path:` copies of the two trees, where `self` has no revision, and the
  drvPath is then a sound identity check. nixos-config retired ashashi
  (nix-darwin) and thinkpad-x1e at `7d7a8c9d`, so no other host's drvPath
  exists to report. `lib.mkDarwinSystem`, which `flake.nix` still defines,
  is covered by `flake.nix`'s parse identity (DECISIONS D-67).
- **north.** Recorded event sequences give the same admissions, effects and
  projections on the Galileo runtime as on Clause (§10.4), and north's tests
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
- **G10.** Every example, and everything an importer writes, reads
  idiomatically: as an author who knows the language would write it. A
  construct that forces a workaround, such as a nested `if` standing in for
  a missing test, a `{value: T}` slot standing in for a field, or a
  binding that exists only to reach a literal form, is a language bug. It
  is fixed in the language, and the example is then rewritten, never
  patched around (Tom, 2026-10-11: "don't let the language be ugly. Fix
  those issues fix the gaps"; DECISIONS D-61). The check is the idiom scan
  of PLAN boxes 8.6c and 8.8b.

Traces to: F1, F2, F12, F13, F15, N1, N4, N6, P3, R1, R5–R8, R12–R16,
A-Cedar, A-Apalache, A-Nix, A-Roc, A-Elm.

## 2. Program structure

- **Files and modules.** A program is a set of `.gal` files under a project
  root: the nearest ancestor of the working directory that holds `.git`, or
  the working directory when none does. Each file is one module, named by
  its path from the root without the extension:
  `modules/kanata/kanata.gal` is `modules/kanata/kanata`, wherever
  `galileo` runs, so names and the identities built from them never move. A
  command reads the modules under the paths it is given, the whole project
  by default, and every module they `use`. Each module is read against its
  own vocabulary, and the modules are linked into one Core IR, never
  concatenated. There, everything a module declares is named
  `module/name`, such as `firn/tags/is-tagged`, so adding a module never
  renames anything. A type is too: `firn/tags/Module` is its identity
  everywhere inside, its field `f` is `firn/tags/Module.f` (§4.1), and two
  modules' `Hero` types are two types. A message names a type as the
  source writes it, `Module` (DECISIONS D-55).
- **Libraries.** `use firn/<name>` names a module of Galileo's own library,
  `lib/firn/`, which the `galileo` binary carries. A library module joins a
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
  one; a computed name (§10.3) neither shadows nor is shadowed. Outermost is the prelude: §3.5's functions. `sort … by`, `min … by`
  and the `… where` aggregates are syntax, read by the token after the
  word: `count` opens an aggregate only before `where`, and `sum`, `min`
  and `max` only before a token that begins an expression other than `(`,
  so `count(xs)` is the prelude's call and `count - 1` reads a parameter
  named `count` (§9.4; DECISIONS D-61 addendum 3). Inside a `nix module`
  or `nix file` body come the names Nix binds in every file: `builtins`,
  and the bare globals that `firn/nix-lib` types, `import` and `toString`.
  Every other Nix global is written `builtins.<name>`, and no bare one takes
  a prelude function's name, so `map` is Galileo's. Outside Nix bodies no Nix
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
  normalized content in `galileo.lock` (A-Dhall). The MVP has none: firn and
  north are each one project.
- **Foreign facts.** Data produced outside Galileo, such as the NixOS option
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
| `Path` | `./x`, `../x`, `/x`. Distinct from `Text`, because Nix treats them differently (F4). Interpolated into text, it becomes Nix's store copy (§9.1). |
| `null`, `maybe T` | `maybe T` is `T` or `null` |
| `List T` | ordered |
| records | closed, with named fields |
| `Attrs T` | open maps from text keys to `T` (Nix attribute sets) |
| text unions | `"queued" \| "paused" \| …`, Galileo's enums (N3: north encodes enums and booleans as text) |
| `OptionPath` | A sequence of attribute names, written as a dotted literal such as `myConfig.modules.firefox.default` in a slot typed `OptionPath`. It is never split out of a `Text` value. |
| entities | references to nominal entity types: declared, or created by events |
| functions | first-class in expressions; never stored in facts or state |

There are no fractional numbers in the MVP. North's `F64` stands in for
counters and positions (N3), and for two quotients that north shows as whole
numbers, a percentage and minutes, which `/` computes as `Int` (§9.4,
DECISIONS D-83); firn uses none (F3). They are not yet
(§12): `Int` never converts implicitly, so adding them changes no MVP
program (DECISIONS D-12).

**Attribute names.** In an attrpath segment `{expr}`, a `Text` value is
exactly one attribute name, quoted in the emitted Nix when it needs quotes,
and an entity contributes its name as one attribute name. An `OptionPath`
value contributes all of its segments. So a username key such as `"a.b"`
stays one name (F4), while a tag override's path splices into several (F6).
A quoted segment is a text literal (§9.1) and means what that literal
means as a value: `"a\"b"` is the name `a"b`, and `"user-{name}"`
interpolates, so it is computed. Braces hold any other expression, so a
name has one spelling, bare, quoted or `{expr}`, and `{"…"}` is an error
that offers the quoted segment. A quoted segment emits a quoted Nix key,
and `{expr}` emits `${…}` (DECISIONS D-56).

### 3.2 Two stages: known and Nix

- **Known values** are computed by Galileo itself: at build time for firn
  (literals, given facts, rule results over the given model, definitions),
  or at run time inside a session for north (anything read from the current
  state, §4.5). "Known" never means evaluated by Nix.
- **`Nix T` values** are Nix expressions that Galileo type-checks and emits but
  never evaluates. This covers anything rooted at a module argument (`config`,
  `lib`, `pkgs`, `inputs` and so on) or at a name the Nix target binds
  (`builtins`, `import`, `toString`; §2), any call into those, and a text
  that interpolates a `Path`, whose store path only Nix computes (§9.1).
- Known values lift into `Nix T` implicitly and are emitted as literals.
- A prelude function (§3.5) takes known values and is computed by Galileo, so
  `length("abc")` emits `3`. A Nix-stage argument is a stage error that
  names the Nix-side call, such as `builtins.length`; a prelude name is
  never emitted as a bare Nix name.
- **Opaque payloads.** A `Nix T` value may be stored in a fact slot typed
  `nix T`, such as a tag override's value `[pkgs.firefox]`. Rules can bind it
  and pass it on, and emission writes it back out. Nothing can decide on it:
  conditions, comparisons, arithmetic, aggregates and event guards on a
  `nix T` value are type errors that read "this depends on Nix evaluation;
  Galileo can't decide it". The one exception is equality of the normalized
  syntax tree, which mode checks and deduplication use.
- So no Galileo conclusion, verdict or step ever depends on how Nix evaluates.
- This boundary is the trust boundary. Galileo explains what Galileo determines,
  and what Nix decides is shown as the Nix that was emitted. Evaluated Nix
  data, such as the option schema, enters only as foreign facts with
  recorded provenance.

`Nix`-stage types come from three places:

1. option declarations in Galileo sources;
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
option tree: the options that Galileo sources declare, together with the
declared foreign schema. Where both declare a path, Galileo's declaration
wins. A read such as `cfg.devics` and an entry such as
`myConfig.modules.kanata.enabel: true` walk the tree the same way. Under a
closed node, an unknown name is an error with a did-you-mean and the
declaring line or, when no declared name is near, the node's names, at
most six; under an open node, names pass unchecked.

- With a schema, every node down to a declared option is closed.
- Without one, a node is closed only when a Galileo source declares an option
  directly in it, such as `myConfig.modules.kanata`. So
  `config.services.kanata.enable` passes until a schema is declared, and a
  node that also holds foreign options, such as `services.foo` once Galileo
  adds `services.foo.extra`, needs the schema.
- Past a declared option, the rest of a path is a key into its value, such
  as `main` in `services.kanata.keyboards.main`. A computed segment `{…}`
  ends the walk, and the entries of a `nix file` value are attribute keys,
  not option paths.
- So an option is declared once, and never under another declared option.
  A path declared twice is an error at its second declaration, and a path
  under a declared option is an error at its own line, each naming the
  other declaration's file:line, since the module system refuses both
  (DECISIONS D-73).
- A nested `nix module (…)` has its own module system, so Galileo's
  declarations do not apply to its `config` (§10.2).
- An `imports` value that is not a path is a module Galileo cannot read,
  so its options are foreign: without a schema, each node in which the
  importing module declares an option is open, as firn's `timezone`
  declares `zone` in an imported record.

A `pkgs` attribute missing from the schema's package index is a warning that
names the index's revision.

### 3.3 Types are contracts

A type is a contract the checker proves before anything runs, the same kind
of plain checked statement as a mode (§4.1) or a requirement (§6), checked
earlier (DECISIONS T5's principle 14, D-42). A slot `Room`, or
`?there: Room`, states that every
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
- **Divisor checks narrow.** A division is `maybe Int`, `null` when its
  divisor is 0 (§3.5), and `Int` where the divisor cannot be 0: a nonzero
  integer literal, as in `?seconds / 60`, or a name, a `?` variable or a
  field path that a conjunct of the condition compares with `0` by `!=`,
  `>` or `<`, on either side, in the guarded part, or that `E == 0` tests,
  in the `else` branch. The places, and what `and`, `or`, `not` and a
  `let`-bound condition do, are null narrowing's. So `if ?window > 0 then
  ?used * 100 / ?window else 0` is an `Int`. Dividing by the literal `0` is
  an error, because the answer is fixed (DECISIONS D-83).
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
only total definitions, and they are used by 3 rules." No Galileo code performs
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
- **Partial operations return `maybe`:** `first`, `lookup`, division by 0,
  and `min`, `max`, `min … by` and `max … by` over a condition that
  nothing matches, which give `null` (§4.3). A division whose divisor
  cannot be 0 is an `Int` (§3.3).
  Integer overflow, `-9223372036854775808 / -1` included, is the only
  runtime error.

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
  template's words (`Hero is in Room` is named `is-in`). A shape writes
  `name:` only when the derived name would collide with another shape's
  or an outside reader keeps another name, as north's events and bindings
  keep `move-menu` and `menu-host-effect` (DECISIONS D-61 addendum);
- optional modes and an optional `milestone` flag.

```text
state Hero is in Room
  exactly one room per hero
```

**Slots.** A slot is a blank, and Galileo writes a blank one way: `?name`,
as every sentence that uses the shape fills it (DECISIONS D-61 and its
addendum). A template writes a slot as the bare name of a type the program
declares when the slot takes that type's name: `state Hero is in Room`
declares the slots `hero: Hero` and `room: Room`. The name is the type's
name in lower case, a hyphen before each later capital, so `MenuRow` gives
`menu-row`. Every other slot is `?name: Type`: a prelude type
(`?query: Text`, `?delta: Int`, `?value: nix Any`), a type with arguments
(`?rows: List Text`, `?port: maybe Port`), or a type that two slots of one
template share, written so in every such slot: `?from: Room connects to
?to: Room`. A slot's type reads `maybe` and `nix` and then one type name
with as many arguments as that type takes (`List` one, a declared type
none), so the slot ends there and what follows is the template's. A
declaration always gives a slot's type, so a `?name` with none is an error
asking for it, and `?room: Room`, where the bare type gives that name, is an
error that offers `Room`, so a slot has one spelling. Braces outside text
mean only records (§9.4); the braced slot `{name: Type}` is read until box
2.21g rewrites the corpus and is then an error that offers the new
spelling.

In a template, a word that begins with a capital letter is always a type,
and a type's name always begins with one (§9.1). A capitalized word that
names no type in scope is an error naming the nearest type; a template
that needs a capitalized word writes it in backticks, `` `Codex` ``. So
capitalization alone decides what a template word is: a `use` can only
supply a type a template already names, and never turns a word into a slot
(G5). A slot's name is what mode and domain lines, `settled by` (§5.4) and
generated bindings (§10.4) read, so the two spellings of one slot lower to
the same shape.

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
with one slot, `entity: T`, read as `?x is a T` (IR only; no source
declares it), named `is-a-T` and
exported with `T`. For a `type` it
is given, and its facts are the entities of `T`, which are exactly its
declarations `X is a T`, each cited at its line (§4.2). For a
`state entity` it is a state relation that holds the live entities: its
declarations `X is a T` under `initially` state it at revision 0 (§4.2), a
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

Each field `f` of `T` is the relation `T.f` (IR name
`<module>/T.f`), whose two slots are the entity and the value, with mode
`exactly one f per T`, or `at most one` when the field's type is `maybe`.
`?x is a new T {…}` includes one fact per field it gives (a `maybe` field
given `null` or left out has none), and `?x.f` reads the fact in the state
being evaluated, `null` when a `maybe` field has none. A field declared
under `state entity T` is a **creation field**: it is fixed for the
entity's life, so no `then` line states it, and `then ?x.f = …` on one is
an error naming the field and offering a state field (T5's principles 15
and 18). The history keeps a field after its entity is deleted (§5.7).

**Declared entities of a `state entity`.** `X is a T` under `initially`, for
a `state entity` `T`, declares an entity that is created at revision 0
(§4.2, DECISIONS D-81). `initially` is its creation record, as a `new`
record is a created entity's: `X.f = e` there gives a creation field once,
fixed for the entity's life, and an `X` whose `initially` leaves out a
creation field whose type is not `maybe` is an error at its declaration
that names each such field, as `?x is a new T` names them. The check reads
the whole program, since a module's or a patch's `initially` facts may
give them. Its state fields start under `initially`, as every declared
entity's do. `not ?x is a T` deletes it as it deletes a created entity, and
its name stays declared, so a question about an earlier revision still
names it (§5.7). It takes no id from the allocator, and a given fact that
names it is an error at its line:

```text
state entity Conversation
  id: Text

state for Conversation
  saved-draft: Text

initially
  initial-context
    is a Conversation
    id = ""
    saved-draft = ""
```

**State and derived fields.** A value that one entity has exactly one of,
or at most one of, is a field, whatever the entity's kind. A block names
where the field's facts come from, as a shape's kind does (DECISIONS D-50,
D-61):

```text
state for North
  menu-kind: Text
  menu-selection: Int

derived for MenuRow
  position: Int
```

- **`state for T`** declares state fields of `T`, a `type` or a `state
  entity`. A declared entity's state fields start under `initially`
  (`north-main.menu-kind = ""`), a created entity's in its `new` record, and
  a handler's `then ?x.f = e` states the next value. Each is the state
  relation `T.f`, so stating it supersedes the value at its key and the old
  value stays in the history (§5.2, §5.7): nothing is overwritten. A `maybe`
  state field is withdrawn by `then ?x.f = null`.
- **`derived for T`** declares derived fields, which only rules give:
  `rule menu-row-position: ?row.position = ?rank` (§4.3). An `exactly one`
  derived field fails its mode for each live `T` that no rule gives a value
  (§4.4).
- A field's name is unique within `T` across its blocks, and `?x.f` reads any
  of them the same way.

A field replaces a shape whose template only names the value: `state
{north: North} menu kind {value: Text}` with `exactly one value per north`
says no more than `menu-kind: Text`, and spells the read three ways
(`?north menu kind ?k`, `the ?k where …`, `?k` again in the view). So a
`state` or `derived` shape whose mode keys one entity slot and whose other
slot is named `value` is an error that offers the field. A shape whose words
say something, such as `Hero is in Room` or `Host runs Platform`, stays a
sentence even when its mode makes it a function: the sentence is what `why`
prints and what a reader scans.

`galileo import clause` makes a Clause `one` or `maybe` relation keyed by one
entity a field, named by the relation's name less the key type's prefix
(`menu-row-key` of a MenuRow is `key`): a creation field when only the
clause that creates the entity writes it, apart from one teardown that
withdraws every fact of the entity, which becomes `not ?row is a MenuRow`;
otherwise a state field. A top-level `member of: T` whose `T` a clause
creates is a declared entity, written `X is a T` under `initially`, and its
initial facts are its creation, not writes (DECISIONS D-79, D-81). It makes
a Clause `one` field `exactly one` only when every site that creates the
entity sets it in the same step, each declared entity's initial facts
included. Otherwise the field becomes `maybe`: north creates about 20
Command referents with no data (N3).

### 4.2 Facts

- **A fact** is a ground sentence of a `given` shape, or of a `state` shape
  under `initially` or in a patch's `add` (§8.2), where a declared entity's
  state field is stated as
  `north-main.menu-kind = ""`, any total expression on the right, and a
  declared `state entity`'s creation field the same way (§4.1). A sentence
  of a `derived` shape stated as a fact, at
  the top level, under `initially` or in a patch's `add`, is an error at
  its line: `` `holds` is derived, so only rules give its facts ``
  (DECISIONS D-50).
- **Entities.** `X is a T` declares one, and it is the only way a name
  becomes an entity. It is a fact of `is-a-T` (§4.1), so it stands where
  that shape's kind puts it: at the top level for a `type`, whose entities
  are given, and under `initially` for a `state entity`, whose entities are
  state. So a `state entity`'s entities are the ones declared under
  `initially`, live from revision 0, and the ones `new` creates (§4.1,
  §5.2). A declaration in the other place is an error at its line that
  offers the right one (DECISIONS D-81). A declaration in any module
  counts, under `initially` too, since declarations belong to the program
  (§2). Every name in a slot of an entity type, in a
  fact, a rule, a claim, a handler, a view, a query, a Nix body or a
  command's question, names a declared entity of the slot's type.
  Otherwise the line is an error, at its file:line, that names it, the type
  and the nearest declared entity of that type. Misspelling `anytype` on
  line 13 of §9.6's host file gives `` `anytpe` isn't a declared Module;
  the nearest is `anytype`, and `anytpe is a Module` declares a new one ``.
  With no declared entity near it, the message gives only the declaration.
  So a misspelled name is an error, never a new entity (DECISIONS D-49).
  An entity is its type and its name, so one name may be declared as
  entities of several types, as firn's `python` is a module and a tag. The
  slot's type picks one, and where nothing gives the type, a name of two
  types is an error that lists them. Declaring one entity twice is a
  warning, as stating a fact twice is. A declaration takes the list sugar
  below: `[Entrance, Hall, Library] is a Room` declares one entity per
  element, so a roll-call is one line per type (DECISIONS D-61 addendum).
- **Focus blocks** (a subject line with its predicates indented under it) and
  **list slots** are sugar. A focus block's lines are its subject's facts
  with the subject left out: a predicate (`leads from Entrance to Hall`),
  `is a T`, which declares the subject where its kind puts it (above, so
  under `initially` for a `state entity`), a field fact `f = e` under
  `initially`, and a fact table whose header is a predicate and whose rows
  sit under it. Two or more facts in a run that share a subject are written
  as one focus block, so the subject is written once (G10):

  ```text
  initially
    north-main
      menu-kind = ""
      menu-selection = 0
  ```

  A focus line is read with the subject put first; `f = e` has one reading,
  since `=` is no template word, and a predicate with variables is a table
  header when rows follow it and an error otherwise (DECISIONS D-61 addendum
  4). `whiterabbit enables [terminal, rust]` states one
  fact per element, one level deep, and an empty list there is an error,
  since the line would state nothing. A fact never writes `null`, a `maybe T` slot included: `null` is how a
  missing fact reads (§4.1, §4.3), so the line would state nothing, and it
  is an error at its line. The slot's declared type decides: in a
  slot typed `nix T` or `List T` the list is one value, so
  `[pkgs.firefox]` is one payload (§3.2) and a `List Text` slot holds a
  plan's arguments as one list. Only facts take the sugar; in a rule, a
  query, a `then` line or a claim a list is one value and fits only a slot
  whose type admits it.
- **Fact tables** state many facts of one shape. The header is a sentence
  whose every slot is a distinct `?name`, and each row indented under it
  gives one slot token per variable, in the header's order, separated by
  spaces. A row means the header with its tokens put in place, so it is a
  fact and takes the fact sugar, and `why` cites the row's line:

  ```text
  accepting ?kind menu requests ?effect
    "work"              "work-show"
    "history"           "switch-conversation"
    "archived-history"  "restore-conversation"
  ```

  A slot token holds no space outside its quotes and brackets (§9.2), so a
  row splits one way, and how many spaces separate two tokens means
  nothing: `galileo fmt` aligns the columns, and only a row's indentation
  is read. Gherkin's Examples tables fill one step template the same way
  (A-Gherkin). A row with too many or too few tokens is an error at its
  line that names the header's variables. A header with a constant, a repeated
  variable, `?_` or an expression, a header with no rows, and a text block
  in a row are errors. A table stands wherever a fact or a focus block does,
  and inside a focus block (above). Three or more facts of one template in
  a run are written as a table, and a one-column run as a list, so a
  template is written once (DECISIONS D-61 addendum, addendum 4).
- **Duplicates.** Stating the same fact twice is a warning. Stating facts
  that break a mode is an error that shows both lines.

### 4.3 Rules

A rule has a name, a head sentence of a derived shape and a body: a
conjunction of literals, as a query's or a claim's condition is. Its first
line is `rule <name>: <head>`, and each body literal takes one line under
it, the first opening with `when` and every other with `and`, so the rule
reads as one sentence (DECISIONS D-51):

```text
rule nested-member: ?u is in ?p
  when ?u is in ?g
  and ?g is nested in ?p
```

A rule for a derived field (§4.1) has the head `?x.f = e`, where `?x` and
`e` read only what the body grounds, and `e` may be an aggregate:

```text
rule menu-count: ?north.menu-count = count where ?north lists ?row and ?row.visible
  when ?north is a North
```

It means the sentence `T.f(?x, e)` and is checked as any head is. Where `e`
is `null` the rule derives no fact, so a `maybe` field reads `null`, as
`then ?x.f = null` withdraws one (§5.2); a field that is not `maybe` takes
no `maybe` expression.

A rule body has no `or`: a second rule with the same head is the other
case. The body literals are:

- a positive sentence, `?x is a T` included (§4.1);
- `not` followed by a sentence; `not` followed by an expression is a boolean
  expression literal, holding when the expression is `false`. Each `?_` in
  a `not` sentence is existential inside the `not`: `not ?p borrowed ?_`
  holds when no value makes `?p borrowed ?_` true, so `?p` borrowed
  nothing, as Datalog's `!borrowed(p, _)` reads, and `not ?_ connects ?_`
  holds when nothing connects anything (DECISIONS D-49);
- a comparison: `== != < <= > >=` on `Int` and `Text`, `== !=` on
  `Bool`, text unions and entities, and `e in list`, which holds when the
  list has `e`'s value: `?north.menu-kind in ["history",
  "archived-history"]` (§9.4);
- a boolean expression, such as `contains(lowercase(?label), ?query)`. The
  literal holds when the expression is `true`;
- a binding `?x = expr`, where `expr` is total, an aggregate included:
  `?n = count where ?hero holds ?k`;
- membership: `?x in ?list`, with a variable on the left. It is the `in`
  comparison when `?x` is grounded elsewhere, and grounds `?x` to each
  element otherwise, so the line has one meaning either way and is never
  read as two (§9.4).

**Aggregates are expressions** (DECISIONS D-61 addendum 3). `count where
<body>` and `sum|min|max <expr> where <body>` range over the distinct
bindings of their body's local variables (see scopes below): `count`
counts them, and `sum`, `min` and `max` combine `<expr>` over them, so two
rows with equal costs both count. `sum` adds `Int` values exactly and
checks the `Int` range once, at the end, so the result never depends on
order. `min` and `max` take `Int` or `Text` and order them as `<` does. The
argmin `min ?x by <expr> where <body>` gives the `?x` with the smallest
`<expr>`, an `Int` or `Text` ordered as `<` orders them, and `max ?x by`
the one with the largest, as Python's `min(xs, key=…)` does. Ties go to
the smaller `?x` in canonical value order, so the result is deterministic.
North writes this today as `sum … = 0.0` FIFO-head workarounds (N3).
`count` and `sum` are `Int`, 0 when nothing matches; `min`, `max` and the
argmin are `maybe T`, `null` when nothing matches (§3.5), so a FIFO head
is a `maybe` field: `?queue.head = min ?item by ?item.order where ?queue
holds ?item`. An aggregate stands wherever an expression is due: a rule's
head, a binding, a comparison, a `then` line, a view field or a `define`
body. Its condition reaches as far right as a query's (§9.4), so an
aggregate inside a larger expression is parenthesized, `(count where
?hero holds ?k) > 2`, and in a sentence slot it is one parenthesized token
(§9.2).

**Scopes.** A rule, a handler clause, a claim, a view and a definition each
own their variables, and a definition or view sees its callers only through
its parameters. Each aggregate, argmin and query opens a nested scope,
wherever it stands, and a query's head and sort key and an aggregate's
value are inside it. A name that also appears at the top level of its
owner, or of any scope enclosing it, is outside the nested scope: it is
shared, and must be grounded before the nested scope runs. A rule's head
and a clause's `on` line and `then` lines, outside their nested scopes,
are at the owner's top level, so in `rule menu-row-position: ?row.position
= count where ?north lists ?prior and ?prior.visible and ?prior.order <
?row.order` with `when ?north lists ?row`, `?north` and `?row` are shared
and `?prior` is local. Every other name is local, so two sibling aggregates
can each have their own `?prior`. A declared variable (an `all` variable list, `the ?x`, `min ?x
by`) may not reuse an outside name, and an `all` head reads at least one
local variable, so `all ?x` with `?x` outside is an error (§4.5). In the
record register a `for` block is one more enclosing scope: a query inside it
reads the `for`'s variables as outside variables, already grounded. A `for
?a, ?b where C` declares its variables as an `all` variable list does, and
its condition is a nested scope (§9.4). The
checker, the reference interpreter and the engine use this one scope
function.

Three static checks. Each failure is an error that names the offending
literal or cycle.

- **Safety, by a grounding schedule.** A positive sentence grounds its
  variables. A binding `?x = e`, an aggregate's included, or a membership
  `?x in e` grounds `?x` once every outside variable `e` reads is grounded,
  and an aggregate anywhere is computed once the outside variables it reads
  are.
  Every variable must end up grounded, and the variables of a `not`, a
  comparison or a boolean expression must be grounded before it is tested;
  only a `?_` that is a whole argument of a `not` sentence needs no
  grounding. A `?_` inside an expression, such as `(?_ + 1)` in a slot or
  `?n = ?_ + 1`, is an error, under a `not` too: nothing else names it,
  and an expression is computed from grounded values, never solved
  (DECISIONS D-50). A named variable that only `not`
  literals mention stays an error, so a misspelled `?bok` never means
  "anything".
  Body order does not matter, but a cycle such as `?x = ?y` with
  `?y = ?x` never grounds, so it is an error. Every definition and view is
  checked as its own owner, whether or not anything calls it: only its
  parameters are grounded when its body starts. A handler clause is checked
  the same way, before any event arrives, with only its `on` line's
  variables grounded when its `when` starts (§5.2). The error is at the
  line of the literal that needs the variable and names the variable; when
  the variable fills an entity slot of a rule's head, it offers the domain
  literal, as `and ?hero is a Hero` (DECISIONS D-61 addendum).
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
  conflict is exactly this: `derived ?name: Text flake input url ?url:
  Text` with `at most one url per name` (F8). `galileo build` refuses to emit
  anything while a derived mode Fails.
- **When modes are checked.** A relation's modes are checked as soon as the
  relation is complete: stated facts when they load (§4.2), state facts at
  each step (§5.3), and a derived relation when its stratum is done.
  Evaluation stops at the first stratum that fails a mode, so nothing reads
  a relation whose mode Fails. Each conflicting key of that stratum Fails
  with its two smallest tuples and a proof of each, and so does each live
  key of an `exactly one` mode that has no value. `galileo check` reports them
  and answers its claims `Unknown`, naming the conflict.

### 4.5 Queries as values

`all <head> where <body> [, sorted by <expr>]` returns a list. Its head
is a list of two or more variables or one expression, a lone variable
included. An expression head is computed once for each distinct binding of
its local variables, those it reads that are not outside the query
(§4.3), and the body's other variables are existential: `all ?x where …`
gives each `?x` once, and `all ?row.label where ?north lists ?row` gives a
label per row, so two rows with one label give it twice. Distinct values
are asked for by naming them: `all ?label where ?north lists ?row and
?label = ?row.label`. A record head returns records, so a view's rows are
written `all {?row.key, ?row.position} where ?north lists ?row, sorted by
?row.position` with no binding to name the record; `?row.key` in a record
is the field `key: ?row.key` (§9.4). A
variable list returns records whose fields are the variables' names (`all
?p, ?v` gives `{p, v}` rows), one per distinct binding. Without `sorted
by`, an expression head's rows come in the canonical order of their
values, and a variable list's in the canonical order of its variables,
compared in the order the query names them. A sort key reads only the
head's local variables and the variables the query shares with its owner,
never one that only its condition binds, so each row has one key, and ties
keep the canonical order. A head reads at least one local variable, and a
record head whose two or more fields are each a variable of its own name,
`?name` or `name: ?name`, is an error that offers the variable list in
field-name order, which gives the same
rows in the same order. In a Nix body, a head is computed by Galileo with
its query, so a Nix-stage operand in it is a stage error
(A-Python-comprehension; DECISIONS D-61 addendum 2).

`the ?x where <body>` keeps one variable and returns its one binding, or
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

A view's `for North` declares a slot (§4.1), which its body reads as
`?north`, as a handler reads its event's slots, so in a sentence a variable
is always `?name` and a bare name is always an entity. A `define`'s
parameters are names, read bare in its expression body like `let` and lambda
parameters; a `?` variable there may not reuse one, and a sentence slot
reaches a parameter as a parenthesized expression, `(index)`. Each view
field carries the type the checker infers (a `the` query by §4.4's rule,
`all e` as a `List` of its head's type, `?x.f` by the field's type), so
bindings and JSON output infer nothing, and a field that cannot be typed is
an error naming it.

The default order is the canonical value order:

- `false` before `true`, and `null` before any other value;
- `Int` numerically;
- `Text` and paths by code point;
- entities by name, then by creation order;
- option paths and lists element by element, a prefix first;
- records field by field, in field-name order;
- `nix` payloads by their normalized text.

So emission is deterministic.

**Answers.** A program declares output only for a target that writes a
file: `nix file` and `nix module` for Nix (§10.2). Otherwise its relations
are its answers, and `galileo query` asks for them, as JSON with `--json`
(§11.1), so a program that targets no file has no output record. There is
no `show` declaration: a question is a command's argument (§9.4), and an
in-module question would be a second spelling of one (DECISIONS D-51).

### 4.6 Milestones

A derived shape marked `milestone` is a stable domain milestone.
Derivation-dependence changes are reported only for milestones, so
refactoring internal rules makes no review noise (R3 and the review's advice
to "expose stable domain checkpoints"). The word is not `checkpoint`, which
names a saved runtime state (§10.4).

Traces to: F6, F7, F8, N1, N2, N3, N10, P2, R3, A-Souffle, A-Flix, A-Ascent, A-Datomic-attr, A-Python-in, A-Gherkin,
A-Python-comprehension, A-egglog, A-Datafun.

## 5. State, events and effects

### 5.1 States

A state `S` is a finite set of state facts, the set of live entities, the
entity allocator (the next fresh id) and the pending effect requests,
oldest first (§5.4). Ids are never reused, so the allocator is part of the
state's identity, its hash and its checkpoint, and so are the pending
requests.
Derived facts are always relative to one state, in `M(S)`. A session keeps
every state it admits, and nothing in a state is overwritten (§5.7).

No construct joins facts from two different states, or from two revisions
of one session. This is the fix for the dungeon flaw (R1): "Ash can reach
Hall" and "Ash holds Bronze-Key" combine only if both hold in the same
state. So each `ensures` claim reads one state (§5.2). A patch contract
joins two programs, the baseline and the patched one, and not two states
of one, and each of its sentences says which program it reads (§8.3).

### 5.2 Events and handlers

An event shape declares a transition with typed arguments and a stable name:

```text
event move: Hero moves through Door
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
lines nor the clauses of an event have an order (DECISIONS D-42). A clause
is laid out as a rule is: one literal or post per line, the first opening
with `when` or `then` and every other with `and` (DECISIONS D-51). Each
clause has:

- **`when`**: rule-body literals, evaluated against `S` and `M(S)` (the
  pre-state).
- **`then`**: what the step makes true, at least one line, one line each,
  with every expression evaluated on the pre-state:
  - a sentence of a state relation holds afterwards. When the relation has
    exactly one mode, the sentence supersedes the fact at that mode's key:
    each pre-state fact that the sentence and the mode together contradict
    is withdrawn, which ends it at the next revision and keeps it in the
    history (§5.7), so `then ?hero is in ?there` moves the hero under
    `exactly one room per hero`. Without a mode the fact is added. With two
    or more modes the line does not say which key it replaces, so the fact
    is added and step 5 checks every mode: under `exactly one tile per
    unit` and `at most one unit per tile`, a move says `not ?u is on ?here`
    beside `?u is on ?there`, and a move onto an occupied tile rejects
    rather than silently taking the tile from its unit;
  - `not S`, for a state sentence `S`, withdraws `S`;
  - `?x.f = e`, for a state field `f` (§4.1), states the field's value
    afterwards: it is the sentence `T.f(?x, e)`, which supersedes the value
    at `?x` under the field's mode, so `then ?north.menu-selection = 0`
    moves the selection as `then ?hero is in ?there` moves the hero. `e` is
    any expression, evaluated on the pre-state like every `then`
    expression, so `?north.menu-row-count = ?north.menu-row-count + 1` reads
    the old count, as TLA+'s `count' = count + 1` does (A-TLA). `= null`
    withdraws a `maybe` field. In `when`, `=` binds a variable, so
    `?x.f = e` there is an error that offers `?x.f == e`;
  - `?x is a new T` creates one fresh entity of the `state entity` `T` per
    substitution, written `?x is a new T {field: value, …}` when `T` has
    fields, and gives every field whose type is not `maybe`. `?x` is bound
    by nothing else in the clause;
  - `not ?x is a T` deletes the entity and every state fact that mentions
    it. `?x` is bound by the `on` line or `when`, and `T` is a `state
    entity`;
  - a sentence of an effect shape is a request to the host (§5.4);
  - `every ?v where C: P`, where `P` is one of the posts above other than
    a `new` line, holds `P` for each binding of `C`'s own variables, as the
    claim form `every … where …:` reads (§6.2). `C` is a rule body read
    on the pre-state, opening a nested scope (§4.3) whose outside
    variables are the clause's, so `then ?hero is in ?there` and `and
    every ?trap where ?trap is armed in ?there and ?trap is telegraphed:
    ?trap warned ?hero` move the hero and warn him of every telegraphed
    trap in the room he enters, none when there is none. It is the clause
    plus `C` with `P` as its post, and lowers to one more clause of the
    event at its own line. A consequence under an extra condition is
    written so, not as a second clause that repeats the first one's
    `when` (DECISIONS D-61 addendum), and `then every ?row where ?north
    lists ?row: not ?row is a MenuRow` empties a menu with no `may skip`,
    since its clause matches with no rows. `P` reads no variable that a
    `new` line binds, and one `every` holds no other.
  A `then` line reads only variables that the `on` line binds, that a
  `when` literal grounds (a sentence, `?x = e` with any expression, an
  aggregate included, `?x in e` or `?x is a T`; §4.3), or that a `new` line of its
  clause binds, in any order. `not`, comparisons and boolean expressions
  ground nothing. `galileo check` rejects a clause that breaks this, or
  whose `when` needs a variable nothing grounds (§4.3).
- **`ensures`** (optional): state claims (§6.3) checked on the post-state,
  where a sentence marked `before` reads the pre-state (§6.2). Each claim
  reads one state: every sentence it names is marked, or none is. So
  `before ?hero is in ?here and before ?door opens for ?hero` reads the
  pre-state alone, and a claim that mixes the two is an error, since it
  would join facts from two states (§5.1). North checks these by hand
  today, in 12 Rust `Err(Protocol)` postconditions (N4).
- **`may skip`** (optional): admits the empty step when no clause matches.
  It belongs to the event: every clause of the event says it, or none
  does, and a mismatch is an error naming both clauses.

Events whose clauses make the same posts, and differ only in the values
they post, are one event (G10): what the host knows arrives as an
argument, and what the state holds is read in the clause. North offers
every menu row with one `offer-row` carrying its four texts, and the clause
reads the menu's kind and the active conversation to mark that
conversation's row `current`. No event states another event's posts, and
no block of posts is named for clauses to share: a clause is its event's
whole transition (DECISIONS D-42, D-61 addendum 5).

### 5.3 Step semantics

For one occurrence `e(args)` on state `S`:

1. **Match.** Evaluate every clause's `when` on the pre-state and collect
   every substitution: all matches contribute (N2). Action expressions are
   also evaluated on the pre-state.
2. **No match.** If no clause matched, the step is `Rejected(no match)`, with
   a why-not for each clause. With `may skip`, the empty step is admitted
   instead.

   Clause admits the empty step silently today (N2), which is why north
   re-checks postconditions in Rust. `galileo import clause` therefore marks
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
   itself, a no-op rather than a conflict. A withdrawn fact leaves the
   state, never the history (§5.7).

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
   hash `h(S')`, its net change goes into the history (§5.7), and `E` goes
   to the host.

A sequence `run [e1 … en]` is one atomic admission. Intermediate states stay
local, and any rejection rejects the whole sequence (N2). Its `E` is one
set, the union of its occurrences' requests, so two equal requests in one
sequence are one request, as in one step.

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
Galileo's own, and a rejected step has no successor. Users never write
`@next`.

### 5.4 Effects

An effect shape declares a request the host performs:

```text
effect send: send ?prompt: Text to codex for Turn
```

1. A handler's `then` names effect sentences.
2. The runtime returns them with the admitted step.
3. The host performs them and reports each outcome as an ordinary event (a
   receipt), such as `event finished: codex finished Turn with
   ?status: TurnStatus`.

This replaces north's single-slot host-effect mailbox (N3) and matches its
contract: transition, authorized effect, receipt, settlement (N6). Replay
never performs an effect again, because receipts are in the log. firn's
command plans, today `Plan: steps: Sequence<Execution>` in
`native/firn.clause` (F12), become effect requests on the same terms.

An effect shape can add two lines:

```text
effect send: send ?prompt: Text to codex for Turn
  at most one per step
  settled by finished
```

- **`at most one per step`**: two different requests of this shape in one
  step reject the step, as conflicting writes do. `galileo import clause`
  declares north's mailbox effect this way, so a step that Clause rejected
  for two competing effects still rejects (N2, N3).
- **`settled by <event>`**: an admitted occurrence of that receipt event
  settles the oldest pending request that agrees with it on every slot
  the two shapes both name. The receipt's other slots are free: a
  `finished` receipt settles the oldest pending `send` for its `turn`,
  whatever its `status`. Slots match by name, as mode and domain lines
  name them, never by position. The receipt must name every entity slot
  of the effect, and a slot both name has one type in both; otherwise the
  `settled by` line is an error that names the slot, since that receipt
  could never say which request it settles (DECISIONS D-55). Requests
  not yet settled are pending. Only a shape with `settled by` has pending
  requests: one without it is returned with its step and never pending,
  since nothing could settle it. Pending requests are part of the state
  and its checkpoint, and after a restore `Session::pending()` lists them,
  so the host can reconcile them under its own contract (N6). Galileo never
  performs an effect itself.

### 5.5 Transition systems and temporal questions

A program with events defines a transition system: the states reachable from
the initial state by admitted steps.

**Argument domain.** An entity slot ranges over the live entities of its
type, and a text-union slot over its values. An `Int` or `Text` slot is open
unless its event closes it:

```text
event switch-mode: North switches to ?mode: Text
  mode only in ["normal", "insert"]
event type-query: North types ?query: Text
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
budgets for states, depth and work. A search node is a state without
history (§5.7), so two paths that reach one state merge, and a witness
gets its history when it is replayed (§7.3).

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

given Door leads from ?here: Room to ?there: Room
given Door is unlocked
given Door is locked by Key
  at most one key per door
given Trap is armed in Room
given Trap is telegraphed

state Hero is in Room
  exactly one room per hero
state Key lies in Room
  at most one room per key
state Hero holds Key
state Trap warned Hero
state Trap hurt Hero

derived Door opens for Hero

event Hero moves through Door
event Hero picks up Key
event Trap strikes Hero

rule unlocked-doors-open: ?door opens for ?hero
  when ?door is unlocked
  and ?hero is a Hero

rule held-keys-open: ?door opens for ?hero
  when ?door is locked by ?key
  and ?hero holds ?key

on ?hero moves through ?door
  when ?hero is in ?here
  and ?door leads from ?here to ?there
  and ?door opens for ?hero
  then ?hero is in ?there
  and every ?trap where ?trap is armed in ?there and ?trap is telegraphed: ?trap warned ?hero

on ?hero picks up ?key
  when ?hero is in ?room
  and ?key lies in ?room
  then not ?key lies in ?room
  and ?hero holds ?key

on ?trap strikes ?hero
  when ?trap is armed in ?room
  and ?hero is in ?room
  then ?trap hurt ?hero

requirements for the level
  possibly Ash is in Vault
  Ash is in Vault only after Ash is in Library
  possibly Ash is in Vault has no single point of failure among ?key lies in ?room
  ?trap strikes ?hero only when ?trap warned ?hero
  never Ash is in Shrine
```

(Entity declarations, doors and `initially` are as in the prototype's
`dungeon.clause`, the declarations one line per type, such as `[Entrance,
Hall, Library, Armory, Crypt, Vault, Shrine] is a Room` (§4.2). Ash starts
in Entrance, the Bronze-Key lies in Library and the Iron-Key in Crypt. The
events take the names their templates give, `moves-through`, `picks-up` and
`strikes` (§4.1).)

**Expected answers.** These match an independent `(room, keys held)` search
(R1). The prototype got the third and the Silver-Key placements wrong.

| Requirement or question | Answer |
| --- | --- |
| `possibly Ash is in Vault` | Holds. Shortest witness, 5 steps: through Front-Door, through Library-Arch, through Crypt-Stair, picks up Iron-Key, through Crypt-Door |
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

Traces to: R1, R2, R17, N2, N3, N4, N6, F12, A-Dedalus, A-Statelog, A-TLA,
A-Upsert, A-EventCalculus, A-Alloy6, A-Apalache, A-telingo.

### 5.7 History: nothing is overwritten

A session keeps every state it admits (DECISIONS T5's principle 18, D-52).
Revision 0 is the state `initially` gives. Each admitted step `r → r+1`
makes the next, and a rejected step makes none. A step admitted without
changing the state, such as `may skip`'s empty step or one whose sentences
already hold, still takes its revision: the state and its hash repeat, the
change is empty, and its `E` still goes to the host (§5.3 step 6,
DECISIONS D-56). A step never edits a state
in place: what it withdraws stops holding at `r+1` and stays in the
history, so the state at any revision can still be asked about.

**Spans.** The history holds one record per fact per period it held: the
revision it started holding and, once it stops, the revision it stopped.
That pair is its span `[from, until)`. Live entities have spans the same
way, and the history keeps each revision's allocator. The state at revision
`r` is every fact and live entity whose span contains `r` (`from ≤ r <
until`), with that revision's allocator. An admitted step closes the span of
each fact in `S \ S'` and opens one for each fact in `S' \ S`, both at
`r+1`, so the history changes exactly where the state does:

- **A sentence of a relation with one mode** adds its fact and supersedes
  the fact at that mode's key. The old fact's span ends at `r+1`, the new
  one's starts there, and both stay. This is what "the current state is the
  latest fact per key" means: step 5 holds each key to at most one fact
  per state, so a key's open span is its value now and its closed spans
  are its past values, in revision order. Under `exactly one`, a live key's
  latest record is always a value. Under `at most one`, `then not` can make
  it a withdrawal, which leaves the key with no value.
- **Under no mode, or two or more** (§5.2), a fact's key is the whole fact.
  A sentence opens a span when the fact did not hold, and `then not S`
  closes one when it did.
- **A sentence whose fact already holds records nothing,** and neither does
  `then not S` beside `then S`. The state did not change (§5.3 step 4), so
  the span that holds the fact goes on.
- **`?x is a new T {…}`** opens the entity's live span and its field facts'
  spans at `r+1`, and a `state entity`'s declaration under `initially`
  opens them at the session's first revision (§4.1). **`then not ?x is a
  T`** closes them, along with the span
  of every state fact that mentions the entity. Ids are never reused (§5.1),
  so the entity and its fields can still be asked about as of any revision
  in its life. A creation field has one span (§4.1); a state field is a
  relation with one mode, so it has a span per value, as above.

A sequence `run [e1 … en]` is one revision, and its intermediate states
are not kept, since they were never admitted. Pending effect requests are
state (§5.4), so the history keeps them as it keeps facts. Effects and
receipts are in the event log, not in the history.

**The log and the hash.** The history is derived from the event log
(§10.4). A step is deterministic (§5.3), so replaying the log from
`initially` builds the same history. The revision hash stays `h(S)` over
the state alone (§5.1), never over the history. That lets search merge
two paths that reach one state (§5.5), and lets a checkpoint restored after
any step continue with the same hashes. The history's integrity check is
that its state at each revision hashes to the hash the log recorded there.

**Now, and a revision.** Every construct of a program reads one state:

- a rule, a query and a requirement read the state being evaluated;
- a handler's `when` reads its pre-state;
- an invariant or `ensures` reads its post-state, where `before` reads the
  pre-state.

No construct names a revision or reads the history, so a program means the
same in every run, and §5.1's rule holds over time. A program that needs
its own past states it, as Dedalus states persistence: `{hero} has held
{key}`, written by the clause that makes it true and never withdrawn.
Temporal claims (§6.2) are about the transition system, never about one
run. Only questions read history (§7.3, §10.4):

- `--as-of R` answers in the state at revision `R`;
- `--since R` lists how an answer changed after `R`;
- `galileo why` names the occurrences that made a state fact true.

**Cost.** The state is the materialized current view. A step reads and
writes only the state, then appends its net change to the history, so it
costs what it would without the history, plus the change. The step core
builds that change from its own `W`, `I`, `C` and `D`, never by diffing
whole states. A question about now never reads a span. An as-of question
selects the spans that contain its revision and evaluates rules on that
one state. A since question evaluates only at the revisions where a span of
a relation it depends on opened or closed. No question re-runs events: the
states come from the history, and `why` re-matches only the occurrence it
explains, on its stored pre-state. The history begins at a session's first
revision: 0, or 1 for a session converted from a Clause checkpoint, which
kept no history and names its revisions by hash (§10.4). An as-of before
that is an error naming it. The history grows with change, and compaction
is not yet in the MVP (§12.1).

**Formal anchor.** A span is the set of timesteps at which Dedalus's `p@t`
holds (A-Dedalus). It is also a system-versioned temporal table's period:
the state is the current table, and `AS OF t` selects `from ≤ t < until`
(A-Temporal). An as-of answer is Datomic's as-of database (A-Datomic). The
step semantics of §5.3 do not change: the history records what they admit.

Traces to: R1, N5, A-Dedalus, A-Temporal, A-Datomic, A-EventCalculus.

## 6. Requirements and verdicts

### 6.1 Verdicts

A verdict is one of three values:

- **`Holds(evidence)`**: the evidence is a proof tree, a witness trace or a
  completeness record of what was searched.
- **`Fails(counterexample)`**: a proof of a forbidden fact, a trace, or the
  binding that fails. For `S requires C` it is the binding and the proof of
  `S` that still holds with `C` blocked, each rule at file:line, so the
  bypass is named (DECISIONS D-64). For a single point of failure over a
  sentence no proof survives, since removing the fact `F` makes `S` false,
  so it is the binding, `S holds only through F:` and the proof of `S`
  before the removal, every proof of which depends on `F` (DECISIONS
  D-75).
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

`galileo check` exits 0 when every claim Holds and 1 when any Fails. It exits
2 otherwise: when none Fails but some are Unknown, and when it reaches no
verdict because of a static error, unreadable input or bad usage. A static
error prints to stderr as `FILE:LINE:COLUMN: message` and then its source
line, the one form every command prints, and no verdict is printed. Static
checks cover the whole program, requirements included, and every error that
lowering to a target finds, such as a computed name's (§10.3), before any
claim is evaluated (DECISIONS D-68). A warning, such as a fact stated twice
(§4.2), prints and the check goes on.

### 6.2 Requirement forms

Every claim is evaluated over the program's transition system. A program
without events, such as all of firn, has exactly one state, so there `S`,
`always S` and `possibly S` coincide, and `never S` means `S` is false.

**Quantifiers (R15).** Every variable a claim reads must be bound, by one
of:

- `some`, `every` or `no`;
- the form itself. In `S requires C`, `S has no single point of failure
  among P`, `S only after C` and `E only when C`, the variables of the left
  side are universal: the claim is checked for every binding under which it
  holds or occurs. Variables that appear only in `C` or `P` are pattern
  variables. In `never S` they are existential; `possibly S` and
  `always S` bind nothing; in `ensures`, the clause's variables are
  bound (§5.2), and a patch contract binds nothing, so its claims are
  bound as a requirement's state claims are (§8.3). A temporal claim on
  the left of `has no single point of failure` binds as it does alone,
  so `possibly ?h is in Vault has no single point of failure among ?k
  lies in ?r` is an error naming `?h` and `possibly`;
- a definition: `?x = e`, an aggregate included, which gives one value,
  so it needs no quantifier and is visible to the
  rest of its claim. A membership `?x in e` gives one value per element, so
  it binds nothing here: a bare `?x in [a, b]` would be R15's implicit
  `some`.

A variable that appears only inside an aggregate or query is local to it
(§4.3), and each `?_` is a fresh variable, existential where it stands. Any
other variable is an error: in a sentence or a quantified claim it offers
`some`, `every` or `no`, and elsewhere it names the form, such as
"`possibly` takes a sentence without variables". One core function
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
| `possibly S` | some reachable state satisfies `S`: one witness run reaches it, and other runs need not |
| `always S` | every reachable state satisfies `S` |
| `never S` | no reachable state satisfies `S` (with `S`'s variables existential) |
| `S requires C` | Counterfactual dependence, in every reachable state (below). For every binding under which `S` holds, blocking every fact that matches `C` under that binding (removed, and not derivable) makes `S` false. For monotone cones this equals "every derivation passes through `C`" (`requires_iff`, P1). Under negation it is counterfactual. |
| `S has no single point of failure among P` | In every reachable state (below), for every binding of `S` that holds, `S` still holds after removing any one stated fact that matches `P` |
| `S only after C` | for every binding of `S`'s variables, every path from the initial state to a state where `S` holds under it visits a state where `C` holds under it, at or before. `C`'s other variables are existential in that state |
| `E only when C` | every admitted occurrence matching event pattern `E` has `C` in its pre-state, under the occurrence's binding of `E`'s variables. This replaces the stated "trap is telegraphed" fact (R2) |
| `<temporal claim> has no single point of failure among P` | `P` ranges over initial facts, and the claim must still hold with any one of them removed |

**`possibly`, and `eventually` reserved.** `possibly S` is reachability:
one witness run proves it, and other runs may never reach `S`. Temporal
logic (LTL's F, TLA+'s `<>`, Alloy 6's `eventually`) and model checkers'
query languages (UPPAAL calls `A<>` "eventually" and `E<>` "possibly") read
a requirement `eventually S` as every run reaching `S`, which Galileo does
not check yet (§12.1). So `eventually S` is a static error that offers
`possibly S` (DECISIONS D-64).

**Dependency claims in every reachable state.** On a program with events,
`S requires C` and a single point of failure over a sentence are judged in
every reachable state, through the search of §5.5, as `always S` is
(DECISIONS T6). In each state the claim means what the table says, with
that state's facts in place of the initial ones, so the stated facts a
single point of failure removes include that state's facts. The claim is
universal over states and bindings:

- it Holds only when the search is complete and every binding passes in
  every reachable state, "across all N reachable states (complete)", and
  it is vacuous only when its sentence holds in no reachable state;
- it Fails at the first state the search reaches where a binding fails,
  and the counterexample is the trace of event sentences that reaches that
  state, the binding, and §6.1's counterexample: the proof of `S` that
  holds with `C` blocked, or the proof that depends on the removed fact;
- otherwise it is `Unknown` with the search's reason, such as an open
  domain or a budget (§5.5).

`--events` does not change it, as it does not change a temporal claim
(§10.4). So a bypass that fires only after some events fails `galileo
check`, and a claim whose sentence has no instance in the initial state is
judged in the states where it has one. A program without events has one
state, so there the claim is judged as before.

`has no single point of failure` applies to a sentence or a temporal claim;
over a quantified claim, `requires` or another such claim it is a static
error. So a requirement for every node leaves its variable free, as
`requires` does: `?n holds has no single point of failure among ?n has ?b`
says each node that holds still holds without any one of its own `has`
facts. `every ?n where ?n is a Node: ?n holds has no single point of
failure among ?n has ?b` reads as one over the quantified claim, and is
that error (DECISIONS D-60).

**Two-state claims** (A-Dafny). `before` marks one sentence of a claim,
as `before S` or `before not S`, and the marked sentence reads the
earlier of two states: the pre-state in `ensures` (§5.2) and the baseline
in a patch contract (§8.3). Every other sentence reads the later state,
so no word marks it. `after S` is an error that offers `S`, since `only
after` already uses the word for order along a run. The mark opens its
literal, so `not before S` is an error that offers `before not S`. Only a
sentence reads a state, so a mark on a comparison, a definition, a
membership or an aggregate is an error, and every expression and
aggregate is evaluated in the later state. A mark binds nothing: a
variable names one value in both states, wherever a positive sentence
grounds it. A mark in a requirement or a question is a static error
(DECISIONS D-70).

### 6.3 Where requirements live

- **Placement.** `requirements` blocks can appear in any module. They are
  part of the program, so they are part of its baseline.
- **Checking.** `galileo check` checks them, and review checks them on both
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
    (checked: `hosts/whiterabbit/tags.gal`)", and with no such file,
    every stated fact of the program. For a derived relation it is
    "nothing derives" and the rules that conclude it, with file:line; for
    a state relation, the current state. The scan reads only that
    relation's rows and charges each one. A `?_` prints as written and
    stands for every value, "nothing states `ben has read ?_`", and with
    `?_` as the first argument the scope is every file that states the
    relation;
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
- `galileo why not S --via <rule>` fixes the rule (A-Souffle
  `explainnegation`).

### 7.3 State facts over time, and traces

A run's history (§5.7) answers three questions. Each takes `--events FILE`,
which gives the run (§10.4), and `--as-of R`, which picks the state at
revision `R`. Without `--as-of`, the state is the one after FILE.

- **`galileo why S --events FILE [--as-of R]`** explains a fact by the
  occurrences that made it true, a chain of title. First it prints `S`'s
  proof at `R` as §7.1 does, with each state leaf ending `since revision
  k`. Then it prints one block for each occurrence the proof reaches,
  latest first, each block once. A block's first line names the
  occurrence and its line in FILE. When one clause under one substitution
  added every fact the proof reaches from that occurrence, the first line
  also names the clause, and the body is that clause's `when` proof in the
  state before the occurrence. When two clauses of the event, or one clause
  under two substitutions, added them (N2), the body has one part for each,
  in canonical clause and substitution order (§5.3): a line `by the clause
  at <file:line>:`, then that `when` proof one level deeper, so each
  condition stays with its clause. A fact that two of them added is
  credited to the first (DECISIONS D-56). State leaves again say since when
  they held. A leaf that has held since
  revision 0 comes from `initially` and names that line. A fact that holds
  at `R` and opened at `k` was not withdrawn in between, which is the event
  calculus's inertia (A-EventCalculus). Rejected events made no revision,
  so no chain names one. The work budget bounds the chain, and a chain cut
  short ends with `earlier causes not shown: out of budget` (G9). A fact
  that does not hold at `R` exits 1 and points to `why not`. Here is §5.6's
  dungeon with a six-line `walk.events`: Ash moves through Front-Door, then
  Armory-Gate (rejected, since it is locked), then Library-Arch; picks up
  Bronze-Key; moves through Crypt-Stair; and picks up Iron-Key. Line numbers
  are those of `crates/galileo-syntax/tests/dungeon.gal` at D-52, and the
  proofs of `opens for` are shortened:

  ```text
  $ galileo why Ash holds Iron-Key --events walk.events
  Ash holds Iron-Key, since revision 5

  revision 5: Ash picks up Iron-Key (walk.events:6), by the clause at dungeon.gal:104, when at revision 4:
    Ash is in Crypt, since revision 4
    Iron-Key lies in Crypt, since revision 0 (dungeon.gal:51)
  revision 4: Ash moves through Crypt-Stair (walk.events:5), by the clause at dungeon.gal:86, when at revision 3:
    Ash is in Library, since revision 2
    Crypt-Stair leads from Library to Crypt (dungeon.gal:33)
    Crypt-Stair opens for Ash, by unlocked-doors-open (dungeon.gal:74)
  revision 2: Ash moves through Library-Arch (walk.events:3), by the clause at dungeon.gal:86, when at revision 1:
    Ash is in Hall, since revision 1
    Library-Arch leads from Hall to Library (dungeon.gal:27)
    Library-Arch opens for Ash, by unlocked-doors-open (dungeon.gal:74)
  revision 1: Ash moves through Front-Door (walk.events:1), by the clause at dungeon.gal:86, when at revision 0:
    Ash is in Entrance, since revision 0 (dungeon.gal:49)
    Front-Door leads from Entrance to Hall (dungeon.gal:24)
    Front-Door opens for Ash, by unlocked-doors-open (dungeon.gal:74)
  ```

  Revision 3, the Bronze-Key pick-up, is not in this chain, and neither is
  line 2, the rejected move through the Armory-Gate. When `all set` needs
  `Ash is ready` and `Bo is ready`, and one `start` made both through two
  clauses, its block reads:

  ```text
  revision 1: start (start.events:1), when at revision 0:
    by the clause at team.gal:13:
      Ash trained (team.gal:5)
    by the clause at team.gal:16:
      Bo trained (team.gal:6)
  ```
- **`galileo why not S --events FILE [--as-of R]`** computes §7.2's near
  misses in the state at `R`. For a state sentence that held before `R` it
  adds when it last stopped and why: `held from revision a until revision
  k:` followed by `superseded by S2` when a fact at the same one-mode key
  opened at `k`, `deleted with E`, or `withdrawn`, and then the occurrence
  at `k` as `why` prints it. A sentence that never held says `never held
  in revisions 0 to R`. A missing state condition of a near miss is
  annotated the same way.
- **`galileo query Q --events FILE --since R`** prints `Q`'s answer at `R`,
  then one line for each later revision at which it changed (§10.4).

Witnesses:

- **`trace S`** prints a witness trace as event sentences, one per line:
  an events file that `galileo run` admits and `why … --events` explains.
- **`replay <trace>`** runs a trace through the runtime and confirms `S`, so
  every reported witness is executable (the ChatGPT review's "make the proof
  do something").

### 7.4 `prevent` and `achieve`

Both are interventions over a declared vocabulary, the relations that
`--using` names. Each takes a claim (§6.2), so §5.6's Silver-Key placements
are `galileo achieve possible possibly Ash is in Shrine --using lies-in`:

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
debugger"), A-Souffle, A-KR2026, A-why-depends, A-EventCalculus,
A-Datomic, A-Temporal, A-OPA (residual "what must hold" is post-MVP).

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
- **Requirements.** A requirement's name is its claim as `galileo fmt`
  prints it, without `invariant`: a label, not an identity. Review pairs
  requirements by their lowered claims up to variable names, with the
  `invariant` flag, since one text can be two claims against two
  vocabularies (R16). A requirement repeated with the same lowered claim is
  a warning and is lowered once, as a repeated fact is (§4.2).

### 8.2 Patches

A patch is one of two things:

- **Derived**, from two program images, such as the git base and the working
  tree. It may change any declaration, a shape's kind included (§8.4).
- **Written** as a `patch` block in a restricted delta language, with three
  section words, each naming what it touches:
  - `add` facts and entity declarations, and `add rule <name>`. Each fact
    goes by its shape: a `given` sentence is stated as at the top level,
    and a `state` sentence or a field fact `f = e` joins the initial
    state, as under `initially`, a `state entity`'s declaration and its
    creation fields included (§4.1). So a focus block in `add` may hold a
    declaration, given facts and state facts, and a patch places an item
    as it takes one away (DECISIONS D-73);
  - `remove` facts (each a baseline-stated fact) and `remove <name>` for a
    rule or definition;
  - `replace rule|definition|handler <name>` and `replace requirements
    [for …]`, each with the whole new block beneath.
  One word per edit: a patch removes a fact as it removes a rule. Two more
  sections, `says` and `intends`, at most one of each, hold the patch's
  contract (§8.3) and add nothing to the patched program.

Three restrictions keep the baseline fixed (R9: today a proposal can install
a rule in its own baseline):

1. A patch cannot contain declarations outside those sections.
2. It cannot redefine a baseline name except through `replace`.
3. It is parsed against the frozen baseline's namespace.

The baseline is a program of modules (§2), so each edit lands in one. A
`remove` of a fact deletes it from every module that states it. `replace`
and `remove <name>` act in the module that declares the name, and a name
declared in two modules is an error that names both. `replace requirements
for …` replaces the block with that header in the module that holds it.
`add`, `add rule`, a new requirements block and the `intends` claims lower
in one more module that uses every baseline module, so every exported shape
reads there, and a line that two exports read two ways is an error that
names both (§2). An embedded library module belongs to the `galileo`
binary, so a patch whose edit lands in one is an error that names it
(DECISIONS D-73).

### 8.3 Contracts

A written patch states its contract in two sections (§8.2). Neither joins
the patched program, so its identity, requirements, truth view and edits
are its edits' alone. `galileo check` never reads a contract, and review
checks it once, against the frozen baseline and the patched program.

**`says "…"`** holds one text, shown and never trusted (R20). This idea
comes from the prototype.

**`intends`** lists checked claims about the change, one per line beneath
it:

- a state claim (§6.3): a sentence, or a `some`, `no` or `every` claim.
  A sentence marked `before` reads the baseline, and an unmarked one reads
  the patched program (§6.2). Expressions, definitions and aggregates are
  evaluated in the patched program. On a program with events, both sides
  are read in their initial states;
- inside such a claim, `S becomes true`, `S becomes false` and `S stays`,
  for one sentence `S`, stand for `before not S and S`, `before S and not
  S` and `before S and S`, so they compose under `some`, `every` and `no`.
  Writing out the long form in one condition is an error that offers the
  short one;
- `only P changes`, for one sentence `P`: every fact of `P`'s shape that
  is true on exactly one side matches `P`, whose variables are pattern
  variables. So `only ?m is enabled on whiterabbit changes` says that no
  other host's enables move;
- `requirements unchanged`: §8.1's pairing leaves no requirement over on
  either side.

A contract binds nothing, so each claim is bound as a requirement's state
claim is (R15, §6.2): `some`, `every`, `no` or a definition binds each
variable, and any other variable is an error that offers them. One
variable names one value on both sides. Any other claim form is an error
in `intends`, since a lasting claim about the patched program is a
requirement, written with `replace requirements`.

A patch that takes firefox out of the browsers tag, and means to keep
every module that a responsibility still needs:

```text
patch firefox-leaves-browsers
  says "firefox leaves the browsers tag"
  remove
    firefox is tagged browsers
  intends
    firefox is enabled on whiterabbit becomes false
    every ?m, ?h, ?s where before ?m is enabled on ?h and ?h must realize ?s and ?s is realized by ?m: ?m is enabled on ?h
```

Its last claim reads: every module enabled on a host before the patch,
which that host must still realize software through, is enabled after
it.

**Evaluation.** An intended claim is checked as a requirement is (§6.1),
on the patched program plus, for each shape that one of its `before`
sentences reads, a given relation that holds the baseline model's facts
of that shape under a name no program can spell. So the checker that
decides requirements decides contracts too (DECISIONS D-70). The
baseline's model is computed once under the review's work budget, and
when it runs out, every claim with a `before` sentence is `Unknown`.

A failed intended claim rejects the patch. This catches a change that keeps
every requirement but does not do what it set out to do.

A derived patch, such as a git diff, carries no contract: `galileo review
--base` checks its requirements alone (§12.1).

### 8.4 What review computes

1. **The edits**, as sentences. A derived patch's edits are the stated
   facts added and removed and the declarations added, removed or
   replaced, a shape's changed kind included.
2. **Truth view.** Derived facts gained and lost. A fact true on both
   sides whose origin moved between stated and derived is reported as "now
   stated" or "now derived", not as lost or gained (R11). A shape's kind
   fixes where its facts come from (§4.1), so a fact moves only when its
   shape changes kind, from `derived` to `given` or back. Only a derived
   patch can do that, as a refactor that replaces a rule by the facts it
   derived does; a written patch cannot redeclare a shape, and its `add`
   of a derived shape's sentence is an error (§4.2). Under either kind of
   patch, a derived fact whose support changes, as when one rule stops
   deriving it and another starts, is neither gained nor lost (DECISIONS
   D-50).
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
   option values set in Galileo, generated files and flake inputs.
8. **Definitions behind requirements.** Each added, removed or replaced
   rule, definition or derived shape is listed with every requirement and
   milestone whose claim reads it, directly or through other rules, before
   and after, whether or not a verdict changed. A requirement can keep
   holding while a word it reads, such as `approved`, changes meaning; the
   list informs the reviewer and does not change the verdict (DECISIONS
   D-64).

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

A derived patch, such as a git diff, has no declarations and no contract.
Any requirement change in it is listed first, and approval then needs
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
- **Variables** are `?name`; `?_` is anonymous: each `?_` is its own
  variable, existential where it stands, and under a `not` that is inside
  the `not` (§4.3, §6.2). It stands only as a whole argument of a
  sentence; inside an expression it is an error (§4.3), and every
  diagnostic prints it as `?_` (DECISIONS D-50). A bare `_` is an error
  that offers `?_`. `?` marks every blank a sentence leaves open, and
  only those: a slot in a template (`?name: Type`, §4.1), a fact table's
  column (§4.2), and every variable of a rule, clause, claim, view or
  query at every occurrence, the `on` line's included. So a bare token in
  a sentence is always an entity or a template word, a line shows by
  itself which of its slots are fixed, and it reads the same pasted into
  a handler, a rule, a claim or a question. A `define`'s parameters, `let`
  names and lambda parameters are names in an expression, not blanks, and
  stay bare (§4.5; DECISIONS D-41, D-61 addendum).
- **Names.** Entity names are tokens such as `Front-Door`, `whiterabbit`,
  `kanata` or `north-main`. A type's name begins with a capital letter
  (`Hero`, `MenuRow`), and in a template every word that begins with one is
  a type, so a template word that must be capitalized is written in
  backticks (§4.1, DECISIONS D-61). A name that is a keyword is written in backticks
  where the grammar would read the keyword. Fourteen keywords are read
  wherever an expression is due, whatever follows them: `all`, `and`,
  `else`, `false`, `if`, `in`, `not`, `null`, `or`, `then`, `true`,
  `when`, `where` and `with`. A name spelled like one of them takes
  backticks in every expression, a sentence's slot token (§9.2), a binder
  and a field name included: `` `in` runs `true` ``, `` x.`when` ``. An
  entry's key reads `when` and `for` as keywords, since they end its value
  or open a `for` block (§9.4). Every other word of §9.4's grammar is one
  reading among the templates' (§9.4) or opens its form only before the
  word or `?` variable that form needs next, as `the ?x` and `nix module`
  do, so a name spelled like it needs backticks only where two readings
  fit, as a template word does (DECISIONS D-61 addendum 2; notebook
  DECISION 2.21i). A name that is also a word of
  some template in scope needs none: a slot holds one token, that token
  must name a declared entity of the slot's type (§4.2), and exactly one
  typed reading must remain (§9.2), so `x is realized in boot` reads while
  a template says `{host} has boot responsibility {r} …`. Backticks then
  only settle a line that two readings fit, since a quoted name never
  matches a template word (DECISIONS D-51). Backticks work in every position
  that takes a name, binders included, and hold one bare name:
  `` let `when` = true `` binds `when` (DECISIONS D-34). In a Nix body
  they may instead hold one `{e}`, a computed name: the name that the
  known `Text` `e` spells (§10.3). Anything else between backticks, such
  as `` `a b` `` or `` `a{e}` ``, is an error at its line (DECISIONS
  D-65).
- **Text literals** are `"…"`. Escapes are `\" \\ \n \t \{`, and `{expr}`
  interpolates a `Text`, an `Int` (in decimal, computed by Galileo), a
  `Path` or a Nix value, which Nix coerces as it does in `"${…}"`. In a Nix
  body, an interpolated name that a `let` binds to a known `Text` emits
  `${name}`, as that name does anywhere else there: the `let` is emitted,
  so Nix computes the same text. Galileo computes the text where Nix's
  interpolation would not give it, as with an `Int`, which Nix refuses to
  interpolate. It also computes the text where Nix's interpolation cannot
  be written: a prelude call, a `?` variable, an aggregate or a query.
  Every other known interpolation is computed (DECISIONS D-66). Nix copies
  an interpolated path into the store and the text holds that store path,
  which only Nix evaluation knows, so Galileo never computes or substitutes
  a `Path`, known or not: it emits the expression as written inside `${…}`,
  and the text is `Nix Text` (§3.2). `"{./x}/y"` builds to `"${./x}/y"`. A
  known `Bool`, list, record, `null` or `maybe` value is an error, as in
  Nix (DECISIONS D-48 addendum).
- **Text blocks** open with `'''` at the end of a line and close with `'''`
  at the start of a later line, after nothing but spaces, exactly 2 spaces
  deeper than the line that opens the block (L1); a closer at any other
  depth, or content left of it, is an error, so a forgotten closer never
  swallows the lines after it. What follows the closer on its line
  continues the line that opened the block, so a text block can stand
  inside a call, a list or a record, and that rest may open another block,
  which closes at the same column:
  `text: '''` … `'''})` and `'''), extraInstallCommands: '''` (DECISIONS
  D-59). Content is dedented to the closing marker's column and is raw: no
  escapes and no comments, so a line whose first non-space characters are
  `'''` is always a closer. `{{expr}}` interpolates. The value is every content line followed by a
  newline, which matches Nix `''` strings (F4).
- **Paths** are `./x`, `../x` and `/x`. A `/` followed by a space or the end
  of its line is division (§9.4), so division is written `a / b`, as
  subtraction is `a - b`, and `a /b` is an error that offers `a / b`
  (DECISIONS D-83).
- **Integers** are written in decimal. A literal such as `1.5` is reserved
  for fractional numbers (§12) and is an error in the MVP.

### 9.2 Two registers, fixed by block headers

- **Sentence register:** shapes, facts, rules, handlers, requirements,
  patches, and the sentences and claims that commands ask about.
- **Record register:** `nix module` bodies, `view` bodies and inline record
  values.

The enclosing header decides the register, so a line's meaning never depends
on guessing it.

The two registers are two frames over one expression grammar (§9.4), not
two languages. A sentence-register line is a sentence of a relation, and a
record-register line is a key and its value, because the record register
writes Nix and its shape is the NixOS module's (DECISIONS D-38, D-39).
Operators, calls, `if`, `in`, field access, lists, records, text and
queries are written and mean the same in both. The words they share mean
the same: `when` is a condition, whether it opens a clause's line or ends a
Nix entry; `.f` reads a field, `?row.label` an entity's and `cfg.port` an
option's value. A binding is a `?x = e` literal in a sentence owner and a
`let` in a Nix body, since one is found by matching and the other is
computed once in order (§2, §4.3; DECISIONS D-61 addendum).

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

### 9.3 Galileo is not English

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
decl        = type-decl | shape | entity-decl | fact | focus-block | table
            | initially | rule | handler | define | view | requirements
            | foreign | nix-module | nix-file | patch | field-block
foreign     = "foreign" "nixos-options" "from" path
type-decl   = "type" Name { "," Name }
            | "type" Name "=" text-literal { "|" text-literal }
shape       = kind [ "milestone" ] [ name ":" ] template
              [ INDENT { shape-line } DEDENT ]
kind        = "given" | "state" | "derived" | "event" | "effect"
template    = { word | Name | "?" slot ":" type }
shape-line  = mode-line | domain-line | "at most one per step" | "settled by" name
mode-line   = ("exactly one" | "at most one") slot "per" slot { "," slot }
domain-line = slot ("only in" | "sampled from") list
entity-decl = "state" "entity" Name [ INDENT { field ":" type } DEDENT ]
field-block = ("state" | "derived") "for" Name INDENT { field ":" type } DEDENT
initially   = "initially" INDENT { sentence | focus-block | table | name "." field "=" expr } DEDENT
table       = sentence INDENT { token { token } } DEDENT
rule        = "rule" name ":" head INDENT "when" literal { NEWLINE "and" literal } DEDENT
head        = sentence | "?" var "." field "=" expr
literal     = sentence | "not" sentence | expr cmp expr | expr
            | "?" var "=" expr | "?" var "in" expr
aggregate   = "count" "where" conj | ("sum" | "min" | "max") expr "where" conj
            | ("min" | "max") "?" var "by" expr "where" conj
conj        = literal { "and" literal }
claim       = [ quant vars [ "where" conj ] ":" ] conj
            | ("possibly" | "always" | "never") conj
            | sentence "requires" sentence
            | claim "has no single point of failure among" sentence
            | conj "only after" conj
            | sentence "only when" conj
quant       = "some" | "every" | "no"
vars        = "?" var { "," "?" var }
handler     = "on" sentence INDENT [ "when" literal { NEWLINE "and" literal } NEWLINE ]
              "then" post { NEWLINE "and" post }
              [ "ensures" INDENT { claim } DEDENT ] [ "may skip" ] DEDENT
post        = sentence | "not" sentence | "?" var "is a new" Name [ record ]
            | "?" var "." field "=" expr | "every" vars "where" conj ":" post
define      = "define" name "(" params ")" ":" type [ INDENT expr DEDENT ]
view        = "view" Name "for" slot-list INDENT { field ":" expr } DEDENT
requirements= "requirements" [ "for" words ] INDENT { ["invariant"] claim } DEDENT
nix-module  = "nix module" INDENT "arguments:" args { module-item } DEDENT
nix-file    = "nix file" INDENT [ "arguments:" args ] { "let" name "=" expr }
              "value" ( ":" expr | INDENT { entry } DEDENT ) DEDENT
args        = name { "," name } [ "," "..." ]
module-item = "let" name "=" expr
            | "option" attrpath ":" option-body
            | "option" attrpath INDENT { attrpath ":" option-body } DEDENT
            | "imports:" expr
            | "config" [ "when" expr ] INDENT { entry } DEDENT
            | for-block
option-body = ( type | "enable" expr ) [ INDENT { opt-field } DEDENT ]
for-head    = "for" "?" var "in" expr
            | "for" vars "where" conj [ "," "sorted by" expr ]
for-block   = for-head INDENT { entry } DEDENT
entry       = attrpath ":" expr [ "when" expr ]
            | attrpath [ "when" expr ] INDENT { entry } DEDENT
            | for-block
attrpath    = seg { "." seg }      seg = ident | text-literal | "{" expr "}"
expr        = value-literal | path | ident | "?" var | list | record
            | "(" expr ")" | expr "." seg | call | lambda
            | "if" expr "then" expr "else" expr | expr binop expr
            | "not" expr | "-" expr | "with" expr ":" expr | query | aggregate
            | computed | "nix module" "(" args ")" INDENT { entry } DEDENT
computed    = "`" "{" expr "}" "`"
value-literal = integer | text-literal | text-block | "true" | "false" | "null"
binop       = "or" | "and" | cmp | "//" | "+" | "-" | "*" | "/" | "++"
cmp         = "==" | "!=" | "<" | "<=" | ">" | ">=" | "in"
list        = "[" [ expr { "," expr } ] "]"
record      = "{" [ rfield { "," rfield } ] "}"
rfield      = { for-head ":" } ( attrpath ":" expr | "?" var [ "." ident ] )
query       = "all" ( "?" var "," vars | expr ) "where" conj [ "," "sorted by" expr ]
            | "the" "?" var "where" conj
call        = expr "(" [ expr { "," expr } ] ")"
lambda      = "(" params ")" "=>" expr
params      = param { "," param }
param       = name [ ":" type ]
            | "{" formal { "," formal } [ "," "..." ] "}"
            | "{" "..." "}"
formal      = { for-head ":" } ( name | computed ) [ "=" expr ]
type        = "maybe" type | "nix" type | Name { type-arg }
type-arg    = Name | "(" type ")"
```

Its addendum adds two productions: 2.21l adds `table` beside `fact` and
`focus-block`, and 2.21n adds `every` to `post`:

```text
table       = sentence INDENT { token { token } } DEDENT
post        = … | "every" vars "where" conj ":" post
```

In a `table`, the header `sentence` fills each slot with a distinct
variable, and each `token` is a slot token (§9.2).

An `all` head of two or more variables is a variable list, and any other
head, a lone variable included, is an expression (§4.5). The head ends at
the first `where` outside its brackets, as an aggregate's value does, since
`where` is no operator, so an `if`, `with` or lambda in a head stops there
and a query in a head is parenthesized. `all` is a keyword wherever an
expression is due, as `if` and `with` are: it always opens a query, so
`all(xs)` is an `all` without its `where`, and a function or binding named
`all` is written `` `all` `` (§9.1). So whether `all` opens a query never
depends on a later `where`, or on a Nix body's `with lib:`, which can put
`lib.all` in scope where Galileo cannot see it (G5; DECISIONS D-61
addendum 2).

Its third addendum makes an aggregate an expression, so the binding of
one is the binding literal. 2.21q moves `aggregate` into `expr` with its
`count` and `sum` forms and drops the literal's `"?" var "=" aggregate`,
and 2.21r adds the `min` and `max` forms and the argmin:

```text
literal     = sentence | "not" sentence | expr cmp expr | expr
            | "?" var "=" expr | "?" var "in" expr
expr        = … | query | aggregate
```

Where an expression is due, `count` opens an aggregate only when `where`
follows it, and `sum`, `min` and `max` only when the next token begins an
expression and is not `(`. Otherwise each is a name: `count(xs)` and
`min(a, b)` are calls, and `count - 1` reads a parameter named `count`. So
whether the word opens an aggregate depends on the next token alone, and
never on a later `where` (G5). An aggregate's value ends at its first
`where`, so it is never parenthesized: `max (?a.x) where …` is the call
`max(?a.x)` and a `where` that ends nothing, an error offering `max ?a.x
where …`. An aggregate in an `all` head, a sort key or another aggregate's
value is parenthesized, as a query there is, since that head or value
ends at its first `where`. In a Nix body an aggregate is computed by
Galileo with its condition, as a query is, so a Nix-stage operand in its
value is a stage error (DECISIONS D-61 addendum 3).

Its fourth addendum writes each name once. 2.21s replaces `for-block`,
`rfield` above writes a record field once, and 2.21u lets a focus block
hold `is a T`, field facts and tables (§4.2):

```text
for-block   = "for" "?" var "in" expr INDENT { entry } DEDENT
            | "for" vars "where" conj [ "," "sorted by" expr ] INDENT { entry } DEDENT
```

`for ?name, ?url where C` iterates the rows `all ?name, ?url where C`
returns, in its order, and binds each listed variable directly, so a
`for` header becomes a `galileo query` question by putting `all` where
`for` stood; one variable is the same form. `for ?x in e` iterates a list
value, a query with an expression head included. A `for` over an `all`
whose head is a variable or a variable list is an error offering the
`where` form, and `for ?a, ?b in e` over anything else an error offering
one variable. The token after the variable list, `in` or `where`, decides
the form. In a record, `?x` is the field `x: ?x` and `?x.f` the field `f:
?x.f`, as SQL names a selected column `r.key` `key` and Nix writes
`inherit (row) key;`; written out, either is an error offering the short
element. Only `?` variables take it, since bare names in braces before `=>`
are a lambda's formals, and entries are not records (DECISIONS D-61
addendum 4).

D-65 names Nix binders and references by a known `Text`. 2.22a moves
`for-head` into `for-block` and adds `for` elements, and 2.22b adds
`computed`:

```text
for-head    = "for" "?" var "in" expr
            | "for" vars "where" conj [ "," "sorted by" expr ]
for-block   = for-head INDENT { entry } DEDENT
rfield      = attrpath ":" expr | "?" var [ "." field ] | for-head ":" rfield
param       = name [ ":" type ]
            | "{" formal { "," formal } [ "," "..." ] "}"
            | "{" "..." "}"
formal      = ( name | computed ) [ "=" expr ] | for-head ":" formal
expr        = … | computed
computed    = "`" "{" expr "}" "`"
```

A `for` element's header ends at the first `:` outside its brackets, as
the condition of an `every` post does. Inside braces, `for` opens one
only before `?`. A record key `for` already takes backticks, and
`{for, ...}` keeps a formal named `for`. A computed name stands only as
a brace formal or as a reference in a Nix body. As an attrpath segment
or after `.`, `` `{e}` `` is an error offering `{e}`, which is already
a key's spelling (§3.1). §10.3 gives their meaning (DECISIONS D-65).

D-70 writes the patch and its contract. 6.5a adds `marked` to a claim's
literals, and 6.5b and 6.5d add the contract's sections:

```text
patch       = "patch" name INDENT { section } DEDENT
section     = edit | "says" text-literal | "intends" INDENT { intent } DEDENT
intent      = claim | "only" sentence "changes" | "requirements unchanged"
marked      = "before" ( sentence | "not" sentence )
            | sentence ( "becomes" ( "true" | "false" ) | "stays" )
```

`edit` is one of §8.2's sections. In `intends` a claim's `literal` may be
`marked`. In `ensures` it may be marked `before`, on every sentence of
the claim or on none (§5.2), and elsewhere a mark is an error (§6.2,
DECISIONS D-70).

A `define` without a body is a signature, and only `firn/nix-lib` declares
one (§3.2).

**Cases.** A value chosen among several cases is an `if` chain, and `else
if` needs no parentheses, since `if` reaches as far right as it can. A
`define` body is one expression over all its indented lines, so a chain
reads one case per line, as a piecewise definition does in mathematics:

```text
define menu-help(kind: Text): Text
  if kind == "document" then "↑/↓ scroll · Esc close"
  else if kind == "work" then "Type to filter · ↑/↓ choose"
  else "Type to search · ↑/↓ choose"
```

A mapping that holds only for some keys, where no case means no step, is a
relation of facts instead, such as `accepting "models" menu requests
"select-model"` joined under `when` and stated as a fact table (§4.2,
§9.7). A template's words are its author's, and the language neither drops
nor ignores one; a word that carries no meaning, such as an article no
reading needs, is left out (G10). There is no `match` until sum
types give its arms fields to bind (DECISIONS T4, D-61): over `Text`,
`Int` and `Bool` it would be a second spelling of the chain.

A question is not a declaration. `galileo why`, `why not` and `trace` take a
ground sentence, `prevent` and `achieve` a claim, and `query` a query, each
read as a line of a module that uses every module of the program, with no
private declaration in view. Their options are flags: `--via` and
`--using`, and a run's `--events`, `--as-of` and `--since` (§10.4). The
query language has no word for time: a question's revision is its
command's flag, and a program reads one state (§5.7).

A `nix module` is a `nix file` whose value is a NixOS module (§10.2). A
lambda with one brace parameter emits a Nix lambda with formals, such as `{
self, nixpkgs, ... }: …`, and a formal written `name = expr` is one with a
default: `{hostname, system = "x86_64-linux", extraModules = [], ...}`
emits `{ hostname, system ? "x86_64-linux", extraModules ? [ ], ... }`
(`flake.nix:56`, `:228`). A default reads only names outside the lambda,
and may not mention a name spelled like any formal of its lambda, even one
an outer binding also has, because Nix reads the formal there: `let x =
"outer"; ({x, y = x}) => y` is an error naming `x`. DECISIONS D-35 records
the rulings below, and D-42 the T5 audit's cuts. A brace parameter with no
formals is `{...}`. It emits `{ ... }:`, the module lambda that binds
nothing, which firn's home-manager modules write as `({...}) =>
{home.packages: …}`. `{}` is an error, since no program needs a lambda that
takes only the empty record (DECISIONS D-66).

**Keywords never shadow templates.** A line is read every way the grammar's
keywords and the templates in scope can split it, and exactly one complete
typed reading must remain (G5, §9.2). The literal kinds are readings like
any other: `?x = …`, `?x in …`, `not …` and a boolean expression are tried
beside every template, and no word's presence in some template decides a
line's kind. When no reading remains, a line with a bare word that is not a
keyword gets §9.2's closest shapes. A `when`, `then` or `and` line of a
rule or clause holds one literal or post: a further depth-0 `and` that no
`where` precedes and no template reading uses would separate two, which is
an error asking for one per line, or for `?x = (?a and ?b)` when the
boolean is meant. The old layout, `when` or `then` alone on its line with
the literals indented under it, is an error that shows the new lines. So a template may use any word,
`and`, `requires` and `only when` included: `backup requires capability gpu`
in a `requirements` block claims a sentence of firn's
`{responsibility} requires capability {capability}`, because no split at
`requires` leaves a sentence on each side, and a line that two readings
type-check is an error that shows both (notebook K10, O1). Nothing in the
grammar keeps a keyword reading from a template reading: a one-word
template is a sentence, and its word may name an entity in a slot (§9.1).
With `given build`, `given test` and `given ?a: Task requires ?b: Task`
in scope, and `build` and `test` declared as `Task`s, `build requires
test` in a `requirements` block is both a claim and that template's
sentence, so it is an error that shows both (DECISIONS D-56).

**Operators**, from loosest to tightest: `or`; `and`; prefix `not`; the
comparisons; `//`; `+ -`; `* /`; `++`; prefix `-`; then field access and
calls. `and`, `or`, `+`, `-`, `*` and `/` group to the left, and `//` and
`++` to the right. `a / b` divides two `Int`s and rounds toward zero, as
Rust's `/` and Nix's `builtins.div` do: `7 / 2` is 3, `-7 / 2` is -3, and
`?used * 100 / ?window` is `(?used * 100) / ?window`. It is `null` on a
zero divisor unless its divisor cannot be 0 (§3.3, §3.5). Like `in`, it is
computed by Galileo, so in a Nix body an operand of the Nix stage is a
stage error that names `builtins.div` (DECISIONS D-83). Comparisons don't
chain, so `a < b < c` is an error. `< <= > >=` compare two `Int`s or two
`Text`s, `Text` by code point (§4.3, §4.5);
in a Nix body they lower to Nix's, whose byte order on UTF-8 is the same.
`not a == b` is `not (a == b)`, and `not` is never the bare operand of a
tighter operator: `a == not b` is an error that asks for `a == (not b)`. `e
in list` is a comparison, as in Python (A-Python-in): `true` when the `List
T` holds a value equal to `e`, a `T`, so `not ?k in ["a", "b"]` is `not (?k
in ["a", "b"])` and `if ?k in ["a", "b"] then …` needs no parentheses. It
is computed by Galileo, like a prelude function, so in a Nix body an
operand of the Nix stage is a stage error that names `builtins.elem`
(§3.2). A line whose literal is `?x in e` is membership (§4.3), which means
the comparison when `?x` is grounded (DECISIONS D-61). `if`, `with`,
lambdas and queries reach as far right as they can, so `a + if c then 1
else 2 + 3` adds `a` to the whole `if`. `++` joins both lists and text; the
emitter picks Nix `++` or `+` by type, and when neither operand's type is
known it is an error asking for a typed operand, since either Nix operator
would be a guess. A `++` with a `Text` operand is `Text`. A Galileo name
that is a Nix keyword (`if`, `then`, `else`, `let`, `in`, `with`, `rec`,
`inherit`, `assert`, `or`) cannot name a Nix binding or reference: it is an
error at its line, never renamed. Prefix `-` negates an `Int`, known or
Nix, so `-f(x)` is `-(f(x))` and `a * -b` is `a * (-b)`. Before an integer
literal it is part of the literal, so the smallest `Int`,
`-9223372036854775808`, can be written. Outside a Nix body `-e` is `0 - e`,
overflow included. In a Nix body it emits Nix's prefix `-`, which Nix reads
as `0 - e`. A known negative `Int` emits as `-` and its magnitude, in
parentheses wherever Nix would otherwise read a binary `-` (`f (-1)`, `[
(-1) ]`).

**Reserved characters.** A `-` inside a word belongs to the name
(`kanata-main`), so subtraction is written `count - 1`. A `-` that begins a
token is always the operator. After an operand it subtracts, so `a -1` is
`a - 1`. Where an operand is due it negates, so `OOMScoreAdjust: -500`
reads as written (DECISIONS D-35, D-66). Outside text, paths and comments,
`! ? % & ^ ~ @ $ ; \` are errors, except in `!=` and as a variable's
leading `?`. They are kept for later operators.

**Inline records** are written `{a: 1, b.c: "x", {?k}: v, ?row.key}`. Their
keys are attrpaths, which nest and merge as entry keys do, and an element
that is a variable or its field read names its field by its last name
(above). Like a list or a call, an
inline record stays on one line and takes no `when`; a conditional field is
written as an indented entry. A text block inside one is still one line: the
record goes on after the block's closer (§9.1).

**Field access** takes a name, bare or in backticks: `cfg.port`,
`` x.`when` ``. A selection from a Nix value also takes an attrpath's
other two segments, read as §3.1 reads them: a quoted segment, a text
literal emitted as a quoted Nix key, and `{e}`, emitted as `${e}`, where
`e` is a `Text`, known or Nix. So
`(inputs.zen-browser.packages."{pkgs.stdenv.hostPlatform.system}").default`
emits `(inputs.zen-browser.packages."${pkgs.stdenv.hostPlatform.system}").default`.
Such a selection is `Nix Any`, and its key ends an option-tree walk
(§3.2). On a known value it is an error that offers `lookup`, which
returns `maybe` (§3.5). Under `lib` or `builtins`, whose members are
found by name (§3.2), it is an error naming the parent.

**Where a condition ends.** A `where` reaches as far right as it can: to the
end of the line, to the first `,` outside the brackets it opened, or to a
closing bracket it did not open. That holds for queries, aggregates and `min
… by` alike, and an entry's `when` suffix is split off before its value is
read. What precedes a `where`, an `all` head or an aggregate's value, ends
at the first `where` outside its brackets. So `count(all ?m where ?m is on,
sorted by ?m)` needs no parentheses, and a query inside a larger expression
does: `(all ?x where …) ++ ys`. Inside a condition, `and` separates
literals, and a nested `where` takes the rest of its condition. An `or`
beside a separating `and` must be parenthesized, as `(a or b) and c` or `a
or (b and c)`, because in an expression `and` binds tighter than `or`.

**Types.** `type` is written in slots, entity fields, `define` and lambda
parameters, and after `option`, where §10.2's table says what each emits;
there is one type grammar and one spelling of each type (`Text`, never
`text`). `option P: enable e` takes any `Text` expression, a literal or a
`let` name. A written type outside this grammar is a parse error, and a
name that is not a type is a checker error; neither becomes an unchecked
type. Function types and record types are not in the MVP, and the grammar
leaves `(…) => T` and `{…}` free for them.

### 9.5 First win: `modules/kanata` in Galileo

`modules/kanata/default.gal` emits today's `modules/kanata/default.nix`
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

`modules/kanata/kanata.gal` emits today's `modules/kanata/kanata.nix`,
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

What Galileo checks that Beagle did not:

- **Field names.** `cfg` has type `config.myConfig.modules.kanata`, so
  `cfg.devics` is an error: "`devics` isn't an option of
  `myConfig.modules.kanata`. Did you mean `devices`? (declared at
  default.gal:4)".
- **Types.** `cfg.port != null` type-checks because `port` is `maybe Port`.
- **Staging.** `builtins.readFile(cfg.configFile)` is `Nix Text`, so it can be
  emitted but never turned into a fact.

### 9.6 Module metadata and host tags as facts

Metadata sits in the same file as the module as plain facts, so it never
reaches Nix. Today it is removed by text surgery (F5).

```text
# modules/firefox/default.gal
use firn/tags

firefox
  is a Module
  is tagged browsers
  needs flake input "nur" from "github:nix-community/NUR"
browsers sets myConfig.modules.firefox.default to true for firefox

nix module
  arguments: config, lib, pkgs, ...
  option myConfig.modules.firefox.enable: enable "Enable Firefox browser"
  option myConfig.modules.firefox.default: Bool
    default: true
    description: "Set Firefox as the default browser via MIME types"
  imports: [./firefox.nix]
```

```text
# hosts/whiterabbit/tags.gal
use firn/tags

whiterabbit
  is a Host
  runs linux
  enables [terminal, cli-tools, theming, auth, javascript, python, rust]
  enables browsers
  adds [chrome, qutebrowser, zen-browser] to browsers
  enables desktop
  drops mako from desktop
  adds river to desktop
  disables [anytype, bun, cargo, clang]
```

Each module's file declares its module, and firn's tags and platforms are
declared once each, so a misspelled module, tag or platform anywhere is an
error naming the nearest one (§4.2). Today firn checks only that a host's
edits and `:disabled` name real modules, and its tag universe is whatever
the modules' `:tags` happen to spell. `config/tags.gal` declares firn's
25 tags in one line, written once each (§4.2); its first three:

```text
# config/tags.gal
use firn/tags

[auth, browsers, cli-tools, …] is a Tag
```

```text
# config/platforms.gal
use firn/tags

[linux, darwin] is a Platform
linux allows every module
```

The vocabulary and the resolution algorithm of `docs/tags-composition.md`,
as rules (F6):

```text
# firn/tags.gal
type Module, Tag, Host, Platform

given Module is tagged Tag
given Module can opt into Tag
given Host enables Tag
given Host drops Module from Tag
given Host adds Module to Tag
given Host disables Module
given Host turns on Module
given Host turns off Module
given Host runs Platform
  exactly one platform per host
given Module supports Platform
given Platform allows every module
given Tag sets ?path: OptionPath to ?value: nix Any for Module
given Module needs flake input ?name: Text from ?url: Text
derived Module is supported on Host
derived Module is enabled on Host by Tag
derived Module is enabled on Host by default through Tag
derived Module is tag-enabled on Host
derived milestone Module is enabled on Host
derived Host gets ?path: OptionPath set to ?value: nix Any by Module
  at most one value per host, path
derived ?name: Text flake input url ?url: Text
  at most one url per name

rule enabled-by-default: ?module is enabled on ?host by default through ?tag
  when ?host enables ?tag
  and ?module is tagged ?tag
  and not ?host drops ?module from ?tag

rule default-membership-enables: ?module is enabled on ?host by ?tag
  when ?module is enabled on ?host by default through ?tag

rule enabled-by-opt-in: ?module is enabled on ?host by ?tag
  when ?host enables ?tag
  and ?host adds ?module to ?tag
  and ?module can opt into ?tag

rule supported-everywhere: ?module is supported on ?host
  when ?module is a Module
  and ?host runs ?platform
  and ?platform allows every module

rule supported-by-platform: ?module is supported on ?host
  when ?host runs ?platform
  and ?module supports ?platform

rule tag-enabled: ?module is tag-enabled on ?host
  when ?module is enabled on ?host by ?tag
  and ?module is supported on ?host
  and not ?host disables ?module
  and not ?host turns off ?module

rule enabled-by-tags: ?module is enabled on ?host
  when ?module is tag-enabled on ?host

rule enabled-directly: ?module is enabled on ?host
  when ?host turns on ?module

rule tag-overrides: ?host gets ?path set to ?value by ?module
  when ?module is tag-enabled on ?host
  and ?module is enabled on ?host by default through ?tag
  and ?tag sets ?path to ?value for ?module

rule flake-input-url: ?name flake input url ?url
  when ?module needs flake input ?name from ?url

requirements for tags
  every ?host, ?module, ?tag where ?host adds ?module to ?tag: ?module can opt into ?tag
  every ?host, ?module, ?tag where ?host adds ?module to ?tag: ?module is supported on ?host
  every ?host, ?module where ?host turns on ?module: not ?host turns off ?module
```

A host's literal enables are facts in its own `.gal`: `whiterabbit turns on
kanata` and `whiterabbit turns off bluetooth`, which its `nix file` emits
from one `for` each (DECISIONS D-69). `is enabled on` is what Nix enables:
a module that is tag-enabled or that the host turns on. A host's entry has
normal priority and the generated enables use `lib.mkDefault`, so a
`turns off` also blocks the tag level, and a direct enable takes no
platform gate, since Nix applies it anyway. The last requirement fails a
host that turns one module both on and off, which Nix rejects as a
conflict.

firn's platform policy is facts too: `linux allows every module`, and
`X supports darwin` for each of the 34 modules in today's
`config/darwin-modules.json`. So on ashashi, which runs darwin, a tag
enables only the modules that support darwin, as `native/tag_resolve.bjs`
filters today, and `why not` names the missing support where firn drops
the module without a word (G8). The second requirement catches the one
case where that drop hides a mistake: a host that adds a module its
platform does not support; it is the platform-support requirement box 5.5
names. `firn platform list`, `show` and `safelist` read the same facts
through `galileo query --json`: a module runs on each platform that allows
every module or that it supports, as the two rules say, and `safelist`
prints the `supports` facts as Galileo lines. They compare no option
paths with any schema, since no darwin host remains to extract one from
(DECISIONS D-75). A flake input's name is quoted `Text`, not an entity:
only the module that needs an input names it, so a declaration would
repeat that line. Its URL is a fact, and the input section's other
attributes become facts when 5.6 emits them. A second URL for one name
breaks `at most one url per name`, so `galileo build` refuses to emit and
shows both modules' lines (§4.4, DECISIONS D-51).

The generated enables module comes straight from the rules. Today there is
an intermediate generated source and a second compile (F6).

```text
# hosts/whiterabbit/_generated-enables.gal
use firn/tags

nix module
  arguments: config, lib, pkgs, ...
  for ?module where ?module is tag-enabled on whiterabbit
    myConfig.modules.{?module}.enable: lib.mkDefault(true)
    for ?path, ?value where whiterabbit gets ?path set to ?value by ?module
      {?path}: lib.mkDefault(?value)
```

It iterates `is tag-enabled on`, not the milestone: the host's own file
emits its direct enables. The inner loop emits tag overrides, such as `firefox.default`, right after
their module's `enable`, in the order today's file uses. `{?path}` splices
several attribute names because `?path` is an `OptionPath`, and
`{?module}` is one name because `?module` is an entity (§3.1). A `for`
names its variables once and iterates its condition's rows in the
canonical order of those variables, as `all ?path, ?value where …` would
list them, and `for ?x in e` iterates any other list (§9.4; DECISIONS D-61
addendum 4).

Overrides apply only through default memberships, as `docs/tags-composition.md`
specifies. Two overrides of one path on one host break the `at most one`
mode, so `galileo build` refuses to emit and shows both proofs. Today the
first override silently wins (G8).

`galileo why firefox is enabled on whiterabbit` prints:

```text
firefox is enabled on whiterabbit                                       ← enabled-by-tags
└─ firefox is tag-enabled on whiterabbit                                ← tag-enabled
   ├─ firefox is enabled on whiterabbit by browsers                     ← default-membership-enables
   │  └─ firefox is enabled on whiterabbit by default through browsers  ← enabled-by-default
   │     ├─ whiterabbit enables browsers        hosts/whiterabbit/tags.gal:8
   │     ├─ firefox is tagged browsers          modules/firefox/default.gal:6
   │     └─ nothing states: whiterabbit drops firefox from browsers   (checked: hosts/whiterabbit/tags.gal)
   ├─ firefox is supported on whiterabbit                               ← supported-everywhere
   │  ├─ firefox is a Module                    modules/firefox/default.gal:5
   │  ├─ whiterabbit runs linux                 hosts/whiterabbit/tags.gal:6
   │  └─ linux allows every module              config/platforms.gal:5
   ├─ nothing states: whiterabbit disables firefox                      (checked: hosts/whiterabbit/tags.gal)
   └─ nothing states: whiterabbit turns off firefox                     (checked: hosts/whiterabbit/tags.gal)
```

kanata, by contrast, is not tag-enabled. whiterabbit turns it on in its
host file, so `galileo why kanata is enabled on whiterabbit` answers from
rule `enabled-directly` and names the fact `whiterabbit turns on kanata` at
its line in `hosts/whiterabbit/configuration.gal`. An entry the importer
leaves as Nix, such as `lib.mkForce true` or one under a `when`, is
Nix-stage, so it is shown as emitted Nix and never decided.

### 9.7 North: the menu slice in Galileo

North's menu is `north.clause` L447–827: 13 handlers, 8 north-scoped
relations and 8 MenuRow relations (N10). In Galileo, the north-scoped values
are fields of North's state record, each row's values are fields of the
row, and the menu count, row visibility and row position become derived
fields. That removes the `count-menu` and `filter-menu` maintenance handlers
that today run after every transition (N3). The row teardown becomes `not
?row is a MenuRow`, and the `F64` arithmetic becomes `Int`:

```text
use north/core

state entity MenuRow
  key: Text
  label: Text
  description: Text
  annotation: Text
  order: Int

state for North
  menu-kind: Text
  menu-query: Text
  menu-selection: Int

derived for North
  menu-count: Int

state North lists MenuRow
derived MenuRow matches the menu query of North

rule matching-rows: ?row matches the menu query of ?north
  when ?north lists ?row
  and contains(lowercase("{?row.label} {?row.description} {?row.key}"), lowercase(?north.menu-query))

rule menu-count: ?north.menu-count = count where ?row matches the menu query of ?north
  when ?north is a North

define wrap(index: Int, count: Int): Int
  if index < 0 then count - 1 else if index >= count then 0 else index

event move-menu: North moves menu selection by ?delta: Int
event clear-menu-rows: North clears menu rows

on ?north moves menu selection by ?delta
  when ?north.menu-count > 0
  then ?north.menu-selection = wrap(?north.menu-selection + ?delta, ?north.menu-count)
  may skip

on ?north clears menu rows
  then every ?row where ?north lists ?row: not ?row is a MenuRow

view Menu for North
  kind: ?north.menu-kind
  query: ?north.menu-query
  selection: ?north.menu-selection
  rows: all ?row where ?row matches the menu query of ?north, sorted by ?row.order
```

`may skip` keeps Clause's behaviour on an empty menu, where moving the
selection is a no-op rather than a rejection (§5.3). Clearing needs none:
its `every` post matches with no rows (§5.2).

This is the end state, not the first import. `galileo import clause` first
imports the slice mechanically: every handler is marked `may skip`, and the
`count-menu` and `filter-menu` maintenance handlers stay handlers that run
in Clause's order inside the same atomic sequence. Replacing each one with a
derived relation, and removing each `may skip`, is then a separate change
that must pass the replay check of §10.4. Clause and Galileo can disagree in
between: a handler that ran before maintenance read a stale count in Clause
but reads the current one in Galileo (N2, N3).

From this file, `galileo bindgen rust` generates the following. They replace
the hand-written `MenuState` mirror, its decoding in `decode_projection` and
the per-event wrappers in `clause_state.rs` (N4):

- `struct Menu`;
- `enum Event { MoveMenu { north, delta }, ClearMenuRows { north }, … }`;
- typed `Session::run` and `Session::view::<Menu>(north)`.

North's screen also reads the menu's title, help and empty texts and each
row's position, so north's `Menu` view carries them, each row as a record
of its fields and its derived position (`examples/north-menu.gal`). North
offers each row with one event, `OfferRow { north, key, label, description,
annotation }`, and composes a work item's texts itself; the program marks
the active conversation's row `current`, since that fact is the
program's (§5.2, DECISIONS D-61 addendum 5). The
replacement happens once north runs on `galileo-runtime` (PLAN 8.14):
handlers outside the slice write the menu's kind, title, count and
selection, its mailbox and the active conversation, so the slice never
owns them alone, and no bridge shares them between Clause and Galileo
(DECISIONS D-57).

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
  provenance and state search. A session's steps run on it: the session
  keeps the model of its current state and evaluates a step's post-state
  only when the step changed a relation that a rule reads (DECISIONS D-58).

Both call one expression evaluator and charge one work meter (§6.1), so
they differ only in how they find bindings: the reference by a naive fixed
point over full scans, the engine by join plans, indexes, semi-naive deltas,
sorted groups for a ranked `count` or `sum`, the kept model and provenance.
Steps are compared the same way: a session's steps on the engine against
the reference's steps on the recorded sequences. Expressions are checked
against named references instead: this specification's cases and Clause's
recorded text functions (§3.5). A
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

**`galileo build [paths]`:**

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
  once WP4 has proven identity (DECISIONS T2). Galileo's `let` is sequential
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
  `{ inputs = …; outputs = …; }` and its overlays
  (F4). `nix module` is the same form, specialized to a module. A
  `nix file` that emits `flake.nix` has no root `let`: Nix reads a flake's
  top level without evaluating it, so it must be an attribute set, and the
  bindings go inside `outputs` (DECISIONS D-39).
- `with E: body` lowers to Nix `with`. Inside it, a name that is not a Galileo
  binding resolves to `E.<name>` at the Nix stage, and Galileo bindings always
  win, as Nix's do. When `E` is `pkgs`, such names are checked against the
  schema's package index (F4: `with pkgs;`).
- A parenthesized selection is a Nix selection of its own: `(a.b).c`
  emits `(a.b).c`, and `a.b.c` one attribute path, because Nix's parser
  keeps them apart and parse identity compares its trees (DECISIONS
  D-39). `galileo fmt` keeps the parentheses.
- `a // b` is a right-biased merge. On known records and attribute sets
  Galileo computes it; otherwise it lowers to Nix `//` (F3).
- A nested `nix module (args)` value, such as a home-manager user module,
  has its own argument scope and its own module system: Galileo's option
  declarations do not apply to its `config` (§3.2). Its entries and its
  `config` reads are checked against the foreign schema of that module
  system when one is declared, and otherwise stay unchecked `Nix` with one
  warning per module (F3: 81 `nix/module` forms, nested ones included).

**Option types:**

| Galileo | NixOS |
| --- | --- |
| `Bool` | `lib.types.bool` |
| `Int` | `lib.types.int` |
| `Text` | `lib.types.str` |
| `Path` | `lib.types.path` |
| `Port` | `lib.types.port` |
| `List T` | `lib.types.listOf T` |
| `maybe T` | `lib.types.nullOr T` |

These are every option type firn's 28 `lib.mkOption` calls use. Others,
such as `package`, `attrsOf` and `enum`, come with the first program that
declares one (G1), each as a type that §3.3's grammar already spells.
`lib.types.lines` is not one of them: it holds exactly the values
`lib.types.str` holds and differs only in how NixOS merges two
definitions, so it states no contract of its own (§3.3), and no `option`
type emits it. A module that declares a `lines` option is a `nix file`
whose value calls `lib.mkOption`, as firn's submodule scaffold does
(DECISIONS D-59).

`option P: enable e` emits `lib.mkEnableOption e`, where `e` is any `Text`
expression: a literal, or a `let` name, which emits as that name, as five
firn modules write `lib.mkEnableOption description` (notebook K7).
`option P: T` with fields emits `lib.mkOption { type …; default …;
description …; }`, with the fields in that order, which every firn
`lib.mkOption` call already uses (DECISIONS D-39).

Options that share a node may be declared as a group: `option P` with
no `:`, and under it one or more lines `Q: T` or `Q: enable e`, each
with its fields indented under it:

```text
option myConfig.modules.socrates-chat
  enable: enable description
  homeserver: Text
    description: "Matrix homeserver URL"
  proxyPane: maybe Text
    default: null
    description: "Optional proxy tmux socket and pane, in socket:pane form"
```

A line `Q: …` in the group declares exactly what `option P.Q: …`
declares, at its own line. The option tree, `galileo refs`, every
did-you-mean and every other reader see `P.Q`, never whether a group
declared it. A group emits one binding, `options.P = { Q = …; … };`,
with its declarations in source order and each value what the flat line
emits. Flat lines emit one binding `options.P.Q = …;` each. This is the
record register's nesting rule, under which an entry's path with entries
indented under it is one nested attribute set. So each Nix layout has
one spelling, the emitted Nix stays the image of its source (D-39), and
both spellings round-trip through `galileo import nix` and
`galileo fmt`. firn's socrates-chat writes the group its Nix holds, and
the 50 other Beagle outputs that declare options keep their flat lines.
A group holds declarations only. `option P` with nothing under it, and
a line under a group that has no `:` (a group inside a group), are each
an error at their line (DECISIONS D-66 addendum).

**Migration proof.** Compare `nix eval --raw
"path:$D#nixosConfigurations.whiterabbit.config.system.build.toplevel.drvPath"`
over `git archive` copies of the merge base and the migrated lane. The
copies are needed because a git flake's `self.rev` reaches
`system.configurationRevision`. If the two differ, `nix-diff` on the two
`.drv` files localizes the cause (F15). nixos-config has had no darwin host
since `7d7a8c9d` (DECISIONS D-67). Under the `beagle` layout every
generated file must also be identical: byte-identical for Beagle sources
(`flake.nix` as box 4.5 says), parse-identical for Clause sources.

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

Known-stage queries (§4.5) inside `nix module` comprehensions emit
generated modules directly. The generated enables, the flake's input
section, and its `outputs` formals and four `inputs` records are
examples. This removes the intermediate generated `.bnix` and the marker
splicing in `flake.bnix` (F6, F8).

A `for` block unrolls when Galileo builds, so its source must be known:
a source that depends on Nix evaluation is a stage error, and Nix-side
iteration is a call such as `builtins.map(…)`. A `for` element unrolls
the same way inside braces. `for H: f` in an inline record is the field
`f` once per row of `H`, and `for H: x` in a lambda's brace formals is
the formal `x` once per row. Each lands at the element's place, in
`H`'s row order. `H` is a `for` block's header, and its variables are
bound in its element alone.

A computed name, `` `{e}` ``, is the name that the known `Text` `e`
spells, used as a Nix binder or reference. Written as a brace formal it
binds that name, and written where a Nix body expects an expression it
reads that name. Galileo decides it when its `for` unrolls and emits a
plain identifier. `flake.gal` writes

```text
outputs: ({self, nixpkgs, …, glide, for ?n where ?n flake input url ?_: `{?n}`, ...}) => …
```

and, in each of the four places that pass inputs to modules,

```text
{inputs: {for ?n where ?n flake input url ?_: {?n}: `{?n}`}, flakeRoot: self} // extraSpecialArgs
```

so a module's new flake input reaches all six places from its one fact.
Because the name comes from data, it may not change what any other name
means. Each of the following is an error at the computed name's line,
and the message names its text:

- a text that is not a Nix identifier (`[A-Za-z_][A-Za-z0-9_'-]*`) or
  is a Nix keyword (§9.4);
- a computed formal whose name another formal of its lambda already has,
  or that would shadow a name in scope there: an enclosing binding, a
  prelude function, a name the Nix target binds (§2), or `true`,
  `false` or `null`, which Galileo's literals emit;
- a computed reference that no Nix binding has, which offers the
  nearest one; a Nix binding is an argument, a `let`, a parameter, a
  formal or a name the Nix target binds, never a prelude function
  (§3.2);
- a computed reference whose name two enclosing bindings have, because
  the nearer one would capture it; the message names that binding's
  line. A flake input named `pkgs` is an error at both home-manager
  records, whose module lambdas bind `pkgs`;
- an `e` that is not a known `Text`, since Nix has no computed
  variables.

A bare name never reads a computed formal, which exists only once its
`for` unrolls. A computed name anywhere else is an error, including a
plain parameter, a `let` or `arguments:` name, and anything outside a
Nix body (DECISIONS D-65).

### 10.4 Runtime target (north)

`galileo compile <module> -o <image>` produces a program image: the linked
modules' source and the program's identity (§8.1); `Program::load` re-lowers
it and checks the identity.

The Rust crate `galileo-runtime` provides:

- `Program::load(bytes)`;
- `Session::new(&program)`, `Session::restore(&program, checkpoint)` and
  `Session::converted(&program, state)`, which starts at revision 1 from a
  converted Clause state;
- `session.run(event) -> Result<Step { revision, effects }, Rejection>`;
- `session.run_all(&[event])`, which is atomic;
- `session.view::<V>(arguments)`, `session.checkpoint()` and
  `session.why(sentence)`;
- `session.as_of(revision)`, a read-only view of the state at that
  revision, which `view` and `why` read (§5.7).

`galileo bindgen rust` generates typed Rust for entities, text unions, events,
effects and views. North writes about 1,100 lines of mirror types and
marshaling by hand today (N4). `run` and `run_all` take any type that
implements the runtime's `Event` trait, and `view` any that implements
`Projection`. The generated types implement them, an untyped tuple is an
`Event` too, and `session.project(view, arguments)` reads a view untyped
(DECISIONS D-55).

**Checkpoints** hold the program identity (§8.1), the revision, the state
facts, the live entities, the allocator, the pending effects (§5.4) and the
history (§5.7), so a restored session can answer as of any earlier
revision back to its first. Restoring under a different program identity is
an error.

**Other hosts.** `galileo run` and `galileo query … --json` expose the same
runtime as JSON lines. firn's TypeScript host (DECISIONS T1) calls them for
its command plans, so the MVP needs no JavaScript target (§12).

**Stepping from the command line** (DECISIONS D-51).
`galileo run [PATH…] --events FILE` reads the program from its source paths,
as every other command does, starts from `initially` at revision 0, and
steps each event of FILE in order, each one occurrence (§5.3) and never one
atomic sequence. FILE holds one ground sentence of an `event` shape per
line, such as `Ash moves through Front-Door`; blank lines and comments are
skipped. Each line is read as a question is (§9.4): against the program's
vocabulary, with exactly one typed reading, every entity it names declared
(§4.2) and every closed slot's value in its domain (§5.5). So a misspelled
event or entity is an error at FILE:line, and nothing runs. A created
entity has no name to write in FILE yet: a program keys what its events
create by a field value or a declared entity, a `state entity`'s
included (§4.1). FILE is not a module, so it
is named `*.events`, never `*.gal`, which the project's other commands
would read.

It prints one JSON object per line: first `{"revision": 0, "hash": H}`,
then one per event line, in FILE's order.

- Admitted: `{"line": N, "event": S, "admitted": true, "revision": R,
  "hash": H, "effects": [S, …]}`, with the effect sentences in canonical
  value order (§5.3).
- Rejected: `{"line": N, "event": S, "admitted": false, "rejection": K,
  "explanation": [S, …], "revision": R, "hash": H}`, where `R` and `H` are
  the previous object's, since a rejected step leaves the state unchanged.
  The later lines still run. `K` is one of `no match`, `conflicting
  write`, `unfilled exactly one`, `derived mode`, `invariant`, `ensures`,
  `non-live include` and `competing effects` (§5.3, §5.4).

`S` is a sentence printed from its template, and `H` the state hash
`h(S)` in hex, over the state facts, the live entities, the allocator and
the pending requests (§5.1). It sums one hash per fact and per live entity,
so a step updates it from its change (DECISIONS D-58). It exits 0 when
every event is admitted, 1 when any is rejected, and 2 when nothing runs or a step exhausts `--budget
WORK`, which ends the output with `{"line": N, "unknown": "budget"}`. The same program and FILE
always print the same bytes, so a recorded output replays by running it
again.

`--events FILE` also picks the state that `galileo query [PATH…] Q --json`
and `galileo check [PATH…]` answer in: the state after FILE's admitted steps,
each rejected line skipped as above. A host or a grader reads the final
state with `galileo query`, and `galileo check` evaluates the state claims of
the requirements there, while temporal claims and, on a program with
events, dependency claims (§6.2), which are about the whole transition
system, are checked as they are without the flag. An image
argument waits for `galileo compile` (WP8), and then takes the source paths'
place.

**Earlier states of the run** (DECISIONS D-52). `--as-of R` moves that
state back to revision `R` of the same run, for `query`, `check`, `why` and
`why not`. The state comes from the history (§5.7), never from re-running
events. `R` is a revision as `galileo run` prints it, not a line of FILE,
since a rejected line makes no revision, and `--as-of 0` is the state
`initially` gives. `--as-of` without `--events`, or past the run's last
revision, exits 2 and names the last revision.

`--since R` makes `galileo query … --json` print JSON lines instead of one
value:

- first `{"revision": R, "answer": A}`, the answer at `R`;
- then one object for each later revision at which the answer changed, in
  revision order: `{"revision": r, "line": N, "event": S, "added": [rows],
  "removed": [rows]}`, where `N` and `S` are FILE's line and event that
  made revision `r`.

An answer is read as a set of rows. An `all` query gives its rows, a single
value is one row, and `null` is none, so a `the` query that moves from
`Hall` to `Library` removes `Hall` and adds `Library`. With `--as-of R2`,
the lines stop at `R2`. A `--since` past `--as-of` or past the last
revision exits 2. If only the first line prints, the answer never changed.
The query is evaluated at `R` and then only at the revisions where a
relation it depends on changed (§5.7). Events in JSON lines wait for a host that needs them: north embeds
the runtime crate, and firn's host passes plans through `galileo query`.

**Only the CLI and the LSP touch the operating system.** `galileo-cli` and
`galileo-lsp` read and write files, run processes, start threads, and read the
clock and the environment. Every other crate, the runtime, engine and core
included, takes and returns data: source text, program images and
checkpoints come in as bytes, a host's inputs come in as events, and the
event log goes out as records that the host writes. Budgets count work,
never time (§5.5). So any host can embed the runtime, a browser included
(§12). Box 8.1 checks this.

The event log is the replay corpus. It records each occurrence, its effects
and the resulting revision hash, and the history is derived from it (§5.7). North has no replay tests today (N5).

**Migrating north.** The proof is replay under an explicit observation
contract. For each recorded Clause sequence, Galileo must give the same:

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

A slice replays the recording projected onto its own events. An
occurrence of any other event is dropped, each recorded transaction is one
atomic sequence of its kept occurrences, and a transaction with none kept
makes no step. At each boundary every state relation the slice declares is
compared, so a dropped occurrence that changed the slice shows as a
mismatch, reported with its transaction and event; the slice never imports
an outside handler to make it match. The coverage report lists the kept
events and the dropped ones, each with its count and how many of its
occurrences a clause matched (DECISIONS D-55).

Two conversions run once, at cutover:

- **State.** `galileo_import::clause_checkpoint::convert(&program, term, drop)`
  converts a Clause checkpoint (N4) into a Galileo state (DECISIONS D-72).
  Its input is the canonical projection term that the restored Clause
  session gives, the bytes north's recorder writes as `projection_hex`:
  north opens its own world files with Clause, and Galileo reads no Clause
  or north storage format. Every relation table that names a state relation
  of the program gives one fact per row, a field relation `T.f` reading
  Clause's `kebab(T)-f` when the term has that table; any other table is
  dropped and reported with its row count. Clause keeps no table for a
  relation that only top-level facts state and no rule writes, such as
  north's `usage-help`: the term carries its values only as fields of the
  top-level singletons' objects, where north reads them. For a state
  relation with no table, those fields of its name are its table, one row
  per singleton that carries the field, read as a table's rows are; a field
  that no state relation reads this way is neither converted nor reported
  (DECISIONS D-84). The replay reads every relation's rows the same way. The `Created`
  referents those facts name are live, numbered from 0 in
  ascending order of type name and the referent's 32 identity bytes, and
  named `<Type>#<id>`. Every declared entity of a `state entity` is live
  too, with its membership fact, as `initially` makes it (§4.1). A
  `Declared` referent is the declared entity its singleton names, or, when
  no singleton names it, the one declared entity of its slot's type that no
  singleton names; none or several is an error naming the referent, the
  type and the candidates (DECISIONS D-79, D-81). The allocator is past
  every id in use, and the host's
  opaque-payload handles are kept as the same opaque keys (N6). A state
  relation with no table, a value of the wrong kind or a value kind the
  importer does not read is an error naming it. `drop` names entity types
  whose entities the host re-observes after cutover, each by its local name:
  a state relation with a slot of a named type is not read, its table is
  dropped and reported, and no entity of that type is live; a name that
  matches no type, or a type with declared entities, is an error naming it.
  North's cutover drops `UsageRow`, since only S3's two divisions make a
  fraction and its next usage report recreates every row (DECISIONS D-83
  addendum). The term holds no mailbox,
  so the converted state has no pending requests, and north converts once
  its mailbox is empty. `Session::converted` starts the session at revision
  1, and its history begins there, since Clause kept none (§5.7). The
  importer returns each referent's entity, so the observation contract's
  correspondence numbers the converted entities by id and those created
  after them by creation order.
- **Numbers.** An `F64` relation becomes `Int` only when every literal, and
  every value in the corpus and the checkpoints, is an integer of magnitude
  at most 2^53. Otherwise the import stops and names the value. Clause's
  `/` imports as `/` (§9.4), which drops the fraction that Clause's
  quotient keeps, so a replay compares exactly only where every recorded
  quotient is whole, as north's two are; one with a fraction stops it, as
  above, and Clause's side is never rounded (DECISIONS D-83). Clause's
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
| `galileo build [--check] [--layout beagle\|idiomatic]` | emit Nix; drift check | `firn repo build`, `firn repo diff` (F1, F10) |
| `galileo check [PATH…] [--events FILE [--as-of R]]` | all requirements of the modules under the paths, exit 0/1/2; with `--events`, state claims in the state the events reach, or at revision `R` | prototype `check`, now three-valued (R5); `beagle-validate`, `clause-workbench check-source` (DECISIONS D-33) |
| `galileo why S [--all] [--precise] [--events FILE [--as-of R]]` | proof tree; with `--events`, in a run's state, with the chain of occurrences that made each state fact true (§7.3) | `firn tag resolve` as "the debugger" (F6); R1 |
| `galileo why not S [--via RULE] [--events FILE [--as-of R]]` | near misses; with `--events`, when and by what a state fact last stopped holding (§7.3) | prototype `why not` (P6) |
| `galileo trace S` / `galileo replay T` | executable witnesses, printed as an events file | R1 |
| `galileo prevent C --using …` / `galileo achieve possible\|acceptable C --using …` | interventions | P6, R17 |
| `galileo review [--base REV] [PATCH] [PATH…]` | §8: with PATCH, the written patch against the program at REV, `HEAD` by default; without it, the derived patch from REV to the working tree. Exit 0 approves, 1 rejects, 2 is unknown or a static error (DECISIONS D-73) | R3–R11 |
| `galileo fmt [--check]` | canonical layout: refuses what the lexer rejects (exit 2, file unchanged), keeps every token and whether tokens touch, and normalises spaces, comments and blank lines, one file at a time | F13 (surface churn) |
| `galileo import nix FILE` | prints the Galileo for a Nix file: hand-written, or the committed output of a `.bnix` or Clause module, which is how firn migrates (DECISIONS D-39) | `beagle-import-nix` (F1, F2, F10) |
| `galileo import clause` | north's relations and handlers, mechanically (DECISIONS D-28) | N1 |
| `galileo run [PATH…] --events FILE` / `galileo query [PATH…] Q --json [--events FILE [--as-of R] [--since R]]` | steps a source program through a file of event sentences, one JSON line per revision; answers in the state those steps reach, as of revision `R`, or as its changes since `R` (§10.4, DECISIONS D-51, D-52) | firn's command plans (F12, DECISIONS T1); the medium benchmark's transition sequences |
| `galileo refs NAME` / `galileo build --stdout FILE` | callers; the Nix a file emits | the compiler queries `callers` and `expand` (F10) |
| `galileo schema extract\|explain` | foreign schema facts | `firn schema explain`, `beagle-schema` (F9) |
| `galileo rename option OLD NEW` | rename by resolved references; refuses, writing nothing, a NEW that changes a segment a use reaches through a binding or that nests with a declared option (DECISIONS D-71) | `beagle-rename` (F10) |
| `galileo compile`, `galileo bindgen rust` | runtime image and bindings | N4 |
| `galileo lsp` | language server | F10, beagle issue #13 (F13) |
| `galileo help [TOPIC]` | the commands, and the language card: one example per §9.4 production, each compiled by the tests | `clause-workbench authoring-card` (DECISIONS D-33) |

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
    ashashi (via desktop)", at line 1 of every source under
    `modules/<m>/`. A host that turns the module on reads
    "ashashi (turned on)", one with both reasons "(via desktop; turned on)",
    and a module no host enables reads "enabled on no host" (DECISIONS D-71).
- **Options are static.** Galileo declares options itself, so the LSP knows
  them without evaluating the flake. nixd needs an evaluated
  `nixosConfigurations` to know them (A-nixd).

### 11.3 Change-constraint checks in use

- **firn.** `firn repo validate` runs `galileo check` and
  `galileo build --check`. `firn rebuild` runs
  `galileo review --base <last switched revision>` before it switches, and
  prints the per-host consequences. firn runs the `galileo` that
  `firn-runtime-update` builds from the commit named in
  `nixos-config:config/galileo-revision`, and its launchers refuse a
  checkout that declares another revision (DECISIONS D-33).
- **north.** CI runs `galileo check`, the replay corpus and
  `galileo review --base origin/main` on changes to `.gal` files.
- **Turtles.** Every diagnostic carries a "because" chain, such as why
  `cfg` has type `config.myConfig.modules.kanata`. Compiler checks written as
  Galileo rules, so that `why` works on an error, come after the MVP (first
  draft, "the last turtle").

Traces to: F1, F6, F9, F10, F13, N4, N5, P6, R5, R17, A-salsa, A-Lean,
A-nixd, A-why-depends.

## 12. Not in the MVP

Galileo is meant to be general-purpose (§1.2), so almost everything the MVP
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
| **Tuple-level incremental evaluation and compiled programs.** Semi-naive evaluation per state, with a session's model kept between steps and evaluated again only when a step changes a relation a rule reads, is enough at firn and north scale (A-DD, A-DBSP), and the runtime interprets the Core IR. The salsa memo covers the LSP. | The first step that changes a relation a large derivation reads and misses its step budget, likely a real-time game loop. North's menu was the first program to miss A8's budget, and DECISIONS D-58 brings in the kept model for it. Both are fast paths, differential-tested against the reference (G7). A8's budget, one step at 1,000 rows under 1 ms, is already well inside a 60 Hz frame (16.7 ms). |
| **Embedding:** Wasm, a C ABI, a JavaScript target, a browser playground, and bindings other than `bindgen rust`. north links the Rust crate, and firn's TypeScript host runs the CLI (§10.4). | The first host that can do neither, such as a browser game. Only the CLI and the LSP touch the operating system (§10.4), so a wasm32 build of the runtime is packaging rather than a port. |
| **Package imports** pinned in `galileo.lock` (§2). firn and north are each one project. | The first code that two projects share, such as a library that firn and a game both use. |
| **Greywrought game rules.** | The game milestone and the browser host above. Greywrought runs in the browser today, on TypeScript and Three.js. |
| **Symbolic or SMT analysis, ASP planning, partial-order reduction and symmetry reduction.** Explicit-state search with stated bounds comes first (A-telingo, A-Cedar). | A requirement that matters and stays `Unknown` because explicit search runs out of budget. |
| **History compaction** (§5.7). A session's history grows with every change, and its checkpoint with it. | The first session whose history outgrows its host's memory or checkpoint budget, likely a game loop. Compaction drops the spans that closed before a horizon revision, and that revision becomes the earliest one an as-of answers, recorded in the checkpoint. Spans still open are kept, so now is unchanged (DECISIONS D-52). |
| **Liveness**: every run reaches `S`, the reading `eventually S` keeps for it (§6.2). | A program whose requirement is progress, such as a request that must be answered, with the fairness assumption it needs stated beside it (A-TLA). |
| **Contracts on derived patches and on behaviour.** A git diff carries no `intends`, and a contract reads both programs' initial states, never their runs (§8.3). | For a derived patch, the first change whose author wants its intent checked on a commit, such as a firn rebuild. For behaviour, a written patch that replaces a handler and states what its runs must do. |
| **Timed requirements**, such as "warned at least N steps before". | A program whose events carry time, such as a game's ticks or north's timeouts, with a requirement about that time. |
| **Failure groups** (`among P in groups G`) and **mutation probing of requirements** (R: weak support). | A failing example: facts that fail together, which one-fact removal misses, or a requirement that kept holding while the behaviour it guards broke. |
| **The compiler's checks as Galileo rules**, the last turtle. | After the MVP, the first diagnostic that people cannot act on without its derivation. §4.3's stratification and grounding checks are the likely first, since their errors already print cycles and schedules. |
| **Porting firn's `.bjs` mechanics.** | T1's port to TypeScript on Bun, after the MVP. Until then the Beagle compiler builds only those files, never Nix. |

### 12.2 Out by design

| Out | Why, and what meets the need |
| --- | --- |
| **I/O, clocks and randomness inside Galileo code** | They would break replay and determinism (G4, §5.3). The host performs effects, and time, outside identifiers and random seeds arrive as event arguments. |
| **Mutable variables, and functions in facts or state** | State changes only through events, so every change has a step that explains it (§5.3). State stays data, so it can be compared, hashed, checkpointed and explained (§3.1). Iteration goes through built-ins, and later through recursion (§12.1). |
| **Evaluating Nix** | Galileo never evaluates `config`, `pkgs` or `lib`, so no verdict depends on Nix semantics (§3.2, DECISIONS D-7). Evaluated Nix data enters as foreign facts with provenance, or as a receipt when a host runs Nix. |
| **English authoring** | No free phrasing, plurals or paraphrase variants (§9.3, A-Inform7). Declared templates give the readability, with one reading per line. |
| **Exhaustive provenance by default** | Deciding why-provenance is NP-complete (A-KR2026). `why --all` gives it on request, bounded. |
| **An AI inside the toolchain** | Any AI is an author. It reads `galileo help` and the diagnostics, writes patches, and gets review's verdict and sentences back (§8). The review gate is the interface. |
| **Beagle's evidence-ranked repair pipeline** | Dropped (DECISIONS D-27, D-33). Galileo's diagnostics (did-you-mean, closest shapes) and the review gate do its job. |

Traces to: F10, F12, N6, P2, R (non-adopted recommendations), A-Inform7,
A-KR2026, A-DD, A-DBSP, A-telingo, A-Cedar, A-Flix, A-Ascent.

## 13. Acceptance scenarios

The MVP is done when all of these pass. PLAN.md maps each to a work package.

| # | Scenario | Measured by |
| --- | --- | --- |
| A1 | kanata: `default.gal` and `kanata.gal` emit `default.nix` and `kanata.nix` byte-identical to nixos-config's | `cmp` exits 0 for both |
| A2 | The printer round-trips all 50 Beagle-generated `.nix` | 49/50 byte-identical; `flake.nix` parse-identical, differing only at its two one-operand `++` calls |
| A3 | Every module, host and flake source is migrated, and firn's three Clause programs are replaced | whiterabbit's toplevel drvPath is unchanged. Every Beagle-output file is byte-identical (`flake.nix` as box 4.5 says). Every module no host enables is parse-identical and listed. `rg -c beagle-build\|compile-nix native/repo_build*.bjs` finds 0 compile calls. firn's dispatcher gives the same plan as `firn.clause` for every command in its table |
| A4 | `_generated-enables.nix` comes from rules | Identical for whiterabbit, and for ashashi at the pin `10740a8` (retired on nixos-config main at `7d7a8c9d`). `why firefox is enabled on whiterabbit` names the `browsers` tag and both absence checks |
| A5 | The responsibilities requirement on whiterabbit | Holds. Withdrawing "obsidian is realized by obsidian-module" makes it Fail with a proof |
| A6 | ChatGPT's 9 regression scenarios, in Galileo syntax, plus three `intends` contracts | Each scenario gives the verdict its test asserts. The contracts give approve, reject and unknown as written |
| A7 | The dungeon | §5.6's answers and review verdicts, and `replay` confirms every witness |
| A8 | North's menu slice | The sequences recorded from `main.rs:3547` and `restart_storage_tests.rs:388` replay under §10.4's observation contract (admissions, effects, projections, restart). One menu step at 1,000 rows takes under 1 ms |
| A9 | Budget honesty | On the 20-stage program (R13), `why` returns one proof and `why --all` returns `Unknown(work)` in under 1 s. On the 101-room chain (P7), a budget too small to finish gives `Unknown`, never `Fails`, and the default budget gives the correct answer or `Unknown` |
| A10 | Differential test | 10,000 generated programs, reference against engine, 0 disagreements |
| A11 | North runs on Galileo | Every slice replays under §10.4's contract with its coverage reported. North's tests pass with the Clause crates removed from `Cargo.toml`. `flake.gal` emits north's `flake.nix` |

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
  - in `flake.bnix`: overlays and marker sections.
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
  - `GalileoProofs.lean` checks on Lean 4.29.1 with no `sorry`. Lean 4.33.1
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
- **A-Datomic**: the as-of, since and history views of a database, https://docs.datomic.com/reference/filters.html
- **A-TLA**: TLA+ actions relate a state to the next with primed variables (`x' = x + 1`), https://lamport.azurewebsites.net/tla/book.html
- **A-Python-in**: Python's `in` is a comparison operator, at the precedence of `==`, https://docs.python.org/3/reference/expressions.html#membership-test-operations
- **A-Python-comprehension**: `[e for x in xs if c]` computes `e` once per binding its clauses produce, https://docs.python.org/3/reference/expressions.html#displays-for-lists-sets-and-dictionaries
- **A-Datomic-attr**: Datomic's attributes with cardinality one: an entity's value per attribute, asserted anew and kept in history, https://docs.datomic.com/schema/schema-reference.html
- **A-Gherkin**: a Scenario Outline's step template filled from an Examples table, one row per case, https://cucumber.io/docs/gherkin/reference/#scenario-outline
- **A-Temporal**: system-versioned temporal tables, a validity period per row, `FOR SYSTEM_TIME AS OF`, and a history row for every update, https://learn.microsoft.com/en-us/sql/relational-databases/tables/temporal-tables
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
| 4 Relations, facts, rules | F6, F7, F8, N1, N2, N3, P2, R3 | A-Souffle, A-Flix, A-Ascent, A-egglog, A-Datafun, A-Gherkin |
| 5 State, events, effects | F12, N2, N3, N4, N5, N6, R1, R2, R17 | A-Dedalus, A-Statelog, A-Upsert, A-EventCalculus, A-Alloy6, A-Apalache, A-telingo, A-Temporal, A-Datomic |
| 6 Requirements, verdicts | F7, F8, P1, P3, P4, R2, R4–R8, R12, R15, R17 | A-Alloy6, A-Apalache, A-MOLLY, A-Dafny |
| 7 Explanation | F6, P1, P6, R3, R10, R17, R18 | A-Souffle, A-KR2026, A-why-depends, A-EventCalculus, A-Datomic, A-Temporal, A-OPA |
| 8 Change review | F6, F13, P4, R3, R4, R6, R7, R9, R11, R20 | A-Cedar, A-semver-checks, A-Dafny, A-Dhall, A-Unison |
| 9 Syntax | F3, F4, F5, F6, F8, F9, F14, N3, N4, N10, P3, P4, R16 | A-Inform7, A-Attempto, A-Nix |
| 10 Compilation, evaluation | F1, F3, F4, F5, F6, F8, F10, F11, F12, F13, F15, N2, N3, N4, N5, N6, N7, R14 | A-Cedar, A-Nix, A-Ascent |
| 11 Tooling | F1, F6, F9, F10, F13, N4, N5, P6, R5, R17 | A-salsa, A-Lean, A-nixd, A-why-depends |
| 12 Not in the MVP | F10, F12, N6, P2 | A-Inform7, A-KR2026, A-DD, A-DBSP, A-telingo, A-Cedar, A-Flix, A-Ascent |
| 13 Acceptance | every item above | — |
