// DOF-Core Reactive Circuit with Interruption (C++ port).
// Ties the three layers; switches FAST PASS / DEEP by τ.

#pragma once

#include <optional>
#include <stdexcept>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

#include "dof_core.hpp"
#include "conditional.hpp"  // the v0.11 per-reading report (ReportV011) and the pass
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
        auto gated = gated_candidates(state);
        return core_.evaluate_and_select(state, gated.first, ctx);
    }

    // The candidate set of one cycle, after the §5 gate and the §4.8b resource gate,
    // with what each removed.
    //
    // One home for the gate order — normative at §5 → §4.8b: the reason a reader needs
    // first is the one about the world, not the one about the wallet. It is shared by
    // every entry point so the flat and the per-reading paths cannot diverge on
    // **which** options are evaluated; the mode and the candidate set are read from the
    // observed state (§4.10.5).
    //
    // v0.8 retired the structural gate of §4.5: a charged candidate is no longer
    // removed from the set — it is evaluated, reported in full and barred by the
    // candidate-vector test.
    std::pair<std::vector<ActionOption>, std::vector<RemovedOption>> gated_candidates(
        const SystemStateMatrix& state) const
    {
        // §3.2b (v0.11): τ comes from the resource map — signed, `null` when
        // unmeasured. The deprecated mirror is never an input to a rule.
        const std::optional<double> tau = tau_of(state);
        auto gated = viability_gate(generate(state, tau), tau);
        auto affordable = core_.apply_resource_gate(state, gated.first,
                                                    groups_ptr(), rates_ptr(),
                                                    weights_ptr(), cap_value());
        std::vector<RemovedOption> all_removed = gated.second;
        all_removed.insert(all_removed.end(), affordable.second.begin(), affordable.second.end());
        return {affordable.first, all_removed};
    }

    // §6.2 (v0.7): where the amounts a decision rests on came from — a measured balance
    // or an asserted authority — so a reader can check the ceiling against a measurement
    // instead of against a claim. One home for it: a report that spells it differently
    // in one entry point is the defect this prevents.
    void fill_means_provenance(const SystemStateMatrix& state, ReportInput& in) const
    {
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
        const std::string mode = mode_for(tau_of(state));
        auto gated = gated_candidates(state);
        auto selected = core_.evaluate_and_select(state, gated.first, ctx);

        ReportInput in;
        in.declaration = mapper_.last_declaration;
        in.removed = gated.second;
        in.groups = groups_ptr();
        in.rates = rates_ptr();
        in.weights = weights_ptr();
        in.cap = cap_value();
        in.ctx = ctx;
        fill_means_provenance(state, in);
        DofReport rep = core_.report(state, gated.first, selected, mode, in);
        return {selected, rep};
    }

    // §6.2/§6.3 (v0.11): the same cycle against a **declared hypothesis set**, with the
    // per-reading report — the entry a v0.11 consumer of this port calls.
    //
    // `hset == nullptr` (or an empty set) is the observed-state singleton of §4.10.6:
    // one reading, one entry in each map. It is deliberately not `step_with_report`,
    // which publishes the flat audit of §6.1–§6.3 with no per-reading surface at all —
    // two answers to two questions, and a port that cannot tell them apart cannot be
    // audited per reading.
    std::pair<std::optional<ActionOption>, dof::ReportV011> step_with_report_on_set(
        const std::unordered_map<std::string, RawObservation>& raw,
        const dof::HypothesisSet* hset) const
    {
        return decide_on_set(mapper_.poll_environment(raw), hset);
    }

    // The per-reading decision on an already measured state (see
    // `step_with_report_on_set`; the state is taken as a parameter for the same reason
    // `measure` exists).
    //
    // §4.10: one conditional pass, and it is the one published — the report is built
    // from the pass handed to it, never from a second computation that could describe a
    // decision nobody took.
    //
    // §3.6: a set whose readings are not §4.1-consistent is **refused**, not repaired.
    // There is no conformant decision to publish under such a set, and repairing it
    // silently would publish one under a reading nobody declared.
    std::pair<std::optional<ActionOption>, dof::ReportV011> decide_on_set(
        const SystemStateMatrix& state, const dof::HypothesisSet* hset) const
    {
        const ObservationContext* ctx = mapper_.last_observation ? &*mapper_.last_observation : nullptr;
        auto gated = gated_candidates(state);
        const std::vector<dof::Hypothesis> declared = dof::resolved_members(state, hset);
        const std::vector<std::string> errors = dof::validate_set(state, declared);
        if (!errors.empty()) {
            std::string message = "non-conformant hypothesis set:";
            for (const auto& e : errors) message += " " + e + ";";
            throw std::invalid_argument(message);
        }
        const std::vector<dof::Hypothesis> readings = dof::plausible_members(declared);
        // §4.10: one conditional pass, and it is the one published — and it runs even
        // when the cycle has no observation context. A scene without a graph is
        // decidable (the §4.9 verdicts read `undetermined`, priced by `u(t)`), so an
        // absent context changes what the quantities are, never whether the calculus
        // answers; refusing every candidate for want of a context would publish `none`
        // for a scene in which the reference selects an option (§7, §4.10.6).
        const auto choice = dof::select_conditional(core_, state, gated.first, readings, ctx,
                                                    groups_ptr(), rates_ptr(), weights_ptr(),
                                                    cap_value());
        const std::optional<ActionOption> decision = choice.first;
        const std::optional<dof::ConditionalSelection> selection = choice.second;
        ReportInput in;
        in.declaration = mapper_.last_declaration;
        in.removed = gated.second;
        in.groups = groups_ptr();
        in.rates = rates_ptr();
        in.weights = weights_ptr();
        in.cap = cap_value();
        in.ctx = ctx;
        fill_means_provenance(state, in);
        return dof::report_on_set(core_, state, gated.first, mode_for(tau_of(state)), hset, in,
                                  decision, selection);
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
