// Hypothesis set artifact (DOF-SPEC §3.6) and its validation rules (Rust port).
//
// `v0.10` made the *state* conditional; `v0.11` repaired the artifact so a
// hypothesis is a **complete alternative state** under the **shared ruler**
// (§3.6, §10(A)):
//
//   * a hypothesis supplies the *measured* content — the per-entity lens
//     counters, the measurement durations and the resource map, from which τ
//     follows; `current_dof`, `dof_known` and τ are **computed** from it by the
//     same named procedures, and a stated DoF that its own counters do not
//     produce is non-conformant input;
//   * the **ruler** (§3.4.2) and the observed graph `G` (§3.5) are **shared** —
//     structural uncertainty is *priced, not branched*;
//   * `is_collapse_source` is **not** hypothesis-local: the label is honoured
//     through an observed act, the act comes from the shared graph, and a set
//     that moves the label between readings is non-conformant input;
//   * absence and emptiness are the **observed-state singleton**, with
//     `plausible = true`, so `H_plausible` is never empty and the worst-case
//     operators of §4.10 are total.
//
// The core accepts `H` as supplied: it MUST NOT add, merge, split, drop, reorder
// or re-weight a hypothesis, and MUST NOT compute the `plausible` flag.

use crate::dof_core::{EntityState, SystemStateMatrix};

/// §3.6: the id the observed state carries when it is read as a member of `H`.
pub const OBSERVED_HYPOTHESIS_ID: &str = "$observed$";

/// §4.1/§4.6: the canonical lens order. The DoF identity is the product of these
/// three, in this order, and no port may read them in another.
pub const LENS_ORDER: [&str; 3] = ["variety", "options", "constraint"];

/// The tolerance of the §3.6 identity checks: the stated DoF must equal the
/// product of the reading's own counters to this precision.
pub const HYPOTHESIS_TOLERANCE: f64 = 1e-9;

/// §3.6: one declared interpretation of the same observed state.
#[derive(Clone, Debug)]
pub struct Hypothesis {
    pub id: String,
    pub plausible: bool,
    pub state: SystemStateMatrix,
    /// Report context (§3.4.1): the declared causal reading. Inert — it MUST NOT
    /// affect `calc`, the collapse charges, the §4.9 verdicts or the Axiom-3
    /// exemption. An empty list reads as "names no possible source", never as
    /// "asserts that no source exists".
    pub collapse_source_candidates: Vec<String>,
    pub basis: String,
}

/// §3.6: the artifact. `coverage` is a claim, and its default is cautious.
#[derive(Clone, Debug)]
pub struct HypothesisSet {
    pub coverage: String,
    pub members: Vec<Hypothesis>,
    /// The analysis horizon declared with the set.
    pub horizon_mks: Option<f64>,
}

impl Default for HypothesisSet {
    /// The cautious default: an absent `coverage` reads as `partial`, **never** as
    /// `complete`. A set that does not say how much of the plausible space it
    /// covers has not claimed to cover it, and a reader who assumes otherwise is
    /// reading a claim into silence.
    fn default() -> Self {
        HypothesisSet {
            coverage: "partial".to_string(),
            members: Vec::new(),
            horizon_mks: None,
        }
    }
}

/// The declared coverage of a set, with §3.6's cautious default.
pub fn coverage_of(hset: Option<&HypothesisSet>) -> String {
    match hset {
        Some(h) => h.coverage.clone(),
        None => "partial".to_string(),
    }
}

/// §3.6/§4.10.6: an absent or empty `H` **is** the observed-state singleton.
pub fn observed_singleton(state: &SystemStateMatrix) -> Vec<Hypothesis> {
    vec![Hypothesis {
        id: OBSERVED_HYPOTHESIS_ID.to_string(),
        plausible: true,
        state: state.clone(),
        collapse_source_candidates: Vec::new(),
        basis: "absence or emptiness of H is the observed state".to_string(),
    }]
}

