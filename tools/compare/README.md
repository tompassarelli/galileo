# Comparison tooling

`compare` is a development-only, dependency-free Rust binary. Build it with
`nix develop -c cargo build -p compare --locked --offline`, then run
`target/debug/compare`. `--help` is available on the binary and each command.

## Size

Pass the formatted files for one column and part. Repeat for modules, hosts
and rules; include tag metadata in the modules score. The tool counts each
distinct input path once and prints a TSV header and one row:

```sh
target/debug/compare size --lang clause-beagle --part modules FILE...
target/debug/compare size --physical --lang clause-beagle --part tags FILE...
```

The fields are `column`, `part`, `files`, `lines`. Physical size counts newline
bytes, including blanks, exactly as `wc -l` does; an unterminated final line
adds zero. Default size excludes blank and comment-only lines. It recognizes
Beagle `;`, Clause `//` and `#`, CUE/Rust `//`, Dhall `--`, and other columns'
`#` comments, plus block comments and quoted text. Beagle's `#lang` directive
counts as source. Format inputs with their language's formatter first; Clause
and Beagle use their committed source.

## Token replay

```sh
target/debug/compare replay --dry-run --lang clause-beagle \
  --root "$FIRN_ROOT" --units bench/clause-beagle/units.tsv --cards CASES.tsv
target/debug/compare replay --lang nickel --root SCRATCH \
  --units bench/nickel/units.tsv --cards CASES.tsv \
  --check nickel export --format json main.ncl
```

`--lang` defaults to `clause-beagle`; `--units` to `bench/<lang>/units.tsv`;
`--cards` to `bench/corpus/cases.tsv`. `--root` is required. Put `--check` last:
its program and remaining arguments run directly, without a shell, in the
source root. Use an absolute program path for a checker outside the root.

Units use the existing TSV fields `id`, `kind`, `pin_file`, `start_line`,
`end_line`. A port can name its path field `file` instead of `pin_file`.
Ranges are inclusive and one-based. Paths must remain within the source root.
A unit may map to several ranges, including overlapping ranges.

Cards are tab-separated rows with a header containing these fields:

| Field | Meaning |
| --- | --- |
| `id` | Case identifier |
| `unit` | Unit ID in the column's manifest |
| `anchor` | Exact, nonempty, single-line source text |
| `replacement` | Single-line replacement, possibly empty |

Additional evidence or classification fields are retained in the input but
are not needed by replay. An anchor must match exactly one site within the
unit's ranges. Missing and ambiguous anchors fail with the case ID; replay
does not guess a site. Each card starts from the original source.

Dry run edits an in-memory scratch source and prints `case`, `file`, `line`,
`text`, without writing files or running the checker. Checked replay requires
a caller-owned scratch root, rejects paths under `pins`, checks the unmutated
source first, applies one card, runs the checker, and restores the source
before processing its result or the next card. It prints `case`, `column`,
`exit_status`, `first_diagnostic`, `names_file`, `wall_ms`. A rejected edit is
a recorded outcome, so replay continues; malformed cards or execution and
restoration errors fail replay. Structural cards belong to a later box.

## Blinded answers

```sh
target/debug/compare blind
target/debug/compare blind --bench SCRATCH_BENCH
```

Answers live at `bench/questions/answers/<column>/Q<n>.txt`, for columns
`galileo`, `clause-beagle`, `nix`, `nickel`, `cue`, `dhall` and `kcl`.
Each present column needs its `bench/<column>/units.tsv`. The manifest uses
the same fields as replay; no description field is added. Cited paths and
inclusive line ranges become the matching language-neutral unit IDs.
Overlapping ranges produce every matching ID. Absolute citations can match
the manifest's relative path suffix. Citations without a line name all
units in that file. Unmapped generated paths become "a generated file";
other unmapped paths become "another file".

The command removes column identities and the commands named in the
comparison note, including their identifying flags and source extensions.
Compound column tokens are dropped case-insensitively; stderr reports only their count per answer file.
It randomly assigns letters separately for each question, prints the blinded
answers and writes `bench/questions/blinded/Q<n>.txt`. The separate mapping
is `bench/questions/seal.tsv`, created with owner-only permissions and never
printed. Keep it away from the judge until scoring ends. Existing output
directories and seals are rejected. `--answers`, `--out` and `--seal` override
the individual paths. Partial answer collections are supported for the
two-answer preparation journey.

## Scaled corpus

```sh
FIRN_ROOT=<sample pin> bench/galileo/make.sh
FIRN_ROOT=<sample pin> bench/galileo/check.sh
bench/contract/make-reference.sh
target/debug/compare scale 21 --out bench/scaled
```

