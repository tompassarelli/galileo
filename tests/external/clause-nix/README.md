# Committed Clause Nix outputs (PLAN 4.6, D-39)

The external-reference test is `crates/galileo-cli/tests/clause_nix.rs`.
It reads the 184 committed `modules/*/default.nix` outputs in place at
`FIRN_ROOT`, defaulting to the immutable nixos-config pin
`10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9`. The presence of
`tags.clause` selects module directories; no Clause source is opened and
no fixture is copied. The census visits the existing `galileo-nix` AST
returned by `galileo_import::nix::parse`, shared with `nix-roundtrip`.

Each output passes through the public `galileo import nix` command, the
record printer, and `galileo build --stdout`. Both original and rebuilt
Nix are read by `nix-instantiate --parse -` from standard input in the
same directory. Equality means byte equality of those parsed trees.
An import, checker, build or reference-parser rejection counts as a
failure. Every module is reported as identical or with its first failure;
parse differences include concise original/rebuilt parsed-Nix snippets.
The partial-coverage gate is at least 20 identical outputs; 4.7 owns the
remaining outputs. Nothing renames generated arguments, rewrites text
chains as interpolation, or normalizes either input for comparison.

Done when:

1. Census all 184 committed outputs, with construct occurrences and actual files.
2. Import/build all 184; compare Nix parsed trees, with at least 20 identical.
3. Preserve generated argument names, text-chain association, quoted and computed keys; add only signatures for actual corpus calls.
4. Run affected import/CLI gates after the final rebase, preserving their assertions.
5. Commit the exact four paths and request Claude review after wrappers release.

Not required: Clause parsing or source ledgers, copied fixtures, custom
normalization, proof machinery, a new dependency, all-output acceptance,
or a workspace sweep.

## Construct census

Measured on the initial whole-product pass: 184/184 parsed, 164
parse-identical, 20 first failures. The test prints every contributing
file for each construct; this table gives one actual file per construct.
Occurrences include nested expressions and keys; generated argument
lambdas are also included in the identifier-lambda count.

| Construct | Occurrences | Modules | Actual file |
| --- | ---: | ---: | --- |
| application | 670 | 184 | `modules/activity/default.nix` |
| call builtins.readFile | 4 | 2 | `modules/framework13-mic/default.nix` |
| call builtins.toString | 41 | 15 | `modules/bash/default.nix` |
| call lib.concatStrings | 1 | 1 | `modules/starship/default.nix` |
| call lib.makeBinPath | 2 | 2 | `modules/delivery-liveness/default.nix` |
| call lib.mkDefault | 1 | 1 | `modules/nyxt/default.nix` |
| call lib.mkEnableOption | 183 | 183 | `modules/activity/default.nix` |
| call lib.mkForce | 4 | 2 | `modules/bash/default.nix` |
| call lib.mkIf | 187 | 183 | `modules/activity/default.nix` |
| call lib.mkMerge | 1 | 1 | `modules/bash/default.nix` |
| call lib.mkOption | 8 | 8 | `modules/awscli/default.nix` |
| computed key | 33 | 30 | `modules/activity/default.nix` |
| formals lambda | 184 | 184 | `modules/activity/default.nix` |
| generated argument lambda | 18 | 18 | `modules/activity/default.nix` |
| identifier | 1163 | 184 | `modules/activity/default.nix` |
| identifier lambda | 18 | 18 | `modules/activity/default.nix` |
| if | 4 | 2 | `modules/ghostty/default.nix` |
| integer | 35 | 13 | `modules/activity/default.nix` |
| left-nested + chain | 255 | 27 | `modules/activity/default.nix` |
| list | 228 | 159 | `modules/activity/default.nix` |
| open formals | 184 | 184 | `modules/activity/default.nix` |
| operator + | 320 | 27 | `modules/activity/default.nix` |
| operator == | 3 | 1 | `modules/gtk/default.nix` |
| path | 4 | 2 | `modules/framework13-mic/default.nix` |
| quoted key | 2321 | 184 | `modules/activity/default.nix` |
| quoted selection | 2274 | 184 | `modules/activity/default.nix` |
| record | 1767 | 184 | `modules/activity/default.nix` |
| selection | 1502 | 184 | `modules/activity/default.nix` |
| string | 691 | 184 | `modules/activity/default.nix` |

There are ten distinct `lib`/`builtins` calls. The eight missing
signatures are declared in the existing `firn/nix-lib`, alongside
`builtins.readFile` and `lib.mkForce`. Curried Nix applications are
transcribed as one Galileo call with ordered arguments and lower back to
the same application chain.

## Initial first failures

These are the initial measurement, before the final quoted-key
prerequisite and signature check. The final affected gate prints the
current coverage and each first differing construct. A checker rejection
is never reported as equality.

| Committed output | First failure |
| --- | --- |
| `modules/activity/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/awscli/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/bash/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/delivery-liveness/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/doom-emacs/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/drracket/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/fastfetch/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/gtk/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/ironbar/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/mini-serve/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/neovim/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/north-profile/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/nyxt/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/rofi/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/smashcraft-controller/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/smashcraft-test-pads/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/tealdeer/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/timezone/default.nix` | Checker rejects selection of undeclared `myConfig.modules.timezone.zone` from an option declared inside `imports`. |
| `modules/waybar/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
| `modules/wl-gammarelay/default.nix` | Nix parse tree differs; the final gate prints the first differing original/rebuilt construct. |
