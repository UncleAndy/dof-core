// DOF-Core calculus kernel (Rust port).
// Mirrors patterns/calculus_core.py: pure Nash evaluation index (sum of ln(DoF)),
// the `calc` calculation set, Collapse-Source isolation, Delta-T-aware selection,
// and the Proof-of-Implementation audit report (DOF-SPEC §6).
//
// Axioms: Axiom 1 (system AND its constituent entities); Axiom 3 (no trading one
// entity's collapse for another's gain); Axiom 5 (prefer reversible actions; never
// assume unknown possibilities have zero DoF).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::conditional::{ConditionalSelection, ConditionalVector};
use crate::hypothesis::Hypothesis;
use crate::measurement::{
    psi_var, EntityMeasurement, LensTerm, MeasurementDeclaration, MandateValue, PsiReference, Rate,
};
use crate::world_graph::{q6, ClosedRef, Verdict, WorldGraph};

/// The observation a cycle is decided over (§3.5, §4.9).
///
/// Deliberately NOT a state field: the world graph is a Perception artifact supplied
/// to the cycle, exactly as the derived groups and the observed rates are (§4.8).
/// Without it every verdict is `undetermined`, which means no entity at a known zero
/// §3.2a (v0.9.1): a resource as an observable quantity with metadata.
#[derive(Clone, Debug)]
pub struct ResourceObservation {
    pub value: Option<f64>,
    pub unit: String,
    pub scale: f64,
    pub source: String,
    pub last_measured_at: f64,
    pub aging_time: f64,
    pub estimated: Option<f64>,
    pub estimation_source: Vec<String>,
}

impl Default for ResourceObservation {
    fn default() -> Self {
        ResourceObservation {
            value: None,
            unit: String::new(),
            scale: 1.0,
            source: String::new(),
            last_measured_at: 0.0,
            aging_time: 0.0,
            estimated: None,
            estimation_source: Vec::new(),
        }
    }
}

impl PartialEq for ResourceObservation {
    fn eq(&self, other: &Self) -> bool {
        const EPS: f64 = 1e-12;
        self.value == other.value
            && self.unit == other.unit
            && (self.scale - other.scale).abs() < EPS
            && self.source == other.source
            && (self.last_measured_at - other.last_measured_at).abs() < EPS
            && (self.aging_time - other.aging_time).abs() < EPS
            && self.estimated == other.estimated
            && self.estimation_source == other.estimation_source
    }
}

impl ResourceObservation {
    /// §4.8 (v0.9.1): resolve a resource's usable value for the gate.
    pub fn resource_value(&self, use_estimated: bool) -> f64 {
        if let Some(v) = self.value {
            v
        } else if use_estimated {
            self.estimated.unwrap_or(0.0)
        } else {
            0.0
        }
    }

    /// Check if the observation is stale.
    pub fn is_stale(&self, now: f64) -> bool {
        if self.aging_time <= 0.0 {
            return false;
        }
        (now - self.last_measured_at) > self.aging_time
    }
}

/// Compare two resource maps (used in reports).
pub fn resource_map_equal(a: &BTreeMap<String, ResourceObservation>, b: &BTreeMap<String, ResourceObservation>) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (key, val) in a {
        match b.get(key) {
            Some(other) => {
                if val != other {
                    return false;
                }
            }
            None => return false,
        }
    }
    true
}

#[derive(Clone, Debug)]
pub struct ObservationContext {
    pub world: WorldGraph,
    pub means_class: Vec<String>,
    pub t_rec: BTreeMap<String, f64>,
    pub counting_horizon_mks: Option<f64>,
    pub observation_digest: String,
    /// §3.6/§4.9/§4.10 (v0.11): `DoF(X | h)` — the per-entity degrees of freedom
    /// **under one reading**, which the §4.9 verdict consumes. `None` means "read
    /// the graph's own value", which is what the observed reading does; a
    /// hypothesis reading supplies its own numbers here. Structural inputs — `G`,
    /// the paths and their admissibility, `M(S)`, `T_rec(X)` — stay shared: only
    /// the DoF is conditional.
    pub dof_override: Option<HashMap<String, f64>>,
}

impl ObservationContext {
    /// The reading's `DoF(X | h)` for one entity, or `None` when this context
    /// carries no reading (§4.9).
    pub fn dof_override_of(&self, entity_id: &str) -> Option<f64> {
        self.dof_override
            .as_ref()
            .and_then(|m| m.get(entity_id).copied())
    }

    /// §3.6/§4.9: the same observation, read under one hypothesis's DoF.
    ///
    /// Every shared input is carried over untouched — the graph, `M(S)`, the
    /// horizons, the observation digest — because §3.6 shares them by construction:
    /// only `DoF(X | h)` differs between readings, and only it is replaced here.
    pub fn with_dof(&self, dofs: HashMap<String, f64>) -> ObservationContext {
        let mut clone = self.clone();
        clone.dof_override = Some(dofs);
        clone
    }

    pub fn horizon(&self, entity_id: &str) -> Option<f64> {
        self.t_rec.get(entity_id).copied()
    }
    /// §4.9: the verdict **under this reading**. The observed reading passes no
    /// override and is therefore byte-identical to the `v0.9.1` call — the
    /// conditional form is the general one and the observed form is its instance,
    /// not a second rule (§3.6, §7 п.30).
    pub fn verdict(&self, entity_id: &str) -> String {
        self.world
            .verdict_with_dof(
                entity_id,
                &self.means_class,
                self.horizon(entity_id),
                self.dof_override_of(entity_id),
            )
            .verdict
    }
    pub fn v_before(&self, entity_id: &str) -> usize {
        self.world.v_count(entity_id, &self.means_class, self.counting_horizon_mks)
    }
    pub fn v_after_closure(&self, entity_id: &str, closed: &[ClosedRef]) -> usize {
        if closed.is_empty() {
            return self.v_before(entity_id);
        }
        self.world
            .with_closed(closed)
            .v_count(entity_id, &self.means_class, self.counting_horizon_mks)
    }
}

#[derive(Clone, Debug)]
pub struct EntityState {
    pub entity_id: String,
    pub is_autonomous: bool,
    pub agency_index: f64,
    pub current_dof: f64,
    pub is_collapse_source: bool,
    /// Whether `current_dof` is a known value; unknown DoF is never treated as 0 (Axiom 5).
    pub dof_known: bool,
    pub time_to_collapse_mks: f64,
    /// Port-level extension (not a §3.1 field): the measurement that produced
    /// `current_dof`, kept so the audit can show the per-lens terms (§6.1).
    pub measurement: Option<EntityMeasurement>,
}

