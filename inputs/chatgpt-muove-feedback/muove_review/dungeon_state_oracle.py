"""Independent dungeon-model audit. Python 3.10+, standard library only.

Reads the uploaded dungeon's base facts. Compares the example's reachability
rules with explicit (room, inventory) states under its documented ONE-WAY
passage semantics. This is not an execution of the Rust Muove implementation.
Assumes persistent, non-consumed keys picked up on entering their room,
no teleport/backtracking not declared as a door, and ignores trap damage
because the example has no health/death state.

Usage: python dungeon_state_oracle.py /path/to/muove/examples/dungeon.clause
"""
from __future__ import annotations
import json
import re
import sys
from collections import deque
from pathlib import Path


def read_base(path: Path) -> dict:
    text = path.read_text(encoding='utf-8')
    lines = text.splitlines()
    start = lines.index('level') + 1
    raw = []
    focus = None
    for line in lines[start:]:
        if line.strip() and not line.startswith(' ') and not line.startswith('#'):
            break
        s = line.strip()
        if not s or s.startswith('#') or '∈' in s:
            continue
        if line.startswith('    '):
            if focus is None:
                raise ValueError('Nested fact without a focus')
            raw.append(f'{focus} {s}')
        elif any(x in s for x in (' starts in ', ' lies in ')):
            raw.append(s)
        else:
            focus = s
    edges, unlocked, locked, keys = {}, set(), {}, {}
    initial = None
    for fact in raw:
        if m := re.fullmatch(r'(\S+) leads from (\S+) to (\S+)', fact):
            edges[m[1]] = (m[2], m[3])
        elif m := re.fullmatch(r'(\S+) is unlocked', fact):
            unlocked.add(m[1])
        elif m := re.fullmatch(r'(\S+) is locked by (\S+)', fact):
            locked[m[1]] = m[2]
        elif m := re.fullmatch(r'(\S+) lies in (\S+)', fact):
            keys[m[1]] = m[2]
        elif m := re.fullmatch(r'Ash starts in (\S+)', fact):
            initial = m[1]
    if initial is None or len(edges) != 7:
        raise ValueError('Unexpected dungeon base; inspect the parser assumptions')
    return {'edges': edges, 'unlocked': unlocked, 'locked': locked,
            'keys': keys, 'initial': initial}


def declarative_rooms(d: dict) -> set[str]:
    """Direct reading of start, pick-up, walk, and unlock in the source."""
    reached, held = {d['initial']}, set()
    while True:
        old = (reached.copy(), held.copy())
        held.update(k for k, room in d['keys'].items() if room in reached)
        reached.update(dst for door, (src, dst) in d['edges'].items()
                       if src in old[0] and
                       (door in d['unlocked'] or d['locked'].get(door) in held))
        if old == (reached, held):
            return reached


def state_paths(d: dict) -> dict[str, list[str]]:
    def collect(room: str, held: frozenset[str]) -> frozenset[str]:
        return held | frozenset(k for k, loc in d['keys'].items() if loc == room)
    state = (d['initial'], collect(d['initial'], frozenset()))
    seen = {state}
    queue = deque([(state, [d['initial']])])
    paths = {}
    while queue:
        (room, held), path = queue.popleft()
        paths.setdefault(room, path)
        for door, (src, dst) in d['edges'].items():
            if src != room or not (door in d['unlocked'] or d['locked'].get(door) in held):
                continue
            nxt = (dst, collect(dst, held))
            if nxt not in seen:
                seen.add(nxt)
                queue.append((nxt, path + [f'--{door}-->', dst]))
    return paths


def run(path: Path) -> dict:
    base = read_base(path)
    def variant(remove_key=None, collapse=False, ladder=False):
        d = {k: v.copy() if isinstance(v, (set, dict)) else v for k, v in base.items()}
        if remove_key:
            d['keys'].pop(remove_key)
        if collapse:
            d['edges'].pop('Armory-Gate')
        if ladder:
            d['edges']['Library-Ladder'] = ('Library', 'Armory')
            d['unlocked'].add('Library-Ladder')
        p = state_paths(d)
        return {'declarative_reachable': sorted(declarative_rooms(d)),
                'state_reachable': sorted(p), 'vault_state_path': p.get('Vault')}
    results = {
        'base': variant(),
        'without_iron_key': variant(remove_key='Iron-Key'),
        'armory_collapsed': variant(collapse=True),
        'ladder_added_without_iron_key': variant(remove_key='Iron-Key', ladder=True),
    }
    assert 'Armory' in results['base']['declarative_reachable']
    assert 'Armory' not in results['base']['state_reachable']
    assert 'Vault' in results['without_iron_key']['declarative_reachable']
    assert 'Vault' not in results['without_iron_key']['state_reachable']
    assert results['base']['state_reachable'] == results['armory_collapsed']['state_reachable']
    assert 'Vault' in results['ladder_added_without_iron_key']['state_reachable']
    placements = {'declarative': [], 'state': []}
    for room in ['Entrance', 'Hall', 'Library', 'Armory', 'Crypt', 'Vault', 'Shrine']:
        d = {k: v.copy() if isinstance(v, (set, dict)) else v for k, v in base.items()}
        d['keys']['Silver-Key'] = room
        if 'Shrine' in declarative_rooms(d): placements['declarative'].append(room)
        if 'Shrine' in state_paths(d): placements['state'].append(room)
    results['silver_key_placements_that_unlock_shrine'] = placements
    return results

if __name__ == '__main__':
    if len(sys.argv) != 2:
        raise SystemExit('Usage: python dungeon_state_oracle.py /path/to/examples/dungeon.clause')
    print(json.dumps(run(Path(sys.argv[1])), indent=2))
