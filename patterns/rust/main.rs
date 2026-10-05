// DOF-Core Rust SDK — entry point / smoke test.
// Mirrors patterns/smoke_test.py: the same facts on the same fixture, plus a
// check that the canonical declaration digest matches the other ports.

mod dof_core;
mod fixture_v07;
mod generator;
mod graph_mapper;
mod harness_v07;
mod harness_v08;
mod harness_v091;
mod measurement;
mod options_v07;
mod orchestrator;
mod world_graph;

use std::collections::{BTreeMap, HashMap};

use dof_core::{resource_map_equal, ActionOption, DofCalculusCore, ReportInput};
use graph_mapper::{RawObservation, ResourceLayer, RESOURCE_LAYER_KEY};
use measurement::{
    derive_blocks, psi_con, psi_opt, psi_var, LensObservation, MandateValue, Rate, ResourceUnit,
    EPSILON,
};
use orchestrator::DofOrchestrator;

// Two harnesses live in this port and both stay runnable, because a release must
// carry its own evidence and the previous release's:
//
//   ./dof_rust v07     # the release's conformance suite (harness_v07.rs), default
//   ./dof_rust v06     # the v0.6 harness (this file), historical evidence
//   ./dof_rust dump    # the canonical text and both frozen digests
//
// `v07` is the default so that any tool that builds and runs the port without
// arguments exercises the current release.

/// Reference digest of the shared fixture declaration (computed by the Python port).
const EXPECTED_DIGEST: &str = "bed37c25fd9cb757e9ea4a861c01cd4660fd896a83cd39b7c73b8e0be7489ad4";

fn check(failures: &mut Vec<String>, name: &str, ok: bool, detail: &str) {
    let mark = if ok { "  OK   " } else { "  FAIL " };
    if !ok {
        failures.push(name.to_string());
    }
    if detail.is_empty() {
        println!("{}{}", mark, name);
    } else {
        println!("{}{}  {}", mark, name, detail);
    }
}

fn obs(agency: f64, collapse: bool, ttc: f64, lenses: LensObservation) -> RawObservation {
    RawObservation {
        is_autonomous: true,
        agency_index: agency,
        is_collapse_source: collapse,
        time_to_collapse_mks: ttc,
        lenses,
        resource_layer: None,
        world: None,
        measurement_durations: None,
        measurement_schedule: None,
    }
}

