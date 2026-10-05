//! DOF-SPEC v0.7 fixture (§11.10) — the release's world, Rust mirror of
//! patterns/python/fixture_v07.py, patterns/go/fixture_v07.go and
//! patterns/cpp/fixture_v07.hpp.
//!
//! One world for all four ports plus RUN VARIANTS, never separate worlds. Every
//! number here is either taken from §11.10 or DERIVED by the same named procedures
//! the ports implement, so a port that disagrees shows up as a difference in a
//! derived value and not in a hand-copied constant.
//!
//! The world, in one paragraph: an acting agent with four resources and one exchange
//! group; three entities sitting at a known zero with three different reachability
//! verdicts (`passive` unreachable, `revivable` reachable through a medic's act
//! inside T_rec, `unobserved` undetermined because the observation is partial); a
//! `forged` entity that CLAIMS to be a collapse source while no observed act of
//! collapse exists; and `robot`, whose nine reachable means make the Variety counter
//! and the price of a closure measurable rather than illustrative.
//!
//! Nothing in this file may depend on the candidate set: the fixture is an
//! observation, and an observation that changed with the options offered would make
//! the decision unreproducible (§4.2).

use std::collections::{BTreeMap, HashMap};

use crate::graph_mapper::{RawObservation, ResourceLayer, RESOURCE_LAYER_KEY, WORLD_KEY};
use crate::measurement::{LensObservation, MandateValue, ResourceUnit};
use crate::world_graph::{
    ActEdge, EntityNode, ExchangeEdge, WorldGraph, WorldObservation,
};

pub const TREC_MKS: f64 = 4000000.0;
pub const HORIZON_MKS: f64 = 4000000.0;
pub const MEDKIT: &str = "medkit";

/// The admissible-means class and the per-entity recovery horizon. Entities at a
/// positive DoF get none: the verdict is only read for a known zero, and an
/// undeclared horizon yields `undetermined` — the honest answer for "nobody asked".
pub fn means_class() -> Vec<String> {
    vec!["medical".to_string(), "technical".to_string()]
}

pub fn t_rec() -> BTreeMap<String, f64> {
    let mut m = BTreeMap::new();
    m.insert("passive".to_string(), TREC_MKS);
    m.insert("revivable".to_string(), TREC_MKS);
    m.insert("unobserved".to_string(), TREC_MKS);
    m
}

pub fn numeraire() -> String {
    "credit".to_string()
}
pub const MANDATE_CAP: f64 = 4.0;
pub const EXTERNAL_LIMIT_CREDIT: f64 = 100.0;

pub fn group() -> Vec<String> {
    vec![
        "credit".to_string(),
        "energy".to_string(),
        "machine_hour".to_string(),
        "parts".to_string(),
    ]
}

pub fn means() -> BTreeMap<String, f64> {
    let mut m = BTreeMap::new();
    m.insert("credit".to_string(), 6.0);
    m.insert("energy".to_string(), 10.0);
    m.insert("machine_hour".to_string(), 2.0);
    m.insert("parts".to_string(), 0.0);
    m
}

pub fn resources() -> Vec<ResourceUnit> {
    vec![
        ResourceUnit { id: "credit".to_string(), unit: "RUB".to_string(), scale: 1.0 },
        ResourceUnit { id: "energy".to_string(), unit: "joule".to_string(), scale: 1.0 },
        ResourceUnit { id: "machine_hour".to_string(), unit: "hour".to_string(), scale: 1.0 },
        ResourceUnit { id: "parts".to_string(), unit: "piece".to_string(), scale: 1.0 },
    ]
}

/// 6·1 + 10·0.5 + 2·1.0 + 0·1.0 = 13.0     (§11.10 п.4)
pub const BALANCE_IN_NUMERAIRE: f64 = 13.0;

pub fn robot_means() -> Vec<String> {
    (1..=9).map(|i| format!("m{}", i)).collect()
}

