// DOF-Core C++ SDK — entry point / smoke test.
// Mirrors patterns/smoke_test.py: the same facts on the same fixture, plus a
// check that the canonical declaration digest matches the other ports.

#include <cmath>
#include <iostream>
#include <map>
#include <string>
#include <unordered_map>
#include <vector>

#include "orchestrator.hpp"
#include "harness_v07.hpp"
#include "harness_v08.hpp"
#include "harness_v091.hpp"
#include "harness_v011.hpp"

// Two harnesses live in this port and both stay runnable, because a release must
// carry its own evidence and the previous release's:
//
//   ./dof_cpp v07     # the release's conformance suite (harness_v07.hpp), default
//   ./dof_cpp v06     # the v0.6 harness (this file), historical evidence
//   ./dof_cpp dump    # the canonical text and both frozen digests
//
// `v07` is the default so that any tool that builds and runs the port without
// arguments exercises the current release.

namespace {

const std::string kExpectedDigest =
    "bed37c25fd9cb757e9ea4a861c01cd4660fd896a83cd39b7c73b8e0be7489ad4";

std::vector<std::string> g_failures;

void check(const std::string& name, bool ok, const std::string& detail = "") {
    std::cout << (ok ? "  OK   " : "  FAIL ") << name;
    if (!detail.empty()) std::cout << "  " << detail;
    std::cout << "\n";
    if (!ok) g_failures.push_back(name);
}

std::string num(double x, int prec = 12) {
    std::ostringstream os;
    os << std::fixed << std::setprecision(prec) << x;
    return os.str();
}

RawObservation obs(double agency, bool collapse, double ttc, dof::LensObservation lenses) {
    RawObservation o;
    o.is_autonomous = true;
    o.agency_index = agency;
    o.is_collapse_source = collapse;
    o.time_to_collapse_mks = ttc;
    o.lenses = std::move(lenses);
    return o;
}

// The shared fixture: the same five entities as the other ports, including a
// passive object and an entity whose Options lens was never measured.
std::unordered_map<std::string, RawObservation> fixture() {
    std::unordered_map<std::string, RawObservation> m;

    dof::LensObservation adult;
    adult.variety = std::make_pair(3.0, 2.0);
    adult.options = std::vector<std::pair<double, double>>{{1.0, 10.0}};
    adult.constraint = std::make_pair(4.0, 1.0);
    m["adult"] = obs(0.9, false, 100000000.0, adult);

    dof::LensObservation child;
    child.variety = std::make_pair(1.0, 5.0);
    child.options = std::vector<std::pair<double, double>>{{2.0, 4.0}};
    child.constraint = std::make_pair(1.0, 3.0);
    m["child"] = obs(0.1, false, 4000000.0, child);

    dof::LensObservation aggressor;
    aggressor.variety = std::make_pair(5.0, 1.0);
    aggressor.options = std::vector<std::pair<double, double>>{{1.0, 100.0}};
    aggressor.constraint = std::make_pair(5.0, 1.0);
    m["aggressor"] = obs(0.5, true, 100000000.0, aggressor);

    dof::LensObservation stone;  // passive object: no responses, no budget, no free variables
    stone.variety = std::make_pair(0.0, 0.0);
    stone.options = std::vector<std::pair<double, double>>{};
    stone.constraint = std::make_pair(0.0, 0.0);
    m["stone"] = obs(0.0, false, 100000000.0, stone);

    dof::LensObservation unmapped;  // the Options lens was never measured
    unmapped.variety = std::make_pair(2.0, 2.0);
    unmapped.constraint = std::make_pair(1.0, 1.0);
    m["unmapped"] = obs(0.4, false, 100000000.0, unmapped);

    // v0.6: this entity declares what its transitions *require*, not the blocks
    // themselves — the blocks are derived against the agent's means (§4.6).
    dof::LensObservation drone;
    drone.variety = std::make_pair(4.0, 2.0);
    drone.requirements = std::map<std::string, double>{{"energy", 4.0}};
    drone.constraint = std::make_pair(3.0, 1.0);
    m["drone"] = obs(0.6, false, 100000000.0, drone);

    // §3.2/§4.8 (v0.6): the acting agent, the derived groups, the observed rates,
    // the declared units and the mandate. Part of the ruler: the declaration
    // carries it, so a ruler with different units is a different ruler.
    ResourceLayer layer;
    layer.means = {{"credit", 6.0}, {"energy", 10.0}};
    layer.groups = {{"credit", "energy"}};
    layer.rates["credit->energy"] = dof::Rate{2.0, 1000.0};
    layer.resources = {{"credit", "credit", 1.0}, {"energy", "joule", 1.0}};
    layer.mandate["external_limit_credit"] = dof::MandateValue::num(100.0);
    layer.mandate["scope"] = dof::MandateValue::str("household");
    RawObservation layer_obs;
    layer_obs.resource_layer = layer;
    m[kResourceLayerKey] = layer_obs;

    return m;
}

std::unordered_map<std::string, RawObservation> with_deadline(
    const std::unordered_map<std::string, RawObservation>& src, double ttc)
{
    std::unordered_map<std::string, RawObservation> copy = src;
    for (auto& kv : copy) {
        if (kv.first == kResourceLayerKey) continue;
        kv.second.time_to_collapse_mks = ttc;
    }
    return copy;
}

}  // namespace

