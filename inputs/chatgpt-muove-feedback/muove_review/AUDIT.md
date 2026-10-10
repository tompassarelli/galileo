# Muove: deeper-review evidence

## Scope and verification

Reviewed the uploaded `muove.zip`, SHA-256 `fde69e60f8cc45bead4d4e69bdfefec659731edff664597d919aad2ac7d02e89`. Source locations below refer to that snapshot, not a later repository version. The uploaded archive and its original sources were not modified.

The Rust engine, query/review implementation, relevant parser paths, examples, tests, and Lean proof sketch were inspected. **No Rust or Lean toolchain is installed in this environment, and toolchain download failed. The project's Rust tests and Lean proofs were therefore NOT run.** Static findings below should not be represented as compiler-tested reproductions.

Two Python checks were executed successfully:

- `dungeon_state_oracle.py`: reads the dungeon's actual input facts; compares the documented reachability rules against an independent explicit `(room, inventory)` state search.
- `counterfactual_budget_oracle.py`: a small constant-unary model of the current engine's round/work-limit semantics, demonstrating that a counterfactual can be incomplete even when the original evaluation is complete.

`evolution_regressions.rs` contains nine proposed tests, including one characterization test. These tests were **not compiled or executed**. Several intentionally assert desired behavior and are expected to fail on the uploaded implementation. Copy to `tests/evolution_regressions.rs` to run locally with `cargo test --test evolution_regressions`.

## 1. Dungeon state mismatch

The example explicitly calls its doors one-way. The Bronze Key is in the Library, while the Armory gate starts in the Hall. There is no return edge from Library to Hall. Combining “Ash can reach Hall” with “Ash holds Bronze-Key” conflates facts from different moments/possible player states.

Assumptions for the explicit state oracle: keys persist and are not consumed, keys are picked up on entering their room, movement follows declared one-way doors, no unstated teleport/backtracking, trap damage does not change reachability because health/death are not modeled.

The state oracle finds Armory unreachable in the base dungeon. Removing the Iron Key makes the Vault unreachable in that state model, while the declarative rule closure still derives it. Adding the Library-to-Armory ladder makes a genuine Bronze-Key route available. This is a mismatch between the intended game interpretation and the declared abstraction, not an unsound inference by the positive-rule engine.

Results:

```json
{
  "base": {
    "declarative_reachable": [
      "Armory",
      "Crypt",
      "Entrance",
      "Hall",
      "Library",
      "Vault"
    ],
    "state_reachable": [
      "Crypt",
      "Entrance",
      "Hall",
      "Library",
      "Vault"
    ],
    "vault_state_path": [
      "Entrance",
      "--Front-Door-->",
      "Hall",
      "--Library-Arch-->",
      "Library",
      "--Crypt-Stair-->",
      "Crypt",
      "--Crypt-Door-->",
      "Vault"
    ]
  },
  "without_iron_key": {
    "declarative_reachable": [
      "Armory",
      "Crypt",
      "Entrance",
      "Hall",
      "Library",
      "Vault"
    ],
    "state_reachable": [
      "Crypt",
      "Entrance",
      "Hall",
      "Library"
    ],
    "vault_state_path": null
  },
  "armory_collapsed": {
    "declarative_reachable": [
      "Crypt",
      "Entrance",
      "Hall",
      "Library",
      "Vault"
    ],
    "state_reachable": [
      "Crypt",
      "Entrance",
      "Hall",
      "Library",
      "Vault"
    ],
    "vault_state_path": [
      "Entrance",
      "--Front-Door-->",
      "Hall",
      "--Library-Arch-->",
      "Library",
      "--Crypt-Stair-->",
      "Crypt",
      "--Crypt-Door-->",
      "Vault"
    ]
  },
  "ladder_added_without_iron_key": {
    "declarative_reachable": [
      "Armory",
      "Crypt",
      "Entrance",
      "Hall",
      "Library",
      "Vault"
    ],
    "state_reachable": [
      "Armory",
      "Crypt",
      "Entrance",
      "Hall",
      "Library",
      "Vault"
    ],
    "vault_state_path": [
      "Entrance",
      "--Front-Door-->",
      "Hall",
      "--Library-Arch-->",
      "Library",
      "--Library-Ladder-->",
      "Armory",
      "--Vault-Gate-->",
      "Vault"
    ]
  },
  "silver_key_placements_that_unlock_shrine": {
    "declarative": [
      "Entrance",
      "Hall",
      "Library",
      "Armory",
      "Crypt",
      "Vault"
    ],
    "state": [
      "Entrance",
      "Hall",
      "Library",
      "Crypt"
    ]
  }
}

```

## 2. Approval and evidence findings

| Finding | Relevant code | Observation |
|---|---|---|
| Unknown can pass `check` | `src/lib.rs:45–63`; `src/ask.rs:815–847` | `intents_report` uses booleans without completeness; an unfinished negative search can look satisfied. |
| Unknown can pass `review` | `src/lib.rs:69–107`; `src/render.rs:475–490` | Success/verdict uses `broken`, not completeness. |
| Already-broken requirements can be approved | `src/ask.rs:943–975`; `src/render.rs:475–490` | `broken` counts only true-to-false transitions. |
| Counterfactual status is discarded | `src/ask.rs:849–903` | `requires`/single-point checks read `.holds` from inner evaluations but retain the outer closure status. |
| Proof fallback can invent an asserted leaf | `src/ask.rs:237–274` | A missing rank/firing returns `Proof::Stated` even when the fact is absent from the support. |
| Proposal can contaminate baseline | `src/lib.rs:69–84`; `src/program.rs:323–369` | Proposal text is concatenated with baseline, then all top-level rules are installed in base models before revisions are resolved. |
| Truth diff conflates asserted/derived origin | `src/ask.rs:438–484` | It compares derived-only sets, so a still-true conclusion that becomes explicitly stated can be labeled lost. |

