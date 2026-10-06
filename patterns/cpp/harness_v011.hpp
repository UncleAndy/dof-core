// Conformance harness of the C++ port, DOF-SPEC v0.11 (§3.3, §3.4.2, §3.6, §4.4,
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

#pragma once

#include <cmath>
#include <iostream>
#include <map>
#include <optional>
#include <string>
#include <unordered_map>
#include <vector>

#include "conditional.hpp"
#include "fixture_v011.hpp"
#include "harness_v07.hpp"  // check7, near, num7, the frozen digests
#include "hypothesis.hpp"

namespace {

// The mark is `"  OK   "` — the same three spaces the other ports print, so that
// the cross-port verifier's row greps (`^  OK   the ruler is shared…`) read this
// port's evidence exactly as they read the others'.
void check11(std::vector<std::string>& failures, const std::string& name, bool ok,
             const std::string& detail = "") {
    std::cout << (ok ? "  OK   " : "  FAIL  ") << name;
    if (!detail.empty()) std::cout << "  " << detail;
    std::cout << "\n";
    if (!ok) failures.push_back(name);
}

bool has_substring(const std::vector<std::string>& errs, const std::string& needle) {
    for (const auto& e : errs) {
        if (e.find(needle) != std::string::npos) return true;
    }
    return false;
}

// Compares two lost-path reports on the facts that carry meaning: the entity and the
// two verdicts. Order is the fixture's sorted iteration order in every port, so it is
// compared positionally.
bool same_lost_rows(const std::vector<LostPathEntry>& a, const std::vector<LostPathEntry>& b) {
    if (a.size() != b.size()) return false;
    for (std::size_t i = 0; i < a.size(); ++i) {
        if (a[i].entity_id != b[i].entity_id || a[i].verdict_before != b[i].verdict_before ||
            a[i].verdict_after != b[i].verdict_after) {
            return false;
        }
    }
    return true;
}

std::string num11(double x) { return num7(x, 6); }

dof::Hypothesis hyp(const std::string& id, bool plausible, const SystemStateMatrix& state,
                    const std::string& basis) {
    dof::Hypothesis h;
    h.id = id;
    h.plausible = plausible;
    h.state = state;
    h.collapse_source_candidates.clear();
    h.basis = basis;
    return h;
}

using dof::ConditionalVector;

const std::map<std::string, ConditionalVector>& per_h_of(
    const std::map<std::string, std::map<std::string, ConditionalVector>>& all,
    const std::string& option_id) {
    static const std::map<std::string, ConditionalVector> kEmpty;
    auto it = all.find(option_id);
    return (it == all.end()) ? kEmpty : it->second;
}

const ConditionalVector& vector_of(const std::map<std::string, ConditionalVector>& per_h,
                                   const std::string& hypothesis_id) {
    static const ConditionalVector kEmpty;
    auto it = per_h.find(hypothesis_id);
    return (it == per_h.end()) ? kEmpty : it->second;
}

double robust_key(const dof::ConditionalSelection& selection, const std::string& option_id) {
    auto it = selection.net_delta_robust.find(option_id);
    return (it == selection.net_delta_robust.end()) ? 0.0 : it->second;
}

// Builds the §4.10.2 counterexample the hard way: two options with **identical**
// robust deltas, one of which declares a closure under the second reading only.
// The closure names a mean that exists nowhere in the graph, so it costs nothing in
// key 2 — no charge, no lost path — and the decision is made by key 3 alone.
//
// The ids are chosen so the check discriminates: the closing option sorts FIRST.
// Reading key 3 from the observed reading alone would call it reversible, keep it
// as a survivor and elect it lexicographically; the robust reading drops it.
bool selects_reversible(const DOFCalculusCore& core, const SystemStateMatrix& state,
                        const ObservationContext& ctx,
                        const std::vector<dof::Hypothesis>& members) {
    const std::string entity = dof::v011_entity(state, 0);
    ActionOption closing = dof::v011_option("a_closing", entity, 0.5);
    closing.projected_dof_delta.clear();
    closing.projected_by_hypothesis = {
        {kObservedHypothesisId, {{entity, 0.5}}},
        {"h_alt", {{entity, 0.5}}},
    };
    dof::ClosedRef absent;
    absent.kind = "mean";
    absent.id = "v011_absent_mean";
    closing.closed_by_hypothesis = {
        {kObservedHypothesisId, {}},
        {"h_alt", {absent}},
    };
    const ActionOption open = dof::v011_option("b_open", entity, 0.5);
    auto result = dof::select_conditional(core, state, {closing, open}, members, &ctx);
    const dof::ConditionalSelection& selection = result.second;
    if (std::fabs(robust_key(selection, "a_closing") - robust_key(selection, "b_open")) >
        DOFCalculusCore::net_delta_tolerance) {
        return false;  // key 2 separated them: this is not the key-3 counterexample
    }
    return result.first.has_value() && result.first->option_id == "b_open";
}

}  // namespace

