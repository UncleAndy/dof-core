// DOF-SPEC v0.11 §4.10 — robust selection over the declared readings (C++ port).
// Mirrors patterns/python/calculus_core.py (`conditional_vector`,
// `conditional_vectors`, `robust_admissible`, `least_favourable`,
// `admissible_under`, `hypothesis_conflict`, `robust_reversible`,
// `select_conditional`), patterns/go/conditional.go and patterns/rust/conditional.rs.
//
// The shape of the rule, in one place:
//
//   - the **candidate set is the same for every hypothesis**; only the projection
//     differs. A reading never adds or removes an option;
//   - admissibility is a **universally quantified conjunction** over `H_plausible`
//     — `viable ∧ resources_ok ∧ D1 = D2 = D3 = 0` under *every* plausible reading;
//   - the ordering key is the **least-favourable** conditional delta
//     `min_h NetDelta(o | h)`, never a maximum: the greatest conditional delta
//     would be exactly the optimistic aggregation §4.10 refuses;
//   - §4.5 keys 3 and 4 then apply unchanged, key 3 read **robustly** (reversible
//     under every plausible reading) because key 2 has already charged the closure
//     at its worst;
//   - an empty robust candidate set yields "stay". There is **no fallback to
//     admissible support**: ranking the survivors of an inadmissible set would be
//     the compensation Axiom 3 forbids.

#ifndef DOF_CONDITIONAL_HPP
#define DOF_CONDITIONAL_HPP

#include <map>
#include <optional>
#include <string>
#include <vector>

#include "dof_core.hpp"
#include "hypothesis.hpp"
#include "viability.hpp"

namespace dof {

// §6.3: one candidate's vector **under one reading**.
//
// It carries the whole admissibility predicate — `viable`, `resources_ok` and the
// three dimensions — so that §4.10's aggregation reads one structure and not three.
struct ConditionalVector {
    std::string option_id;
    std::string hypothesis_id;
    int d1 = 0;
    int d2 = 0;
    int d3 = 0;
    double net_delta = 0.0;
    bool reversible = true;
    bool viable = false;
    bool resources_ok = false;

