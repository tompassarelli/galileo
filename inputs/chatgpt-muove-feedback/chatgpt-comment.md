**Muove should evolve toward a language where the same executable rules determine behavior, explain behavior, and constrain changes to behavior.** That is stronger than an English-looking programming language, and stronger than a specification layer sitting beside ordinary code.

The deeper inspection changes two parts of my earlier assessment. I found a modeling flaw in the dungeon example that my first review missed. And I would revise my recommendation that Muove could primarily be something humans approve while AI writes a separate implementation. Without a reliable connection between those two, you recreate the drift you are trying to eliminate.

I inspected the Rust implementation, tests, examples, and Lean sketch, and ran independent Python checks of two specific problems. **I could not run the Rust tests or Lean proofs because this environment lacks those toolchains.** Source excerpts and reproduction details are in the AUDIT.md[audit notes](sandbox:/mnt/data/muove_review/AUDIT.md).

## 1. The most important discovery: the dungeon has a state-modeling problem

The dungeon explicitly describes its doors as **one-way passages**.

The Bronze Key is in the Library. The gate into the Armory starts in the Hall and requires that key. But there is no passage back from the Library to the Hall.

Muove nevertheless derives the Armory as reachable because it combines:

```text
Ash can reach Hall
Ash holds Bronze-Key
Armory-Gate leads from Hall to Armory
Armory-Gate is locked by Bronze-Key
```

The problem is that those first two facts do not describe a compatible player state. Ash can reach the Hall *before* collecting the Bronze Key. Once Ash collects the key, Ash cannot return there under the declared movement rules.

The model combines “there exists a way to have reached this room” with “there exists a way to acquire this key,” as though they necessarily belong to one executable journey.

I checked this with an independent search over explicit `(room, inventory)` states, assuming persistent keys, movement only through declared passages, and no unstated teleportation. It found:

**The Armory is not reachable in the original dungeon. Removing the Iron Key makes the Vault unreachable, even though the declarative rules still derive it as reachable.** Adding the Library-to-Armory ladder creates a genuinely usable Bronze-Key route. It does not merely add another entrance to an already accessible room. dungeon_state_oracle_results.json[State-search results](sandbox:/mnt/data/muove_review/dungeon_state_oracle_results.json)

This is not an unsound inference by the rule engine. The engine is correctly evaluating an abstraction that is too permissive for the example’s stated gameplay.

But it matters enormously to your thesis:

> A sentence can have exactly one well-typed reading, and still fail to describe what its human reader thinks it describes.

My first review treated the example’s two proofs as two playable routes. That was wrong.

### What this means for the next version

I would put **explicit state and executable witnesses** ahead of unrestricted negation, more English phrasings, or compiler self-hosting.

A proof of “Ash can reach Vault” should, for this domain, yield a sequence of legal moves that a tiny game can actually execute.

Likewise, this requirement:

```text
?trap hurts ?hero requires ?trap is telegraphed
```

currently establishes a dependency on a static telegraph fact. It does not establish that the warning happened **before** the damage, or that the player had time to respond.

The next major step is not merely adding a `time` type. It is distinguishing a static conclusion, an event, and a legal sequence of state transitions.

That would make the project meaningfully stronger, not just more expressive.

## 2. Separate three things that currently sit under “reasons”

The current implementation’s minimal-support representation is a good foundation. For a fixed positive program, it records the smallest sets of stated facts sufficient to derive a conclusion.

Mathematically:

\[
\operatorname{Supports}(q)
=
\min_{\subseteq}\{S\subseteq B \mid q\in\operatorname{closure}_P(S)\}.
\]