inline int run_harness_v011() {
    using namespace fixture_v07;
    using namespace options_v07;
    using dof::Hypothesis;
    using dof::HypothesisSet;
    using dof::validate_set;
    using dof::resolved_members;
    using dof::coverage_of;
    using dof::hypothesis_coverage;

    std::vector<std::string> failures;

    // --- 0. the released evidence still stands ------------------------------
    // The released fixture's own fingerprints, asserted on the released fixture,
    // exactly as `harness_v08` asserts them: the T1 scene is a different scene and
    // its digests are legitimately its own.
    DOFOrchestrator orch_rel(0.05);
    orch_rel.measure(scene());
    const dof::MeasurementDeclaration& rel_decl = *orch_rel.mapper_ref().last_declaration;
    const ObservationContext& rel_ctx = *orch_rel.mapper_ref().last_observation;
    {
        const std::string d = rel_decl.digest();
        check11(failures, "the v0.7 declaration digest is still reproduced",
                d == kExpectedRulerDigest, d.substr(0, 16));
    }
    {
        const std::string o = rel_ctx.observation_digest;
        check11(failures, "the released fixture still carries the v0.7 observation digest",
                o == kExpectedObservationDigest, o.substr(0, 16));
    }
    {
        const std::string r = rel_decl.ruler_digest();
        check11(failures,
                "the v0.7 ruler digest is still reproduced (the exclusion rule did not move)",
                r == kExpectedRulerDigestV011, r.substr(0, 16));
    }

    DOFOrchestrator orch_obs(0.05);
    const SystemStateMatrix state = orch_obs.measure(t1_scene());
    const ObservationContext& ctx = *orch_obs.mapper_ref().last_observation;
    const dof::MeasurementDeclaration& decl = *orch_obs.mapper_ref().last_declaration;
    const DOFCalculusCore& core = orch_obs.core_ref();

    std::cout << "=== 0. §3.4.2/§4.10: one ruler, two readings ===\n";

    // The second reading is a **transformation of one observed state**, mapped
    // through its own orchestrator so that its declaration, its ruler and its graph
    // come from the same named procedures as the observed one.
    DOFOrchestrator orch_alt(0.05);
    const SystemStateMatrix state_b =
        dof::v011_reconcile(orch_alt.measure(dof::v011_scaled_scene(t1_scene(), 1.25)));
    const ObservationContext& ctx_b = *orch_alt.mapper_ref().last_observation;
    const dof::MeasurementDeclaration& decl_b = *orch_alt.mapper_ref().last_declaration;

    check11(failures, "the ruler is shared across the release: v0.11 ruler == v0.7 ruler",
            decl.ruler_digest() == rel_decl.ruler_digest(),
            decl.ruler_digest().substr(0, 16) + " vs " + rel_decl.ruler_digest().substr(0, 16));
    check11(failures, "two readings of one cycle share the ruler digest byte for byte",
            decl.ruler_digest() == decl_b.ruler_digest(),
            decl.ruler_digest().substr(0, 16) + " vs " + decl_b.ruler_digest().substr(0, 16));
    check11(failures, "…while their full declaration digests differ",
            decl.digest() != decl_b.digest(),
            decl.digest().substr(0, 16) + " vs " + decl_b.digest().substr(0, 16));
    check11(failures, "…and the observation digest is **shared** — §3.5's `G` is not branched",
            ctx.observation_digest == ctx_b.observation_digest,
            ctx.observation_digest.substr(0, 16) + " vs " + ctx_b.observation_digest.substr(0, 16));

    const Hypothesis h_obs = hyp(kObservedHypothesisId, true, state, "the observed reading");
    const Hypothesis h_alt =
        hyp("h_alt", true, state_b, "declared Variety counter scaled by 1.25");
    std::cout << "\n";

    // --- 1. §3.6: the hypothesis set and its validation rules ---------------
    std::cout << "=== 1. §3.6: five validation rules, each on its own defect ===\n";
    const std::vector<std::string> well_formed = validate_set(state, {h_obs, h_alt});
    {
        std::string detail;
        for (const auto& e : well_formed) detail += e + " ";
        check11(failures, "a well-formed two-member set is admissible", well_formed.empty(),
                detail);
    }

    check11(failures, "duplicate ids are refused",
            has_substring(validate_set(state, {h_obs, hyp(kObservedHypothesisId, true,
                                                          state_b, "")}),
                          "ids are not unique"));

    const std::string entity = dof::v011_entity(state, 0);
    const SystemStateMatrix omitted = dof::v011_without_entity(state_b, entity);
    check11(failures, "a reading that omits an entity of S is refused",
            has_substring(validate_set(state, {h_obs, hyp("h_omit", true, omitted, "")}),
                          "omits entities"));

    const SystemStateMatrix extra = dof::v011_with_extra_entity(state_b);
    check11(failures, "a reading that declares an entity not in S is refused",
            has_substring(validate_set(state, {h_obs, hyp("h_extra", true, extra, "")}),
                          "declares entities not in the observed state"));

    const SystemStateMatrix broken = dof::v011_break_lens_product(state_b, entity);
    check11(failures, "a stated DoF that its own counters do not produce is refused",
            has_substring(validate_set(state, {h_obs, hyp("h_broken", true, broken, "")}),
                          "differs from the product of its own lens values"));

    const SystemStateMatrix moved = dof::v011_move_collapse_label(state_b, entity);
    check11(failures, "moving the collapse-source label between readings is refused",
            has_substring(validate_set(state, {h_obs, hyp("h_moved", true, moved, "")}),
                          "is_collapse_source differs"));

    const SystemStateMatrix durations = dof::v011_change_durations(state_b);
    check11(failures, "measurement durations are ruler-level and may not vary per reading",
            has_substring(validate_set(state, {h_obs, hyp("h_dur", true, durations, "")}),
                          "durations are ruler-level"));

    check11(failures, "the observed state must be present in H",
            has_substring(validate_set(state, {h_alt}), "the observed state is absent"));

    check11(failures, "a set that marks every reading implausible is refused",
            has_substring(validate_set(state,
                                       {hyp(kObservedHypothesisId, false, state, ""),
                                        hyp("h_alt", false, state_b, "")}),
                          "H_plausible would be empty"));
    std::cout << "\n";

    // --- 2. §3.6/§4.10.6: absence and emptiness -----------------------------
    std::cout << "=== 2. §3.6: absence and emptiness are the observed singleton ===\n";
    const std::vector<Hypothesis> singleton = resolved_members(state, nullptr);
    check11(failures, "an absent set resolves to exactly one member", singleton.size() == 1);
    check11(failures, "…whose id is the observed one",
            !singleton.empty() && singleton[0].id == kObservedHypothesisId);
    check11(failures, "…and which is plausible", !singleton.empty() && singleton[0].plausible);
    HypothesisSet empty_set;
    check11(failures, "an empty set resolves the same way",
            resolved_members(state, &empty_set).size() == 1);
    check11(failures, "an absent coverage claim reads as `partial`, never `complete`",
            coverage_of(nullptr) == "partial" && coverage_of(&empty_set) == "partial");
    HypothesisSet complete_set;
    complete_set.coverage = "complete";
    check11(failures, "a declared coverage claim is reported as declared",
            hypothesis_coverage(&complete_set) == "complete");
    std::cout << "\n";

    // --- 3. §3.3/§4.4: the two forms, and no mixing ------------------------
    std::cout << "=== 3. §3.3/§4.4: the two forms of an option ===\n";
    const ActionOption flat = dof::v011_option("flat_opt", entity, 0.01);
    check11(failures, "a single map of deltas is the flat form",
            projection_form(flat) == "flat");
    check11(failures, "an empty closure list is the flat form",
            closure_form(flat) == "flat");
    check11(failures, "a flat option is well formed", forms_consistent(flat).empty());

    ActionOption per_h = dof::v011_option("per_h_opt", entity, 0.01);
    per_h.projected_dof_delta.clear();
    per_h.projected_by_hypothesis = {
        {kObservedHypothesisId, {{entity, 0.01}}},
        {"h_alt", {{entity, 0.02}}},
    };
    check11(failures, "a map keyed by reading is the per-hypothesis form",
            projection_form(per_h) == "per_hypothesis");
    check11(failures, "…and the delta read under a reading is that reading's",
            delta_for(per_h, kObservedHypothesisId, entity) == 0.01 &&
                delta_for(per_h, "h_alt", entity) == 0.02);
    check11(failures, "…with entities unlisted for a reading taking 0.0",
            delta_for(per_h, "h_alt", "nobody") == 0.0);

    ActionOption mixed = dof::v011_option("mixed_opt", entity, 0.01);
    mixed.projected_by_hypothesis = {{kObservedHypothesisId, {{entity, 0.02}}}};
    check11(failures, "mixing both forms in the projection is `invalid`",
            projection_form(mixed) == "invalid");
    check11(failures, "…and the option says so", !forms_consistent(mixed).empty());

    ActionOption mixed_closure = dof::v011_option("mixed_closed_opt", entity, 0.01);
    dof::ClosedRef radio;
    radio.kind = "mean";
    radio.id = "radio";
    mixed_closure.closed = {radio};
    mixed_closure.closed_by_hypothesis = {{kObservedHypothesisId, {radio}}};
    check11(failures, "mixing both forms in the closure list is `invalid`",
            closure_form(mixed_closure) == "invalid");
    check11(failures, "…and the option says so too",
            !forms_consistent(mixed_closure).empty());
    std::cout << "\n";

    // --- 4. §4.8b: the temporal condition ----------------------------------
    std::cout << "=== 4. §4.8b: the temporal condition, as a condition ===\n";
    const std::optional<double> tau_opt = tau_of(state);
    check11(failures, "the released fixture carries a measured τ", tau_opt.has_value());
    const double tau = tau_opt.value_or(0.0);

    const ActionOption short_act = dof::v011_option("short_act", entity, 0.01);
    check11(failures, "an act within τ is viable", dof::viability(state, short_act).viable);

    ActionOption long_act = dof::v011_option("long_act", entity, 0.01);
    long_act.estimated_duration_mks = tau * 2.0;
    check11(failures, "an act that cannot complete within τ is not viable",
            !dof::viability(state, long_act).viable);

    const SystemStateMatrix no_tau = dof::v011_unknown_tau(state);
    check11(failures, "an unknown τ is not a licence for an ordinary act",
            !dof::viability(no_tau, short_act).viable);
    check11(failures, "…and the reason names the missing measurement (§4.8b)",
            dof::viability(no_tau, short_act).reason.find("unmeasured") != std::string::npos);

    ActionOption measure_tau = dof::v011_option("measure_tau", entity, 0.01);
    measure_tau.discovers = {"tau"};
    measure_tau.estimated_duration_mks = 1000.0;
    measure_tau.projected_tau_value = tau + 1.0e6;
    check11(failures, "a τ measurement is viable under an unknown τ when it can complete",
            dof::viability(no_tau, measure_tau).viable);
    check11(failures, "…and viable under a measured τ as well",
            dof::viability(state, measure_tau).viable);

    ActionOption dead_measure = dof::v011_option("dead_measure", entity, 0.01);
    dead_measure.discovers = {"tau"};
    dead_measure.estimated_duration_mks = 1000.0;
    dead_measure.projected_tau_value = -1.0;
    check11(failures, "a τ measurement whose own declared result is dead is not viable",
            !dof::viability(state, dead_measure).viable);
    check11(failures,
            "…and the retired disjunction `τ = null` OR `τ >= t_m` would have admitted it",
            !dof::viability(no_tau, dead_measure).viable);

    const std::optional<double> derived = dof::derived_tau_delta(state, measure_tau);
    const double expected =
        measure_tau.projected_tau_value.value_or(0.0) - (tau - measure_tau.estimated_duration_mks);
    check11(failures, "`projected_tau_delta` is derived for a τ measurement",
            derived.has_value() && std::fabs(*derived - expected) < 1e-6,
            (derived ? num11(*derived) : std::string("null")) + " vs " + num11(expected));
    check11(failures, "…and is null, not zero, when τ is unknown",
            !dof::derived_tau_delta(no_tau, measure_tau).has_value());
    std::cout << "\n";

    // --- 5. §4.10: the robust selection ------------------------------------
    std::cout << "=== 5. §4.10: robust selection over the declared readings ===\n";
    // The arithmetic needs room: `coerce_dof` clamps an entity's DoF into [0,1] and
    // the released fixture sits at the top of that interval, where a positive
    // projection cannot move anything. The observed reading of this section is the
    // same fixture with one entity's declared Variety counter scaled down — still a
    // state §4.1 accepts, since its stated DoF is the product of its own counters.
    const SystemStateMatrix observed = dof::v011_set_entity_dof(state, entity, 0.25);
    const SystemStateMatrix other = dof::v011_set_entity_dof(state_b, entity, 0.30);
    const std::vector<Hypothesis> members = {
        hyp(kObservedHypothesisId, true, observed, ""),
        hyp("h_alt", true, other, ""),
    };
    const ActionOption strong = dof::v011_option("strong", entity, 0.5);
    const ActionOption weak = dof::v011_option("weak", entity, 0.1);
    const std::vector<ActionOption> candidates = {strong, weak};

    {
        const std::vector<std::string> consistency = validate_set(observed, members);
        std::string detail;
        for (const auto& e : consistency) detail += e + " ";
        check11(failures, "the readings of this section are §4.1-consistent",
                consistency.empty(), detail);
    }

    const auto per_h_all = dof::conditional_vectors(core, members, candidates, &ctx);
    check11(failures, "the conditional vectors are produced per option and per reading",
            per_h_all.size() == 2 && per_h_of(per_h_all, "strong").size() == 2);
    const double strong_obs = vector_of(per_h_of(per_h_all, "strong"), kObservedHypothesisId)
                                  .net_delta;
    const double strong_alt = vector_of(per_h_of(per_h_all, "strong"), "h_alt").net_delta;
    check11(failures, "…and they are not all alike (the readings really differ)",
            strong_obs != strong_alt, num11(strong_obs) + " vs " + num11(strong_alt));

    const double least_strong = dof::least_favourable(per_h_of(per_h_all, "strong"), members);
    const double greatest_strong = std::max(strong_obs, strong_alt);
    check11(failures, "the ordering key is the least-favourable delta, not the greatest",
            least_strong == std::min(strong_obs, strong_alt) && least_strong < greatest_strong,
            "min=" + num11(least_strong) + " max=" + num11(greatest_strong));

    // A reading under which one candidate crosses into the zero: the entity sits
    // just above it, and the option's negative projection drives it in. Under the
    // observed reading the same option leaves the entity positive.
    const SystemStateMatrix lows = dof::v011_set_entity_dof(state_b, entity, 0.02);
    const std::vector<Hypothesis> bar_members = {
        h_obs,
        hyp("h_bar", true, lows, "one entity sits just above the zero"),
    };
    const ActionOption sink = dof::v011_option("sink", entity, -0.05);
    const auto per_h_bar = dof::conditional_vectors(core, bar_members, {sink}, &ctx);
    const std::map<std::string, ConditionalVector>& sink_vectors = per_h_of(per_h_bar, "sink");
    check11(failures,
            "an option that crosses into the zero under one reading is charged there (D1 > 0)",
            vector_of(sink_vectors, "h_bar").d1 > 0,
            "d1=" + std::to_string(vector_of(sink_vectors, "h_bar").d1) +
                " under the bar, d1=" +
                std::to_string(vector_of(sink_vectors, kObservedHypothesisId).d1) +
                " under the observed reading");
    check11(failures, "…and is not charged under the observed reading",
            vector_of(sink_vectors, kObservedHypothesisId).d1 == 0);
    check11(failures, "a candidate barred under one plausible reading is not robustly admissible",
            !dof::robust_admissible(sink_vectors, bar_members));

    {
        const std::map<std::string, bool> split = dof::admissible_under(sink_vectors, bar_members);
        std::string detail;
        for (const auto& kv : split) {
            detail += kv.first + "=" + (kv.second ? "1 " : "0 ");
        }
        auto io = split.find(kObservedHypothesisId);
        auto ia = split.find("h_bar");
        check11(failures, "the per-reading admissibility is reported per reading",
                io != split.end() && ia != split.end() && io->second != ia->second, detail);
    }
    check11(failures, "and the split is surfaced as a conflict (§4.10.5)",
            dof::hypothesis_conflict(per_h_bar, bar_members, {}));
    check11(failures, "a conflict is not reported for a single-member set",
            !dof::hypothesis_conflict(per_h_bar, {h_obs}, {"sink"}));

    const auto chosen = dof::select_conditional(core, observed, candidates, members, &ctx);
    check11(failures, "among robustly admissible candidates the greatest robust delta wins",
            chosen.first.has_value() && chosen.first->option_id == "strong",
            chosen.first ? chosen.first->option_id : std::string("none"));
    check11(failures, "the payload lists the robust candidates",
            chosen.second.robust_admissible.size() == 2,
            std::to_string(chosen.second.robust_admissible.size()));
    check11(failures, "the payload lists the robust key of every option",
            chosen.second.net_delta_robust.size() == 2);

    // §4.10.4: no fallback to admissible support.
    const auto chosen_none = dof::select_conditional(core, observed, {sink}, bar_members, &ctx);
    check11(failures, "an empty robust candidate set yields no action at all",
            !chosen_none.first.has_value(),
            chosen_none.first ? chosen_none.first->option_id : std::string("none"));
    check11(failures, "…and the payload still reports what it refused",
            chosen_none.second.robust_admissible.empty() &&
                chosen_none.second.hypothesis_conflict);

    // §4.5 key 3 under its robust reading: an option that closes nothing under the
    // observed reading but closes something under another reading must NOT collect
    // the preference — key 2 has already charged that closure at its worst.
    ActionOption closer = dof::v011_option("closer", entity, 0.02);
    closer.projected_dof_delta.clear();
    closer.projected_by_hypothesis = {
        {kObservedHypothesisId, {{entity, 0.02}}},
        {"h_alt", {{entity, 0.02}}},
    };
    closer.closed_by_hypothesis = {
        {kObservedHypothesisId, {}},
        {"h_alt", {radio}},
    };
    check11(failures, "a per-hypothesis closure is read per reading",
            is_reversible_for(closer, kObservedHypothesisId) &&
                !is_reversible_for(closer, "h_alt"));
    check11(failures, "…so the observed reading alone would call it reversible",
            is_reversible_for(closer, kObservedHypothesisId));
    check11(failures, "…but the robust reading does not (§4.5 key 3, §4.10.2)",
            !dof::robust_reversible(closer, members));
    check11(failures, "…and the decision prefers the one that closes nothing under every reading",
            selects_reversible(core, observed, ctx, members));
    check11(failures, "with no hypothesis set the robust reading is the flat one (§4.5)",
            dof::robust_reversible(closer, {h_obs}));

    // §4.4 guards range over every reading's closure list.
    ActionOption guarded = dof::v011_option("guarded", entity, 0.01);
    guarded.act_id = "act_x";
    dof::ClosedRef self_act;
    self_act.kind = "act";
    self_act.id = "act_x";
    guarded.closed_by_hypothesis = {
        {kObservedHypothesisId, {}},
        {"h_alt", {self_act}},
    };
    check11(failures, "the self-closure guard (§4.4 guard 1) catches a per-hypothesis self-closure",
            !core.closure_error(guarded).empty());
    std::cout << "\n";

    // --- 6. §6.3: the after-state of a reading is its own -------------------
    std::cout << "=== 6. §6.3: a reading's after-state is its own ===\n";
    // `D2`/`D3` are recomputed on **that reading's** after-state — the pruned graph
    // **and** the counters. An option that closes only under `h_alt` must be charged
    // there. A port that prunes the graph with `closure[h]` but recomputes the
    // counters from `closure[$observed$]` builds a state no hypothesis produces, and
    // this fixture separates the two: trainee's own mean drives its counter to zero
    // while the supervise mean removes the path that would raise it back.
    const ActionOption closer_h = dof::v011_h_only_closer();
    const std::vector<LostPathEntry> rows_h = core.lost_paths_for(state, closer_h, &ctx, "h_alt");
    check11(failures, "§6.3: a reading's after-state is built from that reading's own closures",
            rows_h.size() == 1 && rows_h[0].entity_id == "trainee" &&
                rows_h[0].verdict_before == "reachable" &&
                rows_h[0].verdict_after == "proven_unreachable",
            std::to_string(rows_h.size()) + " rows");
    check11(failures, "…and the observed reading of the same option charges nothing",
            core.lost_paths(state, closer_h, &ctx).empty());

    // The form is a declaration style, not a semantics: under one reading, two options
    // declaring the same closure list — one flat, one per-hypothesis — must produce
    // the same lost paths. This is the invariance the mixed triple breaks.
    ActionOption flat_closer = dof::v011_h_only_closer();
    flat_closer.option_id = "flat_closer";
    flat_closer.closed = options_v07::closures(
        {fixture_v07::kTraineeMean, fixture_v07::kSuperviseMean});
    flat_closer.closed_by_hypothesis.clear();
    const std::vector<LostPathEntry> rows_flat =
        core.lost_paths_for(state, flat_closer, &ctx, "h_alt");
    const std::vector<LostPathEntry> rows_per_h =
        core.lost_paths_for(state, dof::v011_h_only_closer(), &ctx, "h_alt");
    check11(failures,
            "§3.3/§6.3: the per-hypothesis form agrees with the flat form on the same list",
            same_lost_rows(rows_flat, rows_per_h),
            std::to_string(rows_flat.size()) + " vs " + std::to_string(rows_per_h.size()) +
                " rows");
    std::cout << "\n";

    // --- 7. §6.1/§6.2/§6.3: the report under a declared set -----------------
    std::cout << "=== 7. §6.1/§6.2/§6.3: the audit report is per hypothesis ===";
    std::cout << "\n";
    // A layer a port can compute but cannot publish is not landed: the conditional
    // vectors lived in this port's core and in this harness, and nothing a consumer
    // could call returned them. The report is the release's output, so the layer is
    // asserted here on the **report** and not on the core.
    //
    // The fixture: the entity is read well above the zero under the observed reading
    // and just above it under `h_bar`, so an option that drains it is barred under
    // one reading and admissible under the other — the asymmetry §6.3 exists to
    // report rather than average away.
    const std::vector<ActionOption> report_candidates = {strong, sink};

    ReportInput report_input;
    report_input.ctx = &ctx;
    const dof::ReportV011 report =
        dof::report_on_set(core, observed, report_candidates, "FAST_PASS", nullptr, report_input).second;

    // With no declared set the observed state alone is the answer (§4.10.6): one
    // reading, and the flat part of the report unchanged.
    const dof::ReportV011 flat_report =
        dof::report_on_set(core, observed, report_candidates, "FAST_PASS", nullptr, report_input).second;

    dof::HypothesisSet hset;
    hset.members = bar_members;
    const dof::ReportV011 set_report =
        dof::report_on_set(core, observed, report_candidates, "FAST_PASS", &hset, report_input).second;

    check11(failures, "§4.10.6: without a declared set the report has one observed reading",
            report.hypotheses.size() == 1 &&
                report.hypotheses.count(kObservedHypothesisId) == 1,
            std::to_string(report.hypotheses.size()) + " readings");
    // The overlay has no flat-without-readings type at all (the per-reading rows live in
    // `ReportV011`, not in `DofReport`), so the Go/Rust assertion "the flat report
    // carries no per-reading surface" is a type-level fact here. What can be asserted is
    // the positive form of the same thing: with no declared set every per-reading entry
    // belongs to the **one** observed reading, and not to a reading that was never
    // declared.
    bool singleton_only = !flat_report.conditional_vectors.empty();
    for (const auto& kv : flat_report.conditional_vectors) {
        if (kv.second.size() != 1 || kv.second.count(kObservedHypothesisId) != 1) {
            singleton_only = false;
            break;
        }
    }
    check11(failures, "…and every per-reading entry belongs to the observed reading",
            singleton_only,
            std::to_string(flat_report.conditional_vectors.size()) +
                " options, " + std::to_string(flat_report.plausible_hypotheses.size()) + " plausible");
    check11(failures, "§6.2: the scalar total names the reading it belongs to",
            set_report.total_system_dof_reading == std::string(kObservedHypothesisId),
            set_report.total_system_dof_reading);
    check11(failures, "§6.2: the report names the declared set and the plausible readings",
            set_report.hypotheses.size() == 2 && set_report.plausible_hypotheses.size() == 2,
            std::to_string(set_report.hypotheses.size()) + " declared, " +
                std::to_string(set_report.plausible_hypotheses.size()) + " plausible");
    check11(failures, "§6.2: an undeclared coverage is reported as `partial`",
            set_report.hypothesis_coverage == "partial", set_report.hypothesis_coverage);
    check11(failures, "§6.2: the index under a set is a map, one entry per reading",
            set_report.total_system_dof_by_hypothesis.size() == 2 &&
                set_report.total_system_dof_by_hypothesis.at(kObservedHypothesisId) !=
                    set_report.total_system_dof_by_hypothesis.at("h_bar"),
            num11(set_report.total_system_dof_by_hypothesis.at(kObservedHypothesisId)) + " vs " +
                num11(set_report.total_system_dof_by_hypothesis.at("h_bar")));

    // Every candidate carries its whole vector per reading, and the vectors are
    // compared against the core's own per-reading computation — a report that drifted
    // from the decision it publishes is caught here and nowhere else.
    bool vec_ok = !set_report.conditional_vectors.empty();
    std::string vec_detail;
    for (const auto& kv : set_report.conditional_vectors) {
        if (kv.second.size() != bar_members.size()) {
            vec_ok = false;
            vec_detail = kv.first + ": " + std::to_string(kv.second.size()) + " readings";
            break;
        }
        for (const auto& per_h : kv.second) {
            // The reading's **own** state, which is what the conditional pass reads
            // (§4.9): comparing against the observed state would accept a report
            // that had averaged the readings back together.
            const SystemStateMatrix* h_state = nullptr;
            for (const auto& h : bar_members) {
                if (h.id == per_h.first) h_state = &h.state;
            }
            if (h_state == nullptr) {
                vec_ok = false;
                vec_detail = kv.first + ": unknown reading " + per_h.first;
                continue;
            }
            const ActionOption& option = per_h.second.option_id == "sink" ? sink : strong;
            const ConditionalVector direct =
                dof::conditional_vector_of(core, *h_state, option, &ctx, per_h.first);
            if (per_h.second.d1 != direct.d1 || per_h.second.d2 != direct.d2 ||
                per_h.second.d3 != direct.d3 ||
                std::fabs(per_h.second.net_delta - direct.net_delta) > 1e-9) {
                vec_ok = false;
                vec_detail = kv.first + " under " + per_h.first;
            }
        }
    }
    check11(failures, "§6.3: every candidate carries its conditional vector per reading", vec_ok,
            vec_detail);

    // The barring condition must be visible **under the reading that barred it**.
    const auto sink_it = set_report.admissible_under.find("sink");
    check11(failures, "§6.3: a candidate barred under one reading is visible as barred there",
            sink_it != set_report.admissible_under.end() &&
                sink_it->second.at("h_bar") == false &&
                sink_it->second.at(kObservedHypothesisId) == true,
            sink_it != set_report.admissible_under.end()
                ? (std::string(sink_it->second.at("h_bar") ? "true" : "false") + " / " +
                   std::string(sink_it->second.at(kObservedHypothesisId) ? "true" : "false"))
                : std::string("absent"));
    const auto keys_it = set_report.barring_key_by_hypothesis.find("sink");
    check11(failures, "§6.3: the key that barred it is reported per reading",
            keys_it != set_report.barring_key_by_hypothesis.end() &&
                keys_it->second.at("h_bar").has_value() &&
                keys_it->second.at(kObservedHypothesisId).value_or("none") !=
                    keys_it->second.at("h_bar").value_or("none"),
            keys_it != set_report.barring_key_by_hypothesis.end()
                ? (keys_it->second.at(kObservedHypothesisId).value_or("none") + " / " +
                   keys_it->second.at("h_bar").value_or("none"))
                : std::string("absent"));
    check11(failures, "§4.10.5: the split admissibility is surfaced as a conflict",
            set_report.hypothesis_conflict, set_report.hypothesis_conflict ? "true" : "false");

    // §6.1: the reason a value is what it is belongs to a reading too — there is no
    // shared `ψ` to print once the readings differ.
    const auto lens_it = set_report.lens_terms_by_hypothesis.find(entity);
    check11(failures, "§6.1: the lens terms and the verdict are reported per reading",
            lens_it != set_report.lens_terms_by_hypothesis.end() &&
                lens_it->second.size() == 2 &&
                set_report.recoverability_by_hypothesis.count(entity) == 1 &&
                set_report.recoverability_by_hypothesis.at(entity).size() == 2,
            lens_it != set_report.lens_terms_by_hypothesis.end()
                ? std::to_string(lens_it->second.size()) + " readings"
                : std::string("absent"));

    // §6.2/§6.3: the layer must be reachable for a **consumer of the port**, not only
    // for this harness. The orchestrator's set entry publishes the per-reading report;
    // the same entry without a set publishes the observed singleton (§4.10.6), while
    // `step_with_report` keeps publishing the flat audit.
    dof::HypothesisSet members_set;
    members_set.members = members;
    const auto orch_choice = orch_obs.decide_on_set(observed, &members_set);
    const auto orch_flat = orch_obs.decide_on_set(observed, nullptr);
    bool orch_per_reading = !orch_choice.second.conditional_vectors.empty();
    for (const auto& kv : orch_choice.second.conditional_vectors) {
        if (kv.second.size() != 2) {
            orch_per_reading = false;
            break;
        }
    }
    check11(failures, "§6.2/§6.3: the orchestrator publishes the per-reading report for a set",
            orch_choice.second.hypotheses.size() == 2 && orch_per_reading &&
                orch_choice.second.total_system_dof_reading == kObservedHypothesisId &&
                orch_choice.second.hypothesis_coverage == "partial",
            std::to_string(orch_choice.second.hypotheses.size()) + " readings, " +
                std::to_string(orch_choice.second.conditional_vectors.size()) + " options");
    check11(failures, "§4.10.6: the set entry without a set publishes the observed singleton",
            orch_flat.second.hypotheses.size() == 1 &&
                orch_flat.second.hypotheses.count(kObservedHypothesisId) == 1 &&
                orch_flat.second.plausible_hypotheses.size() == 1 &&
                orch_flat.second.total_system_dof_by_hypothesis.size() == 1,
            std::to_string(orch_flat.second.hypotheses.size()) + " readings, " +
                std::to_string(orch_flat.second.plausible_hypotheses.size()) + " plausible");
    // §4.10: the published decision is the one the report's own robust support licenses
    // — a report describing a decision other than the one made is the defect the single
    // conditional pass exists to prevent.
    const auto& robust = orch_choice.second.robust_admissible;
    const bool decided_ok =
        orch_choice.first.has_value()
            ? std::find(robust.begin(), robust.end(), orch_choice.first->option_id) != robust.end()
            : robust.empty();
    check11(failures, "§4.10: the published decision is robustly admissible in its own report",
            decided_ok,
            std::to_string(robust.size()) + " robust");
    // §3.6: a set whose observed reading is not the state it is evaluated on is
    // **refused**, not repaired — the entry point is where that obligation lives.
    bool refused = false;
    std::string refusal;
    try {
        dof::HypothesisSet inconsistent;
        inconsistent.members = bar_members;
        orch_obs.decide_on_set(observed, &inconsistent);
    } catch (const std::invalid_argument& e) {
        refused = true;
        refusal = e.what();
    }
    check11(failures, "§3.6: the entry point refuses a set whose observed reading is absent",
            refused, refusal);
    // §6.2/§6.3: the set-aware entry is reachable from a **bare scene** as well as from a
    // measured state — this is the call a v0.11 consumer makes, and without a set it must
    // publish the observed singleton rather than the empty surface the flat entry gives.
    // A layer only the harness can reach has not been landed.
    DOFOrchestrator orch_entry(0.05);
    const auto entry = orch_entry.step_with_report_on_set(t1_scene(), nullptr);
    check11(failures, "§4.10.6: the set entry on a bare scene publishes the observed singleton",
            entry.second.hypotheses.size() == 1 &&
                entry.second.hypotheses.count(kObservedHypothesisId) == 1 &&
                entry.second.plausible_hypotheses.size() == 1 &&
                entry.second.total_system_dof_by_hypothesis.size() == 1 &&
                entry.second.hypothesis_coverage == "partial" &&
                !entry.second.conditional_vectors.empty(),
            std::to_string(entry.second.hypotheses.size()) + " readings, " +
                std::to_string(entry.second.conditional_vectors.size()) + " options");
    // §4.10.6: a cycle with **no observation context** still decides and reports. A
    // scene without a graph is decidable — the §4.9 verdicts read `undetermined` and
    // are priced by `u(t)` — so an absent context changes what the quantities are,
    // never whether the calculus answers; and the refusal the entry publishes must be
    // the arithmetic's refusal: the robustly admissible set it reports must equal the
    // set its **own published vectors** license. A fresh orchestrator has no context
    // until it polls, which is exactly the case this asserts.
    DOFOrchestrator orch_noc(0.05);
    const auto noc = orch_noc.decide_on_set(observed, nullptr);
    std::vector<std::string> licensed;
    bool noc_surface = !noc.second.base.options.empty() &&
                       !noc.second.conditional_vectors.empty();
    for (const auto& kv : noc.second.conditional_vectors) {
        const auto it = kv.second.find(kObservedHypothesisId);
        if (it == kv.second.end()) {
            noc_surface = false;
            continue;
        }
        if (!it->second.barred()) licensed.push_back(kv.first);
    }
    std::sort(licensed.begin(), licensed.end());
    std::vector<std::string> published = noc.second.robust_admissible;
    std::sort(published.begin(), published.end());
    // And the decision must be the arithmetic's decision too: a cycle that publishes
    // `[fallback_0]` as robustly admissible and then reports `none` has refused where
    // its own numbers license action.
    const bool neg_ok =
        noc.first.has_value() == !licensed.empty() &&
        (!noc.first.has_value() ||
         std::find(licensed.begin(), licensed.end(), noc.first->option_id) != licensed.end());
    check11(failures, "§4.10.6: a cycle with no observation context still decides and reports",
            noc_surface && published == licensed && neg_ok,
            std::string("surface=") + (noc_surface ? "yes" : "no") + ", published " +
                std::to_string(published.size()) + " vs licensed " +
                std::to_string(licensed.size()) + ", decided=" +
                (noc.first.has_value() ? "yes" : "no"));
    std::cout << "\n";

    return static_cast<int>(failures.size());
}