impl EntityState {
    pub fn new(
        entity_id: String,
        is_autonomous: bool,
        agency_index: f64,
        current_dof: f64,
        is_collapse_source: bool,
        time_to_collapse_mks: f64,
    ) -> Self {
        EntityState {
            entity_id,
            is_autonomous,
            agency_index,
            current_dof,
            is_collapse_source,
            dof_known: true,
            time_to_collapse_mks,
            measurement: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SystemStateMatrix {
    pub global_time_to_collapse_mks: f64,
    pub context_switch_cost: f64,
    pub entities: HashMap<String, EntityState>,
    pub psi: Option<PsiReference>,
    /// §3.2 (v0.9.1): resources per resource_id as ResourceObservation.
    pub resources: HashMap<String, ResourceObservation>,
    /// §3.2b (v0.9.1): τ as ResourceObservation.
    pub tau: Option<ResourceObservation>,
    /// §3.2b (v0.11): the active individual deadlines τ is derived from. τ is
    /// `null` when **any** active deadline is unmeasured — never the minimum over
    /// the measured ones alone, and never `0.0`.
    pub deadlines: BTreeMap<String, Option<f64>>,
    /// §4.7 (v0.11): the declared measurement durations `t_m`, `t_v` per lens.
    /// Hashed ruler content: two states differing only in `t_m` produce different
    /// `ruler_digest` (§3.4.1).
    pub measurement_durations: BTreeMap<String, BTreeMap<String, f64>>,
    /// §4.7 (v0.11): the declared **schedule** `t` per lens — when the measurement
    /// is planned to happen. An undeclared schedule reads as `t = 0` (`u₀`).
    pub measurement_schedule: BTreeMap<String, f64>,
}

/// §4.7: does this option resolve the named resource — is it a measurement *of* it?
///
/// Stated once: the viability condition (§4.8b), the temporal derivation and the
/// orchestrator's gate all ask the same question, and a second copy of it is how
/// "measure τ first" drifts into "measure anything first".
pub fn option_discovers(option: &ActionOption, resource: &str) -> bool {
    option.discovers.iter().any(|d| d == resource)
}

/// §3.2b: τ as the calculus reads it — from the **resource map**, signed.
///
/// `state.tau` is the `tau` `ResourceObservation`. When it is absent or its
/// `value` is `null`, τ is **unmeasured** (`None`), never the minimum over the
/// measured deadlines alone and never `0.0`: an unmeasured active deadline may be
/// the most urgent one, so acting on the budget the measured ones support is
/// acting on a budget the state does not establish, and writing `0.0` invents a
/// catastrophe (§3.1, §3.2b).
///
/// A **negative** value is a deadline that has passed, `|τ|` ago. It is a *known*
/// state and MUST NOT be clamped to `0.0` or replaced by `None`, which means
/// unmeasured only (§3.2b, §4.8b).
///
/// The deprecated mirror is read **only** when the state carries no resource-map τ
/// and no deadline set, so it can never override a measurement (§3.2b).
pub fn tau_of(state: &SystemStateMatrix) -> Option<f64> {
    if let Some(obs) = state.tau.as_ref() {
        return obs.value;
    }
    if !state.deadlines.is_empty() {
        let mut best = f64::INFINITY;
        for v in state.deadlines.values() {
            match v {
                Some(x) => {
                    if *x < best {
                        best = *x;
                    }
                }
                None => return None,
            }
        }
        return Some(best);
    }
    Some(state.global_time_to_collapse_mks)
}

/// §5's `t*` for a measurement of `t_meas_mks`: `None` when τ is unmeasured — an
/// unknown budget is not a closed window, and the strict `t* > 0` rule of §5 is
/// not applied to it (a τ measurement is governed by §4.8b instead).
pub fn measurement_window(state: &SystemStateMatrix, t_meas_mks: f64) -> Option<f64> {
    tau_of(state).map(|tau| tau - t_meas_mks)
}

#[derive(Clone, Debug)]
pub struct ActionOption {
    pub option_id: String,
    pub description: String,
    pub projected_dof_delta: HashMap<String, f64>,
    pub is_reversible: bool,
    /// Estimated execution time in microseconds (DOF-SPEC §3.3).
    pub estimated_duration_mks: f64,
    /// §3.3 (v0.6): what the option draws from the acting agent, attributed to
    /// the entity whose transitions consume it. Negative = consumption, positive
    /// = production; `energy` must be present (as 0.0) for every entity named in
    /// `projected_dof_delta`.
    pub projected_resource_delta: HashMap<String, HashMap<String, f64>>,
    /// §3.3/§4.4 (v0.7): the transitions this option CLOSES — the acts and means
    /// that cease to exist once it executes. `is_reversible` is DERIVED from this
    /// list (true exactly when it is empty) and is kept only as a reported field: a
    /// label that could be set to dodge the price is not a rule.
    pub closed: Vec<ClosedRef>,
    /// The graph act implementing this option.
    pub act_id: String,
    /// §3.3 (v0.11): the same declaration in the **per-hypothesis** form —
    /// `{hypothesis_id: {entity_id: delta}}`. Empty means the option declares the
    /// flat form. An option that fills **both** forms is **invalid** input, never a
    /// merge: the two forms are different claims about the same effect, and an
    /// option that makes both cannot be scored under either (§3.3, §10(B)).
    pub projected_by_hypothesis: HashMap<String, HashMap<String, f64>>,
    /// §4.4 (v0.11): the closure list in the per-hypothesis form, with the same
    /// two-forms-never-mixed rule.
    pub closed_by_hypothesis: HashMap<String, Vec<ClosedRef>>,
    /// §3.3 (v0.9.1): projected change of τ (time-to-collapse) caused by this
    /// option. §3.3 (v0.11): `None` means **not computable** — which happens
    /// exactly when τ is unknown — and is admissible only for an act that resolves
    /// τ.
    pub projected_tau_delta: Option<f64>,
    /// §3.3 (v0.11): the value the option expects `tau` to hold AFTER it executes.
    /// Present iff `discovers` names "tau". MAY be negative (§3.2b).
    pub projected_tau_value: Option<f64>,
    /// §3.3 (v0.9.1): resources whose value becomes known after this option executes.
    pub discovers: Vec<String>,
    /// §3.3 (v0.9.1): resources needed for gate checks.
    pub requires: Vec<String>,
}

impl ActionOption {
    pub fn new(
        option_id: String,
        description: String,
        projected_dof_delta: HashMap<String, f64>,
        is_reversible: bool,
        estimated_duration_mks: f64,
    ) -> Self {
        ActionOption {
            option_id,
            description,
            projected_dof_delta,
            is_reversible,
            estimated_duration_mks,
            projected_resource_delta: HashMap::new(),
            closed: Vec::new(),
            act_id: String::new(),
            projected_by_hypothesis: HashMap::new(),
            closed_by_hypothesis: HashMap::new(),
            projected_tau_delta: None,
            projected_tau_value: None,
            discovers: Vec::new(),
            requires: Vec::new(),
        }
    }

    // ------------------------------------------------------------ §3.3 forms
    //
    // `v0.11` gave an option's declared effect **two forms, never mixed within one
    // option** (§3.3, §4.4):
    //
    //   * **flat** — the effect is the same under every reading, which is what an
    //     option asserts when it says nothing about the causal reading;
    //   * **per_hypothesis** — the entry for `h` is used under `h`, and an entity
    //     the entry does not list takes `0.0` under `h`.
    //
    // The form is decided in **one** place. Reading the declared content directly
    // at each call site is how the two forms drift apart: a port that reads the
    // flat map "by default" silently scores a per-hypothesis option as if it had
    // one effect, and the worst-case operators of §4.10 then minimise over
    // readings of a number that never depended on them.

    /// `"flat"` | `"per_hypothesis"` | `"invalid"` (§3.3, §10(B)).
    pub fn projection_form(&self) -> &'static str {
        let flat = !self.projected_dof_delta.is_empty();
        let nested = !self.projected_by_hypothesis.is_empty();
        if flat && !nested {
            return "flat";
        }
        if nested && !flat {
            return "per_hypothesis";
        }
        // Both forms filled: **invalid**, never a merge (§3.3, §10(B)). Reading
        // the pair as flat would score a per-hypothesis option as if it had one
        // effect under every reading, which is precisely the drift this decision
        // exists to prevent.
        if flat && nested {
            return "invalid";
        }
        // An option declaring no delta at all is the flat form: the empty map is a
        // flat map, and reading it as "invalid" would refuse every baseline.
        "flat"
    }

    /// `"flat"` | `"per_hypothesis"` | `"invalid"` (§4.4, §10(B)).
    pub fn closure_form(&self) -> &'static str {
        let flat = !self.closed.is_empty();
        let nested = !self.closed_by_hypothesis.is_empty();
        if flat && !nested {
            return "flat";
        }
        if nested && !flat {
            return "per_hypothesis";
        }
        // Both forms filled: **invalid** (§4.4). The guards of §4.4 range over
        // every list the option declares, and an option that declares two
        // contradictory closure lists under one reading has not declared a closure.
        if flat && nested {
            return "invalid";
        }
        "flat"
    }

    /// The declared delta of `entity_id` **under the reading `hypothesis_id`**.
    ///
    /// In the per-hypothesis form an entity the reading does not list takes `0.0`:
    /// "not declared under this reading" is a claim of no effect, and a missing
    /// entry is never inherited from another reading.
    pub fn delta_for(&self, hypothesis_id: &str, entity_id: &str) -> f64 {
        if self.projection_form() == "per_hypothesis" {
            return self
                .projected_by_hypothesis
                .get(hypothesis_id)
                .and_then(|m| m.get(entity_id))
                .copied()
                .unwrap_or(0.0);
        }
        self.projected_dof_delta.get(entity_id).copied().unwrap_or(0.0)
    }

    /// The closure list **under the reading `hypothesis_id`** (§4.4).
    pub fn closed_for(&self, hypothesis_id: &str) -> Vec<ClosedRef> {
        if self.closure_form() == "per_hypothesis" {
            return self
                .closed_by_hypothesis
                .get(hypothesis_id)
                .cloned()
                .unwrap_or_default();
        }
        self.closed.clone()
    }

    /// The flat map as declared, for callers that have no reading in hand.
    pub fn flat_delta(&self) -> HashMap<String, f64> {
        self.projected_dof_delta.clone()
    }

    /// The empty string when both forms are consistent, otherwise the reason.
    pub fn forms_consistent(&self) -> String {
        if self.projection_form() == "invalid" {
            return format!(
                "{}: `projected_dof_delta` mixes the flat and per-hypothesis forms (§3.3)",
                self.option_id
            );
        }
        if self.closure_form() == "invalid" {
            return format!(
                "{}: `closed` mixes the flat and per-hypothesis forms (§4.4)",
                self.option_id
            );
        }
        String::new()
    }

    /// §4.5: `is_reversible` **under the reading `hypothesis_id`** — the flag is
    /// derived from that reading's closure list and is never trusted as declared.
    pub fn is_reversible_for(&self, hypothesis_id: &str) -> bool {
        self.closed_for(hypothesis_id).is_empty()
    }

    /// Every closure list the option declares: one for a flat option, one per reading
    /// for a per-hypothesis option (§3.3, §4.4). The declaration guards must hold
    /// under **every** reading, so they range over these lists and never over the
    /// flat field alone: an option whose declaration is well-formed under one reading
    /// and malformed under another is malformed.
    pub fn all_closed_lists(&self) -> Vec<Vec<ClosedRef>> {
        if self.closure_form() != "per_hypothesis" {
            return vec![self.closed.clone()];
        }
        let mut keys: Vec<&String> = self.closed_by_hypothesis.keys().collect();
        keys.sort();
        keys.iter()
            .map(|k| self.closed_by_hypothesis[*k].clone())
            .collect()
    }

    /// Declare what the option closes (§3.3/§4.4). `is_reversible` is derived from
    /// this list, so declaring a closure is what makes an option irreversible — the
    /// flag is reported, never trusted.
    pub fn with_closed(mut self, closed: Vec<ClosedRef>, act_id: &str) -> Self {
        self.is_reversible = closed.is_empty();
        self.closed = closed;
        self.act_id = act_id.to_string();
        self
    }

    /// Declare what the option draws (§3.3). Kept separate so existing callers
    /// of `new` stay valid and an option without a declared draw is an explicit
    /// empty map rather than a missing field.
    pub fn with_draw(
        mut self,
        projected_resource_delta: HashMap<String, HashMap<String, f64>>,
    ) -> Self {
        self.projected_resource_delta = projected_resource_delta;
        self
    }
}

/// §6.1 (v0.7): the recoverability verdict, its witness, and the completeness claim
/// behind it. A `proven_unreachable` verdict without a witness is not a verdict, so
/// the report carries both — and names the observation, because "no path" is only
/// meaningful together with "and the observation was complete for this entity".
#[derive(Clone, Debug, Default)]
pub struct RecoverabilityRow {
    pub verdict: String,
    pub witness: Vec<String>,
    pub horizon_mks: Option<f64>,
    pub observation: String,
    pub admissible_seen: usize,
    pub reason: String,
}

/// One entity row of the audit report.
#[derive(Clone, Debug)]
pub struct EntityReportRow {
    pub entity_id: String,
    pub is_collapse_source: bool,
    pub included_in_sum: bool,
    pub current_dof: f64,
    /// Whether `current_dof` is a known value; unknown DoF is never treated as 0 (Axiom 5).
    pub dof_known: bool,
    pub contribution: f64,
    /// §6.1: why, not only what — one row per lens of the frozen set.
    pub lens_terms: Vec<LensTerm>,
    /// The lens that actually holds this entity back.
    pub binding_lens: Option<String>,
    /// The ε-floor of §4.1 was applied at the entity level, not to one term.
    pub floored: bool,
    /// §4.6 (v0.6): the derived blocks and the derivation behind them, so a
    /// reader can recompute `(c_g, C_g)` from the raw requirements.
    pub blocks: Vec<(f64, f64)>,
    pub derivation: Option<crate::measurement::DerivationInfo>,
    /// §6.1 (v0.7): the recoverability verdict and its witness.
    pub recoverability: RecoverabilityRow,
    // §6.1 (v0.11): the same three facts **per reading**. The lens terms and the
    // binding channel belong to a measurement, so under a declared set there is no
    // shared `ψ` to print and no single verdict to report: the channel that binds
    // and the reading under which the entity can be revived are results, not
    // noise. Empty when the cycle ran on the observed state alone, where the flat
    // fields are the whole answer.
    pub lens_terms_by_hypothesis: BTreeMap<String, Vec<LensTerm>>,
    pub binding_lens_by_hypothesis: BTreeMap<String, Option<String>>,
    pub recoverability_by_hypothesis: BTreeMap<String, RecoverabilityRow>,
}

/// A deficit covered by an exchange (§4.8): the audit line that shows the price
/// was paid by trade, at an observed rate, and how long the trade itself took.
#[derive(Clone, Debug)]
pub struct Conversion {
    pub from: String,
    pub to: String,
    pub amount_from: f64,
    pub amount_to: f64,
    pub rate: f64,
    pub duration_mks: f64,
}

/// The result of §4.8's funding decision: what the option needs, what actually
/// leaves the agent's stock (the spend ledger — a deficit bought from another
/// resource spends *that* resource), the trades performed, and the deficit that
/// survived full verified conversion.
#[derive(Clone, Debug)]
pub struct FundingPlan {
    pub covered: bool,
    pub need: BTreeMap<String, f64>,
    pub spend: BTreeMap<String, f64>,
    pub conversions: Vec<Conversion>,
    pub uncovered: BTreeMap<String, f64>,
    /// §4.8 (v0.7): the part of the spend above the mandate ceiling, in the group
    /// numeraire. It can only remove an option a larger balance would have paid for.
    pub mandate_exceeded: f64,
    pub total_duration_mks: f64,
}

/// A candidate removed before evaluation (§6.2).
#[derive(Clone, Debug)]
pub struct RemovedOption {
    pub option_id: String,
    pub gate: String,
}

/// One entity a candidate drove from a counted state to a known zero (§4.2, §6.3):
/// the audit line that makes the price of destruction explicit.
#[derive(Clone, Debug)]
pub struct CollapseCharge {
    pub entity_id: String,
    pub dof_before: f64,
}

/// The v0.8 candidate vector (§4.5): three counts of entities — the protected
/// dimensions — plus the index and the reversibility preference.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CandidateVector {
    pub d1: usize,
    pub d2: usize,
    pub d3: usize,
    pub net_delta: f64,
    pub reversible: bool,
    pub option_id: String,
    /// §6.3: `viable` and `resources_ok` are **conditions of admissibility**, not
    /// annotations — a `false` in either bars the option exactly as a positive `d`
    /// does — so they are part of `candidate_vector`, not of the row's context.
    pub viable: bool,
    pub resources_ok: bool,
}