/// §3.5 nodes with their completeness claim. `unobserved` is `partial`.
pub fn graph_entities(overrides: &BTreeMap<String, String>) -> BTreeMap<String, EntityNode> {
    let mut completeness: BTreeMap<String, String> = BTreeMap::new();
    for (eid, obs) in [
        ("adult", "complete"),
        ("child", "complete"),
        ("drone", "complete"),
        ("forged", "complete"),
        ("robot", "complete"),
        ("passive", "complete"),
        ("revivable", "complete"),
        ("unobserved", "partial"),
    ] {
        completeness.insert(eid.to_string(), obs.to_string());
    }
    for (k, v) in overrides.iter() {
        completeness.insert(k.clone(), v.clone());
    }
    let mut dof_by_id: BTreeMap<String, f64> = BTreeMap::new();
    for (eid, d) in [
        ("adult", 0.447856),
        ("child", 0.020833),
        ("drone", 0.25),
        ("forged", 0.3),
        ("robot", 0.755756),
        ("passive", 0.0),
        ("revivable", 0.0),
        ("unobserved", 0.0),
    ] {
        dof_by_id.insert(eid.to_string(), d);
    }
    let mut out: BTreeMap<String, EntityNode> = BTreeMap::new();
    for (eid, observation) in completeness.iter() {
        out.insert(
            eid.clone(),
            EntityNode {
                id: eid.clone(),
                observation: observation.clone(),
                current_dof: *dof_by_id.get(eid).unwrap_or(&0.0),
            },
        );
    }
    out
}

/// Every entity's declared V equals its own response vectors. `act_medkit` is
/// performed by `adult`: the verdict of §4.9 does not care who acts —
/// recoverability is about SOME admissible act raising the entity's DoF inside
/// T_rec, while V counts only the entity's own repertoire. The two questions are
/// deliberately different, and this act is where the difference is visible:
/// `revivable` has zero response vectors and is still recoverable.
pub fn graph_acts(include_forged_kill: bool) -> Vec<ActEdge> {
    let mut acts: Vec<ActEdge> = Vec::new();
    for i in 1..=9 {
        let mut a = ActEdge::default();
        a.id = format!("r{}", i);
        a.source = "robot".to_string();
        a.target = "robot".to_string();
        a.category = "technical".to_string();
        a.requires = vec![format!("m{}", i)];
        a.effect.insert("robot".to_string(), 0.01);
        a.duration_mks = 1000.0;
        acts.push(a);
    }
    for (owner, count) in [("adult", 3usize), ("child", 1), ("drone", 4), ("forged", 3)] {
        for i in 1..=count {
            let mut a = ActEdge::default();
            a.id = format!("a_{}_{}", owner, i);
            a.source = owner.to_string();
            a.target = owner.to_string();
            a.category = "technical".to_string();
            a.requires = vec![format!("q_{}_{}", owner, i)];
            a.effect.insert(owner.to_string(), 0.01);
            a.duration_mks = 1000.0;
            if a.id == "a_adult_3" {
                continue; // `adult` declared three response vectors
            }
            acts.push(a);
        }
    }
    // The medic: an admissible act by another entity, lifting a patient off the floor
    // within T_rec. It is one of `adult`'s three response vectors.
    let mut medkit = ActEdge::default();
    medkit.id = "act_medkit".to_string();
    medkit.source = "adult".to_string();
    medkit.target = "revivable".to_string();
    medkit.category = "medical".to_string();
    medkit.requires = vec![MEDKIT.to_string()];
    medkit.effect.insert("revivable".to_string(), 0.6);
    medkit.duration_mks = 2000000.0;
    acts.push(medkit);
    if include_forged_kill {
        // Run variant: with this act the forged label acquires a witness (§4.2) and
        // the entity leaves `calc` — measurable as +|ln 0.3| nats.
        let mut kill = ActEdge::default();
        kill.id = "act_kill_robot".to_string();
        kill.source = "forged".to_string();
        kill.target = "robot".to_string();
        kill.category = "technical".to_string();
        kill.effect.insert("robot".to_string(), -1.0);
        kill.duration_mks = 1000.0;
        acts.push(kill);
    }
    acts
}

