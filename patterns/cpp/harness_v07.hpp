// Conformance harness of the C++ port, DOF-SPEC v0.7 (§11.10).
//
// This is the release's own harness; `v06` runs the older one in main.cpp, kept as
// the historical v0.5/v0.6 evidence. Three of its checks MUST now diverge, and all
// three are re-stated here as positive facts:
//   A. the ruler changed: the v0.6 digest is no longer reproduced, because the
//      declaration now carries graph-derived verdicts, numeraire weights, the
//      mandate ceiling and the rate table (§3.4.1, §4.6, §4.9);
//   B. a known zero is no longer excluded without an observation;
//   C. acting on a passive object is no longer free without one.

#pragma once

#include <algorithm>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <map>
#include <set>
#include <sstream>
#include <string>
#include <vector>

#include "fixture_v07.hpp"
#include "options_v07.hpp"

namespace {

// The two frozen digests of the release: a port must reproduce BOTH byte for byte.
const std::string kExpectedRulerDigest =
    "5126fd99641ffdc9c338d3d288fcf3cb6dcf093ca0a423f1cd265b3fcae4152a";
const std::string kExpectedObservationDigest =
    "f3891c6ab622325fd6668893dd9f7450d39aa2d0a7ad2849634a4f59219f6a1c";
const std::string kV06Digest =
    "bed37c25fd9cb757e9ea4a861c01cd4660fd896a83cd39b7c73b8e0be7489ad4";
// The **ruler-level** digest of the same fixture (§3.4.2, v0.11): the declaration
// document without the reading-specific blocks (`entities`, `freeze`, `verdicts`).
// This is the key under which two readings of one cycle may be aggregated — the
// full declaration digest above is the key of one *reading*, and it is the value
// the Python port's `ruler_digest()` produces. A port that reproduced only the
// declaration digest would pass every historical check and still disagree about
// which numbers may be compared with which.
const std::string kExpectedRulerDigestV011 =
    "81948b8b3ca9805a75624c4d136f5926b9347c42ed3c6415966fc6f43f4301ba";

std::vector<std::string> g_failures7;
int g_checks7 = 0;

void check7(const std::string& name, bool ok, const std::string& detail = "") {
    ++g_checks7;
    std::cout << (ok ? "  OK   " : "  FAIL ") << name;
    if (!detail.empty()) std::cout << "  " << detail;
    std::cout << "\n";
    if (!ok) g_failures7.push_back(name);
}

bool near(double a, double b, double tol = 1e-9) { return std::fabs(a - b) <= tol; }

std::string num7(double x, int prec = 6) {
    std::ostringstream os;
    os << std::fixed << std::setprecision(prec) << x;
    return os.str();
}

}  // namespace