Run from the repository root. The output must be new and stays ignored because
it contains private runtime settings. The command checks that the sample's
Galileo build is current and writes the shared `context.json`, `private.json`,
`contract/{ashashi,whiterabbit}.json`, and `galileo-expected/` answer key.
Each sample module identity becomes `<name>_001` through `<name>_021`; the
originals become `_001`. Its `myConfig.modules` declarations and settings
follow that identity. NixOS target options, schema-type keys, package and
input references, strings, tags and rules stay unchanged. Cross-module reads
in the expected Nix stay within the same clone.

The contracts repeat each host's actual sample export: whiterabbit has 210
module exports, and ashashi has 21. Enabled arrays retain the sample's order
within each clone. The expected tree contains 210 module directories with
294 Nix files and three combined host Nix files; non-sample host settings
appear once. Only module file names, relative module imports and module
option paths change. Context entries outside the sample remain once, while
private module settings follow the cloned identities.

The command prints `clones` and `modules` as TSV (`21` and `210`). Each port's
separate `scale.py` creates one 210-definition program under
`bench/scaled/<lang>/`; its `check.sh --scaled` compares both exports to the
shared contracts. Galileo also compares its emitted Nix tree to
`galileo-expected/`. AF.36b–AF.36f implement those native generators and checks.

## Option types

```sh
target/debug/compare types
target/debug/compare types --schema bench/schema/sample.json --out NEW_DIRECTORY
nix develop ./bench --command nickel typecheck bench/schema/nickel/types.ncl
nix develop ./bench --command cue vet bench/schema/cue/types.cue
nix develop ./bench --command dhall type --file bench/schema/dhall/types.dhall
nix develop ./bench --command kcl run bench/schema/kcl/types.k
```

The command reads the frozen extractor subset and writes all 73 option
descriptors. Unsupported native types fail with their option name before
writing any files. The output root defaults to `bench/schema`; rerunning
replaces the four generated catalogs. Nickel's record maps exact option
names to contracts; CUE's `#Options` maps them to definitions; Dhall's
`types` maps them to types. KCL's `Option000`, etc. schemas follow sorted
option-name order and retain the exact name in their `name` field. The
`value` field checks that descriptor, for example `Option001 {value = True}`.

Nested records carrying `container` describe the extractor's container
element, not an inferred leaf type. Submodules remain open records or maps;
the printer invents no fields. The 16 unknown descriptors and six opaque
`coercedTo` descriptors remain unchecked in every language: Nickel `Dyn`,
CUE `_`, KCL `any`, and Dhall's hash-pinned Prelude JSON type. The native
`attrsOf` record without an inner descriptor also has arbitrary JSON values.
Dhall maps use `List {mapKey : Text, mapValue : T}`; JSON values and union
constructors use the language's documented JSON encoding. KCL nullable and
untyped values use optional attributes to allow `None`.

Packages use the existing symbolic `$ref` record with a string or list of
strings; there are two package descriptor occurrences, including the list
element. The single path descriptor accepts a string or a `$ref`/`$call`
record. All four apply those same shape checks, with no native package or
absolute-path check. Dhall's `Path` and `$ref` payload use union constructors
whose tags are erased by its JSON exporter.

The nullable unsigned 16-bit port uses a Nickel contract, a CUE numeric
bound, KCL checks, and Dhall `Optional Natural` plus `validPort`. Dhall callers
must assert the predicate on every port value:

```dhall
let S = ./bench/schema/dhall/types.dhall
let port = Some 123
let _ = assert : S.validPort port === True
in port : S.types.`myConfig.modules.kanata.port`
```

