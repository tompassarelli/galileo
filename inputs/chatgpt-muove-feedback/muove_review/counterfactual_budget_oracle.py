"""Ground, constant-unary model of Muove's current bounded evaluation loop.
Not the Rust implementation. Mirrors candidate scans and discards the current
round's fresh facts on exhaustion, as src/engine.rs does in the uploaded file.
Run: python counterfactual_budget_oracle.py
"""
import json

RULES = []
for i in range(1, 6):
    RULES += [(f'C{i}', 'Gate'), (f'C{i}', 'Seed' if i == 1 else f'C{i-1}')]
RULES += [('Goal', 'C5')]


def close(blocked=False, budget=1_000_000):
    facts = {'Seed', 'Gate'} - ({'Gate'} if blocked else set())
    work = 0
    rounds = 0
    while True:
        rounds += 1
        fresh = set()
        for head, body in RULES:
            for fact in sorted(facts):
                work += 1
                if work > budget:
                    return {'facts': sorted(facts), 'complete': False, 'work': work, 'rounds': rounds}
                if fact == body and head not in facts:
                    fresh.add(head)
        if not fresh:
            return {'facts': sorted(facts), 'complete': True, 'work': work, 'rounds': rounds}
        facts |= fresh


def run():
    results = {
        'base_budget187': close(False, 187),
        'blocked_budget187': close(True, 187),
        'blocked_complete': close(True),
    }
    assert results['base_budget187']['complete']
    assert not results['blocked_budget187']['complete']
    assert 'Goal' not in results['blocked_budget187']['facts']
    assert results['blocked_complete']['complete']
    assert 'Goal' in results['blocked_complete']['facts']
    return results

if __name__ == '__main__':
    print(json.dumps(run(), indent=2))