That representation supports a coherent family of operations: alternative explanations, necessary inputs, minimal withdrawals, and minimal additions. This fits established provenance semantics, where different representations retain different amounts of information about how an answer follows. The choice of representation determines which questions remain answerable. [arXiv](https://arxiv.org/html/2412.07986v1)

The important issue is that **minimal fact support is not the same thing as derivation structure**.

Consider this schematic program:

```text
Gate follows from Seed.
Goal follows from Gate.
```

The sole minimal stated-fact support for `Goal` is `{Seed}`.

Now add:

```text
Goal follows directly from Seed.
```

The minimal stated-fact support is still `{Seed}`. However:

```text
Goal requires Gate
```

has become false.

Your implementation reflects that distinction already. With a complete evaluation, `requires` catches the bypass by blocking `Gate`. But the fact-support comparison in `diff.changed` does not record a change to `Goal`, because its support sets are identical. See `src/ask.rs:438–523` and `815–905`.

**This is an architectural distinction worth making explicit now.**

Muove needs three separately understandable views:

| View | Question |
|---|---|
| Truth | Does the conclusion hold? |
| Fact support | Which stated inputs suffice to establish it? |
| Derivation dependence | Which semantic checkpoints must every derivation pass through? |

I would preserve all three, rather than forcing one representation to pretend to answer every question.

For derivation dependence, expose stable domain checkpoints such as “payment authorized” or “tutorial completed,” not every incidental internal rule name. Otherwise, harmless refactoring will create noisy review changes.

### “More reasons” is not a general measure of strength

There is another distinction to tighten. The renderer calls minimal supports “independent reasons,” and review sometimes assesses weakening by counting them.

Consider:

```text
Before: {A}, {B}

After:  {A, C}, {B, C}, {D, C}
```

The number of minimal supports increases from two to three. Yet the conclusion acquires a single point of failure: `C`.

Before, two input withdrawals were necessary to stop it. After, one is enough.

Conversely, introducing a simpler sufficient condition can cause several older supports to cease being minimal. The support count falls even though the conclusion becomes easier to preserve under input loss.

I would change the default language to **“distinct minimal supports”**, and report specific properties separately:

> Three supports remain. Every one now depends on C. The smallest permitted failure set has fallen from two facts to one.

That says something operationally meaningful.

Later, add **failure groups**. Two different key placements in the same collapsible room are separate facts but not independent against a room-collapse event. Independence needs a declared failure model, not just different fact identities.

## 3. Make approval trustworthy before expanding the language

The earlier budget and approval concerns are present in the source. I also found additional issues at the boundary between analysis and what the tool tells the user.

These are the highest-priority implementation findings:

| Finding | Why it matters |
|---|---|
| `check` can succeed despite incomplete analysis. | `intents_report` uses `holds` without considering completeness. An unfinished search for a forbidden conclusion can look like success. |
| `review` can approve an already-invalid result. | `broken` counts only requirements that transition from holding to not holding. An existing violation can remain while the renderer says every requirement holds. |
| Counterfactual checks discard their own completion status. | The original closure can finish while the “without this prerequisite” computation does not. |
| Proposal text can alter the baseline before comparison. | `review` concatenates both sources; top-level rules in the proposal are installed into the base model during parsing. |
| Proof construction can label an unproved fact as stated. | `build_proof` falls back to `Proof::Stated` when it cannot find a rank or suitable firing, even when that fact is absent from the support. |
| A derived fact becoming explicitly stated can be called “no longer true.” | The diff compares derived-only sets, confusing a change of origin with a loss of truth. |

The exact code locations and excerpts are in the AUDIT.md[source audit](sandbox:/mnt/data/muove_review/AUDIT.md).

The counterfactual-budget problem is not just hypothetical. I constructed a small program with a fast route through `Gate` and a slower route that bypasses it. A model of the current evaluation loop completes the original closure in 187 steps. Blocking `Gate` exhausts that same budget before discovering `Goal`, although a complete search does find it.

Therefore, “the original evaluation completed” cannot justify a conclusion about the counterfactual evaluation.

### Replace boolean compliance with evidence-bearing verdicts

I would make the core result conceptually:

```text
Proved(evidence)
Disproved(counterexample)
Unknown(reason)
```

Not a boolean that callers have to remember to interpret alongside a status field.

The evidence requirements differ by question. Finding a valid derivation can establish a positive claim before the entire closure finishes. Failing to find a forbidden fact cannot establish its absence before the relevant search completes. A universal dependency claim needs enough coverage to justify its “every.”

Initially, it is perfectly reasonable to be conservative and withhold approval whenever a necessary analysis is incomplete. Later, you can make completeness more local and precise.

Also separate:

> This revision introduces no additional violations.

from:

> This revision satisfies the requirements.

Both can be useful. They are not interchangeable verdicts.

### Freeze what the proposal is being judged against

Parse the baseline into an immutable program and requirement set. Then parse the proposal as a restricted patch against that baseline.

A proposal should not be able to install a rule into both sides of its own comparison simply by putting it outside a revision block.

Requirements should be equally explicit. An implementation change and a requirement change are different review actions. Muove can support both, but the latter must not silently redefine success for the former.

These fixes are not peripheral hardening. **Approval is the feature whose correctness everything else depends on.**

## 4. Muove should execute its first real domain, not merely describe it

This is where I would revise my earlier advice most strongly.

Suppose an AI writes a Rust implementation and a Muove model of that implementation. Muove approves the model. The Rust code does something slightly different.

You are back to having a beautifully maintained explanation that is not necessarily what runs.

The strongest first architecture is therefore:

**The domain behavior lives in Muove. The host calls it. Review analyzes that same behavior.**

For a small dungeon, the host can handle rendering, input, and presentation. Muove owns movement eligibility, inventory-dependent transitions, progression gates, and the relevant event rules.

That does not require Muove to become a language for graphics drivers or networking. It requires it to be a real executable language for the behavior it claims to explain.

### State does not force you to abandon the positive core immediately

The game’s current location changes, but the analysis can monotonically accumulate **reachable states**.

Instead of combining:

```text
Ash can reach Hall
Ash holds Bronze-Key
```

from potentially incompatible situations, the model can reason over a state containing both location and inventory.

Discovering another reachable state adds information. It does not retract previously discovered states.

A finite-state prototype can therefore address the dungeon flaw while retaining positive reachability reasoning. The cost is potentially many more states, so the analyzed domain and bounds must be visible.

You should distinguish at least these scopes in output:

> Checked in this snapshot.

> Checked along these observed or bounded traces.

> Checked across all reachable states of this finite model.

None of those means “proved for every possible input to an arbitrary program.”

### Make the proof do something

The most convincing demo would let the user ask:

> Why can Ash reach the Vault?

Then click the answer and watch the witness route execute.

When a proposal changes the answer, the before-and-after witnesses should execute too.

That gives you a concrete test of the explanation. The displayed proof is not just a convincing tree; it corresponds to behavior the system can reproduce.

For a later compiled backend, retain a simple interpreter as the reference semantics and compare generated behavior against it. Host-provided facts and foreign functions should remain visible trust boundaries rather than disappearing into an apparently self-contained proof.

## 5. The next high-value language feature is a checked change contract

The `says:` feature makes a useful distinction between the author’s claim and the actual consequences.

The next evolution is to let part of that claim become checkable.

For example, a proposal could declare these intentions, in future syntax:

> The Attic becomes reachable.

> Existing progression requirements remain satisfied.

> Reachability outside the Attic is unchanged.

Now review has three jobs: establish that the requested change happened, establish that protected properties survived, and report consequences outside the declared scope.

That is more valuable than merely showing a larger semantic diff.

It also solves a subtle problem with checking only standing requirements: a proposal can satisfy every requirement while accomplishing none of its stated purpose.

### Separate diagnostic counterfactuals from acceptable repairs

The current `achieve` operation answers:

> Which additions make this target true?

That is useful as a diagnostic question. It is not yet:

> Which acceptable additions achieve this target without breaking anything protected?

The dungeon makes the distinction obvious. A query can ask how to make the Shrine reachable even though a standing requirement says it must never be reachable. The raw counterfactual should still be allowed to explain the possibilities. A repair assistant should not present those possibilities as compliant changes.

I would introduce two clearly distinct modes:

**Explain possibility:** explore the permitted intervention vocabulary without pretending every result is acceptable.

**Find an acceptable change:** achieve the requested goal while preserving the frozen requirements and respecting change permissions.

The second mode needs more than filtering on a friendly description. Each proposed change needs to be checked against the complete contract.

And the theory needs attention: a positive target can become true monotonically while a `never` requirement is broken by those same additions. The combined repair problem is not simply the existing positive achievement problem under another name.

### Let Muove probe the requirements themselves

Another powerful development is targeted mutation testing.

Generate changes such as bypassing a checkpoint, removing a warning condition, introducing a shared dependency, or making an obligation vacuous. Then ask whether the requirements notice.

The useful report is:

> Removing this condition changes these behaviors, but no existing must-sentence rejects the change.

That does not prove the specification is incomplete in every meaningful sense. It exposes a concrete blind spot.

There is a useful precedent in lineage-driven fault injection: the MOLLY work uses information about successful outcomes to guide the failures it explores, rather than relying only on random fault injection. That is a productive connection for Muove’s counterfactual machinery. [ACM Digital Library](https://dl.acm.org/doi/10.1145/2723372.2723711)

The product opportunity is not only “check my specification.” It is also **“show me what my specification currently allows.”**

## 6. Scale by separating truth from explanations

The current implementation repeatedly computes closures, reasons, and proof reconstructions across queries and requirement checks. Its simplicity is valuable while establishing semantics. It will become expensive as examples grow.

The first performance step should be **reuse and demand-driven analysis**, not a complicated backend.

Cache evaluation for immutable worlds. Reuse the same closure across requirements. Calculate detailed provenance for the targets and checkpoints being reviewed. Construct one proof when the user asks for one, rather than treating that request as a reason to enumerate every support everywhere.

Soufflé provides a relevant architectural example: its provenance system retains annotations during evaluation but builds proof trees lazily when queried. That does not supply all of Muove’s semantics, but it demonstrates the usefulness of separating evaluation from explanation construction. [Soufflé](https://souffle-lang.github.io/provenance)

### Exhaustive supports have an unavoidable bad case

Imagine twenty independent stages, each of which can be satisfied by either of two facts. The final goal needs every stage.

That compact structure has:

\[
2^{20}=1{,}048{,}576
\]

minimal supports.

A faster set implementation does not remove the size of the answer.

I would keep a compact representation of alternative and joint derivations, then enumerate minimal supports only when needed. Some queries can be answered without producing the entire family.

Also move resource checks inside expensive expansion loops. In the current reason computation, work limits are checked after batches of combinations, and the per-conclusion support cap is applied after constructing candidates. A nominal bound is not a strict resource bound when a large intermediate can be built first. See `src/engine.rs:264–343`.

### Preserve a genuinely independent oracle

The brute-force tests are valuable, but they call the same `engine::close` used by production.

That checks whether the provenance and intervention algorithms agree with the evaluator. It does not independently establish that the evaluator is correct, and it cannot establish that the dungeon models actual play correctly.

Maintain a deliberately simple second evaluator for small randomized programs. Separately maintain domain-level checks, like the explicit player-state search.

The Lean sketch is another useful layer, but its own comments correctly distinguish abstract characterizations from the Rust implementation. Machine-check those proofs, make their assumptions explicit in the language specification, and avoid presenting them as verification of code they do not cover.

## 7. Expand expressiveness without blurring the guarantees

I would not permanently ban negation or decide that Muove must remain a tiny positive language forever.

Negation is a real design problem, not a dead end. A paper in the July 2026 KR proceedings extends why/why-not provenance to Datalog with negation under well-founded and stable-model semantics. There is existing research to build on rather than treating positive provenance as the final boundary. [KR Conference Proceedings](https://proceedings.kr.org/2026/19/)

But the meaning of an explanation must expand with the language.

If a conclusion depends on “no suspension record exists,” its reason is partly about an absence in a particular data scope. Adding a suspension record can invalidate it. The positive-program technique of adding every candidate and computing supports cannot simply be carried over unchanged.

My preferred order would be: explicit finite state, bounded numeric values and comparisons, carefully scoped absence checks, then more complex negation and aggregation when a real example requires them.

Each addition should answer:

> What constitutes evidence for this operation, and what makes that evidence stop being valid?

### Make quantifiers and scope more visible

The current bare must-sentence with variables means “at least one match.”

That is a significant semantic choice for a language intended to be read without mental translation. “Some,” “every,” and “none” should be explicit where confusion would be costly.

Similarly, namespacing should preserve local meaning. Importing an unrelated module should not make an existing sentence ambiguous because every declared relation shape is globally considered.

I would prioritize explicit quantification, module visibility, and a canonical display of the parsed meaning before adding numerous alternate phrasings.

Controlled-English systems such as Attempto already demonstrate that domain-readable requirements can be translated into formal representations. Muove’s opportunity is not merely to repeat that idea, but to connect the readable representation to executable behavior and reason-sensitive change review. [arXiv](https://arxiv.org/abs/cmp-lg/9603003)

## 8. The development sequence I would actually follow

| Milestone | Concrete deliverable | Completion criterion |
|---|---|---|
| **Trustworthy review gate** | Evidence-bearing verdicts, immutable baseline, explicit requirement changes, truthful proof and diff output. | Incomplete analysis cannot approve; existing violations cannot be presented as compliance; proposals cannot contaminate the baseline. |
| **Executable, state-correct dungeon** | A minimal playable host driven by Muove’s state-transition semantics, with executable witness paths. | The one-way-door/key problem is caught, and every reported winning witness can actually be followed. |
| **Richer change review** | Separate truth, support, and checkpoint changes; checked proposal intentions; constraint-preserving repairs. | Bypasses, shared dependencies, vacuity changes, and unintended consequences are explained accurately. |
| **Measured embedding and expansion** | Cached evaluation, on-demand evidence, a small stable host interface, and one substantially larger real model. | Known-answer and independent-oracle checks remain correct under measured resource limits; new features are justified by actual modeling needs. |

I would evaluate the result against more than “did Muove reject the five example proposals?”

Prepare a mixed set of valid changes and plausible bad changes, with expected outcomes established independently. Include cases that preserve ordinary output checks but alter dependency structure. Measure false approvals, false rejections, explanation correctness, and how much effort it takes to express the intended protection.

Also compare against carefully written conventional structural tests, not only naïve outcome assertions. Other systems can test these properties with enough machinery. Muove’s advantage should be that the right property is substantially easier to state, understand, maintain, and review.

### My strongest recommendation

**Build the small playable dungeon whose explanations can be executed, before building a larger language.**

It is the shortest route to testing your central promise under pressure. It forces you to confront state, scope, proof meaning, model fidelity, and review correctness in one contained system.

The core idea still looks strong to me. The deeper opportunity is not “English that a compiler accepts.” It is that a human can approve a requirement about *how* something is allowed to happen, and the running system, its explanations, and its change review all honor that same requirement.

The Muove_deep_review\.zip[review bundle](sandbox:/mnt/data/Muove_deep_review.zip) contains the source audit, the executed Python checks and results, and nine proposed Rust regression tests. The Rust tests are supplied for local execution; they were not compiled or run here.
