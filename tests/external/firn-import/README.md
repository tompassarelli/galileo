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
| `with` | `modules/protonup-qt/default.nix` | `with pkgs; [protonup-qt]` | `with pkgs; [ protonup-qt ]` |
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
DECISION 4.2 records the prerequisite for full-file inline indented-string
lowering; nested formal defaults already have the SPEC §9.4 representation.
