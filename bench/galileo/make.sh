#!/usr/bin/env bash
set -euo pipefail

if (( $# != 0 )); then
  printf 'usage: FIRN_ROOT=<sample pin> %s\n' "$0" >&2
  exit 2
fi
: "${FIRN_ROOT:?Set FIRN_ROOT to the sample pin}"
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
export FIRN_ROOT=$(realpath "$FIRN_ROOT")
if [[ $(git -C "$FIRN_ROOT" rev-parse HEAD) != 10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9 ]]; then
  printf 'galileo make: FIRN_ROOT must be the sample pin 10740a8\n' >&2
  exit 2
fi
export CARGO_TARGET_DIR="$root/target" PYTHONDONTWRITEBYTECODE=1
cd "$root"
exec nix develop -c bash -euo pipefail <<'SH'
cargo test -p galileo-cli --locked --offline --test generated_enables -- --nocapture
cargo test -p galileo-cli --locked --offline --test firn_responsibilities a5_realization_holds_on_whiterabbit_and_fails_with_a_proof_after_the_withdrawal -- --exact --nocapture
python3 - <<'PY'
import json
import os
import re
import subprocess
from pathlib import Path

pin = Path(os.environ['FIRN_ROOT'])
target = Path(os.environ['CARGO_TARGET_DIR'])
out = Path('bench/galileo/out')
binary = target / 'debug/galileo'
sample = {'firefox', 'zen-browser', 'kanata', 'users', 'lane-gc', 'atuin', 'chrome', 'godot', 'bun', 'obsidian'}
scratch = target / 'tmp/generated-enables-whiterabbit'

def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)

def imported(path):
    return subprocess.check_output([binary, 'import', 'nix', path], text=True)

for name in sorted(sample):
    source = scratch / f'modules/{name}/default.gal'
    metadata = source.read_text().split('nix module', 1)[0]
    for nix in sorted((pin / f'modules/{name}').glob('*.nix')):
        if nix.stem not in ('default', name):
            continue
        text = imported(nix)
        if nix.stem == 'default' and (nix.parent / 'default.bnix').exists():
            text = metadata + text
        write(out / f'modules/{name}/{nix.stem}.gal', text)

write(out / 'config/tags.gal', (scratch / 'config/tags.gal').read_text())
names = set(json.loads((pin / 'config/darwin-modules.json').read_text()))
for host in ('ashashi', 'whiterabbit'):
    text = (target / f'tmp/generated-enables-{host}/_generated-enables.gal').read_text()
    write(out / f'hosts/{host}/_generated-enables.gal', text)
    names.update(re.findall(r'^  (?:adds|drops|disables) (\S+)', text, re.M))

context = ['use firn/responsibilities', 'use firn/tags', '']
for name in sorted(names - sample):
    metadata = (scratch / f'modules/{name}/default.gal').read_text().split('nix module', 1)[0]
    focus = False
    for line in metadata.splitlines():
        if line == name:
            context.append(line)
            focus = True
        elif re.fullmatch(r'  (?:is a Module|is tagged \S+|can opt into \S+)', line) and focus:
            context.append(line)
        elif re.fullmatch(re.escape(name) + r' (?:is a Module|is tagged \S+|can opt into \S+)', line):
            context.append(line)
    context.append('')
responsibilities = (target / 'tmp/whiterabbit.gal').read_text().splitlines()
context.extend(line for line in responsibilities if line in (
    'knowledge-authoring-workflow is a Responsibility', 'knowledge-authoring is a Capability'))
write(out / 'context.gal', '\n'.join(context) + '\n')
selected = [line for line in responsibilities if line in (
    'obsidian is a Software', 'obsidian is a Module',
    'knowledge-authoring-workflow requires software obsidian',
    'obsidian provides capability knowledge-authoring', 'obsidian is realized by obsidian')]
if len(selected) != 5:
    raise SystemExit('galileo make: expected five obsidian statements from the existing generator')
write(out / 'hosts/whiterabbit/responsibilities.gal',
      'use firn/responsibilities\nuse firn/tags\n\n' + '\n'.join(selected) + '\n')

configuration = imported(pin / 'hosts/whiterabbit/configuration.nix')
write(out / 'probes/configuration.txt', configuration)
lines = configuration.splitlines()
selected = [line for line in lines if re.fullmatch(r'whiterabbit turns on (users|kanata|lane-gc)', line)]
selected.extend(['', 'nix file', '  arguments: config, lib, pkgs, ...', '  value'])
for index, line in enumerate(lines):
    if line.startswith('    for ?m where whiterabbit turns on ?m'):
        selected.extend(lines[index:index + 2])
    elif line.startswith('    myConfig.modules.users.'):
        selected.append(line)
    elif line == '    myConfig.modules.kanata':
        selected.append(line)
        for child in lines[index + 1:]:
            if not child.startswith('      '):
                break
            selected.append(child)
write(out / 'hosts/whiterabbit/configuration.gal', 'use firn/tags\n\n' + '\n'.join(selected) + '\n')
print(f'generated ten sample modules; excluded context: {len(names - sample)} nonsample modules, '
      f'{len(context)} physical lines, {sum(bool(line.strip()) for line in context)} nonblank lines')
PY
target/debug/galileo build bench/galileo/out
SH
