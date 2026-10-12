#!/usr/bin/env bash
set -euo pipefail

if (( $# != 0 )); then
  printf 'usage: %s\n' "$0" >&2
  exit 2
fi
cd "$(dirname "$(readlink -f "$0")")/../.."
bench/contract/make-reference.sh
mkdir -p bench/cue/out
nix develop ./bench -c bash -euo pipefail <<'SH'
inputs=(bench/schema/cue/types.cue bench/cue/main.cue bench/cue/hosts.cue bench/cue/modules.cue bench/cue/rules.cue)
inputs+=(--with-context -l 'strings.TrimSuffix(path.Base(filename), ".json")' bench/context.json bench/contract/out/private.json)
for host in whiterabbit ashashi; do
  cue vet "${inputs[@]}" -t "host=$host"
  cue eval "${inputs[@]}" -t "host=$host" -c -e _realization -e _inputs -e _contributions.firefox -e _active.firefox -e _overrides >/dev/null
  cue export "${inputs[@]}" -t "host=$host" -e export --out json > "bench/cue/out/$host.json"
  diff -q <(jq -S . "bench/contract/out/$host.json") <(jq -S . "bench/cue/out/$host.json")
  printf 'cue: %s export matches reference\n' "$host"
done
SH