### Concrete counterfactual-budget construction

Use Seed and Gate as stated facts. For C1...C5, provide a one-step derivation from Gate and a slow chain from Seed; Goal follows from C5. Put every atom in the same unary relation. The original closure completes in 187 candidate scans. Blocking Gate requires the slow chain and exhausts a budget of 187 before committing Goal. Full evaluation still derives Goal, so Gate is not necessary.

The Rust regression file spells out this construction. The independent loop model produced:

```json
{
  "base_budget187": {
    "facts": [
      "C1",
      "C2",
      "C3",
      "C4",
      "C5",
      "Gate",
      "Goal",
      "Seed"
    ],
    "complete": true,
    "work": 187,
    "rounds": 3
  },
  "blocked_budget187": {
    "facts": [
      "C1",
      "C2",
      "C3",
      "C4",
      "C5",
      "Seed"
    ],
    "complete": false,
    "work": 188,
    "rounds": 6
  },
  "blocked_complete": {
    "facts": [
      "C1",
      "C2",
      "C3",
      "C4",
      "C5",
      "Goal",
      "Seed"
    ],
    "complete": true,
    "work": 308,
    "rounds": 7
  }
}

```

## 3. Fact support is not derivation structure

For `Gate <- Seed` and `Goal <- Gate`, Goal's only minimal stated-fact support is `{Seed}`. Adding `Goal <- Seed` leaves that minimal support unchanged but breaks `Goal requires Gate`. Current `requires` catches this with a complete search; current fact-support `diff.changed` does not describe the checkpoint-dependency change.

Keep fact supports for withdrawal/addition questions. Keep proof/checkpoint structure separately for mechanism-sensitive review. Do not equate minimal fact supports with all derivations, disjoint paths, or statistically independent reasons.

## 4. Recommended evolution order

1. A trustworthy review gate: evidence-based Pass/Fail/Unknown, fail-closed approval, immutable baseline, frozen requirements, truthful proof objects, origin-aware diffs.
2. An executable dungeon slice using explicit player states and the same operational semantics for play and review; finite-state and trace scope should be visible.
3. Richer reason-sensitive diffs, explicit failure sets/groups, change contracts, and repairs that preserve protected requirements.
4. A measured evaluation architecture: cached truth closure, on-demand proofs, target-directed support computation, inner-loop work/memory guards, and an independent reference evaluator.
5. Deliberate expansion into bounded numbers, state/time, scoped negation and constrained interventions. Do not apply positive-program hitting-set theorems unchanged to nonmonotone models.

The original tests' brute-force checks call the same `engine::close` as production. They are valuable checks of the support/hitting-set algorithms, but are not an independent validation of closure semantics or of the gameplay interpretation.

The Lean file explicitly states that it is unverified and proves abstract characterizations, not the Rust evaluator. Its hypotheses should become a concise executable semantic contract for the kernel.

## Source excerpts

The following are verbatim extracts from the uploaded code, with original file line numbers.


### Dungeon facts and rules: `examples/dungeon.clause:1–143`

```text
1: # A small dungeon level, written as rules a level designer can read.
2: # Doors are one-way passages. Some are locked and need a key, and keys
3: # lie in rooms. Traps strike only after the game has warned the player.
4: # The must-sentences say what has to stay true; the questions at the bottom
5: # are the ones a designer asks.
6: 
7: Hero
8: Room
9: Door
10: Key
11: Trap
12: 
13: level/starts-in: RelationShape
14:   {hero: Hero} starts in {room: Room}
15: 
16: level/leads: RelationShape
17:   {door: Door} leads from {here: Room} to {there: Room}
18: 
19: level/unlocked: RelationShape
20:   {door: Door} is unlocked
21: 
22: level/locked-by: RelationShape
23:   {door: Door} is locked by {key: Key}
24: 
25: level/lies-in: RelationShape
26:   {key: Key} lies in {room: Room}
27: 
28: level/reach: RelationShape
29:   {hero: Hero} can reach {room: Room}
30: 
31: level/holds: RelationShape
32:   {hero: Hero} holds {key: Key}
33: 
34: level/armed-in: RelationShape
35:   {trap: Trap} is armed in {room: Room}
36: 
37: level/telegraphed: RelationShape
38:   {trap: Trap} is telegraphed
39: 
40: level/hurts: RelationShape
41:   {trap: Trap} hurts {hero: Hero}
42: 
43: level
44:   Ash ∈ Hero
45:   Entrance ∈ Room
46:   Hall ∈ Room
47:   Library ∈ Room
48:   Armory ∈ Room
49:   Crypt ∈ Room
50:   Vault ∈ Room
51:   Shrine ∈ Room
52:   Front-Door ∈ Door
53:   Library-Arch ∈ Door
54:   Armory-Gate ∈ Door
55:   Crypt-Stair ∈ Door
56:   Vault-Gate ∈ Door
57:   Crypt-Door ∈ Door
58:   Shrine-Door ∈ Door
59:   Bronze-Key ∈ Key
60:   Iron-Key ∈ Key
61:   Silver-Key ∈ Key
62:   Spike-Trap ∈ Trap
63: 
64:   Ash starts in Entrance
65: 
66:   Front-Door
67:     leads from Entrance to Hall
68:     is unlocked
69:   Library-Arch
70:     leads from Hall to Library
71:     is unlocked
72:   Armory-Gate
73:     leads from Hall to Armory
74:     is locked by Bronze-Key
75:   Crypt-Stair
76:     leads from Library to Crypt
77:     is unlocked
78:   Vault-Gate
79:     leads from Armory to Vault
80:     is locked by Bronze-Key
81:   Crypt-Door
82:     leads from Crypt to Vault
83:     is locked by Iron-Key
84:   Shrine-Door
85:     leads from Crypt to Shrine
86:     is locked by Silver-Key
87: 
88:   Bronze-Key lies in Library
89:   Iron-Key lies in Crypt
90: 
91:   Spike-Trap
92:     is armed in Hall
93:     is telegraphed
94: 
95: level/start: DerivationRule
96:   ?hero can reach ?room
97:   when:
98:     ?hero starts in ?room
99: 
100: level/walk: DerivationRule
101:   ?hero can reach ?there
102:   when:
103:     ?hero can reach ?here
104:     ?door leads from ?here to ?there
105:     ?door is unlocked
106: 
107: level/unlock: DerivationRule
108:   ?hero can reach ?there
109:   when:
110:     ?hero can reach ?here
111:     ?door leads from ?here to ?there
112:     ?door is locked by ?key
113:     ?hero holds ?key
114: 
115: level/pick-up: DerivationRule
116:   ?hero holds ?key
117:   when:
118:     ?key lies in ?room
119:     ?hero can reach ?room
120: 
121: level/trap-strike: DerivationRule
122:   ?trap hurts ?hero
123:   when:
124:     ?trap is armed in ?room
125:     ?trap is telegraphed
126:     ?hero can reach ?room
127: 
128: # What must stay true, whoever edits this level: a person or an AI.
129: # Every edit is checked against these. Try `muove review` on a proposal.
130: level must:
131:   Ash can reach Vault
132:   Ash can reach Vault requires Ash can reach Library
133:   Ash can reach Vault has no single point of failure among ?key lies in ?room
134:   ?trap hurts ?hero requires ?trap is telegraphed
135:   never Ash can reach Shrine
136: 
137: # Which rooms can Ash get to?
138: find all ?room in level:
139:   Ash can reach ?room
140: 
141: # Every distinct way to beat the level.
142: why all in level:
143:   Ash can reach Vault
```


