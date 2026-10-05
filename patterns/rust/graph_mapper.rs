// DOF-Core Perception & Mapping layer (Rust port).
// Builds a SystemStateMatrix **through the measurement layer** (§4.6–§4.8):
// raw lens inputs -> ψ per lens -> the product that becomes `current_dof`,
// plus the frozen declaration and its digest (§3.4), plus the acting agent's
// means and the exchange layer the resource gate of §4.8 decides against.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::dof_core::{EntityState, ObservationContext, ResourceObservation, SystemStateMatrix};
use crate::measurement::{
    measure_entity, LensObservation, MandateValue, MeasurementDeclaration, PsiReference, Rate,
    ResourceUnit, VerdictRecord,
};
use crate::world_graph::{WorldGraph, WorldObservation};

/// The reserved observation that carries the resource layer (§3.2, §4.8) instead
/// of describing an entity. It is part of the ruler: its units, groups and rates
/// enter the hashed declaration, so a ruler that declares different units is a
/// different ruler.
pub const RESOURCE_LAYER_KEY: &str = "resource_layer";

/// The reserved key that carries the §3.5 observation of the world. Like the
/// resource layer it is an *observation*, not an entity: the same state plus a
/// different observation is a different decision, and the report says which one.
pub const WORLD_KEY: &str = "world";

/// §3.4.1/§4.7 (v0.11): the reserved observation that carries the declared
/// measurement durations `t_m`, `t_v` per lens. They are **declared ruler
/// content**, never inferred: two states differing only in `t_m` produce
/// different `ruler_digest`, and a hypothesis may not reinterpret how long the
/// measuring takes (§3.6).
pub const MEASUREMENT_DURATIONS_KEY: &str = "measurement_durations";

/// §4.7 (v0.11): the reserved observation that carries the declared **schedule**
/// `t` per lens — when the measurement is planned to happen. An undeclared
/// schedule reads as `t = 0` (`u₀`); a declared `t > t*` is non-conformant input,
/// never clamped.
pub const MEASUREMENT_SCHEDULE_KEY: &str = "measurement_schedule";

/// Whether an observation key is reserved: a description of the ruler or of the
/// world, not of an entity. A reserved entry left in the entity loop would drag τ
/// down to its own default of zero.
pub fn is_reserved_key(key: &str) -> bool {
    key == RESOURCE_LAYER_KEY
        || key == WORLD_KEY
        || key == MEASUREMENT_DURATIONS_KEY
        || key == MEASUREMENT_SCHEDULE_KEY
}

/// §4.6/§4.9: declared derived numbers MUST equal what their procedures compute.
/// Returns the mismatches (empty = the ruler is honest). A declaration that claims a
/// counter its own observation does not support is exactly the "declared, not
/// derived" defect this revision removes.
pub fn verify_graph_derived(
    declaration: &MeasurementDeclaration,
    graph: &WorldGraph,
    counting_horizon: Option<f64>,
) -> Vec<String> {
    let mut problems: Vec<String> = Vec::new();
    for (entity_id, declared) in declaration.verdicts.iter() {
        let v_here = graph.v_count(entity_id, &declaration.means_class, counting_horizon);
        if declared.v as usize != v_here {
            problems.push(format!(
                "{}: declared V={} but the counting procedure gives {}",
                entity_id, declared.v, v_here
            ));
        }
        let verdict_here = graph
            .verdict(entity_id, &declaration.means_class, declared.t_rec_mks)
            .verdict;
        if declared.verdict != verdict_here {
            problems.push(format!(
                "{}: declared verdict {} but the verdict procedure returns {}",
                entity_id, declared.verdict, verdict_here
            ));
        }
    }
    problems
}

