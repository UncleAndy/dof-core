// Conformance harness of the Rust port, DOF-SPEC v0.11 (§3.3, §3.4.2, §3.6, §4.4,
// §4.8b, §4.10).
//
// This is the release's own harness for the conditional layer. `v07`, `v08` and
// `v091` are left untouched: they are the evidence of their releases, and their
// fingerprints are asserted here again as the precondition of everything below —
// a moved digest is an error to be fixed, not a new version.
//
// Sections, in order: one ruler, two readings; the hypothesis set and its five
// validation rules; absence and emptiness reduce to the observed singleton; the
// two forms of §3.3/§4.4 and the refusal to mix them; the temporal condition of
// §4.8b; the robust selection of §4.10.

use std::collections::HashMap;

use crate::conditional::hypothesis_coverage;
use crate::dof_core::{
    tau_of, ActionOption, DofCalculusCore, ObservationContext, SystemStateMatrix,
};
use crate::fixture_v011 as f11;
use crate::fixture_v07 as fx;
use crate::harness_v07::{EXPECTED_OBSERVATION_DIGEST, EXPECTED_RULER_DIGEST};
use crate::hypothesis::{
    coverage_of, resolved_members, validate_set, Hypothesis, HypothesisSet,
    OBSERVED_HYPOTHESIS_ID,
};
use crate::orchestrator::DofOrchestrator;
use crate::world_graph::ClosedRef;

/// The **ruler-level** digest of the released v0.7 fixture: the same declaration
/// with `entities`, `freeze` and `verdicts` excluded (§3.4.2/§3.4.3). It is the
/// value the Python port's `ruler_digest()` produces for that fixture, byte for
/// byte, and it is the comparability key every release must keep — which is a
/// different claim from the declaration digest, and is asserted separately.
const V07_RULER_DIGEST_V011: &str =
    "81948b8b3ca9805a75624c4d136f5926b9347c42ed3c6415966fc6f43f4301ba";

