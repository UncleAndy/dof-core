// Hypothesis set artifact (DOF-SPEC §3.6) and its validation rules (C++ port).
//
// `v0.10` made the *state* conditional; `v0.11` repaired the artifact so a
// hypothesis is a **complete alternative state** under the **shared ruler**
// (§3.6, §10(A)):
//
//   * a hypothesis supplies the *measured* content — the per-entity lens
//     counters, the measurement durations and the resource map, from which τ
//     follows; `current_dof`, `dof_known` and τ are **computed** from it by the
//     same named procedures, and a stated DoF that its own counters do not
//     produce is non-conformant input;
//   * the **ruler** (§3.4.2) and the observed graph `G` (§3.5) are **shared** —
//     structural uncertainty is *priced, not branched*;
//   * `is_collapse_source` is **not** hypothesis-local: the label is honoured
//     through an observed act, the act comes from the shared graph, and a set
//     that moves the label between readings is non-conformant input;
//   * absence and emptiness are the **observed-state singleton**, with
//     `plausible = true`, so `H_plausible` is never empty and the worst-case
//     operators of §4.10 are total.
//
// The core accepts `H` as supplied: it MUST NOT add, merge, split, drop, reorder
// or re-weight a hypothesis, and MUST NOT compute the `plausible` flag.

#ifndef DOF_HYPOTHESIS_HPP
#define DOF_HYPOTHESIS_HPP

#include <algorithm>
#include <cmath>
#include <map>
#include <string>
#include <vector>

#include "dof_core.hpp"

