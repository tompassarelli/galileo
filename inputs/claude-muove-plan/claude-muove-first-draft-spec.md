Muove: the 10 version
Oct 5, 2026 · @Tom
The thesis
Muove is a language built for people who read meaning, not for people who type code. AI will write most code, so the bottleneck becomes humans understanding and approving what it wrote.
Today's languages are bad at that. You review diffs of syntax, trust comments that nothing checks, and hope the tests cover what you care about.
At a 10, people only write and read what must be true, in English the compiler checks. The code, its explanations and the review of every change all come from those same sentences.
Requirements about why, not just whether
Comments become must-sentences, and every edit is checked against them. A must-sentence can state whether something holds, or why it holds. The second kind is what no other language can check, because no other language treats reasons as values.
Form
Example
What is checked
A plain sentence
Ash can reach Vault
It must hold
never …
never Ash can reach Shrine
It must not hold
… requires …
Ash can reach Vault requires Ash can reach Library
Every way it holds goes through the second sentence
… has no single point of failure among …
Ash can reach Vault has no single point of failure among ?key lies in ?room
No single stated fact matching the pattern can stop it on its own
Fairness becomes a compiler check. ?trap hurts ?hero requires ?trap is telegraphed means every hit the player takes was warned about first.
A comment that is checked is a must-sentence. So is a test, and so is a requirement. The English layer that drifts today becomes the one that is verified.
Turtles all the way down
There is only one kind of thing: sentences in one world, checked against each other. Every layer that is a separate artifact today becomes part of that world.
• Facts: "The Armory Gate is locked by the Bronze Key."
• Rules: "A hero can reach a room when …"
• Intents: the must-sentences, replacing comments, tests and requirements.
• Questions: why, why not, what would stop it, what would make it true. Asked and answered in the same language.
• History: every edit and every game tick is a revision, so "why was this true at tick 4,000?" is just another question.
• Grammar: sentence shapes are facts, so each domain grows its own vocabulary, and that vocabulary is checked.
• The compiler: its checks become rules too, so a rejected sentence can be asked why and answers with a proof tree.
These are turtles of explanation, not authority. Every layer can explain itself in the same language, and no layer has to authorize the next. Clause built the second kind and stalled.
The workflow: AI proposes, humans approve consequences
You write intents, an AI writes facts and rules, and every proposed change arrives as a review in English before it lands.
A change that breaks an intent comes back with the broken intent and the exact way it breaks; only a change that keeps every intent reaches you for approval. You never read code you didn't mean to write; you approve what it does. This is Clause's propose-then-admit idea doing its real job.
The killer demo: characters who can't lie about the world
Greywrought stops being a test bed and becomes the proof. When the world's rules are sentences, the same three questions that serve the designer also drive the characters.
• achieve is enemy planning. An enemy computes the smallest change to the world that gets it to you: break the door, take the key, wait out the guard.
• prevent is defensive thinking. A guard finds the single points of failure in its post and covers them.
• why and why not are honest dialogue. Ask the gatekeeper why he won't let you through, and the answer comes from the real proof: the gate is locked by the Iron Key, and you don't hold it.
An LLM can phrase the answer in character, but it cannot invent rules, because the content comes from the engine. Players can interrogate the world, modders write English, and designers see which encounters a rule change broke before anyone playtests. I don't know of a shipped game that does this.
What separates a 10 from an 8
Three things, and all three are needed.
1. Intents replace comments and tests, including intents about reasons.
2. One language for writing, asking, answering and changing, all the way down to the compiler.
3. AI writes and humans approve consequences. The language is built for reviewing, not typing.
Drop the first and it's a nice rules DSL. Drop the second and it's Inform 7 again. Drop the third and it's the AppleScript trap: English that is painful to write by hand.
The prototype
The 10 version runs today: about 3,600 lines of Rust with no dependencies and 25 passing tests. It ships as muove.zip. It takes a level designer's dungeon, five must-sentences, and five changes written the way an AI assistant would propose them.
Proposal
What it says about itself
Verdict
Secret passage
"A hidden shortcut. The main route is unchanged."
Reject: the Vault no longer requires the Library
Armory collapse
"Players can still finish the level."
Reject: true, but the Iron Key becomes a single point of failure
Pitfall
"Makes the Crypt scarier with a pit trap."
Reject: traps now hit without warning, the old spike trap included
Crowbar
"Force locked doors with a crowbar."
Reject: skips the Library and opens the sealed Shrine
Library ladder
"A second way into the Armory."
Approve
Every proposal carries a says: line, its author's own description. Muove shows it, never trusts it, and then shows what the change actually does. The armory review is the clearest case of why that matters:
  The proposal says: "Collapses the Armory for drama. Players can still finish the level."
  That's the author's claim, and nothing checks it. Here is what the change actually does.

  It changes:
    − Armory-Gate leads from Hall to Armory

  Consequences:
    − no longer true: Ash can reach Armory
    ~ weaker: Ash can reach Vault   (reasons 2 → 1)

  Must-sentences:
    ⚠ Ash can reach Vault
        Still holds, but for fewer reasons: 2 → 1.
    ✗ Ash can reach Vault has no single point of failure among ?key lies in ?room
        This alone would stop Ash can reach Vault:
          Iron-Key lies in Crypt

  Verdict: reject. It breaks 1 of 5 must-sentences.
What's built:
• Must-sentences in all four forms, checked in every world. muove check exits non-zero when one breaks, so it can gate CI.
• Reviews of proposed changes to facts and rules (add rule, remove rule), with consequences, every must-sentence, and a verdict.
• Why not: for anything false, how close each rule gets and exactly what's missing. It's the honest-dialogue engine for game characters.
• A compiler that explains itself in English: "Did you mean Vault?" for typos, and the closest sentence shapes for a line it can't read.
• Everything from before: find, why, prevent, achieve and diff. The original Clause test cases still pass, and prevent and achieve match brute-force search.
To run it: cargo run -- review examples/dungeon.clause examples/proposals/*.clause.
Honest status and what's next
The first pillar works end to end. The third works with hand-written proposals standing in for an AI. The second stops short of the compiler, whose checks are still Rust rather than Muove rules. The Lean proofs are written but not yet machine-checked, because the sandbox couldn't download Lean.
Next, in order:
1. Negation with an explicit scope ("has no blocker", "not stunned"). It's the first thing real game rules need, and the first place the guarantees can quietly break.
2. A real AI in the loop: prose → proposal file → review → the AI revises from the English rejection until the change is approved or a person decides.
3. Events and ticks: each tick becomes a revision, so "what changed this tick, and why" is just a review.
4. Numbers in rules, with explanations like "40 = 25 base × 1.6 burn".
5. Explaining on demand from snapshots, plus a C interface for Odin and a browser playground. Then one Greywrought encounter on Muove.
6. The compiler's checks as Muove rules, the last turtle.
How you'll know it's real
Give a designer who can't code an AI-written change and Muove's English review of it. If they can predict how the game will feel different without playing it, it's a 10. If they can't, it's a nice DSL.
Run that test with real people from the first playable build onward. It is the only score that counts.
Guardrails for whoever builds it next
These three rules would have prevented most of what happened to Clause.
• No new concept without a runnable example that needs it. If an example can't show why it exists, it waits.
• Every answer prints as sentences. Never hashes, never internal ids.
• Nothing is called complete unless a test proves it. Bounded searches say when they ran out.
And one about scope: Muove owns rules and their explanations. The engine, rendering and physics stay in the host language.