/// `H` as the core reads it, including the absence/emptiness reduction.
pub fn resolved_members(state: &SystemStateMatrix, hset: Option<&HypothesisSet>) -> Vec<Hypothesis> {
    match hset {
        None => observed_singleton(state),
        Some(h) if h.members.is_empty() => observed_singleton(state),
        Some(h) => h.members.clone(),
    }
}

/// `H_plausible = { h ∈ H : h.plausible }`.
///
/// Never empty: the set is either the observed singleton or a declared set whose
/// observed member MUST be present and plausible (§3.6), so a set that resolves to
/// nothing is non-conformant input caught by `validate_set`.
pub fn plausible_members(members: &[Hypothesis]) -> Vec<Hypothesis> {
    let out: Vec<Hypothesis> = members.iter().filter(|h| h.plausible).cloned().collect();
    if out.is_empty() {
        members.iter().take(1).cloned().collect()
    } else {
        out
    }
}

/// The entity's DoF as the product of its **own** lens values (§4.1, §4.6).
///
/// `None` when the entity carries no measurement declaration: the port-level
/// `measurement` object is the carrier of the counters, and without it the
/// identity cannot be checked. A missing declaration is not a contradiction.
pub fn lens_product(entity: &EntityState) -> Option<f64> {
    let m = entity.measurement.as_ref()?;
    let mut product = 1.0;
    for lens in LENS_ORDER.iter() {
        match m.psi.get(*lens) {
            Some(Some(v)) => product *= *v,
            _ => return None,
        }
    }
    Some(product)
}

/// §3.6 non-conformance checks. An empty list means the input is admissible.
///
/// Every rule here is a *check*, not a hope: each one corresponds to a way a
/// manipulated or careless hypothesis set could otherwise move the index, the
/// choice or the collapse-source label without leaving a trace.
pub fn validate_set(state: &SystemStateMatrix, members: &[Hypothesis]) -> Vec<String> {
    let mut errs: Vec<String> = Vec::new();
    let mut ids: Vec<String> = members.iter().map(|h| h.id.clone()).collect();
    ids.sort();
    let unique = {
        let mut u = ids.clone();
        u.dedup();
        u.len()
    };
    if unique != ids.len() {
        errs.push("hypothesis ids are not unique".to_string());
    }
    let mut observed_present = false;
    for h in members {
        let hs = &h.state;
        // (1) State completeness: every entity of `S` appears in every reading,
        //     with an explicit value, never omitted.
        let mut missing: Vec<String> = hs
            .entities
            .keys()
            .filter(|k| !state.entities.contains_key(*k))
            .cloned()
            .collect();
        missing.sort();
        if !missing.is_empty() {
            errs.push(format!(
                "{}: declares entities not in the observed state {:?}",
                h.id, missing
            ));
        }
        let mut omitted: Vec<String> = state
            .entities
            .keys()
            .filter(|k| !hs.entities.contains_key(*k))
            .cloned()
            .collect();
        omitted.sort();
        if !omitted.is_empty() {
            errs.push(format!(
                "{}: omits entities {:?} (§3.6 state completeness)",
                h.id, omitted
            ));
        }
        // (2) The lens identity holds under every hypothesis: the stated DoF must
        //     equal the product of that reading's OWN counters (§4.1, §3.6).
        let mut eids: Vec<&String> = hs.entities.keys().collect();
        eids.sort();
        for e_id in eids.iter() {
            let ent = &hs.entities[*e_id];
            if let Some(product) = lens_product(ent) {
                if (product - ent.current_dof).abs() > HYPOTHESIS_TOLERANCE {
                    errs.push(format!(
                        "{}/{}: stated DoF {} differs from the product of its own lens \
                         values {} (§4.1, §3.6)",
                        h.id, e_id, ent.current_dof, product
                    ));
                }
            }
            // (3) `is_collapse_source` is not hypothesis-local: the label is
            //     honoured through an act of the SHARED graph, so it must equal the
            //     observed value under every reading (§3.6).
            if let Some(obs) = state.entities.get(*e_id) {
                if ent.is_collapse_source != obs.is_collapse_source {
                    errs.push(format!(
                        "{}/{}: is_collapse_source differs from the observed value — the \
                         label is not hypothesis-local (§3.6)",
                        h.id, e_id
                    ));
                }
            }
        }
        // (5) §3.4.1/§3.4.2 (v0.11): the measurement durations are **ruler-level**.
        //     A hypothesis reinterprets what was *measured*; it may not reinterpret
        //     how long the *measuring* takes. If it could, `T_meas` and hence `t*`
        //     would differ between readings that claim one ruler, and
        //     `min_h NetDelta(o | h)` would compare numbers produced by different
        //     measuring systems — which is what the shared `psi_ruler_digest` exists
        //     to make impossible.
        if hs.measurement_durations != state.measurement_durations {
            errs.push(format!(
                "{}: measurement durations differ from the observed procedure — the \
                 durations are ruler-level, not hypothesis-level (§3.4.1, §3.4.2)",
                h.id
            ));
        }
        // (4) The observed state MUST be one of the readings (§3.6).
        if same_state(hs, state) {
            observed_present = true;
        }
    }
    if !observed_present {
        errs.push("the observed state is absent from H (§3.6: it MUST be present)".to_string());
    }
    if !members.iter().any(|h| h.plausible) && members.len() > 1 {
        // A single-member set is the observed singleton and is plausible by
        // construction; a larger set that marks everything implausible leaves
        // `H_plausible` empty, which §3.6 declares an invalid input.
        errs.push("every hypothesis is implausible (§3.6: H_plausible would be empty)".to_string());
    }
    errs
}