namespace dof {

// §3.6: the observed-state id (`kObservedHypothesisId`) is declared next to the
// option in `dof_core.hpp`, because the per-hypothesis accessors live there.

// §3.6: the tolerance of the identity checks — the stated DoF must equal the
// product of the reading's own counters to this precision. The same constant is
// used by `same_state`, so two readings that are "equal" by one rule and
// "different" by the other cannot happen.
inline const double kHypothesisTolerance = 1e-9;

// §3.6: one declared interpretation of the same observed state.
struct Hypothesis {
    std::string id;
    bool plausible = true;
    SystemStateMatrix state;
    // Report context (§3.4.1): the declared causal reading. Inert — it MUST NOT
    // affect `calc`, the collapse charges, the §4.9 verdicts or the Axiom-3
    // exemption. An empty list reads as "names no possible source", never as
    // "asserts that no source exists".
    std::vector<std::string> collapse_source_candidates;
    std::string basis;
};

// §3.6: the artifact. `coverage` is a claim, and its default is cautious.
struct HypothesisSet {
    std::string coverage = "partial";
    std::vector<Hypothesis> members;
    // The analysis horizon declared with the set.
    std::optional<double> horizon_mks;
};

// The declared coverage of a set, with §3.6's cautious default.
//
// An **empty** claim reads as `partial` exactly like an absent one: a set that
// did not say how much of the space it covers has not claimed to cover it, and
// inferring `complete` from silence is the inference §3.6 forbids.
inline std::string coverage_of(const HypothesisSet* hset) {
    if (hset != nullptr && !hset->coverage.empty()) return hset->coverage;
    return "partial";
}

// §3.6/§4.10.6: an absent or empty `H` **is** the observed-state singleton.
inline std::vector<Hypothesis> observed_singleton(const SystemStateMatrix& state) {
    Hypothesis h;
    h.id = kObservedHypothesisId;
    h.plausible = true;
    h.state = state;
    h.basis = "absence or emptiness of H is the observed state";
    return {h};
}

// `H` as the core reads it, including the absence/emptiness reduction.
inline std::vector<Hypothesis> resolved_members(const SystemStateMatrix& state,
                                                const HypothesisSet* hset) {
    if (hset == nullptr || hset->members.empty()) return observed_singleton(state);
    return hset->members;
}

// `H_plausible = { h ∈ H : h.plausible }`.
//
// Never empty: the set is either the observed singleton or a declared set whose
// observed member MUST be present and plausible (§3.6), so a set that resolves to
// nothing is non-conformant input caught by `validate_set`.
inline std::vector<Hypothesis> plausible_members(const std::vector<Hypothesis>& members) {
    std::vector<Hypothesis> out;
    for (const auto& h : members) {
        if (h.plausible) out.push_back(h);
    }
    if (out.empty() && !members.empty()) return {members.front()};
    return out;
}

// The entity's DoF as the product of its **own** lens values (§4.1, §4.6).
//
// `nullopt` when the entity carries no measurement declaration or a lens that was
// never measured: the port-level `measurement` object is the carrier of the
// counters, and without it the identity cannot be checked. A missing declaration
// is not a contradiction — it is an unpriced statement.
inline std::optional<double> lens_product(const EntityState& entity) {
    if (!entity.measurement) return std::nullopt;
    double product = 1.0;
    for (const auto& lens : lens_order()) {
        auto it = entity.measurement->psi_by_lens.find(lens);
        if (it == entity.measurement->psi_by_lens.end() || !it->second) return std::nullopt;
        product *= *it->second;
    }
    return product;
}

// Whether two matrices are the same *measured* state.
//
// Only the measured content is compared: the per-entity DoF, the lens counters,
// the resource map and τ. Report context and provenance are not part of the
// comparison — two readings that differ only in a `basis` string are the same
// state.
inline bool same_state(const SystemStateMatrix& a, const SystemStateMatrix& b) {
    if (a.entities.size() != b.entities.size()) return false;
    for (const auto& kv : a.entities) {
        auto it = b.entities.find(kv.first);
        if (it == b.entities.end()) return false;
        const EntityState& ea = kv.second;
        const EntityState& eb = it->second;
        if (std::fabs(ea.current_dof - eb.current_dof) > kHypothesisTolerance) return false;
        if (ea.dof_known != eb.dof_known) return false;
        if (static_cast<bool>(ea.measurement) != static_cast<bool>(eb.measurement)) return false;
        if (ea.measurement) {
            for (const auto& lens : lens_order()) {
                auto ia = ea.measurement->psi_by_lens.find(lens);
                auto ib = eb.measurement->psi_by_lens.find(lens);
                const bool pa = ia != ea.measurement->psi_by_lens.end();
                const bool pb = ib != eb.measurement->psi_by_lens.end();
                if (pa != pb) return false;
                if (!pa) continue;
                // A lens unmeasured under BOTH readings is the same measured
                // content: `nullopt` is "never measured", not a value to compare.
                // Reading the pair as a difference would declare two identical
                // states different, and rule (4) of §3.6 — "the observed state
                // MUST be one of the readings" — could then never hold for any set
                // whose fixture leaves a lens unmeasured.
                const std::optional<double> va = ia->second;
                const std::optional<double> vb = ib->second;
                if (static_cast<bool>(va) != static_cast<bool>(vb)) return false;
                if (va && std::fabs(*va - *vb) > kHypothesisTolerance) return false;
            }
        }
    }
    if (a.resources.size() != b.resources.size()) return false;
    for (const auto& kv : a.resources) {
        auto it = b.resources.find(kv.first);
        if (it == b.resources.end()) return false;
        const std::optional<double> va = kv.second.value;
        const std::optional<double> vb = it->second.value;
        if (static_cast<bool>(va) != static_cast<bool>(vb)) return false;
        if (va && std::fabs(*va - *vb) > kHypothesisTolerance) return false;
    }
    if (static_cast<bool>(a.tau) != static_cast<bool>(b.tau)) return false;
    if (a.tau) {
        const std::optional<double> va = a.tau->value;
        const std::optional<double> vb = b.tau->value;
        if (static_cast<bool>(va) != static_cast<bool>(vb)) return false;
        if (va && std::fabs(*va - *vb) > kHypothesisTolerance) return false;
    }
    return true;
}

// §3.6 non-conformance checks. An empty list means the input is admissible.
//
// Every rule here is a *check*, not a hope: each one corresponds to a way a
// manipulated or careless hypothesis set could otherwise move the index, the
// choice or the collapse-source label without leaving a trace.
inline std::vector<std::string> validate_set(const SystemStateMatrix& state,
                                             const std::vector<Hypothesis>& members) {
    std::vector<std::string> errs;
    std::vector<std::string> ids;
    ids.reserve(members.size());
    for (const auto& h : members) ids.push_back(h.id);
    std::vector<std::string> sorted_ids = ids;
    std::sort(sorted_ids.begin(), sorted_ids.end());
    sorted_ids.erase(std::unique(sorted_ids.begin(), sorted_ids.end()), sorted_ids.end());
    if (sorted_ids.size() != ids.size()) {
        errs.push_back("hypothesis ids are not unique");
    }
    bool observed_present = false;
    for (const auto& h : members) {
        const SystemStateMatrix& hs = h.state;
        // (1) State completeness: every entity of `S` appears in every reading,
        //     with an explicit value, never omitted.
        std::vector<std::string> missing;
        for (const auto& kv : hs.entities) {
            if (state.entities.find(kv.first) == state.entities.end()) missing.push_back(kv.first);
        }
        std::sort(missing.begin(), missing.end());
        if (!missing.empty()) {
            errs.push_back(h.id + ": declares entities not in the observed state");
        }
        std::vector<std::string> omitted;
        for (const auto& kv : state.entities) {
            if (hs.entities.find(kv.first) == hs.entities.end()) omitted.push_back(kv.first);
        }
        std::sort(omitted.begin(), omitted.end());
        if (!omitted.empty()) {
            errs.push_back(h.id + ": omits entities (§3.6 state completeness)");
        }
        // (2) The lens identity holds under every hypothesis: the stated DoF must
        //     equal the product of that reading's OWN counters (§4.1, §3.6).
        std::vector<std::string> eids;
        for (const auto& kv : hs.entities) eids.push_back(kv.first);
        std::sort(eids.begin(), eids.end());
        for (const auto& e_id : eids) {
            const EntityState& ent = hs.entities.at(e_id);
            const std::optional<double> product = lens_product(ent);
            if (product && std::fabs(*product - ent.current_dof) > kHypothesisTolerance) {
                errs.push_back(h.id + "/" + e_id +
                               ": stated DoF differs from the product of its own lens values "
                               "(§4.1, §3.6)");
            }
            // (3) `is_collapse_source` is not hypothesis-local: the label is
            //     honoured through an act of the SHARED graph, so it must equal the
            //     observed value under every reading (§3.6).
            auto obs = state.entities.find(e_id);
            if (obs != state.entities.end() &&
                ent.is_collapse_source != obs->second.is_collapse_source) {
                errs.push_back(h.id + "/" + e_id +
                               ": is_collapse_source differs from the observed value — the label "
                               "is not hypothesis-local (§3.6)");
            }
        }
        // (5) §3.4.1/§3.4.2 (v0.11): the measurement durations are **ruler-level**.
        //     A hypothesis reinterprets what was *measured*; it may not reinterpret
        //     how long the *measuring* takes. If it could, `T_meas` and hence `t*`
        //     would differ between readings that claim one ruler, and
        //     `min_h NetDelta(o | h)` would compare numbers produced by different
        //     measuring systems — which is what the shared ruler digest exists to
        //     make impossible.
        if (hs.measurement_durations != state.measurement_durations) {
            errs.push_back(h.id +
                           ": measurement durations differ from the observed procedure — the "
                           "durations are ruler-level, not hypothesis-level (§3.4.1, §3.4.2)");
        }
        // (4) The observed state MUST be one of the readings (§3.6).
        if (same_state(hs, state)) observed_present = true;
    }
    if (!observed_present) {
        errs.push_back("the observed state is absent from H (§3.6: it MUST be present)");
    }
    if (members.size() > 1) {
        bool any_plausible = false;
        for (const auto& h : members) {
            if (h.plausible) any_plausible = true;
        }
        if (!any_plausible) {
            // A single-member set is the observed singleton and is plausible by
            // construction; a larger set that marks everything implausible leaves
            // `H_plausible` empty, which §3.6 declares an invalid input.
            errs.push_back(
                "every hypothesis is implausible (§3.6: H_plausible would be empty)");
        }
    }
    return errs;
}

}  // namespace dof

#endif  // DOF_HYPOTHESIS_HPP
