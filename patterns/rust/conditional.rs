// DOF-SPEC v0.11 §4.10 — robust selection over the declared readings.
// Reference port; mirrors patterns/python/calculus_core.py (`conditional_vector`,
// `conditional_vectors`, `robust_admissible`, `least_favourable`,
// `admissible_under`, `hypothesis_conflict`, `robust_reversible`,
// `select_conditional`) and patterns/go/conditional.go.
//
// The shape of the rule, in one place:
//
//   - the **candidate set is the same for every hypothesis**; only the
//     projection differs. A reading never adds or removes an option;
//   - admissibility is a **universally quantified conjunction** over
//     `H_plausible` — `viable ∧ resources_ok ∧ D1 = D2 = D3 = 0` under *every*
//     plausible reading;
//   - the ordering key is the **least-favourable** conditional delta
//     `min_h NetDelta(o | h)`, never a maximum: the greatest conditional delta
//     would be exactly the optimistic aggregation §4.10 refuses;
//   - §4.5 keys 3 and 4 then apply unchanged, key 3 read **robustly**
//     (reversible under every plausible reading) because key 2 has already
//     charged the closure at its worst;
//   - an empty robust candidate set yields `none`. There is **no fallback to
//     admissible support**: ranking the survivors of an inadmissible set would
//     be the compensation Axiom 3 forbids.

use std::collections::{BTreeMap, HashMap};

use crate::dof_core::{
    ActionOption, CandidateVector, DofCalculusCore, ObservationContext, SystemStateMatrix,
};
use crate::hypothesis::{coverage_of, Hypothesis, HypothesisSet};
use crate::measurement::Rate;

/// §6.3: one candidate's vector **under one reading**.
///
/// It carries the whole admissibility predicate — `viable`, `resources_ok` and
/// the three dimensions — so that §4.10's aggregation reads one structure and not
/// three.
#[derive(Clone, Debug)]
pub struct ConditionalVector {
    pub option_id: String,
    pub hypothesis_id: String,
    pub d1: usize,
    pub d2: usize,
    pub d3: usize,
    pub net_delta: f64,
    pub reversible: bool,
    pub viable: bool,
    pub resources_ok: bool,
}

impl ConditionalVector {
    /// One reading's vector: the §4.5 keys computed under that reading, plus the
    /// temporal condition of §4.8b and the financial one of §4.8 — the latter two
    /// are per-option and do not vary with the reading, but they belong to the
    /// predicate, and a predicate assembled in two places drifts.
    pub fn from_vector(
        vector: &CandidateVector,
        hypothesis_id: &str,
        viable: bool,
        resources_ok: bool,
    ) -> ConditionalVector {
        ConditionalVector {
            option_id: vector.option_id.clone(),
            hypothesis_id: hypothesis_id.to_string(),
            d1: vector.d1,
            d2: vector.d2,
            d3: vector.d3,
            net_delta: vector.net_delta,
            reversible: vector.reversible,
            viable,
            resources_ok,
        }
    }

    /// §4.10.1: the admissibility predicate of one reading's vector.
    pub fn barred(&self) -> bool {
        !self.viable || !self.resources_ok || self.d1 > 0 || self.d2 > 0 || self.d3 > 0
    }
}

/// §6.3: the decision payload — everything the report needs to show what was
/// decided and under which reading.
///
/// `Default` is the empty selection, and it is a real value rather than a
/// placeholder: with no observation context there is no reading to decide under, and
/// the cycle then stays put (§4.10.6 — no `H`, no readings). Deriving it here keeps
/// the two ports that need it from spelling their own empty payload.
#[derive(Clone, Debug, Default)]
pub struct ConditionalSelection {
    pub conditional_vectors: BTreeMap<String, BTreeMap<String, ConditionalVector>>,
    pub admissible_under: BTreeMap<String, BTreeMap<String, bool>>,
    pub hypothesis_conflict: bool,
    pub robust_admissible: Vec<String>,
    pub net_delta_robust: BTreeMap<String, f64>,
}

