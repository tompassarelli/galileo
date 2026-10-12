#!/usr/bin/env bash
set -euo pipefail

if (( $# != 0 )); then
  printf 'usage: %s\n' "$0" >&2
  exit 2
fi
nickel_root=$(dirname "$(readlink -f "$0")")
bench_root=$(dirname "$nickel_root")
pin=10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9
export FIRN_ROOT=$(readlink -f "${FIRN_ROOT:-$HOME/code/nixos-config/pins/$pin}")
mkdir -p "$nickel_root/out"
"$bench_root/contract/make-reference.sh"
for host in whiterabbit ashashi; do
  nix develop "$bench_root" -c nickel export "$nickel_root/main.ncl" --field "$host" --output "$nickel_root/out/$host.json"
  jq -S . "$nickel_root/out/$host.json" > "$nickel_root/out/$host.sorted.json"
  if ! diff -u "$bench_root/contract/out/$host.json" "$nickel_root/out/$host.sorted.json" > "$nickel_root/out/$host.diff"; then
    printf 'nickel: %s export differs from reference\n' "$host" >&2
    exit 1
  fi
  printf 'nickel: %s whole export equals reference\n' "$host"
done
nix eval --impure --raw --expr 'let flake = builtins.getFlake ("git+file://" + builtins.getEnv "FIRN_ROOT" + "?ref=HEAD&rev=10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9"); in flake.nixosConfigurations.whiterabbit.config.system.build.toplevel.drvPath' > "$nickel_root/out/pin.drvPath"
"$bench_root/contract/gate.sh" "$nickel_root/out/whiterabbit.json" > "$nickel_root/out/gate.drvPath"
test "$(cat "$nickel_root/out/pin.drvPath")" = "$(cat "$nickel_root/out/gate.drvPath")"
cat "$nickel_root/out/gate.drvPath"