/// Every mean the acts above require, plus the medic's kit.
pub fn graph_means() -> Vec<String> {
    let mut out = robot_means();
    out.push(MEDKIT.to_string());
    for (owner, count) in [("adult", 3usize), ("child", 1), ("drone", 4), ("forged", 3)] {
        for i in 1..=count {
            out.push(format!("q_{}_{}", owner, i));
        }
    }
    out
}

/// The market. Quotes are baskets (`gives` → `wants`), and the reverse edge
/// `machine_hour->credit` is not decoration: without a cycle the no-arbitrage
/// criterion would be vacuous.
pub fn exchanges() -> Vec<ExchangeEdge> {
    fn quote(id: &str, gives: (&str, f64), wants: (&str, f64), dur: f64) -> ExchangeEdge {
        let mut e = ExchangeEdge::default();
        e.id = id.to_string();
        e.gives.insert(gives.0.to_string(), gives.1);
        e.wants.insert(wants.0.to_string(), wants.1);
        e.duration_mks = dur;
        e
    }
    vec![
        quote("q1", ("credit", 1.0), ("energy", 2.0), 1000.0),
        quote("q2", ("energy", 1.0), ("machine_hour", 0.5), 1000.0),
        quote("q3", ("credit", 1.0), ("machine_hour", 1.0), 500.0),
        quote("q4", ("parts", 1.0), ("energy", 3.0), 1000.0),
        quote("q5", ("parts", 1.0), ("credit", 1.0), 1000.0),
        quote("q6", ("machine_hour", 1.0), ("credit", 0.5), 1000.0),
    ]
}

/// Raw observations of the eight entities, before any graph is attached.
pub fn entity_specs() -> HashMap<String, RawObservation> {
    fn raw(
        is_autonomous: bool,
        agency: f64,
        collapse: bool,
        ttc: f64,
        lenses: LensObservation,
    ) -> RawObservation {
        RawObservation {
            is_autonomous,
            agency_index: agency,
            is_collapse_source: collapse,
            time_to_collapse_mks: ttc,
            lenses,
            resource_layer: None,
            world: None,
            // §4.7 (v0.11): the historical fixture declares no measurement
            // durations and no schedule — the ruler stays exactly as it was.
            measurement_durations: None,
            measurement_schedule: None,
        }
    }
    let mut m: HashMap<String, RawObservation> = HashMap::new();

    let mut adult = LensObservation::default();
    adult.variety = Some((3.0, 2.0));
    adult.options = Some(vec![(1.0, 10.0)]);
    adult.constraint = Some((4.0, 1.0));
    m.insert("adult".to_string(), raw(true, 0.9, false, 1e8, adult));

    let mut child = LensObservation::default();
    child.variety = Some((1.0, 5.0));
    child.options = Some(vec![(2.0, 4.0)]);
    child.constraint = Some((1.0, 3.0));
    m.insert("child".to_string(), raw(false, 0.1, false, 4e6, child));

    let mut drone = LensObservation::default();
    drone.variety = Some((4.0, 2.0));
    let mut reqs: BTreeMap<String, f64> = BTreeMap::new();
    reqs.insert("energy".to_string(), 4.0);
    drone.requirements = Some(reqs);
    drone.constraint = Some((3.0, 1.0));
    m.insert("drone".to_string(), raw(true, 0.6, false, 1e8, drone));

    // Its own repertoire is empty (V = 0 ⇒ ψ_var = 0), so it sits at a known zero
    // while an admissible act raises it: at a zero, not proven dead.
    let mut revivable = LensObservation::default();
    revivable.variety = Some((0.0, 1.0));
    revivable.options = Some(vec![(1.0, 10.0)]);
    revivable.constraint = Some((1.0, 1.0));
    m.insert("revivable".to_string(), raw(false, 0.0, false, 1e8, revivable));

    // A passive object: no response vectors, no budget, no free variables.
    let mut passive = LensObservation::default();
    passive.variety = Some((0.0, 0.0));
    passive.options = Some(Vec::new());
    passive.constraint = Some((0.0, 0.0));
    m.insert("passive".to_string(), raw(false, 0.0, false, 1e8, passive));

    // The Options lens is UNMEASURED: u(t) applies, `dof_known` is false, and the
    // graph observation is partial — so it is held in `calc` twice over, and an
    // incomplete observation is never read as proof (§4.9).
    let mut unobserved = LensObservation::default();
    unobserved.variety = Some((0.0, 2.0));
    unobserved.constraint = Some((1.0, 1.0));
    m.insert("unobserved".to_string(), raw(false, 0.0, false, 1e8, unobserved));

    // Claims to be a collapse source, with no observed act of collapse: the label
    // alone must not move the index (it would, by |ln 0.3| = 1.204).
    let mut forged = LensObservation::default();
    forged.variety = Some((3.0, 2.0));
    forged.options = Some(vec![(1.0, 2.0)]);
    forged.constraint = Some((1.0, 0.0));
    m.insert("forged".to_string(), raw(true, 0.5, true, 1e8, forged));

    let mut robot = LensObservation::default();
    robot.variety = Some((9.0, 1.0));
    robot.options = Some(vec![(1.0, 10.0)]);
    robot.constraint = Some((9.0, 1.0));
    m.insert("robot".to_string(), raw(true, 0.4, false, 1e8, robot));

    m
}

