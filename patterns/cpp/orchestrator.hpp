// DOF-Core Reactive Circuit with Interruption (C++ port).
// Ties the three layers; switches FAST PASS / DEEP by τ.

#pragma once

#include <optional>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

#include "dof_core.hpp"
#include "generator.hpp"
#include "graph_mapper.hpp"
#include "measurement.hpp"

class DOFOrchestrator {
public:
    double fast_pass_threshold = 5000000.0;  // microseconds (DOF-SPEC §5)

private:
    GraphMapper mapper_;
    Generator generator_;
    DOFCalculusCore core_;

    std::vector<ActionOption> generate(const SystemStateMatrix& state,
                                       const std::optional<double>& tau) const {
        // §3.2b (v0.11): an unknown budget selects the cheap path. An unknown
        // budget never licenses the expensive one — and reading the clamped mirror
        // instead would make an unknown τ indistinguishable from a passed one.
        if (!tau || *tau < fast_pass_threshold) {
            return generator_.safe_fallback(state, 1);
        }
        return generator_.synthesize(state, 5);
    }

    // §5's reactive-circuit mode, evaluated **once** on the observed τ (§4.10): the
    // threshold is deliberately not hypothesis-conditional, because its only
    // consequence is the mode and the Generator runs once.
    //
    // An unknown budget selects `FAST_PASS`: an unknown budget never licenses the
    // expensive path. This is the rule `tau_of`'s `null` feeds — reading the clamped
    // mirror instead would make an unknown τ indistinguishable from a passed one.
    std::string mode_for(const std::optional<double>& tau) const {
        if (!tau || *tau < fast_pass_threshold) return "FAST_PASS";
        return "DEEP_DIVERSIFICATION";
    }

    // §5: keep the options that can complete before τ and record every removal
    // — a removal is a decision and must be visible (§6.2).
    //
    // §3.2b (v0.11): with an **unmeasured** τ the gate cannot ask "does it fit" —
    // an unknown budget is not a licence for an action, and it is not a passed
    // deadline either. Only a measurement of τ resolves it, so with `null` τ the
    // gate keeps exactly the τ measurements and removes everything else.
    static std::pair<std::vector<ActionOption>, std::vector<RemovedOption>> viability_gate(
        const std::vector<ActionOption>& options, const std::optional<double>& tau) {
        std::vector<ActionOption> viable;
        std::vector<RemovedOption> removed;
        for (const auto& option : options) {
            if (!tau) {
                if (option_discovers(option, "tau")) {
                    viable.push_back(option);
                } else {
                    removed.push_back(RemovedOption{option.option_id, "viability"});
                }
                continue;
            }
            if (option.estimated_duration_mks <= *tau) {
                viable.push_back(option);
            } else {
                removed.push_back(RemovedOption{option.option_id, "viability"});
            }
        }
        return {viable, removed};
    }

public:
    explicit DOFOrchestrator(double context_switch_cost = 0.05)
        : mapper_(context_switch_cost) {}

    // The three layers, exposed read-only for the conformance harness: a check
    // must be able to ask what the mapper observed and what the core decided.
    const GraphMapper& mapper_ref() const { return mapper_; }
    const DOFCalculusCore& core_ref() const { return core_; }

    SystemStateMatrix measure(const std::unordered_map<std::string, RawObservation>& raw) const {
        return mapper_.poll_environment(raw);
    }

    std::optional<ActionOption> step(
        const std::unordered_map<std::string, RawObservation>& raw) const
    {
        SystemStateMatrix state = mapper_.poll_environment(raw);
        const ObservationContext* ctx = mapper_.last_observation ? &*mapper_.last_observation : nullptr;
        // §3.2b (v0.11): τ comes from the resource map — signed, `null` when
        // unmeasured. The deprecated mirror is never an input to a rule.
        const std::optional<double> tau = tau_of(state);
        auto gated = viability_gate(generate(state, tau), tau);
        // v0.8 retired the structural gate of §4.5: a charged candidate is no longer
        // removed from the set — it is evaluated, reported in full and barred by the
        // candidate-vector test.
        auto affordable = core_.apply_resource_gate(state, gated.first,
                                                    groups_ptr(), rates_ptr(),
                                                    weights_ptr(), cap_value());
        return core_.evaluate_and_select(state, affordable.first, ctx);
    }

    // Like step(), but also returns the Proof-of-Implementation audit.
    // Gate order is normative (§5 → §4.8): the reason a reader needs first is the
    // one about the world, not the one about the wallet. v0.8 retired the
    // structural gate of §4.5 — a charged candidate is evaluated, reported in full
    // and barred by the candidate-vector test of §4.5, so no removal is recorded
    // for it.
    std::pair<std::optional<ActionOption>, DofReport> step_with_report(
        const std::unordered_map<std::string, RawObservation>& raw) const
    {
        SystemStateMatrix state = mapper_.poll_environment(raw);
        const ObservationContext* ctx = mapper_.last_observation ? &*mapper_.last_observation : nullptr;
        // §3.2b (v0.11): τ is read from the resource map — signed, and `null` when
        // unmeasured. The deprecated mirror is never an input to a rule.
        const std::optional<double> tau = tau_of(state);
        const std::string mode = mode_for(tau);
        auto gated = viability_gate(generate(state, tau), tau);
        auto affordable = core_.apply_resource_gate(state, gated.first,
                                                    groups_ptr(), rates_ptr(),
                                                    weights_ptr(), cap_value());
        auto selected = core_.evaluate_and_select(state, affordable.first, ctx);
        std::vector<RemovedOption> all_removed = gated.second;
        all_removed.insert(all_removed.end(), affordable.second.begin(), affordable.second.end());

        // §6.2 (v0.7): where the amounts a decision rests on came from — a measured
        // balance or an asserted authority — so a reader can check the ceiling
        // against a measurement instead of against a claim.
        ReportInput in;
        in.declaration = mapper_.last_declaration;
        in.removed = all_removed;
        in.groups = groups_ptr();
        in.rates = rates_ptr();
        in.weights = weights_ptr();
        in.cap = cap_value();
        in.ctx = ctx;
        in.means_provenance["source"] = dof::MandateValue::str("measured balance (§4.8)");
        for (const auto& kv : state.resources) {
            in.means_provenance["measured:" + kv.first] = dof::MandateValue::num(resource_value(kv.second));
        }
        if (mapper_.last_declaration && mapper_.last_declaration->numeraire) {
            in.means_provenance["numeraire"] = dof::MandateValue::str(*mapper_.last_declaration->numeraire);
        }
        if (cap_value()) {
            in.means_provenance["mandate_cap"] = dof::MandateValue::num(*cap_value());
        }
        DofReport rep = core_.report(state, affordable.first, selected, mode, in);
        return {selected, rep};
    }

private:
    // The derived groups, observed rates, numeraire weights and mandate ceiling of
    // the ruler frozen on this cycle. They live in the declaration, so the gate and
    // the report see exactly the exchange layer the digest covers.
    const std::vector<std::vector<std::string>>* groups_ptr() const {
        return mapper_.last_declaration ? &mapper_.last_declaration->groups : nullptr;
    }
    const std::map<std::string, dof::Rate>* rates_ptr() const {
        return mapper_.last_declaration ? &mapper_.last_declaration->rates : nullptr;
    }
    const std::map<std::string, double>* weights_ptr() const {
        return mapper_.last_declaration ? &mapper_.last_declaration->weights : nullptr;
    }
    std::optional<double> cap_value() const {
        return mapper_.last_declaration ? mapper_.last_declaration->mandate_cap : std::nullopt;
    }
};