/// The acting agent's means and the observed exchange layer.
#[derive(Clone, Debug, Default)]
pub struct ResourceLayer {
    pub means: BTreeMap<String, f64>,
    pub groups: Vec<Vec<String>>,
    pub rates: BTreeMap<String, Rate>,
    pub resources: Vec<ResourceUnit>,
    pub mandate: BTreeMap<String, MandateValue>,
    /// §3.2b (v0.11): the declared individual deadlines τ is derived from. An
    /// entry whose value is `None` is an **active but unmeasured** deadline, and
    /// it makes τ unknown rather than letting the measured deadlines decide.
    pub deadlines: BTreeMap<String, Option<f64>>,
    /// §3.2b (v0.11): the `tau` observation of the resource map. `None` means not
    /// declared — τ then falls through to the legacy entity-minimum;
    /// `Some(None)` means declared but **unmeasured**, which is `null` and not
    /// `0.0`; `Some(Some(v))` is a measured τ, negative when the deadline passed.
    pub tau_observation: Option<Option<f64>>,
}

/// Raw observation of one entity. A lens left `None` is **unmeasured**: u(t)
/// applies to it, `dof_known` becomes false, and §4.2 keeps the entity in
/// `calc` — ignorance is never zero and never ideal.
#[derive(Clone)]
pub struct RawObservation {
    pub is_autonomous: bool,
    pub agency_index: f64,
    pub is_collapse_source: bool,
    pub time_to_collapse_mks: f64,
    pub lenses: LensObservation,
    /// Set on the reserved `resource_layer` entry only.
    pub resource_layer: Option<ResourceLayer>,
    /// Set on the reserved `world` entry only: the §3.5 observation of the world.
    pub world: Option<WorldObservation>,
    /// Set on the reserved `measurement_durations` entry only (§4.7, v0.11): the
    /// declared `t_m`, `t_v` per lens.
    pub measurement_durations: Option<BTreeMap<String, BTreeMap<String, f64>>>,
    /// Set on the reserved `measurement_schedule` entry only (§4.7, v0.11): the
    /// declared `t` per lens.
    pub measurement_schedule: Option<BTreeMap<String, f64>>,
}

pub struct GraphMapper {
    pub context_switch_cost: f64,
    pub psi_id: String,
    pub u0_prior_q: Option<f64>,
    /// The declaration frozen on the state being built; the orchestrator hands
    /// it to the audit report (§6.2).
    pub last_declaration: Option<MeasurementDeclaration>,
    /// The observation the state was decided over (§3.5/§4.9), kept beside the state
    /// and never inside it: a world graph is a Perception artifact, exactly like the
    /// derived groups and the observed rates.
    pub last_observation: Option<ObservationContext>,
    /// Mismatches between the declared derived numbers and what the named procedures
    /// recompute over the observation (§4.6/§4.9). Empty = honest.
    pub last_graph_problems: Vec<String>,
}

impl GraphMapper {
    pub fn new(context_switch_cost: f64) -> Self {
        GraphMapper {
            context_switch_cost,
            psi_id: "perception-v1".to_string(),
            u0_prior_q: None,
            last_declaration: None,
            last_observation: None,
            last_graph_problems: Vec::new(),
        }
    }