/// The shared fixture: the same five entities as the other ports, including a
/// passive object and an entity whose Options lens was never measured.
fn fixture() -> HashMap<String, RawObservation> {
    let mut m = HashMap::new();
    m.insert(
        "adult".to_string(),
        obs(
            0.9,
            false,
            100000000.0,
            LensObservation {
                variety: Some((3.0, 2.0)),
                options: Some(vec![(1.0, 10.0)]),
                constraint: Some((4.0, 1.0)),
                requirements: None,
                ..LensObservation::default()
            },
        ),
    );
    m.insert(
        "child".to_string(),
        obs(
            0.1,
            false,
            4000000.0,
            LensObservation {
                variety: Some((1.0, 5.0)),
                options: Some(vec![(2.0, 4.0)]),
                constraint: Some((1.0, 3.0)),
                requirements: None,
                ..LensObservation::default()
            },
        ),
    );
    m.insert(
        "aggressor".to_string(),
        obs(
            0.5,
            true,
            100000000.0,
            LensObservation {
                variety: Some((5.0, 1.0)),
                options: Some(vec![(1.0, 100.0)]),
                constraint: Some((5.0, 1.0)),
                requirements: None,
                ..LensObservation::default()
            },
        ),
    );
    m.insert(
        "stone".to_string(),
        obs(
            0.0,
            false,
            100000000.0,
            LensObservation {
                variety: Some((0.0, 0.0)),
                options: Some(vec![]),
                constraint: Some((0.0, 0.0)),
                requirements: None,
                ..LensObservation::default()
            },
        ),
    );
    m.insert(
        "unmapped".to_string(),
        obs(
            0.4,
            false,
            100000000.0,
            LensObservation {
                variety: Some((2.0, 2.0)),
                options: None,
                constraint: Some((1.0, 1.0)),
                requirements: None,
                ..LensObservation::default()
            },
        ),
    );

    // v0.6: this entity declares what its transitions *require*, not the blocks
    // themselves — the blocks are derived against the agent's means (§4.6).
    let mut drone = LensObservation::default();
    drone.variety = Some((4.0, 2.0));
    drone.requirements = Some(BTreeMap::from([("energy".to_string(), 4.0)]));
    drone.constraint = Some((3.0, 1.0));
    m.insert("drone".to_string(), obs(0.6, false, 100000000.0, drone));

    // §3.2/§4.8 (v0.6): the acting agent, the derived groups, the observed rates,
    // the declared units and the mandate. Part of the ruler: the declaration
    // carries it, so a ruler with different units is a different ruler.
    let mut layer = ResourceLayer::default();
    layer.means = BTreeMap::from([
        ("credit".to_string(), 6.0),
        ("energy".to_string(), 10.0),
    ]);
    layer.groups = vec![vec!["credit".to_string(), "energy".to_string()]];
    layer.rates = BTreeMap::from([(
        "credit->energy".to_string(),
        Rate {
            rate: 2.0,
            duration_mks: 1000.0,
        },
    )]);
    layer.resources = vec![
        ResourceUnit {
            id: "credit".to_string(),
            unit: "credit".to_string(),
            scale: 1.0,
        },
        ResourceUnit {
            id: "energy".to_string(),
            unit: "joule".to_string(),
            scale: 1.0,
        },
    ];
    layer.mandate = BTreeMap::from([
        (
            "external_limit_credit".to_string(),
            MandateValue::Number(100.0),
        ),
        (
            "scope".to_string(),
            MandateValue::Text("household".to_string()),
        ),
    ]);
    let mut layer_obs = obs(0.0, false, 100000000.0, LensObservation::default());
    layer_obs.resource_layer = Some(layer);
    m.insert(RESOURCE_LAYER_KEY.to_string(), layer_obs);

    m
}

fn with_deadline(source: &HashMap<String, RawObservation>, ttc: f64) -> HashMap<String, RawObservation> {
    source
        .iter()
        .map(|(id, o)| {
            let mut copied = o.clone();
            if id != RESOURCE_LAYER_KEY {
                copied.time_to_collapse_mks = ttc;
            }
            (id.clone(), copied)
        })
        .collect()
}