/// One entity this option drops out of a `reachable` verdict, with the witness it
/// lost (§4.5, §6.3). A path loss counts even where no exclusion follows from it.
#[derive(Clone, Debug, PartialEq)]
pub struct LostPathEntry {
    pub entity_id: String,
    pub verdict_before: String,
    pub verdict_after: String,
    pub critical: bool,
    pub witness_lost: Vec<String>,
}

/// One option row of the audit report.
#[derive(Clone, Debug)]
pub struct OptionReportRow {
    pub option_id: String,
    pub is_reversible: bool,
    pub projected_dof: f64,
    pub net_delta: f64,
    pub selected: bool,
    pub estimated_duration_mks: f64,
    /// §6.3: every collapse this option causes, as an auditable line of the ledger.
    pub collapse_charges: Vec<CollapseCharge>,
    /// §6.3 (v0.6): what the option draws, and how "affordable" was established —
    /// by cash in hand or by an observed trade — plus whatever stayed uncovered.
    pub resource_consumption: HashMap<String, HashMap<String, f64>>,
    pub conversion_applied: Vec<Conversion>,
    pub resources_uncovered: BTreeMap<String, f64>,
    /// §4.8/§6.3 (v0.7): how much of the mandate the spend would have used, and what
    /// the closure decomposes into.
    pub mandate_exceeded: f64,
    pub closed: Vec<ClosedRef>,
    pub closure_share: BTreeMap<String, f64>,
    /// §6.3 (v0.8): the protected dimensions, the dimension that barred the
    /// candidate (empty when nothing did), and the path losses line by line.
    pub candidate_vector: CandidateVector,
    pub barring_key: Option<String>,
    pub lost_paths: Vec<LostPathEntry>,
    // §6.3 (v0.11): the v0.11 report surface. The flat fields above describe the
    // **observed** reading (`is_reversible`, `closed`, `closure_share`); the
    // per-reading values live here, and the two are not interchangeable — a report
    // carrying only the flat field presents the ordering key's input as a constant
    // while the key ranges over the set.
    //
    // `conditional_vectors` is the structure of the change across readings — an
    // asymmetry between hypotheses is a result, not an intermediate — and every
    // entry is computed from that reading's own triple: the state `S | h`, the
    // projection `projection[h]` and the closures `closure[h]` (§6.3).
    pub conditional_vectors: BTreeMap<String, ConditionalVector>,
    pub admissible_under: BTreeMap<String, bool>,
    pub barring_key_by_hypothesis: BTreeMap<String, Option<String>>,
    pub net_delta_robust: Option<f64>,
    pub robust_is_reversible: bool,
    pub projection_form: String,
    pub closure_form: String,
    pub viability: bool,
    pub resources_ok: bool,
    pub forms_consistent: bool,
}

/// Full Proof-of-Implementation audit (DOF-SPEC §6).
#[derive(Clone, Debug)]
pub struct DofReport {
    pub entities: Vec<EntityReportRow>,
    pub total_system_dof: f64,
    pub context_switch_cost: f64,
    /// §3.2b (v0.11): the **deprecated mirror** of τ, carried for a reader of the
    /// historical reports only. It is clamped — `0.0` for an unknown and for a
    /// passed deadline — and it is **not** the τ any rule reads: every rule takes
    /// `tau_of(state)`, which is signed and `null` when unmeasured. A report that
    /// presents this field as τ is non-conformant.
    pub global_time_to_collapse_mks: f64,
    pub mode: String,
    pub options: Vec<OptionReportRow>,
    pub psi_id: String,
    pub psi_digest: String,
    pub declaration: String,
    pub removed_options: Vec<RemovedOption>,
    /// §6.2: a resolvable unknown was left unmeasured in every candidate, so the
    /// decision is declared incomplete rather than presented as informed.
    pub incomplete: bool,
    /// §6.2 (v0.6): the acting agent's means at the start of the cycle and after
    /// the selected option's consumption. Multi-step accumulation is auditable
    /// only if the spend is written where the next cycle can see it (§4.8).
    pub resources_before: BTreeMap<String, ResourceObservation>,
    pub resources_after: BTreeMap<String, ResourceObservation>,
    /// §6.2 (v0.7): the identity of the observation a reported subgraph was taken
    /// from, and where the amounts a decision rests on came from — a measured
    /// balance or an asserted authority.
    pub observation_digest: Option<String>,
    pub means_provenance: BTreeMap<String, MandateValue>,
    /// §6.2 (v0.8): the vector every candidate was compared against, and whether
    /// any candidate beat it. A refusal to act is a decision and must be audible.
    pub baseline: CandidateVector,
    pub no_candidate_better: bool,
    // --- §6.2 (v0.11): the report is **per hypothesis** ----------------------
    //
    // There is no single total under a hypothesis set and the report MUST NOT
    // present one: `total_system_dof` above is the **observed reading's** value —
    // the reading the flat fields of §6.3 also describe — and this map is the
    // state's index per reading. A scalar here would be the average the standard
    // refuses (§2), and it would contradict the per-entity contributions and the
    // conditional vectors reported beside it.
    pub psi_ruler_digest: Option<String>,
    pub hypotheses: Vec<Hypothesis>,
    pub hypothesis_coverage: String,
    pub plausible_hypotheses: Vec<String>,
    pub hypothesis_horizon_mks: Option<f64>,
    pub total_system_dof_by_hypothesis: BTreeMap<String, f64>,
    pub hypothesis_conflict: bool,
    /// §6.3: the whole decision payload, so a consumer can read the conditional
    /// vectors without walking every option row.
    pub conditional_vectors: BTreeMap<String, BTreeMap<String, ConditionalVector>>,
    pub admissible_under: BTreeMap<String, BTreeMap<String, bool>>,
    pub net_delta_robust: BTreeMap<String, f64>,
    pub robust_admissible: Vec<String>,
}

/// What a report needs beyond the state, the candidates and the selection. It keeps
/// the report call site readable now that the report is where the release's reasons
/// are written down (§6).
#[derive(Clone, Debug, Default)]
pub struct ReportInput<'a> {
    pub declaration: Option<&'a MeasurementDeclaration>,
    pub removed: Vec<RemovedOption>,
    pub groups: Option<&'a Vec<Vec<String>>>,
    pub rates: Option<&'a BTreeMap<String, Rate>>,
    pub weights: Option<&'a BTreeMap<String, f64>>,
    pub cap: Option<f64>,
    pub ctx: Option<&'a ObservationContext>,
    pub means_provenance: BTreeMap<String, MandateValue>,
    // §6.2/§6.3 (v0.11): the reading context. `readings` is the set the
    // conditional quantities were computed over — **`H_plausible`**, the reduction
    // of §4.10.6 being the observed-state singleton — while `declared` is the whole
    // declared set, which is what `hypotheses` reports: a member excluded by the
    // plausibility partition is still part of the artifact, and §6.2 asks for the
    // set *and* for the plausible subset separately. `selection` is the §4.10
    // decision payload, so the report shows every candidate's conditional vector
    // instead of one reading's flat fields.
    pub readings: Vec<Hypothesis>,
    pub declared: Vec<Hypothesis>,
    pub coverage: String,
    pub horizon_mks: Option<f64>,
    pub ruler_digest: Option<String>,
    pub selection: Option<&'a ConditionalSelection>,
}

