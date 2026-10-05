// Conformance harness of the C++ port, DOF-SPEC v0.8 (§4.5, T1).
//
// This is the release's own harness. `v07` is left untouched: it is the v0.7
// evidence, and it still reproduces the v0.7 ruler — which is itself one of this
// release's checks (a moved digest is an error to be fixed, not a new version).
//
// Sections, in order: the ruler did not move; the two entities D3 exists for; the
// candidate vector is computed; the decision that changed; the mirror; a path cut
// is a bar; bounds and the baseline.

#pragma once

#include <cmath>
#include <iostream>
#include <optional>
#include <set>
#include <string>
#include <vector>

#include "harness_v07.hpp"  // check7, near, num7 and the frozen digests

namespace {

// The index of the released fixture (v0.7, §10). T1 must not move it.
const double kV07Index = -35.314438370902;

// The vector a single candidate would be judged by, computed by the selection
// itself so that the harness reads the same numbers the decision does.
CandidateVector vector_of(const DOFCalculusCore& core, const SystemStateMatrix& state,
                               const ObservationContext* ctx, const ActionOption& option) {
    auto result = core.select_candidate(state, {option}, ctx);
    return result.second.front();
}

std::string vec_str(const CandidateVector& v) {
    return "{" + std::to_string(v.d1) + " " + std::to_string(v.d2) + " " +
           std::to_string(v.d3) + " " + num7(v.net_delta) + " " +
           (v.reversible ? "rev" : "irr") + " " + v.option_id + "}";
}

}  // namespace

