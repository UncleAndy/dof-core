//! Fixture helpers of the v0.11 harness (§3.6, §4.10) — Rust mirror of
//! patterns/go/harness_v011_fixtures.go and patterns/python/harness_v011.py.
//!
//! The readings are built as **transformations of one observed state**, never as
//! hand-written states: scaling a copy's lens values, zeroing a copy's τ, removing
//! an entity. Every transformation keeps the lens identity of §4.1 exactly — the
//! copy's stated DoF is set to the product of the copy's own counters — so a
//! reading is refused by `validate_set` only when the harness deliberately breaks
//! it.

use std::collections::HashMap;

use crate::dof_core::{ActionOption, EntityState, ResourceObservation, SystemStateMatrix};
use crate::graph_mapper::RawObservation;
use crate::hypothesis::{lens_product, LENS_ORDER};

/// Scales an entity's lens values so that their product becomes `target`, then
/// states that product as the entity's DoF (§4.1).
///
/// Setting the DoF to the product **as computed** is what makes the identity hold
/// exactly: the harness never asserts a number the port would compute differently.
pub fn v011_set_product(entity: &mut EntityState, target: f64) {
    if entity.measurement.is_none() {
        return;
    }
    let product = match lens_product(entity) {
        Some(p) => p,
        None => return,
    };
    if product <= 0.0 || target <= 0.0 {
        return;
    }
    let ratio = (target / product).powf(1.0 / LENS_ORDER.len() as f64);
    {
        let measurement = entity.measurement.as_mut().unwrap();
        let keys: Vec<String> = measurement.psi.keys().cloned().collect();
        for key in keys {
            if let Some(Some(value)) = measurement.psi.get(&key).cloned() {
                measurement.psi.insert(key, Some(value * ratio));
            }
        }
    }
    if let Some(p) = lens_product(entity) {
        entity.current_dof = p;
        if let Some(m) = entity.measurement.as_mut() {
            m.current_dof = p;
        }
    }
}

/// A copy of a state. Rust's `Clone` is deep, so a transformation never touches
/// the original — which is what the Go port needs a hand-written copy for.
pub fn v011_copy(state: &SystemStateMatrix) -> SystemStateMatrix {
    state.clone()
}

/// Scales the declared Variety counter of every entity in a scene by `k`.
///
/// The counter is a *measured* input, so scaling it produces a genuinely different
/// reading, while the ruler — the procedures, the lens set, the units, the means
/// class — is untouched. The resulting state's stated DoF is reconciled to the
/// product of its own counters by `v011_reconcile`, which is what §4.1 requires of
/// any reading.
pub fn v011_scaled_scene(
    scene: &HashMap<String, RawObservation>,
    k: f64,
) -> HashMap<String, RawObservation> {
    let mut out: HashMap<String, RawObservation> = HashMap::new();
    for (key, obs) in scene.iter() {
        let mut copied = obs.clone();
        if let Some((variety, v_env)) = copied.lenses.variety {
            copied.lenses.variety = Some((variety * k, v_env));
        }
        out.insert(key.clone(), copied);
    }
    out
}

/// States each entity's DoF as the product of its own counters, so that a mapped
/// copy satisfies the lens identity of §4.1 by construction.
pub fn v011_reconcile(state: &SystemStateMatrix) -> SystemStateMatrix {
    let mut clone = v011_copy(state);
    let keys: Vec<String> = clone.entities.keys().cloned().collect();
    for key in keys {
        let product = {
            let entity = &clone.entities[&key];
            lens_product(entity)
        };
        if let Some(p) = product {
            if let Some(entity) = clone.entities.get_mut(&key) {
                entity.current_dof = p;
                if let Some(m) = entity.measurement.as_mut() {
                    m.current_dof = p;
                }
            }
        }
    }
    clone
}

/// Sets one entity's DoF to `target` by scaling its own counters, which is the
/// only way a reading may arrive at a different DoF (§4.1).
pub fn v011_set_entity_dof(
    state: &SystemStateMatrix,
    entity_id: &str,
    target: f64,
) -> SystemStateMatrix {
    let mut clone = v011_copy(state);
    if let Some(entity) = clone.entities.get_mut(entity_id) {
        v011_set_product(entity, target);
    }
    clone
}

/// Removes every trace of τ: the resource-map observation is present but
/// **unmeasured**, which is what §3.2b calls an unknown τ. Zeroing the fields
/// instead would leave the deprecated mirror in place and read as a **measured** τ
/// of zero — a passed deadline, not an unmeasured budget.
pub fn v011_unknown_tau(state: &SystemStateMatrix) -> SystemStateMatrix {
    let mut clone = v011_copy(state);
    clone.tau = Some(ResourceObservation {
        value: None,
        unit: "mks".to_string(),
        scale: 1.0,
        source: "sensor".to_string(),
        last_measured_at: 0.0,
        aging_time: 0.0,
        estimated: None,
        estimation_source: Vec::new(),
    });
    clone.deadlines.clear();
    clone.global_time_to_collapse_mks = 0.0;
    clone
}