/// Run variants of the one world.
#[derive(Clone, Default)]
pub struct Options {
    pub no_world: bool,
    pub include_forged_kill: bool,
    pub observation_overrides: BTreeMap<String, String>,
    pub exchanges_override: Option<Vec<ExchangeEdge>>,
    pub t_rec_override: Option<BTreeMap<String, f64>>,
    pub means_class_override: Option<Vec<String>>,
    pub numeraire_override: Option<String>,
    pub horizon_override: Option<f64>,
    pub means_override: Option<BTreeMap<String, f64>>,
    pub cap_override: Option<f64>,
    pub declare_rates: bool,
}

pub fn world_observation(opts: &Options) -> WorldObservation {
    let mut w = WorldObservation::default();
    let mut g = WorldGraph::default();
    g.entities = graph_entities(&opts.observation_overrides);
    g.means = graph_means();
    g.acts = graph_acts(opts.include_forged_kill);
    g.exchanges = opts.exchanges_override.clone().unwrap_or_else(exchanges);
    w.graph = g;
    w.means_class = opts
        .means_class_override
        .clone()
        .unwrap_or_else(means_class);
    w.t_rec = opts.t_rec_override.clone().unwrap_or_else(t_rec);
    w.counting_horizon_mks = Some(opts.horizon_override.unwrap_or(HORIZON_MKS));
    w.numeraire = Some(
        opts.numeraire_override
            .clone()
            .unwrap_or_else(numeraire),
    );
    w.procedure = "perception-v1:world_verdicts".to_string();
    w
}

/// The resource layer (§3.2/§4.8). `rates` is deliberately NOT declared: in v0.7 the
/// rate is the output of a procedure over the observation (§3.5), so the fixture
/// proves the derivation instead of restating it.
pub fn resource_layer(opts: &Options) -> ResourceLayer {
    let mut layer = ResourceLayer::default();
    layer.means = opts.means_override.clone().unwrap_or_else(means);
    layer.groups = vec![group()];
    layer.resources = resources();
    layer
        .mandate
        .insert("scope".to_string(), MandateValue::Text("household".to_string()));
    layer.mandate.insert(
        "cap".to_string(),
        MandateValue::Number(opts.cap_override.unwrap_or(MANDATE_CAP)),
    );
    layer.mandate.insert(
        "external_limit_credit".to_string(),
        MandateValue::Number(EXTERNAL_LIMIT_CREDIT),
    );
    if opts.declare_rates {
        layer.rates.insert(
            "credit->energy".to_string(),
            crate::measurement::Rate { rate: 2.0, duration_mks: 1000.0 },
        );
        layer.rates.insert(
            "energy->machine_hour".to_string(),
            crate::measurement::Rate { rate: 0.5, duration_mks: 1000.0 },
        );
        layer.rates.insert(
            "credit->machine_hour".to_string(),
            crate::measurement::Rate { rate: 1.0, duration_mks: 500.0 },
        );
    }
    layer
}

