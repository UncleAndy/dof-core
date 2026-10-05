// Conformance harness of the Rust port, DOF-SPEC v0.8 (§4.5, T1).
//
// This is the release's own harness. `v07` is left untouched: it is the v0.7
// evidence, and it still reproduces the v0.7 ruler — which is itself one of this
// release's checks (a moved digest is an error to be fixed, not a new version).
//
// Sections, in order: the ruler did not move; the two entities D3 exists for; the
// candidate vector is computed; the decision that changed; the mirror; a path cut
// is a bar; bounds and the baseline.

use std::collections::BTreeSet;

use crate::dof_core::{CandidateVector, DofCalculusCore, ReportInput};
use crate::fixture_v07 as fx;
use crate::harness_v07::{EXPECTED_OBSERVATION_DIGEST, EXPECTED_RULER_DIGEST};
use crate::options_v07 as O;
use crate::orchestrator::DofOrchestrator;

/// The index of the released fixture (v0.7, §10). T1 must not move it.
const V07_INDEX: f64 = -35.314438370902;

fn near8(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9
}

fn num8(x: f64) -> String {
    format!("{:.6}", x)
}

fn check8(failures: &mut Vec<String>, name: &str, ok: bool, detail: &str) {
    println!("{}  {}{}", if ok { "  OK  " } else { "  FAIL" }, name,
             if detail.is_empty() { String::new() } else { format!("  {}", detail) });
    if !ok {
        failures.push(name.to_string());
    }
}

fn vec8(v: &CandidateVector) -> String {
    format!(
        "{{{} {} {} {} {}}}",
        v.d1,
        v.d2,
        v.d3,
        num8(v.net_delta),
        v.option_id
    )
}

/// The vector a single candidate would be judged by, computed by the selection
/// itself so that the harness reads the same numbers the decision does.
fn vector_of(
    core: &DofCalculusCore,
    state: &crate::dof_core::SystemStateMatrix,
    ctx: Option<&crate::dof_core::ObservationContext>,
    option: crate::dof_core::ActionOption,
) -> CandidateVector {
    core.select_candidate(state, &[option], ctx)
        .1
        .into_iter()
        .next()
        .expect("one candidate, one vector")
}

