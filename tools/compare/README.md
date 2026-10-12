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
