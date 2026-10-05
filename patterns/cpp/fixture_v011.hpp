// Fixture helpers of the v0.11 harness (§3.6, §4.10) — C++ mirror of
// patterns/go/harness_v011_fixtures.go, patterns/python/harness_v011.py and
// patterns/rust/fixture_v011.rs.
//
// The readings are built as **transformations of one observed state**, never as
// hand-written states: scaling a copy's lens values, zeroing a copy's τ, removing
// an entity. Every transformation keeps the lens identity of §4.1 exactly — the
// copy's stated DoF is set to the product of the copy's own counters — so a reading
// is refused by `validate_set` only when the harness deliberately breaks it.

#ifndef DOF_FIXTURE_V011_HPP
#define DOF_FIXTURE_V011_HPP

#include <algorithm>
#include <cmath>
#include <map>
#include <string>
#include <unordered_map>
#include <vector>

#include "dof_core.hpp"
#include "fixture_v07.hpp"
#include "graph_mapper.hpp"
#include "hypothesis.hpp"
#include "measurement.hpp"
#include "options_v07.hpp"

namespace dof {

// Scales an entity's lens values so that their product becomes `target`, then
// states that product as the entity's DoF (§4.1).
//
// Setting the DoF to the product **as computed** is what makes the identity hold
// exactly: the harness never asserts a number the port would compute differently.
inline void v011_set_product(EntityState& entity, double target) {
    if (!entity.measurement) return;
    const std::optional<double> product = lens_product(entity);
    if (!product || *product <= 0.0 || target <= 0.0) return;
    const double ratio = std::pow(target / *product, 1.0 / static_cast<double>(lens_order().size()));
    std::vector<std::string> keys;
    for (const auto& kv : entity.measurement->psi_by_lens) keys.push_back(kv.first);
    for (const auto& key : keys) {
        auto it = entity.measurement->psi_by_lens.find(key);
        if (it != entity.measurement->psi_by_lens.end() && it->second) {
            it->second = *it->second * ratio;
        }
    }
    const std::optional<double> after = lens_product(entity);
    if (after) {
        entity.current_dof = *after;
        entity.measurement->current_dof = *after;
    }
}

// A copy of a state. C++ value semantics are deep, so a transformation never
// touches the original.
inline SystemStateMatrix v011_copy(const SystemStateMatrix& state) { return state; }

// Scales the declared Variety counter of every entity in a scene by `k`.
//
// The counter is a *measured* input, so scaling it produces a genuinely different
// reading, while the ruler — the procedures, the lens set, the units, the means
// class — is untouched. The resulting state's stated DoF is reconciled to the
// product of its own counters by `v011_reconcile`, which is what §4.1 requires of
// any reading.
inline std::unordered_map<std::string, RawObservation> v011_scaled_scene(
    const std::unordered_map<std::string, RawObservation>& scene, double k) {
    std::unordered_map<std::string, RawObservation> out;
    for (const auto& kv : scene) {
        RawObservation copied = kv.second;
        if (copied.lenses.variety) {
            copied.lenses.variety = std::make_pair(copied.lenses.variety->first * k,
                                                   copied.lenses.variety->second);
        }
        out[kv.first] = copied;
    }
    return out;
}

// States each entity's DoF as the product of its own counters, so that a mapped
// copy satisfies the lens identity of §4.1 by construction.
inline SystemStateMatrix v011_reconcile(const SystemStateMatrix& state) {
    SystemStateMatrix clone = v011_copy(state);
    std::vector<std::string> keys;
    for (const auto& kv : clone.entities) keys.push_back(kv.first);
    for (const auto& key : keys) {
        auto it = clone.entities.find(key);
        if (it == clone.entities.end()) continue;
        const std::optional<double> product = lens_product(it->second);
        if (!product) continue;
        it->second.current_dof = *product;
        if (it->second.measurement) it->second.measurement->current_dof = *product;
    }
    return clone;
}

// Sets one entity's DoF to `target` by scaling its own counters, which is the only
// way a reading may arrive at a different DoF (§4.1).
inline SystemStateMatrix v011_set_entity_dof(const SystemStateMatrix& state,
                                             const std::string& entity_id, double target) {
    SystemStateMatrix clone = v011_copy(state);
    auto it = clone.entities.find(entity_id);
    if (it != clone.entities.end()) v011_set_product(it->second, target);
    return clone;
}

// Removes every trace of τ: the resource-map observation is present but
// **unmeasured**, which is what §3.2b calls an unknown τ. Zeroing the fields
// instead would leave the deprecated mirror in place and read as a **measured** τ
// of zero — a passed deadline, not an unmeasured budget.
inline SystemStateMatrix v011_unknown_tau(const SystemStateMatrix& state) {
    SystemStateMatrix clone = v011_copy(state);
    ResourceObservation unmeasured;
    unmeasured.value = std::nullopt;
    unmeasured.unit = "mks";
    unmeasured.scale = 1.0;
    unmeasured.source = "sensor";
    unmeasured.last_measured_at = 0.0;
    unmeasured.aging_time = 0.0;
    unmeasured.estimated = std::nullopt;
    unmeasured.estimation_source.clear();
    clone.tau = unmeasured;
    clone.deadlines.clear();
    clone.global_time_to_collapse_mks = 0.0;
    return clone;
}

// Drops an entity from a copy: the state-completeness defect of §3.6.
inline SystemStateMatrix v011_without_entity(const SystemStateMatrix& state,
                                             const std::string& entity_id) {
    SystemStateMatrix clone = v011_copy(state);
    clone.entities.erase(entity_id);
    return clone;
}

// Adds an entity the observed state does not declare.
inline SystemStateMatrix v011_with_extra_entity(const SystemStateMatrix& state) {
    SystemStateMatrix clone = v011_copy(state);
    EntityState extra;
    extra.entity_id = "v011_extra";
    extra.is_autonomous = false;
    extra.current_dof = 0.0;
    extra.agency_index = 0.5;
    extra.is_collapse_source = false;
    extra.time_to_collapse_mks = 0.0;
    clone.entities["v011_extra"] = extra;
    return clone;
}

// States a DoF its own counters do not produce.
inline SystemStateMatrix v011_break_lens_product(const SystemStateMatrix& state,
                                                 const std::string& entity_id) {
    SystemStateMatrix clone = v011_copy(state);
    auto it = clone.entities.find(entity_id);
    if (it != clone.entities.end()) {
        it->second.current_dof = it->second.current_dof * 2.0 + 0.5;
    }
    return clone;
}

// Flips the collapse-source label of one entity: the defect §3.6 refuses, because
// the label is honoured through an act of the shared graph.
inline SystemStateMatrix v011_move_collapse_label(const SystemStateMatrix& state,
                                                  const std::string& entity_id) {
    SystemStateMatrix clone = v011_copy(state);
    auto it = clone.entities.find(entity_id);
    if (it != clone.entities.end()) {
        it->second.is_collapse_source = !it->second.is_collapse_source;
    }
    return clone;
}

// Gives a reading its own measurement durations: the defect §3.4.1/§3.4.2 refuse,
// because the durations are ruler-level.
inline SystemStateMatrix v011_change_durations(const SystemStateMatrix& state) {
    SystemStateMatrix clone = v011_copy(state);
    clone.measurement_durations["variety"] = {{"t_m", 123456.0}, {"t_v", 654321.0}};
    return clone;
}

// A candidate that adds `delta` to one entity, requires nothing and closes
// nothing — the simplest well-formed option, in the flat form of §3.3.
inline ActionOption v011_option(const std::string& id, const std::string& entity_id,
                                double delta) {
    ActionOption option;
    option.option_id = id;
    option.description = "v0.11 fixture candidate";
    option.projected_dof_delta[entity_id] = delta;
    option.is_reversible = true;
    option.estimated_duration_mks = 1000.0;
    return option;
}

// The `skip`-th entity id with a positive DoF, in sorted order, so the harness does
// not hard-code a fixture name for its arithmetic.
inline std::string v011_entity(const SystemStateMatrix& state, std::size_t skip) {
    std::vector<std::string> ids;
    for (const auto& kv : state.entities) ids.push_back(kv.first);
    std::sort(ids.begin(), ids.end());
    std::size_t left = skip;
    for (const auto& id : ids) {
        auto it = state.entities.find(id);
        if (it == state.entities.end()) continue;
        if (it->second.current_dof > 0.0) {
            if (left == 0) return id;
            --left;
        }
    }
    return std::string();
}

// The discriminating candidate of §6.3. Under `h_alt` it closes **two** means:
// trainee's own mean, which drives its Variety counter to zero, and the supervise
// mean, which removes the act path that would otherwise raise it back. Under the
// observed reading it closes nothing.
//
// The pair is what makes the check discriminate. With the closure declared only for
// `h_alt`, the correct after-state has trainee at `DoF = 0` with no raising path — a
// `proven_unreachable` verdict, so `D2 = 1` — while a port that recomputes the
// counters from `closure[$observed$]` (the empty list) leaves trainee at its observed
// `DoF > 0` and charges nothing. The released §4.5 scene is the world of this
// fixture, so no new scene is needed.
inline ActionOption v011_h_only_closer() {
    ActionOption o = options_v07::t1_mirror();
    o.option_id = "h_only_closer";
    o.projected_dof_delta.clear();
    o.projected_by_hypothesis = {
        {kObservedHypothesisId, {{"drone", 0.1}}},
        {"h_alt", {{"drone", 0.1}}},
    };
    o.closed.clear();
    o.closed_by_hypothesis = {
        {kObservedHypothesisId, {}},
        {"h_alt", options_v07::closures({fixture_v07::kTraineeMean,
                                         fixture_v07::kSuperviseMean})},
    };
    return o;
}

}  // namespace dof

#endif  // DOF_FIXTURE_V011_HPP
