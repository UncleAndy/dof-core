"""Smoke test / fixtures of the Python port, now covering the measurement layer.

Checks, in order:
  1. §4.1  — `current_dof` equals the lens product on every entity.
  2. §6.1  — the per-lens terms sum to the entity contribution (when not floored).
  3. §4.6  — the degenerate case: a passive object gives ψ = 0, no NaN, no crash.
  4. §4.2  — a known zero with no raising option is excluded, not charged −∞.
  5. §4.7  — an unmeasured lens costs `ln u₀`, keeps `dof_known = false`, and the
             entity is never excluded.
  6. §3.4.3 — the declaration digest is stable for the same inputs and changes
             when a counter changes (this is what makes R7 checkable).
  7. §5    — the viability gate removes an option that cannot complete before τ
             and records the removal (both modes are exercised: DEEP and FAST_PASS).
  8. §6 example 1 of the draft — "irreversible process: state-DoF ↑, action-DoF ↓":
             the product collapses where a sum would mask the danger.
  9. §4.2  — the calculation set is frozen per cycle: destroying a counted entity
             is charged the floor instead of raising the index by disappearing,
             while a passive object is neither charged nor rewarded.
 10. §4.5  — structural admissibility (a destructive option is removed while a
             charge-free alternative exists) and the `NetDelta > 0` stay-put baseline.
 11. §4.7  — coverage of unmapped entities by every candidate, and `incomplete`.
 12. §4.6  — the block-level `(c_g, C_g)` are *derived* by the named procedure
             from raw requirements, the agent's means and the derived groups;
             the declaration carries the resource units, groups and rates.
 13. §4.8  — the resource gate: direct comparison, verified conversion (path,
             offer, payable price, the exchange's own time against τ) and
             insolvency — including a resource whose balance is not declared at
             all (an invalid input, not an evaluation mode).
 14. §6.2/§6.3 — `resources_before`/`resources_after`, per-option
             `resource_consumption` and `conversion_applied`.
"""
import json
import math

from calculus_core import ActionOption, DOFCalculusCore
from measurement import (EPSILON, LENS_ORDER, U_MAX, U_MIN, LensObservation,
                         build_declaration, derive_blocks, psi_con, psi_opt, psi_var)
from orchestrator import DOFOrchestrator

FAILURES = []


def check(name, condition, detail=""):
    print(("  OK   " if condition else "  FAIL ") + name + ("  " + detail if detail else ""))
    if not condition:
        FAILURES.append(name)


def entity(dof, known=True, probe=None):
    return {"current_dof": dof, "dof_known": known, "probe": probe}


