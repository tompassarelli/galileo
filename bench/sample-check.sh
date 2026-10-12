#!/usr/bin/env bash
set -euo pipefail

if (( $# > 1 )); then
  printf 'usage: FIRN_ROOT=<pin> %s [sample.tsv]\n' "$0" >&2
  exit 2
fi
: "${FIRN_ROOT:?Set FIRN_ROOT to the firn pin 10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9}"
pin=10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9
if [[ $(git -C "$FIRN_ROOT" rev-parse HEAD) != "$pin" ]]; then
  printf 'sample-check: FIRN_ROOT must be at %s\n' "$pin" >&2
  exit 2
fi
sample=${1:-$(dirname "${BASH_SOURCE[0]}")/sample.tsv}
PYTHONDONTWRITEBYTECODE=1 python3 - "$FIRN_ROOT" "$sample" <<'PY'
import csv
import sys
from collections import Counter, defaultdict
from pathlib import Path

root, sample = map(Path, sys.argv[1:])
scopes = {}
for module in ("firefox", "zen-browser", "kanata", "users"):
    for name in ("default", module):
        scopes[f"modules/{module}/{name}.bnix"] = None
scopes["modules/lane-gc/default.bnix"] = None
for module in ("atuin", "chrome", "godot", "bun", "obsidian"):
    scopes[f"native/nix/{module}.clause"] = None
    scopes[f"modules/{module}/tags.clause"] = None
for host in ("ashashi", "whiterabbit"):
    scopes[f"hosts/{host}/enabled-tags.bnix"] = None
scopes["hosts/whiterabbit/configuration.bnix"] = [(7, 10), (30, 38), (63, 63)]
scopes["hosts/whiterabbit/responsibilities.bnix"] = [(n, n) for n in (26, 28, 38, 40, 42)]
context_path = "config/darwin-modules.json"
lines = {}
allowed = {}
try:
    for path, ranges in [*scopes.items(), (context_path, None)]:
        lines[path] = (root / path).read_text().splitlines()
        ranges = ranges if ranges is not None else [(1, len(lines[path]))]
        allowed[path] = {n for start, end in ranges for n in range(start, end + 1)}
        if any(n > len(lines[path]) for n in allowed[path]):
            raise ValueError(f"pinned range exceeds {path}")
    counts = Counter()
    kinds = defaultdict(lambda: [0, 0])
    ids = set()
    duplicate_ids = []
    rule_ids = {
        "rule.tag-resolution", "rule.platform-gate", "rule.tag-override",
        "rule.realization", "rule.flake-input",
    }
    seen_rules = set()
    known_kinds = {
        "syntax", "tag", "opt-in", "override", "input", "option", "import",
        "implementation", "host-syntax", "host-platform", "host-tag",
        "host-disabled", "host-option", "responsibility", "context", "rule",
    }
    with sample.open(newline="") as stream:
        reader = csv.reader(stream, delimiter="\t")
        if next(reader, None) != ["id", "kind", "pin_file", "start_line", "end_line"]:
            raise ValueError("expected TSV header: id, kind, pin_file, start_line, end_line")
        for row_number, row in enumerate(reader, 2):
            if len(row) != 5:
                raise ValueError(f"row {row_number}: expected five fields")
            unit, kind, path, start, end = row
            if not unit:
                raise ValueError(f"row {row_number}: empty id")
            if unit in ids:
                duplicate_ids.append(unit)
            ids.add(unit)
            if kind not in known_kinds:
                raise ValueError(f"row {row_number}: unknown kind {kind!r}")
            if kind == "rule":
                if unit not in rule_ids or (path, start, end) != ("-", "0", "0"):
                    raise ValueError(f"row {row_number}: expected a named rule unit with file '-' and lines 0 0")
                seen_rules.add(unit)
                kinds[kind][0] += 1
                continue
            if path not in allowed:
                raise ValueError(f"row {row_number}: path outside the sample and context")
            if (kind == "context") != (path == context_path):
                raise ValueError(f"row {row_number}: context kind must identify the Darwin list")
            start, end = int(start), int(end)
            if start < 1 or end < start or end > len(lines[path]):
                raise ValueError(f"row {row_number}: invalid line range")
            selected = set(range(start, end + 1))
            if not selected <= allowed[path]:
                raise ValueError(f"row {row_number}: range includes excluded host lines")
            nonblank = [n for n in selected if lines[path][n - 1].strip()]
            kinds[kind][0] += 1
            kinds[kind][1] += len(nonblank)
            counts.update((path, n) for n in nonblank)
    missing_rules = rule_ids - seen_rules
    if missing_rules:
        raise ValueError(f"missing rule units: {', '.join(sorted(missing_rules))}")
except (OSError, ValueError, csv.Error) as error:
    print(f"sample-check: {error}", file=sys.stderr)
    sys.exit(2)

universe = {(path, n) for path in scopes for n in allowed[path] if lines[path][n - 1].strip()}
context = {(context_path, n) for n in allowed[context_path] if lines[context_path][n - 1].strip()}
unassigned = sum(counts[key] == 0 for key in universe)
doubled = sum(counts[key] > 1 for key in universe)
print(f"unassigned: {unassigned}")
print(f"doubly assigned: {doubled}")
print("kind\trows\tnon-blank lines")
for kind, (rows, nonblank) in sorted(kinds.items()):
    print(f"{kind}\t{rows}\t{nonblank}")
print(f"sample non-blank lines: {len(universe)}")
print(f"context non-blank lines: {len(context)}")
context_unassigned = sum(counts[key] == 0 for key in context)
context_doubled = sum(counts[key] > 1 for key in context)
print(f"context unassigned: {context_unassigned}")
print(f"context doubly assigned: {context_doubled}")
for path, ranges in scopes.items():
    if ranges is not None:
        excluded = sum(bool(line.strip()) and n not in allowed[path] for n, line in enumerate(lines[path], 1))
        print(f"excluded non-blank lines: {path}\t{excluded}")
for unit in duplicate_ids:
    print(f"sample-check: duplicate id {unit!r}", file=sys.stderr)
sys.exit(1 if unassigned or doubled or context_unassigned or context_doubled or duplicate_ids else 0)
PY
