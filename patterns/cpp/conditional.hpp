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
    std::vector<std::string> robust_admissible;
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
    const ObservationContext* ctx, const std::string& hypothesis_id,
    const std::vector<std::vector<std::string>>* groups = nullptr,
    const std::map<std::string, dof::Rate>* rates = nullptr,
    const std::map<std::string, double>* weights = nullptr,
    const std::optional<double>& cap = std::nullopt) {
    std::unordered_map<std::string, double> dofs;
    for (const auto& kv : state.entities) {
        dofs[kv.first] = kv.second.current_dof;
    }
    // §4.9/§4.10: the context may be **absent**, and that is not a refusal. A scene
    // without a graph is still decidable — the verdicts read `undetermined` and are
    // priced by `u(t)` — so an absent context changes what the quantities are, never
    // whether they exist. Refusing here would publish `none` for a scene in which the
    // reference selects an option: a divergence a reader can see (§7, §4.10.6).
    const std::optional<ObservationContext> h_ctx =
        ctx != nullptr ? std::optional<ObservationContext>(ctx->with_dof(dofs)) : std::nullopt;
    const ObservationContext* h_ctx_ptr = h_ctx ? &*h_ctx : nullptr;
    const double current_index = core.calculate_system_dof(state, nullptr, h_ctx_ptr);
    const ViabilityResult viable = viability(state, option);
    const FundingPlan plan = core.plan_funding(state, option, groups, rates, weights, cap);
    const CandidateVector vector =
        core.candidate_vector_for(state, option, h_ctx_ptr, current_index, hypothesis_id);
    return conditional_vector_from(vector, hypothesis_id, viable.viable, plan.covered);
}

