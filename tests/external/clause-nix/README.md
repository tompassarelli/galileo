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

Measured after rebasing on `209fa44` (the landed 4.16b prerequisite):
184/184 parsed, 167 parse-identical, 17 first failures. The corpus test
passed in 24.75 seconds. The test prints every contributing
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

## Measured first failures

The final gate includes the eight signatures and ordered application
arguments. Fifteen outputs first differ at a nested selection: the
original tree selects each segment separately, while the rebuilt tree
selects a path. One output changes `+` to `++`, and one is rejected by
the checker. The snippets below are excerpts of the actual parsed trees,
except timezone's rejected Galileo expression. A checker rejection is
never reported as equality.

| Committed output | First failing construct | Original excerpt | Rebuilt excerpt |
| --- | --- | --- | --- |
| `modules/activity/default.nix` | Nested selection, character 715 | `(((((__clause_argument_0).config).lib).file).mkOutOfStoreSymlink` | `(__clause_argument_0).config.lib.file.mkOutOfStoreSymlink` |
| `modules/awscli/default.nix` | Addition becomes list concatenation, character 741 | `+ (config).sops.placeholder.aws-secret-access-key` | `++ (config).sops.placeholder.aws-secret-access-key` |
| `modules/bash/default.nix` | Nested selection, character 713 | `(((((__clause_argument_1).config).lib).file).mkOutOfStoreSymlink` | `(__clause_argument_1).config.lib.file.mkOutOfStoreSymlink` |
| `modules/delivery-liveness/default.nix` | Nested selection, character 503 | `(((__clause_argument_0).config).home).homeDirectory` | `(__clause_argument_0).config.home.homeDirectory` |
| `modules/doom-emacs/default.nix` | Nested selection, character 240 | `(((((__clause_argument_0).config).lib).dag).entryAfter` | `(__clause_argument_0).config.lib.dag.entryAfter` |
| `modules/fastfetch/default.nix` | Nested selection, character 319 | `(((((__clause_argument_0).config).lib).file).mkOutOfStoreSymlink` | `(__clause_argument_0).config.lib.file.mkOutOfStoreSymlink` |
| `modules/gtk/default.nix` | Nested selection, character 689 | `((((__clause_argument_3).config).gtk).font).name` | `(__clause_argument_3).config.gtk.font.name` |
| `modules/ironbar/default.nix` | Nested selection, character 1090 | `(((((__clause_argument_0).config).lib).file).mkOutOfStoreSymlink` | `(__clause_argument_0).config.lib.file.mkOutOfStoreSymlink` |
| `modules/neovim/default.nix` | Nested selection, character 241 | `(((((__clause_argument_0).config).lib).file).mkOutOfStoreSymlink` | `(__clause_argument_0).config.lib.file.mkOutOfStoreSymlink` |
| `modules/north-profile/default.nix` | Nested selection, character 253 | `(((((__clause_argument_26).config).lib).dag).entryAfter` | `(__clause_argument_26).config.lib.dag.entryAfter` |
| `modules/nyxt/default.nix` | Nested selection, character 304 | `(((((__clause_argument_0).config).lib).file).mkOutOfStoreSymlink` | `(__clause_argument_0).config.lib.file.mkOutOfStoreSymlink` |
| `modules/rofi/default.nix` | Nested selection, character 969 | `(((((__clause_argument_4).config).lib).file).mkOutOfStoreSymlink` | `(__clause_argument_4).config.lib.file.mkOutOfStoreSymlink` |
| `modules/smashcraft-controller/default.nix` | Nested selection, character 337 | `(((__clause_argument_0).config).home).homeDirectory` | `(__clause_argument_0).config.home.homeDirectory` |
| `modules/tealdeer/default.nix` | Nested selection, character 315 | `(((((__clause_argument_0).config).lib).file).mkOutOfStoreSymlink` | `(__clause_argument_0).config.lib.file.mkOutOfStoreSymlink` |
| `modules/timezone/default.nix` | Checker rejects `zone` at line 5:52; its option is declared inside `imports` | `config."myConfig"."modules"."timezone"."zone"` | `config.myConfig.modules.timezone.zone` (build rejected) |
| `modules/waybar/default.nix` | Nested selection, character 1105 | `(((((__clause_argument_0).config).lib).file).mkOutOfStoreSymlink` | `(__clause_argument_0).config.lib.file.mkOutOfStoreSymlink` |
| `modules/wl-gammarelay/default.nix` | Nested selection, character 818 | `(((((__clause_argument_10).config).lib).file).mkOutOfStoreSymlink` | `(__clause_argument_10).config.lib.file.mkOutOfStoreSymlink` |