pub struct DofCalculusCore {
    epsilon: f64,
}

impl DofCalculusCore {
    pub fn new() -> Self {
        DofCalculusCore { epsilon: 1e-6 }
    }

    /// v0.8 (§10): the tolerance that decides whether two candidates' NetDelta are
    /// tied. The index is a sum of logarithms over a SET, so two ports that iterate
    /// their container in different orders can disagree in the last bits (~1e-15)
    /// while agreeing on every derivation — and a tie must be resolved identically
    /// everywhere, because §7 requires the same CHOICE, not only the same numbers.
    pub const NET_DELTA_TOLERANCE: f64 = 1e-9;

    /// Whether an entity belongs to the calculation set `calc` (DOF-SPEC §4.2).
    /// Excluded if it is a **witnessed** collapse source, or if its DoF is a known
    /// zero whose recoverability verdict is `proven_unreachable` (§4.2/§4.9). A node
    /// with unknown DoF is never excluded (Axiom 5), and neither is a node whose
    /// verdict is `reachable` or `undetermined` — incompleteness of an observation is
    /// never read as proof.
    ///
    /// The witness of exclusion MUST NOT be the Generator's candidate set (§4.2), and
    /// a verdict is computed from the observation, never asserted.
    pub fn is_included(
        &self,
        entity: &EntityState,
        ctx: Option<&ObservationContext>,
        state: Option<&SystemStateMatrix>,
    ) -> bool {
        if entity.is_collapse_source && self.label_witnessed(entity, ctx, state) {
            return false;
        }
        self.is_included_without_label(entity, ctx)
    }

    /// `calc` membership with the collapse-source label NOT honoured (§4.2). Used in
    /// two places, and it must be the same rule in both: deciding who is counted, and
    /// deciding whether a label has a witness. The witness question is "would this
    /// entity be counted if its own label were ignored" — asking it with the label
    /// already applied would be circular, and would make every label unfalsifiable.
    pub fn is_included_without_label(
        &self,
        entity: &EntityState,
        ctx: Option<&ObservationContext>,
    ) -> bool {
        if entity.current_dof > 0.0 {
            return true;
        }
        if !entity.dof_known {
            return true;
        }
        match ctx {
            None => true, // fail-safe: no observation, no proof
            Some(c) => c.verdict(&entity.entity_id) != "proven_unreachable",
        }
    }

    /// A label is honoured only with a machine-verifiable act (§4.2): an act performed
    /// by this entity that drives an entity which would otherwise be counted to a known
    /// zero.
    pub fn label_witnessed(
        &self,
        entity: &EntityState,
        ctx: Option<&ObservationContext>,
        state: Option<&SystemStateMatrix>,
    ) -> bool {
        let (c, s) = match (ctx, state) {
            (Some(c), Some(s)) => (c, s),
            _ => return false,
        };
        let mut counted: BTreeSet<String> = BTreeSet::new();
        let mut dof_before: BTreeMap<String, f64> = BTreeMap::new();
        for (eid, e) in s.entities.iter() {
            if self.is_included_without_label(e, Some(c)) {
                counted.insert(eid.clone());
            }
            dof_before.insert(eid.clone(), e.current_dof);
        }
        if !counted.contains(&entity.entity_id) {
            return false;
        }
        let acts = c.world.collapse_acts(&counted, &dof_before);
        c.world
            .acts
            .iter()
            .any(|a| a.source == entity.entity_id && acts.contains(&a.id))
    }

    /// `calc(S)`, frozen for the whole cycle (§4.2): computed once, on `S`, and the
    /// same entities are summed in `S` and in `S'`, so a term cannot appear or
    /// disappear between the two sides of `NetDelta`.
    pub fn calc_members(
        &self,
        state: &SystemStateMatrix,
        ctx: Option<&ObservationContext>,
    ) -> BTreeSet<String> {
        state
            .entities
            .values()
            .filter(|e| self.is_included(e, ctx, Some(state)))
            .map(|e| e.entity_id.clone())
            .collect()
    }

    /// Evaluation index: pure Nash product (sum of ln(DoF)) over the frozen calc set.
    /// Values are negative; only their ordering matters. See DOF-SPEC §4.1.
    /// Pass `None` for `members` to use `calc(state)` itself.
    pub fn calculate_system_dof(
        &self,
        state: &SystemStateMatrix,
        members: Option<&BTreeSet<String>>,
        ctx: Option<&ObservationContext>,
    ) -> f64 {
        let owned;
        let set = match members {
            Some(m) => m,
            None => {
                owned = self.calc_members(state, ctx);
                &owned
            }
        };
        let mut total = 0.0;
        for eid in set {
            if let Some(entity) = state.entities.get(eid) {
                total += entity.current_dof.max(self.epsilon).ln();
            }
        }
        total
    }

    fn coerce_dof(v: f64) -> f64 {
        v.clamp(0.0, 1.0)
    }

    /// §4.3/§4.4: the DoF the entity would have after the option's closure, recomputed
    /// from the counters. Only the Variety share moves, so the whole product moves by
    /// its ratio. `None` means the entity is not affected or its Variety lens was
    /// unmeasured.
    /// §4.10 (v0.11): the closure list is read **under `hypothesis_id`** — with a
    /// per-hypothesis `closed` the same option destroys different transitions under
    /// different readings, and a quantity of §4.3–§4.4 that read the flat list would
    /// silently decide every reading by the observed one.
    pub fn dof_after_closure_for(
        &self,
        entity: &EntityState,
        option: &ActionOption,
        ctx: &ObservationContext,
        hypothesis_id: &str,
    ) -> Option<f64> {
        let measurement = entity.measurement.as_ref()?;
        let counters = measurement.variety_counters.as_ref()?;
        let var_before = measurement.psi.get("variety").copied().flatten()?;
        let v_env = counters.get("V_env").copied().unwrap_or(0.0);
        let v_before = ctx.v_before(&entity.entity_id);
        let v_after = ctx.v_after_closure(&entity.entity_id, &option.closed_for(hypothesis_id));
        if v_after == v_before {
            return None; // this entity is not affected
        }
        Some(Self::coerce_dof(
            entity.current_dof / var_before * psi_var(v_after as f64, v_env),
        ))
    }

    /// The **observed-reading** entry point (absence of a set is the observed
    /// singleton, §3.6).
    pub fn dof_after_closure(
        &self,
        entity: &EntityState,
        option: &ActionOption,
        ctx: &ObservationContext,
    ) -> Option<f64> {
        self.dof_after_closure_for(entity, option, ctx, crate::hypothesis::OBSERVED_HYPOTHESIS_ID)
    }

    /// The DoF this option would leave the entity with, closure included (§4.3),
    /// **under the reading `hypothesis_id`**. ONE definition, used by both
    /// `simulate` and `collapse_charges`: if the charge were computed from the raw
    /// delta while the index was computed from the closure-aware value, an option
    /// that destroys an entity BY CLOSING ITS TRANSITIONS would be scored as a
    /// collapse and charged as nothing — the structural gate of §4.5 would then pass
    /// exactly the option it exists to stop.
    ///
    /// Both the raw delta and the closure list are read **under `hypothesis_id`**
    /// (§3.3, §4.4): a per-hypothesis option projects a different DoF under each
    /// reading, and the closure the projection is corrected by must be the closure of
    /// the same reading.
    pub fn projected_dof_for(
        &self,
        entity: &EntityState,
        option: &ActionOption,
        ctx: Option<&ObservationContext>,
        hypothesis_id: &str,
    ) -> f64 {
        let add = option.delta_for(hypothesis_id, &entity.entity_id);
        let mut new_dof = Self::coerce_dof(entity.current_dof + add);
        if let Some(c) = ctx {
            if !option.closed_for(hypothesis_id).is_empty() {
                if let Some(recomputed) =
                    self.dof_after_closure_for(entity, option, c, hypothesis_id)
                {
                    new_dof = recomputed;
                }
            }
        }
        new_dof
    }

    /// The observed-reading entry point of `projected_dof_for`.
    pub fn projected_dof(
        &self,
        entity: &EntityState,
        option: &ActionOption,
        ctx: Option<&ObservationContext>,
    ) -> f64 {
        self.projected_dof_for(entity, option, ctx, crate::hypothesis::OBSERVED_HYPOTHESIS_ID)
    }

    /// §4.4 guards: closing one's own execution path, or a false label with nothing
    /// closed. `None` when the option is conformant.
    ///
    /// The guards range over **every** closure list the option declares
    /// (`all_closed_lists`), not over the flat field: an option whose declaration is
    /// well-formed under one reading and malformed under another is malformed (§4.4).
    pub fn closure_error(&self, option: &ActionOption) -> Option<String> {
        for lst in option.all_closed_lists() {
            if !lst.is_empty() && !option.act_id.is_empty() {
                for c in lst.iter() {
                    if c.kind == "act" && c.id == option.act_id {
                        return Some(format!(
                            "{}: closes its own execution path (§4.4 guard 1)",
                            option.option_id
                        ));
                    }
                }
            }
        }
        if option.closure_form() == "flat" && option.closed.is_empty() && !option.is_reversible {
            return Some(format!(
                "{}: is_reversible=false with an empty closure list (§4.4 guard 2)",
                option.option_id
            ));
        }
        None
    }

    /// §4.4: the reported flag is DERIVED — true exactly when nothing is closed.
    ///
    /// The flag is read under a **reading** (§4.10): with a per-hypothesis `closed`
    /// the same option is reversible under one hypothesis and not under another, and
    /// the robust reading of §4.5 key 3 is the conjunction over `H_plausible`
    /// (`robust_reversible` in `conditional.rs`). This entry point is the observed
    /// reading.
    pub fn is_reversible(&self, option: &ActionOption) -> bool {
        option.is_reversible_for(crate::hypothesis::OBSERVED_HYPOTHESIS_ID)
    }