Use `nickel export` to force a value's runtime contract; `typecheck` validates
the printed contract file. CUE validates a value against
`-d '#Options["myConfig.modules.kanata.port"]'`. Run a KCL scratch value
together with `types.k`, and use `dhall type` on a typed scratch expression.
The contract, definitions and schema idioms follow the tools' primary docs:
[Nickel contracts](https://nickel-lang.org/user-manual/contracts/),
[CUE specification](https://cuelang.org/docs/reference/spec/),
[Dhall validation](https://docs.dhall-lang.org/howtos/How-to-validate-a-configuration-file.html),
and [KCL schemas](https://www.kcl-lang.io/docs/0.12/reference/lang/spec/schema).

## Sample review packets

```sh
target/debug/compare packets --sources target/tmp/review-sources \
  --reviews target/tmp/review-outputs
target/debug/compare packets --answers target/tmp/review-answers.tsv
```

For each of the nine committed `bench/review/cases.tsv` cards, the source
directory contains `<card>/base/` and `<card>/patched/` with the actual Galileo
sources, including the generated sample and patched local rule modules.
Keep Git metadata and generated Nix outside those directories. The output
directory contains `<card>.stdout` and `<card>.stderr` captured from a whole
native default-budget `galileo review` with the guarantees in scope. Unknown
output is retained. The command runs native `diff -ru base patched` from each
card directory, so packet text reveals no card identifier or absolute path.

The new `bench/review/effort/` directory receives `<card>-I.txt` and
`<card>-R.txt`. Both contain only the four numbered intent statements from
`intents.md`. I adds the native unified diff; R adds the exact
`bench/galileo/guarantees.gal` bytes and complete stdout followed by stderr.
No card descriptions or truth labels enter either packet. Preparation prints
the packet paths, physical lines and bytes; physical lines count newline bytes.
`--cards`, `--intents`, `--guarantees` and `--out` override the defaults.

For judging, copy each packet to its own fresh `mktemp -d` directory outside
any Git repository. Give the directory and file random opaque names and keep
the identity map in ignored preparation scratch. Replace native base and
patched temporary-path prefixes with `base/` and `patched/` in these copies;
leave all other bytes unchanged. Keep the committed card-named packets as
the source artifacts. Each directory contains only its one packet. Run one
fresh reviewer session with its working directory set there, pointing it only
to the opaque file and asking for approve or reject and the broken intent on
rejection.
Record the actual runtime measurements in an answers TSV with fields `card`,
`condition` (I or R), `verdict`, `intent` (I1–I4 on reject, `none` on approve),
`tokens` and `wall_ms`. The scoring invocation reads those answers and the
existing packets, then writes `bench/results/review-effort.tsv` (`--results`
overrides it). Size 10 is the ten-module sample. Verdict correctness uses the
committed breaking/control label; the named intent is recorded separately.
The command requires all eighteen unique answers, prints right verdicts out
of nine and median tokens, bytes and wall time for each condition, and marks
loss1 tripped exactly when R has fewer right verdicts than I. Agent sessions
stand in for people; this sample is the within-language, single-state case,
and R is limited by the requirements' stated dependencies.

The recorded sample run uses eighteen fresh Codex sessions in separate
isolated directories, with one judgment per opaque packet. Native session
metadata confirmed each working directory; structured command events show
only reading that session's assigned file. All eighteen verdicts and named
broken intents match the discarded first run, whose card-named paths and
repository working directory exposed the card classification. The corrected
run replaces every token and wall measurement in the result table.

Tokens are the actual Codex `threads.tokens_used` session totals; wall
milliseconds are `updated_at_ms - created_at_ms`, including startup and the
packet read. These sessions had no coach wait. Session IDs, rollout paths and
coldness checks are retained in the worker result receipt. Approved answers'
`n/a` intent is normalized to the table's `none` spelling.

Both conditions score 9/9; loss1 is not tripped. Median tokens are I 35,328
and R 38,108; median wall milliseconds are I 13,714 and R 13,204. Table bytes
measure the unchanged committed source packets: medians I 1,584 and R 10,177.
The judge copies preserve every I byte and shorten each R by 564 bytes through
only the neutral path replacements, so their R median is 9,613 bytes. These
session totals include overhead and do not isolate effort spent reading.

## Scaled review packets

```sh
target/debug/compare packets --scaled --sources target/tmp/scaled-review-sources \
  --reviews target/tmp/scaled-review-outputs
target/debug/compare packets --scaled --answers target/tmp/scaled-review-answers.tsv
```

The scaled route reads exactly three frozen cards from the single line
`Cards: B1, B4, C1` in `bench/review/scale.md`: two breaking cards from B1–B4
and one control. Commit that registration and its sweep before making any
corpus patch. Use the landed `bench/scaled/galileo` program to prepare each
base and patched source tree, with the project-local Firn rules and actual
guarantees in scope. Join each selected card to the registered sweep over
all 210 module clones. Build both trees and compare every emitted file with
`diff -r`; measure each whole native default-budget review once. Each native
implementation diff must contain at least 210 hunks. Keep the base and
patched sources and whole review outputs in ignored preparation scratch.

Packets default to `bench/review/effort/210/`. The four intents, native diff,
guarantees and whole review output follow the sample's packet format. The
scaled route retains partial native output if a once-only review times out.
Judge each packet in its own opaque `mktemp -d` directory outside every
repository, with one fresh session whose working directory is that directory. Retain
the actual rollout path and the same session token and wall measurements
used for the sample, then remove each exact judge directory.

Scoring requires six unique cold answers and appends six size-210 rows to
the existing eighteen size-10 rows without changing any sample bytes. It
prints right verdicts out of three, median tokens, bytes and wall time for
each condition, and their growth ratios against the recorded sample
medians. Loss2 trips when R gets fewer verdicts right than I or its median
tokens are not below I. The sample route retains its nine-card behavior.