/// Whether two matrices are the same *measured* state.
///
/// Only the measured content is compared: the per-entity DoF, the lens counters,
/// the resource map and τ. Report context and provenance are not part of the
/// comparison — two readings that differ only in a `basis` string are the same
/// state.
pub fn same_state(a: &SystemStateMatrix, b: &SystemStateMatrix) -> bool {
    if a.entities.len() != b.entities.len() {
        return false;
    }
    let mut eids: Vec<&String> = a.entities.keys().collect();
    eids.sort();
    for e_id in eids.iter() {
        let eb = match b.entities.get(*e_id) {
            Some(e) => e,
            None => return false,
        };
        let ea = &a.entities[*e_id];
        if (ea.current_dof - eb.current_dof).abs() > HYPOTHESIS_TOLERANCE {
            return false;
        }
        if ea.dof_known != eb.dof_known {
            return false;
        }
        match (ea.measurement.as_ref(), eb.measurement.as_ref()) {
            (None, None) => {}
            (Some(ma), Some(mb)) => {
                for lens in LENS_ORDER.iter() {
                    match (ma.psi.get(*lens), mb.psi.get(*lens)) {
                        (None, None) => {}
                        (Some(Some(x)), Some(Some(y))) => {
                            if (x - y).abs() > HYPOTHESIS_TOLERANCE {
                                return false;
                            }
                        }
                        _ => return false,
                    }
                }
            }
            _ => return false,
        }
    }
    if a.resources.len() != b.resources.len() {
        return false;
    }
    for (r, ra) in a.resources.iter() {
        let rb = match b.resources.get(r) {
            Some(x) => x,
            None => return false,
        };
        match (ra.value, rb.value) {
            (None, None) => {}
            (Some(x), Some(y)) => {
                if (x - y).abs() > HYPOTHESIS_TOLERANCE {
                    return false;
                }
            }
            _ => return false,
        }
    }
    match (a.tau.as_ref(), b.tau.as_ref()) {
        (None, None) => {}
        (Some(ta), Some(tb)) => match (ta.value, tb.value) {
            (None, None) => {}
            (Some(x), Some(y)) => {
                if (x - y).abs() > HYPOTHESIS_TOLERANCE {
                    return false;
                }
            }
            _ => return false,
        },
        _ => return false,
    }
    true
}