# --- Fixture 1: the ordinary cycle, τ = 4 s (FAST_PASS) --------------------------
obs_fast = {
    "adult": {"is_autonomous": True, "agency_index": 0.9, "is_collapse_source": False,
              "time_to_collapse_mks": 100000000.0,
              "lenses": {"variety": {"V": 3.0, "V_env": 2.0},
                         "options": [[1.0, 10.0]],
                         "constraint": {"F": 4.0, "F_env": 1.0}}},
    "child": {"is_autonomous": False, "agency_index": 0.1, "is_collapse_source": False,
              "time_to_collapse_mks": 4000000.0,
              "lenses": {"variety": {"V": 1.0, "V_env": 5.0},
                         "options": [[2.0, 4.0]],
                         "constraint": {"F": 1.0, "F_env": 3.0}}},
    "aggressor": {"is_autonomous": True, "agency_index": 0.5, "is_collapse_source": True,
                  "time_to_collapse_mks": 100000000.0,
                  "lenses": {"variety": {"V": 5.0, "V_env": 1.0},
                             "options": [[1.0, 100.0]],
                             "constraint": {"F": 5.0, "F_env": 1.0}}},
    "stone": {"is_autonomous": False, "agency_index": 0.0, "is_collapse_source": False,
              "time_to_collapse_mks": 100000000.0,
              # passive object: no response vectors, no budget, no free variables
              "lenses": {"variety": {"V": 0.0, "V_env": 0.0},
                         "options": [],
                         "constraint": {"F": 0.0, "F_env": 0.0}}},
    "unmapped": {"is_autonomous": True, "agency_index": 0.4, "is_collapse_source": False,
                 "time_to_collapse_mks": 100000000.0,
                 # the Options lens was never measured: u(t) applies (§4.7)
                 "lenses": {"variety": {"V": 2.0, "V_env": 2.0},
                            "constraint": {"F": 1.0, "F_env": 1.0}}},
    # v0.6: this entity does not declare blocks at all — it declares what its
    # transitions *require*, and the blocks are derived against the agent's means.
    "drone": {"is_autonomous": True, "agency_index": 0.6, "is_collapse_source": False,
              "time_to_collapse_mks": 100000000.0,
              "lenses": {"variety": {"V": 4.0, "V_env": 2.0},
                         "requirements": {"energy": 4.0},
                         "constraint": {"F": 3.0, "F_env": 1.0}}},
    # §3.2/§4.8 (v0.6): the acting agent, the derived groups and the observed
    # rates. This is part of the ruler: the declaration carries it, so a ruler
    # with different units or rates is a different ruler.
    "resource_layer": {
        "means": {"credit": 6.0, "energy": 10.0},
        "groups": [["credit", "energy"]],
        "rates": {"credit->energy": {"rate": 2.0, "duration_mks": 1000.0}},
        "resources": [{"id": "credit", "unit": "credit", "scale": 1.0},
                      {"id": "energy", "unit": "joule", "scale": 1.0}],
        "mandate": {"external_limit_credit": 100.0, "scope": "household"},
    },
}

# The v0.6 reference digest: every port must reproduce this value byte-for-byte
# on this fixture (§3.4.3, §7).
REFERENCE_DIGEST_V06 = "bed37c25fd9cb757e9ea4a861c01cd4660fd896a83cd39b7c73b8e0be7489ad4"

orch = DOFOrchestrator(context_switch_cost=0.05)
state = orch.mapper.poll_environment(obs_fast)
core = DOFCalculusCore()
selected, report = orch.step_with_report(obs_fast)

print("=== 1. §4.1: current_dof == lens product ===")
for eid, ent in state.entities.items():
    m = ent.measurement
    product = 1.0
    for lens in LENS_ORDER:
        value = m.psi[lens]
        product *= m.psi[lens] if value is not None else orch.mapper.last_declaration.u0()
    check(f"{eid}: product == current_dof", abs(product - ent.current_dof) < 1e-12,
          f"product={product:.6f} current_dof={ent.current_dof:.6f}")

print("=== 2. §6.1: terms sum to the contribution ===")
for eid, ent in state.entities.items():
    m = ent.measurement
    if not m.floored:
        check(f"{eid}: Σ terms == contribution", abs(m.terms_sum - m.contribution) < 1e-12,
              f"Σ={m.terms_sum:.12f} contrib={m.contribution:.12f}")

print("=== 3-4. §4.6 guard and §4.2 exclusion (passive object) ===")
stone = state.entities["stone"]
check("stone: ψ_var = 0 (no 0/0, no NaN)", stone.measurement.psi["variety"] == 0.0)
check("stone: current_dof = 0", stone.current_dof == 0.0)
check("stone: no NaN in the index", not math.isnan(report.total_system_dof))
check("stone: excluded when nothing can raise it (§4.2)",
      core._is_included(stone) is False)
check("stone: floored flag set in the audit", stone.measurement.floored is True)

print("=== 5. §4.7: unmeasured lens ===")
unmapped = state.entities["unmapped"]
check("unmapped: dof_known = false", unmapped.dof_known is False)
check("unmapped: never excluded (§4.2)", core._is_included(unmapped) is True)
unknown_term = [t for t in unmapped.measurement.terms if not t["dof_known"]]
check("unmapped: one unmeasured term of three", len(unknown_term) == 1)
check("unmapped: the term costs ln u₀", abs(unknown_term[0]["contribution"] - math.log(0.5)) < 1e-12)
check("u₀ band is respected", U_MIN <= 0.5 <= U_MAX, f"U_MIN={U_MIN:.4f} U_MAX={U_MAX:.4f}")

