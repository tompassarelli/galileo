# Firn construct import reference

The reference is the committed Nix at `FIRN_ROOT`, defaulting to
`~/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9`.
Files are read in place. The test writes only imported Galileo under its
lane-local Cargo scratch directory, invokes `galileo import nix`, then
`galileo fmt --check` and `galileo build --stdout`. Six emitted files must
match every reference byte. The committed and emitted `flake.nix` text must
print identical bytes through `nix-instantiate --parse -`, each read from
standard input in the same `FIRN_ROOT` directory (D-38, D-59).
An import or checker rejection is a failed file, never an equal comparison.

The eight constructs named by PLAN 4.2 map as follows. Forms show the source
register spelling and the corresponding emitted Nix; full-file comparison is
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

PLAN 4.3's case imports and builds every `modules/**` file a `.bnix` emits
(the 1.5 manifest's module rows) and requires each to be byte-identical; it prints
how many import as `nix module` and `nix file`. `modules/quickshell/default.nix` and
`modules/zen-browser/zen-browser.nix` select by a computed key, which SPEC
§9.4 keeps out of the MVP; they wait on DECISION 4.3.

The external-reference cases live in `cases.rs`, with a two-line entry in
`crates/galileo-cli/tests/firn_import.rs`. Run the affected CLI and importer
gates through the repository's documented depbox/Nix capacity wrapper.

The lift reuses the existing text-block and nested formal-default
representations. D-59 allows the inline text-block closer to carry its
record or call delimiters, and compares the flake by parse identity because
its two one-operand `++` calls have no byte-identical spelling. Every selected
file must import, pass `fmt --check`, build and satisfy its comparison;
no file or construct is skipped.