fn check11(failures: &mut Vec<String>, name: &str, ok: bool, detail: &str) {
    // The mark is `"  OK   "` — the same three spaces the other ports print, so
    // that the cross-port verifier's row greps (`^  OK   the ruler is shared…`)
    // read this port's evidence exactly as they read the others'.
    println!(
        "{}{}{}",
        if ok { "  OK   " } else { "  FAIL  " },
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

fn has_substring(errs: &[String], needle: &str) -> bool {
    errs.iter().any(|e| e.contains(needle))
}

fn num11(x: f64) -> String {
    format!("{:.6}", x)
}

fn hyp(id: &str, plausible: bool, state: &SystemStateMatrix, basis: &str) -> Hypothesis {
    Hypothesis {
        id: id.to_string(),
        plausible,
        state: state.clone(),
        collapse_source_candidates: Vec::new(),
        basis: basis.to_string(),
    }
}

pub fn run_harness_v011() -> Vec<String> {
    let mut failures: Vec<String> = Vec::new();

    // --- 0. the released evidence still stands ------------------------------
    // The released fixture's own fingerprints, asserted on the released fixture,
    // exactly as `harness_v08` asserts them: the T1 scene is a different scene and
    // its digests are legitimately its own.
    let mut orch_rel = DofOrchestrator::new(0.05);
    orch_rel.measure(&fx::scene(&fx::Options::default()));
    let rel_decl = orch_rel.mapper().last_declaration.clone().unwrap();
    let rel_ctx = orch_rel.mapper().last_observation.clone().unwrap();
    check11(
        &mut failures,
        "the v0.7 declaration digest is still reproduced",
        rel_decl.digest() == EXPECTED_RULER_DIGEST,
        &rel_decl.digest()[..16],
    );
    check11(
        &mut failures,
        "the released fixture still carries the v0.7 observation digest",
        rel_ctx.observation_digest == EXPECTED_OBSERVATION_DIGEST,
        &rel_ctx.observation_digest[..16],
    );
    check11(
        &mut failures,
        "the v0.7 ruler digest is still reproduced (the exclusion rule did not move)",
        rel_decl.ruler_digest() == V07_RULER_DIGEST_V011,
        &rel_decl.ruler_digest()[..16],
    );

    let mut orch_obs = DofOrchestrator::new(0.05);
    let state = orch_obs.measure(&fx::t1_scene());
    let ctx = orch_obs.mapper().last_observation.clone().unwrap();
    let decl = orch_obs.mapper().last_declaration.clone().unwrap();
    let core = orch_obs.core();

    println!("=== 0. §3.4.2/§4.10: one ruler, two readings ===");

    // The second reading is a **transformation of one observed state**, mapped
    // through its own orchestrator so that its declaration, its ruler and its graph
    // come from the same named procedures as the observed one.
    let mut orch_alt = DofOrchestrator::new(0.05);
    let state_b = f11::v011_reconcile(&orch_alt.measure(&f11::v011_scaled_scene(
        &fx::t1_scene(),
        1.25,
    )));
    let ctx_b = orch_alt.mapper().last_observation.clone().unwrap();
    let decl_b = orch_alt.mapper().last_declaration.clone().unwrap();

    check11(
        &mut failures,
        "the ruler is shared across the release: v0.11 ruler == v0.7 ruler",
        decl.ruler_digest() == rel_decl.ruler_digest(),
        &format!(
            "{} vs {}",
            &decl.ruler_digest()[..16],
            &rel_decl.ruler_digest()[..16]
        ),
    );
    check11(
        &mut failures,
        "two readings of one cycle share the ruler digest byte for byte",
        decl.ruler_digest() == decl_b.ruler_digest(),
        &format!(
            "{} vs {}",
            &decl.ruler_digest()[..16],
            &decl_b.ruler_digest()[..16]
        ),
    );
    check11(
        &mut failures,
        "…while their full declaration digests differ",
        decl.digest() != decl_b.digest(),
        &format!("{} vs {}", &decl.digest()[..16], &decl_b.digest()[..16]),
    );
    check11(
        &mut failures,
        "…and the observation digest is **shared** — §3.5's `G` is not branched",
        ctx.observation_digest == ctx_b.observation_digest,
        &format!(
            "{} vs {}",
            &ctx.observation_digest[..16],
            &ctx_b.observation_digest[..16]
        ),
    );

    let h_obs = hyp(
        OBSERVED_HYPOTHESIS_ID,
        true,
        &state,
        "the observed reading",
    );
    let h_alt = hyp(
        "h_alt",
        true,
        &state_b,
        "declared Variety counter scaled by 1.25",
    );
    println!();

    // --- 1. §3.6: the hypothesis set and its validation rules ---------------
    println!("=== 1. §3.6: five validation rules, each on its own defect ===");
    let well_formed = validate_set(&state, &[h_obs.clone(), h_alt.clone()]);
    check11(
        &mut failures,
        "a well-formed two-member set is admissible",
        well_formed.is_empty(),
        &format!("{:?}", well_formed),
    );

    let dup = vec![
        h_obs.clone(),
        hyp(OBSERVED_HYPOTHESIS_ID, true, &state_b, ""),
    ];
    check11(
        &mut failures,
        "duplicate ids are refused",
        has_substring(&validate_set(&state, &dup), "ids are not unique"),
        "",
    );

    let entity = f11::v011_entity(&state, 0);
    let omitted = f11::v011_without_entity(&state_b, &entity);
    check11(
        &mut failures,
        "a reading that omits an entity of S is refused",
        has_substring(
            &validate_set(
                &state,
                &[h_obs.clone(), hyp("h_omit", true, &omitted, "")],
            ),
            "omits entities",
        ),
        "",
    );

    let extra = f11::v011_with_extra_entity(&state_b);
    check11(
        &mut failures,
        "a reading that declares an entity not in S is refused",
        has_substring(
            &validate_set(&state, &[h_obs.clone(), hyp("h_extra", true, &extra, "")]),
            "declares entities not in the observed state",
        ),
        "",
    );

    let broken = f11::v011_break_lens_product(&state_b, &entity);
    check11(
        &mut failures,
        "a stated DoF that its own counters do not produce is refused",
        has_substring(
            &validate_set(&state, &[h_obs.clone(), hyp("h_broken", true, &broken, "")]),
            "differs from the product of its own lens values",
        ),
        "",
    );

    let moved = f11::v011_move_collapse_label(&state_b, &entity);
    check11(
        &mut failures,
        "moving the collapse-source label between readings is refused",
        has_substring(
            &validate_set(&state, &[h_obs.clone(), hyp("h_moved", true, &moved, "")]),
            "is_collapse_source differs",
        ),
        "",
    );

    let durations = f11::v011_change_durations(&state_b);
    check11(
        &mut failures,
        "measurement durations are ruler-level and may not vary per reading",
        has_substring(
            &validate_set(&state, &[h_obs.clone(), hyp("h_dur", true, &durations, "")]),
            "durations are ruler-level",
        ),
        "",
    );

    check11(
        &mut failures,
        "the observed state must be present in H",
        has_substring(
            &validate_set(&state, &[h_alt.clone()]),
            "the observed state is absent",
        ),
        "",
    );

    let all_implausible = vec![
        hyp(OBSERVED_HYPOTHESIS_ID, false, &state, ""),
        hyp("h_alt", false, &state_b, ""),
    ];
    check11(
        &mut failures,
        "a set that marks every reading implausible is refused",
        has_substring(
            &validate_set(&state, &all_implausible),
            "H_plausible would be empty",
        ),
        "",
    );
    println!();

    // --- 2. §3.6/§4.10.6: absence and emptiness -----------------------------
    println!("=== 2. §3.6: absence and emptiness are the observed singleton ===");
    let singleton = resolved_members(&state, None);
    check11(
        &mut failures,
        "an absent set resolves to exactly one member",
        singleton.len() == 1,
        "",
    );
    check11(
        &mut failures,
        "…whose id is the observed one",
        singleton[0].id == OBSERVED_HYPOTHESIS_ID,
        "",
    );
    check11(
        &mut failures,
        "…and which is plausible",
        singleton[0].plausible,
        "",
    );
    let empty_set = HypothesisSet {
        coverage: String::new(),
        members: Vec::new(),
        horizon_mks: None,
    };
    check11(
        &mut failures,
        "an empty set resolves the same way",
        resolved_members(&state, Some(&empty_set)).len() == 1,
        "",
    );
    check11(
        &mut failures,
        "an absent coverage claim reads as `partial`, never `complete`",
        coverage_of(None) == "partial" && coverage_of(Some(&empty_set)) == "partial",
        "",
    );
    let complete_set = HypothesisSet {
        coverage: "complete".to_string(),
        members: Vec::new(),
        horizon_mks: None,
    };
    check11(
        &mut failures,
        "a declared coverage claim is reported as declared",
        hypothesis_coverage(Some(&complete_set)) == "complete",
        "",
    );
    println!();

    // --- 3. §3.3/§4.4: the two forms, and no mixing ------------------------
    println!("=== 3. §3.3/§4.4: the two forms of an option ===");
    let flat = f11::v011_option("flat_opt", &entity, 0.01);
    check11(
        &mut failures,
        "a single map of deltas is the flat form",
        flat.projection_form() == "flat",
        "",
    );
    check11(
        &mut failures,
        "an empty closure list is the flat form",
        flat.closure_form() == "flat",
        "",
    );
    check11(
        &mut failures,
        "a flat option is well formed",
        flat.forms_consistent().is_empty(),
        "",
    );

    let mut per_h = f11::v011_option("per_h_opt", &entity, 0.01);
    per_h.projected_dof_delta.clear();
    per_h.projected_by_hypothesis = HashMap::from([
        (
            OBSERVED_HYPOTHESIS_ID.to_string(),
            HashMap::from([(entity.clone(), 0.01)]),
        ),
        (
            "h_alt".to_string(),
            HashMap::from([(entity.clone(), 0.02)]),
        ),
    ]);
    check11(
        &mut failures,
        "a map keyed by reading is the per-hypothesis form",
        per_h.projection_form() == "per_hypothesis",
        "",
    );
    check11(
        &mut failures,
        "…and the delta read under a reading is that reading's",
        per_h.delta_for(OBSERVED_HYPOTHESIS_ID, &entity) == 0.01
            && per_h.delta_for("h_alt", &entity) == 0.02,
        "",
    );
    check11(
        &mut failures,
        "…with entities unlisted for a reading taking 0.0",
        per_h.delta_for("h_alt", "nobody") == 0.0,
        "",
    );

    let mut mixed = f11::v011_option("mixed_opt", &entity, 0.01);
    mixed.projected_by_hypothesis = HashMap::from([(
        OBSERVED_HYPOTHESIS_ID.to_string(),
        HashMap::from([(entity.clone(), 0.02)]),
    )]);
    check11(
        &mut failures,
        "mixing both forms in the projection is `invalid`",
        mixed.projection_form() == "invalid",
        "",
    );
    check11(
        &mut failures,
        "…and the option says so",
        !mixed.forms_consistent().is_empty(),
        "",
    );

    let mut mixed_closure = f11::v011_option("mixed_closed_opt", &entity, 0.01);
    mixed_closure.closed = vec![ClosedRef {
        kind: "mean".to_string(),
        id: "radio".to_string(),
    }];
    mixed_closure.closed_by_hypothesis = HashMap::from([(
        OBSERVED_HYPOTHESIS_ID.to_string(),
        vec![ClosedRef {
            kind: "mean".to_string(),
            id: "radio".to_string(),
        }],
    )]);
    check11(
        &mut failures,
        "mixing both forms in the closure list is `invalid`",
        mixed_closure.closure_form() == "invalid",
        "",
    );
    check11(
        &mut failures,
        "…and the option says so too",
        !mixed_closure.forms_consistent().is_empty(),
        "",
    );
    println!();

    // --- 4. §4.8b: the temporal condition ----------------------------------
    println!("=== 4. §4.8b: the temporal condition, as a condition ===");
    let tau = tau_of(&state);
    check11(
        &mut failures,
        "the released fixture carries a measured τ",
        tau.is_some(),
        "",
    );
    let tau = tau.unwrap_or(0.0);

    let short_act = f11::v011_option("short_act", &entity, 0.01);
    check11(
        &mut failures,
        "an act within τ is viable",
        core.viability(&state, &short_act).viable,
        "",
    );

    let mut long_act = f11::v011_option("long_act", &entity, 0.01);
    long_act.estimated_duration_mks = tau * 2.0;
    check11(
        &mut failures,
        "an act that cannot complete within τ is not viable",
        !core.viability(&state, &long_act).viable,
        "",
    );

    let no_tau = f11::v011_unknown_tau(&state);
    check11(
        &mut failures,
        "an unknown τ is not a licence for an ordinary act",
        !core.viability(&no_tau, &short_act).viable,
        "",
    );
    check11(
        &mut failures,
        "…and the reason names the missing measurement (§4.8b)",
        core.viability(&no_tau, &short_act).reason.contains("unmeasured"),
        "",
    );

    let mut measure = f11::v011_option("measure_tau", &entity, 0.01);
    measure.discovers = vec!["tau".to_string()];
    measure.estimated_duration_mks = 1000.0;
    measure.projected_tau_value = Some(tau + 1.0e6);
    check11(
        &mut failures,
        "a τ measurement is viable under an unknown τ when it can complete",
        core.viability(&no_tau, &measure).viable,
        "",
    );
    check11(
        &mut failures,
        "…and viable under a measured τ as well",
        core.viability(&state, &measure).viable,
        "",
    );

    let mut dead_measure = f11::v011_option("dead_measure", &entity, 0.01);
    dead_measure.discovers = vec!["tau".to_string()];
    dead_measure.estimated_duration_mks = 1000.0;
    dead_measure.projected_tau_value = Some(-1.0);
    check11(
        &mut failures,
        "a τ measurement whose own declared result is dead is not viable",
        !core.viability(&state, &dead_measure).viable,
        "",
    );
    check11(
        &mut failures,
        "…and the retired disjunction `τ = null` OR `τ >= t_m` would have admitted it",
        !core.viability(&no_tau, &dead_measure).viable,
        "",
    );

    let derived = core.derived_tau_delta(&state, &measure);
    let expected = measure.projected_tau_value.unwrap_or(0.0)
        - (tau - measure.estimated_duration_mks);
    check11(
        &mut failures,
        "`projected_tau_delta` is derived for a τ measurement",
        derived.map(|d| (d - expected).abs() < 1e-6).unwrap_or(false),
        &format!("{:?} vs {}", derived, expected),
    );
    check11(
        &mut failures,
        "…and is null, not zero, when τ is unknown",
        core.derived_tau_delta(&no_tau, &measure).is_none(),
        "",
    );
    println!();

    // --- 5. §4.10: the robust selection ------------------------------------
    println!("=== 5. §4.10: robust selection over the declared readings ===");
    // The arithmetic needs room: `coerce_dof` clamps an entity's DoF into [0,1] and
    // the released fixture sits at the top of that interval, where a positive
    // projection cannot move anything. The observed reading of this section is the
    // same fixture with one entity's declared Variety counter scaled down — still a
    // state §4.1 accepts, since its stated DoF is the product of its own counters.
    let observed = f11::v011_set_entity_dof(&state, &entity, 0.25);
    let other = f11::v011_set_entity_dof(&state_b, &entity, 0.30);
    let h_obs_w = hyp(OBSERVED_HYPOTHESIS_ID, true, &observed, "");
    let h_alt_w = hyp("h_alt", true, &other, "");
    let members = vec![h_obs_w.clone(), h_alt_w.clone()];
    let strong = f11::v011_option("strong", &entity, 0.5);
    let weak = f11::v011_option("weak", &entity, 0.1);
    let candidates = vec![strong.clone(), weak.clone()];

    let consistency = validate_set(&observed, &members);
    check11(
        &mut failures,
        "the readings of this section are §4.1-consistent",
        consistency.is_empty(),
        &format!("{:?}", consistency),
    );

    let per_h_all = core.conditional_vectors(&members, &candidates, &ctx, None, None, None, None);
    check11(
        &mut failures,
        "the conditional vectors are produced per option and per reading",
        per_h_all.len() == 2 && per_h_all["strong"].len() == 2,
        "",
    );
    let strong_obs = per_h_all["strong"][OBSERVED_HYPOTHESIS_ID].net_delta;
    let strong_alt = per_h_all["strong"]["h_alt"].net_delta;
    check11(
        &mut failures,
        "…and they are not all alike (the readings really differ)",
        strong_obs != strong_alt,
        &format!("{} vs {}", num11(strong_obs), num11(strong_alt)),
    );

    let least_strong = core.least_favourable(&per_h_all["strong"], &members);
    let greatest_strong = strong_obs.max(strong_alt);
    check11(
        &mut failures,
        "the ordering key is the least-favourable delta, not the greatest",
        least_strong == strong_obs.min(strong_alt) && least_strong < greatest_strong,
        &format!("min={} max={}", num11(least_strong), num11(greatest_strong)),
    );

    // A reading under which one candidate crosses into the zero: the entity sits
    // just above it, and the option's negative projection drives it in. Under the
    // observed reading the same option leaves the entity positive.
    let lows = f11::v011_set_entity_dof(&state_b, &entity, 0.02);
    let h_bar = hyp("h_bar", true, &lows, "one entity sits just above the zero");
    let bar_members = vec![h_obs.clone(), h_bar.clone()];
    let sink = f11::v011_option("sink", &entity, -0.05);
    let per_h_bar = core.conditional_vectors(
        &bar_members,
        &[sink.clone()],
        &ctx,
        None,
        None,
        None,
        None,
    );
    check11(
        &mut failures,
        "an option that crosses into the zero under one reading is charged there (D1 > 0)",
        per_h_bar["sink"]["h_bar"].d1 > 0,
        &format!(
            "d1={} under the bar, d1={} under the observed reading",
            per_h_bar["sink"]["h_bar"].d1, per_h_bar["sink"][OBSERVED_HYPOTHESIS_ID].d1
        ),
    );
    check11(
        &mut failures,
        "…and is not charged under the observed reading",
        per_h_bar["sink"][OBSERVED_HYPOTHESIS_ID].d1 == 0,
        "",
    );
    check11(
        &mut failures,
        "a candidate barred under one plausible reading is not robustly admissible",
        !core.robust_admissible(&per_h_bar["sink"], &bar_members),
        "",
    );

    let split = core.admissible_under(&per_h_bar["sink"], &bar_members);
    check11(
        &mut failures,
        "the per-reading admissibility is reported per reading",
        split[OBSERVED_HYPOTHESIS_ID] != split["h_bar"],
        &format!("{:?}", split),
    );
    check11(
        &mut failures,
        "and the split is surfaced as a conflict (§4.10.5)",
        core.hypothesis_conflict(&per_h_bar, &bar_members, &[]),
        "",
    );
    check11(
        &mut failures,
        "a conflict is not reported for a single-member set",
        !core.hypothesis_conflict(
            &per_h_bar,
            &[h_obs.clone()],
            &["sink".to_string()],
        ),
        "",
    );

    let (chosen, selection) =
        core.select_conditional(&observed, &candidates, &members, &ctx, None, None, None, None);
    check11(
        &mut failures,
        "among robustly admissible candidates the greatest robust delta wins",
        chosen
            .as_ref()
            .map(|c| c.option_id == "strong")
            .unwrap_or(false),
        &format!("{:?}", chosen.as_ref().map(|c| c.option_id.clone())),
    );
    check11(
        &mut failures,
        "the payload lists the robust candidates",
        selection.robust_admissible.len() == 2,
        &format!("{:?}", selection.robust_admissible),
    );
    check11(
        &mut failures,
        "the payload lists the robust key of every option",
        selection.net_delta_robust.len() == 2,
        "",
    );

    // §4.10.4: no fallback to admissible support.
    let (chosen_none, none_selection) = core.select_conditional(
        &observed,
        &[sink.clone()],
        &bar_members,
        &ctx,
        None,
        None,
        None,
        None,
    );
    check11(
        &mut failures,
        "an empty robust candidate set yields no action at all",
        chosen_none.is_none(),
        &format!("{:?}", chosen_none.as_ref().map(|c| c.option_id.clone())),
    );
    check11(
        &mut failures,
        "…and the payload still reports what it refused",
        none_selection.robust_admissible.is_empty() && none_selection.hypothesis_conflict,
        "",
    );

    // §4.5 key 3 under its robust reading: an option that closes nothing under the
    // observed reading but closes something under another reading must NOT collect
    // the preference — key 2 has already charged that closure at its worst.
    let mut closer = f11::v011_option("closer", &entity, 0.02);
    closer.projected_dof_delta.clear();
    closer.projected_by_hypothesis = HashMap::from([
        (
            OBSERVED_HYPOTHESIS_ID.to_string(),
            HashMap::from([(entity.clone(), 0.02)]),
        ),
        (
            "h_alt".to_string(),
            HashMap::from([(entity.clone(), 0.02)]),
        ),
    ]);
    closer.closed_by_hypothesis = HashMap::from([
        (OBSERVED_HYPOTHESIS_ID.to_string(), Vec::new()),
        (
            "h_alt".to_string(),
            vec![ClosedRef {
                kind: "mean".to_string(),
                id: "radio".to_string(),
            }],
        ),
    ]);
    check11(
        &mut failures,
        "a per-hypothesis closure is read per reading",
        closer.is_reversible_for(OBSERVED_HYPOTHESIS_ID) && !closer.is_reversible_for("h_alt"),
        "",
    );
    check11(
        &mut failures,
        "…so the observed reading alone would call it reversible",
        core.is_reversible(&closer),
        "",
    );
    check11(
        &mut failures,
        "…but the robust reading does not (§4.5 key 3, §4.10.2)",
        !core.robust_reversible(&closer, &members),
        "",
    );
    check11(
        &mut failures,
        "…and the decision prefers the one that closes nothing under every reading",
        selects_reversible(core, &observed, &ctx, &members),
        "",
    );
    check11(
        &mut failures,
        "with no hypothesis set the robust reading is the flat one (§4.5)",
        core.robust_reversible(&closer, &[h_obs.clone()]),
        "",
    );

    // §4.4 guards range over every reading's closure list.
    let mut guarded = f11::v011_option("guarded", &entity, 0.01);
    guarded.act_id = "act_x".to_string();
    guarded.closed_by_hypothesis = HashMap::from([
        (OBSERVED_HYPOTHESIS_ID.to_string(), Vec::new()),
        (
            "h_alt".to_string(),
            vec![ClosedRef {
                kind: "act".to_string(),
                id: "act_x".to_string(),
            }],
        ),
    ]);
    check11(
        &mut failures,
        "the self-closure guard (§4.4 guard 1) catches a per-hypothesis self-closure",
        core.closure_error(&guarded).is_some(),
        "",
    );
    println!();

    // --- 6. §6.3: the after-state of a reading is its own -------------------
    println!("=== 6. §6.3: a reading's after-state is its own ===");
    // `D2`/`D3` are recomputed on **that reading's** after-state — the pruned graph
    // **and** the counters. An option that closes only under `h_alt` must be charged
    // there. A port that prunes the graph with `closure[h]` but recomputes the
    // counters from `closure[$observed$]` builds a state no hypothesis produces, and
    // this fixture separates the two: trainee's own mean drives its counter to zero
    // while the supervise mean removes the path that would raise it back.
    let closer_h = f11::v011_h_only_closer();
    let rows_h = core.lost_paths_for(&state, &closer_h, Some(&ctx), "h_alt");
    check11(
        &mut failures,
        "§6.3: a reading's after-state is built from that reading's own closures",
        rows_h.len() == 1
            && rows_h[0].entity_id == "trainee"
            && rows_h[0].verdict_before == "reachable"
            && rows_h[0].verdict_after == "proven_unreachable",
        &format!("{} rows", rows_h.len()),
    );
    check11(
        &mut failures,
        "…and the observed reading of the same option charges nothing",
        core.lost_paths(&state, &closer_h, Some(&ctx)).is_empty(),
        "",
    );

    // The form is a declaration style, not a semantics: under one reading, two
    // options declaring the same closure list — one flat, one per-hypothesis — must
    // produce the same lost paths. This is the invariance the mixed triple breaks.
    let mut flat_closer = f11::v011_h_only_closer();
    flat_closer.option_id = "flat_closer".to_string();
    flat_closer.closed = crate::options_v07::closures(&[
        crate::fixture_v07::TRAINEE_MEAN,
        crate::fixture_v07::SUPERVISE_MEAN,
    ]);
    flat_closer.closed_by_hypothesis.clear();
    let rows_flat = core.lost_paths_for(&state, &flat_closer, Some(&ctx), "h_alt");
    let rows_per_h = core.lost_paths_for(&state, &f11::v011_h_only_closer(), Some(&ctx), "h_alt");
    check11(
        &mut failures,
        "§3.3/§6.3: the per-hypothesis form agrees with the flat form on the same list",
        same_lost_rows(&rows_flat, &rows_per_h),
        &format!("{} vs {} rows", rows_flat.len(), rows_per_h.len()),
    );
    println!();

    // --- 7. §6.1/§6.2/§6.3: the report under a declared set -----------------
    println!("=== 7. §6.1/§6.2/§6.3: the audit report is per hypothesis ===");
    // The layer a port can compute but cannot publish is not landed: the conditional
    // vectors existed in this port's core and in this harness, and nothing a
    // consumer could call returned them. The report is the release's output, so the
    // layer is asserted here on the **report** and not on the core.
    //
    // The fixture is section 5's: the entity is read well above the zero under the
    // observed reading and just above it under `h_bar`, so draining it is barred
    // under one reading and admissible under the other — the asymmetry §6.3 exists
    // to report rather than average away.
    let report_candidates = vec![strong.clone(), sink.clone()];
    let (report_selected, report_selection) = core.select_conditional(
        &observed,
        &report_candidates,
        &bar_members,
        &ctx,
        None,
        None,
        None,
        None,
    );
    let report = core.report(
        &observed,
        &report_candidates,
        &report_selected,
        "FAST_PASS",
        crate::dof_core::ReportInput {
            ctx: Some(&ctx),
            readings: bar_members.clone(),
            declared: bar_members.clone(),
            coverage: "partial".to_string(),
            selection: Some(&report_selection),
            ..crate::dof_core::ReportInput::default()
        },
    );

    check11(
        &mut failures,
        "§6.2: the report names the declared set and the plausible readings",
        report.hypotheses.len() == 2 && report.plausible_hypotheses.len() == 2,
        &format!(
            "{} declared, {} plausible",
            report.hypotheses.len(),
            report.plausible_hypotheses.len()
        ),
    );
    check11(
        &mut failures,
        "§6.2: an undeclared coverage is reported as `partial`",
        report.hypothesis_coverage == "partial",
        &report.hypothesis_coverage,
    );
    check11(
        &mut failures,
        "§6.2: the index under a set is a map, one entry per reading",
        report.total_system_dof_by_hypothesis.len() == 2
            && report.total_system_dof_by_hypothesis[OBSERVED_HYPOTHESIS_ID]
                != report.total_system_dof_by_hypothesis["h_bar"],
        &format!("{:?}", report.total_system_dof_by_hypothesis),
    );

    // Every candidate carries its whole vector per reading, and the vectors are
    // compared against the core's own per-reading computation — a report that
    // drifted from the decision it publishes is caught here and nowhere else.
    let mut vec_ok = true;
    let mut vec_detail = String::new();
    for row in report.options.iter() {
        if row.conditional_vectors.len() != bar_members.len() {
            vec_ok = false;
            vec_detail = format!("{}: {} readings", row.option_id, row.conditional_vectors.len());
            break;
        }
        let option = report_candidates
            .iter()
            .find(|o| o.option_id == row.option_id)
            .cloned()
            .unwrap();
        for h in bar_members.iter() {
            let direct = core.conditional_vector_of(
                &h.state,
                &option,
                &ctx,
                &h.id,
                None,
                None,
                None,
                None,
            );
            let got = &row.conditional_vectors[&h.id];
            if got.d1 != direct.d1
                || got.d2 != direct.d2
                || got.d3 != direct.d3
                || (got.net_delta - direct.net_delta).abs() > DofCalculusCore::NET_DELTA_TOLERANCE
            {
                vec_ok = false;
                vec_detail = format!("{} under {}", row.option_id, h.id);
            }
        }
    }
    check11(
        &mut failures,
        "§6.3: every candidate carries its conditional vector per reading",
        vec_ok,
        &vec_detail,
    );

    // The barring condition must be visible **under the reading that barred it**:
    // a candidate that fails a condition of admissibility in one reading is barred
    // there, and that is what §4.10.5's flag names.
    let bar_row = report.options.iter().find(|r| r.option_id == "sink");
    check11(
        &mut failures,
        "§6.3: a candidate barred under one reading is visible as barred there",
        bar_row.map(|r| r.admissible_under.get("h_bar") == Some(&false)).unwrap_or(false)
            && bar_row.map(|r| r.conditional_vectors.len() == 2).unwrap_or(false),
        &format!("{:?}", bar_row.map(|r| r.admissible_under.clone())),
    );
    check11(
        &mut failures,
        "§6.3: the key that barred it is reported per reading",
        bar_row
            .map(|r| r.barring_key_by_hypothesis.get("h_bar").cloned().flatten().is_some())
            .unwrap_or(false),
        &format!("{:?}", bar_row.map(|r| r.barring_key_by_hypothesis.clone())),
    );
    check11(
        &mut failures,
        "§4.10.5: the split admissibility is surfaced as a conflict",
        report.hypothesis_conflict,
        &format!("{}", report.hypothesis_conflict),
    );

    // §6.1: the reason a value is what it is belongs to a reading too — there is no
    // shared `ψ` to print once the readings differ.
    let ent_row = report.entities.iter().find(|r| r.entity_id == entity);
    check11(
        &mut failures,
        "§6.1: the lens terms and the verdict are reported per reading",
        ent_row
            .map(|r| r.lens_terms_by_hypothesis.len() == 2)
            .unwrap_or(false)
            && ent_row
                .map(|r| r.recoverability_by_hypothesis.len() == 2)
                .unwrap_or(false),
        &format!(
            "{:?}",
            ent_row.map(|r| (
                r.lens_terms_by_hypothesis.len(),
                r.recoverability_by_hypothesis.len()
            ))
        ),
    );

    // §4.10.6: with no declared set the observed state alone is the answer, and the
    // per-reading surface is **absent** rather than a one-entry map.
    let flat_report = core.report(
        &observed,
        &report_candidates,
        &report_selected,
        "FAST_PASS",
        crate::dof_core::ReportInput {
            ctx: Some(&ctx),
            ..crate::dof_core::ReportInput::default()
        },
    );
    check11(
        &mut failures,
        "§4.10.6: without a declared set the flat report carries no per-reading surface",
        flat_report.hypotheses.is_empty()
            && flat_report.total_system_dof_by_hypothesis.is_empty()
            && flat_report.options[0].conditional_vectors.is_empty(),
        &format!(
            "{} hypotheses, {} totals",
            flat_report.hypotheses.len(),
            flat_report.total_system_dof_by_hypothesis.len()
        ),
    );
    println!();

    failures
}

/// Compares two lost-path reports on the facts that carry meaning: the entity and
/// the two verdicts. Order is the fixture's sorted iteration order in every port, so
/// it is compared positionally.
fn same_lost_rows(a: &[crate::dof_core::LostPathEntry], b: &[crate::dof_core::LostPathEntry]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (x, y) in a.iter().zip(b.iter()) {
        if x.entity_id != y.entity_id
            || x.verdict_before != y.verdict_before
            || x.verdict_after != y.verdict_after
        {
            return false;
        }
    }
    true
}

/// Builds the §4.10.2 counterexample the hard way: two options with **identical**
/// robust deltas, one of which declares a closure under the second reading only.
/// The closure names a mean that exists nowhere in the graph, so it costs nothing
/// in key 2 — no charge, no lost path — and the decision is made by key 3 alone.
///
/// The ids are chosen so the check discriminates: the closing option sorts FIRST.
/// Reading key 3 from the observed reading alone would call it reversible, keep it
/// as a survivor and elect it lexicographically; the robust reading drops it.
fn selects_reversible(
    core: &DofCalculusCore,
    state: &SystemStateMatrix,
    ctx: &ObservationContext,
    members: &[Hypothesis],
) -> bool {
    let entity = f11::v011_entity(state, 0);
    let mut closing = f11::v011_option("a_closing", &entity, 0.5);
    closing.projected_dof_delta.clear();
    closing.projected_by_hypothesis = HashMap::from([
        (
            OBSERVED_HYPOTHESIS_ID.to_string(),
            HashMap::from([(entity.clone(), 0.5)]),
        ),
        (
            "h_alt".to_string(),
            HashMap::from([(entity.clone(), 0.5)]),
        ),
    ]);
    closing.closed_by_hypothesis = HashMap::from([
        (OBSERVED_HYPOTHESIS_ID.to_string(), Vec::new()),
        (
            "h_alt".to_string(),
            vec![ClosedRef {
                kind: "mean".to_string(),
                id: "v011_absent_mean".to_string(),
            }],
        ),
    ]);
    let open = f11::v011_option("b_open", &entity, 0.5);
    let (chosen, selection) = core.select_conditional(
        state,
        &[closing, open],
        members,
        ctx,
        None,
        None,
        None,
        None,
    );
    if (selection.net_delta_robust["a_closing"] - selection.net_delta_robust["b_open"]).abs()
        > DofCalculusCore::NET_DELTA_TOLERANCE
    {
        return false; // key 2 separated them: this is not the key-3 counterexample
    }
    chosen
        .as_ref()
        .map(|c| c.option_id == "b_open")
        .unwrap_or(false)
}