// §6.3: `{option_id: {hypothesis_id: vector}}`.
inline std::map<std::string, std::map<std::string, ConditionalVector>> conditional_vectors(
    const DOFCalculusCore& core, const std::vector<Hypothesis>& members,
    const std::vector<ActionOption>& options, const ObservationContext* ctx,
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
    const ObservationContext* ctx,
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
    selection.robust_admissible = robust_ids;
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

// ---------------------------------------------------------------------------
// §6.2/§6.3 (v0.11): the audit report over a declared hypothesis set.
//
// `dof_core.hpp`'s `DofReport` is complete for **one** reading, and the per-reading
// surface cannot live there: this header is an overlay over the core — it depends on
// it, never the other way round — so the types the surface needs (`Hypothesis`, the
// conditional vectors) are invisible to the core. The v0.11 report is therefore the
// core's flat report **plus** the same facts per reading, and it is the object a
// v0.11 consumer reads.
//
// The observed-state reduction (§4.10.6) is the same call with `hset == nullptr`:
// one reading, one entry in every map, and the flat part unchanged.
struct ReportV011 {
    // §6.1–§6.3 flat: the observed reading's values, exactly as `report()` produces.
    DofReport base;
    // §6.2: the declared set as provenance — each reading with its `state` in full,
    // because that state is what the conditional quantities were read from.
    std::map<std::string, Hypothesis> hypotheses;
    std::vector<std::string> plausible_hypotheses;
    std::string hypothesis_coverage = "partial";
    std::optional<double> hypothesis_horizon_mks;
    bool hypothesis_conflict = false;
    std::map<std::string, double> total_system_dof_by_hypothesis;
    // §6.2: the reading `base.total_system_dof` belongs to. A scalar beside a map is
    // admissible only when the report says whose value it is — an unlabelled number
    // is read as a summary of the readings, which is the aggregate the standard
    // refuses. (`base` carries no such field: without a set the scalar *is* the
    // total, and no label is needed to keep a reader from averaging.)
    std::string total_system_dof_reading = std::string(kObservedHypothesisId);
    std::vector<std::string> robust_admissible;
    std::map<std::string, double> net_delta_robust;
    // §6.3: the per-option surface, keyed by option and then by reading.
    std::map<std::string, std::map<std::string, ConditionalVector>> conditional_vectors;
    std::map<std::string, std::map<std::string, bool>> admissible_under;
    std::map<std::string, std::map<std::string, std::optional<std::string>>> barring_key_by_hypothesis;
    // §6.1: the reason a value is what it is belongs to a reading too — there is no
    // shared `ψ` to print once the readings differ.
    std::map<std::string, std::map<std::string, std::vector<LensTerm>>> lens_terms_by_hypothesis;
    std::map<std::string, std::map<std::string, std::optional<std::string>>> binding_lens_by_hypothesis;
    std::map<std::string, std::map<std::string, RecoverabilityRow>> recoverability_by_hypothesis;
};

// §4.9: the same observation read under **this** reading's DoF. The graph, the
// paths, `M(S)` and `T_rec(X)` stay shared (§3.6); only the measured content moves.
inline ObservationContext reading_context(const ObservationContext& ctx,
                                         const Hypothesis& hypothesis) {
    std::unordered_map<std::string, double> dofs;
    for (const auto& kv : hypothesis.state.entities) {
        dofs[kv.first] = kv.second.current_dof;
    }
    return ctx.with_dof(dofs);
}

// The full report of one cycle over a declared set, **with the decision it describes**.
//
// The conditional pass is made once. When the caller has already made it — the
// orchestrator has, because it publishes the decision — it hands the pass in
// (`selected`, `selection`) and the report describes *that* pass; a second computation
// could describe a decision nobody took (§4.10). A caller that only wants the report
// leaves them empty and the pass is made here.
inline std::pair<std::optional<ActionOption>, ReportV011> report_on_set(
                                const DOFCalculusCore& core, const SystemStateMatrix& state,
                                const std::vector<ActionOption>& options,
                                const std::string& mode, const HypothesisSet* hset,
                                const ReportInput& input = ReportInput{},
                                const std::optional<ActionOption>& selected = std::nullopt,
                                const std::optional<ConditionalSelection>& selection = std::nullopt) {
    std::vector<Hypothesis> declared = resolved_members(state, hset);
    std::vector<Hypothesis> readings = plausible_members(declared);
    const ObservationContext* ctx = input.ctx;
    ReportV011 out;
    std::optional<ActionOption> decision = selected;
    // The pass is made **whether or not** the cycle has an observation context: a
    // scene without a graph is decidable, and the reference computes here (§4.10.6).
    std::optional<ConditionalSelection> pass = selection;
    if (!pass.has_value()) {
        auto choice = select_conditional(core, state, options, readings, ctx, input.groups,
                                         input.rates, input.weights, input.cap);
        decision = choice.first;
        pass = choice.second;
    }
    out.base = core.report(state, options, decision, mode, input);
    out.conditional_vectors = pass->conditional_vectors;
    out.admissible_under = pass->admissible_under;
    out.net_delta_robust = pass->net_delta_robust;
    out.robust_admissible = pass->robust_admissible;
    out.hypothesis_conflict = pass->hypothesis_conflict;
    for (const auto& h : declared) {
        out.hypotheses[h.id] = h;
    }
    out.hypothesis_coverage = coverage_of(hset);
    // §6.2: the scalar in `base` is the observed reading's, and the report says so.
    out.total_system_dof_reading = std::string(kObservedHypothesisId);
    out.hypothesis_horizon_mks = hset != nullptr ? hset->horizon_mks : std::nullopt;
    for (const auto& h : readings) {
        out.plausible_hypotheses.push_back(h.id);
        // §6.2: the per-reading surface is published **even with no observation
        // context** — the reading's own state is what the totals are read from, and a
        // context that does not exist is not a reason to drop the map the audit is
        // read from (the reference computes `h_ctx = None` here, and so does Go).
        const std::optional<ObservationContext> h_ctx =
            ctx != nullptr ? std::optional<ObservationContext>(reading_context(*ctx, h))
                           : std::nullopt;
        const ObservationContext* h_ctx_ptr = h_ctx ? &*h_ctx : nullptr;
        out.total_system_dof_by_hypothesis[h.id] =
            core.calculate_system_dof(h.state, nullptr, h_ctx_ptr);
        for (const auto& kv : h.state.entities) {
            if (kv.second.measurement.has_value()) {
                out.lens_terms_by_hypothesis[kv.first][h.id] = kv.second.measurement->terms;
                out.binding_lens_by_hypothesis[kv.first][h.id] =
                    kv.second.measurement->binding_lens;
            }
            out.recoverability_by_hypothesis[kv.first][h.id] =
                core.recoverability_row(kv.first, h_ctx_ptr);
        }
    }
    // §6.3: the key that barred a candidate is read **under the reading that barred
    // it** — a candidate barred for executability must not be reported as barred for
    // a structural reason it never reached.
    for (const auto& kv : out.conditional_vectors) {
        for (const auto& per_h : kv.second) {
            const ConditionalVector& v = per_h.second;
            CandidateVector flat;
            flat.d1 = v.d1;
            flat.d2 = v.d2;
            flat.d3 = v.d3;
            flat.net_delta = v.net_delta;
            flat.reversible = v.reversible;
            flat.option_id = v.option_id;
            flat.viable = v.viable;
            flat.resources_ok = v.resources_ok;
            out.barring_key_by_hypothesis[kv.first][per_h.first] =
                DOFCalculusCore::barring_key(flat);
        }
    }
    return {decision, out};
}

}  // namespace dof

#endif  // DOF_CONDITIONAL_HPP
