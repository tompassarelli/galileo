#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")/../.."
mkdir -p bench/kcl/out
trap 'printf "kcl: check failed; diagnostics retained in ignored out/\n" >&2' ERR
for host in whiterabbit ashashi; do
  kcl run bench/kcl/modules/main.k bench/schema/kcl/types.k bench/kcl/main.k --format json -D "host=$host" -o "bench/kcl/out/$host.json" 2>"bench/kcl/out/$host.log"
  jq -S . "bench/kcl/out/$host.json" > "bench/kcl/out/$host.sorted.json"
  jq -S . "bench/contract/out/$host.json" > "bench/kcl/out/$host.reference.json"
  diff --brief "bench/kcl/out/$host.reference.json" "bench/kcl/out/$host.sorted.json"
  printf 'kcl: %s whole export matches reference\n' "$host"
done