    /// §6.3: the per-entity decomposition of a closure's price. A DECOMPOSITION of the
    /// loss already inside `NetDelta` (§4.3/§4.4), never an extra charge.
    pub fn closure_share(
        &self,
        state: &SystemStateMatrix,
        option: &ActionOption,
        ctx: Option<&ObservationContext>,
    ) -> BTreeMap<String, f64> {
        let mut out: BTreeMap<String, f64> = BTreeMap::new();
        let c = match ctx {
            Some(c) => c,
            None => return out,
        };
        if option.closed.is_empty() {
            return out;
        }
        for (eid, entity) in state.entities.iter() {
            let measurement = match entity.measurement.as_ref() {
                Some(m) => m,
                None => continue,
            };
            let counters = match measurement.variety_counters.as_ref() {
                Some(v) => v,
                None => continue,
            };
            let var_before = match measurement.psi.get("variety").copied().flatten() {
                Some(v) => v,
                None => continue,
            };
            let _ = var_before;
            let v_env = counters.get("V_env").copied().unwrap_or(0.0);
            let v_after = c.v_after_closure(eid, &option.closed);
            let v_before = c.v_before(eid);
            if v_after == v_before {
                continue;
            }
            let after = psi_var(v_after as f64, v_env).max(self.epsilon);
            let before = psi_var(v_before as f64, v_env).max(self.epsilon);
            out.insert(eid.clone(), q6(after.ln() - before.ln()));
        }
        out
    }

    /// §6.1: the verdict, its witness and the completeness claim behind it.
    pub fn recoverability_row(
        &self,
        entity_id: &str,
        ctx: Option<&ObservationContext>,
    ) -> RecoverabilityRow {
        let c = match ctx {
            Some(c) => c,
            None => {
                return RecoverabilityRow {
                    verdict: "undetermined".to_string(),
                    witness: Vec::new(),
                    horizon_mks: None,
                    observation: "unobserved".to_string(),
                    admissible_seen: 0,
                    reason: "no observation was supplied for this cycle".to_string(),
                }
            }
        };
        let v: Verdict = c.world.verdict(entity_id, &c.means_class, c.horizon(entity_id));
        let observation = c
            .world
            .entities
            .get(entity_id)
            .map(|n| n.observation.clone())
            .unwrap_or_else(|| "unobserved".to_string());
        RecoverabilityRow {
            verdict: v.verdict,
            witness: v.witness,
            horizon_mks: c.horizon(entity_id),
            observation,
            admissible_seen: v.admissible_seen,
            reason: v.reason,
        }
    }

    /// Simulate an option's projected deltas into a new state and return it with the
    /// **frozen** member set of `calc(S)` (§4.2), **under the reading
    /// `hypothesis_id`**.
    pub fn simulate_for(
        &self,
        current: &SystemStateMatrix,
        option: &ActionOption,
        ctx: Option<&ObservationContext>,
        hypothesis_id: &str,
    ) -> (SystemStateMatrix, BTreeSet<String>) {
        let members = self.calc_members(current, ctx);
        let mut simulated = current.entities.clone();
        for (eid, e_state) in current.entities.iter() {
            let new_dof = self.projected_dof_for(e_state, option, ctx, hypothesis_id);
            if let Some(ent) = simulated.get_mut(eid) {
                ent.current_dof = new_dof;
            }
        }
        (
            SystemStateMatrix {
                global_time_to_collapse_mks: current.global_time_to_collapse_mks,
                context_switch_cost: current.context_switch_cost,
                entities: simulated,
                psi: current.psi.clone(),
                // The agent's means travel unchanged: `simulate` scores the DoF
                // consequences, and the resource side is decided by §4.8.
                resources: current.resources.clone(),
                tau: current.tau.clone(),
                // §3.2b/§4.7 (v0.11): the deadlines, the durations and the
                // schedule are ruler-level content of the same observation, so a
                // simulated state carries them unchanged — `simulate` scores the
                // DoF consequences of an option, not a new measurement regime.
                deadlines: current.deadlines.clone(),
                measurement_durations: current.measurement_durations.clone(),
                measurement_schedule: current.measurement_schedule.clone(),
            },
            members,
        )
    }

    /// The **observed-reading** entry point of `simulate_for` (absence of a set is
    /// the observed singleton, §3.6).
    pub fn simulate(
        &self,
        current: &SystemStateMatrix,
        option: &ActionOption,
        ctx: Option<&ObservationContext>,
    ) -> (SystemStateMatrix, BTreeSet<String>) {
        self.simulate_for(current, option, ctx, crate::hypothesis::OBSERVED_HYPOTHESIS_ID)
    }

    /// §4.2: the counted entities a candidate drives to a known zero, **under the
    /// reading `hypothesis_id`**. The charge depends on neither the Generator's
    /// candidate set nor the victim's post-collapse prospects.
    pub fn collapse_charges_for(
        &self,
        current: &SystemStateMatrix,
        option: &ActionOption,
        ctx: Option<&ObservationContext>,
        hypothesis_id: &str,
    ) -> Vec<CollapseCharge> {
        let mut charges: Vec<CollapseCharge> = Vec::new();
        for eid in self.calc_members(current, ctx) {
            let entity = match current.entities.get(&eid) {
                Some(e) => e,
                None => continue,
            };
            if !entity.dof_known {
                continue; // unknown DoF is never a collapse (§4.2)
            }
            // The projected value is the closure-aware one (§4.3): an option can
            // destroy a counted entity by closing its transitions while declaring no
            // delta at all, and that is exactly the case §4.5 must catch.
            let new_dof = self.projected_dof_for(entity, option, ctx, hypothesis_id);
            // §4.2: a charge requires a *transition* into the zero, not a stay at it —
            // charging an entity that was already at zero would make every option
            // destructive in any state containing a recoverable zero.
            if new_dof == 0.0 && entity.current_dof > 0.0 {
                charges.push(CollapseCharge {
                    entity_id: eid,
                    dof_before: entity.current_dof,
                });
            }
        }
        charges
    }

    /// The **observed-reading** entry point of `collapse_charges_for`.
    pub fn collapse_charges(
        &self,
        current: &SystemStateMatrix,
        option: &ActionOption,
        ctx: Option<&ObservationContext>,
    ) -> Vec<CollapseCharge> {
        self.collapse_charges_for(current, option, ctx, crate::hypothesis::OBSERVED_HYPOTHESIS_ID)
    }

    /// §4.5: removes options that destroy a counted entity while a charge-free
    /// candidate exists (Axiom 3). Every removal is recorded as `gate = "collapse"`.
    /// RETIRED in v0.8: the live path no longer calls this. A charged candidate is
    /// evaluated, reported in full, and made inadmissible by the candidate-vector
    /// test of §4.5 (`select_candidate`), so `removed_options` carries no
    /// structural removal. Kept because the v0.6 harness asserts the rule that was
    /// in force then, and history must stay reproducible.
    pub fn apply_structural_gate(
        &self,
        current: &SystemStateMatrix,
        options: &[ActionOption],
        ctx: Option<&ObservationContext>,
    ) -> (Vec<ActionOption>, Vec<RemovedOption>) {
        if options.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let charge_free_exists = options
            .iter()
            .any(|o| self.collapse_charges(current, o, ctx).is_empty());
        if !charge_free_exists {
            // No alternative exists: the ladder decides among the destructive candidates.
            return (options.to_vec(), Vec::new());
        }
        let mut admissible = Vec::new();
        let mut removed = Vec::new();
        for option in options {
            if self.collapse_charges(current, option, ctx).is_empty() {
                admissible.push(option.clone());
            } else {
                removed.push(RemovedOption {
                    option_id: option.option_id.clone(),
                    gate: "collapse".to_string(),
                });
            }
        }
        (admissible, removed)
    }

    // --- §4.8 resource gate ---------------------------------------------------

    fn means_of(state: &SystemStateMatrix, resource: &str) -> f64 {
        state.resources.get(resource).map(|o| o.resource_value(false)).unwrap_or(0.0)
    }

    // ---------------------------------------------------------------------
    // §4.5 (v0.8): the candidate vector and the ordered test
    // ---------------------------------------------------------------------

    /// The set of entities of calc(S) whose `current_dof` is the minimum over
    /// calc(S) (§4.5). A set, not a node: a minimum attained by several known
    /// zeros has no unique "critical node", and a flag would have to invent a
    /// tie-break by `entity_id`.
    pub fn critical_members(
        &self,
        state: &SystemStateMatrix,
        ctx: Option<&ObservationContext>,
    ) -> BTreeSet<String> {
        let mut out: BTreeSet<String> = BTreeSet::new();
        let members = self.calc_members(state, ctx);
        let mut lowest = f64::INFINITY;
        let mut found = false;
        for id in members.iter() {
            if let Some(ent) = state.entities.get(id) {
                if ent.current_dof < lowest {
                    lowest = ent.current_dof;
                }
                found = true;
            }
        }
        if !found {
            return out;
        }
        for id in members.iter() {
            if let Some(ent) = state.entities.get(id) {
                if ent.current_dof == lowest {
                    out.insert(id.clone());
                }
            }
        }
        out
    }