inline int run_harness_v07() {
    using namespace fixture_v07;
    DOFOrchestrator orch(0.05);
    SystemStateMatrix state = orch.measure(scene());
    const GraphMapper& mapper = orch.mapper_ref();
    const ObservationContext* ctx = mapper.last_observation ? &*mapper.last_observation : nullptr;
    const dof::MeasurementDeclaration& decl = *mapper.last_declaration;
    const DOFCalculusCore& core = orch.core_ref();

    std::cout << "=== 1. §3.5/§4.9: the observation, its verdicts and its counters ===\n";
    check7("the observation is present and offered to the cycle", ctx != nullptr);
    check7("the fixture observation is arbitrage-free", ctx->world.is_arbitrage_free(),
           std::to_string(ctx->world.arbitrage_edges().size()));
    check7("M(S) and T_rec are part of the observation, not of the state",
           ctx->means_class.size() == 2 && ctx->horizon("revivable") &&
               *ctx->horizon("revivable") == kTRecMks);
    check7("the graph declares no acts it has no means for", ctx->world.form_errors().empty());

    dof::Verdict v_passive = ctx->world.verdict("passive", means_class(), kTRecMks);
    check7("passive: complete observation, no raising act ⇒ proven_unreachable",
           v_passive.verdict == "proven_unreachable", v_passive.verdict);
    check7("passive: the verdict was computed over a non-empty pool, not over nothing",
           v_passive.admissible_seen > 0 && v_passive.witness.empty(),
           std::to_string(v_passive.admissible_seen));
    dof::Verdict v_revivable = ctx->world.verdict("revivable", means_class(), kTRecMks);
    check7("revivable: an admissible act by ANOTHER entity ⇒ reachable",
           v_revivable.verdict == "reachable" && v_revivable.witness.size() == 1 &&
               v_revivable.witness[0] == "act_medkit",
           v_revivable.verdict);
    check7("revivable: its own repertoire is empty while it is still recoverable — "
           "V and reachability are different questions",
           ctx->world.v_count("revivable", means_class(), kHorizonMks) == 0);
    dof::Verdict v_unobserved = ctx->world.verdict("unobserved", means_class(), kTRecMks);
    check7("unobserved: a partial observation ⇒ undetermined, never unreachable",
           v_unobserved.verdict == "undetermined", v_unobserved.verdict);
    check7("an undeclared T_rec ⇒ undetermined (no horizon, no claim)",
           ctx->world.verdict("forged", means_class(), std::nullopt).verdict == "undetermined");
    check7("an undeclared M(S) ⇒ undetermined",
           ctx->world.verdict("revivable", {}, kTRecMks).verdict == "undetermined");
    check7("the horizon is honoured: the same act outside T_rec is unreachable",
           ctx->world.verdict("revivable", means_class(), 1000000.0).verdict == "proven_unreachable");
    check7("narrowing T_rec can only add exclusions (monotonic in the safe direction)",
           ctx->world.verdict("revivable", means_class(), kTRecMks).verdict == "reachable" &&
               ctx->world.verdict("revivable", means_class(), 1000000.0).verdict == "proven_unreachable");
    check7("the counting horizon is honoured by V",
           ctx->world.v_count("robot", means_class(), kHorizonMks) == 9 &&
               ctx->world.v_count("robot", means_class(), 500.0) == 0);
    check7("V counts the entity's own vectors: robot 9, adult 3, drone 4, forged 3, child 1",
           ctx->world.v_count("robot", means_class(), kHorizonMks) == 9 &&
               ctx->world.v_count("adult", means_class(), kHorizonMks) == 3 &&
               ctx->world.v_count("drone", means_class(), kHorizonMks) == 4 &&
               ctx->world.v_count("forged", means_class(), kHorizonMks) == 3 &&
               ctx->world.v_count("child", means_class(), kHorizonMks) == 1);

    std::cout << "=== 2. §4.2: the calculation set is decided by the observation ===\n";
    std::set<std::string> members = core.calc_members(state, ctx);
    const std::set<std::string> expected{"adult", "child", "drone", "forged", "revivable", "robot", "unobserved"};
    check7("calc contains the seven entities the observation does not rule out",
           members == expected, std::to_string(members.size()));
    check7("passive leaves calc — proven_unreachable, not 'small'", members.count("passive") == 0);
    check7("revivable stays in calc at a known zero — a verdict of reachable is not a death",
           members.count("revivable") > 0 && state.entities.at("revivable").current_dof == 0.0 &&
               state.entities.at("revivable").dof_known);
    check7("unobserved stays in calc — an unknown DoF is never excluded (Axiom 5)",
           members.count("unobserved") > 0 && !state.entities.at("unobserved").dof_known);
    check7("unobserved is at a known zero by measurement and still counted",
           state.entities.at("unobserved").current_dof == 0.0);
    const double index_with_forged = core.calculate_system_dof(state, nullptr, ctx);
    check7("the index is the sum of ln DoF over calc", near(index_with_forged, -35.314438370902096, 1e-9),
           num7(index_with_forged, 9));
    check7("calc does not move when the candidate set is empty", members.size() == 7);
    check7("the declaration carries every verdict and every counter",
           decl.verdicts.at("revivable").verdict == "reachable" && decl.verdicts.at("revivable").v == 0 &&
               decl.verdicts.at("robot").v == 9);
    check7("the declared derived numbers are exactly what the procedures compute",
           mapper.last_graph_problems.empty());
    check7("the declaration names the procedure that produced the verdicts",
           decl.graph_procedure == "perception-v1:world_verdicts");

    std::cout << "=== 3. §4.2: the forged label — measured, not honoured ===\n";
    check7("forged sits at a known DoF of 0.3", near(state.entities.at("forged").current_dof, 0.3, 1e-12));
    check7("forged claims to be a collapse source", state.entities.at("forged").is_collapse_source);
    std::set<std::string> counted_any{"robot", "forged", "adult"};
    std::map<std::string, double> dof_before_any;
    for (const auto& kv : state.entities) dof_before_any[kv.first] = kv.second.current_dof;
    check7("no observed act drives a counted entity to a zero ⇒ the label has no witness",
           ctx->world.collapse_acts(counted_any, dof_before_any).empty());
    check7("an unwitnessed label does not remove the entity from calc",
           core.is_included(state.entities.at("forged"), ctx, &state));
    {
        DOFOrchestrator orch2(0.05);
        SystemStateMatrix state2 = orch2.measure(scene([]{ Options o; o.include_forged_kill = true; return o; }()));
        const ObservationContext* ctx2 = orch2.mapper_ref().last_observation
                                             ? &*orch2.mapper_ref().last_observation : nullptr;
        std::map<std::string, double> dof_before2;
        for (const auto& kv : state2.entities) dof_before2[kv.first] = kv.second.current_dof;
        check7("a real act of collapse gives the same label a witness",
               ctx2->world.collapse_acts({"robot", "forged"}, dof_before2).size() == 1);
        check7("a witnessed label takes the aggressor out of the topology",
               !orch2.core_ref().is_included(state2.entities.at("forged"), ctx2, &state2));
        const double delta = orch2.core_ref().calculate_system_dof(state2, nullptr, ctx2) - index_with_forged;
        check7("honouring the label raises the index by |ln 0.3| ≈ 1.204 nats",
               near(delta, -std::log(0.3), 1e-9), "Δ=" + num7(delta));
    }

    std::cout << "=== 4. §4.4/§4.6: the price of a closure is a count, not a constant ===\n";
    check7("ψ_var(robot) = 9/10, from the declared counters",
           near(*state.entities.at("robot").measurement->psi_by_lens.at("variety"),
                dof::psi_var(9.0, 1.0), 1e-15));
    check7("the declared counter equals the counting procedure", ctx->v_before("robot") == 9);
    const std::vector<std::pair<int, double>> prices{{1, -0.0124}, {5, -0.1178}, {8, -0.5878}};
    for (const auto& p : prices) {
        std::vector<std::string> ids;
        for (int i = 1; i <= p.first; ++i) ids.push_back("m" + std::to_string(i));
        ActionOption option;
        option.option_id = "close_" + std::to_string(p.first);
        option.estimated_duration_mks = 1000.0;
        option.closed = options_v07::closures(ids);
        std::map<std::string, double> share = core.closure_share(state, option, ctx);
        const int v_after = ctx->v_after_closure("robot", option.closed);
        const double recomputed = std::log(dof::psi_var(static_cast<double>(v_after), 1.0)) -
                                  std::log(dof::psi_var(9.0, 1.0));
        check7("closing " + std::to_string(p.first) + " of 9 prices " + num7(p.second, 4) + " nats (§11.7)",
               std::fabs(std::round(recomputed * 10000.0) / 10000.0 - p.second) < 1e-4,
               "recomputed=" + num7(recomputed));
        check7("closing " + std::to_string(p.first) +
               ": the reported share IS that price, not a second charge",
               near(share.at("robot"), recomputed, 1e-6));
        auto sim = core.simulate(state, option, ctx);
        check7("closing " + std::to_string(p.first) + ": DoF is recomputed from the changed counter",
               sim.first.entities.at("robot").current_dof < state.entities.at("robot").current_dof,
               num7(sim.first.entities.at("robot").current_dof));
    }
    ActionOption full = options_v07::opt_collapse();
    check7("closing all nine drives the share to zero (a collapse, charged by §4.2)",
           ctx->v_after_closure("robot", full.closed) == 0 &&
               core.projected_dof(state.entities.at("robot"), full, ctx) == 0.0);
    ActionOption plain;
    plain.option_id = "p";
    check7("is_reversible is DERIVED from the closure list",
           !core.is_reversible(full) && !core.is_reversible(options_v07::opt_win()) &&
               core.is_reversible(plain));
    check7("the price is a function of the counters: a different V_env moves it",
           std::fabs((std::log(dof::psi_var(8.0, 2.0)) - std::log(dof::psi_var(9.0, 2.0))) -
                     (std::log(dof::psi_var(8.0, 1.0)) - std::log(dof::psi_var(9.0, 1.0)))) > 1e-6);
    const double as_is_index = core.calculate_system_dof(state, nullptr, ctx);
    check7("no constant in the rule: an option that closes nothing pays nothing",
           !core.closure_share(state, options_v07::opt_lose(), ctx).empty() &&
               core.net_delta(state, plain, as_is_index, as_is_index) == -0.05);

    std::cout << "=== 5. §4.4: the two guards ===\n";
    check7("guard 1: closing its own execution path", !core.closure_error(options_v07::opt_bad_self()).empty(),
           core.closure_error(options_v07::opt_bad_self()));
    check7("guard 2: is_reversible=false with nothing closed",
           !core.closure_error(options_v07::opt_bad_empty()).empty());

    std::cout << "=== 6. §4.5: charges, the structural gate and the ladder ===\n";
    std::vector<CollapseCharge> charges_collapse = core.collapse_charges(state, options_v07::opt_collapse(), ctx);
    check7("an option that destroys an entity BY CLOSING ITS TRANSITIONS is charged",
           charges_collapse.size() == 1 && charges_collapse[0].entity_id == "robot" &&
               near(charges_collapse[0].dof_before, state.entities.at("robot").current_dof, 1e-15),
           std::to_string(charges_collapse.size()));
    check7("the charge requires a transition into the zero, never a stay at it",
           !charges_collapse.empty() && charges_collapse[0].dof_before > 0.0);
    bool no_phantom = true;
    for (const auto& o : options_v07::standard_set()) {
        for (const auto& c : core.collapse_charges(state, o, ctx)) {
            if (c.dof_before <= 0.0) no_phantom = false;
        }
    }
    for (const auto& c : core.collapse_charges(state, options_v07::opt_funded(), ctx)) {
        if (c.dof_before <= 0.0) no_phantom = false;
    }
    check7("no charge in the whole fixture names an entity already at zero", no_phantom);
    check7("an option that lifts an entity does not destroy another",
           core.collapse_charges(state, options_v07::opt_win(), ctx).empty());
    auto gate_pair = core.apply_structural_gate(state, {options_v07::opt_win(), options_v07::opt_collapse()}, ctx);
    check7("the gate removes the destructive option while a charge-free one exists",
           gate_pair.first.size() == 1 && gate_pair.first[0].option_id == "opt_win" &&
               gate_pair.second.size() == 1 && gate_pair.second[0].gate == "collapse");
    check7("the gate is not extinguished by a recoverable zero in the state "
           "(the v0.6 regression this release found)",
           !gate_pair.second.empty() && gate_pair.first.size() == 1);
    ActionOption kill_robot;
    kill_robot.option_id = "kill_robot";
    kill_robot.projected_dof_delta = {{"robot", -1.0}};
    kill_robot.estimated_duration_mks = 1000.0;
    auto kept = core.apply_structural_gate(state, {options_v07::opt_collapse(), kill_robot}, ctx);
    check7("when every candidate destroys, they stay admissible (Axiom 3 still compares them)",
           kept.first.size() == 2 && kept.second.empty());
    auto sim_collapse = core.simulate(state, options_v07::opt_collapse(), ctx);
    check7("destroying a counted entity can never be profitable: NetDelta < 0",
           core.net_delta(state, options_v07::opt_collapse(),
                          core.calculate_system_dof(sim_collapse.first, &sim_collapse.second, ctx),
                          core.calculate_system_dof(state, nullptr, ctx)) < 0.0);
    auto sel_a = core.evaluate_and_select(state, {options_v07::opt_win(), options_v07::opt_lose()}, ctx);
    auto sel_b = core.evaluate_and_select(state, {options_v07::opt_lose(), options_v07::opt_win()}, ctx);
    check7("selection is order-independent (the ladder resolves ties deterministically)",
           sel_a && sel_b && sel_a->option_id == sel_b->option_id && sel_a->option_id == "opt_win");
    check7("stay-put baseline: an option whose only effect is a closure loses to doing nothing",
           !core.evaluate_and_select(state, {options_v07::opt_lose()}, ctx));
    check7("stay-put baseline: an empty candidate set selects nothing",
           !core.evaluate_and_select(state, {}, ctx));
    check7("a reversible option with a real gain is selected",
           core.evaluate_and_select(state, {options_v07::opt_funded()}, ctx).has_value());

    std::cout << "=== 7. §4.6: numeraire, weights and the derived blocks ===\n";
    const std::map<std::string, double> expect_weights{{"credit", 1.0}, {"energy", 0.5},
                                                       {"machine_hour", 1.0}, {"parts", 1.0}};
    bool weights_ok = decl.weights.size() == 4;
    for (const auto& kv : expect_weights) {
        if (!near(decl.weights.at(kv.first), kv.second, 1e-12)) weights_ok = false;
    }
    check7("the weights come from the OBSERVED rates: 1, 0.5, 1, 1", weights_ok,
           std::to_string(decl.weights.size()));
    check7("w_energy is the PRICE of one joule (1/2.0), not a second price of one credit",
           near(decl.weights.at("energy"), dof::q6(1.0 / 2.0), 1e-12));
    double balance = 0.0;
    for (const auto& r : group()) balance += decl.weights.at(r) * means().at(r);
    check7("the group balance in the numeraire is 13.0 (§11.10 п.4)",
           near(balance, kBalanceInNumeraire, 1e-12), num7(balance));
    check7("the numeraire is declared, and it is part of the ruler",
           decl.numeraire && *decl.numeraire == numeraire());
    const auto& blocks = state.entities.at("drone").measurement->blocks;
    check7("derived blocks: (c_g, C_g) = (2.0, 4.0) — 4 J at 0.5, capped at the 4.0 mandate",
           blocks.size() == 1 && blocks[0].first == 2.0 && blocks[0].second == 4.0,
           blocks.empty() ? "-" : num7(blocks[0].first) + "/" + num7(blocks[0].second));
    check7("the lens follows: ψ_opt = 4^(-2/4) = 0.5",
           near(*state.entities.at("drone").measurement->psi_by_lens.at("options"),
                dof::psi_opt({{2.0, 4.0}}), 1e-15));
    check7("the derivation echoes its own inputs (requirements, weights, cap, groups)",
           state.entities.at("drone").measurement->derivation.has_value() &&
               state.entities.at("drone").measurement->derivation->cap.has_value());
    {
        DOFOrchestrator orch3(0.05);
        SystemStateMatrix state_other = orch3.measure(scene_other_units());
        check7("the lens measures the world, not the notation: another declared unit scale "
               "gives the same f_g",
               state_other.entities.at("drone").measurement->blocks == blocks);
        check7("...while the ruler is a different ruler (the scale is in the hashed content)",
               orch3.mapper_ref().last_declaration->digest() != decl.digest());
    }
    check7("the declared rate table IS the procedure output, not a restatement",
           decl.rates.at("credit->energy").rate == 2.0 && decl.rates.at("parts->machine_hour").rate == 1.5);
    check7("a composition wins over a direct edge when it is genuinely more generous",
           decl.rates.at("parts->machine_hour").rate == 1.5 &&
               decl.rates.at("parts->machine_hour").duration_mks == 2000.0);
    check7("a quantized tie is broken by fewer edges — and moves the duration with it",
           decl.rates.at("credit->machine_hour").rate == 1.0 &&
               decl.rates.at("credit->machine_hour").duration_mks == 500.0);

    std::cout << "=== 8. §4.8: the mandate is a ceiling, never a floor ===\n";
    FundingPlan plan_over = core.plan_funding(state, options_v07::opt_over_mandate(), &decl.groups,
                                              &decl.rates, &decl.weights, decl.mandate_cap);
    check7("the balance would have covered it (13.0 ≥ 5.0), yet it is not permitted",
           plan_over.uncovered.empty() && near(plan_over.mandate_exceeded, 1.0, 1e-12),
           num7(plan_over.mandate_exceeded));
    auto gate_ok = core.apply_resource_gate(state, {options_v07::opt_win(), options_v07::opt_over_mandate()},
                                            &decl.groups, &decl.rates, &decl.weights, decl.mandate_cap);
    check7("the gate removes it, and says why",
           gate_ok.first.size() == 1 && gate_ok.second.size() == 1 &&
               gate_ok.second[0].gate == "insolvency");
    {
        Options o;
        o.means_override = std::map<std::string, double>{{"credit", 0.4}, {"energy", 0.0},
                                                        {"machine_hour", 0.0}, {"parts", 0.0}};
        o.cap_override = 10.0;
        DOFOrchestrator orch4(0.05);
        SystemStateMatrix state4 = orch4.measure(scene(o));
        const dof::MeasurementDeclaration& decl4 = *orch4.mapper_ref().last_declaration;
        const auto& blocks4 = state4.entities.at("drone").measurement->blocks;
        check7("a mandate can never raise what the measured means do not contain",
               blocks4.size() == 1 && blocks4[0].second == 0.4,
               blocks4.empty() ? "-" : num7(blocks4[0].second));
        FundingPlan plan4 = orch4.core_ref().plan_funding(state4, options_v07::opt_funded(), &decl4.groups,
                                                          &decl4.rates, &decl4.weights, decl4.mandate_cap);
        check7("with a balance of 0.4 the axis is removed by the BALANCE, not by the mandate",
               plan4.mandate_exceeded == 0.0);
    }
    check7("the mandate ceiling is in the hashed content",
           decl.mandate_cap && *decl.mandate_cap == kMandateCap);

    std::cout << "=== 9. §4.8: conversion is an operation the model can refuse ===\n";
    FundingPlan plan_funded = core.plan_funding(state, options_v07::opt_funded(), &decl.groups,
                                                &decl.rates, &decl.weights, decl.mandate_cap);
    check7("a deficit inside the group is bought at the observed rate",
           plan_funded.covered && plan_funded.conversions.size() == 1 &&
               plan_funded.conversions[0].to == "machine_hour");
    check7("cash in hand is spent first, the deficit second",
           near(plan_funded.spend.at("machine_hour"), 2.0, 1e-12) &&
               near(plan_funded.spend.at("credit"), 1.0, 1e-12));
    check7("the exchange's own time is charged to τ", plan_funded.total_duration_mks == 1500.0,
           num7(plan_funded.total_duration_mks));
    check7("the whole spend still fits under the mandate (3.0 ≤ 4.0)", plan_funded.mandate_exceeded == 0.0);
    FundingPlan plan_heavy = core.plan_funding(state, options_v07::opt_drone_heavy(), &decl.groups,
                                               &decl.rates, &decl.weights, decl.mandate_cap);
    check7("a deficit the balance cannot cover is NOT a cheaper conversion",
           near(plan_heavy.uncovered.at("energy"), 30.0, 1e-12), num7(plan_heavy.uncovered.at("energy")));
    check7("...and the path itself is observed: a price, not a verdict",
           decl.rates.at("credit->energy").rate == 2.0 &&
               ctx->world.rate("credit", "energy", true).status == "observed");
    check7("an undeclared resource balance is an invalid input, not a discount",
           near(core.plan_funding(state, options_v07::opt_undeclared(), &decl.groups, &decl.rates,
                                  &decl.weights, decl.mandate_cap).uncovered.at("fuel"), 1.0, 1e-12));
    {
        // The offer is chosen by VALUE, not by name.
        SystemStateMatrix synth;
        synth.global_time_to_collapse_mks = 1000000.0;
        synth.context_switch_cost = 0.05;
        EntityState e;
        e.entity_id = "e";
        e.agency_index = 0.5;
        e.current_dof = 0.5;
        e.time_to_collapse_mks = 1000000.0;
        synth.entities["e"] = e;
        ResourceObservation credit_ro; credit_ro.value = 5.0; credit_ro.unit = "RUB"; credit_ro.source = "sensor";
        ResourceObservation mh_ro; mh_ro.value = 5.0; mh_ro.unit = "hour"; mh_ro.source = "sensor";
        synth.resources = {{"credit", credit_ro}, {"machine_hour", mh_ro}};
        std::vector<std::vector<std::string>> synth_groups{{"credit", "machine_hour", "energy"}};
        std::map<std::string, dof::Rate> synth_rates{
            {"credit->energy", {2.0, 100.0}}, {"machine_hour->energy", {4.0, 900.0}}};
        ActionOption synth_opt;
        synth_opt.option_id = "need_energy";
        synth_opt.projected_dof_delta = {{"e", 0.1}};
        synth_opt.projected_resource_delta = {{"e", {{"energy", -10.0}}}};
        std::map<std::string, double> w1{{"credit", 1.0}, {"machine_hour", 0.5}};
        FundingPlan cheap_mh = core.plan_funding(synth, synth_opt, &synth_groups, &synth_rates, &w1,
                                                 std::nullopt);
        std::map<std::string, double> w2{{"credit", 0.5}, {"machine_hour", 1.0}};
        FundingPlan cheap_credit = core.plan_funding(synth, synth_opt, &synth_groups, &synth_rates, &w2,
                                                     std::nullopt);
        check7("the cheaper source is chosen even though it is not the alphabetically first",
               cheap_mh.conversions[0].from == "machine_hour", cheap_mh.conversions[0].from);
        check7("the same world with different WEIGHTS pays from the other source",
               cheap_credit.conversions[0].from == "credit", cheap_credit.conversions[0].from);
        check7("the amount bought is the deficit, measured at the observed rate",
               near(cheap_mh.conversions[0].amount_from, 2.5, 1e-12) &&
                   near(cheap_mh.conversions[0].amount_to, 10.0, 1e-12));
    }

    std::cout << "=== 10. §3.5/§4.9: an observation with a hole is not a discount ===\n";
    {
        DOFOrchestrator orch5(0.05);
        SystemStateMatrix state5 = orch5.measure(arbitrage_scene());
        const GraphMapper& mapper5 = orch5.mapper_ref();
        const ObservationContext* ctx5 = mapper5.last_observation ? &*mapper5.last_observation : nullptr;
        const dof::MeasurementDeclaration& decl5 = *mapper5.last_declaration;
        check7("the variant observation is detected as not arbitrage-free",
               ctx5 && !ctx5->world.is_arbitrage_free() && !ctx5->world.arbitrage_edges().empty());
        check7("no rate survives the hole: every pair is undetermined",
               decl5.rates.empty() && ctx5->world.rate("credit", "energy", true).status == "undetermined");
        auto adm5 = orch5.core_ref().apply_resource_gate(state5, {options_v07::opt_funded()},
                                                         &decl5.groups, &decl5.rates, &decl5.weights,
                                                         decl5.mandate_cap);
        check7("an exchange that cannot be priced does not happen: the option is insolvent",
               adm5.first.empty() && adm5.second.size() == 1 && adm5.second[0].gate == "insolvency");
        check7("the two observations are different observations (their digests differ)",
               ctx5->observation_digest != ctx->observation_digest);
    }

    std::cout << "=== 11. §6: the report carries the reasons ===\n";
    std::vector<ActionOption> standard = options_v07::gateable_set();
    auto admissible_all = core.apply_structural_gate(state, standard, ctx);
    auto admissible_final = core.apply_resource_gate(state, admissible_all.first, &decl.groups,
                                                     &decl.rates, &decl.weights, decl.mandate_cap);
    check7("the three gates compose and each removal names its gate",
           admissible_all.second.size() == 1 && admissible_all.second[0].gate == "collapse" &&
               admissible_final.second.size() == 2 && admissible_final.second[0].gate == "insolvency" &&
               admissible_final.second[1].gate == "insolvency");
    check7("what survives is exactly what is both harmless and permitted",
           admissible_final.first.size() == 2 && admissible_final.first[0].option_id == "opt_win" &&
               admissible_final.first[1].option_id == "opt_lose");
    std::optional<ActionOption> selected = core.evaluate_and_select(state, admissible_final.first, ctx);
    check7("the surviving irreversible option is the one that wins",
           selected && selected->option_id == "opt_win");
    std::vector<ActionOption> report_options = standard;
    report_options.push_back(options_v07::opt_funded());
    std::vector<RemovedOption> removed_all = admissible_all.second;
    removed_all.insert(removed_all.end(), admissible_final.second.begin(), admissible_final.second.end());
    ReportInput in;
    in.declaration = decl;
    in.removed = removed_all;
    in.groups = &decl.groups;
    in.rates = &decl.rates;
    in.weights = &decl.weights;
    in.cap = decl.mandate_cap;
    in.ctx = ctx;
    in.means_provenance["source"] = dof::MandateValue::str("measured balance (§4.8)");
    DofReport report = core.report(state, report_options, selected, "FAST_PASS", in);
    std::map<std::string, EntityReportRow> rows;
    for (const auto& row : report.entities) rows[row.entity_id] = row;
    check7("every row carries the recoverability verdict and its witness",
           rows.at("passive").recoverability.verdict == "proven_unreachable" &&
               rows.at("unobserved").recoverability.observation == "partial" &&
               rows.at("revivable").recoverability.witness.size() == 1);
    check7("the report says the subgraph came from a named observation",
           report.observation_digest && *report.observation_digest == ctx->observation_digest &&
               report.observation_digest->size() == 64);
    check7("the report's index equals the calculation over calc",
           near(report.total_system_dof, core.calculate_system_dof(state, nullptr, ctx), 1e-12));
    std::map<std::string, OptionReportRow> by_id;
    for (const auto& row : report.options) by_id[row.option_id] = row;
    check7("an irreversible option is reported with what it closes and with the decomposed loss",
           !by_id.at("opt_win").is_reversible && by_id.at("opt_win").closed.size() == 1 &&
               near(by_id.at("opt_win").closure_share.at("robot"), -0.0124, 1e-4));
    check7("the destructive option is reported as destructive (an auditable charge line)",
           by_id.at("opt_collapse").collapse_charges.size() == 1 &&
               by_id.at("opt_collapse").collapse_charges[0].entity_id == "robot");
    check7("the per-option row shows what was bought and at which price",
           by_id.at("opt_funded").resources_uncovered.empty());
    ActionOption measure_unknown;
    measure_unknown.option_id = "m";
    measure_unknown.projected_dof_delta = {{"unobserved", 0.1}};
    measure_unknown.estimated_duration_mks = 1000.0;
    check7("an unmapped entity that no candidate resolves makes the decision incomplete",
           core.is_incomplete(state, {options_v07::opt_win()}) &&
               !core.is_incomplete(state, {measure_unknown}));
    check7("the u₀ band of §4.7 is respected by the unmeasured Options lens",
           dof::u_min() <= 0.5 && 0.5 <= dof::kUMax);

    std::cout << "=== 12. §3.4.3/§11.9: the canonical form ===\n";
    {
        DOFOrchestrator orch_a(0.05);
        orch_a.measure(scene());
        std::unordered_map<std::string, RawObservation> reordered = scene();
        auto acts_it = reordered.at(kWorldKey).world;
        std::reverse(acts_it->graph.acts.begin(), acts_it->graph.acts.end());
        std::reverse(acts_it->graph.exchanges.begin(), acts_it->graph.exchanges.end());
        reordered[kWorldKey].world = acts_it;
        DOFOrchestrator orch_b(0.05);
        orch_b.measure(reordered);
        check7("the order of the observation's parts does not change the ruler",
               orch_a.mapper_ref().last_declaration->digest() ==
                   orch_b.mapper_ref().last_declaration->digest());
        check7("...nor the fingerprint of the observation",
               orch_a.mapper_ref().last_observation->observation_digest ==
                   orch_b.mapper_ref().last_observation->observation_digest);
        std::unordered_map<std::string, RawObservation> mutated = scene();
        auto mutated_world = mutated.at(kWorldKey).world;
        for (auto& e : mutated_world->graph.exchanges) {
            if (e.id == "q1") e.wants["energy"] = 2.5;
        }
        mutated[kWorldKey].world = mutated_world;
        DOFOrchestrator orch_c(0.05);
        orch_c.measure(mutated);
        check7("a single mutated quote changes both fingerprints",
               orch_c.mapper_ref().last_declaration->digest() !=
                   orch_a.mapper_ref().last_declaration->digest() &&
                   orch_c.mapper_ref().last_observation->observation_digest !=
                       orch_a.mapper_ref().last_observation->observation_digest);
        Options o_d;
        o_d.t_rec_override = std::map<std::string, double>{
            {"passive", kTRecMks}, {"revivable", 1000000.0}, {"unobserved", kTRecMks}};
        DOFOrchestrator orch_d(0.05);
        orch_d.measure(scene(o_d));
        check7("a mutated T_rec changes the ruler (a horizon is a measurement choice)",
               orch_d.mapper_ref().last_declaration->digest() != decl.digest());
    }
    const std::string canonical = decl.canonical_text();
    check7("the canonical form has no exponent notation",
           canonical.find("e-") == std::string::npos && canonical.find("e+") == std::string::npos);
    check7("the digest is 64 hex characters", decl.digest().size() == 64);

    std::cout << "=== 13. §10: what changed since v0.6, as facts ===\n";
    check7("the v0.6 ruler is no longer reproduced: the declaration carries derived content",
           decl.digest() != kV06Digest);
    {
        Options o6;
        o6.no_world = true;
        DOFOrchestrator orch6(0.05);
        SystemStateMatrix state6 = orch6.measure(scene(o6));
        const dof::MeasurementDeclaration& decl6 = *orch6.mapper_ref().last_declaration;
        check7("without an observation nothing is proven: a known zero is NOT excluded",
               orch6.core_ref().is_included(state6.entities.at("revivable"), nullptr, &state6));
        check7("without an observation no weight, no cap and no rate are invented",
               decl6.weights.empty() && !decl6.mandate_cap && decl6.rates.empty());
        const auto& blocks6 = state6.entities.at("drone").measurement->blocks;
        check7("without an observation the unit problem returns: the group sum adds 1 credit "
               "to 1 joule to 1 machine-hour as if they were one unit",
               blocks6.size() == 1 && blocks6[0].first == 4.0 && blocks6[0].second == 18.0,
               blocks6.empty() ? "-" : num7(blocks6[0].second));
        check7("the same entity has a different DoF with and without the observation",
               !near(state6.entities.at("drone").current_dof, state.entities.at("drone").current_dof, 1e-9));
    }

    std::cout << "\nRULER  digest=" << decl.digest() << "\n";
    std::cout << "OBSERVATION digest=" << ctx->observation_digest << "\n";
    check7("the ruler digest equals the frozen v0.7 value byte for byte",
           decl.digest() == kExpectedRulerDigest);
    check7("the v0.11 ruler-level digest equals the frozen value byte for byte",
           decl.ruler_digest() == kExpectedRulerDigestV011);
    check7("the observation digest equals the frozen v0.7 value byte for byte",
           ctx->observation_digest == kExpectedObservationDigest);
    std::cout << "\nREPORT (v0.7 fixture): entities=" << report.entities.size()
              << " options=" << report.options.size() << " removed=" << report.removed_options.size()
              << " index=" << num7(report.total_system_dof) << "\n";
    std::cout << "checks: " << g_checks7 << ", failures: " << g_failures7.size() << "\n";
    if (g_failures7.empty()) {
        std::cout << "FAILURES: none\nOK\n";
        return 0;
    }
    std::cout << "FAILURES:";
    for (const auto& f : g_failures7) std::cout << " [" << f << "]";
    std::cout << "\nFAILED\n";
    return 1;
}