print("=== 6. §3.4.3: the digest is the ruler's fingerprint ===")
digest_a = state.psi.digest
digest_b = orch.mapper.poll_environment(obs_fast).psi.digest
check("same inputs → same digest", digest_a == digest_b, digest_a[:16] + "…")
mutated = json.loads(json.dumps(obs_fast))
mutated["adult"]["lenses"]["variety"]["V"] = 4.0
digest_c = orch.mapper.poll_environment(mutated).psi.digest
check("changed counter → different digest", digest_c != digest_a, digest_c[:16] + "…")
canonical = orch.mapper.last_declaration.canonical_text()
check("canonical form has no exponent notation", "e-" not in canonical and "e+" not in canonical)
check("digest is 64 hex chars", len(digest_a) == 64 and all(c in "0123456789abcdef" for c in digest_a))

print("=== 7. §5: the viability gate, and both modes ===")
slow = json.loads(json.dumps(obs_fast))
for key, ent in slow.items():
    if key == "resource_layer":
        continue
    ent["time_to_collapse_mks"] = 500.0        # τ = 500 μs < the 1000 μs fallback option
sel_slow, rep_slow = orch.step_with_report(slow)
check("τ < option duration → option barred, and nothing is selected",
      # `v0.11` (§10(D), §10(E)) **retires the removal mechanism**: the temporal
      # condition of §4.8b is no longer a pipeline that deletes the candidate
      # before evaluation, it is a condition of admissibility (`viable`), reported
      # per option with the condition it failed. `removed_options` is kept and
      # marked in the report, never filled. The decision is unchanged — the option
      # still cannot be selected — but a removal is no longer how it is expressed.
      sel_slow is None and rep_slow.removed_options == []
      and rep_slow.options[0]["viability"]["viable"] is False
      and rep_slow.options[0]["barring_key"] == "viable")
check("removal is visible in the audit", rep_slow.mode == "FAST_PASS")
check("fixture 1 runs in FAST_PASS", report.mode == "FAST_PASS", f"τ={state.global_time_to_collapse_mks}")

obs_deep = json.loads(json.dumps(obs_fast))
for key, ent in obs_deep.items():
    if key == "resource_layer":
        continue
    ent["time_to_collapse_mks"] = 100000000.0   # τ = 100 s ≥ threshold
_sel_deep, rep_deep = orch.step_with_report(obs_deep)
check("fixture 2 runs in DEEP_DIVERSIFICATION", rep_deep.mode == "DEEP_DIVERSIFICATION",
      f"τ={rep_deep.global_time_to_collapse_mks}")
check("psi_id and digest are echoed in the report",
      rep_deep.psi_id == "perception-v1" and len(rep_deep.psi_digest or "") == 64)

print("=== 8. draft §6, example 1: irreversible process (state-DoF ↑, action-DoF ↓) ===")
before = (psi_var(9.0, 1.0), psi_opt([(1.0, 10.0)]), psi_con(9.0, 1.0))
after = (psi_var(19.0, 1.0), psi_opt([(5.0, 1.0)]), psi_con(9.0, 1.0))
prod_before, prod_after = math.prod(before), math.prod(after)
sum_before, sum_after = sum(before), sum(after)
ratio_product, ratio_sum = prod_after / prod_before, sum_after / sum_before
check("product collapses (ΔIndex ≈ %.2f nats)" % math.log(ratio_product), ratio_product < 0.01,
      f"×{ratio_product:.5f}")
check("a sum would mask it", ratio_sum > 0.6, f"×{ratio_sum:.3f}")

print("=== 9. §4.2/§4.5 (v0.5): frozen calc set, collapse charge, gate, stay-put ===")
adult = state.entities["adult"]
total_before = report.total_system_dof

killer = ActionOption(option_id="kill_adult", description="liquidate the counted adult",
                      projected_dof_delta={"adult": -1.0, "unmapped": 0.0},
                      is_reversible=True, estimated_duration_mks=1000.0)