    /// The entities this option drops out of a `reachable` verdict, line by line
    /// (§4.5, §6.3), **under the reading `hypothesis_id`**.
    ///
    /// The verdict procedure runs twice over the SAME observation — once as
    /// observed, once with the option's closure applied — so a verdict can only
    /// move away from `reachable`, and the difference is computed rather than
    /// declared. A lost witness is a loss: an entity that leaves `reachable` counts
    /// even where no exclusion follows from it, because §4.2 excludes only on a
    /// `proven_unreachable` verdict over a complete observation.
    ///
    /// §4.10 (v0.11): the closure list is read **under the reading** — with a
    /// per-hypothesis `closed` the same option destroys different transitions under
    /// different readings, and a D2 that read the flat list would decide every
    /// reading by the observed one.
    pub fn lost_paths_for(
        &self,
        state: &SystemStateMatrix,
        option: &ActionOption,
        ctx: Option<&ObservationContext>,
        hypothesis_id: &str,
    ) -> Vec<LostPathEntry> {
        let mut out: Vec<LostPathEntry> = Vec::new();
        let ctx = match ctx {
            Some(c) => c,
            None => return out,
        };
        let closed = option.closed_for(hypothesis_id);
        if closed.is_empty() {
            return out;
        }
        let closed_world = ctx.world.with_closed(&closed);
        let critical = self.critical_members(state, Some(ctx));
        let mut ids: Vec<String> = state.entities.keys().cloned().collect();
        ids.sort();
        for id in ids {
            let entity = match state.entities.get(&id) {
                Some(e) => e,
                None => continue,
            };
            // §4.9 (v0.11): both verdicts consume `DoF(X | h)` — the observed
            // reading's DoF for the before-state, and, for the after-state, the DoF
            // the CLOSURE leaves and **never** the option's projected DoF. D2 asks
            // "did the closure destroy a recovery path?", not "is the entity better
            // off after the option's promised effect?" — the latter is NetDelta.
            // Were the projection admitted here, an option could raise the
            // after-state DoF with its own promise and buy back the very
            // recoverability it destroys. A `None` means the closure did not touch
            // this entity, which then keeps its observed DoF.
            let before = ctx.world.verdict_with_dof(
                &id,
                &ctx.means_class,
                ctx.horizon(&id),
                Some(entity.current_dof),
            );
            if before.verdict != "reachable" {
                continue;
            }
            // §6.3: the after-state is **this reading's** own, so the counters are
            // recomputed from the same `closure[h]` the graph above was pruned by.
            // Reading `closure[$observed$]` here — as this port did before — built a
            // state no hypothesis produces: one reading's graph with another
            // reading's counters, which is what §6.3's "MUST NOT read one reading's
            // closures against another reading's state" forbids.
            let after_dof = self.dof_after_closure_for(entity, option, ctx, hypothesis_id);
            let after = closed_world.verdict_with_dof(
                &id,
                &ctx.means_class,
                ctx.horizon(&id),
                after_dof,
            );
            if after.verdict == "reachable" {
                continue;
            }
            out.push(LostPathEntry {
                entity_id: id.clone(),
                verdict_before: before.verdict.clone(),
                verdict_after: after.verdict.clone(),
                critical: critical.contains(&id),
                witness_lost: before.witness.clone(),
            });
        }
        out
    }

    /// The **observed-reading** entry point of `lost_paths_for`.
    pub fn lost_paths(
        &self,
        state: &SystemStateMatrix,
        option: &ActionOption,
        ctx: Option<&ObservationContext>,
    ) -> Vec<LostPathEntry> {
        self.lost_paths_for(state, option, ctx, crate::hypothesis::OBSERVED_HYPOTHESIS_ID)
    }

    /// The keys of one candidate (§4.5) **under the reading `hypothesis_id`**: all of
    /// them, from quantities the earlier releases already produce.
    ///
    /// §4.10: every quantity that depends on a hypothesis — the simulated DoF, the
    /// collapse charges, the lost paths and the reversibility — is read under the
    /// SAME reading. A vector assembled from mixed readings is a number no
    /// hypothesis produces, and it would be reported as one the reading did.
    pub fn candidate_vector_for(
        &self,
        state: &SystemStateMatrix,
        option: &ActionOption,
        ctx: Option<&ObservationContext>,
        current_index: f64,
        hypothesis_id: &str,
    ) -> CandidateVector {
        let (simulated, members) = self.simulate_for(state, option, ctx, hypothesis_id);
        let projected = self.calculate_system_dof(&simulated, Some(&members), ctx);
        let lost = self.lost_paths_for(state, option, ctx, hypothesis_id);
        let d3 = lost.iter().filter(|row| row.critical).count();
        CandidateVector {
            d1: self.collapse_charges_for(state, option, ctx, hypothesis_id).len(),
            d2: lost.len(),
            d3,
            net_delta: self.net_delta(state, option, projected, current_index),
            reversible: option.is_reversible_for(hypothesis_id),
            option_id: option.option_id.clone(),
            // §6.3: executability is a condition of admissibility, so it belongs to
            // the vector. `conditional.rs` carries the per-reading values; these are
            // the observed reading's, which is what a flat report row shows.
            viable: self.viability(state, option).viable,
            resources_ok: self
                .plan_funding(state, option, None, None, None, None)
                .covered,
        }
    }

    /// The **observed-reading** entry point of `candidate_vector_for`.
    pub fn candidate_vector(
        &self,
        state: &SystemStateMatrix,
        option: &ActionOption,
        ctx: Option<&ObservationContext>,
        current_index: f64,
    ) -> CandidateVector {
        self.candidate_vector_for(
            state,
            option,
            ctx,
            current_index,
            crate::hypothesis::OBSERVED_HYPOTHESIS_ID,
        )
    }

    /// Staying put: the zero vector, `NetDelta = 0` by definition. Admissible by
    /// construction — an option that cannot be executed is never better than doing
    /// nothing — so both executability conditions hold (§6.2, §6.3).
    pub fn baseline_vector() -> CandidateVector {
        CandidateVector {
            d1: 0,
            d2: 0,
            d3: 0,
            net_delta: 0.0,
            reversible: true,
            option_id: String::new(),
            viable: true,
            resources_ok: true,
        }
    }

    /// The first dimension on which a candidate fails to beat staying put (§4.5,
    /// §6.2). `None` means nothing barred it: it outranks the baseline, or ties it
    /// while staying reversible.
    pub fn barring_key(vector: &CandidateVector) -> Option<String> {
        // The executability conditions come first, in the order §4.8b evaluates
        // them — temporal (`viable`), then structural, then financial
        // (`resources_ok`). The order is unobservable to the result, so any of the
        // keys reports the same bar; leaving them out would let an option that
        // cannot be executed be reported as barred for a structural reason it never
        // reached.
        if !vector.viable {
            return Some("viable".to_string());
        }
        if !vector.resources_ok {
            return Some("resources_ok".to_string());
        }
        if vector.d1 > 0 {
            return Some("d1".to_string());
        }
        if vector.d2 > 0 {
            return Some("d2".to_string());
        }
        if vector.d3 > 0 {
            return Some("d3".to_string());
        }
        if vector.net_delta <= 0.0 {
            return Some("net_delta".to_string());
        }
        None
    }

    fn dimension(vector: &CandidateVector, key: &str) -> usize {
        match key {
            "d1" => vector.d1,
            "d2" => vector.d2,
            _ => vector.d3,
        }
    }

    /// The v0.8 selection (§4.5): admissibility first, the index second. Returns
    /// the winner — `None` when the system stays, which is a decision and not an
    /// absence of one — and every candidate's vector.
    ///
    /// Staying put is a candidate LIKE ANY OTHER, so its zero vector enters the
    /// set: that is what makes a protected dimension a BAR instead of a
    /// comparison. Any candidate with `d1`, `d2` or `d3` above zero loses to it,
    /// and no candidate can ever be preferred for cutting a path. Comparing
    /// against the baseline only at the `NetDelta` step would let a positive delta
    /// buy a lost path back — exactly the defect this release removes.
    pub fn select_candidate(
        &self,
        current_state: &SystemStateMatrix,
        options: &[ActionOption],
        ctx: Option<&ObservationContext>,
    ) -> (Option<ActionOption>, Vec<CandidateVector>) {
        let mut vectors: Vec<CandidateVector> = Vec::new();
        if options.is_empty() {
            return (None, vectors);
        }
        struct Entry {
            option: Option<ActionOption>,
            vector: CandidateVector,
        }
        let current = self.calculate_system_dof(current_state, None, ctx);
        let mut survivors: Vec<Entry> = Vec::new();
        for option in options {
            let vector = self.candidate_vector(current_state, option, ctx, current);
            vectors.push(vector.clone());
            survivors.push(Entry {
                option: Some(option.clone()),
                vector,
            });
        }
        survivors.push(Entry {
            option: None,
            vector: Self::baseline_vector(),
        });

        // 1. Structural admissibility: d1 = d2 = d3 = 0. Inadmissible candidates
        //    are never compared with one another.
        for key in ["d1", "d2", "d3"] {
            if survivors.is_empty() {
                break;
            }
            let best = survivors
                .iter()
                .map(|e| Self::dimension(&e.vector, key))
                .min()
                .unwrap_or(0);
            survivors.retain(|e| Self::dimension(&e.vector, key) == best);
        }
        // 2. The index, ties grouped with the tolerance of §10.
        if !survivors.is_empty() {
            let best = survivors
                .iter()
                .map(|e| e.vector.net_delta)
                .fold(f64::NEG_INFINITY, f64::max);
            survivors.retain(|e| (e.vector.net_delta - best).abs() <= Self::NET_DELTA_TOLERANCE);
        }
        // 3. Reversibility: a preference among equals, not a penalty (§4.4).
        if survivors.iter().any(|e| e.vector.reversible) {
            survivors.retain(|e| e.vector.reversible);
        }
        // 4. A complete tie goes to staying put, if it is still a candidate.
        if survivors.iter().any(|e| e.option.is_none()) {
            return (None, vectors);
        }
        if !survivors.is_empty() {
            let best_id = survivors
                .iter()
                .map(|e| e.vector.option_id.clone())
                .min()
                .unwrap_or_default();
            survivors.retain(|e| e.vector.option_id == best_id);
        }
        match survivors.first() {
            // The survivor is selected only if it beats the baseline. With the
            // baseline in the set this is already implied; the guard states the rule.
            Some(entry) if entry.vector.net_delta > 0.0 => {
                (entry.option.clone(), vectors)
            }
            _ => (None, vectors),
        }
    }