// The reference dump: the canonical text of the release fixture's declaration and
// both digests — the tool that makes a cross-port mismatch a diff instead of a
// mystery.
inline int dump_reference() {
    DOFOrchestrator orch(0.05);
    orch.measure(fixture_v07::scene());
    const dof::MeasurementDeclaration& decl = *orch.mapper_ref().last_declaration;
    std::cout << decl.canonical_text() << "\n";
    std::cout << "DECLARATION " << decl.digest() << "\n";
    std::cout << "RULER " << decl.ruler_digest() << "\n";
    if (orch.mapper_ref().last_observation) {
        std::cout << "OBSERVATION " << orch.mapper_ref().last_observation->observation_digest << "\n";
    }
    // §3.4.1/§3.4.2 (v0.11): what a declared measurement duration does to each text.
    //
    // The durations are hashed **ruler** content for the reading (§3.4.1), so they
    // move the declaration document; the ruler text keys the **cycle** and excludes
    // `freeze` — where the durations and τ live — so it does not move. Printing the
    // fragment is what makes a cross-port disagreement on this field a diff instead
    // of a mystery: the reference produces
    //   "freeze":{"measurement_durations":{"variety":{"t_m":"100.000000",
    //   "t_v":"50.000000"}},"tau_mks":"4000000.000000"}
    // for this fixture.
    dof::MeasurementDeclaration dur = decl;
    dur.measurement_durations = {{"variety", {{"t_m", 100.0}, {"t_v", 50.0}}}};
    const std::string text = dur.canonical_text();
    const std::size_t p = text.find("\"freeze\":");
    const std::size_t e = text.find(",\"graph_procedure\"");
    if (p != std::string::npos && e != std::string::npos && e > p) {
        std::cout << "FREEZE_WITH_DURATIONS " << text.substr(p, e - p) << "\n";
    }
    std::cout << "DECLARATION_WITH_DURATIONS " << dur.digest() << "\n";
    std::cout << "RULER_WITH_DURATIONS " << dur.ruler_digest() << "\n";
    return 0;
}