### Approval APIs: `src/lib.rs:40–108`

```text
40:     let (intents, _) = intents_report(&program, limits);
41:     if !intents.is_empty() {
42:         if !out.is_empty() {
43:             out.push('\n');
44:         }
45:         out.push_str(&intents);
46:     }
47:     Ok(out)
48: }
49: 
50: /// Check every model's must-sentences in every world: models in full,
51: /// revisions only where something breaks. Returns the text and whether all hold.
52: pub fn intents_report(program: &Program, limits: &Limits) -> (String, bool) {
53:     let mut out = String::new();
54:     let mut all_hold = true;
55:     for world in 0..program.worlds.len() {
56:         let checks = ask::check_world(program, world, limits);
57:         if checks.is_empty() {
58:             continue;
59:         }
60:         let broken = checks.iter().any(|(_, c)| !c.holds);
61:         all_hold &= !broken;
62:         if program.worlds[world].base.is_none() || broken {
63:             if !out.is_empty() {
64:                 out.push('\n');
65:             }
66:             out.push_str(&render::render_world_intents(program, world, &checks));
67:         }
68:     }
69:     (out, all_hold)
70: }
71: 
72: /// Review every revision in `proposal` against the program in `base`.
73: /// Returns the review text and whether every proposal can be approved.
74: pub fn review(base: &str, proposal: &str, limits: &Limits) -> Result<(String, bool), Error> {
75:     let base = base.trim_end_matches('\n');
76:     let offset = base.lines().count() + 1;
77:     let combined = format!("{base}\n\n{proposal}");
78:     let program = Program::parse(&combined).map_err(|mut e| {
79:         if e.line > offset {
80:             e.line -= offset;
81:             e.message = format!("in the proposal: {}", e.message);
82:         }
83:         e
84:     })?;
85:     let proposed: Vec<usize> = (0..program.worlds.len())
86:         .filter(|&w| program.worlds[w].base.is_some() && program.worlds[w].line > offset)
87:         .collect();
88:     if proposed.is_empty() {
89:         return Err(Error::bare("the proposal defines no revision to review"));
90:     }
91:     let mut out = String::new();
92:     let mut approved = true;
93:     for successor in proposed {
94:         let base = program.worlds[successor].base.unwrap();
95:         let result = ask::review(&program, base, successor, limits);
96:         approved &= result.broken == 0;
97:         let item = program::RequestItem {
98:             line: 0,
99:             source: vec![format!("review {} -> {}", program.worlds[base].name, program.worlds[successor].name)],
100:             request: program::Request::Review { base, successor },
101:         };
102:         if !out.is_empty() {
103:             out.push('\n');
104:         }
105:         out.push_str(&render::render(&program, &item, &Answer::Review(result)));
106:     }
107:     Ok((out, approved))
108: }
```


### Counterfactual requirements and review: `src/ask.rs:793–976`

