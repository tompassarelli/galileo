/-!
# The claims `prevent`, `achieve`, `diff` and must-sentences rest on

Status: written alongside the Rust engine but NOT yet machine-checked (the
Lean toolchain couldn't be downloaded where this was written). Run
`lake build` in this directory to check it. No Mathlib; core Lean only.

Setting: facts are any type `α`, a set of facts is a predicate, and whether a
fixed conclusion follows is any *monotone* predicate on fact sets. Positive
rules (no negation) are monotone: adding facts never removes a conclusion.
That one property makes the answers below exact. It is also exactly what
negation breaks, which is why negation needs an explicit scope before
`prevent` and `achieve` can stay honest.

What is proved here is the characterisation the engine relies on, not the
Rust implementation itself. The Rust tests cross-check the implementation
against brute-force search.
-/

namespace Muove

/-- A set of facts. -/
abbrev Facts (α : Type) : Type := α → Prop

variable {α : Type}

abbrev Within (A B : Facts α) : Prop := ∀ x, A x → B x
abbrev without (B W : Facts α) : Facts α := fun x => B x ∧ ¬ W x
abbrev adding (B A : Facts α) : Facts α := fun x => B x ∨ A x

/-- Whether one fixed conclusion follows from a set of facts. -/
structure Entailment (α : Type) where
  holds : Facts α → Prop
  mono : ∀ {A B : Facts α}, Within A B → holds A → holds B

/-- A reason: a set of facts that suffices, with no smaller sufficient set inside it. -/
abbrev IsReason (E : Entailment α) (R : Facts α) : Prop :=
  E.holds R ∧ ∀ R', Within R' R → E.holds R' → Within R R'

/-- Every sufficient set contains a reason. True for finite programs; it is what
lets the engine enumerate reasons instead of searching all subsets. -/
abbrev ReasonsComplete (E : Entailment α) : Prop :=
  ∀ A, E.holds A → ∃ R, IsReason E R ∧ Within R A

/-- `W` touches every reason that lies within `B`. -/
abbrev HitsEveryReason (E : Entailment α) (B W : Facts α) : Prop :=
  ∀ R, IsReason E R → Within R B → ∃ x, R x ∧ W x

/-- **Prevent.** Withdrawing `W` from `B` makes the conclusion false exactly
when `W` removes at least one fact from every reason. -/
theorem prevent_iff (E : Entailment α) (hc : ReasonsComplete E) (B W : Facts α) :
    ¬ E.holds (without B W) ↔ HitsEveryReason E B W := by
  constructor
  · intro hnot R hR hRB
    apply Classical.byContradiction
    intro hmiss
    apply hnot
    refine E.mono ?_ hR.1
    intro x hx
    exact ⟨hRB x hx, fun hw => hmiss ⟨x, hx, hw⟩⟩
  · intro hhit hholds
    obtain ⟨R, hR, hRsub⟩ := hc _ hholds
    obtain ⟨x, hx, hw⟩ := hhit R hR (fun y hy => (hRsub y hy).1)
    exact (hRsub x hx).2 hw

/-- **Minimality carries over.** The minimal preventing withdrawals are exactly
the minimal sets hitting every reason, which the engine computes with Berge's
hitting-set algorithm. -/
theorem minimal_prevention_iff (E : Entailment α) (hc : ReasonsComplete E) (B W : Facts α) :
    (¬ E.holds (without B W) ∧
        ∀ V, Within V W → ¬ E.holds (without B V) → Within W V) ↔
      (HitsEveryReason E B W ∧
        ∀ V, Within V W → HitsEveryReason E B V → Within W V) := by
  constructor
  · intro ⟨hW, hmin⟩
    exact ⟨(prevent_iff E hc B W).1 hW,
      fun V hVW hV => hmin V hVW ((prevent_iff E hc B V).2 hV)⟩
  · intro ⟨hW, hmin⟩
    exact ⟨(prevent_iff E hc B W).2 hW,
      fun V hVW hV => hmin V hVW ((prevent_iff E hc B V).1 hV)⟩

/-- **Achieve.** Adding `A` (drawn from the allowed candidates `C`) makes the
conclusion true exactly when some reason, found in the world where every
candidate has been added, needs nothing outside `B` except facts in `A`.
So the minimal additions are the minimal "reason minus what's already
stated" sets: the engine computes reasons once in that world instead of
searching subsets of candidates. -/
theorem achieve_iff (E : Entailment α) (hc : ReasonsComplete E)
    (B C A : Facts α) (hA : Within A C) :
    E.holds (adding B A) ↔
      ∃ R, IsReason E R ∧ Within R (adding B C) ∧ ∀ x, R x → ¬ B x → A x := by
  constructor
  · intro h
    obtain ⟨R, hR, hRsub⟩ := hc _ h
    refine ⟨R, hR, ?_, ?_⟩
    · intro x hx
      rcases hRsub x hx with hb | ha
      · exact Or.inl hb
      · exact Or.inr (hA x ha)
    · intro x hx hnb
      rcases hRsub x hx with hb | ha
      · exact absurd hb hnb
      · exact ha
  · intro ⟨R, hR, _, hnew⟩
    refine E.mono ?_ hR.1
    intro x hx
    rcases Classical.em (B x) with hb | hnb
    · exact Or.inl hb
    · exact Or.inr (hnew x hx hnb)

/-- **Diff, lost conclusions.** After withdrawing `W`, a conclusion that held
is lost exactly when every one of its reasons used a withdrawn fact. -/
theorem lost_iff (E : Entailment α) (hc : ReasonsComplete E) (B W : Facts α) :
    (E.holds B ∧ ¬ E.holds (without B W)) ↔ (E.holds B ∧ HitsEveryReason E B W) := by
  constructor
  · intro ⟨h, hn⟩
    exact ⟨h, (prevent_iff E hc B W).1 hn⟩
  · intro ⟨h, hhit⟩
    exact ⟨h, (prevent_iff E hc B W).2 hhit⟩

/-- **Diff, kept reasons.** A reason survives the withdrawal exactly when it
avoids every withdrawn fact. -/
theorem reason_survives (B W R : Facts α) (hRB : Within R B) :
    Within R (without B W) ↔ ∀ x, R x → ¬ W x := by
  constructor
  · intro h x hx
    exact (h x hx).2
  · intro h x hx
    exact ⟨hRB x hx, h x hx⟩

/-! ## Must-sentences

The two reason-level must-sentences, for stated facts. (`requires` over a
concluded sentence is checked by blocking it during evaluation, which is
"no derivation passes through it" by construction.) -/

/-- **`requires`, for a stated fact.** Every reason uses `t` exactly when the
conclusion fails once `t` is withdrawn. -/
theorem requires_iff (E : Entailment α) (hc : ReasonsComplete E) (B : Facts α) (t : α) :
    ¬ E.holds (without B (fun x => x = t)) ↔
      ∀ R, IsReason E R → Within R B → R t := by
  constructor
  · intro h R hR hRB
    obtain ⟨x, hx, hxt⟩ := (prevent_iff E hc B (fun x => x = t)).1 h R hR hRB
    exact (show x = t from hxt) ▸ hx
  · intro h
    apply (prevent_iff E hc B (fun x => x = t)).2
    intro R hR hRB
    exact ⟨t, h R hR hRB, rfl⟩

/-- **No single point of failure.** No single stated fact matching `P` stops
the conclusion exactly when, for each such fact, some reason avoids it. -/
theorem no_single_point_iff (E : Entailment α) (hc : ReasonsComplete E) (B P : Facts α) :
    (∀ t, P t → B t → E.holds (without B (fun x => x = t))) ↔
      (∀ t, P t → B t → ∃ R, IsReason E R ∧ Within R B ∧ ¬ R t) := by
  constructor
  · intro h t hP hB
    obtain ⟨R, hR, hRsub⟩ := hc _ (h t hP hB)
    refine ⟨R, hR, fun y hy => (hRsub y hy).1, fun hRt => ?_⟩
    exact (hRsub t hRt).2 rfl
  · intro h t hP hB
    obtain ⟨R, hR, hRB, hRt⟩ := h t hP hB
    refine E.mono ?_ hR.1
    intro x hx
    refine ⟨hRB x hx, fun hxt => ?_⟩
    exact hRt ((show x = t from hxt) ▸ hx)

end Muove