impl DofCalculusCore {
    /// §4.5's full vector of one candidate under one reading.
    ///
    /// The state passed in is that reading's own state, and the context is the
    /// same observation read under that reading's DoF (§4.9): every quantity of
    /// §4.1–§4.9 is conditional (§4.10). The financial condition is evaluated with
    /// the **same declared mandate and observed rates** the cycle uses (§4.8) —
    /// passing none of them would make every option with a `requires` look
    /// insolvent, because a deficit could never be converted.
    pub fn conditional_vector_of(
        &self,
        state: &SystemStateMatrix,
        option: &ActionOption,
        ctx: Option<&ObservationContext>,
        hypothesis_id: &str,
        groups: Option<&Vec<Vec<String>>>,
        rates: Option<&BTreeMap<String, Rate>>,
        weights: Option<&BTreeMap<String, f64>>,
        cap: Option<f64>,
    ) -> ConditionalVector {
        let mut dofs: HashMap<String, f64> = HashMap::new();
        for (eid, entity) in state.entities.iter() {
            dofs.insert(eid.clone(), entity.current_dof);
        }
        // §4.9/§4.10: the context may be **absent**, and that is not a refusal. A
        // scene without a graph is still decidable — the verdicts read `undetermined`
        // and are priced by `u(t)` — so an absent context changes what the quantities
        // are, never whether they exist. Both the reference and the Go port compute
        // here; refusing would be an answer the calculus does not support.
        let h_ctx: Option<ObservationContext> = ctx.map(|c| c.with_dof(dofs));
        let current_index = self.calculate_system_dof(state, None, h_ctx.as_ref());
        let viability = self.viability(state, option);
        let plan = self.plan_funding(state, option, groups, rates, weights, cap);
        let vector = self.candidate_vector_for(
            state,
            option,
            h_ctx.as_ref(),
            current_index,
            hypothesis_id,
        );
        ConditionalVector::from_vector(&vector, hypothesis_id, viability.viable, plan.covered)
    }

    /// §6.3: `{option_id: {hypothesis_id: vector}}`.
    pub fn conditional_vectors(
        &self,
        members: &[Hypothesis],
        options: &[ActionOption],
        ctx: Option<&ObservationContext>,
        groups: Option<&Vec<Vec<String>>>,
        rates: Option<&BTreeMap<String, Rate>>,
        weights: Option<&BTreeMap<String, f64>>,
        cap: Option<f64>,
    ) -> BTreeMap<String, BTreeMap<String, ConditionalVector>> {
        let mut out: BTreeMap<String, BTreeMap<String, ConditionalVector>> = BTreeMap::new();
        for option in options.iter() {
            let mut per_h: BTreeMap<String, ConditionalVector> = BTreeMap::new();
            for h in members.iter() {
                per_h.insert(
                    h.id.clone(),
                    self.conditional_vector_of(
                        &h.state,
                        option,
                        ctx,
                        &h.id,
                        groups,
                        rates,
                        weights,
                        cap,
                    ),
                );
            }
            out.insert(option.option_id.clone(), per_h);
        }
        out
    }

    /// §4.5 key 3 under a hypothesis set:
    ///
    /// `reversible_robust(o) ⇔ ∀ h ∈ H_plausible : o.closed[h] = []`
    ///
    /// The direction is not a matter of taste. Key 2 worst-cases the **price** of a
    /// closure (`min_h NetDelta(o | h)`), so a closure invisible at one reading but
    /// real at another is already charged at its worst. If this preference were read
    /// from a single reading — the observed one included — that same closure would
    /// be charged at its worst *and* rewarded as if it did not exist, and the robust
    /// ordering could be reversed by the very closure the worst case exists to
    /// weigh. So the preference is taken over the same set, in the same direction,
    /// as the price. With no hypothesis set this reduces to the flat `closed = []`
    /// of §4.5.
    pub fn robust_reversible(&self, option: &ActionOption, members: &[Hypothesis]) -> bool {
        if members.is_empty() {
            return self.is_reversible(option);
        }
        for h in members.iter() {
            if !option.is_reversible_for(&h.id) {
                return false;
            }
        }
        true
    }

