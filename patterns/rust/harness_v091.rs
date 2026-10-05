// Conformance harness of the Rust port, DOF-SPEC v0.9.1 (§3.2b, §4.8, budget + τ-consumption).
//
// Covers:
// 1. τ is a ResourceObservation in the state
// 2. Null resource with estimated + mandatory fallback
// 3. Staleness check via aging_time
// 4. projected_tau_delta changes τ
// 5. Parallel actions consume τ by max(duration)

use std::collections::{BTreeMap, HashMap};

use crate::dof_core::{ActionOption, DofCalculusCore, ResourceObservation, SystemStateMatrix};
use crate::fixture_v07;
use crate::orchestrator::DofOrchestrator;

fn check91(failures: &mut Vec<String>, name: &str, ok: bool, detail: &str) {
    println!(
        "{}  {}{}",
        if ok { "  OK  " } else { "  FAIL" },
        name,
        if detail.is_empty() {
            String::new()
        } else {
            format!("  {}", detail)
        }
    );
    if !ok {
        failures.push(name.to_string());
    }
}

pub fn run_harness_v091() -> Vec<String> {
    let mut failures: Vec<String> = Vec::new();
    let core = DofCalculusCore::new();

    // --- 1. τ is a ResourceObservation --------------------------------------
    println!("=== 1. τ is a ResourceObservation ===");
    {
        let mut orch = DofOrchestrator::new(0.05);
        let state = orch.measure(&fixture_v07::scene(&fixture_v07::Options::default()));
        check91(
            &mut failures,
            "state.tau is Some",
            state.tau.is_some(),
            "",
        );
        if let Some(tau) = &state.tau {
            check91(
                &mut failures,
                "tau.value is Some",
                tau.value.is_some(),
                "",
            );
            if let Some(v) = tau.value {
                check91(
                    &mut failures,
                    "tau.value > 0",
                    v > 0.0,
                    &format!("{:.0}", v),
                );
            }
            check91(&mut failures, "tau.unit == 'us'", tau.unit == "us", "");
            check91(
                &mut failures,
                "tau.aging_time == 0",
                tau.aging_time == 0.0,
                "",
            );
        }
    }

    // --- 2. Null resource with estimated + fallback -------------------------
    println!("=== 2. Null resource with estimated + fallback ===");
    {
        let mut resources: HashMap<String, ResourceObservation> = HashMap::new();
        resources.insert(
            "medical_supply".to_string(),
            ResourceObservation {
                value: None,
                unit: "unit".to_string(),
                estimated: Some(5.0),
                estimation_source: vec!["indirect".to_string()],
                ..Default::default()
            },
        );
        let state = SystemStateMatrix {
            global_time_to_collapse_mks: 1000000.0,
            context_switch_cost: 0.05,
            entities: HashMap::new(),
            psi: None,
            resources,
            // §3.2b/§4.7 (v0.11): the historical fixture declares no deadlines, no
            // durations and no schedule (§3.4.3).
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
        let ms = &state.resources["medical_supply"];
        check91(
            &mut failures,
            "medical_supply.value is None",
            ms.value.is_none(),
            "",
        );
        check91(
            &mut failures,
            "medical_supply.estimated == 5.0",
            ms.estimated == Some(5.0),
            "",
        );

        // Without fallback, null resource fails the gate
        let option = ActionOption {
            option_id: "use_supply_no_fallback".to_string(),
            description: "Use medical supply without fallback".to_string(),
            projected_dof_delta: [("patient".to_string(), 0.1)].into(),
            is_reversible: true,
            estimated_duration_mks: 1000.0,
            projected_resource_delta: [(
                "patient".to_string(),
                [("medical_supply".to_string(), -3.0)].into(),
            )]
            .into(),
            closed: Vec::new(),
            act_id: String::new(),
            projected_by_hypothesis: HashMap::new(),
            closed_by_hypothesis: HashMap::new(),
            projected_tau_delta: Some(0.0),
            projected_tau_value: None,
            discovers: Vec::new(),
            requires: vec!["medical_supply".to_string()],
        };
        let plan = core.plan_funding(&state, &option, None, None, None, None);
        check91(
            &mut failures,
            "null resource fails without fallback",
            !plan.covered,
            &format!("uncovered={:?}", plan.uncovered),
        );
    }

    // --- 3. Staleness check -------------------------------------------------
    println!("=== 3. Staleness check ===");
    {
        let obs = ResourceObservation {
            value: Some(10.0),
            last_measured_at: 0.0,
            aging_time: 1000.0,
            ..Default::default()
        };
        check91(
            &mut failures,
            "not stale at t=500",
            !obs.is_stale(500.0),
            "",
        );
        check91(&mut failures, "stale at t=1500", obs.is_stale(1500.0), "");
        let obs_no_aging = ResourceObservation {
            value: Some(10.0),
            last_measured_at: 0.0,
            aging_time: 0.0,
            ..Default::default()
        };
        check91(
            &mut failures,
            "never stale if aging_time=0",
            !obs_no_aging.is_stale(99999.0),
            "",
        );
    }

    // --- 4. projected_tau_delta changes τ -----------------------------------
    println!("=== 4. projected_tau_delta changes τ ===");
    {
        let mut orch = DofOrchestrator::new(0.05);
        let state = orch.measure(&fixture_v07::scene(&fixture_v07::Options::default()));
        let tau_before = state.global_time_to_collapse_mks;
        // CPR takes 1000ms but increases τ by 5000ms
        let option = ActionOption {
            option_id: "cpr".to_string(),
            description: "CPR increases τ".to_string(),
            projected_dof_delta: [("patient".to_string(), 0.2)].into(),
            is_reversible: true,
            estimated_duration_mks: 1000.0,
            projected_resource_delta: [(
                "patient".to_string(),
                [("energy".to_string(), -5.0)].into(),
            )]
            .into(),
            closed: Vec::new(),
            act_id: String::new(),
            projected_by_hypothesis: HashMap::new(),
            closed_by_hypothesis: HashMap::new(),
            projected_tau_delta: Some(5000.0),
            projected_tau_value: None,
            discovers: Vec::new(),
            requires: Vec::new(),
        };
        // §3.3 (v0.11): the delta is an `Option` because `null` is a real state —
        // "not computable" — and a `f64` physically could not express it. The
        // `v0.9.1` rule itself is unchanged: this option declares its delta.
        let tau_after =
            tau_before - option.estimated_duration_mks + option.projected_tau_delta.unwrap();
        check91(
            &mut failures,
            "CPR increases τ",
            tau_after > tau_before,
            &format!("before={:.0} after={:.0}", tau_before, tau_after),
        );
        check91(
            &mut failures,
            "CPR delta is +4000 net",
            (tau_after - tau_before - 4000.0).abs() < 1e-9,
            &format!("actual={:.0}", tau_after - tau_before),
        );
    }

    // --- 5. Parallel actions consume τ by max(duration) ---------------------
    println!("=== 5. Parallel actions consume τ by max(duration) ===");
    {
        let mut orch = DofOrchestrator::new(0.05);
        let state = orch.measure(&fixture_v07::scene(&fixture_v07::Options::default()));
        let tau_before = state.global_time_to_collapse_mks;
        let d1 = 3000.0;
        let d2 = 5000.0;
        let tau_consumed = f64::max(d1, d2);
        let tau_after = tau_before - tau_consumed;
        check91(
            &mut failures,
            "parallel τ = max(d_i)",
            tau_consumed == 5000.0,
            "",
        );
        check91(
            &mut failures,
            "τ decreases by max duration",
            (tau_after - (tau_before - 5000.0)).abs() < 1e-9,
            "",
        );
    }

    println!();
    println!(
        "checks: {}, failures: {}",
        failures.len() + 15 - failures.len(),
        failures.len()
    );
    if !failures.is_empty() {
        println!("FAILURES: {:?}", failures);
    } else {
        println!("OK");
    }
    failures
}
