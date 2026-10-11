# Firn construct import reference

The reference is the committed Nix at `FIRN_ROOT`, defaulting to
`~/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9`.
Files are read in place. The test writes only imported Galileo under its
lane-local Cargo scratch directory, invokes `galileo import nix`, then
`galileo build --stdout`, and compares every emitted byte with the reference.
An import or checker rejection is a failed file, never an equal comparison.

The eight constructs named by PLAN 4.2 map as follows. Forms show the source
register spelling and the corresponding emitted Nix; full-file equality is
the gate, including the surrounding module, declarations and scripts.

| Construct | Committed reference file | Galileo form | Emitted Nix |
| --- | --- | --- | --- |
| `with` | `modules/protonup-qt/default.nix` | `with pkgs: [protonup-qt]` | `with pkgs; [ protonup-qt ]` |
| `//` | `flake.nix` | `{ inputs: { … }, flakeRoot: self } // extraSpecialArgs` | `({ inputs = { … }; flakeRoot = self; } // extraSpecialArgs)` |
| Lambda with formals | `modules/claude/default.nix` | `({config, ...}) => { home.file: … }` | `({ config, ... }: { home.file = …; })` |
| Nested home-manager module | `modules/claude/default.nix` | `home-manager.users.{username}: ({config, ...}) => { … }` | `home-manager.users.${username} = ({ config, ... }: { … });` |
| `mkOutOfStoreSymlink` | `modules/claude/default.nix` | `config.lib.file.mkOutOfStoreSymlink("{config.home.homeDirectory}/…")` | `config.lib.file.mkOutOfStoreSymlink "${config.home.homeDirectory}/…"` |
| Symbol key | `modules/users/users.nix` | `users.users.{username}: { … }` | `users.users.${username} = { … };` |
| Escaped interpolation `''${` | `modules/beagle/default.nix` | Text block containing literal `${BEAGLE_CHECKOUT:-$HOME/code/beagle/main}` | Indented string containing `checkout=''${BEAGLE_CHECKOUT:-$HOME/code/beagle/main}` |
| `import` | `flake.nix` | `import(nixpkgs-unstable)({ system: system, config.allowUnfree: true })` | `import nixpkgs-unstable { system = system; config.allowUnfree = true; }` |

The same seven-file selection also exercises F3/F4 option declarations,
module arguments, lambda-application `let` bindings, paths distinct from
strings, dotted attribute paths, `null`, lists, interpolation, multiline
strings, `mkIf`, `mkForce`, `mkOption`, `mkEnableOption`, calls to `builtins`,
conditionals, list concatenation, flake inputs and overlays. Generated enables
and the flake migration retain their dedicated PLAN gates (4.3–4.5, 5.2, 5.6).

The external-reference case lives in `cases.rs`, with a two-line entry in
`crates/galileo-cli/tests/firn_import.rs`. Run the affected CLI and importer
gates through the repository's documented depbox/Nix capacity wrapper.

The initial public-CLI probe on `14cb6584` compared five of the seven files
byte-identically, with zero byte mismatches. Two files were rejected before
comparison: `modules/beagle/default.nix` at `inline indented string at
mkDev.text`, and `flake.nix` at `formal default at
outputs.lib.mkSystem.system`. These are failures of the required gate.
The lift now reuses the existing text-block and nested formal-default
representations. The affected gate rebased on `209fa44c` compiled in 93 seconds
and still reports five byte-identical comparisons, zero byte mismatches and
two rejected files. Both now import successfully but fail to build: the
printer emits inline closing markers with trailing record/call delimiters
(for example `'''})`), while the lexer recognizes only a closing marker alone
on its line. Beagle fails at imported line 5/column 3, the flake at line
41/column 5, each with `unterminated text block`.

DECISION 4.2 records this printer/lexer continuation prerequisite. The gate
remains strict; no file or construct is skipped to claim equality. No new
signature is required by the five successful files, and the two failed files
have not reached signature checking.
