// DOF-SPEC v0.11 §3.3/§4.8b — the temporal condition, stated once (C++ port).
// Mirrors patterns/python/calculus_core.py (`viability`, `derived_tau_delta`),
// patterns/go/viability.go and patterns/rust/viability.rs exactly.
//
// Nothing here takes a core instance: the rule reads the state and the option, and
// a rule that needs an object to be asked is a rule that can be asked twice with
// two objects.

#ifndef DOF_VIABILITY_HPP
#define DOF_VIABILITY_HPP

#include <optional>
#include <string>

#include "dof_core.hpp"

namespace dof {

// §4.8b's temporal condition, as a **condition** and not a removal: `v0.11`
// retired the pre-evaluation removal pipeline, so a candidate that fails this test
// is reported per option with the condition it failed rather than deleted from the
// set (§10(E)).
struct ViabilityResult {
    bool viable = false;
    std::optional<double> tau;
    std::optional<double> tau_after;
    std::optional<double> projected_tau_delta;
    std::string reason;
};

// §3.3/§4.8b: `projected_tau_delta` is **derived** for a τ measurement, never
// independently declared.
//
// The declared field is the whole projection for every other act. For an act that
// resolves τ it MUST equal `projected_tau_value − (τ − estimated_duration)` — `τ'`
// computed from the value the act expects to leave behind — and it MUST be
// `nullopt` when τ is unknown, because the difference is not computable; the
// measurement is still admitted and evaluated from `projected_tau_value` directly.
inline std::optional<double> derived_tau_delta(const SystemStateMatrix& state,
                                               const ActionOption& option) {
    if (!option_discovers(option, "tau")) return option.projected_tau_delta;
    const std::optional<double> tau = tau_of(state);
    if (!tau || !option.projected_tau_value) return std::nullopt;
    return *option.projected_tau_value - (*tau - option.estimated_duration_mks);
}

// §4.8b: `τ' = τ − estimated_duration_mks + projected_tau_delta >= 0` is the whole
// τ arithmetic of an action — the exchange duration is already inside the field
// (§3.3), so a second term would charge τ twice.
//
// Two cases are named apart:
//
//   * an act that resolves **τ** is viable iff it can complete (`τ >= t_m`, or `τ`
//     is unknown) **and** its declared `projected_tau_value` is live (`>= 0`);
//   * an act that resolves a **lens or another resource** is governed by §4.7 and
//     the strict window of §5, and with an unknown τ it is not viable at all: an
//     unknown budget is not a licence for an action, and it is not a passed
//     deadline either — only a measurement of τ resolves it.
//
// The old disjunction `τ >= t_m` OR `τ = null` is withdrawn: as a disjunction it
// admitted a measurement whose own declared projection is negative.
inline ViabilityResult viability(const SystemStateMatrix& state, const ActionOption& option) {
    const double d = option.estimated_duration_mks;
    const std::optional<double> tau = tau_of(state);
    const bool resolves_tau = option_discovers(option, "tau");
    const std::optional<double> delta = derived_tau_delta(state, option);
    if (resolves_tau) {
        const bool can_complete = !tau || *tau >= d;
        const bool live = option.projected_tau_value && *option.projected_tau_value >= 0.0;
        ViabilityResult r;
        r.viable = can_complete && live;
        r.tau = tau;
        r.tau_after = option.projected_tau_value;
        r.projected_tau_delta = delta;
        r.reason =
            "a τ measurement is viable iff it can complete and its declared result is live "
            "(§4.8b)";
        return r;
    }
    if (!tau) {
        ViabilityResult r;
        r.viable = false;
        r.projected_tau_delta = delta;
        r.reason = "τ is unmeasured; measure it first (§4.8b)";
        return r;
    }
    const double tau_after = *tau - d + delta.value_or(0.0);
    ViabilityResult r;
    r.viable = tau_after >= 0.0;
    r.tau = tau;
    r.tau_after = tau_after;
    r.projected_tau_delta = delta;
    r.reason = "τ' = τ − d + projected_tau_delta must be >= 0 (§4.8b)";
    return r;
}

}  // namespace dof

#endif  // DOF_VIABILITY_HPP