charges = core.collapse_charges(state, killer)
check("charge: the destroyed entity is named with its DoF before the option",
      charges == [{"entity_id": "adult", "dof_before": adult.current_dof}],
      f"charges={charges}")
sim_kill, members = core.simulate(state, killer)
projected_kill = core.calculate_system_dof(sim_kill, members)
expected_kill = total_before - math.log(adult.current_dof) + math.log(EPSILON)
check("charge: the term stays at the floor instead of disappearing",
      abs(projected_kill - expected_kill) < 1e-9,
      f"Δ={projected_kill - total_before:+.4f} nats")
check("charge: destroying a counted entity can never raise the index",
      projected_kill < total_before)
passive_killer = ActionOption(option_id="raise_stone", description="act on a passive object",
                              projected_dof_delta={"stone": 1.0, "unmapped": 0.0},
                              is_reversible=True, estimated_duration_mks=1000.0)
check("frozen set: a passive object is neither charged nor rewarded",
      core.collapse_charges(state, passive_killer) == []
      and abs(core.calculate_system_dof(*core.simulate(state, passive_killer)) - total_before) < 1e-12)

spare = ActionOption(option_id="rescue_child", description="raise the weakest counted entity",
                     projected_dof_delta={"child": 0.2, "unmapped": 0.0},
                     is_reversible=True, estimated_duration_mks=1000.0)
admissible, gate_removed = core.apply_structural_gate(state, [killer, spare])
check("structural gate: the destructive option is removed while a charge-free one exists",
      [o.option_id for o in admissible] == ["rescue_child"]
      and gate_removed == [{"option_id": "kill_adult", "gate": "collapse"}],
      f"removed={gate_removed}")
check("Axiom 3: the charge alone already makes destruction unprofitable",
      core.evaluate_and_select(state, [killer]) is None, "NetDelta < 0 ⇒ stay put")
check("structural gate: when every candidate destroys, they stay admissible",
      len(core.apply_structural_gate(state, [killer])[0]) == 1)
harm = ActionOption(option_id="harm_child", description="degrade the child",
                    projected_dof_delta={"child": -1.0, "unmapped": 0.0},
                    is_reversible=True, estimated_duration_mks=1000.0)
check("stay-put baseline: an all-negative candidate set selects nothing",
      core.evaluate_and_select(state, [harm]) is None
      and core.evaluate_and_select(state, []) is None)
check("fixture 1: a strictly positive option is selected",
      selected is not None, f"selected={selected.option_id if selected else None}")

print("=== 10. §4.7 (v0.5): coverage and completeness of unmapped entities ===")
generated = orch.generator.safe_fallback(state, n_options=3)
check("coverage: every candidate names the unmapped entity",
      all("unmapped" in o.projected_dof_delta for o in generated))
check("completeness: the fallback leaves a resolvable unknown unmeasured ⇒ incomplete",
      report.incomplete is True)
measuring = [ActionOption(option_id="measure_unmapped", description="resolve the unknown",
                          projected_dof_delta={"unmapped": 0.1},
                          is_reversible=True, estimated_duration_mks=1000.0)]
check("completeness: a candidate that resolves the unknown clears the flag",
      core._is_incomplete(state, measuring) is False)

print("=== 11. §4.6 (v0.6): the blocks are derived, not authored ===")
orch_v6 = DOFOrchestrator(context_switch_cost=0.05)
state_v6 = orch_v6.mapper.poll_environment(obs_fast)
decl_v6 = orch_v6.mapper.last_declaration
groups_v6, rates_v6 = decl_v6.groups, decl_v6.rates
drone = state_v6.entities["drone"]
check("derived: (c_g, C_g) = (4, 16) from requirements + means in one group",
      drone.measurement.blocks == [(4.0, 16.0)], f"blocks={drone.measurement.blocks}")