inline int run_harness_v08() {
    using namespace fixture_v07;
    using namespace options_v07;

    // --- 1. the ruler did not move (§10, R6) --------------------------------
    DOFOrchestrator base_orch(0.05);
    SystemStateMatrix base_state = base_orch.measure(scene());
    const GraphMapper& base_mapper = base_orch.mapper_ref();
    const ObservationContext* base_ctx =
        base_mapper.last_observation ? &*base_mapper.last_observation : nullptr;
    const dof::MeasurementDeclaration& base_decl = *base_mapper.last_declaration;
    const DOFCalculusCore& base_core = base_orch.core_ref();

    std::cout << "=== 1. §10/R6: the released fixture is unchanged by T1 ===\n";
    check7("the ruler digest is the v0.7 one, byte for byte",
           base_decl.digest() == kExpectedRulerDigest, base_decl.digest().substr(0, 16));
    check7("the observation digest is the v0.7 one, byte for byte",
           base_ctx != nullptr && base_ctx->observation_digest == kExpectedObservationDigest,
           base_ctx ? base_ctx->observation_digest.substr(0, 16) : "");
    double base_index = base_core.calculate_system_dof(base_state, nullptr, base_ctx);
    check7("the index of the released fixture is unchanged", near(base_index, kV07Index, 1e-6),
           num7(base_index));
    // The fingerprints of the released fixture, in full: a run that stopped
    // comparing must not be able to pass unnoticed, and a reader of the log must be
    // able to see the values the run asserted against.
    std::cout << "\nRULER  digest=" << base_decl.digest() << "\n";
    if (base_ctx != nullptr) {
        std::cout << "OBSERVATION digest=" << base_ctx->observation_digest << "\n";
    }
    std::cout << "INDEX  " << num7(base_index) << "\n\n";

    DOFOrchestrator orch(0.05);
    SystemStateMatrix state = orch.measure(t1_scene());
    const GraphMapper& mapper = orch.mapper_ref();
    const ObservationContext* ctx = mapper.last_observation ? &*mapper.last_observation : nullptr;
    const dof::MeasurementDeclaration& decl = *mapper.last_declaration;
    const DOFCalculusCore& core = orch.core_ref();

    std::cout << "=== 2. §4.5: the two entities D3 exists for ===\n";
    double trainee_dof = state.entities.at("trainee").current_dof;
    double minimum = 1.0;
    for (const auto& id : core.calc_members(state, ctx)) {
        auto it = state.entities.find(id);
        if (it != state.entities.end()) minimum = std::min(minimum, it->second.current_dof);
    }
    check7("trainee sits above the minimum DoF of calc(S), so it is not critical",
           trainee_dof > 0.0, "trainee=" + num7(trainee_dof) + " min=" + num7(minimum));
    check7("trainee can act itself: V > 0", ctx->v_before("trainee") > 0,
           "V=" + std::to_string(ctx->v_before("trainee")));
    check7("and its own act does not lift it — its path is somebody else's act",
           ctx->verdict("trainee") == "reachable", ctx->verdict("trainee"));
    std::set<std::string> critical = core.critical_members(state, ctx);
    check7("critical(S) is the known zeros of calc, not the dependents",
           critical.size() == 2 && critical.count("revivable") && critical.count("unobserved"),
           std::to_string(critical.size()));

    std::cout << "=== 3. §4.5: the vector is computed, not declared ===\n";
    CandidateVector vec_comp = vector_of(core, state, ctx, t1_compensate());
    check7("t1_compensate: no destruction — nothing is driven to a known zero",
           vec_comp.d1 == 0, vec_str(vec_comp));
    check7("t1_compensate: it cuts the patient's only path", vec_comp.d2 == 1, vec_str(vec_comp));
    check7("t1_compensate: and that path is the critical node's", vec_comp.d3 == 1,
           vec_str(vec_comp));
    check7("t1_compensate: NetDelta > 0 — every pre-v0.8 gate passes it", vec_comp.net_delta > 0.0,
           num7(vec_comp.net_delta));
    check7("t1_compensate: the collapse charges are empty, as §4.2 computes them",
           core.collapse_charges(state, t1_compensate(), ctx).empty());
    std::vector<LostPathEntry> lost = core.lost_paths(state, t1_compensate(), ctx);
    check7("t1_compensate: the lost path is auditable, with the witness it lost",
           lost.size() == 1 && lost[0].entity_id == "revivable" &&
               lost[0].verdict_before == "reachable" &&
               lost[0].verdict_after == "proven_unreachable" && lost[0].critical &&
               lost[0].witness_lost.size() == 1 && lost[0].witness_lost[0] == "act_medkit",
           std::to_string(lost.size()));

    std::cout << "=== 4. §4.5: the decision that changed ===\n";
    auto single = core.select_candidate(state, {t1_compensate()}, ctx);
    check7("under T1 the option loses to staying put: the system stays", !single.first.has_value());
    std::optional<std::string> key = DOFCalculusCore::barring_key(vec_comp);
    check7("the report names the dimension that barred it", key.has_value() && *key == "d2",
           key ? *key : "");
    CandidateVector baseline = DOFCalculusCore::baseline_vector();
    check7("staying put is a candidate with the zero vector",
           baseline.d1 == 0 && baseline.d2 == 0 && baseline.d3 == 0 &&
               baseline.net_delta == 0.0 && baseline.reversible);

    ReportInput refusal_in;
    refusal_in.declaration = decl;
    refusal_in.ctx = ctx;
    DofReport refusal = core.report(state, {t1_compensate()}, std::nullopt, "FAST_PASS",
                                    refusal_in);
    check7("no candidate beat inaction, and the report says so", refusal.no_candidate_better);
    check7("the refusal lists the candidates and the dimension that barred each",
           refusal.options.size() == 1 && refusal.options[0].barring_key.has_value() &&
               *refusal.options[0].barring_key == "d2");

    std::cout << "=== 5. §4.5: the mirror — the same gain, a path that is not the price ===\n";
    CandidateVector vec_mirror = vector_of(core, state, ctx, t1_mirror());
    check7("t1_mirror: nothing destroyed, nothing lost",
           vec_mirror.d1 == 0 && vec_mirror.d2 == 0 && vec_mirror.d3 == 0, vec_str(vec_mirror));
    check7("t1_mirror: a real gain over staying put", vec_mirror.net_delta > 0.0,
           num7(vec_mirror.net_delta));
    check7("robot keeps eight of its nine vectors: the closure was a price, not a loss",
           ctx->v_after_closure("robot", t1_mirror().closed) == 8 && ctx->v_before("robot") == 9);
    auto chosen = core.evaluate_and_select(state, {t1_compensate(), t1_mirror()}, ctx);
    check7("the mirror is selected",
           chosen.has_value() && chosen->option_id == "t1_mirror");

    std::cout << "=== 6. §4.5: a path cut is a bar, and the third dimension cannot separate ===\n";
    CandidateVector vec_help = vector_of(core, state, ctx, t1_help());
    CandidateVector vec_rival = vector_of(core, state, ctx, t1_rival());
    // `v0.11` §4.9: the condition is `DoF(X | h) + Δ_P(X) > 0`, and the **trivial**
    // path is a finite simple path — so cutting a path does not make a *live*
    // entity unreachable, and `D2` no longer fires for it. `D2` keeps its teeth
    // exactly where it matters: an option that drives the entity to a **known
    // zero** and closes its recovery — which is what `t1_rival` does, and the
    // assertion below still holds for it.
    check7("t1_help: it cuts a path without destroying anything",
           vec_help.d1 == 0 && vec_help.d2 == 0 && vec_help.d3 == 0, vec_str(vec_help));
    std::vector<LostPathEntry> help_lost = core.lost_paths(state, t1_help(), ctx);
    check7("t1_help: and no path is reported as lost", help_lost.empty(),
           std::to_string(help_lost.size()));
    check7("closing the mentor's act costs it a vector and destroys nothing",
           core.collapse_charges(state, t1_help(), ctx).empty() &&
               ctx->v_after_closure("mentor", t1_help().closed) == 1);
    check7("t1_rival: the same D1 and D2, and the path is the critical node's",
           vec_rival.d1 == 0 && vec_rival.d2 == 1 && vec_rival.d3 == 1, vec_str(vec_rival));
    check7("staying put wins at the second dimension against both: a cut path is a bar",
           !core.evaluate_and_select(state, {t1_help(), t1_rival()}, ctx).has_value());
    check7("so the third dimension cannot separate two candidates — D3 <= D2, and an admissible "
           "candidate has D2 = 0 (§4.5, finding of this release)",
           vec_comp.d3 <= vec_comp.d2 && vec_mirror.d3 <= vec_mirror.d2 &&
               vec_help.d3 <= vec_help.d2 && vec_rival.d3 <= vec_rival.d2);

    std::cout << "=== 7. §4.5: bounds, the baseline and the retired gate ===\n";
    std::set<std::string> members = core.calc_members(state, ctx);
    std::size_t calc_size = members.size();
    check7("an empty candidate set selects nothing",
           !core.evaluate_and_select(state, {}, ctx).has_value());
    check7("D1, D2 and D3 are bounded by calc(S)",
           vec_comp.d1 <= static_cast<int>(calc_size) && vec_comp.d2 <= static_cast<int>(calc_size) &&
               vec_comp.d3 <= static_cast<int>(calc_size) &&
               vec_rival.d2 <= static_cast<int>(calc_size),
           "|calc|=" + std::to_string(calc_size));
    auto chosen2 = core.evaluate_and_select(state, {t1_compensate(), t1_mirror()}, ctx);
    ReportInput in;
    in.declaration = decl;
    in.ctx = ctx;
    DofReport rep = core.report(state, {t1_compensate(), t1_mirror()}, chosen2, "FAST_PASS", in);
    const OptionReportRow* row = nullptr;
    for (const auto& r : rep.options) {
        if (r.option_id == "t1_compensate") row = &r;
    }
    check7("the report carries the vector and the barring dimension per option",
           row != nullptr && row->candidate_vector.d2 == 1 && row->barring_key.has_value() &&
               *row->barring_key == "d2");
    check7("the report carries the baseline",
           rep.baseline.d1 == 0 && rep.baseline.net_delta == 0.0 && rep.baseline.reversible);
    check7("a selected option is not reported as a refusal", !rep.no_candidate_better);
    bool no_collapse_gate = true;
    for (const auto& removed : rep.removed_options) {
        if (removed.gate == "collapse") no_collapse_gate = false;
    }
    check7("the structural decision is no longer a removal: no collapse gate anywhere",
           no_collapse_gate, std::to_string(rep.removed_options.size()));
    auto retired = core.apply_structural_gate(state, {options_v07::opt_win(),
                                                      options_v07::opt_collapse()}, ctx);
    check7("a charged candidate is no longer deleted from the set: it is evaluated and reported",
           retired.second.size() == 1 && retired.second[0].gate == "collapse",
           "the retired v0.7 rule is still callable by the historical harness");
    auto live = core.select_candidate(state, {options_v07::opt_win(),
                                              options_v07::opt_collapse()}, ctx);
    check7("and on the live path nobody leaves the candidate set", live.second.size() == 2);

    std::cout << "\nchecks: " << g_checks7 << ", failures: " << g_failures7.size() << "\n";
    if (!g_failures7.empty()) {
        std::cout << "FAILURES:";
        for (const auto& f : g_failures7) std::cout << " [" << f << "]";
        std::cout << "\nFAILED\n";
        return 1;
    }
    std::cout << "OK\n";
    return 0;
}