    /// §4.8: the option's net draw on the agent, per resource. Consumption is the
    /// negative component of the declared delta summed over the entities the
    /// option names; a resource produced more than consumed yields no requirement.
    pub fn requirement(&self, option: &ActionOption) -> BTreeMap<String, f64> {
        let mut net: BTreeMap<String, f64> = BTreeMap::new();
        for per_entity in option.projected_resource_delta.values() {
            for (resource, delta) in per_entity.iter() {
                *net.entry(resource.clone()).or_insert(0.0) += *delta;
            }
        }
        net.into_iter()
            .filter(|(_, v)| *v < 0.0)
            .map(|(k, v)| (k, -v))
            .collect()
    }

    /// §4.8: exchange is possible only inside a derived group.
    fn same_group(a: &str, b: &str, groups: Option<&Vec<Vec<String>>>) -> bool {
        if a == b {
            return true;
        }
        match groups {
            Some(gs) => gs.iter().any(|g| g.iter().any(|r| r == a) && g.iter().any(|r| r == b)),
            None => false,
        }
    }

    /// §4.8: decide *how* an option is paid for, and whether it can be. Step 1 is
    /// a direct comparison against the agent's means. Step 2 is **verified**
    /// conversion: the exchange path must exist (declared rate), the resources
    /// must share a group, an offer must satisfy the requirement (deficit /
    /// rate), the price must be payable from the agent's means, and the
    /// exchange's **own time** must still fit in τ. Anything that fails is not a
    /// cheaper conversion — it is a deficit that stays uncovered, and step 3
    /// turns that into insolvency.
    pub fn plan_funding(
        &self,
        state: &SystemStateMatrix,
        option: &ActionOption,
        groups: Option<&Vec<Vec<String>>>,
        rates: Option<&BTreeMap<String, Rate>>,
        weights: Option<&BTreeMap<String, f64>>,
        cap: Option<f64>,
    ) -> FundingPlan {
        let need = self.requirement(option);
        let mut spend: BTreeMap<String, f64> = BTreeMap::new();
        let mut conversions: Vec<Conversion> = Vec::new();
        let mut uncovered: BTreeMap<String, f64> = BTreeMap::new();
        let mut total_duration = option.estimated_duration_mks;
        // The numeraire weights: used to choose an offer canonically and to express
        // the mandate ceiling in one unit.
        let weight_of = |r: &str| -> f64 {
            match weights {
                Some(w) => w.get(r).copied().unwrap_or(1.0),
                None => 1.0,
            }
        };

        for (resource, needed) in need.iter() {
            let mut remaining = *needed;
            let available = (Self::means_of(state, resource) - spend.get(resource).copied().unwrap_or(0.0))
                .max(0.0);
            let direct = remaining.min(available);
            *spend.entry(resource.clone()).or_insert(0.0) += direct;
            remaining -= direct;

            if let Some(rate_table) = rates {
                // §4.8 (v0.7): the offer is chosen CANONICALLY — the cheapest in the
                // group numeraire first, then the shorter exchange, then the key.
                // Choosing by declaration order (or by resource name) would let a
                // rename change what the report says happened, and two ports would
                // describe the same world differently.
                let mut offers: Vec<(f64, f64, String, String, f64, f64)> = Vec::new();
                for (key, spec) in rate_table.iter() {
                    let (source, target) = match key.split_once("->") {
                        Some((s, t)) => (s, t),
                        None => continue,
                    };
                    if target != resource {
                        continue;
                    }
                    if spec.rate <= 0.0 || !Self::same_group(source, resource, groups) {
                        continue;
                    }
                    let amount_source = remaining / spec.rate;
                    let source_available =
                        (Self::means_of(state, source) - spend.get(source).copied().unwrap_or(0.0)).max(0.0);
                    if amount_source > source_available {
                        continue; // the price is not payable
                    }
                    // §4.8 (v0.11): the exchange is taken only if it fits inside the
                    // budget the **resource map** establishes. With an unmeasured τ
                    // nothing fits, so the deficit stays uncovered instead of being
                    // closed against a fabricated deadline (§3.2b).
                    match tau_of(state) {
                        Some(t) if total_duration + spec.duration_mks <= t => {}
                        _ => continue,
                    }
                    offers.push((
                        weight_of(source) * amount_source,
                        spec.duration_mks,
                        key.clone(),
                        source.to_string(),
                        amount_source,
                        spec.rate,
                    ));
                }
                if remaining > 0.0 && !offers.is_empty() {
                    offers.sort_by(|a, b| {
                        a.0.partial_cmp(&b.0)
                            .unwrap_or(std::cmp::Ordering::Equal)
                            .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                            .then(a.2.cmp(&b.2))
                    });
                    let best = &offers[0];
                    *spend.entry(best.3.clone()).or_insert(0.0) += best.4;
                    total_duration += best.1;
                    conversions.push(Conversion {
                        from: best.3.clone(),
                        to: resource.clone(),
                        amount_from: best.4,
                        amount_to: remaining,
                        rate: best.5,
                        duration_mks: best.1,
                    });
                    remaining = 0.0;
                }
            }
            if remaining > 0.0 {
                uncovered.insert(resource.clone(), remaining);
            }
        }

        // §4.8 (v0.7): the mandate caps what may be spent, in the group numeraire. It
        // can only remove an option a larger balance would have paid for, and it can
        // never make payable what the measured means cannot cover.
        let mut mandate_exceeded = 0.0;
        if let Some(c) = cap {
            let mut spent = 0.0;
            for (r, amount) in spend.iter() {
                spent += weight_of(r) * amount;
            }
            if spent > c {
                mandate_exceeded = spent - c;
            }
        }

        FundingPlan {
            covered: uncovered.is_empty() && mandate_exceeded <= 0.0,
            need,
            spend,
            conversions,
            uncovered,
            mandate_exceeded,
            total_duration_mks: total_duration,
        }
    }

    /// §4.8 step 3: an unpayable option is inadmissible, unconditionally. Unlike the
    /// structural gate of §4.5 there is no "no alternative" escape: a shortage that
    /// survives full verified conversion is a verdict, not a price.
    pub fn apply_resource_gate(
        &self,
        state: &SystemStateMatrix,
        options: &[ActionOption],
        groups: Option<&Vec<Vec<String>>>,
        rates: Option<&BTreeMap<String, Rate>>,
        weights: Option<&BTreeMap<String, f64>>,
        cap: Option<f64>,
    ) -> (Vec<ActionOption>, Vec<RemovedOption>) {
        if options.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let mut admissible = Vec::new();
        let mut removed = Vec::new();
        for option in options {
            if self
                .plan_funding(state, option, groups, rates, weights, cap)
                .covered
            {
                admissible.push(option.clone());
            } else {
                removed.push(RemovedOption {
                    option_id: option.option_id.clone(),
                    gate: "insolvency".to_string(),
                });
            }
        }
        (admissible, removed)
    }

    /// §4.4 (v0.7): no flat penalty. An irreversible option's price is already inside
    /// `projected`, because the closure lowered the affected entities' Variety counter
    /// in `S'` (§4.3); subtracting anything here would charge the same loss twice.
    fn net_delta(
        &self,
        current: &SystemStateMatrix,
        _option: &ActionOption,
        projected: f64,
        current_dof: f64,
    ) -> f64 {
        projected - current_dof - current.context_switch_cost
    }

    /// Public view of the §4.4 arithmetic, for the conformance harness: a check must
    /// be able to ask what the rule computes without going through a selection.
    pub fn net_delta_pub(
        &self,
        current: &SystemStateMatrix,
        option: &ActionOption,
        projected: f64,
        current_dof: f64,
    ) -> f64 {
        self.net_delta(current, option, projected, current_dof)
    }

    /// §4.5 (v0.8): admissibility first, the index second. Returns the winner, or
    /// `None` when the system stays — a decision and not an absence of one.
    pub fn evaluate_and_select(
        &self,
        current_state: &SystemStateMatrix,
        options: &[ActionOption],
        ctx: Option<&ObservationContext>,
    ) -> Option<ActionOption> {
        self.select_candidate(current_state, options, ctx).0
    }

    /// §4.7: a resolvable unknown left unmeasured in every candidate.
    pub fn is_incomplete(&self, state: &SystemStateMatrix, options: &[ActionOption]) -> bool {
        let cheapest = options
            .iter()
            .map(|o| o.estimated_duration_mks)
            .filter(|d| *d > 0.0)
            .fold(f64::INFINITY, f64::min);
        if !cheapest.is_finite() {
            return false; // no procedure available at all
        }
        for (_eid, entity) in &state.entities {
            if entity.dof_known {
                continue;
            }
            let touched = options
                .iter()
                .any(|o| o.projected_dof_delta.get(&entity.entity_id).copied().unwrap_or(0.0) != 0.0);
            if touched {
                continue;
            }
            // §4.7 (v0.11): the window is `τ − T_meas` over the resource map's τ. An
            // **unmeasured** τ has no window at all, and that is not the same as a
            // window of zero: `t* ≤ 0` says the deadline is already lost, `null`
            // says it was never measured, so the strict `t* > 0` rule is not applied
            // to it (§5, §3.2b).
            if let Some(t_star) = measurement_window(state, cheapest) {
                if t_star > 0.0 {
                    return true;
                }
            }
        }
        false
    }

