#!/usr/bin/env bash
set -euo pipefail

if (( $# != 1 )); then
  printf 'usage: %s export.json\n' "$0" >&2
  exit 2
fi
pin=10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9
firn_root=$(readlink -f "${FIRN_ROOT:-$HOME/code/nixos-config/pins/$pin}")
export_file=$(readlink -f "$1")
shim=$(readlink -f "$(dirname "$0")/shim.nix")
scratch="$HOME/code/nixos-config/worktrees/af-5-contract"
if [[ $(git -C "$firn_root" rev-parse HEAD) != "$pin" ]]; then
  printf 'contract: FIRN_ROOT must be at %s\n' "$pin" >&2
  exit 2
fi
names=$(jq -er '.modules | keys[]' "$export_file")
jq -e '.enabled | type == "array"' "$export_file" >/dev/null
jq -e '.settings | type == "object"' "$export_file" >/dev/null
while IFS= read -r name; do
  if [[ ! "$name" =~ ^[a-z][a-z0-9-]*$ || ! -d "$firn_root/modules/$name" ]]; then
    printf 'contract: unknown module %s\n' "$name" >&2
    exit 2
  fi
done <<< "$names"
git -C "$firn_root" worktree add --quiet --detach "$scratch" "$pin" >&2
cleanup() {
  status=$?
  trap - EXIT
  if ! git -C "$firn_root" worktree remove --force "$scratch" >&2; then
    if (( status == 0 )); then status=1; fi
  fi
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
cp "$export_file" "$scratch/.contract-export.json"
cp "$shim" "$scratch/.contract-shim.nix"
while IFS= read -r name; do
  printf '{ config, lib, pkgs, inputs, flakeRoot, ... }@args: import ../../.contract-shim.nix { exportFile = ../../.contract-export.json; firnRoot = flakeRoot; moduleName = "%s"; } args\n' "$name" > "$scratch/modules/$name/default.nix"
done <<< "$names"
cat > "$scratch/.contract-eval.nix" <<'NIX'
{ firnRoot, scratch }:
let
  flake = builtins.getFlake "git+file://${firnRoot}?ref=HEAD&rev=10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9";
  root = flake.outPath;
  lib = flake.inputs.nixpkgs.lib;
  export = builtins.fromJSON (builtins.readFile (scratch + "/.contract-export.json"));
  names = builtins.attrNames export.modules;
  host = { config, lib, pkgs, inputs, flakeRoot, ... }@args:
    import (scratch + "/.contract-shim.nix") {
      exportFile = scratch + "/.contract-export.json";
      firnRoot = root;
      original = root + "/hosts/whiterabbit/configuration.nix";
      hostSettings = true;
    } args;
  enables = { config, lib, pkgs, inputs, flakeRoot, ... }@args:
    import (scratch + "/.contract-shim.nix") {
      exportFile = scratch + "/.contract-export.json";
      firnRoot = root;
      original = root + "/hosts/whiterabbit/_generated-enables.nix";
    } args;
  replacements = builtins.listToAttrs (
    map (name: {
      name = builtins.unsafeDiscardStringContext "${flake.modules}/${name}";
      value = scratch + "/modules/${name}";
    }) names
    ++ [
      {
        name = builtins.unsafeDiscardStringContext "${root}/hosts/whiterabbit/configuration.nix";
        value = host;
      }
      {
        name = builtins.unsafeDiscardStringContext "${root}/hosts/whiterabbit/_generated-enables.nix";
        value = enables;
      }
    ]
  );
  rewrite = module:
    if lib.isFunction module then
      lib.setFunctionArgs (args: rewrite (module args)) (lib.functionArgs module)
    else if builtins.isAttrs module then
      module // lib.optionalAttrs (module ? imports) {
        imports = map rewrite module.imports;
      }
    else if builtins.hasAttr (builtins.unsafeDiscardStringContext (toString module)) replacements then
      rewrite replacements.${builtins.unsafeDiscardStringContext (toString module)}
    else
      module;
  outputs = (import (root + "/flake.nix")).outputs (flake.inputs // {
    self = flake // outputs;
    nixpkgs = flake.inputs.nixpkgs // {
      lib = lib // {
        nixosSystem = args: flake.inputs.nixpkgs.lib.nixosSystem (args // {
          modules = map rewrite args.modules;
        });
      };
    };
  });
in
outputs.nixosConfigurations.whiterabbit.config.system.build.toplevel.drvPath
NIX
AF5_FIRN_ROOT="$firn_root" AF5_SCRATCH="$scratch" nix eval --impure --raw --expr '
  import (builtins.toPath (builtins.getEnv "AF5_SCRATCH" + "/.contract-eval.nix")) {
    firnRoot = builtins.getEnv "AF5_FIRN_ROOT";
    scratch = builtins.getEnv "AF5_SCRATCH";
  }
'
printf '\n'