    /// §4.10.1: admissible under **every** plausible hypothesis.
    ///
    /// The predicate is both families of conditions — the temporal one of §4.8b,
    /// the financial one of §4.8 and the three dimensions of §4.5 — and the
    /// aggregation is a universally quantified conjunction. Neither is a maximum
    /// over hypotheses.
    pub fn robust_admissible(
        &self,
        per_h: &BTreeMap<String, ConditionalVector>,
        members: &[Hypothesis],
    ) -> bool {
        for h in members.iter() {
            match per_h.get(&h.id) {
                Some(vector) if !vector.barred() => {}
                _ => return false,
            }
        }
        true
    }

    /// `NetDelta_robust(o) = min_{h ∈ H_plausible} NetDelta(o | h)` (§4.10.2). An
    /// empty set yields 0.0: staying put.
    pub fn least_favourable(
        &self,
        per_h: &BTreeMap<String, ConditionalVector>,
        members: &[Hypothesis],
    ) -> f64 {
        let mut values: Vec<f64> = Vec::new();
        for h in members.iter() {
            if let Some(v) = per_h.get(&h.id) {
                values.push(v.net_delta);
            }
        }
        if values.is_empty() {
            return 0.0;
        }
        let mut least = values[0];
        for v in values.iter() {
            if *v < least {
                least = *v;
            }
        }
        least
    }

    /// §6.3: per reading, whether the option is admissible.
    pub fn admissible_under(
        &self,
        per_h: &BTreeMap<String, ConditionalVector>,
        members: &[Hypothesis],
    ) -> BTreeMap<String, bool> {
        let mut out: BTreeMap<String, bool> = BTreeMap::new();
        for h in members.iter() {
            let flag = match per_h.get(&h.id) {
                Some(vector) => !vector.barred(),
                None => false,
            };
            out.insert(h.id.clone(), flag);
        }
        out
    }

    /// §4.10.5: an unresolved conflict the report MUST surface.
    ///
    /// Fires when `H_plausible` has more than one element and either (a) the robust
    /// **candidate** set is empty, or (b) some candidate is admissible under some
    /// plausible hypotheses and inadmissible under others — stated over the **whole
    /// predicate**, so an option executable under one reading and physically
    /// impossible under another is as much a conflict as one that destroys a counted
    /// entity under one reading only.
    ///
    /// A difference that does **not** move admissibility — the same bar, of
    /// different magnitude, under different readings — is **not** a conflict: the
    /// choice is the same under both readings, the conditional vectors are listed
    /// anyway, and the least-favourable key already resolves it.
    pub fn hypothesis_conflict(
        &self,
        per_h_all: &BTreeMap<String, BTreeMap<String, ConditionalVector>>,
        members: &[Hypothesis],
        robust_ids: &[String],
    ) -> bool {
        if members.len() <= 1 {
            return false;
        }
        if robust_ids.is_empty() {
            return true;
        }
        for (option_id, per_h) in per_h_all.iter() {
            let flags = self.admissible_under(per_h, members);
            let mut any_admissible = false;
            let mut any_barred = false;
            for v in flags.values() {
                if *v {
                    any_admissible = true;
                } else {
                    any_barred = true;
                }
            }
            if any_admissible && any_barred {
                return true;
            }
            if any_admissible && !robust_ids.iter().any(|id| id == option_id) {
                return true;
            }
        }
        false
    }