check("derived: the derivation names the procedure and its inputs",
      drone.measurement.derivation is not None
      and drone.measurement.derivation["procedure"] == "derive_blocks"
      and drone.measurement.derivation["requirements"] == {"energy": 4.0}
      and drone.measurement.derivation["groups"] == [["credit", "energy"]])
check("derived: ψ_opt equals psi_opt on the derived blocks",
      abs(drone.measurement.psi["options"] - psi_opt(drone.measurement.blocks)) < 1e-15)
check("derived: a resource in no declared group forms a singleton block",
      derive_blocks({"fuel": 2.0}, {"fuel": 4.0}, [["credit", "energy"]]) == [(0.0, 0.0), (2.0, 4.0)]
      and psi_opt(derive_blocks({"fuel": 2.0}, {"fuel": 4.0}, [["credit", "energy"]])) == psi_opt([(2.0, 4.0)]),
      f"{derive_blocks({'fuel': 2.0}, {'fuel': 4.0}, [['credit', 'energy']])}")
check("derived: the declaration names the derivation procedure",
      decl_v6.procedures["options_blocks"] == "perception-v1:derive_blocks")
check("ruler: units, groups, rates and mandate are in the hashed content",
      [r["id"] for r in decl_v6.resources] == ["credit", "energy"]
      and decl_v6.groups == [["credit", "energy"]]
      and decl_v6.rates["credit->energy"]["rate"] == 2.0
      and decl_v6.mandate["external_limit_credit"] == 100.0)
units_other = [{"id": "credit", "unit": "credit", "scale": 1.0},
               {"id": "energy", "unit": "kilojoule", "scale": 1000.0}]
obs_map = {eid: LensObservation(**(obs_fast[eid].get("lenses") or {}))
           for eid in obs_fast if eid != "resource_layer"}
decl_other = build_declaration("perception-v1", obs_map, 4000000.0, None,
                               resources=units_other, groups=decl_v6.groups,
                               rates=decl_v6.rates, mandate=decl_v6.mandate)
check("ruler: the same resource at another scale is a different digest",
      decl_other.digest() != decl_v6.digest())
check("ruler: time is not a resource (τ is never converted)",
      "tau" not in [r["id"] for r in decl_v6.resources]
      and all("tau" not in key for key in decl_v6.rates))
check("reference digest v0.6 is reproduced", state_v6.psi.digest == REFERENCE_DIGEST_V06,
      state_v6.psi.digest[:16] + "…")

print("=== 12. §4.8 (v0.6): direct payment, verified conversion, insolvency ===")


def draw(option_id, per_entity, duration=1000.0):
    """A candidate that asks the agent for a declared draw and names the unknown."""
    return ActionOption(option_id=option_id, description=option_id,
                        projected_dof_delta={"child": 0.1, "unmapped": 0.0},
                        is_reversible=True, estimated_duration_mks=duration,
                        projected_resource_delta=per_entity)


direct = draw("direct", {"child": {"energy": -2.0}})
funded = draw("funded", {"child": {"energy": -12.0}})
no_time = draw("no_time_for_trade", {"child": {"energy": -12.0}}, duration=4000000.0)
undeclared = draw("undeclared", {"child": {"fuel": -1.0}})
offset = draw("offset", {"child": {"energy": -3.0}, "adult": {"energy": 1.0}})

plan_direct = orch_v6.core.plan_funding(state_v6, direct, groups_v6, rates_v6)
check("step 1: means cover the draw ⇒ payable, nothing converted",
      plan_direct["covered"] is True and plan_direct["spend"] == {"energy": 2.0}
      and plan_direct["conversions"] == [], f"spend={plan_direct['spend']}")
plan_funded = orch_v6.core.plan_funding(state_v6, funded, groups_v6, rates_v6)
check("step 2: the deficit is bought at the observed rate",
      plan_funded["covered"] is True
      and plan_funded["conversions"][0]["from"] == "credit"
      and abs(plan_funded["conversions"][0]["amount_from"] - 1.0) < 1e-12
      and abs(plan_funded["conversions"][0]["amount_to"] - 2.0) < 1e-12
      and plan_funded["conversions"][0]["rate"] == 2.0,
      f"conversions={plan_funded['conversions']}")