```text
793: pub struct IntentCheck {
794:     pub holds: bool,
795:     /// It holds only because nothing triggers it yet.
796:     pub vacuous: bool,
797:     pub problem: Option<Problem>,
798:     pub status: Status,
799: }
800: 
801: #[derive(Debug)]
802: pub enum Problem {
803:     /// A must-hold sentence doesn't hold.
804:     Missing { why_not: Option<WhyNot> },
805:     /// A `never` sentence holds.
806:     Present { facts: Vec<Fact>, proof: Proof },
807:     /// A conclusion holds without what it requires; `proof` avoids it.
808:     /// `others` are further conclusions with the same problem.
809:     Bypassed { fact: Fact, needed: String, proof: Proof, others: Vec<Fact> },
810:     /// Each of these stated facts can stop the conclusion on its own.
811:     SinglePoints { fact: Fact, points: Vec<Fact> },
812: }
813: 
814: /// Check one must-sentence in one world.
815: pub fn check_intent(program: &Program, world: usize, intent: &Intent, limits: &Limits) -> IntentCheck {
816:     let w = &program.worlds[world];
817:     let closure = close(program, world, &w.facts, limits);
818:     let width = intent.vars.len();
819:     let ok = |vacuous| IntentCheck {
820:         holds: true,
821:         vacuous,
822:         problem: None,
823:         status: closure.status.clone(),
824:     };
825:     let broken = |problem| IntentCheck {
826:         holds: false,
827:         vacuous: false,
828:         problem: Some(problem),
829:         status: closure.status.clone(),
830:     };
831:     match &intent.kind {
832:         IntentKind::Holds(atom) => {
833:             if !matches_of(&closure.facts, atom, width).is_empty() {
834:                 return ok(false);
835:             }
836:             let why_not = ground_partial(atom, &vec![None; width]).map(|f| why_not(program, world, &f, limits));
837:             broken(Problem::Missing { why_not })
838:         }
839:         IntentKind::Never(atom) => {
840:             let mut facts: Vec<Fact> = matches_of(&closure.facts, atom, width).into_iter().map(|(f, _)| f).collect();
841:             if facts.is_empty() {
842:                 return ok(false);
843:             }
844:             sort_facts(program, &mut facts);
845:             let first = why(program, world, &facts[0], Pick::One, limits);
846:             let proof = first
847:                 .reasons
848:                 .into_iter()
849:                 .next()
850:                 .map(|r| r.proof)
851:                 .unwrap_or(Proof::Stated(facts[0].clone()));
852:             broken(Problem::Present { facts, proof })
853:         }
854:         IntentKind::Requires(subject, needs) => {
855:             let found = matches_of(&closure.facts, subject, width);
856:             let vacuous = found.is_empty();
857:             let mut cases: Vec<(Fact, Vec<Option<usize>>)> = found
858:                 .into_iter()
859:                 .filter(|(fact, binding)| {
860:                     let blocked = |f: &Fact| fits(needs, f, &mut binding.clone());
861:                     close_blocking(program, world, &w.facts, &blocked, limits).holds(fact)
862:                 })
863:                 .collect();
864:             if cases.is_empty() {
865:                 return ok(vacuous);
866:             }
867:             cases.sort_by_cached_key(|(fact, _)| program.sentence(fact));
868:             let others = cases[1..].iter().map(|(fact, _)| fact.clone()).collect();
869:             let (fact, binding) = cases.swap_remove(0);
870:             // Show the simplest way it holds while avoiding what it requires.
871:             let blocked = |f: &Fact| fits(needs, f, &mut binding.clone());
872:             let without = close_blocking(program, world, &w.facts, &blocked, limits);
873:             let base: BTreeSet<Fact> = w.facts.iter().filter(|f| !blocked(f)).cloned().collect();
874:             let simplest = reasons(&without, &base, &|_| true, limits)
875:                 .of
876:                 .get(&fact)
877:                 .and_then(|r| r.iter().min_by_key(|s| s.len()).cloned())
878:                 .unwrap_or_default();
879:             let proof = prove_avoiding(program, world, &simplest, &fact, &blocked, limits);
880:             let needed = partial_sentence(program, needs, &binding, &intent.vars);
881:             broken(Problem::Bypassed { fact, needed, proof, others })
882:         }
883:         IntentKind::NoSinglePoint(subject, among) => {
884:             let found = matches_of(&closure.facts, subject, width);
885:             let vacuous = found.is_empty();
886:             for (fact, binding) in found {
887:                 let mut points: Vec<Fact> = w
888:                     .facts
889:                     .iter()
890:                     .filter(|f| fits(among, f, &mut binding.clone()))
891:                     .filter(|candidate| {
892:                         let rest: BTreeSet<Fact> = w.facts.iter().filter(|f| f != candidate).cloned().collect();
893:                         !close(program, world, &rest, limits).holds(&fact)
894:                     })
895:                     .cloned()
896:                     .collect();
897:                 if !points.is_empty() {
898:                     sort_facts(program, &mut points);
899:                     return broken(Problem::SinglePoints { fact, points });
900:                 }
901:             }
902:             ok(vacuous)
903:         }
904:     }
905: }
906: 
907: /// Every must-sentence of the world's model, checked in that world.
908: pub fn check_world(program: &Program, world: usize, limits: &Limits) -> Vec<(usize, IntentCheck)> {
909:     let model = program.worlds[world].model;
910:     program
911:         .intents
912:         .iter()
913:         .enumerate()
914:         .filter(|(_, intent)| intent.model == model)
915:         .map(|(i, intent)| (i, check_intent(program, world, intent, limits)))
916:         .collect()
917: }
918: 
919: // ---------------------------------------------------------------- review
920: 
921: #[derive(Debug)]
922: pub struct Review {
923:     /// What the author says the change does. Unchecked.
924:     pub says: Option<String>,
925:     pub diff: Diff,
926:     pub intents: Vec<IntentReview>,
927:     /// Intents that held before and don't after.
928:     pub broken: usize,
929:     pub status: Status,
930: }
931: 
932: #[derive(Debug)]
933: pub struct IntentReview {
934:     pub intent: usize,
935:     pub before: IntentCheck,
936:     pub after: IntentCheck,
937:     /// A must-hold sentence that still holds for fewer reasons: (before, after, now required).
938:     pub weaker: Option<(usize, usize, Vec<Fact>)>,
939: }
940: 
941: /// Review a proposed revision: what it changes, what follows, and which
942: /// must-sentences it keeps or breaks.
943: pub fn review(program: &Program, base: usize, successor: usize, limits: &Limits) -> Review {
944:     let diff = diff(program, base, successor, limits);
945:     let mut status = diff.status.clone();
946:     let mut intents = Vec::new();
947:     for ((i, before), (_, after)) in check_world(program, base, limits).into_iter().zip(check_world(program, successor, limits)) {
948:         status = status.and(before.status.clone()).and(after.status.clone());
949:         let mut weaker = None;
950:         if let IntentKind::Holds(atom) = &program.intents[i].kind
951:             && after.holds
952:             && let Some(fact) = ground_partial(atom, &vec![None; program.intents[i].vars.len()])
953:             && let Some(change) = diff.changed.iter().find(|c| c.fact == fact && !c.lost.is_empty())
954:         {
955:             let was = change.kept.len() + change.lost.len();
956:             let now = change.kept.len() + change.new.len();
957:             if now < was {
958:                 weaker = Some((was, now, change.now_required.clone()));
959:             }
960:         }
961:         intents.push(IntentReview {
962:             intent: i,
963:             before,
964:             after,
965:             weaker,
966:         });
967:     }
968:     let broken = intents.iter().filter(|r| r.before.holds && !r.after.holds).count();
969:     Review {
970:         says: program.worlds[successor].says.clone(),
971:         diff,
972:         intents,
973:         broken,
974:         status,
975:     }
976: }
```