    // §4.10.1: the admissibility predicate of one reading's vector.
    bool barred() const {
        return !viable || !resources_ok || d1 > 0 || d2 > 0 || d3 > 0;
    }
};

// One reading's vector: the §4.5 keys computed under that reading, plus the
// temporal condition of §4.8b and the financial one of §4.8 — the latter two are
// per-option and do not vary with the reading, but they belong to the predicate,
// and a predicate assembled in two places drifts.
inline ConditionalVector conditional_vector_from(const CandidateVector& vector,
                                                 const std::string& hypothesis_id,
                                                 bool viable, bool resources_ok) {
    ConditionalVector cv;
    cv.option_id = vector.option_id;
    cv.hypothesis_id = hypothesis_id;
    cv.d1 = vector.d1;
    cv.d2 = vector.d2;
    cv.d3 = vector.d3;
    cv.net_delta = vector.net_delta;
    cv.reversible = vector.reversible;
    cv.viable = viable;
    cv.resources_ok = resources_ok;
    return cv;
}

// §6.3: the decision payload — everything the report needs to show what was
// decided and under which reading.
struct ConditionalSelection {
    // {option_id: {hypothesis_id: vector}} — ordered like the reference's maps.
    std::map<std::string, std::map<std::string, ConditionalVector>> conditional_vectors;
    std::map<std::string, std::map<std::string, bool>> admissible_under;
    bool hypothesis_conflict = false;
    std::vector<std::string> robust_candidates;
    std::map<std::string, double> net_delta_robust;
};

// §4.5's full vector of one candidate under one reading.
//
// The state passed in is that reading's own state, and the context is the same
// observation read under that reading's DoF (§4.9): every quantity of §4.1–§4.9 is
// conditional (§4.10). The financial condition is evaluated with the **same
// declared mandate and observed rates** the cycle uses (§4.8) — passing none of
// them would make every option with a `requires` look insolvent, because a deficit
// could never be converted.
inline ConditionalVector conditional_vector_of(
    const DOFCalculusCore& core, const SystemStateMatrix& state, const ActionOption& option,
    const ObservationContext& ctx, const std::string& hypothesis_id,
    const std::vector<std::vector<std::string>>* groups = nullptr,
    const std::map<std::string, dof::Rate>* rates = nullptr,
    const std::map<std::string, double>* weights = nullptr,
    const std::optional<double>& cap = std::nullopt) {
    std::unordered_map<std::string, double> dofs;
    for (const auto& kv : state.entities) {
        dofs[kv.first] = kv.second.current_dof;
    }
    const ObservationContext h_ctx = ctx.with_dof(dofs);
    const double current_index = core.calculate_system_dof(state, nullptr, &h_ctx);
    const ViabilityResult viable = viability(state, option);
    const FundingPlan plan = core.plan_funding(state, option, groups, rates, weights, cap);
    const CandidateVector vector =
        core.candidate_vector_for(state, option, &h_ctx, current_index, hypothesis_id);
    return conditional_vector_from(vector, hypothesis_id, viable.viable, plan.covered);
}

// §6.3: `{option_id: {hypothesis_id: vector}}`.
inline std::map<std::string, std::map<std::string, ConditionalVector>> conditional_vectors(
    const DOFCalculusCore& core, const std::vector<Hypothesis>& members,
    const std::vector<ActionOption>& options, const ObservationContext& ctx,
    const std::vector<std::vector<std::string>>* groups = nullptr,
    const std::map<std::string, dof::Rate>* rates = nullptr,
    const std::map<std::string, double>* weights = nullptr,
    const std::optional<double>& cap = std::nullopt) {
    std::map<std::string, std::map<std::string, ConditionalVector>> out;
    for (const auto& option : options) {
        std::map<std::string, ConditionalVector> per_h;
        for (const auto& h : members) {
            per_h[h.id] = conditional_vector_of(core, h.state, option, ctx, h.id, groups, rates,
                                                weights, cap);
        }
        out[option.option_id] = per_h;
    }
    return out;
}

// §4.5 key 3 under a hypothesis set:
//
//   `reversible_robust(o) ⇔ ∀ h ∈ H_plausible : o.closed[h] = []`
//
// The direction is not a matter of taste. Key 2 worst-cases the **price** of a
// closure (`min_h NetDelta(o | h)`), so a closure invisible at one reading but real
// at another is already charged at its worst. If this preference were read from a
// single reading — the observed one included — that same closure would be charged
// at its worst *and* rewarded as if it did not exist, and the robust ordering could
// be reversed by the very closure the worst case exists to weigh. So the preference
// is taken over the same set, in the same direction, as the price. With no
// hypothesis set this reduces to the flat `closed = []` of §4.5.
inline bool robust_reversible(const ActionOption& option,
                              const std::vector<Hypothesis>& members) {
    if (members.empty()) return is_reversible_for(option, kObservedHypothesisId);
    for (const auto& h : members) {
        if (!is_reversible_for(option, h.id)) return false;
    }
    return true;
}

// §4.10.1: admissible under **every** plausible hypothesis.
//
// The predicate is both families of conditions — the temporal one of §4.8b, the
// financial one of §4.8 and the three dimensions of §4.5 — and the aggregation is a
// universally quantified conjunction. Neither is a maximum over hypotheses.
inline bool robust_admissible(const std::map<std::string, ConditionalVector>& per_h,
                              const std::vector<Hypothesis>& members) {
    for (const auto& h : members) {
        auto it = per_h.find(h.id);
        if (it == per_h.end() || it->second.barred()) return false;
    }
    return true;
}

// `NetDelta_robust(o) = min_{h ∈ H_plausible} NetDelta(o | h)` (§4.10.2). An empty
// set yields 0.0: staying put.
inline double least_favourable(const std::map<std::string, ConditionalVector>& per_h,
                               const std::vector<Hypothesis>& members) {
    bool seen = false;
    double least = 0.0;
    for (const auto& h : members) {
        auto it = per_h.find(h.id);
        if (it == per_h.end()) continue;
        if (!seen || it->second.net_delta < least) least = it->second.net_delta;
        seen = true;
    }
    return seen ? least : 0.0;
}

// §6.3: per reading, whether the option is admissible.
inline std::map<std::string, bool> admissible_under(
    const std::map<std::string, ConditionalVector>& per_h,
    const std::vector<Hypothesis>& members) {
    std::map<std::string, bool> out;
    for (const auto& h : members) {
        auto it = per_h.find(h.id);
        out[h.id] = (it != per_h.end()) && !it->second.barred();
    }
    return out;
}

// §4.10.5: an unresolved conflict the report MUST surface.
//
// Fires when `H_plausible` has more than one element and either (a) the robust
// **candidate** set is empty, or (b) some candidate is admissible under some
// plausible hypotheses and inadmissible under others — stated over the **whole
// predicate**, so an option executable under one reading and physically impossible
// under another is as much a conflict as one that destroys a counted entity under
// one reading only.
//
// A difference that does **not** move admissibility — the same bar, of different
// magnitude, under different readings — is **not** a conflict: the choice is the
// same under both readings, the conditional vectors are listed anyway, and the
// least-favourable key already resolves it.
inline bool hypothesis_conflict(
    const std::map<std::string, std::map<std::string, ConditionalVector>>& per_h_all,
    const std::vector<Hypothesis>& members, const std::vector<std::string>& robust_ids) {
    if (members.size() <= 1) return false;
    if (robust_ids.empty()) return true;
    for (const auto& kv : per_h_all) {
        const std::string& option_id = kv.first;
        const std::map<std::string, bool> flags = admissible_under(kv.second, members);
        bool any_admissible = false;
        bool any_barred = false;
        for (const auto& f : flags) {
            if (f.second) {
                any_admissible = true;
            } else {
                any_barred = true;
            }
        }
        if (any_admissible && any_barred) return true;
        if (any_admissible) {
            bool is_robust = false;
            for (const auto& id : robust_ids) {
                if (id == option_id) is_robust = true;
            }
            if (!is_robust) return true;
        }
    }
    return false;
}

// §4.10: robust selection over the declared readings.
//
// The candidate set is the same for every hypothesis; only the projection differs.
// Among the robustly admissible candidates take the greatest `NetDelta_robust`,
// then apply §4.5 keys 3 and 4 (reversibility — read robustly — then the comparison
// origin), then the lexicographically smallest `option_id`.
//
// **There is no fallback to admissible support.** An empty robust candidate set
// yields "stay" — every proposed action is barred under at least one plausible
// reading, and ranking the survivors of an inadmissible set would be the
// compensation Axiom 3 forbids (§4.10.4).
inline std::pair<std::optional<ActionOption>, ConditionalSelection> select_conditional(
    const DOFCalculusCore& core, const SystemStateMatrix& state,
    const std::vector<ActionOption>& options, const std::vector<Hypothesis>& members,
    const ObservationContext& ctx,
    const std::vector<std::vector<std::string>>* groups = nullptr,
    const std::map<std::string, dof::Rate>* rates = nullptr,
    const std::map<std::string, double>* weights = nullptr,
    const std::optional<double>& cap = std::nullopt) {
    (void)state;
    const std::map<std::string, std::map<std::string, ConditionalVector>> per_h_all =
        conditional_vectors(core, members, options, ctx, groups, rates, weights, cap);

    std::vector<ActionOption> robust;
    for (const auto& option : options) {
        static const std::map<std::string, ConditionalVector> kEmpty;
        auto it = per_h_all.find(option.option_id);
        const std::map<std::string, ConditionalVector>& per_h =
            (it == per_h_all.end()) ? kEmpty : it->second;
        if (robust_admissible(per_h, members)) robust.push_back(option);
    }
    std::vector<std::string> robust_ids;
    for (const auto& o : robust) robust_ids.push_back(o.option_id);

    ConditionalSelection selection;
    selection.conditional_vectors = per_h_all;
    selection.hypothesis_conflict = hypothesis_conflict(per_h_all, members, robust_ids);
    selection.robust_candidates = robust_ids;
    for (const auto& option : options) {
        static const std::map<std::string, ConditionalVector> kEmpty;
        auto it = per_h_all.find(option.option_id);
        const std::map<std::string, ConditionalVector>& per_h =
            (it == per_h_all.end()) ? kEmpty : it->second;
        selection.admissible_under[option.option_id] = admissible_under(per_h, members);
        selection.net_delta_robust[option.option_id] = least_favourable(per_h, members);
    }
    if (robust.empty()) return {std::nullopt, selection};

    // §4.5 key 2: the greatest least-favourable delta, ties by tolerance.
    double best = selection.net_delta_robust.at(robust.front().option_id);
    for (const auto& option : robust) {
        const double v = selection.net_delta_robust.at(option.option_id);
        if (v > best) best = v;
    }
    std::vector<ActionOption> survivors;
    for (const auto& option : robust) {
        if (std::fabs(selection.net_delta_robust.at(option.option_id) - best) <=
            DOFCalculusCore::net_delta_tolerance) {
            survivors.push_back(option);
        }
    }

    // §4.5 key 3, under its **robust** reading (§4.10.2): revert only if some
    // survivor is reversible under every plausible reading.
    bool any_reversible = false;
    for (const auto& o : survivors) {
        if (robust_reversible(o, members)) any_reversible = true;
    }
    if (any_reversible) {
        std::vector<ActionOption> kept;
        for (const auto& o : survivors) {
            if (robust_reversible(o, members)) kept.push_back(o);
        }
        survivors = kept;
    }

    // §4.5 key 4: the survivor must beat the comparison origin.
    {
        std::vector<ActionOption> kept;
        for (const auto& o : survivors) {
            if (selection.net_delta_robust.at(o.option_id) > 0.0) kept.push_back(o);
        }
        survivors = kept;
    }
    if (survivors.empty()) return {std::nullopt, selection};

    // §4.5 final key: the lexicographically smallest `option_id`.
    ActionOption chosen = survivors.front();
    for (std::size_t i = 1; i < survivors.size(); ++i) {
        if (survivors[i].option_id < chosen.option_id) chosen = survivors[i];
    }
    return {chosen, selection};
}

// §6.2: the declared coverage of a set, reported next to the decision it produced.
// An absent claim reads as `"partial"` — the cautious default — and is never
// inferred to be complete.
inline std::string hypothesis_coverage(const HypothesisSet* hset) { return coverage_of(hset); }

}  // namespace dof

#endif  // DOF_CONDITIONAL_HPP
