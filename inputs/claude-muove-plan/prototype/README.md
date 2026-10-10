# Muove

*Eppur si muove.* A language for people who read meaning, not people who type code.

You write facts, rules and **must-sentences** in the words of your domain.
Muove checks every edit, whether yours or an AI's, against the
must-sentences, and explains everything in the same sentences you wrote.
Comments can't drift, because the English *is* what gets checked.

```text
level must:
  Ash can reach Vault
  Ash can reach Vault requires Ash can reach Library
  Ash can reach Vault has no single point of failure among ?key lies in ?room
  ?trap hurts ?hero requires ?trap is telegraphed
  never Ash can reach Shrine
```

An AI proposes a change, and Muove reviews it in English:

```text
$ muove review examples/dungeon.clause examples/proposals/pitfall.clause

▸ review level -> level/pitfall

  The proposal says: "Makes the Crypt scarier with a pit trap."
  That's the author's claim, and nothing checks it. Here is what the change actually does.

  It changes:
    + Pit-Trap ∈ Trap
    + Pit-Trap is armed in Crypt
    + rule level/pit-drop: ?trap hurts ?hero when ?trap is armed in ?room and ?hero can reach ?room

  Consequences:
    + newly true: Pit-Trap hurts Ash
    ~ changed: Spike-Trap hurts Ash   (no longer needs Spike-Trap is telegraphed)

  Must-sentences:
    ✓ Ash can reach Vault
    ✓ Ash can reach Vault requires Ash can reach Library
    ✓ Ash can reach Vault has no single point of failure among ?key lies in ?room
    ✗ ?trap hurts ?hero requires ?trap is telegraphed   ← broken by this change
        Pit-Trap hurts Ash holds without Pit-Trap is telegraphed:
          Pit-Trap hurts Ash   ← level/pit-drop
          ├─ Pit-Trap is armed in Crypt
          └─ Ash can reach Crypt   ← level/walk
             ...
        The same goes for: Spike-Trap hurts Ash
    ✓ never Ash can reach Shrine

  Verdict: reject. It breaks 1 of 5 must-sentences.
```

## Run it

```sh
cargo run -- examples/dungeon.clause                 # every question, then the must-sentences
cargo run -- check examples/dungeon.clause           # exit 1 if a must-sentence breaks (for CI)
cargo run -- review examples/dungeon.clause examples/proposals/*.clause
cargo test
```

The five proposals in `examples/proposals/` are written as an AI assistant
would write them, each with a `says:` line describing itself:

| Proposal | What it says | Verdict |
| --- | --- | --- |
| `secret-passage` | "A hidden shortcut. The main route is unchanged." | Reject: the Vault no longer requires the Library |
| `armory-collapse` | "Players can still finish the level." | Reject: true, but the Iron Key becomes a single point of failure |
| `pitfall` | "Makes the Crypt scarier with a pit trap." | Reject: traps now hit without warning, including the old one |
| `crowbar` | "Force locked doors with a crowbar." | Reject: skips the Library and opens the sealed Shrine |
| `library-ladder` | "A second way into the Armory." | Approve |

## Must-sentences

| Form | Checked meaning |
| --- | --- |
| `<sentence>` | It must hold (with `?variables`: at least one match) |
| `never <sentence>` | It must not hold |
| `<sentence> requires <sentence>` | Every way the first holds goes through the second |
| `<sentence> has no single point of failure among <pattern>` | No single stated fact matching the pattern stops it on its own |

The last two talk about *reasons*, which is what other languages can't check.

## Questions

Every question is written in sentences and answered in sentences.

| Question | Answers |
| --- | --- |
| `find all ?x in W:` | What matches |
| `why all in W:` | Every minimal set of stated facts it follows from, as proof trees |
| `why not in W:` | How close each rule gets, and exactly what's missing |
| `prevent all minimal in W:` + `using:` | The minimal withdrawals that make it false |
| `achieve all minimal in W:` + `using:` | The minimal additions that make it true |
| `diff A -> B` | What changed, including conclusions that lost or gained reasons |
| `review A -> B` | The change, its consequences, and every must-sentence: approve or reject |

`why not` is what an in-game character could honestly say:

```text
  level/unlock needs:
    ✓ Ash can reach Crypt
    ✓ Shrine-Door leads from Crypt to Shrine
    ✓ Shrine-Door is locked by Silver-Key
    ✗ Ash holds Silver-Key
        level/pick-up needs:
          ✗ Silver-Key lies in ?room   (nothing states this)
          · Ash can reach ?room
```

## The language in one screen

An illustrative sketch; `examples/` has complete, runnable files.

```text
Room                                   # a line on its own declares a type
Door

level/leads: RelationShape             # a relation and its sentence shape
  {door: Door} leads from {here: Room} to {there: Room}

level                                  # a model: names and stated facts
  Hall ∈ Room
  [Door 101..104] ∈ Door               # ranges
  Front-Door                           # focus: the subject starts each line
    leads from Entrance to Hall
    is unlocked

level/walk: DerivationRule             # a rule: conclusion, then conditions
  ?hero can reach ?there
  when:
    ?hero can reach ?here
    ?door leads from ?here to ?there
    ?door is unlocked

level must:                            # what every edit is checked against
  Ash can reach Vault

level/edit: Revision                   # a proposed change
  from: level
  says: "What the author claims it does"
  withdraw:
    Door 101 leads from Hall to Crypt
  add rule level/climb:
    ?hero can reach ?there
    when:
      ?hero can reach ?here
      ?here is below ?there
  remove rule level/walk
```

Sentences are read against every declared shape, and names may contain
spaces. A sentence must have exactly one reading that type-checks. When it
doesn't, the error says why, with "did you mean" for misspelled names and the
closest shapes for unrecognised sentences.

## Honest answers

Every search is bounded (`--max-reasons`, `--max-work`, `--max-facts`,
`--max-candidates`). When a bound is hit, the answer says **Incomplete**.
Every `prevent` and `achieve` answer is checked by applying it before it is
reported.

## How it's checked

- `tests/oracle.rs`: the original Clause oracles (hospital, impact, typed
  achievement) still pass on the original files. Every `prevent` and `achieve`
  answer in the examples matches a brute-force subset search, and every `why`
  reason is checked to suffice and to be minimal.
- `tests/ten.rs`: all five proposals get the right verdict for the right
  reason; why-not, rule removal, blocking, and the compiler's own error
  messages are tested too.
- `lean/MuoveProofs.lean` proves the characterisations the engine relies on:
  prevent = hitting every reason, achieve = reasons in the world with every
  candidate added, diff's lost and kept reasons, and the two reason-level
  must-sentences. **Not yet machine-checked:** run `lake build` in `lean/`.

## What's missing

- **Negation, numbers and time**, in the rules themselves. These are the next
  design problems, and each one complicates the answers. Negation breaks
  monotonicity, the property the Lean file shows everything rests on.
- **Several phrasings for one relation**, plurals and "and" lists.
- **The compiler's own checks as Muove rules**, so a rejected sentence can be
  asked `why`.
- **Speed.** Evaluation is a simple fixpoint loop, and reasons are worked out
  exhaustively. That's fine for designer tooling, not yet for a frame loop.
  The plan is to explain on demand from snapshots.
- **Embedding:** a C ABI for hosts such as Odin, and Wasm for a browser playground.

This is the minimal core extracted from the pre-reset Clause prototype. It
still reads the original Clause surface: `hospital.clause`, `impact.clause` and
`catalog.clause` are byte-for-byte the original files.