/// Drops an entity from a copy: the state-completeness defect of §3.6.
pub fn v011_without_entity(state: &SystemStateMatrix, entity_id: &str) -> SystemStateMatrix {
    let mut clone = v011_copy(state);
    clone.entities.remove(entity_id);
    clone
}

/// Adds an entity the observed state does not declare.
pub fn v011_with_extra_entity(state: &SystemStateMatrix) -> SystemStateMatrix {
    let mut clone = v011_copy(state);
    clone.entities.insert(
        "v011_extra".to_string(),
        EntityState::new(
            "v011_extra".to_string(),
            false,
            0.0,
            0.5,
            false,
            0.0,
        ),
    );
    clone
}

/// States a DoF its own counters do not produce.
pub fn v011_break_lens_product(
    state: &SystemStateMatrix,
    entity_id: &str,
) -> SystemStateMatrix {
    let mut clone = v011_copy(state);
    if let Some(entity) = clone.entities.get_mut(entity_id) {
        entity.current_dof = entity.current_dof * 2.0 + 0.5;
    }
    clone
}

/// Flips the collapse-source label of one entity: the defect §3.6 refuses, because
/// the label is honoured through an act of the shared graph.
pub fn v011_move_collapse_label(
    state: &SystemStateMatrix,
    entity_id: &str,
) -> SystemStateMatrix {
    let mut clone = v011_copy(state);
    if let Some(entity) = clone.entities.get_mut(entity_id) {
        entity.is_collapse_source = !entity.is_collapse_source;
    }
    clone
}

/// Gives a reading its own measurement durations: the defect §3.4.1/§3.4.2 refuse,
/// because the durations are ruler-level.
pub fn v011_change_durations(state: &SystemStateMatrix) -> SystemStateMatrix {
    let mut clone = v011_copy(state);
    clone.measurement_durations.insert(
        "variety".to_string(),
        std::collections::BTreeMap::from([
            ("t_m".to_string(), 123456.0),
            ("t_v".to_string(), 654321.0),
        ]),
    );
    clone
}

/// A candidate that adds `delta` to one entity, requires nothing and closes
/// nothing — the simplest well-formed option, in the flat form of §3.3.
pub fn v011_option(id: &str, entity_id: &str, delta: f64) -> ActionOption {
    ActionOption::new(
        id.to_string(),
        "v0.11 fixture candidate".to_string(),
        HashMap::from([(entity_id.to_string(), delta)]),
        true,
        1000.0,
    )
}

/// The `skip`-th entity id with a positive DoF, in sorted order, so the harness
/// does not hard-code a fixture name for its arithmetic.
pub fn v011_entity(state: &SystemStateMatrix, skip: usize) -> String {
    let mut ids: Vec<&String> = state.entities.keys().collect();
    ids.sort();
    let mut left = skip;
    for id in ids {
        if state.entities[id].current_dof > 0.0 {
            if left == 0 {
                return id.clone();
            }
            left -= 1;
        }
    }
    String::new()
}
/// The discriminating candidate of §6.3. Under `h_alt` it closes **two** means:
/// trainee's own mean, which drives its Variety counter to zero, and the supervise
/// mean, which removes the act path that would otherwise raise it back. Under the
/// observed reading it closes nothing.
///
/// The pair is what makes the check discriminate. With the closure declared only for
/// `h_alt`, the correct after-state has trainee at `DoF = 0` with no raising path — a
/// `proven_unreachable` verdict, so `D2 = 1` — while a port that recomputes the
/// counters from `closure[$observed$]` (the empty list) leaves trainee at its observed
/// `DoF > 0` and charges nothing. The released §4.5 scene is the world of this
/// fixture, so no new scene is needed.
pub fn v011_h_only_closer() -> ActionOption {
    let mut o = crate::options_v07::t1_mirror();
    o.option_id = "h_only_closer".to_string();
    o.projected_dof_delta.clear();
    o.projected_by_hypothesis = HashMap::from([
        (
            crate::hypothesis::OBSERVED_HYPOTHESIS_ID.to_string(),
            HashMap::from([("drone".to_string(), 0.1)]),
        ),
        (
            "h_alt".to_string(),
            HashMap::from([("drone".to_string(), 0.1)]),
        ),
    ]);
    o.closed.clear();
    o.closed_by_hypothesis = HashMap::from([
        (
            crate::hypothesis::OBSERVED_HYPOTHESIS_ID.to_string(),
            Vec::new(),
        ),
        (
            "h_alt".to_string(),
            crate::options_v07::closures(&[
                crate::fixture_v07::TRAINEE_MEAN,
                crate::fixture_v07::SUPERVISE_MEAN,
            ]),
        ),
    ]);
    o
}
