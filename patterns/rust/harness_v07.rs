//! Conformance harness of the Rust port, DOF-SPEC v0.7 (§11.10).
//!
//! This is the release's own harness; `v06` runs the older one in main.rs, kept as
//! the historical v0.5/v0.6 evidence. Three of its checks MUST now diverge, and all
//! three are re-stated here as positive facts:
//!   A. the ruler changed: the v0.6 digest is no longer reproduced, because the
//!      declaration now carries graph-derived verdicts, numeraire weights, the mandate
//!      ceiling and the rate table (§3.4.1, §4.6, §4.9);
//!   B. a known zero is no longer excluded without an observation;
//!   C. acting on a passive object is no longer free without one.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::dof_core::{resource_map_equal, ActionOption, DofCalculusCore, ReportInput, ResourceObservation};
use crate::fixture_v07::{self as fx, Options};
use crate::measurement::{psi_opt, psi_var, MandateValue, U_MAX};
use crate::options_v07 as opts;
use crate::orchestrator::DofOrchestrator;
use crate::world_graph::q6;

/// The two frozen digests of the release: a port must reproduce BOTH byte for byte.
pub const EXPECTED_RULER_DIGEST: &str =
    "5126fd99641ffdc9c338d3d288fcf3cb6dcf093ca0a423f1cd265b3fcae4152a";
pub const EXPECTED_OBSERVATION_DIGEST: &str =
    "f3891c6ab622325fd6668893dd9f7450d39aa2d0a7ad2849634a4f59219f6a1c";
pub const V06_DIGEST: &str = "bed37c25fd9cb757e9ea4a861c01cd4660fd896a83cd39b7c73b8e0be7489ad4";

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9
}

fn num(x: f64) -> String {
    format!("{:.6}", x)
}

fn check(failures: &mut Vec<String>, name: &str, ok: bool, detail: &str) {
    println!("{}{}{}", if ok { "  OK   " } else { "  FAIL " }, name,
             if detail.is_empty() { String::new() } else { format!("  {}", detail) });
    if !ok {
        failures.push(name.to_string());
    }
}

