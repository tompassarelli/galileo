# North menu transition reference

This corpus records the two tests named by PLAN box 8.5 at North revision
`eaee1838d1968b7caba65195866e5d52fd30b2e1`, using the recorder commit below.
The Clause compiler/runtime pin is `258e6956aa11dab2e724e89439e864e5e5dafdd6`
(MIT OR Apache-2.0). Only public Clause APIs were used; source fixtures remain
in their immutable pins and no Clause source is copied here.

Each JSONL sequence contains sessions, ordered dispatcher occurrences and
arguments, occurrence admissions, ordered effects, transaction begin/commit
boundaries, full canonical Clause projections, and continuation markers.
Projections are sampled only after all six appended maintenance handlers.
The readable `menu` field is a convenience view; `projection_hex` contains the
complete canonical Clause term. Raw entity identities in the reference are
not an equivalence key: the Muove comparison must establish the creation-order
correspondence required by SPEC §10.4.

`effects.ordered` contains host requests in occurrence order.
`effects.mailbox_at_boundary` records the final mailbox, including a request
retained from an earlier transaction. `evaluation.ordered_state_effects`
retains the selected Clause assignments in their evaluator order.
An admitted occurrence with `evaluation.rule_applied: false` is a skipped
handler, not a rejection.

Handler coverage is counted where North dispatches the occurrence chain,
including text-selected handlers and appended maintenance. Clause coverage
counts pinned compiled rule ordinals, both evaluations and selected matches.
Initialization uses the actual retained Clause trace. Other transactions
inspect each occurrence in a separate Clause boundary restored from the same
admitted checkpoint; the resulting canonical projection must equal the actual
North admission before observations and coverage are published. This inspection
preserves repeated occurrences whose traces Clause would otherwise overwrite.
Coverage is limited to these two sequences and does not claim whole-source
replay equivalence. Reports distinguish dispatch counts from inspection counts.

The second sequence records a real storage close/reopen and its pre-continuation
projection. Checkpoint markers identify admitted states from which a downstream
replay can take a checkpoint; they do not include serialized Clause source.

Run each test separately with `NORTH_TRANSITION_RECORD` set to its JSONL path,
inside the North recorder lane:

```sh
NORTH_TRANSITION_RECORD=/absolute/path/history.jsonl nix develop -c cargo test --offline --bin north history_search_navigation_and_cancel_keep_the_composer -- --nocapture
NORTH_TRANSITION_RECORD=/absolute/path/restart.jsonl nix develop -c cargo test --offline --bin north reopening_a_filtered_menu_restores_its_search_editor -- --nocapture
```

Wrap Cargo with the machine-capacity helper as required by the worker brief.
The recorder appends JSONL and accumulates its sibling coverage JSON by session;
choose a fresh output path when recording a new corpus.

Measured on 2026-10-10:

| Sequence | Sessions | Transactions | Occurrences/admissions | Handler names | Evaluated/selected compiled clauses |
| --- | ---: | ---: | ---: | ---: | ---: |
| history | 1 | 16 | 142 | 17 | 19 / 11 |
| restart | 2 | 5 | 43 | 17 | 17 / 8 |
| union | 3 | 21 | 185 | 23 | 25 / 15 |

The compiler produces 153 clauses in total. The corpus observes 213 clause
evaluation instances. All 21 admitted projections follow the maintenance
chain. The history sequence emits exactly one ordered host request,
`switch-conversation` with payload `gamma`. The restart sequence contains
exactly one restore marker. The two recorded tests pass in 9.02 s and 13.74 s.
North's full host check passes: 137 tests passed, 0 failed, 9 existing provider
or private-fixture tests ignored, in 185.25 s.

North recorder commit: `2daaad3a60a62d8e8479818c2a7d7b02aa483e58`.
It remains in `~/code/north/worktrees/muove-menu-recorder` on branch
`muove-menu-recorder`, unmerged and unpublished, for the downstream slice work.
The final recorder source also passes `nix develop -c cargo test --offline --no-run`.