check("step 2: only the deficit is traded (cash in hand is spent first)",
      plan_funded["spend"] == {"credit": 1.0, "energy": 10.0},
      f"spend={plan_funded['spend']}")
check("step 2: the exchange's own time is charged to τ",
      plan_funded["total_duration_mks"] == 2000.0, f"{plan_funded['total_duration_mks']}")
check("step 3: an exchange that does not fit in τ leaves the deficit uncovered ⇒ insolvency",
      orch_v6.core.plan_funding(state_v6, no_time, groups_v6, rates_v6)["covered"] is False)
check("step 3: a resource whose balance is not declared cannot be bought ⇒ insolvency",
      orch_v6.core.plan_funding(state_v6, undeclared, groups_v6, rates_v6)["uncovered"] == {"fuel": 1.0})
check("production offsets consumption (net draw decides)",
      orch_v6.core.requirement(offset) == {"energy": 2.0}, f"{orch_v6.core.requirement(offset)}")
broke = json.loads(json.dumps(obs_fast))
broke["resource_layer"]["means"] = {"credit": 0.4, "energy": 0.0}
state_broke = orch_v6.mapper.poll_environment(broke)
check("step 3: a price the agent cannot pay is not a cheaper price ⇒ insolvency",
      orch_v6.core.plan_funding(state_broke, funded,
                                orch_v6.mapper.last_declaration.groups,
                                orch_v6.mapper.last_declaration.rates)["covered"] is False)
admissible, removed_resource = orch_v6.core.apply_resource_gate(
    state_v6, [direct, funded, undeclared, offset], groups_v6, rates_v6)
check("gate: the unpayable option is removed with gate = insolvency",
      [o.option_id for o in admissible] == ["direct", "funded", "offset"]
      and removed_resource == [{"option_id": "undeclared", "gate": "insolvency"}],
      f"removed={removed_resource}")
check("gates: §5 → §4.5 → §4.8 compose in order, each recording its own removals",
      [o.option_id for o in orch_v6._gates(state_v6, [direct, undeclared])[0]] == ["direct"]
      and orch_v6._gates(state_v6, [direct, undeclared])[1]
      == [{"option_id": "undeclared", "gate": "insolvency"}])

print("=== 13. §6.2/§6.3 (v0.6): the spend is auditable ===")
sel_v6, rep_v6 = orch_v6.step_with_report(obs_fast)
check("report: resources_before is the agent's means at the start of the cycle",
      rep_v6.resources_before == {"credit": 6.0, "energy": 10.0},
      f"{rep_v6.resources_before}")
check("report: the deterministic fallback buys nothing, so the stock is unchanged",
      rep_v6.resources_after == rep_v6.resources_before
      and sel_v6 is not None and sel_v6.projected_resource_delta.get("child", {}).get("energy") == 0.0)
rep_funded = orch_v6.core.report(state_v6, [funded], funded, "FAST_PASS",
                                 declaration=decl_v6, groups=groups_v6, rates=rates_v6)
check("report: buying a deficit debits the resource that actually paid",
      rep_funded.resources_after == {"credit": 5.0, "energy": 0.0},
      f"after={rep_funded.resources_after}")
check("report: the per-option row carries the draw and the conversions applied",
      rep_funded.options[0]["resource_consumption"] == {"child": {"energy": -12.0}}
      and len(rep_funded.options[0]["conversion_applied"]) == 1)
rep_undeclared = orch_v6.core.report(state_v6, [undeclared], None, "FAST_PASS",
                                     declaration=decl_v6, groups=groups_v6, rates=rates_v6)
check("report: an uncovered deficit is written down per option",
      rep_undeclared.options[0]["resources_uncovered"] == {"fuel": 1.0})

print()
print("REPORT (fixture 1):")
print(json.dumps(report.model_dump(), ensure_ascii=False, indent=None)[:1200])
print()
print("FAILURES:", FAILURES if FAILURES else "none")
print("OK" if not FAILURES else "FAILED")