    /// Transparent audit (DOF-SPEC §6). Required by the license (PoI).
    pub fn report(
        &self,
        current_state: &SystemStateMatrix,
        options: &[ActionOption],
        selected: &Option<ActionOption>,
        mode: &str,
        input: ReportInput,
    ) -> DofReport {
        let ctx = input.ctx;
        // §6.2/§6.3 (v0.11): the report is **per hypothesis**. `input.readings` is
        // the set the conditional quantities were computed over — `H_plausible`, the
        // reduction of §4.10.6 being the observed-state singleton — and reading a
        // hypothesis means reading *its* state under the same observation, whose
        // §4.9 verdicts are that reading's. The graph and the ruler stay shared;
        // only the measured content moves, so nothing here re-derives an
        // observation.
        let ctx_of = |h: &Hypothesis| -> Option<ObservationContext> {
            match ctx {
                Some(c) => {
                    let mut dofs: HashMap<String, f64> = HashMap::new();
                    for (eid, e) in &h.state.entities {
                        dofs.insert(eid.clone(), e.current_dof);
                    }
                    Some(c.with_dof(dofs))
                }
                None => None,
            }
        };
        let mut entity_rows: Vec<EntityReportRow> = Vec::new();
        for (_eid, ent) in &current_state.entities {
            let included = self.is_included(ent, ctx, Some(current_state));
            let contribution = if included {
                ent.current_dof.max(self.epsilon).ln()
            } else {
                0.0
            };
            let (lens_terms, binding_lens, floored, blocks, derivation) = match &ent.measurement {
                Some(m) => (
                    m.terms.clone(),
                    m.binding_lens.clone(),
                    m.floored,
                    m.blocks.clone(),
                    m.derivation.clone(),
                ),
                None => (Vec::new(), None, false, Vec::new(), None),
            };
            let mut row = EntityReportRow {
                entity_id: ent.entity_id.clone(),
                is_collapse_source: ent.is_collapse_source,
                included_in_sum: included,
                current_dof: ent.current_dof,
                dof_known: ent.dof_known,
                contribution,
                lens_terms,
                binding_lens,
                floored,
                blocks,
                derivation,
                // §6.1 (v0.7): the verdict, its witness and the completeness behind it.
                recoverability: self.recoverability_row(&ent.entity_id, ctx),
                // §6.1 (v0.11): the same facts per reading, filled below.
                lens_terms_by_hypothesis: BTreeMap::new(),
                binding_lens_by_hypothesis: BTreeMap::new(),
                recoverability_by_hypothesis: BTreeMap::new(),
            };
            // §6.1 (v0.11): the reason a value is what it is belongs to a reading
            // too — there is no shared `ψ` to print once the readings differ. Empty
            // when the cycle ran on the observed state alone, where the flat fields
            // are the whole answer and a one-entry map would claim a comparison
            // nobody asked for.
            if input.readings.len() > 1 {
                for h in &input.readings {
                    if let Some(e) = h.state.entities.get(&ent.entity_id) {
                        if let Some(m) = &e.measurement {
                            row.lens_terms_by_hypothesis.insert(h.id.clone(), m.terms.clone());
                            row.binding_lens_by_hypothesis
                                .insert(h.id.clone(), m.binding_lens.clone());
                        }
                    }
                    row.recoverability_by_hypothesis.insert(
                        h.id.clone(),
                        self.recoverability_row(&ent.entity_id, ctx_of(h).as_ref()),
                    );
                }
            }
            entity_rows.push(row);
        }
        let total = self.calculate_system_dof(current_state, None, ctx);

        // §6.2 (v0.6): the means before the cycle and after the selected option's
        // spend ledger — what actually left the stock, not what was declared.
        let mut resources_before: BTreeMap<String, ResourceObservation> = BTreeMap::new();
        for (resource, obs) in current_state.resources.iter() {
            resources_before.insert(resource.clone(), obs.clone());
        }
        let mut resources_after = resources_before.clone();
        if let Some(chosen) = selected {
            let plan = self.plan_funding(
                current_state,
                chosen,
                input.groups,
                input.rates,
                input.weights,
                input.cap,
            );
            for (resource, amount) in plan.spend.iter() {
                let before = resources_after.get(resource);
                let before_val = before.map(|o| o.resource_value(false)).unwrap_or(0.0);
                let mut after = before.cloned().unwrap_or_default();
                after.value = Some((before_val - amount).max(0.0));
                resources_after.insert(resource.clone(), after);
            }
        }

        let mut option_rows: Vec<OptionReportRow> = Vec::new();
        for option in options {
            let (simulated, members) = self.simulate(current_state, option, ctx);
            let projected = self.calculate_system_dof(&simulated, Some(&members), ctx);
            let vector = self.candidate_vector(current_state, option, ctx, total);
            let net = vector.net_delta;
            let is_selected = match selected {
                Some(s) => s.option_id == option.option_id,
                None => false,
            };
            let plan = self.plan_funding(
                current_state,
                option,
                input.groups,
                input.rates,
                input.weights,
                input.cap,
            );
            let mut row = OptionReportRow {
                option_id: option.option_id.clone(),
                is_reversible: self.is_reversible(option),
                projected_dof: projected,
                net_delta: net,
                selected: is_selected,
                estimated_duration_mks: option.estimated_duration_mks,
                // §6.3: every collapse this option causes, as an auditable line
                collapse_charges: self.collapse_charges(current_state, option, ctx),
                // §6.3 (v0.6): what it draws, and how "affordable" was established.
                resource_consumption: option.projected_resource_delta.clone(),
                conversion_applied: plan.conversions,
                resources_uncovered: plan.uncovered,
                // §6.3 (v0.7): the mandate that would have been exceeded, what the
                // option closes, and how the closure's loss decomposes per entity.
                mandate_exceeded: plan.mandate_exceeded,
                closed: option.closed.clone(),
                closure_share: self.closure_share(current_state, option, ctx),
                // §6.3 (v0.8): the protected dimensions, the dimension that barred
                // the candidate (empty when nothing did), and the path losses.
                candidate_vector: vector.clone(),
                barring_key: Self::barring_key(&vector),
                lost_paths: self.lost_paths(current_state, option, ctx),
                // §6.3 (v0.11): the per-reading surface, filled below.
                conditional_vectors: BTreeMap::new(),
                admissible_under: BTreeMap::new(),
                barring_key_by_hypothesis: BTreeMap::new(),
                net_delta_robust: None,
                robust_is_reversible: false,
                projection_form: option.projection_form().to_string(),
                closure_form: option.closure_form().to_string(),
                viability: self.viability(current_state, option).viable,
                resources_ok: plan.covered,
                forms_consistent: option.forms_consistent().is_empty(),
            };
            // §6.3 (v0.11): the per-reading values come from the payload the
            // decision itself used — never from a second computation, or the report
            // could show a vector no reading produced. The flat fields above stay
            // the observed reading's, which is what §6.3 says they are.
            if let Some(sel) = input.selection {
                row.conditional_vectors = sel
                    .conditional_vectors
                    .get(&option.option_id)
                    .cloned()
                    .unwrap_or_default();
                row.admissible_under = sel
                    .admissible_under
                    .get(&option.option_id)
                    .cloned()
                    .unwrap_or_default();
                row.net_delta_robust = sel.net_delta_robust.get(&option.option_id).copied();
                row.robust_is_reversible = self.robust_reversible(option, &input.readings);
                let mut first_bar = row.barring_key.clone();
                for (hid, vec) in row.conditional_vectors.iter() {
                    let flat = CandidateVector {
                        d1: vec.d1,
                        d2: vec.d2,
                        d3: vec.d3,
                        net_delta: vec.net_delta,
                        reversible: vec.reversible,
                        option_id: vec.option_id.clone(),
                        // The barring key must see the executability conditions of
                        // the reading it is asked about, or a barred-for-execution
                        // candidate would look barred for a structural reason.
                        viable: vec.viable,
                        resources_ok: vec.resources_ok,
                    };
                    let key = Self::barring_key(&flat);
                    if first_bar.is_none() && key.is_some() {
                        first_bar = key.clone();
                    }
                    row.barring_key_by_hypothesis.insert(hid.clone(), key);
                }
                row.barring_key = first_bar;
            }
            option_rows.push(row);
        }
        let (psi_id, psi_digest, declaration_text) = match input.declaration {
            Some(d) => (d.psi_id.clone(), d.digest(), d.canonical_text()),
            None => match &current_state.psi {
                Some(p) => (p.id.clone(), p.digest.clone(), String::new()),
                None => (String::new(), String::new(), String::new()),
            },
        };
        let observation_digest = match ctx {
            Some(c) if !c.observation_digest.is_empty() => Some(c.observation_digest.clone()),
            _ => None,
        };
        // §6.2 (v0.11): the declared set as report context, and the index **per
        // reading**. `total_system_dof` above stays the observed reading's value;
        // this map is what a reader of a per-reading report must use, and no single
        // number is presented as the state's total.
        let mut totals_by_h: BTreeMap<String, f64> = BTreeMap::new();
        let mut plausible: Vec<String> = Vec::new();
        for h in &input.readings {
            totals_by_h.insert(h.id.clone(), self.calculate_system_dof(&h.state, None, ctx_of(h).as_ref()));
            if h.plausible {
                plausible.push(h.id.clone());
            }
        }
        let hypotheses = if input.declared.is_empty() {
            input.readings.clone()
        } else {
            input.declared.clone()
        };
        let (hyp_conflict, cond_vecs, adm_under, net_robust, robust_ids) = match input.selection {
            Some(sel) => (
                sel.hypothesis_conflict,
                sel.conditional_vectors.clone(),
                sel.admissible_under.clone(),
                sel.net_delta_robust.clone(),
                sel.robust_admissible.clone(),
            ),
            None => (
                false,
                BTreeMap::new(),
                BTreeMap::new(),
                BTreeMap::new(),
                Vec::new(),
            ),
        };
        DofReport {
            entities: entity_rows,
            total_system_dof: total,
            context_switch_cost: current_state.context_switch_cost,
            global_time_to_collapse_mks: current_state.global_time_to_collapse_mks,
            mode: mode.to_string(),
            options: option_rows,
            psi_id,
            psi_digest,
            declaration: declaration_text,
            removed_options: input.removed,
            incomplete: self.is_incomplete(current_state, options),
            resources_before,
            resources_after,
            observation_digest,
            means_provenance: input.means_provenance,
            // §6.2 (v0.8): what the candidates were compared against, and whether
            // any of them beat it. A silent "no action" is an omission.
            baseline: Self::baseline_vector(),
            no_candidate_better: !options.is_empty() && selected.is_none(),
            psi_ruler_digest: input.ruler_digest,
            hypotheses,
            hypothesis_coverage: if input.coverage.is_empty() {
                "partial".to_string()
            } else {
                input.coverage
            },
            plausible_hypotheses: plausible,
            hypothesis_horizon_mks: input.horizon_mks,
            total_system_dof_by_hypothesis: totals_by_h,
            hypothesis_conflict: hyp_conflict,
            conditional_vectors: cond_vecs,
            admissible_under: adm_under,
            net_delta_robust: net_robust,
            robust_admissible: robust_ids,
        }
    }
}