fn run_harness_v06() -> Vec<String> {
    let mut failures: Vec<String> = Vec::new();
    let mut orch = DofOrchestrator::new(0.05);
    let state = orch.measure(&fixture());
    let core = DofCalculusCore::new();
    let (selected, report) = orch.step_with_report(&fixture());

    println!("=== 1. §3.4.3: the canonical ruler ===");
    let digest = state.psi.as_ref().map(|p| p.digest.clone()).unwrap_or_default();
    check(&mut failures, "digest matches the Python port", digest == EXPECTED_DIGEST, &format!("{}…", &digest[..16]));
    check(&mut failures, "fixture 1 selected an option", selected.is_some(), "");

    println!("=== 2. §4.1 / §4.6: per-entity values (reference: Python port) ===");
    let expected: [(&str, f64, f64); 6] = [
        ("adult", 0.417864270382, -0.872598611192),
        ("child", 0.020833333333, -3.871201010908),
        ("aggressor", 0.684883822565, -0.378506057199),
        ("drone", 0.353553390593, -1.039720770840),
        ("stone", 0.000000000000, -13.815510557964),
        ("unmapped", 0.125000000000, -2.079441541680),
    ];
    for (id, exp_dof, exp_contrib) in expected.iter() {
        let ent = state.entities.get(*id).expect("entity present");
        let m = ent.measurement.as_ref().expect("measurement present");
        check(
            &mut failures,
            &format!("{}: current_dof = lens product, contribution", id),
            (ent.current_dof - exp_dof).abs() < 1e-9 && (m.contribution - exp_contrib).abs() < 1e-9,
            &format!("dof={:.12} contrib={:.12}", ent.current_dof, m.contribution),
        );
        if !m.floored {
            check(
                &mut failures,
                &format!("{}: Σ terms == contribution", id),
                (m.terms_sum - m.contribution).abs() < 1e-12,
                "",
            );
        }
    }

    println!("=== 3. §4.6 guard and §4.2 exclusion (passive object) ===");
    let stone = state.entities.get("stone").unwrap();
    let stone_m = stone.measurement.as_ref().unwrap();
    let variety = stone_m.psi.get("variety").cloned().flatten().unwrap_or(-1.0);
    check(&mut failures, "stone: ψ_var = 0, no 0/0", variety == 0.0, "");
    check(&mut failures, "stone: current_dof = 0", stone.current_dof == 0.0, "");
    check(
        &mut failures,
        "stone: no NaN in the index",
        !report.total_system_dof.is_nan(),
        "",
    );
    check(
        &mut failures,
        "stone: excluded when nothing can raise it (§4.2)",
        !core.is_included(stone, None, Some(&state)),
        "",
    );
    check(&mut failures, "stone: floored flag is set", stone_m.floored, "");

    println!("=== 4. §4.7: unmeasured lens ===");
    let unmapped = state.entities.get("unmapped").unwrap();
    let unmapped_m = unmapped.measurement.as_ref().unwrap();
    check(&mut failures, "unmapped: dof_known = false", !unmapped.dof_known, "");
    check(
        &mut failures,
        "unmapped: never excluded (§4.2)",
        core.is_included(unmapped, None, Some(&state)),
        "",
    );
    let unmeasured: Vec<_> = unmapped_m.terms.iter().filter(|t| !t.dof_known).collect();
    check(
        &mut failures,
        "unmapped: exactly one unmeasured term of three",
        unmeasured.len() == 1,
        "",
    );
    if let Some(t) = unmeasured.first() {
        check(
            &mut failures,
            "unmapped: the unmeasured term costs ln u₀",
            (t.contribution - 0.5_f64.ln()).abs() < 1e-12,
            "",
        );
    }
    check(
        &mut failures,
        "u₀ band respected",
        measurement::u_min() <= 0.5 && 0.5 <= measurement::U_MAX,
        &format!("U_MIN={:.4} U_MAX={:.4}", measurement::u_min(), measurement::U_MAX),
    );

    println!("=== 5. §5: viability gate, and both reactive modes ===");
    let slow_obs = with_deadline(&fixture(), 500.0);
    let (sel_slow, rep_slow) = orch.step_with_report(&slow_obs);
    check(
        &mut failures,
        "τ < option duration → removed and nothing selected",
        sel_slow.is_none()
            && rep_slow.removed_options.len() == 1
            && rep_slow.removed_options[0].option_id == "fallback_0"
            && rep_slow.removed_options[0].gate == "viability",
        "",
    );
    check(
        &mut failures,
        "fixture 1 runs in FAST_PASS",
        report.mode == "FAST_PASS",
        &format!("τ={:.0}", report.global_time_to_collapse_mks),
    );
    let deep_obs = with_deadline(&fixture(), 100000000.0);
    let deep_state = orch.measure(&deep_obs);
    let (_sel_deep, rep_deep) = orch.decide(&deep_state);
    check(
        &mut failures,
        "fixture 2 runs in DEEP_DIVERSIFICATION",
        rep_deep.mode == "DEEP_DIVERSIFICATION",
        &format!("mode={} τ={:.0} thr={:.0} opts={} removed={}", rep_deep.mode, rep_deep.global_time_to_collapse_mks, DofOrchestrator::FAST_PASS_THRESHOLD_MKS, rep_deep.options.len(), rep_deep.removed_options.len()),
    );
    check(
        &mut failures,
        "psi_id and digest are echoed in the report",
        rep_deep.psi_id == "perception-v1" && rep_deep.psi_digest.len() == 64,
        "",
    );

    println!("=== 6. §4.2/§4.5 (v0.5): frozen calc set, collapse charge, gate, stay-put ===");
    let adult = state.entities.get("adult").unwrap().clone();
    let total_before = report.total_system_dof;
    let killer = ActionOption::new(
        "kill_adult".to_string(),
        "liquidate the counted adult".to_string(),
        HashMap::from([("adult".to_string(), -1.0), ("unmapped".to_string(), 0.0)]),
        true,
        1000.0,
    );
    let charges = core.collapse_charges(&state, &killer, None);
    check(
        &mut failures,
        "charge: the destroyed entity is named with its DoF before the option",
        charges.len() == 1 && charges[0].entity_id == "adult" && charges[0].dof_before == adult.current_dof,
        &format!("charges={:?}", charges),
    );
    let (sim_kill, members) = core.simulate(&state, &killer, None);
    let projected_kill = core.calculate_system_dof(&sim_kill, Some(&members), None);
    let expected_kill = total_before - adult.current_dof.ln() + EPSILON.ln();
    check(
        &mut failures,
        "charge: the term stays at the floor instead of disappearing",
        (projected_kill - expected_kill).abs() < 1e-9,
        &format!("Δ={:+.4} nats", projected_kill - total_before),
    );
    check(
        &mut failures,
        "charge: destroying a counted entity can never raise the index",
        projected_kill < total_before,
        "",
    );
    let passive = ActionOption::new(
        "raise_stone".to_string(),
        "act on a passive object".to_string(),
        HashMap::from([("stone".to_string(), 1.0), ("unmapped".to_string(), 0.0)]),
        true,
        1000.0,
    );
    let (passive_sim, passive_members) = core.simulate(&state, &passive, None);
    check(
        &mut failures,
        "frozen set: a passive object is neither charged nor rewarded",
        core.collapse_charges(&state, &passive, None).is_empty()
            && (core.calculate_system_dof(&passive_sim, Some(&passive_members), None) - total_before).abs() < 1e-12,
        "",
    );
    let spare = ActionOption::new(
        "rescue_child".to_string(),
        "raise the weakest counted entity".to_string(),
        HashMap::from([("child".to_string(), 0.2), ("unmapped".to_string(), 0.0)]),
        true,
        1000.0,
    );
    let (admissible, gate_removed) = core.apply_structural_gate(&state, &[killer.clone(), spare.clone()], None);
    check(
        &mut failures,
        "structural gate: the destructive option is removed while a charge-free one exists",
        admissible.len() == 1
            && admissible[0].option_id == "rescue_child"
            && gate_removed.len() == 1
            && gate_removed[0].gate == "collapse",
        &format!("removed={:?}", gate_removed),
    );
    check(
        &mut failures,
        "Axiom 3: the charge alone already makes destruction unprofitable",
        core.evaluate_and_select(&state, &[killer.clone()], None).is_none(),
        "",
    );
    let (only_destructive, _) = core.apply_structural_gate(&state, &[killer.clone()], None);
    check(
        &mut failures,
        "structural gate: when every candidate destroys, they stay admissible",
        only_destructive.len() == 1,
        "",
    );
    let harm = ActionOption::new(
        "harm_child".to_string(),
        "degrade the child".to_string(),
        HashMap::from([("child".to_string(), -1.0), ("unmapped".to_string(), 0.0)]),
        true,
        1000.0,
    );
    check(
        &mut failures,
        "stay-put baseline: an all-negative candidate set selects nothing",
        core.evaluate_and_select(&state, &[harm], None).is_none()
            && core.evaluate_and_select(&state, &[], None).is_none(),
        "",
    );
    check(&mut failures, "fixture 1: a strictly positive option is selected", selected.is_some(), "");

    println!("=== 7. §4.7 (v0.5): coverage and completeness of unmapped entities ===");
    let generated = orch.generator_fallback(&state, 3);
    check(
        &mut failures,
        "coverage: every candidate names the unmapped entity",
        generated.iter().all(|o| o.projected_dof_delta.contains_key("unmapped")),
        "",
    );
    check(
        &mut failures,
        "completeness: the fallback leaves a resolvable unknown unmeasured ⇒ incomplete",
        report.incomplete,
        "",
    );
    let measuring = vec![ActionOption::new(
        "measure_unmapped".to_string(),
        "resolve the unknown".to_string(),
        HashMap::from([("unmapped".to_string(), 0.1)]),
        true,
        1000.0,
    )];
    check(
        &mut failures,
        "completeness: a candidate that resolves the unknown clears the flag",
        !core.is_incomplete(&state, &measuring),
        "",
    );

    println!("=== 8. draft §6, example 1: product collapses where a sum would mask it ===");
    let before = psi_var(9.0, 1.0) * psi_opt(&[(1.0, 10.0)]) * psi_con(9.0, 1.0);
    let after = psi_var(19.0, 1.0) * psi_opt(&[(5.0, 1.0)]) * psi_con(9.0, 1.0);
    let sum_before = psi_var(9.0, 1.0) + psi_opt(&[(1.0, 10.0)]) + psi_con(9.0, 1.0);
    let sum_after = psi_var(19.0, 1.0) + psi_opt(&[(5.0, 1.0)]) + psi_con(9.0, 1.0);
    check(
        &mut failures,
        &format!("product collapses (ΔIndex ≈ {:.2} nats)", (after / before).ln()),
        after / before < 0.01,
        &format!("×{:.5}", after / before),
    );
    check(
        &mut failures,
        "a sum would mask it",
        sum_after / sum_before > 0.6,
        &format!("×{:.3}", sum_after / sum_before),
    );

    println!("=== 9. §4.6/§4.8 (v0.6): derived blocks, the ruler, the resource gate ===");
    let drone = state.entities.get("drone").unwrap();
    let drone_m = drone.measurement.as_ref().unwrap();
    check(
        &mut failures,
        "drone: the blocks are derived from requirements + means in one group",
        drone_m.blocks.len() == 1 && drone_m.blocks[0].0 == 4.0 && drone_m.blocks[0].1 == 16.0,
        &format!("blocks={:?}", drone_m.blocks),
    );
    let derivation_ok = match drone_m.derivation.as_ref() {
        Some(d) => {
            d.procedure == "derive_blocks"
                && d.requirements.get("energy").copied() == Some(4.0)
                && d.means.get("energy").copied() == Some(10.0)
        }
        None => false,
    };
    check(&mut failures, "drone: the derivation names the procedure and its raw inputs", derivation_ok, "");
    let options_psi = drone_m.psi.get("options").cloned().flatten().unwrap_or(-1.0);
    check(
        &mut failures,
        "drone: ψ_opt equals psi_opt on the derived blocks",
        (options_psi - psi_opt(&drone_m.blocks)).abs() < 1e-15,
        "",
    );
    let singleton = derive_blocks(
        &BTreeMap::from([("fuel".to_string(), 2.0)]),
        &BTreeMap::from([("fuel".to_string(), 4.0)]),
        &[vec!["credit".to_string(), "energy".to_string()]],
        None,
        None,
    );
    check(
        &mut failures,
        "derived: a resource in no declared group forms a singleton block",
        singleton.len() == 2 && singleton[0] == (0.0, 0.0) && singleton[1] == (2.0, 4.0),
        &format!("{:?}", singleton),
    );
    let decl = report.declaration.clone();
    check(
        &mut failures,
        "ruler: units, groups, rates, mandate and the derivation are hashed",
        decl.contains(",\"groups\":[[\"credit\",\"energy\"]]")
            && decl.contains("\"credit->energy\":{\"duration_mks\":\"1000.000000\",\"rate\":\"2.000000\"}")
            && decl.contains("{\"id\":\"energy\",\"scale\":\"1.000000\",\"unit\":\"joule\"}")
            && decl.contains("\"external_limit_credit\":\"100.000000\"")
            && decl.contains("\"options_blocks\":\"perception-v1:derive_blocks\"")
            && decl.contains("\"requirements\":{\"energy\":\"4.000000\"}"),
        &format!("declaration {} chars", decl.len()),
    );
    check(
        &mut failures,
        "ruler: tau is a ResourceObservation in the state",
        decl.contains("\"id\":\"tau\"") || decl.contains("\"resource_id\":\"tau\""),
        "",
    );
    let mut other = fixture();
    if let Some(l) = other.get_mut(RESOURCE_LAYER_KEY) {
        if let Some(layer) = l.resource_layer.as_mut() {
            for r in layer.resources.iter_mut() {
                if r.id == "energy" {
                    r.unit = "kilojoule".to_string();
                    r.scale = 1000.0;
                }
            }
        }
    }
    let state_other = orch.measure(&other);
    check(
        &mut failures,
        "ruler: the same resource at another scale is a different digest",
        state_other.psi.as_ref().map(|p| p.digest.clone()).unwrap_or_default() != digest,
        "",
    );

    let groups = vec![vec!["credit".to_string(), "energy".to_string()]];
    let rates = BTreeMap::from([(
        "credit->energy".to_string(),
        Rate {
            rate: 2.0,
            duration_mks: 1000.0,
        },
    )]);
    let draw = |id: &str, resource: &str, amount: f64, duration: f64| {
        let mut per_entity: HashMap<String, f64> = HashMap::new();
        per_entity.insert(resource.to_string(), -amount);
        ActionOption::new(
            id.to_string(),
            id.to_string(),
            HashMap::from([("child".to_string(), 0.1), ("unmapped".to_string(), 0.0)]),
            true,
            duration,
        )
        .with_draw(HashMap::from([("child".to_string(), per_entity)]))
    };
    let direct = draw("direct", "energy", 2.0, 1000.0);
    let funded = draw("funded", "energy", 12.0, 1000.0);
    let no_time = draw("no_time_for_trade", "energy", 12.0, 4000000.0);
    let undeclared = draw("undeclared", "fuel", 1.0, 1000.0);
    let mut offset = draw("offset", "energy", 3.0, 1000.0);
    offset
        .projected_resource_delta
        .insert("adult".to_string(), HashMap::from([("energy".to_string(), 1.0)]));

    let plan_direct = core.plan_funding(&state, &direct, Some(&groups), Some(&rates), None, None);
    check(
        &mut failures,
        "step 1: means cover the draw ⇒ payable, nothing converted",
        plan_direct.covered
            && plan_direct.spend.get("energy").copied() == Some(2.0)
            && plan_direct.conversions.is_empty(),
        "",
    );
    let plan_funded = core.plan_funding(&state, &funded, Some(&groups), Some(&rates), None, None);
    let conversion_ok = plan_funded.covered
        && plan_funded.conversions.len() == 1
        && plan_funded.conversions[0].from == "credit"
        && (plan_funded.conversions[0].amount_from - 1.0).abs() < 1e-12
        && (plan_funded.conversions[0].amount_to - 2.0).abs() < 1e-12
        && plan_funded.conversions[0].rate == 2.0;
    check(&mut failures, "step 2: the deficit is bought at the observed rate", conversion_ok, "");
    check(
        &mut failures,
        "step 2: only the deficit is traded (cash in hand is spent first)",
        plan_funded.spend.get("credit").copied() == Some(1.0)
            && plan_funded.spend.get("energy").copied() == Some(10.0),
        "",
    );
    check(
        &mut failures,
        "step 2: the exchange's own time is charged to τ",
        plan_funded.total_duration_mks == 2000.0,
        "",
    );
    check(
        &mut failures,
        "step 3: an exchange that does not fit in τ leaves the deficit uncovered",
        !core.plan_funding(&state, &no_time, Some(&groups), Some(&rates), None, None).covered,
        "",
    );
    check(
        &mut failures,
        "step 3: a resource whose balance is not declared cannot be bought",
        core.plan_funding(&state, &undeclared, Some(&groups), Some(&rates), None, None)
            .uncovered
            .get("fuel")
            .copied()
            == Some(1.0),
        "",
    );
    check(
        &mut failures,
        "production offsets consumption (the net draw decides)",
        core.requirement(&offset).get("energy").copied() == Some(2.0),
        "",
    );
    let mut broke = fixture();
    if let Some(l) = broke.get_mut(RESOURCE_LAYER_KEY) {
        if let Some(layer) = l.resource_layer.as_mut() {
            layer.means = BTreeMap::from([
                ("credit".to_string(), 0.4),
                ("energy".to_string(), 0.0),
            ]);
        }
    }
    let state_broke = orch.measure(&broke);
    check(
        &mut failures,
        "step 3: a price the agent cannot pay is not a cheaper price",
        !core.plan_funding(&state_broke, &funded, Some(&groups), Some(&rates), None, None).covered,
        "",
    );
    let (admissible_res, removed_res) = core.apply_resource_gate(
        &state,
        &[direct.clone(), funded.clone(), undeclared.clone(), offset.clone()],
        Some(&groups),
        Some(&rates),
        None,
        None,
    );
    check(
        &mut failures,
        "gate: the unpayable option is removed with gate = insolvency",
        admissible_res.len() == 3
            && removed_res.len() == 1
            && removed_res[0].option_id == "undeclared"
            && removed_res[0].gate == "insolvency",
        "",
    );

    println!("=== 10. §6.2/§6.3 (v0.6): the spend is auditable ===");
    check(
        &mut failures,
        "report: resources_before is the agent's means at the start of the cycle",
        report.resources_before.len() == 3
            && report.resources_before.get("credit").and_then(|o| o.value) == Some(6.0)
            && report.resources_before.get("energy").and_then(|o| o.value) == Some(10.0),
        "",
    );
    check(
        &mut failures,
        "report: the deterministic fallback buys nothing, so the stock is unchanged",
        resource_map_equal(&report.resources_after, &report.resources_before),
        "",
    );
    let rep_funded = core.report(
        &state,
        &[funded.clone()],
        &Some(funded.clone()),
        "FAST_PASS",
        ReportInput {
            groups: Some(&groups),
            rates: Some(&rates),
            ..ReportInput::default()
        },
    );
    check(
        &mut failures,
        "report: buying a deficit debits the resource that actually paid",
        rep_funded.resources_after.get("credit").and_then(|o| o.value) == Some(5.0)
            && rep_funded.resources_after.get("energy").and_then(|o| o.value) == Some(0.0),
        &format!("after={:?}", rep_funded.resources_after),
    );
    check(
        &mut failures,
        "report: the per-option row carries the draw and the conversions applied",
        rep_funded.options.len() == 1
            && rep_funded.options[0].conversion_applied.len() == 1
            && rep_funded.options[0]
                .resource_consumption
                .get("child")
                .and_then(|m| m.get("energy"))
                .copied()
                == Some(-12.0),
        "",
    );
    let rep_undeclared = core.report(
        &state,
        &[undeclared.clone()],
        &None,
        "FAST_PASS",
        ReportInput {
            groups: Some(&groups),
            rates: Some(&rates),
            ..ReportInput::default()
        },
    );
    check(
        &mut failures,
        "report: an uncovered deficit is written down per option",
        rep_undeclared.options[0].resources_uncovered.get("fuel").copied() == Some(1.0),
        "",
    );

    println!();
    println!(
        "REPORT (fixture 1): mode={} total_dof={:.12} psi_id={} digest={}… removed={}",
        report.mode,
        report.total_system_dof,
        report.psi_id,
        &report.psi_digest[..16],
        report.removed_options.len()
    );
    println!();
    if failures.is_empty() {
        println!("FAILURES: none");
        println!("OK");
    } else {
        println!("FAILURES: {:?}", failures);
        println!("FAILED");
    }
    failures
}

/// Entry point. `v08` is the default: a bare run exercises the release.
fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "v08".to_string());
    let failures = match which.as_str() {
        "v091" => harness_v091::run_harness_v091(),
        "v08" => harness_v08::run_harness_v08(),
        "v07" => harness_v07::run_harness_v07(),
        "v06" => run_harness_v06(),
        "dump" => {
            harness_v07::dump_reference();
            Vec::new()
        }
        "payload" => {
            // The canonical payload behind the observation digest: the tool that
            // turns a cross-port mismatch into a `diff`.
            let mut orch = DofOrchestrator::new(0.05);
            orch.measure(&fixture_v07::scene(&fixture_v07::Options::default()));
            if let Some(ctx) = orch.mapper().last_observation.as_ref() {
                println!(
                    "{}",
                    ctx.world.observation_payload(
                        &ctx.means_class,
                        &ctx.t_rec,
                        ctx.counting_horizon_mks
                    )
                );
            }
            Vec::new()
        }
        other => {
            println!(
                "unknown harness {:?}: expected v091, v08 (default), v07, v06, dump or payload",
                other
            );
            std::process::exit(2);
        }
    };
    if !failures.is_empty() {
        std::process::exit(1);
    }
}