### Proof construction: `src/ask.rs:237–277`

```text
237: pub fn prove(program: &Program, world: usize, support: &Support, target: &Fact, limits: &Limits) -> Proof {
238:     prove_avoiding(program, world, support, target, &|_| false, limits)
239: }
240: 
241: /// Like [`prove`], but never passing through a `blocked` fact.
242: pub fn prove_avoiding(
243:     program: &Program,
244:     world: usize,
245:     support: &Support,
246:     target: &Fact,
247:     blocked: &dyn Fn(&Fact) -> bool,
248:     limits: &Limits,
249: ) -> Proof {
250:     let closure = close_blocking(program, world, support, blocked, limits);
251:     build_proof(&closure, support, target)
252: }
253: 
254: fn build_proof(closure: &Closure, support: &Support, fact: &Fact) -> Proof {
255:     if support.contains(fact) {
256:         return Proof::Stated(fact.clone());
257:     }
258:     let Some(&rank) = closure.rank.get(fact) else {
259:         return Proof::Stated(fact.clone());
260:     };
261:     // Pick an application whose premises all appeared strictly earlier, so the
262:     // proof is well-founded even when rules are recursive.
263:     let firing = closure.firings.iter().find(|firing| {
264:         &firing.head == fact
265:             && firing
266:                 .body
267:                 .iter()
268:                 .all(|b| closure.rank.get(b).is_some_and(|&r| r < rank))
269:     });
270:     match firing {
271:         Some(firing) => Proof::Derived {
272:             fact: fact.clone(),
273:             rule: firing.rule,
274:             from: firing.body.iter().map(|b| build_proof(closure, support, b)).collect(),
275:         },
276:         None => Proof::Stated(fact.clone()),
277:     }
```


### Diff classification: `src/ask.rs:438–523`

```text
438: pub fn diff(program: &Program, base: usize, successor: usize, limits: &Limits) -> Diff {
439:     let (a, b) = (&program.worlds[base], &program.worlds[successor]);
440:     let mut added: Vec<Fact> = b.facts.difference(&a.facts).cloned().collect();
441:     let mut removed: Vec<Fact> = a.facts.difference(&b.facts).cloned().collect();
442:     sort_facts(program, &mut added);
443:     sort_facts(program, &mut removed);
444: 
445:     let closure_a = close(program, base, &a.facts, limits);
446:     let closure_b = close(program, successor, &b.facts, limits);
447:     let reasons_a = reasons(&closure_a, &a.facts, &|_| true, limits);
448:     let reasons_b = reasons(&closure_b, &b.facts, &|_| true, limits);
449:     let status = reasons_a.status.clone().and(reasons_b.status.clone());
450: 
451:     let derived_a: BTreeSet<&Fact> = closure_a.facts.iter().filter(|f| !a.facts.contains(f)).collect();
452:     let derived_b: BTreeSet<&Fact> = closure_b.facts.iter().filter(|f| !b.facts.contains(f)).collect();
453:     let of = |r: &crate::engine::Reasons, f: &Fact| r.of.get(f).cloned().unwrap_or_default();
454: 
455:     let mut lost = Vec::new();
456:     for &fact in derived_a.difference(&derived_b) {
457:         let before = of(&reasons_a, fact);
458:         let mut because: Vec<Fact> = removed
459:             .iter()
460:             .filter(|r| before.iter().any(|reason| reason.contains(r)))
461:             .cloned()
462:             .collect();
463:         sort_facts(program, &mut because);
464:         lost.push(Lost {
465:             fact: fact.clone(),
466:             reasons_before: ordered(program, before),
467:             because,
468:         });
469:     }
470:     let mut gained = Vec::new();
471:     for &fact in derived_b.difference(&derived_a) {
472:         let after = of(&reasons_b, fact);
473:         let mut thanks_to: Vec<Fact> = added
474:             .iter()
475:             .filter(|r| after.iter().any(|reason| reason.contains(r)))
476:             .cloned()
477:             .collect();
478:         sort_facts(program, &mut thanks_to);
479:         gained.push(Gained {
480:             fact: fact.clone(),
481:             reasons_after: ordered(program, after),
482:             thanks_to,
483:         });
484:     }
485:     let mut changed = Vec::new();
486:     for &fact in derived_a.intersection(&derived_b) {
487:         let before = of(&reasons_a, fact);
488:         let after = of(&reasons_b, fact);
489:         let before_set: BTreeSet<&Support> = before.iter().collect();
490:         let after_set: BTreeSet<&Support> = after.iter().collect();
491:         if before_set == after_set {
492:             continue;
493:         }
494:         let required_before = intersection(&before);
495:         let required_after = intersection(&after);
496:         let mut now_required: Vec<Fact> = required_after.difference(&required_before).cloned().collect();
497:         let mut no_longer_required: Vec<Fact> = required_before.difference(&required_after).cloned().collect();
498:         sort_facts(program, &mut now_required);
499:         sort_facts(program, &mut no_longer_required);
500:         changed.push(Changed {
501:             fact: fact.clone(),
502:             kept: ordered(program, before_set.intersection(&after_set).map(|s| (*s).clone())),
503:             lost: ordered(program, before_set.difference(&after_set).map(|s| (*s).clone())),
504:             new: ordered(program, after_set.difference(&before_set).map(|s| (*s).clone())),
505:             now_required,
506:             no_longer_required,
507:         });
508:     }
509:     lost.sort_by_cached_key(|l| program.sentence(&l.fact));
510:     gained.sort_by_cached_key(|g| program.sentence(&g.fact));
511:     changed.sort_by_cached_key(|c| program.sentence(&c.fact));
512:     let new_names = b.entities.difference(&a.entities).copied().collect();
513:     Diff {
514:         new_names,
515:         rules_added: b.rules.difference(&a.rules).copied().collect(),
516:         rules_removed: a.rules.difference(&b.rules).copied().collect(),
517:         added,
518:         removed,
519:         lost,
520:         gained,
521:         changed,
522:         status,
523:     }
```