// The v0.6 harness (historical evidence). It is kept runnable on purpose and now
// shows three DOCUMENTED divergences that v0.7 makes deliberate (see
// harness_v07.hpp section 13): the ruler digest moved, a known zero is no longer
// excluded without an observation, and acting on a passive object is no longer
// free without one. It is the historical record, not the release's conformance
// suite — that is `run_harness_v07`.
int run_harness_v06() {
    g_failures.clear();
    DOFOrchestrator orch(0.05);
    SystemStateMatrix state = orch.measure(fixture());
    DOFCalculusCore core;
    auto [selected, report] = orch.step_with_report(fixture());

    std::cout << "=== 1. §3.4.3: the canonical ruler ===\n";
    check("digest matches the Python port", state.psi && state.psi->digest == kExpectedDigest,
          state.psi ? state.psi->digest.substr(0, 16) + "…" : "no psi");
    check("fixture 1 selected an option", selected.has_value(),
          selected ? selected->option_id : "");

    std::cout << "=== 2. §4.1 / §4.6: per-entity values (reference: Python port) ===\n";
    const std::map<std::string, std::pair<double, double>> expected = {
        {"adult", {0.417864270382, -0.872598611192}},
        {"aggressor", {0.684883822565, -0.378506057199}},
        {"child", {0.020833333333, -3.871201010908}},
        {"drone", {0.353553390593, -1.03972077084}},
        {"stone", {0.000000000000, -13.815510557964}},
        {"unmapped", {0.125000000000, -2.079441541680}},
    };
    for (const auto& kv : expected) {
        const EntityState& ent = state.entities.at(kv.first);
        const dof::EntityMeasurement& m = *ent.measurement;
        check(kv.first + ": current_dof = lens product, contribution",
              std::fabs(ent.current_dof - kv.second.first) < 1e-9 &&
                  std::fabs(m.contribution - kv.second.second) < 1e-9,
              "dof=" + num(ent.current_dof) + " contrib=" + num(m.contribution));
        if (!m.floored) {
            check(kv.first + ": Σ terms == contribution",
                  std::fabs(m.terms_sum - m.contribution) < 1e-12);
        }
    }

    std::cout << "=== 3. §4.6 guard and §4.2 exclusion (passive object) ===\n";
    const EntityState& stone = state.entities.at("stone");
    check("stone: ψ_var = 0, no 0/0", stone.measurement->psi_by_lens.at("variety").value_or(-1.0) == 0.0);
    check("stone: current_dof = 0", stone.current_dof == 0.0);
    check("stone: no NaN in the index", !std::isnan(report.total_system_dof));
    check("stone: excluded when nothing can raise it (§4.2)",
          !core.is_included(stone));
    check("stone: floored flag is set", stone.measurement->floored);

    std::cout << "=== 4. §4.7: unmeasured lens ===\n";
    const EntityState& unmapped = state.entities.at("unmapped");
    check("unmapped: dof_known = false", !unmapped.dof_known);
    check("unmapped: never excluded (§4.2)",
          core.is_included(unmapped));
    int unknown_terms = 0;
    for (const auto& term : unmapped.measurement->terms) {
        if (!term.dof_known) {
            ++unknown_terms;
            check("unmapped: the unmeasured term costs ln u₀",
                  std::fabs(term.contribution - std::log(0.5)) < 1e-12);
        }
    }
    check("unmapped: exactly one unmeasured term of three", unknown_terms == 1);
    check("u₀ band respected", dof::u_min() <= 0.5 && 0.5 <= dof::kUMax,
          "U_MIN=" + num(dof::u_min(), 4) + " U_MAX=" + num(dof::kUMax, 4));

    std::cout << "=== 5. §5: viability gate, and both reactive modes ===\n";
    auto slow = orch.step_with_report(with_deadline(fixture(), 500.0));
    check("τ < option duration → removed and nothing selected",
          !slow.first.has_value() && slow.second.removed_options.size() == 1 &&
              slow.second.removed_options[0].option_id == "fallback_0" &&
              slow.second.removed_options[0].gate == "viability");
    check("fixture 1 runs in FAST_PASS", report.mode == "FAST_PASS",
          "τ=" + num(report.global_time_to_collapse_mks, 0));
    auto deep = orch.step_with_report(with_deadline(fixture(), 100000000.0));
    check("fixture 2 runs in DEEP_DIVERSIFICATION", deep.second.mode == "DEEP_DIVERSIFICATION");
    check("psi_id and digest are echoed in the report",
          deep.second.psi_id == "perception-v1" && deep.second.psi_digest.size() == 64);

    std::cout << "=== 6. §4.2/§4.5 (v0.5): frozen calc set, collapse charge, gate, stay-put ===\n";
    const EntityState& adult = state.entities.at("adult");
    double total_before = report.total_system_dof;
    ActionOption killer;
    killer.option_id = "kill_adult";
    killer.description = "liquidate the counted adult";
    killer.projected_dof_delta = {{"adult", -1.0}, {"unmapped", 0.0}};
    killer.is_reversible = true;
    killer.estimated_duration_mks = 1000.0;
    auto charges = core.collapse_charges(state, killer);
    check("charge: the destroyed entity is named with its DoF before the option",
          charges.size() == 1 && charges[0].entity_id == "adult" &&
              charges[0].dof_before == adult.current_dof,
          "charges=" + std::to_string(charges.size()));
    auto sim_kill = core.simulate(state, killer);
    double projected_kill = core.calculate_system_dof(sim_kill.first, &sim_kill.second);
    double expected_kill = total_before - std::log(adult.current_dof) + std::log(dof::kEpsilon);
    check("charge: the term stays at the floor instead of disappearing",
          std::abs(projected_kill - expected_kill) < 1e-9,
          "Δ=" + num(projected_kill - total_before, 4) + " nats");
    check("charge: destroying a counted entity can never raise the index",
          projected_kill < total_before);
    ActionOption passive;
    passive.option_id = "raise_stone";
    passive.description = "act on a passive object";
    passive.projected_dof_delta = {{"stone", 1.0}, {"unmapped", 0.0}};
    passive.is_reversible = true;
    passive.estimated_duration_mks = 1000.0;
    auto sim_passive = core.simulate(state, passive);
    check("frozen set: a passive object is neither charged nor rewarded",
          core.collapse_charges(state, passive).empty() &&
              std::abs(core.calculate_system_dof(sim_passive.first, &sim_passive.second) - total_before) < 1e-12);

    ActionOption spare;
    spare.option_id = "rescue_child";
    spare.description = "raise the weakest counted entity";
    spare.projected_dof_delta = {{"child", 0.2}, {"unmapped", 0.0}};
    spare.is_reversible = true;
    spare.estimated_duration_mks = 1000.0;
    auto gated_struct = core.apply_structural_gate(state, {killer, spare});
    check("structural gate: the destructive option is removed while a charge-free one exists",
          gated_struct.first.size() == 1 && gated_struct.first[0].option_id == "rescue_child" &&
              gated_struct.second.size() == 1 && gated_struct.second[0].gate == "collapse",
          "removed=" + std::to_string(gated_struct.second.size()));
    check("Axiom 3: the charge alone already makes destruction unprofitable",
          !core.evaluate_and_select(state, {killer}).has_value());
    auto only_destructive = core.apply_structural_gate(state, {killer});
    check("structural gate: when every candidate destroys, they stay admissible",
          only_destructive.first.size() == 1);
    ActionOption harm;
    harm.option_id = "harm_child";
    harm.description = "degrade the child";
    harm.projected_dof_delta = {{"child", -1.0}, {"unmapped", 0.0}};
    harm.is_reversible = true;
    harm.estimated_duration_mks = 1000.0;
    check("stay-put baseline: an all-negative candidate set selects nothing",
          !core.evaluate_and_select(state, {harm}).has_value() &&
              !core.evaluate_and_select(state, {}).has_value());
    check("fixture 1: a strictly positive option is selected", selected.has_value());

    std::cout << "=== 7. §4.7 (v0.5): coverage and completeness of unmapped entities ===\n";
    Generator gen;
    auto generated = gen.safe_fallback(state, 3);
    bool covered = true;
    for (const auto& o : generated) {
        if (o.projected_dof_delta.find("unmapped") == o.projected_dof_delta.end()) covered = false;
    }
    check("coverage: every candidate names the unmapped entity", covered);
    check("completeness: the fallback leaves a resolvable unknown unmeasured ⇒ incomplete",
          report.incomplete);
    ActionOption measuring;
    measuring.option_id = "measure_unmapped";
    measuring.description = "resolve the unknown";
    measuring.projected_dof_delta = {{"unmapped", 0.1}};
    measuring.is_reversible = true;
    measuring.estimated_duration_mks = 1000.0;
    check("completeness: a candidate that resolves the unknown clears the flag",
          !core.is_incomplete(state, {measuring}));

    std::cout << "=== 8. draft §6, example 1: product collapses where a sum would mask it ===\n";
    double before = dof::psi_var(9.0, 1.0) * dof::psi_opt({{1.0, 10.0}}) * dof::psi_con(9.0, 1.0);
    double after = dof::psi_var(19.0, 1.0) * dof::psi_opt({{5.0, 1.0}}) * dof::psi_con(9.0, 1.0);
    double sum_before = dof::psi_var(9.0, 1.0) + dof::psi_opt({{1.0, 10.0}}) + dof::psi_con(9.0, 1.0);
    double sum_after = dof::psi_var(19.0, 1.0) + dof::psi_opt({{5.0, 1.0}}) + dof::psi_con(9.0, 1.0);
    check("product collapses (ΔIndex ≈ " + num(std::log(after / before), 2) + " nats)",
          after / before < 0.01, "×" + num(after / before, 5));
    check("a sum would mask it", sum_after / sum_before > 0.6,
          "×" + num(sum_after / sum_before, 3));

    std::cout << "=== 9. §4.6/§4.8 (v0.6): derived blocks, the ruler, the resource gate ===\n";
    const EntityState& drone = state.entities.at("drone");
    check("drone: the blocks are derived from requirements + means in one group",
          drone.measurement->blocks.size() == 1 &&
              drone.measurement->blocks[0].first == 4.0 &&
              drone.measurement->blocks[0].second == 16.0,
          "blocks=[(" + num(drone.measurement->blocks[0].first, 0) + "," +
              num(drone.measurement->blocks[0].second, 0) + ")]");
    check("drone: the derivation names the procedure and its raw inputs",
          drone.measurement->derivation.has_value() &&
              drone.measurement->derivation->procedure == "derive_blocks" &&
              drone.measurement->derivation->requirements.at("energy") == 4.0 &&
              drone.measurement->derivation->means.at("energy") == 10.0);
    check("drone: ψ_opt equals psi_opt on the derived blocks",
          std::fabs(drone.measurement->psi_by_lens.at("options").value_or(-1.0) -
                    dof::psi_opt(drone.measurement->blocks)) < 1e-15);
    auto singleton = dof::derive_blocks({{"fuel", 2.0}}, {{"fuel", 4.0}}, {{"credit", "energy"}});
    check("derived: a resource in no declared group forms a singleton block",
          singleton.size() == 2 && singleton[0].first == 0.0 && singleton[0].second == 0.0 &&
              singleton[1].first == 2.0 && singleton[1].second == 4.0);
    const std::string decl = report.declaration;
    check("ruler: units, groups, rates, mandate and the derivation are hashed",
          decl.find("\"groups\":[[\"credit\",\"energy\"]]") != std::string::npos &&
              decl.find("\"credit->energy\":{\"duration_mks\":\"1000.000000\",\"rate\":\"2.000000\"}") != std::string::npos &&
              decl.find("{\"id\":\"energy\",\"scale\":\"1.000000\",\"unit\":\"joule\"}") != std::string::npos &&
              decl.find("\"external_limit_credit\":\"100.000000\"") != std::string::npos &&
              decl.find("\"options_blocks\":\"perception-v1:derive_blocks\"") != std::string::npos &&
              decl.find("\"requirements\":{\"energy\":\"4.000000\"}") != std::string::npos,
          "declaration " + std::to_string(decl.size()) + " chars");
    check("ruler: tau is a ResourceObservation in the state",
          decl.find("\"id\":\"tau\"") != std::string::npos || decl.find("\"resource_id\":\"tau\"") != std::string::npos);
    auto other = fixture();
    for (auto& r : other[kResourceLayerKey].resource_layer->resources) {
        if (r.id == "energy") { r.unit = "kilojoule"; r.scale = 1000.0; }
    }
    SystemStateMatrix state_other = orch.measure(other);
    check("ruler: the same resource at another scale is a different digest",
          state_other.psi && state.psi && state_other.psi->digest != state.psi->digest);

    const std::vector<std::vector<std::string>> groups{{"credit", "energy"}};
    const std::map<std::string, dof::Rate> rates{{"credit->energy", dof::Rate{2.0, 1000.0}}};
    auto draw = [](const std::string& id, const std::string& res, double amount, double duration = 1000.0) {
        ActionOption o;
        o.option_id = id;
        o.description = id;
        o.projected_dof_delta = {{"child", 0.1}, {"unmapped", 0.0}};
        o.is_reversible = true;
        o.estimated_duration_mks = duration;
        o.projected_resource_delta = {{"child", {{res, -amount}}}};
        return o;
    };
    ActionOption direct = draw("direct", "energy", 2.0);
    ActionOption funded = draw("funded", "energy", 12.0);
    ActionOption no_time = draw("no_time_for_trade", "energy", 12.0, 4000000.0);
    ActionOption undeclared = draw("undeclared", "fuel", 1.0);
    ActionOption offset = draw("offset", "energy", 3.0);
    offset.projected_resource_delta["adult"] = {{"energy", 1.0}};

    FundingPlan plan_direct = core.plan_funding(state, direct, &groups, &rates);
    check("step 1: means cover the draw ⇒ payable, nothing converted",
          plan_direct.covered && plan_direct.spend.at("energy") == 2.0 &&
              plan_direct.conversions.empty());
    FundingPlan plan_funded = core.plan_funding(state, funded, &groups, &rates);
    check("step 2: the deficit is bought at the observed rate",
          plan_funded.covered && plan_funded.conversions.size() == 1 &&
              plan_funded.conversions[0].from == "credit" &&
              std::fabs(plan_funded.conversions[0].amount_from - 1.0) < 1e-12 &&
              std::fabs(plan_funded.conversions[0].amount_to - 2.0) < 1e-12 &&
              plan_funded.conversions[0].rate == 2.0);
    check("step 2: only the deficit is traded (cash in hand is spent first)",
          plan_funded.spend.at("credit") == 1.0 && plan_funded.spend.at("energy") == 10.0);
    check("step 2: the exchange's own time is charged to τ",
          plan_funded.total_duration_mks == 2000.0);
    check("step 3: an exchange that does not fit in τ leaves the deficit uncovered",
          !core.plan_funding(state, no_time, &groups, &rates).covered);
    check("step 3: a resource whose balance is not declared cannot be bought",
          core.plan_funding(state, undeclared, &groups, &rates).uncovered.at("fuel") == 1.0);
    check("production offsets consumption (the net draw decides)",
          core.requirement(offset).at("energy") == 2.0);
    auto broke = fixture();
    broke[kResourceLayerKey].resource_layer->means = {{"credit", 0.4}, {"energy", 0.0}};
    SystemStateMatrix state_broke = orch.measure(broke);
    check("step 3: a price the agent cannot pay is not a cheaper price",
          !core.plan_funding(state_broke, funded, &groups, &rates).covered);
    auto gated_res = core.apply_resource_gate(state, {direct, funded, undeclared, offset}, &groups, &rates);
    check("gate: the unpayable option is removed with gate = insolvency",
          gated_res.first.size() == 3 && gated_res.second.size() == 1 &&
              gated_res.second[0].option_id == "undeclared" && gated_res.second[0].gate == "insolvency");

    std::cout << "=== 10. §6.2/§6.3 (v0.6): the spend is auditable ===\n";
    check("report: resources_before is the agent's means at the start of the cycle",
          report.resources_before.size() == 3 && report.resources_before.at("credit").value == 6.0 &&
              report.resources_before.at("energy").value == 10.0);
    check("report: the deterministic fallback buys nothing, so the stock is unchanged",
          resource_map_equal(report.resources_after, report.resources_before));
    ReportInput in_funded;
    in_funded.groups = &groups;
    in_funded.rates = &rates;
    DofReport rep_funded = core.report(state, {funded}, funded, "FAST_PASS", in_funded);
    check("report: buying a deficit debits the resource that actually paid",
          rep_funded.resources_after.at("credit").value == 5.0 &&
              rep_funded.resources_after.at("energy").value == 0.0);
    check("report: the per-option row carries the draw and the conversions applied",
          rep_funded.options.size() == 1 &&
              rep_funded.options[0].conversion_applied.size() == 1 &&
              rep_funded.options[0].resource_consumption.at("child").at("energy") == -12.0);
    ReportInput in_undeclared;
    in_undeclared.groups = &groups;
    in_undeclared.rates = &rates;
    DofReport rep_undeclared = core.report(state, {undeclared}, std::nullopt, "FAST_PASS", in_undeclared);
    check("report: an uncovered deficit is written down per option",
          rep_undeclared.options[0].resources_uncovered.at("fuel") == 1.0);

    std::cout << "\nREPORT (fixture 1): mode=" << report.mode
              << " total_dof=" << num(report.total_system_dof)
              << " psi_id=" << report.psi_id
              << " digest=" << report.psi_digest.substr(0, 16) << "…"
              << " removed=" << report.removed_options.size() << "\n";
    std::cout << "entities:";
    for (const auto& row : report.entities) {
        std::cout << " " << row.entity_id << "(binding=" << (row.binding_lens ? *row.binding_lens : "-")
                  << (row.floored ? ",floored" : "") << ")";
    }
    std::cout << "\n\n";

    if (g_failures.empty()) {
        std::cout << "FAILURES: none\n";
        std::cout << "OK\n";
    } else {
        std::cout << "FAILURES:";
        for (const auto& f : g_failures) std::cout << " [" << f << "]";
        std::cout << "\nFAILED\n";
    }
    return g_failures.empty() ? 0 : 1;
}

int main(int argc, char** argv) {
    const std::string which = (argc > 1) ? argv[1] : "v08";
    if (which == "v08") return run_harness_v08();
    if (which == "v07") return run_harness_v07();
    if (which == "v06") return run_harness_v06();
    if (which == "v091") return run_harness_v091();
    if (which == "v011") return run_harness_v011();
    if (which == "dump") return dump_reference();
    std::cout << "unknown harness \"" << which
              << "\": expected v011, v091, v08 (default), v07, v06 or dump\n";
    return 2;
}
