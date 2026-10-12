#!/usr/bin/env bash
set -euo pipefail

scaled=false
if (( $# == 1 )) && [[ $1 == --scaled ]]; then
  scaled=true
elif (( $# != 0 )); then
  printf 'usage: %s\n' "$0" >&2
  exit 2
fi
cd "$(dirname "$(readlink -f "$0")")/../.."
if $scaled; then
  cue_root=bench/scaled/cue
  contract_root=bench/scaled/contract
  context_file=bench/scaled/context.json
  private_file=bench/scaled/private.json
else
  bench/contract/make-reference.sh
  cue_root=bench/cue
  contract_root=bench/contract/out
  context_file=bench/context.json
  private_file=bench/contract/out/private.json
fi
mkdir -p "$cue_root/out"
export scaled cue_root contract_root context_file private_file
nix develop ./bench -c bash -euo pipefail <<'SH'
inputs=(bench/schema/cue/types.cue "$cue_root/main.cue")
if ! $scaled; then
  inputs+=(bench/cue/hosts.cue bench/cue/modules.cue bench/cue/rules.cue)
fi
inputs+=(--with-context -l 'strings.TrimSuffix(path.Base(filename), ".json")' "$context_file" "$private_file")
for host in whiterabbit ashashi; do
  cue vet "${inputs[@]}" -t "host=$host"
  if $scaled; then
    cue eval "${inputs[@]}" -t "host=$host" -c -e _realization -e _inputs -e _contributions -e _active -e _overrides >/dev/null
  else
    cue eval "${inputs[@]}" -t "host=$host" -c -e _realization -e _inputs -e _contributions.firefox -e _active.firefox -e _overrides >/dev/null
  fi
  cue export "${inputs[@]}" -t "host=$host" -e export --out json > "$cue_root/out/$host.json"
  diff -q <(jq -S . "$contract_root/$host.json") <(jq -S . "$cue_root/out/$host.json")
  printf 'cue: %s export matches reference\n' "$host"
done
SH