/// A complete raw observation mapping, fresh on every call.
pub fn scene(opts: &Options) -> HashMap<String, RawObservation> {
    let mut m = entity_specs();
    let layer_entry = RawObservation {
        is_autonomous: true,
        agency_index: 0.0,
        is_collapse_source: false,
        time_to_collapse_mks: 0.0,
        lenses: LensObservation::default(),
        resource_layer: Some(resource_layer(opts)),
        world: None,
        measurement_durations: None,
        measurement_schedule: None,
    };
    m.insert(RESOURCE_LAYER_KEY.to_string(), layer_entry);
    if !opts.no_world {
        let world_entry = RawObservation {
            is_autonomous: true,
            agency_index: 0.0,
            is_collapse_source: false,
            time_to_collapse_mks: 0.0,
            lenses: LensObservation::default(),
            resource_layer: None,
            world: Some(world_observation(opts)),
            measurement_durations: None,
            measurement_schedule: None,
        };
        m.insert(WORLD_KEY.to_string(), world_entry);
    }
    m
}

/// §11.10 п.3, variant B: an observation that is not arbitrage-free.
/// `credit->energy = 5.0` closes a cycle with product `5.0·0.5·0.5 = 1.25 > 1`, so
/// the rate is not "very favourable", it is UNDETERMINED and no exchange happens at
/// all: a hole in the observation is not a discount.
pub fn arbitrage_scene() -> HashMap<String, RawObservation> {
    let mut quotes = exchanges();
    for e in quotes.iter_mut() {
        if e.id == "q1" {
            e.wants.insert("energy".to_string(), 5.0);
        }
    }
    let opts = Options {
        exchanges_override: Some(quotes),
        ..Options::default()
    };
    scene(&opts)
}

/// The unit-declaration variant: the same world with `energy` declared in kJ.
pub fn scene_other_units() -> HashMap<String, RawObservation> {
    let mut m = scene(&Options::default());
    if let Some(entry) = m.get_mut(RESOURCE_LAYER_KEY) {
        if let Some(layer) = entry.resource_layer.as_mut() {
            for r in layer.resources.iter_mut() {
                if r.id == "energy" {
                    r.unit = "kilojoule".to_string();
                    r.scale = 1000.0;
                }
            }
        }
    }
    m
}

// ---------------------------------------------------------------------------
// §4.5 (v0.8) run variant: the "compensation" fixture
// ---------------------------------------------------------------------------

/// The means of the v0.8 run variant.
pub const SUPERVISE_MEAN: &str = "radio";
pub const TRAINEE_MEAN: &str = "q_trainee_1";
pub const MENTOR_MEAN: &str = "q_mentor_2";

