# Muove

Muove is a rule language whose rules determine, explain and constrain
changes. It replaces Beagle and Clause in firn (nixos-config) first, then in
north.

## Routers

- `SPEC.md`: the MVP semantics, syntax, Nix and runtime targets, and the
  acceptance scenarios A1–A11.
- `PLAN.md`: the work packages WP1–WP8 and their boxes, each with a measured
  Done when.
- `DECISIONS.md`: the decisions taken, including Tom's T1–T3, and the
  review record.
- `inputs/`: the first-draft spec, the Claude prototype and the ChatGPT
  review, kept as source. Port from them; do not edit them.

## Rules

- Read nixos-config and north fixtures in place through `FIRN_ROOT` and
  `NORTH_ROOT`, the pins PLAN.md names. Never copy them into this
  repository (D-19).
- Work in `~/code/muove/worktrees/<slug>` and land with
  `git -C ~/code/muove/main merge --ff-only <branch>`. There is no remote;
  Tom decides whether one is created.
- Build through `nix develop -c cargo …` and keep `target/` inside the lane.
- Verdicts are `Holds`, `Fails` or `Unknown`, and `Unknown` never approves.
  The reference interpreter is the oracle for the engine; never test the
  engine against itself.
- The core crates take no dependencies. A dependency must replace an
  external format or protocol implementation (D-20).
- Every DECISIONS.md entry, design choice and blocker decision is made by
  an Opus max planner (Agent `planner`, effort max). Leads and workers
  record the question and wait; they never decide it themselves.