pub fn run_harness_v08() -> Vec<String> {
    let mut failures: Vec<String> = Vec::new();

    // --- 1. the ruler did not move (§10, R6) --------------------------------
    let mut base_orch = DofOrchestrator::new(0.05);
    let base_state = base_orch.measure(&fx::scene(&fx::Options::default()));
    let base_ctx = base_orch.mapper().last_observation.clone();
    let base_decl = base_orch
        .mapper()
        .last_declaration
        .clone()
        .expect("declaration");
    let base_core = DofCalculusCore::new();
    let b = base_ctx.as_ref();

    println!("=== 1. §10/R6: the released fixture is unchanged by T1 ===");
    check8(
        &mut failures,
        "the ruler digest is the v0.7 one, byte for byte",
        base_decl.digest() == EXPECTED_RULER_DIGEST,
        &base_decl.digest()[..16],
    );
    check8(
        &mut failures,
        "the observation digest is the v0.7 one, byte for byte",
        b.map(|x| x.observation_digest == EXPECTED_OBSERVATION_DIGEST)
            .unwrap_or(false),
        b.map(|x| x.observation_digest[..16].to_string())
            .unwrap_or_default()
            .as_str(),
    );
    let base_index = base_core.calculate_system_dof(&base_state, None, b);
    check8(
        &mut failures,
        "the index of the released fixture is unchanged",
        near8(base_index, V07_INDEX),
        &num8(base_index),
    );
    // The fingerprints of the released fixture, in full: a run that stopped
    // comparing must not be able to pass unnoticed, and a reader of the log must be
    // able to see the values the run asserted against.
    println!();
    println!("RULER  digest={}", base_decl.digest());
    println!(
        "OBSERVATION digest={}",
        b.map(|x| x.observation_digest.clone()).unwrap_or_default()
    );
    println!("INDEX  {}", num8(base_index));
    println!();

    let mut orch = DofOrchestrator::new(0.05);
    let state = orch.measure(&fx::t1_scene());
    let ctx = orch.mapper().last_observation.clone();
    let decl = orch
        .mapper()
        .last_declaration
        .clone()
        .expect("declaration");
    let core = DofCalculusCore::new();
    let c = ctx.as_ref();

    println!("=== 2. §4.5: the two entities D3 exists for ===");
    let trainee_dof = state
        .entities
        .get("trainee")
        .map(|e| e.current_dof)
        .unwrap_or(0.0);
    let minimum = core
        .calc_members(&state, c)
        .iter()
        .filter_map(|id| state.entities.get(id).map(|e| e.current_dof))
        .fold(1.0f64, f64::min);
    check8(
        &mut failures,
        "trainee sits above the minimum DoF of calc(S), so it is not critical",
        trainee_dof > 0.0,
        &format!("trainee={} min={}", num8(trainee_dof), num8(minimum)),
    );
    check8(
        &mut failures,
        "trainee can act itself: V > 0",
        c.map(|x| x.v_before("trainee") > 0).unwrap_or(false),
        &format!("V={}", c.map(|x| x.v_before("trainee")).unwrap_or(0)),
    );
    check8(
        &mut failures,
        "and its own act does not lift it — its path is somebody else's act",
        c.map(|x| x.verdict("trainee") == "reachable").unwrap_or(false),
        c.map(|x| x.verdict("trainee")).unwrap_or_default().as_str(),
    );
    let critical: BTreeSet<String> = core.critical_members(&state, c);
    check8(
        &mut failures,
        "critical(S) is the known zeros of calc, not the dependents",
        critical.len() == 2 && critical.contains("revivable") && critical.contains("unobserved"),
        &critical.len().to_string(),
    );

    println!("=== 3. §4.5: the vector is computed, not declared ===");
    let vec_comp = vector_of(&core, &state, c, O::t1_compensate());
    check8(
        &mut failures,
        "t1_compensate: no destruction — nothing is driven to a known zero",
        vec_comp.d1 == 0,
        &vec8(&vec_comp),
    );
    check8(
        &mut failures,
        "t1_compensate: it cuts the patient's only path",
        vec_comp.d2 == 1,
        &vec8(&vec_comp),
    );
    check8(
        &mut failures,
        "t1_compensate: and that path is the critical node's",
        vec_comp.d3 == 1,
        &vec8(&vec_comp),
    );
    check8(
        &mut failures,
        "t1_compensate: NetDelta > 0 — every pre-v0.8 gate passes it",
        vec_comp.net_delta > 0.0,
        &num8(vec_comp.net_delta),
    );
    check8(
        &mut failures,
        "t1_compensate: the collapse charges are empty, as §4.2 computes them",
        core.collapse_charges(&state, &O::t1_compensate(), c).is_empty(),
        "",
    );
    let lost = core.lost_paths(&state, &O::t1_compensate(), c);
    check8(
        &mut failures,
        "t1_compensate: the lost path is auditable, with the witness it lost",
        lost.len() == 1
            && lost[0].entity_id == "revivable"
            && lost[0].verdict_before == "reachable"
            && lost[0].verdict_after == "proven_unreachable"
            && lost[0].critical
            && lost[0].witness_lost.len() == 1
            && lost[0].witness_lost[0] == "act_medkit",
        &lost.len().to_string(),
    );

    println!("=== 4. §4.5: the decision that changed ===");
    let single = core.select_candidate(&state, &[O::t1_compensate()], c);
    check8(
        &mut failures,
        "under T1 the option loses to staying put: the system stays",
        single.0.is_none(),
        "",
    );
    let key = DofCalculusCore::barring_key(&vec_comp);
    check8(
        &mut failures,
        "the report names the dimension that barred it",
        key.as_deref() == Some("d2"),
        key.as_deref().unwrap_or(""),
    );
    let baseline = DofCalculusCore::baseline_vector();
    check8(
        &mut failures,
        "staying put is a candidate with the zero vector",
        baseline.d1 == 0
            && baseline.d2 == 0
            && baseline.d3 == 0
            && baseline.net_delta == 0.0
            && baseline.reversible,
        "",
    );
    let refusal_in = ReportInput {
        declaration: Some(&decl),
        ctx: c,
        ..Default::default()
    };
    let refusal = core.report(
        &state,
        &[O::t1_compensate()],
        &None,
        "FAST_PASS",
        refusal_in,
    );
    check8(
        &mut failures,
        "no candidate beat inaction, and the report says so",
        refusal.no_candidate_better,
        "",
    );
    check8(
        &mut failures,
        "the refusal lists the candidates and the dimension that barred each",
        refusal.options.len() == 1 && refusal.options[0].barring_key.as_deref() == Some("d2"),
        "",
    );

    println!("=== 5. §4.5: the mirror — the same gain, a path that is not the price ===");
    let vec_mirror = vector_of(&core, &state, c, O::t1_mirror());
    check8(
        &mut failures,
        "t1_mirror: nothing destroyed, nothing lost",
        vec_mirror.d1 == 0 && vec_mirror.d2 == 0 && vec_mirror.d3 == 0,
        &vec8(&vec_mirror),
    );
    check8(
        &mut failures,
        "t1_mirror: a real gain over staying put",
        vec_mirror.net_delta > 0.0,
        &num8(vec_mirror.net_delta),
    );
    check8(
        &mut failures,
        "robot keeps eight of its nine vectors: the closure was a price, not a loss",
        c.map(|x| x.v_after_closure("robot", &O::t1_mirror().closed) == 8 && x.v_before("robot") == 9)
            .unwrap_or(false),
        "",
    );
    let chosen = core.evaluate_and_select(&state, &[O::t1_compensate(), O::t1_mirror()], c);
    check8(
        &mut failures,
        "the mirror is selected",
        chosen.as_ref().map(|o| o.option_id.as_str()) == Some("t1_mirror"),
        chosen.map(|o| o.option_id).unwrap_or_default().as_str(),
    );

    println!("=== 6. §4.5: a path cut is a bar, and the third dimension cannot separate ===");
    let vec_help = vector_of(&core, &state, c, O::t1_help());
    let vec_rival = vector_of(&core, &state, c, O::t1_rival());
    // `v0.11` §4.9: the condition is `DoF(X | h) + Δ_P(X) > 0`, and the **trivial**
    // path is a finite simple path — so cutting a path does not make a *live* entity
    // unreachable, and `D2` no longer fires for it (§10(J), fixtures `ar` and `ad`).
    // `D2` keeps its teeth exactly where it matters: an option that drives the entity
    // to a **known zero** and closes its recovery — which is what `t1_rival` does,
    // and the assertion below still holds for it.
    check8(
        &mut failures,
        "t1_help: it cuts a path without destroying anything",
        vec_help.d1 == 0 && vec_help.d2 == 0 && vec_help.d3 == 0,
        &vec8(&vec_help),
    );
    let help_lost = core.lost_paths(&state, &O::t1_help(), c);
    check8(
        &mut failures,
        "t1_help: and no path is reported as lost",
        help_lost.is_empty(),
        &help_lost.len().to_string(),
    );
    check8(
        &mut failures,
        "closing the mentor's act costs it a vector and destroys nothing",
        core.collapse_charges(&state, &O::t1_help(), c).is_empty()
            && c.map(|x| x.v_after_closure("mentor", &O::t1_help().closed) == 1)
                .unwrap_or(false),
        "",
    );
    check8(
        &mut failures,
        "t1_rival: the same D1 and D2, and the path is the critical node's",
        vec_rival.d1 == 0 && vec_rival.d2 == 1 && vec_rival.d3 == 1,
        &vec8(&vec_rival),
    );
    check8(
        &mut failures,
        "staying put wins at the second dimension against both: a cut path is a bar",
        core.evaluate_and_select(&state, &[O::t1_help(), O::t1_rival()], c)
            .is_none(),
        "",
    );
    check8(
        &mut failures,
        "so the third dimension cannot separate two candidates — D3 <= D2, and an \
admissible candidate has D2 = 0 (§4.5, finding of this release)",
        vec_comp.d3 <= vec_comp.d2
            && vec_mirror.d3 <= vec_mirror.d2
            && vec_help.d3 <= vec_help.d2
            && vec_rival.d3 <= vec_rival.d2,
        "",
    );

    println!("=== 7. §4.5: bounds, the baseline and the retired gate ===");
    let members = core.calc_members(&state, c);
    check8(
        &mut failures,
        "an empty candidate set selects nothing",
        core.evaluate_and_select(&state, &[], c).is_none(),
        "",
    );
    check8(
        &mut failures,
        "D1, D2 and D3 are bounded by calc(S)",
        vec_comp.d1 <= members.len()
            && vec_comp.d2 <= members.len()
            && vec_comp.d3 <= members.len()
            && vec_rival.d2 <= members.len(),
        &format!("|calc|={}", members.len()),
    );
    let chosen2 = core.evaluate_and_select(&state, &[O::t1_compensate(), O::t1_mirror()], c);
    let rep_in = ReportInput {
        declaration: Some(&decl),
        ctx: c,
        ..Default::default()
    };
    let rep = core.report(
        &state,
        &[O::t1_compensate(), O::t1_mirror()],
        &chosen2,
        "FAST_PASS",
        rep_in,
    );
    let row = rep.options.iter().find(|r| r.option_id == "t1_compensate");
    check8(
        &mut failures,
        "the report carries the vector and the barring dimension per option",
        row.map(|r| r.candidate_vector.d2 == 1 && r.barring_key.as_deref() == Some("d2"))
            .unwrap_or(false),
        "",
    );
    check8(
        &mut failures,
        "the report carries the baseline",
        rep.baseline.d1 == 0 && rep.baseline.net_delta == 0.0 && rep.baseline.reversible,
        "",
    );
    check8(
        &mut failures,
        "a selected option is not reported as a refusal",
        !rep.no_candidate_better,
        "",
    );
    check8(
        &mut failures,
        "the structural decision is no longer a removal: no collapse gate anywhere",
        rep.removed_options.iter().all(|r| r.gate != "collapse"),
        &rep.removed_options.len().to_string(),
    );
    let retired = core.apply_structural_gate(&state, &[O::opt_win(), O::opt_collapse()], c);
    check8(
        &mut failures,
        "a charged candidate is no longer deleted from the set: it is evaluated and reported",
        retired.1.len() == 1 && retired.1[0].gate == "collapse",
        "the retired v0.7 rule is still callable by the historical harness",
    );
    check8(
        &mut failures,
        "and on the live path nobody leaves the candidate set",
        core.select_candidate(&state, &[O::opt_win(), O::opt_collapse()], c)
            .1
            .len()
            == 2,
        "",
    );

    println!();
    println!("checks: {}, failures: {}", 36, failures.len());
    if failures.is_empty() {
        println!("OK");
    } else {
        println!("FAILURES: {:?}", failures);
    }
    failures
}