### Combined parsing and baseline installation: `src/program.rs:323–370`

```text
323:     pub fn parse(source: &str) -> Result<Program, Error> {
324:         let top = blocks(source)?;
325:         let kinds: Vec<Kind> = top.iter().map(classify).collect();
326:         let mut p = Program::default();
327: 
328:         for (block, kind) in top.iter().zip(&kinds) {
329:             if let Kind::Type = kind {
330:                 p.declare_type(&block.line)?;
331:             }
332:         }
333:         for (block, kind) in top.iter().zip(&kinds) {
334:             if let Kind::Relation(name) = kind {
335:                 p.declare_relation(name, block)?;
336:             }
337:         }
338:         let mut model_blocks = Vec::new();
339:         for (block, kind) in top.iter().zip(&kinds) {
340:             if let Kind::Model = kind {
341:                 let model = p.declare_model(block)?;
342:                 model_blocks.push((model, block));
343:             }
344:         }
345:         for (model, block) in &model_blocks {
346:             p.fill_model(*model, block)?;
347:         }
348:         for (block, kind) in top.iter().zip(&kinds) {
349:             if let Kind::Rule(name) = kind {
350:                 let id = p.read_rule(name, &block.line, &block.children)?;
351:                 let root = p.world_index[&p.models[p.rules[id].model]];
352:                 p.worlds[root].rules.insert(id);
353:             }
354:         }
355:         for (block, kind) in top.iter().zip(&kinds) {
356:             if let Kind::Intents(model) = kind {
357:                 let Some(m) = p.models.iter().position(|name| name == model) else {
358:                     return Err(Error::at(&block.line, format!("no model named `{model}`")));
359:                 };
360:                 for child in &block.children {
361:                     if !child.children.is_empty() {
362:                         return Err(Error::at(&child.children[0].line, "must-sentences don't nest"));
363:                     }
364:                     let intent = p.read_intent(m, &child.line)?;
365:                     p.intents.push(intent);
366:                 }
367:             }
368:         }
369: 
370:         let mut revisions: HashMap<String, &Block> = HashMap::new();
```


### Minimal-support representation and limits: `src/engine.rs:242–343`

