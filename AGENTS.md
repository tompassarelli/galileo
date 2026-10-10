# Muove

Muove is meant to be a general-purpose language whose rules determine,
explain and constrain changes (DECISIONS T4). Its MVP replaces Beagle and
Clause in firn (nixos-config) first, then in north.

## Routers

- `SPEC.md`: the MVP semantics, syntax, Nix and runtime targets, and the
  acceptance scenarios A1–A11.
- `PLAN.md`: the work packages WP1–WP8 and their boxes, each with a measured
  Done when.
- `DECISIONS.md`: the decisions taken, including Tom's T1–T4, and the
  review record.
- `notes/language.md`: the language owner's notebook: how rulings are
  made, where each language ruling lives, contradictions, open questions
  and who writes Muove.
- `inputs/`: the first-draft spec, the Claude prototype and the ChatGPT
  review, kept as source. Port from them; do not edit them.

## Rules

- Read nixos-config and north fixtures in place through `FIRN_ROOT` and
  `NORTH_ROOT`, the pins PLAN.md names. Never copy them into this
  repository (D-19).
- Work in `~/code/muove/worktrees/<slug>` and land with
  `git -C ~/code/muove/main merge --ff-only <branch>`, then publish with
  `safe-push --to main` from main. The remote is private:
  github.com/tompassarelli/muove.
- Build through `nix develop -c cargo …` and keep `target/` inside the lane.
- Verdicts are `Holds`, `Fails` or `Unknown`, and `Unknown` never approves.
  The reference interpreter is the oracle for the engine; never test the
  engine against itself.
- The core crates take no dependencies. A dependency must replace an
  external format or protocol implementation (D-20). Only `muove-cli` and
  `muove-lsp` touch the operating system; every other crate takes and
  returns data (SPEC §10.4).
- Every DECISIONS.md entry, design choice and blocker decision is made by
  an Opus xhigh planner (Agent `planner`, effort xhigh; max only when Tom
  asks for it by name); leads and workers never make one. A question about Muove's syntax or semantics goes to the
  one language owner, which the lead keeps between batches and restarts
  from `notes/language.md` (D-36). Other questions go to one-shot
  planners. A language ruling binds once SPEC states it.
- A worker writes each open question as a `DECISION <box>:` line in the
  peer file, takes the narrowest reading SPEC allows, says so in that
  line, and lands. The box stays unticked only when its Done when depends
  on the answer.
- Codex Sol workers implement most boxes in every package: high, or medium
  for mechanical boxes. Claude Opus medium workers take only critical-path
  or quality-critical boxes. A Claude box lands when its checks pass, then
  a Codex reviewer tries to break the landed commit. A Codex box lands only
  after Claude's review.
- Claude Opus owns delivery quality and lands every box, including Codex's.
  Its review checks the box against SPEC's intent and cuts formal
  machinery, proof obligations and provenance that SPEC does not ask for.
