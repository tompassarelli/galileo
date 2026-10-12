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
It randomly assigns letters separately for each question, prints the blinded
answers and writes `bench/questions/blinded/Q<n>.txt`. The separate mapping
is `bench/questions/seal.tsv`, created with owner-only permissions and never
printed. Keep it away from the judge until scoring ends. Existing output
directories and seals are rejected. `--answers`, `--out` and `--seal` override
the individual paths. Partial answer collections are supported for the
two-answer preparation journey.

## Scaled corpus

```sh
target/debug/compare scale 21 --root SCRATCH_PORT --out NEW_SCALED_DIRECTORY
```

The source port's `modules/` contains one directory or source file per module.
For each clone the command copies the full UTF-8 port into `clone-001`, etc.,
suffixing each module name with `_001`, etc. It renames module paths and
exact identifier tokens throughout all files, including import strings and
references in host or entrypoint files. It preserves executable permissions.
The source stays unchanged, and the output must be new and outside it.
Ten source modules and 21 clones produce 210 renamed modules; the command
prints a TSV with `clones` and `modules`. Combining a language's clone
entrypoints and running its checker belong to AF.36 after the ports exist.