```text
242: pub fn insert_minimal(list: &mut Vec<Support>, set: Support) -> bool {
243:     if list.iter().any(|existing| existing.is_subset(&set)) {
244:         return false;
245:     }
246:     list.retain(|existing| !set.is_subset(existing));
247:     list.push(set);
248:     true
249: }
250: 
251: #[derive(Debug)]
252: pub struct Reasons {
253:     /// For each fact in the closure, its minimal reasons over tracked facts.
254:     pub of: HashMap<Fact, Vec<Support>>,
255:     pub status: Status,
256: }
257: 
258: /// Minimal reasons for every fact, counting only `tracked` stated facts.
259: ///
260: /// Untracked stated facts are treated as free (they contribute nothing to a
261: /// reason). With everything tracked this is the classic minimal-witness
262: /// provenance; with only some facts tracked it answers "which of *these*
263: /// facts does the conclusion depend on?".
264: pub fn reasons(
265:     closure: &Closure,
266:     base: &BTreeSet<Fact>,
267:     tracked: &dyn Fn(&Fact) -> bool,
268:     limits: &Limits,
269: ) -> Reasons {
270:     let mut of: HashMap<Fact, Vec<Support>> = HashMap::new();
271:     for fact in base {
272:         let reason = if tracked(fact) {
273:             BTreeSet::from([fact.clone()])
274:         } else {
275:             BTreeSet::new()
276:         };
277:         of.insert(fact.clone(), vec![reason]);
278:     }
279:     let mut dependents: HashMap<&Fact, Vec<usize>> = HashMap::new();
280:     for (i, firing) in closure.firings.iter().enumerate() {
281:         for fact in &firing.body {
282:             dependents.entry(fact).or_default().push(i);
283:         }
284:     }
285:     let mut queue: VecDeque<usize> = (0..closure.firings.len()).collect();
286:     let mut queued = vec![true; closure.firings.len()];
287:     let mut capped: HashSet<Fact> = HashSet::new();
288:     let mut work = 0usize;
289:     let mut status = closure.status.clone();
290: 
291:     while let Some(i) = queue.pop_front() {
292:         queued[i] = false;
293:         let firing = &closure.firings[i];
294:         if capped.contains(&firing.head) {
295:             continue;
296:         }
297:         let mut combined: Vec<Support> = vec![Support::new()];
298:         for fact in &firing.body {
299:             let Some(options) = of.get(fact) else {
300:                 combined.clear();
301:                 break;
302:             };
303:             let mut next = Vec::new();
304:             for partial in &combined {
305:                 for option in options {
306:                     work += 1;
307:                     let mut union = partial.clone();
308:                     union.extend(option.iter().cloned());
309:                     insert_minimal(&mut next, union);
310:                 }
311:             }
312:             combined = next;
313:             if work > limits.max_work {
314:                 let status = status.and(Status::Incomplete(format!(
315:                     "stopped after {} steps while collecting reasons; raise --max-work",
316:                     limits.max_work
317:                 )));
318:                 return Reasons { of, status };
319:             }
320:         }
321:         if combined.is_empty() {
322:             continue;
323:         }
324:         let list = of.entry(firing.head.clone()).or_default();
325:         let mut changed = false;
326:         for reason in combined {
327:             changed |= insert_minimal(list, reason);
328:         }
329:         if list.len() > limits.max_reasons {
330:             list.sort_by_key(|r| r.len());
331:             list.truncate(limits.max_reasons);
332:             capped.insert(firing.head.clone());
333:             status = status.and(Status::Incomplete(format!(
334:                 "kept only {} reasons for one conclusion; raise --max-reasons",
335:                 limits.max_reasons
336:             )));
337:         }
338:         if changed && let Some(deps) = dependents.get(&firing.head) {
339:             for &d in deps {
340:                 if !queued[d] {
341:                     queued[d] = true;
342:                     queue.push_back(d);
343:                 }
```


### Review rendering: `src/render.rs:451–493`

```text
451:     if out.len() == before {
452:         let _ = writeln!(out, "    No conclusions change.");
453:     }
454: 
455:     let _ = writeln!(out, "\n  Must-sentences:");
456:     let mut warnings = 0;
457:     for item in &review.intents {
458:         let text = &program.intents[item.intent].text;
459:         let note = match (item.before.holds, item.after.holds) {
460:             (true, false) => "   ← broken by this change",
461:             (false, false) => "   (already broken before this change)",
462:             (false, true) => "   ← fixed by this change",
463:             (true, true) => "",
464:         };
465:         if let (Some((was, now, required)), true) = (&item.weaker, item.after.holds) {
466:             warnings += 1;
467:             let _ = writeln!(out, "    ⚠ {text}");
468:             let _ = writeln!(out, "        Still holds, but for fewer reasons: {was} → {now}.");
469:             if !required.is_empty() {
470:                 list_block(program, out, "        It now depends on every one of: ", required);
471:             }
472:             continue;
473:         }
474:         render_intent(program, out, text, &item.after, note, "    ");
475:     }
476: 
477:     let total = review.intents.len();
478:     out.push('\n');
479:     if total == 0 {
480:         let _ = writeln!(out, "  Verdict: nothing to check. This model has no must-sentences yet.");
481:     } else if review.broken > 0 {
482:         let _ = writeln!(
483:             out,
484:             "  Verdict: reject. It breaks {} of {}.",
485:             review.broken,
486:             plural(total, "must-sentence", "must-sentences")
487:         );
488:     } else if warnings > 0 {
489:         let _ = writeln!(out, "  Verdict: approve, with {}.", plural(warnings, "warning", "warnings"));
490:     } else {
491:         let _ = writeln!(out, "  Verdict: approve. Every must-sentence still holds.");
492:     }
493: }
```


### Brute-force oracle shares the evaluator: `tests/oracle.rs:65–161`

