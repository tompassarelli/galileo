#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"
mkdir -p out
trap 'printf "dhall: check failed; diagnostics retained in ignored out/\n" >&2' ERR
json-to-dhall ./context-type.dhall --file ../context.json --output out/context.dhall 2>out/diagnostics.log
json-to-dhall ./private-type.dhall --file ../contract/out/private.json --output out/private.dhall 2>>out/diagnostics.log
dhall-to-json --preserve-null --file main.dhall --output out/both.json 2>>out/diagnostics.log
jq -e '.whiterabbit.inputs == .ashashi.inputs and .whiterabbit.inputs == {nur: "github:nix-community/NUR", "zen-browser": "github:0xc000022070/zen-browser-flake"}' out/both.json >/dev/null
for host in whiterabbit ashashi; do
  jq --sort-keys ".$host | del(.inputs)" out/both.json > "out/$host.json"
  jq --sort-keys . "../contract/out/$host.json" > "out/$host.reference.json"
  diff --brief "out/$host.reference.json" "out/$host.json"
  printf 'dhall: %s exact whole export matches reference\n' "$host"
done
