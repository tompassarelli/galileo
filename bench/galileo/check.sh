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
  printf 'galileo check: FIRN_ROOT must be the sample pin 10740a8\n' >&2
  exit 2
fi
export CARGO_TARGET_DIR="$root/target" PYTHONDONTWRITEBYTECODE=1
cd "$root"
exec nix develop -c bash -euo pipefail <<'SH'
target/debug/galileo check bench/galileo/out
target/debug/galileo build --check bench/galileo/out
python3 - <<'PY'
import csv
import os
import subprocess
from pathlib import Path

pin = Path(os.environ['FIRN_ROOT'])
out = Path('bench/galileo/out')
beagle = ('firefox', 'zen-browser', 'kanata', 'users', 'lane-gc')
clause = ('atuin', 'chrome', 'godot', 'bun', 'obsidian')
sample = set(beagle + clause)
if {path.name for path in (out / 'modules').iterdir()} != sample:
    raise SystemExit('galileo check: expected exactly ten sample modules')
byte_count = parse_count = 0
for name in beagle:
    for source in sorted((pin / f'modules/{name}').glob('*.nix')):
        if source.stem not in ('default', name):
            continue
        actual = out / f'modules/{name}/{source.name}'
        if source.read_bytes() != actual.read_bytes():
            raise SystemExit(f'galileo check: byte mismatch: {actual}')
        byte_count += 1
for name in clause:
    source = pin / f'modules/{name}/default.nix'
    actual = out / f'modules/{name}/default.nix'
    parsed = [subprocess.check_output(['nix-instantiate', '--parse', '-'],
              input=path.read_bytes(), cwd=pin) for path in (source, actual)]
    if parsed[0] != parsed[1]:
        raise SystemExit(f'galileo check: parse mismatch: {actual}')
    parse_count += 1
print(f'module outputs: {byte_count} byte-identical, {parse_count} parse-identical')

def sample_lines(path):
    selected = []
    for line in path.read_text().splitlines():
        if any(f'myConfig.modules.{name}.enable =' in line or f'myConfig.modules."{name}".enable =' in line for name in sample):
            selected.append(line)
        elif 'myConfig.modules.firefox.default =' in line:
            selected.append(line)
    return selected

for host in ('ashashi', 'whiterabbit'):
    expected = sample_lines(pin / f'hosts/{host}/_generated-enables.nix')
    actual = sample_lines(out / f'hosts/{host}/_generated-enables.nix')
    if expected != actual:
        raise SystemExit(f'galileo check: sample enable/override mismatch: {host}')
    print(f'{host}: {len(actual)} sample enable/override lines identical')

source = subprocess.check_output([Path(os.environ['CARGO_TARGET_DIR']) / 'debug/galileo',
         'import', 'nix', pin / 'hosts/whiterabbit/configuration.nix'], text=True).splitlines()
configuration = (out / 'hosts/whiterabbit/configuration.gal').read_text().splitlines()
facts = [f'whiterabbit turns on {name}' for name in ('users', 'kanata', 'lane-gc')]
for fact in facts:
    if source.count(fact) != 1 or configuration.count(fact) != 1:
        raise SystemExit(f'galileo check: selected host fact mismatch: {fact}')
def selected_fields(lines):
    fields = []
    for index, line in enumerate(lines):
        if line.startswith('    myConfig.modules.users.'):
            fields.append(line)
        elif line == '    myConfig.modules.kanata':
            fields.append(line)
            for child in lines[index + 1:]:
                if not child.startswith('      '):
                    break
                fields.append(child)
    return fields

if selected_fields(configuration) != selected_fields(source):
    raise SystemExit('galileo check: selected host option fields differ from the actual host import')
print('selected host configuration: three direct-enable facts and eight option/group lines identical')

with Path('bench/sample.tsv').open() as stream:
    expected = {row['id']: row['kind'] for row in csv.DictReader(stream, delimiter='\t')}
seen = set()
rules = 0
with Path('bench/galileo/units.tsv').open() as stream:
    for row in csv.DictReader(stream, delimiter='\t'):
        unit = row['id']
        if unit not in expected or row['kind'] != expected[unit]:
            raise SystemExit(f'galileo check: unknown or misclassified unit: {unit}')
        path = Path(row['file'])
        allowed_rule = Path('lib/firn/responsibilities.gal' if unit == 'rule.realization' else 'lib/firn/tags.gal')
        if row['kind'] == 'rule':
            if path != allowed_rule:
                raise SystemExit(f'galileo check: incorrect rule source: {unit}')
            rules += 1
        elif not str(path).startswith('bench/galileo/out/') or path == out / 'context.gal':
            raise SystemExit(f'galileo check: unit outside generated sample: {unit}')
        lines = path.read_text().splitlines()
        start, end = int(row['start_line']), int(row['end_line'])
        if not 1 <= start <= end <= len(lines):
            raise SystemExit(f'galileo check: invalid generated range: {unit}')
        seen.add(unit)
if seen != set(expected) or len(seen) != 120 or rules != 5:
    raise SystemExit('galileo check: expected all 120 units including five rules')
context = (out / 'context.gal').read_text().splitlines()
print(f'units: {len(seen)} ids, {rules} rule ranges; excluded context: '
      f'{len(context)} physical lines, {sum(bool(line.strip()) for line in context)} nonblank lines')
PY
SH
