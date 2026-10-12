#!/usr/bin/env bash
set -euo pipefail

if (( $# != 0 )); then
  printf 'usage: %s\n' "$0" >&2
  exit 2
fi
pin=10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9
firn_root=$(readlink -f "${FIRN_ROOT:-$HOME/code/nixos-config/pins/$pin}")
if [[ $(git -C "$firn_root" rev-parse HEAD) != "$pin" ]]; then
  printf 'contract: FIRN_ROOT must be at %s\n' "$pin" >&2
  exit 2
fi
contract_root=$(dirname "$(readlink -f "$0")")
mkdir -p "$contract_root/out"
for host in whiterabbit ashashi; do
  AF6_FIRN_ROOT="$firn_root" AF6_HOST="$host" nix eval --impure --json --file "$contract_root/reference.nix" --apply 'reference: reference { firnRoot = builtins.getEnv "AF6_FIRN_ROOT"; host = builtins.getEnv "AF6_HOST"; }' | jq -S . > "$contract_root/out/$host.json"
done
jq -n -S --slurpfile ashashi "$contract_root/out/ashashi.json" --slurpfile whiterabbit "$contract_root/out/whiterabbit.json" '{ashashi: {users: {email: $ashashi[0].settings.myConfig.modules.users.email}}, whiterabbit: {kanata: {devices: $whiterabbit[0].settings.myConfig.modules.kanata.devices}, users: {email: $whiterabbit[0].settings.myConfig.modules.users.email}}}' > "$contract_root/out/private.json"