/// The released world plus the two entities D3 exists for (§4.5, T1).
///
/// `trainee` CAN act (V = 1) but its own act does not raise its own DoF, so its
/// recoverability rests entirely on someone else's act. `mentor` is that someone:
/// it performs `act_supervise` and holds a second vector of its own, so closing the
/// mean behind the act costs the mentor a response vector — a price inside the
/// index — without driving it to a known zero, which would make the option
/// destructive instead of merely path-cutting. Both sit at 0.125 exactly:
/// ψ_var = ½, ψ_opt = 4^(−½) = ½, ψ_con = ½.
///
/// Built as a POST-PROCESSING of the released scene, not as a branch inside it:
/// the released fixture's own numbers and digests must not move.
pub fn t1_scene() -> HashMap<String, RawObservation> {
    let mut m = scene(&Options::default());

    let mut mentor = LensObservation::default();
    mentor.variety = Some((2.0, 2.0));
    mentor.options = Some(vec![(1.0, 2.0)]);
    mentor.constraint = Some((1.0, 1.0));
    m.insert(
        "mentor".to_string(),
        RawObservation {
            is_autonomous: true,
            agency_index: 0.5,
            is_collapse_source: false,
            time_to_collapse_mks: 1e8,
            lenses: mentor,
            resource_layer: None,
            world: None,
            // §4.7 (v0.11): the historical fixture declares no measurement
            // durations and no schedule — the ruler stays exactly as it was.
            measurement_durations: None,
            measurement_schedule: None,
        },
    );

    let mut trainee = LensObservation::default();
    trainee.variety = Some((1.0, 1.0));
    trainee.options = Some(vec![(1.0, 2.0)]);
    trainee.constraint = Some((1.0, 1.0));
    m.insert(
        "trainee".to_string(),
        RawObservation {
            is_autonomous: false,
            agency_index: 0.2,
            is_collapse_source: false,
            time_to_collapse_mks: 1e8,
            lenses: trainee,
            resource_layer: None,
            world: None,
            // §4.7 (v0.11): the historical fixture declares no measurement
            // durations and no schedule — the ruler stays exactly as it was.
            measurement_durations: None,
            measurement_schedule: None,
        },
    );

    if let Some(entry) = m.get_mut(WORLD_KEY) {
        if let Some(w) = entry.world.as_mut() {
            for (id, dof) in [("mentor", 0.125), ("trainee", 0.125)] {
                w.graph.entities.insert(
                    id.to_string(),
                    EntityNode {
                        id: id.to_string(),
                        observation: "complete".to_string(),
                        current_dof: dof,
                    },
                );
            }
            for mean in [SUPERVISE_MEAN, TRAINEE_MEAN, MENTOR_MEAN] {
                w.graph.means.push(mean.to_string());
            }
            // The trainee's own vector: it acts, on the robot, and never on itself
            // — which is why losing the mentor's act costs it the path while its V
            // stays above zero (so there is no collapse charge).
            let mut trainee_act = ActEdge::default();
            trainee_act.id = "a_trainee_1".to_string();
            trainee_act.source = "trainee".to_string();
            trainee_act.target = "robot".to_string();
            trainee_act.category = "technical".to_string();
            trainee_act.requires = vec![TRAINEE_MEAN.to_string()];
            trainee_act.effect.insert("robot".to_string(), 0.01);
            trainee_act.duration_mks = 1000.0;
            w.graph.acts.push(trainee_act);
            let mut supervise = ActEdge::default();
            supervise.id = "act_supervise".to_string();
            supervise.source = "mentor".to_string();
            supervise.target = "trainee".to_string();
            supervise.category = "technical".to_string();
            supervise.requires = vec![SUPERVISE_MEAN.to_string()];
            supervise.effect.insert("trainee".to_string(), 0.1);
            supervise.duration_mks = 1000.0;
            w.graph.acts.push(supervise);
            let mut mentor_act = ActEdge::default();
            mentor_act.id = "a_mentor_2".to_string();
            mentor_act.source = "mentor".to_string();
            mentor_act.target = "mentor".to_string();
            mentor_act.category = "technical".to_string();
            mentor_act.requires = vec![MENTOR_MEAN.to_string()];
            mentor_act.effect.insert("mentor".to_string(), 0.01);
            mentor_act.duration_mks = 1000.0;
            w.graph.acts.push(mentor_act);
            // A horizon is what makes a verdict a verdict: with no T_rec the
            // trainee's answer would be `undetermined`, and D2 counts only a LOST
            // `reachable`.
            w.t_rec.insert("trainee".to_string(), TREC_MKS);
        }
    }
    m
}