```text
65: fn brute_prevent(p: &Program, w: usize, target: &Fact, using: &[usize]) -> BTreeSet<BTreeSet<Fact>> {
66:     let world = &p.worlds[w];
67:     let pool: Vec<Fact> = world.facts.iter().filter(|f| using.contains(&f.rel)).cloned().collect();
68:     assert!(pool.len() <= 16, "brute force is for small pools");
69:     let mut hits = Vec::new();
70:     for mask in 0u32..(1 << pool.len()) {
71:         let chosen: BTreeSet<Fact> = (0..pool.len()).filter(|i| mask & (1 << i) != 0).map(|i| pool[i].clone()).collect();
72:         let rest: BTreeSet<Fact> = world.facts.difference(&chosen).cloned().collect();
73:         if !close(p, w, &rest, &limits()).holds(target) {
74:             hits.push(chosen);
75:         }
76:     }
77:     minimal(hits)
78: }
79: 
80: /// Every minimal set of at most `cap` new `using` facts that makes `target` hold.
81: fn brute_achieve(p: &Program, w: usize, target: &Fact, using: &[usize], cap: usize) -> BTreeSet<BTreeSet<Fact>> {
82:     let world = &p.worlds[w];
83:     let mut pool = Vec::new();
84:     for &r in using {
85:         let roles = &p.relations[r].roles;
86:         let pools: Vec<Vec<usize>> = roles
87:             .iter()
88:             .map(|role| world.entities.iter().copied().filter(|&e| p.entities[e].ty == role.ty).collect())
89:             .collect();
90:         let mut args = vec![Vec::new()];
91:         for options in &pools {
92:             args = args
93:                 .into_iter()
94:                 .flat_map(|prefix: Vec<usize>| options.iter().map(move |&o| [prefix.clone(), vec![o]].concat()))
95:                 .collect();
96:         }
97:         for a in args {
98:             let fact = Fact { rel: r, args: a };
99:             if !world.facts.contains(&fact) {
100:                 pool.push(fact);
101:             }
102:         }
103:     }
104:     let mut hits = Vec::new();
105:     let mut choose = |set: BTreeSet<Fact>| {
106:         let with: BTreeSet<Fact> = world.facts.union(&set).cloned().collect();
107:         if close(p, w, &with, &limits()).holds(target) {
108:             hits.push(set);
109:         }
110:     };
111:     for i in 0..pool.len() {
112:         choose(BTreeSet::from([pool[i].clone()]));
113:         if cap >= 2 {
114:             for j in i + 1..pool.len() {
115:                 choose(BTreeSet::from([pool[i].clone(), pool[j].clone()]));
116:             }
117:         }
118:     }
119:     minimal(hits)
120: }
121: 
122: fn minimal(sets: Vec<BTreeSet<Fact>>) -> BTreeSet<BTreeSet<Fact>> {
123:     sets.iter()
124:         .filter(|s| !sets.iter().any(|o| o.len() < s.len() && o.is_subset(s)))
125:         .cloned()
126:         .collect()
127: }
128: 
129: /// Run every prevent/achieve question in a program and compare with brute force.
130: fn cross_check(source: &str, achieve_cap: usize) {
131:     let p = Program::parse(source).unwrap();
132:     for item in &p.requests {
133:         match &item.request {
134:             Request::Prevent { world, target, using, .. } => {
135:                 let got = ask::prevent(&p, *world, target, using, Pick::All, &limits());
136:                 assert!(got.status.is_complete());
137:                 if got.already || got.impossible {
138:                     continue;
139:                 }
140:                 assert_eq!(as_sets(&got.solutions), brute_prevent(&p, *world, target, using), "line {}", item.line);
141:             }
142:             Request::Achieve { world, target, using, .. } => {
143:                 let got = ask::achieve(&p, *world, target, using, Pick::All, &limits());
144:                 assert!(got.status.is_complete());
145:                 if got.already || got.impossible {
146:                     continue;
147:                 }
148:                 assert!(got.solutions.iter().all(|s| s.len() <= achieve_cap));
149:                 assert_eq!(as_sets(&got.solutions), brute_achieve(&p, *world, target, using, achieve_cap), "line {}", item.line);
150:             }
151:             Request::Why { world, target, .. } => {
152:                 // Each reason suffices on its own, and dropping any one fact breaks it.
153:                 let got = ask::why(&p, *world, target, Pick::All, &limits());
154:                 for reason in &got.reasons {
155:                     let set: BTreeSet<Fact> = reason.facts.iter().cloned().collect();
156:                     assert!(close(&p, *world, &set, &limits()).holds(target));
157:                     for fact in &set {
158:                         let mut smaller = set.clone();
159:                         smaller.remove(fact);
160:                         assert!(!close(&p, *world, &smaller, &limits()).holds(target));
161:                     }
```


### Lean scope and assumptions: `lean/MuoveProofs.lean:1–46`

```text
1: /-!
2: # The claims `prevent`, `achieve`, `diff` and must-sentences rest on
3: 
4: Status: written alongside the Rust engine but NOT yet machine-checked (the
5: Lean toolchain couldn't be downloaded where this was written). Run
6: `lake build` in this directory to check it. No Mathlib; core Lean only.
7: 
8: Setting: facts are any type `α`, a set of facts is a predicate, and whether a
9: fixed conclusion follows is any *monotone* predicate on fact sets. Positive
10: rules (no negation) are monotone: adding facts never removes a conclusion.
11: That one property makes the answers below exact. It is also exactly what
12: negation breaks, which is why negation needs an explicit scope before
13: `prevent` and `achieve` can stay honest.
14: 
15: What is proved here is the characterisation the engine relies on, not the
16: Rust implementation itself. The Rust tests cross-check the implementation
17: against brute-force search.
18: -/
19: 
20: namespace Muove
21: 
22: /-- A set of facts. -/
23: abbrev Facts (α : Type) : Type := α → Prop
24: 
25: variable {α : Type}
26: 
27: abbrev Within (A B : Facts α) : Prop := ∀ x, A x → B x
28: abbrev without (B W : Facts α) : Facts α := fun x => B x ∧ ¬ W x
29: abbrev adding (B A : Facts α) : Facts α := fun x => B x ∨ A x
30: 
31: /-- Whether one fixed conclusion follows from a set of facts. -/
32: structure Entailment (α : Type) where
33:   holds : Facts α → Prop
34:   mono : ∀ {A B : Facts α}, Within A B → holds A → holds B
35: 
36: /-- A reason: a set of facts that suffices, with no smaller sufficient set inside it. -/
37: abbrev IsReason (E : Entailment α) (R : Facts α) : Prop :=
38:   E.holds R ∧ ∀ R', Within R' R → E.holds R' → Within R R'
39: 
40: /-- Every sufficient set contains a reason. True for finite programs; it is what
41: lets the engine enumerate reasons instead of searching all subsets. -/
42: abbrev ReasonsComplete (E : Entailment α) : Prop :=
43:   ∀ A, E.holds A → ∃ R, IsReason E R ∧ Within R A
44: 
45: /-- `W` touches every reason that lies within `B`. -/
46: abbrev HitsEveryReason (E : Entailment α) (B W : Facts α) : Prop :=
```
