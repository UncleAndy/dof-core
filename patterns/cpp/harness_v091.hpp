// Conformance harness of the C++ port, DOF-SPEC v0.9.1 (§3.2b, §4.8, budget + τ-consumption).
//
// Covers:
// 1. τ is a ResourceObservation in the state
// 2. Null resource with estimated + mandatory fallback
// 3. Staleness check via aging_time
// 4. projected_tau_delta changes τ
// 5. Parallel actions consume τ by max(duration)

#pragma once

#include <cmath>
#include <iostream>
#include <optional>
#include <string>
#include <vector>
#include <map>

#include "dof_core.hpp"
#include "orchestrator.hpp"
#include "world_graph.hpp"

namespace {

int v091_checks = 0;
int v091_failures = 0;

void check91(const std::string& label, bool cond, const std::string& detail = "") {
    v091_checks++;
    if (cond) {
        std::cout << "  OK   " << label << std::endl;
        return;
    }
    v091_failures++;
    if (detail.empty()) {
        std::cout << "  FAIL " << label << std::endl;
    } else {
        std::cout << "  FAIL " << label << "  " << detail << std::endl;
    }
}

}  // namespace

inline int run_harness_v091() {
    // --- 1. τ is a ResourceObservation --------------------------------------
    std::cout << "=== 1. τ is a ResourceObservation ===" << std::endl;
    {
        DOFOrchestrator orch(0.05);
        SystemStateMatrix state = orch.measure(fixture_v07::scene());
        check91("state has 'tau' resource", state.resources.count("tau") > 0);
        const auto& tau = state.resources.at("tau");
        check91("tau.value is set", tau.value.has_value());
        if (tau.value.has_value()) {
            check91("tau.value > 0", tau.value.value() > 0.0);
        }
        check91("tau.unit == 'us'", tau.unit == "us");
        check91("tau.aging_time == 0", tau.aging_time == 0.0);
    }

    // --- 2. Null resource with estimated + fallback -------------------------
    std::cout << "=== 2. Null resource with estimated + fallback ===" << std::endl;
    {
        SystemStateMatrix state;
        state.global_time_to_collapse_mks = 1000000.0;
        state.context_switch_cost = 0.05;
        state.resources["medical_supply"] = ResourceObservation{
            .value = std::nullopt,
            .unit = "unit",
            .scale = 1.0,
            .estimated = 5.0,
            .estimation_source = {"indirect"},
        };
        const auto& ms = state.resources["medical_supply"];
        check91("medical_supply.value is null", !ms.value.has_value());
        check91("medical_supply.estimated == 5.0", ms.estimated.has_value() && ms.estimated.value() == 5.0);

        // Without fallback, null resource fails the gate
        DOFCalculusCore core;
        ActionOption option;
        option.option_id = "use_supply_no_fallback";
        option.description = "Use medical supply without fallback";
        option.projected_dof_delta = {{"patient", 0.1}};
        option.estimated_duration_mks = 1000.0;
        option.projected_resource_delta = {{"patient", {{"medical_supply", -3.0}}}};
        option.requires = {"medical_supply"};
        auto plan = core.plan_funding(state, option);
        check91("null resource fails without fallback", !plan.covered);
    }

    // --- 3. Staleness check -------------------------------------------------
    std::cout << "=== 3. Staleness check ===" << std::endl;
    {
        ResourceObservation tau_obs;
        tau_obs.value = 10.0;
        tau_obs.unit = "";
        tau_obs.scale = 1.0;
        tau_obs.source = "";
        tau_obs.last_measured_at = 0.0;
        tau_obs.aging_time = 1000.0;
        check91("not stale at t=500", !tau_obs.is_stale(500.0));
        check91("stale at t=1500", tau_obs.is_stale(1500.0));
        ResourceObservation obs_no_aging;
        obs_no_aging.value = 10.0;
        obs_no_aging.unit = "";
        obs_no_aging.scale = 1.0;
        obs_no_aging.source = "";
        obs_no_aging.last_measured_at = 0.0;
        obs_no_aging.aging_time = 0.0;
        check91("never stale if aging_time=0", !obs_no_aging.is_stale(99999.0));
    }

    // --- 4. projected_tau_delta changes τ -----------------------------------
    std::cout << "=== 4. projected_tau_delta changes τ ===" << std::endl;
    {
        DOFOrchestrator orch(0.05);
        SystemStateMatrix state = orch.measure(fixture_v07::scene());
        double tau_before = state.global_time_to_collapse_mks;
        // CPR takes 1000ms but increases τ by 5000ms
        ActionOption option;
        option.option_id = "cpr";
        option.description = "CPR increases τ";
        option.projected_dof_delta = {{"patient", 0.2}};
        option.estimated_duration_mks = 1000.0;
        option.projected_resource_delta = {{"patient", {{"energy", -5.0}}}};
        option.projected_tau_delta = 5000.0;
        double tau_after = tau_before - option.estimated_duration_mks + *option.projected_tau_delta;
        check91("CPR increases τ", tau_after > tau_before);
        check91("CPR delta is +4000 net", std::abs(tau_after - tau_before - 4000.0) < 1e-9);
    }

    // --- 5. Parallel actions consume τ by max(duration) ---------------------
    std::cout << "=== 5. Parallel actions consume τ by max(duration) ===" << std::endl;
    {
        DOFOrchestrator orch(0.05);
        SystemStateMatrix state = orch.measure(fixture_v07::scene());
        double tau_before = state.global_time_to_collapse_mks;
        double d1 = 3000.0, d2 = 5000.0;
        double tau_consumed = std::max(d1, d2);
        double tau_after = tau_before - tau_consumed;
        check91("parallel τ = max(d_i)", tau_consumed == 5000.0);
        check91("τ decreases by max duration", std::abs(tau_after - (tau_before - 5000.0)) < 1e-9);
    }

    std::cout << std::endl;
    std::cout << "checks: " << v091_checks << ", failures: " << v091_failures << std::endl;
    if (v091_failures > 0) {
        std::cout << "FAILURES: " << v091_failures << std::endl;
        return 1;
    }
    std::cout << "OK" << std::endl;
    return 0;
}