    /// §4.10: robust selection over the declared readings.
    ///
    /// The candidate set is the same for every hypothesis; only the projection
    /// differs. Among the robustly admissible candidates take the greatest
    /// `NetDelta_robust`, then apply §4.5 keys 3 and 4 (reversibility — read
    /// robustly — then the comparison origin), then the lexicographically smallest
    /// `option_id`.
    ///
    /// **There is no fallback to admissible support.** An empty robust candidate set
    /// yields `None` — every proposed action is barred under at least one plausible
    /// reading, and ranking the survivors of an inadmissible set would be the
    /// compensation Axiom 3 forbids (§4.10.4).
    pub fn select_conditional(
        &self,
        state: &SystemStateMatrix,
        options: &[ActionOption],
        members: &[Hypothesis],
        ctx: Option<&ObservationContext>,
        groups: Option<&Vec<Vec<String>>>,
        rates: Option<&BTreeMap<String, Rate>>,
        weights: Option<&BTreeMap<String, f64>>,
        cap: Option<f64>,
    ) -> (Option<ActionOption>, ConditionalSelection) {
        let _ = state;
        let per_h_all =
            self.conditional_vectors(members, options, ctx, groups, rates, weights, cap);

        let mut robust: Vec<&ActionOption> = Vec::new();
        for option in options.iter() {
            let empty: BTreeMap<String, ConditionalVector> = BTreeMap::new();
            let per_h = per_h_all.get(&option.option_id).unwrap_or(&empty);
            if self.robust_admissible(per_h, members) {
                robust.push(option);
            }
        }
        let robust_ids: Vec<String> = robust.iter().map(|o| o.option_id.clone()).collect();

        let mut selection = ConditionalSelection {
            conditional_vectors: per_h_all.clone(),
            admissible_under: BTreeMap::new(),
            hypothesis_conflict: self.hypothesis_conflict(&per_h_all, members, &robust_ids),
            robust_admissible: robust_ids.clone(),
            net_delta_robust: BTreeMap::new(),
        };
        for option in options.iter() {
            let empty: BTreeMap<String, ConditionalVector> = BTreeMap::new();
            let per_h = per_h_all.get(&option.option_id).unwrap_or(&empty);
            selection
                .admissible_under
                .insert(option.option_id.clone(), self.admissible_under(per_h, members));
            selection.net_delta_robust.insert(
                option.option_id.clone(),
                self.least_favourable(per_h, members),
            );
        }
        if robust.is_empty() {
            return (None, selection);
        }

        // §4.5 key 2: the greatest least-favourable delta, ties by tolerance.
        let mut best = selection.net_delta_robust[&robust[0].option_id];
        for option in robust.iter() {
            let v = selection.net_delta_robust[&option.option_id];
            if v > best {
                best = v;
            }
        }
        let mut survivors: Vec<&ActionOption> = robust
            .iter()
            .copied()
            .filter(|o| (selection.net_delta_robust[&o.option_id] - best).abs() <= Self::NET_DELTA_TOLERANCE)
            .collect();

        // §4.5 key 3, under its **robust** reading (§4.10.2): revert only if some
        // survivor is reversible under every plausible reading.
        if survivors.iter().any(|o| self.robust_reversible(o, members)) {
            survivors.retain(|o| self.robust_reversible(o, members));
        }

        // §4.5 key 4: the survivor must beat the comparison origin.
        survivors.retain(|o| selection.net_delta_robust[&o.option_id] > 0.0);
        if survivors.is_empty() {
            return (None, selection);
        }

        // §4.5 final key: the lexicographically smallest `option_id`.
        let mut chosen = survivors[0];
        for option in survivors.iter().skip(1) {
            if option.option_id < chosen.option_id {
                chosen = option;
            }
        }
        (Some(chosen.clone()), selection)
    }
}

/// §6.2: the declared coverage of a set, reported next to the decision it
/// produced. An absent claim reads as `"partial"` — the cautious default — and is
/// never inferred to be complete.
pub fn hypothesis_coverage(hset: Option<&HypothesisSet>) -> String {
    coverage_of(hset)
}