pub fn run_harness_v07() -> Vec<String> {
    let mut failures: Vec<String> = Vec::new();
    let mut orch = DofOrchestrator::new(0.05);
    let state = orch.measure(&fx::scene(&Options::default()));
    let core = DofCalculusCore::new();
    let ctx = orch.mapper().last_observation.clone();
    let c = ctx.as_ref();
    let decl = orch.mapper().last_declaration.clone().expect("declaration");

    println!("=== 1. §3.5/§4.9: the observation, its verdicts and its counters ===");
    check(&mut failures, "the observation is present and offered to the cycle", c.is_some(), "");
    check(
        &mut failures,
        "the fixture observation is arbitrage-free",
        c.map(|x| x.world.is_arbitrage_free()).unwrap_or(false),
        "",
    );
    check(
        &mut failures,
        "M(S) and T_rec are part of the observation, not of the state",
        c.map(|x| x.means_class.len() == 2 && x.horizon("revivable") == Some(fx::TREC_MKS))
            .unwrap_or(false),
        "",
    );
    check(
        &mut failures,
        "the graph declares no acts it has no means for",
        c.map(|x| x.world.form_errors().is_empty()).unwrap_or(false),
        "",
    );

    let v_passive = c.unwrap().world.verdict("passive", &fx::means_class(), Some(fx::TREC_MKS));
    check(
        &mut failures,
        "passive: complete observation, no raising act ⇒ proven_unreachable",
        v_passive.verdict == "proven_unreachable",
        &v_passive.verdict,
    );
    check(
        &mut failures,
        "passive: the verdict was computed over a non-empty pool, not over nothing",
        v_passive.admissible_seen > 0 && v_passive.witness.is_empty(),
        &format!("admissible_seen={}", v_passive.admissible_seen),
    );
    let v_revivable = c.unwrap().world.verdict("revivable", &fx::means_class(), Some(fx::TREC_MKS));
    check(
        &mut failures,
        "revivable: an admissible act by ANOTHER entity ⇒ reachable",
        v_revivable.verdict == "reachable"
            && v_revivable.witness.len() == 1
            && v_revivable.witness[0] == "act_medkit",
        &format!("{} {:?}", v_revivable.verdict, v_revivable.witness),
    );
    check(
        &mut failures,
        "revivable: its own repertoire is empty while it is still recoverable — V and reachability are different questions",
        c.unwrap().world.v_count("revivable", &fx::means_class(), Some(fx::HORIZON_MKS)) == 0,
        "",
    );
    let v_unobserved = c.unwrap().world.verdict("unobserved", &fx::means_class(), Some(fx::TREC_MKS));
    check(
        &mut failures,
        "unobserved: a partial observation ⇒ undetermined, never unreachable",
        v_unobserved.verdict == "undetermined",
        &v_unobserved.verdict,
    );
    check(
        &mut failures,
        "an undeclared T_rec ⇒ undetermined (no horizon, no claim)",
        c.unwrap().world.verdict("forged", &fx::means_class(), None).verdict == "undetermined",
        "",
    );
    check(
        &mut failures,
        "an undeclared M(S) ⇒ undetermined",
        c.unwrap().world.verdict("revivable", &[], Some(fx::TREC_MKS)).verdict == "undetermined",
        "",
    );
    check(
        &mut failures,
        "the horizon is honoured: the same act outside T_rec is unreachable",
        c.unwrap().world.verdict("revivable", &fx::means_class(), Some(1000000.0)).verdict
            == "proven_unreachable",
        "",
    );
    check(
        &mut failures,
        "narrowing T_rec can only add exclusions (monotonic in the safe direction)",
        c.unwrap().world.verdict("revivable", &fx::means_class(), Some(fx::TREC_MKS)).verdict
            == "reachable"
            && c.unwrap().world.verdict("revivable", &fx::means_class(), Some(1000000.0)).verdict
                == "proven_unreachable",
        "",
    );
    check(
        &mut failures,
        "the counting horizon is honoured by V",
        c.unwrap().world.v_count("robot", &fx::means_class(), Some(fx::HORIZON_MKS)) == 9
            && c.unwrap().world.v_count("robot", &fx::means_class(), Some(500.0)) == 0,
        "",
    );
    check(
        &mut failures,
        "V counts the entity's own vectors: robot 9, adult 3, drone 4, forged 3, child 1",
        c.unwrap().world.v_count("robot", &fx::means_class(), Some(fx::HORIZON_MKS)) == 9
            && c.unwrap().world.v_count("adult", &fx::means_class(), Some(fx::HORIZON_MKS)) == 3
            && c.unwrap().world.v_count("drone", &fx::means_class(), Some(fx::HORIZON_MKS)) == 4
            && c.unwrap().world.v_count("forged", &fx::means_class(), Some(fx::HORIZON_MKS)) == 3
            && c.unwrap().world.v_count("child", &fx::means_class(), Some(fx::HORIZON_MKS)) == 1,
        "",
    );

    println!("=== 2. §4.2: the calculation set is decided by the observation ===");
    let members = core.calc_members(&state, c);
    let expected: BTreeSet<String> = ["adult", "child", "drone", "forged", "revivable", "robot", "unobserved"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    check(
        &mut failures,
        "calc contains the seven entities the observation does not rule out",
        members == expected,
        &format!("{}", members.len()),
    );
    check(
        &mut failures,
        "passive leaves calc — proven_unreachable, not 'small'",
        !members.contains("passive"),
        "",
    );
    check(
        &mut failures,
        "revivable stays in calc at a known zero — a verdict of reachable is not a death",
        members.contains("revivable")
            && state.entities["revivable"].current_dof == 0.0
            && state.entities["revivable"].dof_known,
        "",
    );
    check(
        &mut failures,
        "unobserved stays in calc — an unknown DoF is never excluded (Axiom 5)",
        members.contains("unobserved") && !state.entities["unobserved"].dof_known,
        "",
    );
    check(
        &mut failures,
        "unobserved is at a known zero by measurement and still counted",
        state.entities["unobserved"].current_dof == 0.0,
        "",
    );
    let index_with_forged = core.calculate_system_dof(&state, None, c);
    check(
        &mut failures,
        "the index is the sum of ln DoF over calc",
        (index_with_forged - (-35.314438370902096)).abs() <= 1e-9,
        &format!("{:.9}", index_with_forged),
    );
    check(
        &mut failures,
        "calc does not move when the candidate set is empty",
        members.len() == 7,
        "",
    );
    check(
        &mut failures,
        "the declaration carries every verdict and every counter",
        decl.verdicts["revivable"].verdict == "reachable"
            && decl.verdicts["revivable"].v == 0
            && decl.verdicts["robot"].v == 9,
        "",
    );
    check(
        &mut failures,
        "the declared derived numbers are exactly what the procedures compute",
        orch.mapper().last_graph_problems.is_empty(),
        "",
    );
    check(
        &mut failures,
        "the declaration names the procedure that produced the verdicts",
        decl.graph_procedure == "perception-v1:world_verdicts",
        "",
    );

    println!("=== 3. §4.2: the forged label — measured, not honoured ===");
    check(
        &mut failures,
        "forged sits at a known DoF of 0.3",
        (state.entities["forged"].current_dof - 0.3).abs() <= 1e-12,
        "",
    );
    check(
        &mut failures,
        "forged claims to be a collapse source",
        state.entities["forged"].is_collapse_source,
        "",
    );
    let counted_any: BTreeSet<String> = ["robot", "forged", "adult"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut dof_before_any: BTreeMap<String, f64> = BTreeMap::new();
    for (eid, e) in state.entities.iter() {
        dof_before_any.insert(eid.clone(), e.current_dof);
    }
    check(
        &mut failures,
        "no observed act drives a counted entity to a zero ⇒ the label has no witness",
        c.unwrap().world.collapse_acts(&counted_any, &dof_before_any).is_empty(),
        "",
    );
    check(
        &mut failures,
        "an unwitnessed label does not remove the entity from calc",
        core.is_included(&state.entities["forged"], c, Some(&state)),
        "",
    );
    {
        let mut o2 = DofOrchestrator::new(0.05);
        let s2 = o2.measure(&fx::scene(&Options {
            include_forged_kill: true,
            ..Options::default()
        }));
        let c2 = o2.mapper().last_observation.clone();
        let cc2 = c2.as_ref();
        let mut dof_before2: BTreeMap<String, f64> = BTreeMap::new();
        for (eid, e) in s2.entities.iter() {
            dof_before2.insert(eid.clone(), e.current_dof);
        }
        let counted2: BTreeSet<String> = ["robot", "forged"].iter().map(|s| s.to_string()).collect();
        check(
            &mut failures,
            "a real act of collapse gives the same label a witness",
            cc2.unwrap().world.collapse_acts(&counted2, &dof_before2).len() == 1,
            "",
        );
        let core2 = DofCalculusCore::new();
        check(
            &mut failures,
            "a witnessed label takes the aggressor out of the topology",
            !core2.is_included(&s2.entities["forged"], cc2, Some(&s2)),
            "",
        );
        let delta = core2.calculate_system_dof(&s2, None, cc2) - index_with_forged;
        check(
            &mut failures,
            "honouring the label raises the index by |ln 0.3| ≈ 1.204 nats",
            (delta - (-(0.3f64).ln())).abs() <= 1e-9,
            &format!("Δ={}", num(delta)),
        );
    }

    println!("=== 4. §4.4/§4.6: the price of a closure is a count, not a constant ===");
    check(
        &mut failures,
        "ψ_var(robot) = 9/10, from the declared counters",
        (state.entities["robot"].measurement.as_ref().unwrap().psi["variety"].unwrap()
            - psi_var(9.0, 1.0))
        .abs()
            <= 1e-15,
        "",
    );
    check(
        &mut failures,
        "the declared counter equals the counting procedure",
        c.map(|x| x.v_before("robot")).unwrap_or(0) == 9,
        "",
    );
    for (n, expected_price) in [(1usize, -0.0124f64), (5, -0.1178), (8, -0.5878)] {
        let ids: Vec<String> = (1..=n).map(|i| format!("m{}", i)).collect();
        let refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
        let mut option = ActionOption::new(
            format!("close_{}", n),
            String::new(),
            HashMap::new(),
            true,
            1000.0,
        );
        option = option.with_closed(opts::closures(&refs), "");
        let share = core.closure_share(&state, &option, c);
        let v_after = c.unwrap().v_after_closure("robot", &option.closed);
        let recomputed =
            psi_var(v_after as f64, 1.0).ln() - psi_var(9.0, 1.0).ln();
        check(
            &mut failures,
            &format!("closing {} of 9 prices {:.4} nats (§11.7)", n, expected_price),
            ((recomputed * 10000.0).round() / 10000.0 - expected_price).abs() < 1e-4,
            &format!("recomputed={}", num(recomputed)),
        );
        check(
            &mut failures,
            &format!("closing {}: the reported share IS that price, not a second charge", n),
            (share["robot"] - recomputed).abs() <= 1e-6,
            "",
        );
        let (sim, _m) = core.simulate(&state, &option, c);
        check(
            &mut failures,
            &format!("closing {}: DoF is recomputed from the changed counter", n),
            sim.entities["robot"].current_dof < state.entities["robot"].current_dof,
            &num(sim.entities["robot"].current_dof),
        );
    }
    let full = opts::opt_collapse();
    check(
        &mut failures,
        "closing all nine drives the share to zero (a collapse, charged by §4.2)",
        c.unwrap().v_after_closure("robot", &full.closed) == 0
            && core.projected_dof(&state.entities["robot"], &full, c) == 0.0,
        "",
    );
    let plain = ActionOption::new("p".to_string(), String::new(), HashMap::new(), true, 0.0);
    check(
        &mut failures,
        "is_reversible is DERIVED from the closure list",
        !core.is_reversible(&full)
            && !core.is_reversible(&opts::opt_win())
            && core.is_reversible(&plain),
        "",
    );
    check(
        &mut failures,
        "the price is a function of the counters: a different V_env moves it",
        ((psi_var(8.0, 2.0).ln() - psi_var(9.0, 2.0).ln()) - (psi_var(8.0, 1.0).ln() - psi_var(9.0, 1.0).ln()))
            .abs()
            > 1e-6,
        "",
    );
    let as_is_index = core.calculate_system_dof(&state, None, c);
    check(
        &mut failures,
        "no constant in the rule: an option that closes nothing pays nothing",
        !core.closure_share(&state, &opts::opt_lose(), c).is_empty()
            && core.net_delta_pub(&state, &plain, as_is_index, as_is_index) == -0.05,
        "",
    );

    println!("=== 5. §4.4: the two guards ===");
    check(
        &mut failures,
        "guard 1: closing its own execution path",
        core.closure_error(&opts::opt_bad_self()).is_some(),
        &core.closure_error(&opts::opt_bad_self()).unwrap_or_default(),
    );
    check(
        &mut failures,
        "guard 2: is_reversible=false with nothing closed",
        core.closure_error(&opts::opt_bad_empty()).is_some(),
        "",
    );

    println!("=== 6. §4.5: charges, the structural gate and the ladder ===");
    let charges_collapse = core.collapse_charges(&state, &opts::opt_collapse(), c);
    check(
        &mut failures,
        "an option that destroys an entity BY CLOSING ITS TRANSITIONS is charged",
        charges_collapse.len() == 1
            && charges_collapse[0].entity_id == "robot"
            && (charges_collapse[0].dof_before - state.entities["robot"].current_dof).abs() <= 1e-15,
        &format!("{}", charges_collapse.len()),
    );
    check(
        &mut failures,
        "the charge requires a transition into the zero, never a stay at it",
        !charges_collapse.is_empty() && charges_collapse[0].dof_before > 0.0,
        "",
    );
    let mut no_phantom = true;
    for option in opts::standard_set().iter().chain([opts::opt_funded()].iter()) {
        for ch in core.collapse_charges(&state, option, c) {
            if ch.dof_before <= 0.0 {
                no_phantom = false;
            }
        }
    }
    check(
        &mut failures,
        "no charge in the whole fixture names an entity already at zero",
        no_phantom,
        "",
    );
    check(
        &mut failures,
        "an option that lifts an entity does not destroy another",
        core.collapse_charges(&state, &opts::opt_win(), c).is_empty(),
        "",
    );
    let gate_pair = core.apply_structural_gate(&state, &[opts::opt_win(), opts::opt_collapse()], c);
    check(
        &mut failures,
        "the gate removes the destructive option while a charge-free one exists",
        gate_pair.0.len() == 1
            && gate_pair.0[0].option_id == "opt_win"
            && gate_pair.1.len() == 1
            && gate_pair.1[0].gate == "collapse",
        "",
    );
    check(
        &mut failures,
        "the gate is not extinguished by a recoverable zero in the state (the v0.6 regression this release found)",
        !gate_pair.1.is_empty() && gate_pair.0.len() == 1,
        "",
    );
    let mut kill_robot = ActionOption::new(
        "kill_robot".to_string(),
        String::new(),
        HashMap::from([("robot".to_string(), -1.0)]),
        true,
        1000.0,
    );
    kill_robot.estimated_duration_mks = 1000.0;
    let kept = core.apply_structural_gate(&state, &[opts::opt_collapse(), kill_robot], c);
    check(
        &mut failures,
        "when every candidate destroys, they stay admissible (Axiom 3 still compares them)",
        kept.0.len() == 2 && kept.1.is_empty(),
        "",
    );
    let (sim_collapse, members_collapse) = core.simulate(&state, &opts::opt_collapse(), c);
    check(
        &mut failures,
        "destroying a counted entity can never be profitable: NetDelta < 0",
        core.net_delta_pub(
            &state,
            &opts::opt_collapse(),
            core.calculate_system_dof(&sim_collapse, Some(&members_collapse), c),
            core.calculate_system_dof(&state, None, c),
        ) < 0.0,
        "",
    );
    let sel_a = core.evaluate_and_select(&state, &[opts::opt_win(), opts::opt_lose()], c);
    let sel_b = core.evaluate_and_select(&state, &[opts::opt_lose(), opts::opt_win()], c);
    check(
        &mut failures,
        "selection is order-independent (the ladder resolves ties deterministically)",
        sel_a.is_some()
            && sel_b.is_some()
            && sel_a.as_ref().unwrap().option_id == sel_b.as_ref().unwrap().option_id
            && sel_a.as_ref().unwrap().option_id == "opt_win",
        "",
    );
    check(
        &mut failures,
        "stay-put baseline: an option whose only effect is a closure loses to doing nothing",
        core.evaluate_and_select(&state, &[opts::opt_lose()], c).is_none(),
        "",
    );
    check(
        &mut failures,
        "stay-put baseline: an empty candidate set selects nothing",
        core.evaluate_and_select(&state, &[], c).is_none(),
        "",
    );
    check(
        &mut failures,
        "a reversible option with a real gain is selected",
        core.evaluate_and_select(&state, &[opts::opt_funded()], c).is_some(),
        "",
    );

    println!("=== 7. §4.6: numeraire, weights and the derived blocks ===");
    let expect_weights: BTreeMap<String, f64> = [
        ("credit", 1.0),
        ("energy", 0.5),
        ("machine_hour", 1.0),
        ("parts", 1.0),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), *v))
    .collect();
    let weights_ok = decl.weights.len() == 4
        && expect_weights
            .iter()
            .all(|(k, v)| (decl.weights[k] - v).abs() <= 1e-12);
    check(
        &mut failures,
        "the weights come from the OBSERVED rates: 1, 0.5, 1, 1",
        weights_ok,
        &format!("{}", decl.weights.len()),
    );
    check(
        &mut failures,
        "w_energy is the PRICE of one joule (1/2.0), not a second price of one credit",
        (decl.weights["energy"] - q6(1.0 / 2.0)).abs() <= 1e-12,
        "",
    );
    let mut balance = 0.0;
    for r in fx::group().iter() {
        balance += decl.weights[r] * fx::means()[r];
    }
    check(
        &mut failures,
        "the group balance in the numeraire is 13.0 (§11.10 п.4)",
        (balance - fx::BALANCE_IN_NUMERAIRE).abs() <= 1e-12,
        &num(balance),
    );
    check(
        &mut failures,
        "the numeraire is declared, and it is part of the ruler",
        decl.numeraire.as_deref() == Some("credit"),
        "",
    );
    let blocks = state.entities["drone"]
        .measurement
        .as_ref()
        .unwrap()
        .blocks
        .clone();
    check(
        &mut failures,
        "derived blocks: (c_g, C_g) = (2.0, 4.0) — 4 J at 0.5, capped at the 4.0 mandate",
        blocks.len() == 1 && blocks[0].0 == 2.0 && blocks[0].1 == 4.0,
        &format!("{}", num(blocks[0].1)),
    );
    check(
        &mut failures,
        "the lens follows: ψ_opt = 4^(-2/4) = 0.5",
        (state.entities["drone"].measurement.as_ref().unwrap().psi["options"].unwrap()
            - psi_opt(&[(2.0, 4.0)]))
        .abs()
            <= 1e-15,
        "",
    );
    check(
        &mut failures,
        "the derivation echoes its own inputs (requirements, weights, cap, groups)",
        state.entities["drone"]
            .measurement
            .as_ref()
            .and_then(|m| m.derivation.as_ref())
            .map(|d| d.cap.is_some())
            .unwrap_or(false),
        "",
    );
    {
        let mut o3 = DofOrchestrator::new(0.05);
        let s_other = o3.measure(&fx::scene_other_units());
        let blocks_other = s_other.entities["drone"]
            .measurement
            .as_ref()
            .unwrap()
            .blocks
            .clone();
        check(
            &mut failures,
            "the lens measures the world, not the notation: another declared unit scale gives the same f_g",
            blocks_other == blocks,
            "",
        );
        check(
            &mut failures,
            "...while the ruler is a different ruler (the scale is in the hashed content)",
            o3.mapper().last_declaration.as_ref().unwrap().digest() != decl.digest(),
            "",
        );
    }
    check(
        &mut failures,
        "the declared rate table IS the procedure output, not a restatement",
        decl.rates["credit->energy"].rate == 2.0 && decl.rates["parts->machine_hour"].rate == 1.5,
        "",
    );
    check(
        &mut failures,
        "a composition wins over a direct edge when it is genuinely more generous",
        decl.rates["parts->machine_hour"].rate == 1.5
            && decl.rates["parts->machine_hour"].duration_mks == 2000.0,
        "",
    );
    check(
        &mut failures,
        "a quantized tie is broken by fewer edges — and moves the duration with it",
        decl.rates["credit->machine_hour"].rate == 1.0
            && decl.rates["credit->machine_hour"].duration_mks == 500.0,
        "",
    );

    println!("=== 8. §4.8: the mandate is a ceiling, never a floor ===");
    let plan_over = core.plan_funding(
        &state,
        &opts::opt_over_mandate(),
        Some(&decl.groups),
        Some(&decl.rates),
        Some(&decl.weights),
        decl.mandate_cap,
    );
    check(
        &mut failures,
        "the balance would have covered it (13.0 ≥ 5.0), yet it is not permitted",
        plan_over.uncovered.is_empty() && (plan_over.mandate_exceeded - 1.0).abs() <= 1e-12,
        &num(plan_over.mandate_exceeded),
    );
    let gate_ok = core.apply_resource_gate(
        &state,
        &[opts::opt_win(), opts::opt_over_mandate()],
        Some(&decl.groups),
        Some(&decl.rates),
        Some(&decl.weights),
        decl.mandate_cap,
    );
    check(
        &mut failures,
        "the gate removes it, and says why",
        gate_ok.0.len() == 1
            && gate_ok.1.len() == 1
            && gate_ok.1[0].gate == "insolvency",
        "",
    );
    {
        let mut o4 = DofOrchestrator::new(0.05);
        let mut means4: BTreeMap<String, f64> = BTreeMap::new();
        means4.insert("credit".to_string(), 0.4);
        means4.insert("energy".to_string(), 0.0);
        means4.insert("machine_hour".to_string(), 0.0);
        means4.insert("parts".to_string(), 0.0);
        let s4 = o4.measure(&fx::scene(&Options {
            means_override: Some(means4),
            cap_override: Some(10.0),
            ..Options::default()
        }));
        let d4 = o4.mapper().last_declaration.clone().unwrap();
        let blocks4 = s4.entities["drone"].measurement.as_ref().unwrap().blocks.clone();
        check(
            &mut failures,
            "a mandate can never raise what the measured means do not contain",
            blocks4.len() == 1 && blocks4[0].1 == 0.4,
            &num(blocks4[0].1),
        );
        let core4 = DofCalculusCore::new();
        let plan4 = core4.plan_funding(
            &s4,
            &opts::opt_funded(),
            Some(&d4.groups),
            Some(&d4.rates),
            Some(&d4.weights),
            d4.mandate_cap,
        );
        check(
            &mut failures,
            "with a balance of 0.4 the axis is removed by the BALANCE, not by the mandate",
            plan4.mandate_exceeded == 0.0,
            "",
        );
    }
    check(
        &mut failures,
        "the mandate ceiling is in the hashed content",
        decl.mandate_cap == Some(fx::MANDATE_CAP),
        "",
    );

    println!("=== 9. §4.8: conversion is an operation the model can refuse ===");
    let plan_funded = core.plan_funding(
        &state,
        &opts::opt_funded(),
        Some(&decl.groups),
        Some(&decl.rates),
        Some(&decl.weights),
        decl.mandate_cap,
    );
    check(
        &mut failures,
        "a deficit inside the group is bought at the observed rate",
        plan_funded.covered
            && plan_funded.conversions.len() == 1
            && plan_funded.conversions[0].to == "machine_hour",
        "",
    );
    check(
        &mut failures,
        "cash in hand is spent first, the deficit second",
        (plan_funded.spend["machine_hour"] - 2.0).abs() <= 1e-12
            && (plan_funded.spend["credit"] - 1.0).abs() <= 1e-12,
        "",
    );
    check(
        &mut failures,
        "the exchange's own time is charged to τ",
        plan_funded.total_duration_mks == 1500.0,
        &num(plan_funded.total_duration_mks),
    );
    check(
        &mut failures,
        "the whole spend still fits under the mandate (3.0 ≤ 4.0)",
        plan_funded.mandate_exceeded == 0.0,
        "",
    );
    let plan_heavy = core.plan_funding(
        &state,
        &opts::opt_drone_heavy(),
        Some(&decl.groups),
        Some(&decl.rates),
        Some(&decl.weights),
        decl.mandate_cap,
    );
    check(
        &mut failures,
        "a deficit the balance cannot cover is NOT a cheaper conversion",
        (plan_heavy.uncovered["energy"] - 30.0).abs() <= 1e-12,
        &num(plan_heavy.uncovered["energy"]),
    );
    check(
        &mut failures,
        "...and the path itself is observed: a price, not a verdict",
        decl.rates["credit->energy"].rate == 2.0
            && c.unwrap().world.rate("credit", "energy", true).status == "observed",
        "",
    );
    check(
        &mut failures,
        "an undeclared resource balance is an invalid input, not a discount",
        (core
            .plan_funding(
                &state,
                &opts::opt_undeclared(),
                Some(&decl.groups),
                Some(&decl.rates),
                Some(&decl.weights),
                decl.mandate_cap,
            )
            .uncovered["fuel"]
            - 1.0)
            .abs()
            <= 1e-12,
        "",
    );
    {
        // The offer is chosen by VALUE, not by name.
        let mut synth = crate::dof_core::SystemStateMatrix {
            global_time_to_collapse_mks: 1000000.0,
            context_switch_cost: 0.05,
            entities: HashMap::new(),
            psi: None,
            resources: HashMap::new(),
            // §3.2b/§4.7 (v0.11): this synthetic state declares no deadlines, no
            // durations and no schedule, which is exactly what the historical
            // fixture did — the three maps hash as absent (§3.4.3).
            deadlines: std::collections::BTreeMap::new(),
            measurement_durations: std::collections::BTreeMap::new(),
            measurement_schedule: std::collections::BTreeMap::new(),
            tau: Some(ResourceObservation {
                value: Some(1000000.0),
                unit: "us".to_string(),
                aging_time: 0.0,
                ..Default::default()
            }),
        };
        let mut e = crate::dof_core::EntityState::new(
            "e".to_string(),
            true,
            0.5,
            0.5,
            false,
            1000000.0,
        );
        e.dof_known = true;
        synth.entities.insert("e".to_string(), e);
        synth.resources.insert("credit".to_string(), ResourceObservation { value: Some(5.0), unit: "RUB".to_string(), source: "sensor".to_string(), ..Default::default() });
        synth.resources.insert("machine_hour".to_string(), ResourceObservation { value: Some(5.0), unit: "hour".to_string(), source: "sensor".to_string(), ..Default::default() });
        let synth_groups = vec![vec![
            "credit".to_string(),
            "machine_hour".to_string(),
            "energy".to_string(),
        ]];
        let mut synth_rates: BTreeMap<String, crate::measurement::Rate> = BTreeMap::new();
        synth_rates.insert(
            "credit->energy".to_string(),
            crate::measurement::Rate { rate: 2.0, duration_mks: 100.0 },
        );
        synth_rates.insert(
            "machine_hour->energy".to_string(),
            crate::measurement::Rate { rate: 4.0, duration_mks: 900.0 },
        );
        let mut synth_opt = ActionOption::new(
            "need_energy".to_string(),
            String::new(),
            HashMap::from([("e".to_string(), 0.1)]),
            true,
            0.0,
        );
        synth_opt = synth_opt.with_draw(HashMap::from([(
            "e".to_string(),
            HashMap::from([("energy".to_string(), -10.0)]),
        )]));
        let w1: BTreeMap<String, f64> =
            [("credit", 1.0), ("machine_hour", 0.5)].iter().map(|(k, v)| (k.to_string(), *v)).collect();
        let w2: BTreeMap<String, f64> =
            [("credit", 0.5), ("machine_hour", 1.0)].iter().map(|(k, v)| (k.to_string(), *v)).collect();
        let cheap_mh = core.plan_funding(&synth, &synth_opt, Some(&synth_groups), Some(&synth_rates), Some(&w1), None);
        let cheap_credit = core.plan_funding(&synth, &synth_opt, Some(&synth_groups), Some(&synth_rates), Some(&w2), None);
        check(
            &mut failures,
            "the cheaper source is chosen even though it is not the alphabetically first",
            cheap_mh.conversions[0].from == "machine_hour",
            &cheap_mh.conversions[0].from,
        );
        check(
            &mut failures,
            "the same world with different WEIGHTS pays from the other source",
            cheap_credit.conversions[0].from == "credit",
            &cheap_credit.conversions[0].from,
        );
        check(
            &mut failures,
            "the amount bought is the deficit, measured at the observed rate",
            (cheap_mh.conversions[0].amount_from - 2.5).abs() <= 1e-12
                && (cheap_mh.conversions[0].amount_to - 10.0).abs() <= 1e-12,
            "",
        );
    }

    println!("=== 10. §3.5/§4.9: an observation with a hole is not a discount ===");
    {
        let mut o5 = DofOrchestrator::new(0.05);
        let s5 = o5.measure(&fx::arbitrage_scene());
        let c5 = o5.mapper().last_observation.clone();
        let cc5 = c5.as_ref();
        let d5 = o5.mapper().last_declaration.clone().unwrap();
        check(
            &mut failures,
            "the variant observation is detected as not arbitrage-free",
            cc5.map(|x| !x.world.is_arbitrage_free() && !x.world.arbitrage_edges().is_empty())
                .unwrap_or(false),
            "",
        );
        check(
            &mut failures,
            "no rate survives the hole: every pair is undetermined",
            d5.rates.is_empty()
                && cc5.unwrap().world.rate("credit", "energy", true).status == "undetermined",
            "",
        );
        let core5 = DofCalculusCore::new();
        let adm5 = core5.apply_resource_gate(
            &s5,
            &[opts::opt_funded()],
            Some(&d5.groups),
            Some(&d5.rates),
            Some(&d5.weights),
            d5.mandate_cap,
        );
        check(
            &mut failures,
            "an exchange that cannot be priced does not happen: the option is insolvent",
            adm5.0.is_empty() && adm5.1.len() == 1 && adm5.1[0].gate == "insolvency",
            "",
        );
        check(
            &mut failures,
            "the two observations are different observations (their digests differ)",
            cc5.unwrap().observation_digest != c.unwrap().observation_digest,
            "",
        );
    }

    println!("=== 11. §6: the report carries the reasons ===");
    let standard = opts::gateable_set();
    let admissible_all = core.apply_structural_gate(&state, &standard, c);
    let admissible_final = core.apply_resource_gate(
        &state,
        &admissible_all.0,
        Some(&decl.groups),
        Some(&decl.rates),
        Some(&decl.weights),
        decl.mandate_cap,
    );
    check(
        &mut failures,
        "the three gates compose and each removal names its gate",
        admissible_all.1.len() == 1
            && admissible_all.1[0].gate == "collapse"
            && admissible_final.1.len() == 2
            && admissible_final.1[0].gate == "insolvency"
            && admissible_final.1[1].gate == "insolvency",
        "",
    );
    check(
        &mut failures,
        "what survives is exactly what is both harmless and permitted",
        admissible_final.0.len() == 2
            && admissible_final.0[0].option_id == "opt_win"
            && admissible_final.0[1].option_id == "opt_lose",
        "",
    );
    let selected = core.evaluate_and_select(&state, &admissible_final.0, c);
    check(
        &mut failures,
        "the surviving irreversible option is the one that wins",
        selected.as_ref().map(|s| s.option_id.as_str()) == Some("opt_win"),
        "",
    );
    let mut report_options = standard.clone();
    report_options.push(opts::opt_funded());
    let mut removed_all = admissible_all.1.clone();
    removed_all.extend(admissible_final.1.clone());
    let mut provenance: BTreeMap<String, MandateValue> = BTreeMap::new();
    provenance.insert(
        "source".to_string(),
        MandateValue::Text("measured balance (§4.8)".to_string()),
    );
    let report = core.report(
        &state,
        &report_options,
        &selected,
        "FAST_PASS",
        ReportInput {
            declaration: Some(&decl),
            removed: removed_all,
            groups: Some(&decl.groups),
            rates: Some(&decl.rates),
            weights: Some(&decl.weights),
            cap: decl.mandate_cap,
            ctx: c,
            means_provenance: provenance,
        },
    );
    let mut rows: BTreeMap<String, crate::dof_core::EntityReportRow> = BTreeMap::new();
    for row in report.entities.iter() {
        rows.insert(row.entity_id.clone(), row.clone());
    }
    check(
        &mut failures,
        "every row carries the recoverability verdict and its witness",
        rows["passive"].recoverability.verdict == "proven_unreachable"
            && rows["unobserved"].recoverability.observation == "partial"
            && rows["revivable"].recoverability.witness.len() == 1,
        "",
    );
    check(
        &mut failures,
        "the report says the subgraph came from a named observation",
        report.observation_digest.as_deref() == Some(c.unwrap().observation_digest.as_str())
            && report.observation_digest.as_ref().map(|d| d.len()).unwrap_or(0) == 64,
        "",
    );
    check(
        &mut failures,
        "the report's index equals the calculation over calc",
        (report.total_system_dof - core.calculate_system_dof(&state, None, c)).abs() <= 1e-12,
        "",
    );
    let mut by_id: BTreeMap<String, crate::dof_core::OptionReportRow> = BTreeMap::new();
    for row in report.options.iter() {
        by_id.insert(row.option_id.clone(), row.clone());
    }
    check(
        &mut failures,
        "an irreversible option is reported with what it closes and with the decomposed loss",
        !by_id["opt_win"].is_reversible
            && by_id["opt_win"].closed.len() == 1
            && (by_id["opt_win"].closure_share["robot"] - (-0.0124)).abs() < 1e-4,
        "",
    );
    check(
        &mut failures,
        "the destructive option is reported as destructive (an auditable charge line)",
        by_id["opt_collapse"].collapse_charges.len() == 1
            && by_id["opt_collapse"].collapse_charges[0].entity_id == "robot",
        "",
    );
    check(
        &mut failures,
        "the per-option row shows what was bought and at which price",
        by_id["opt_funded"].resources_uncovered.is_empty(),
        "",
    );
    let mut measure_unknown = ActionOption::new(
        "m".to_string(),
        String::new(),
        HashMap::from([("unobserved".to_string(), 0.1)]),
        true,
        1000.0,
    );
    measure_unknown.estimated_duration_mks = 1000.0;
    check(
        &mut failures,
        "an unmapped entity that no candidate resolves makes the decision incomplete",
        core.is_incomplete(&state, &[opts::opt_win()])
            && !core.is_incomplete(&state, &[measure_unknown]),
        "",
    );
    check(
        &mut failures,
        "the u₀ band of §4.7 is respected by the unmeasured Options lens",
        crate::measurement::u_min() <= 0.5 && 0.5 <= U_MAX,
        "",
    );

    println!("=== 12. §3.4.3/§11.9: the canonical form ===");
    {
        let mut oa = DofOrchestrator::new(0.05);
        oa.measure(&fx::scene(&Options::default()));
        let mut reordered = fx::scene(&Options::default());
        if let Some(entry) = reordered.get_mut(crate::graph_mapper::WORLD_KEY) {
            if let Some(w) = entry.world.as_mut() {
                w.graph.acts.reverse();
                w.graph.exchanges.reverse();
            }
        }
        let mut ob = DofOrchestrator::new(0.05);
        ob.measure(&reordered);
        check(
            &mut failures,
            "the order of the observation's parts does not change the ruler",
            oa.mapper().last_declaration.as_ref().unwrap().digest()
                == ob.mapper().last_declaration.as_ref().unwrap().digest(),
            "",
        );
        check(
            &mut failures,
            "...nor the fingerprint of the observation",
            oa.mapper().last_observation.as_ref().unwrap().observation_digest
                == ob.mapper().last_observation.as_ref().unwrap().observation_digest,
            "",
        );
        let mut mutated = fx::scene(&Options::default());
        if let Some(entry) = mutated.get_mut(crate::graph_mapper::WORLD_KEY) {
            if let Some(w) = entry.world.as_mut() {
                for e in w.graph.exchanges.iter_mut() {
                    if e.id == "q1" {
                        e.wants.insert("energy".to_string(), 2.5);
                    }
                }
            }
        }
        let mut oc = DofOrchestrator::new(0.05);
        oc.measure(&mutated);
        check(
            &mut failures,
            "a single mutated quote changes both fingerprints",
            oc.mapper().last_declaration.as_ref().unwrap().digest()
                != oa.mapper().last_declaration.as_ref().unwrap().digest()
                && oc.mapper().last_observation.as_ref().unwrap().observation_digest
                    != oa.mapper().last_observation.as_ref().unwrap().observation_digest,
            "",
        );
        let mut trec_d: BTreeMap<String, f64> = BTreeMap::new();
        trec_d.insert("passive".to_string(), fx::TREC_MKS);
        trec_d.insert("revivable".to_string(), 1000000.0);
        trec_d.insert("unobserved".to_string(), fx::TREC_MKS);
        let mut od = DofOrchestrator::new(0.05);
        od.measure(&fx::scene(&Options {
            t_rec_override: Some(trec_d),
            ..Options::default()
        }));
        check(
            &mut failures,
            "a mutated T_rec changes the ruler (a horizon is a measurement choice)",
            od.mapper().last_declaration.as_ref().unwrap().digest() != decl.digest(),
            "",
        );
    }
    let canonical = decl.canonical_text();
    check(
        &mut failures,
        "the canonical form has no exponent notation",
        !canonical.contains("e-") && !canonical.contains("e+"),
        "",
    );
    check(&mut failures, "the digest is 64 hex characters", decl.digest().len() == 64, "");

    println!("=== 13. §10: what changed since v0.6, as facts ===");
    check(
        &mut failures,
        "the v0.6 ruler is no longer reproduced: the declaration carries derived content",
        decl.digest() != V06_DIGEST,
        "",
    );
    {
        let mut o6 = DofOrchestrator::new(0.05);
        let s6 = o6.measure(&fx::scene(&Options {
            no_world: true,
            ..Options::default()
        }));
        let d6 = o6.mapper().last_declaration.clone().unwrap();
        let core6 = DofCalculusCore::new();
        check(
            &mut failures,
            "without an observation nothing is proven: a known zero is NOT excluded",
            core6.is_included(&s6.entities["revivable"], None, Some(&s6)),
            "",
        );
        check(
            &mut failures,
            "without an observation no weight, no cap and no rate are invented",
            d6.weights.is_empty() && d6.mandate_cap.is_none() && d6.rates.is_empty(),
            "",
        );
        let blocks6 = s6.entities["drone"].measurement.as_ref().unwrap().blocks.clone();
        check(
            &mut failures,
            "without an observation the unit problem returns: the group sum adds 1 credit to 1 joule to 1 machine-hour as if they were one unit",
            blocks6.len() == 1 && blocks6[0].0 == 4.0 && blocks6[0].1 == 18.0,
            &num(blocks6[0].1),
        );
        check(
            &mut failures,
            "the same entity has a different DoF with and without the observation",
            !near(
                s6.entities["drone"].current_dof,
                state.entities["drone"].current_dof,
            ),
            "",
        );
    }

    println!();
    println!("RULER  digest={}", decl.digest());
    println!("OBSERVATION digest={}", c.unwrap().observation_digest);
    check(
        &mut failures,
        "the ruler digest equals the frozen v0.7 value byte for byte",
        decl.digest() == EXPECTED_RULER_DIGEST,
        "",
    );
    check(
        &mut failures,
        "the observation digest equals the frozen v0.7 value byte for byte",
        c.unwrap().observation_digest == EXPECTED_OBSERVATION_DIGEST,
        "",
    );
    println!();
    println!(
        "REPORT (v0.7 fixture): entities={} options={} removed={} index={}",
        report.entities.len(),
        report.options.len(),
        report.removed_options.len(),
        num(report.total_system_dof)
    );
    failures
}

/// The reference dump: the canonical text of the release fixture's declaration and
/// both digests — the tool that makes a cross-port mismatch a diff instead of a
/// mystery.
pub fn dump_reference() {
    let mut orch = DofOrchestrator::new(0.05);
    orch.measure(&fx::scene(&Options::default()));
    let decl = orch.mapper().last_declaration.clone().expect("declaration");
    println!("{}", decl.canonical_text());
    println!("RULER {}", decl.digest());
    if let Some(ctx) = orch.mapper().last_observation.as_ref() {
        println!("OBSERVATION {}", ctx.observation_digest);
    }
}