    pub fn poll_environment(&mut self, raw: &HashMap<String, RawObservation>) -> SystemStateMatrix {
        // The resource layer is analysis-side data, not an entity: pull it out
        // first and skip that key in both entity passes.
        let layer: Option<ResourceLayer> = raw
            .get(RESOURCE_LAYER_KEY)
            .and_then(|obs| obs.resource_layer.clone());
        let empty_means: BTreeMap<String, f64> = BTreeMap::new();
        let empty_groups: Vec<Vec<String>> = Vec::new();
        let means = layer.as_ref().map(|l| &l.means).unwrap_or(&empty_means);
        let groups = layer.as_ref().map(|l| &l.groups).unwrap_or(&empty_groups);

        let mut observations: BTreeMap<String, LensObservation> = BTreeMap::new();
        let mut min_ttc = f64::INFINITY;

        // Pass 1: raw lens inputs and the local deadlines. BOTH reserved keys are
        // skipped: the resource layer and the world observation describe the
        // world/agent, not an entity, and a reserved entry left in this loop would
        // drag τ down to its own default of zero.
        for (eid, obs) in raw.iter() {
            if is_reserved_key(eid) {
                continue;
            }
            observations.insert(eid.clone(), obs.lenses.clone());
            if !obs.is_collapse_source && obs.time_to_collapse_mks < min_ttc {
                min_ttc = obs.time_to_collapse_mks;
            }
        }

        // Global τ is driven by the most urgent non-collapse-source entity (§3.2).
        // Since `v0.11` this is the **deprecated mirror**, not τ: it is kept so the
        // historical fixtures of `v0.6`–`v0.9.1` still read as they did (§3.2b).
        let global_ttc = if min_ttc.is_finite() { min_ttc } else { 1e15 };

        // §3.2b (v0.11): τ is read from the **resource map**, and the individual
        // deadlines declared alongside it govern it. Three cases, in this order:
        //
        //   1. declared individual deadlines — τ is their minimum, and is `None`
        //      when **any** active deadline is unmeasured: taking the minimum over
        //      the measured ones alone would read an unknown timer as absent;
        //   2. otherwise the `tau` observation of the map, which may be negative
        //      (a passed deadline keeps its magnitude) or `None`;
        //   3. otherwise the legacy entity-minimum.
        let declared_deadlines: BTreeMap<String, Option<f64>> = layer
            .as_ref()
            .map(|l| l.deadlines.clone())
            .unwrap_or_default();
        let tau_value: Option<f64> = if !declared_deadlines.is_empty() {
            if declared_deadlines.values().any(|v| v.is_none()) {
                None
            } else {
                declared_deadlines.values().filter_map(|v| *v).reduce(f64::min)
            }
        } else if let Some(declared) = layer.as_ref().and_then(|l| l.tau_observation) {
            declared
        } else {
            Some(global_ttc)
        };
        // §3.2b: the deprecated mirror is clamped and is **not** τ. It is `0.0`
        // for an unknown and for a passed deadline, and equals τ otherwise.
        let mirror_ttc = match tau_value {
            Some(v) if v >= 0.0 => v,
            _ => 0.0,
        };

        // §3.4.1/§3.4.2 (v0.11): the measurement durations and the schedule are
        // **declared ruler content**. They are read from the observation, never
        // inferred (§4.7), and they are the same for every reading of the cycle — a
        // hypothesis reinterprets what was measured, not how long the measuring
        // takes.
        let measurement_durations: BTreeMap<String, BTreeMap<String, f64>> = raw
            .get(MEASUREMENT_DURATIONS_KEY)
            .and_then(|o| o.measurement_durations.clone())
            .unwrap_or_default();
        let measurement_schedule: BTreeMap<String, f64> = raw
            .get(MEASUREMENT_SCHEDULE_KEY)
            .and_then(|o| o.measurement_schedule.clone())
            .unwrap_or_default();

        // §3.5 (v0.7): the observed world graph, when the cycle was given one.
        let world: Option<WorldObservation> =
            raw.get(WORLD_KEY).and_then(|obs| obs.world.clone());
        let empty_graph = WorldGraph::default();
        let graph = world.as_ref().map(|w| &w.graph).unwrap_or(&empty_graph);
        let no_class: Vec<String> = Vec::new();
        let no_trec: BTreeMap<String, f64> = BTreeMap::new();
        let means_class = world.as_ref().map(|w| &w.means_class).unwrap_or(&no_class);
        let t_rec = world.as_ref().map(|w| &w.t_rec).unwrap_or(&no_trec);
        let mut counting_horizon = world.as_ref().and_then(|w| w.counting_horizon_mks);
        // §4.9: a response vector must be executable inside the counting horizon, so
        // the default is the cycle's own τ — never an implicit, invisible horizon.
        if world.is_some() && counting_horizon.is_none() {
            counting_horizon = Some(global_ttc);
        }

        // §4.6/§4.8 (v0.7): the numeraire weights, the observed rate table and the
        // mandate cap are DERIVED over the observation, not authored. Without a
        // declared numeraire there is no unit for a scalar cap, so neither applies —
        // which keeps a ruler without a world graph reading exactly as in v0.6.
        let mut weights: BTreeMap<String, f64> = BTreeMap::new();
        let mut derived_rates: BTreeMap<String, Rate> = BTreeMap::new();
        let mut mandate_cap: Option<f64> = None;
        let mut verdicts: BTreeMap<String, VerdictRecord> = BTreeMap::new();
        if let Some(w) = world.as_ref() {
            if let Some(numeraire) = w.numeraire.as_ref() {
                let mut member_set: BTreeSet<String> = BTreeSet::new();
                for grp in groups.iter() {
                    for r in grp.iter() {
                        member_set.insert(r.clone());
                    }
                }
                let members: Vec<String> = member_set.into_iter().collect();
                weights = graph.weights_to(numeraire, &members);
                // §3.5/§4.8: the axis rates are the *output* of the observation
                // procedure, so with a graph in hand the table is derived rather than
                // read from the layer. A declared table beside an observed graph would
                // be a second ruler for the same quantity, free to drift.
                for a in members.iter() {
                    for b in members.iter() {
                        if a == b {
                            continue;
                        }
                        let res = graph.rate(a, b, true);
                        if res.status == "observed" {
                            if let Some(rate) = res.rate {
                                derived_rates.insert(
                                    format!("{}->{}", a, b),
                                    Rate { rate, duration_mks: res.duration_mks },
                                );
                            }
                        }
                    }
                }
                if let Some(l) = layer.as_ref() {
                    let mut limits: Vec<f64> = Vec::new();
                    for (key, value) in l.mandate.iter() {
                        match (key.as_str(), value) {
                            ("cap", MandateValue::Number(n)) => limits.push(*n),
                            ("external_limit_credit", MandateValue::Number(n)) => {
                                // Declared in credits, applied in the numeraire:
                                // converted through the *observed* weight, never a
                                // hard-coded 1.0.
                                if let Some(wc) = weights.get("credit") {
                                    limits.push(*n * wc);
                                }
                            }
                            _ => {}
                        }
                    }
                    if !limits.is_empty() {
                        mandate_cap = Some(limits.iter().copied().fold(f64::INFINITY, f64::min));
                    }
                }
                // §4.6/§4.9: the verdicts and counters are computed by the named
                // procedures and then declared, so the declaration can be checked
                // against the observation it came from.
                for eid in observations.keys() {
                    let horizon = t_rec.get(eid).copied();
                    let v = graph.verdict(eid, means_class, horizon);
                    verdicts.insert(
                        eid.clone(),
                        VerdictRecord {
                            verdict: v.verdict,
                            t_rec_mks: horizon,
                            v: graph.v_count(eid, means_class, counting_horizon) as i64,
                        },
                    );
                }
            }
        }

        // Pass 2: the declaration is frozen on S, so τ is known before measuring.
        let mut declaration = MeasurementDeclaration::new(
            &self.psi_id,
            observations,
            // §3.2b (v0.11): the declaration freezes **τ**, not the deprecated
            // entity-minimum mirror — `None` when unmeasured.
            tau_value,
            self.u0_prior_q,
            layer.as_ref().map(|l| l.resources.clone()).unwrap_or_default(),
            layer.as_ref().map(|l| l.groups.clone()).unwrap_or_default(),
            layer.as_ref().map(|l| l.rates.clone()).unwrap_or_default(),
            layer.as_ref().map(|l| l.mandate.clone()).unwrap_or_default(),
            measurement_durations.clone(),
        );
        if world.is_some() {
            // The derived table replaces a declared one whenever a graph is in hand.
            if !derived_rates.is_empty() {
                declaration.rates = derived_rates;
            }
            declaration.numeraire = world.as_ref().and_then(|w| w.numeraire.clone());
            declaration.weights = weights.clone();
            declaration.mandate_cap = mandate_cap;
            declaration.verdicts = verdicts;
            declaration.means_class = means_class.clone();
            declaration.graph_procedure = world
                .as_ref()
                .map(|w| w.procedure.clone())
                .unwrap_or_default();
        }
        let u0 = declaration.u0(); // at t = 0 the schedule of §4.7 gives u₀

        let mut entities: HashMap<String, EntityState> = HashMap::new();
        for (eid, obs) in raw.iter() {
            if is_reserved_key(eid) {
                continue;
            }
            let mz = measure_entity(
                eid,
                &obs.lenses,
                u0,
                Some(means),
                Some(groups),
                Some(&weights),
                mandate_cap,
            );
            let mut ent = EntityState::new(
                eid.clone(),
                obs.is_autonomous,
                obs.agency_index.max(0.0).min(1.0),
                mz.current_dof,
                obs.is_collapse_source,
                obs.time_to_collapse_mks,
            );
            ent.dof_known = mz.dof_known;
            ent.measurement = Some(mz);
            entities.insert(eid.clone(), ent);
        }

        let reference = PsiReference {
            id: declaration.psi_id.clone(),
            digest: declaration.digest(),
        };
        // v0.9.1: convert flat means to ResourceObservation with metadata.
        let mut resources: HashMap<String, ResourceObservation> = HashMap::new();
        for (rid, val) in means.iter() {
            let mut obs = ResourceObservation {
                value: Some(*val),
                ..Default::default()
            };
            // Find the declared unit for this resource.
            if let Some(layer) = layer.as_ref() {
                for ru in layer.resources.iter() {
                    if ru.id == *rid {
                        obs.unit = ru.unit.clone();
                        obs.scale = ru.scale;
                        break;
                    }
                }
            }
            obs.source = "sensor".to_string();
            obs.aging_time = 3600.0;
            resources.insert(rid.clone(), obs);
        }
        // v0.9.1: τ is stored as ResourceObservation under state.tau, not in resources.
        // §3.2b (v0.11): the observation carries **τ** and its own provenance. The
        // `source` names where it came from — a declared deadline set, the resource
        // map, or the legacy entity-minimum — so a reader can tell a measured τ
        // from the deprecated mirror instead of inferring it from the value.
        let tau_source = if !declared_deadlines.is_empty() {
            "deadlines"
        } else if layer.as_ref().and_then(|l| l.tau_observation).is_some() {
            "resource_map"
        } else {
            "entity_min"
        };
        let tau_obs = ResourceObservation {
            value: tau_value,
            unit: "us".to_string(),
            scale: 1.0,
            source: tau_source.to_string(),
            aging_time: 0.0,
            ..Default::default()
        };

        let state = SystemStateMatrix {
            // §3.2b (v0.11): the deprecated mirror is clamped — `0.0` for an unknown
            // and for a passed deadline — and is not τ. It stays so the historical
            // fixtures of `v0.6`–`v0.9.1` still read as they did.
            global_time_to_collapse_mks: mirror_ttc,
            context_switch_cost: self.context_switch_cost,
            entities,
            psi: Some(reference),
            resources,
            tau: Some(tau_obs),
            deadlines: declared_deadlines,
            measurement_durations,
            measurement_schedule,
        };

        // §3.5/§4.9: the observation itself, pinned by its own digest (§6.2), and the
        // self-check that the declared derived numbers are the ones the named
        // procedures actually return over it.
        self.last_observation = None;
        self.last_graph_problems = Vec::new();
        if let Some(w) = world.as_ref() {
            self.last_observation = Some(ObservationContext {
                world: w.graph.clone(),
                means_class: means_class.clone(),
                t_rec: t_rec.clone(),
                counting_horizon_mks: counting_horizon,
                observation_digest: w
                    .graph
                    .observation_digest(means_class, t_rec, counting_horizon),
                // §3.6/§4.9 (v0.11): the observed reading carries no override — it
                // reads the graph's own `DoF(X)`. A hypothesis reading installs its
                // own numbers here via `with_dof`, and nothing else about the
                // observation changes.
                dof_override: None,
            });
            self.last_graph_problems =
                verify_graph_derived(&declaration, &w.graph, counting_horizon);
        }
        self.last_declaration = Some(declaration);

        state
    }
}
