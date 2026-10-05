"""Conformance harness of the Python port, DOF-SPEC **v0.8** (§4.5, T1).

This is the release's own harness. `smoke_test_v07.py` is left untouched: it is
the v0.7 evidence, and it still reproduces the v0.7 ruler — which is itself one of
this release's checks (a moved digest is an error to be fixed, not a new version).

Sections, in order: the ruler did not move; the two entities `D3` exists for; the
candidate vector is computed; the decision that changed; the mirror; the third
key; bounds and the baseline.
"""
import sys

import fixture_v07 as F
import options_v07 as O
from orchestrator import DOFOrchestrator

# The v0.7 fingerprints of the *released* fixture, from §10. T1 must not move them:
# the candidate vector and the key order are not measurement inputs.
V07_RULER_DIGEST = "5126fd99641ffdc9c338d3d288fcf3cb6dcf093ca0a423f1cd265b3fcae4152a"
V07_OBSERVATION_DIGEST = "f3891c6ab622325fd6668893dd9f7450d39aa2d0a7ad2849634a4f59219f6a1c"
V07_INDEX = -35.314438370902

FAILURES = []
CHECKS = [0]


def check(name, condition, detail=""):
    CHECKS[0] += 1
    print(("  OK   " if condition else "  FAIL ") + name + ("  " + detail if detail else ""))
    if not condition:
        FAILURES.append(name)


def close_enough(a, b, tol=1e-9):
    return abs(a - b) <= tol


def run_t1():
    """A fresh orchestrator per run: a cycle must not inherit a previous state."""
    orch = DOFOrchestrator(context_switch_cost=0.05)
    state = orch.mapper.poll_environment(F.t1_scene())
    return orch.core, state, orch.mapper.last_observation, orch.mapper.last_declaration


# --- 1. the ruler did not move (§10, R6) --------------------------------------
orch = DOFOrchestrator(context_switch_cost=0.05)
base_state = orch.mapper.poll_environment(F.scene())
base_ctx = orch.mapper.last_observation
base_decl = orch.mapper.last_declaration

print("=== 1. §10/R6: the released fixture is unchanged by T1 ===")
check("the ruler digest is the v0.7 one, byte for byte",
      base_decl.digest() == V07_RULER_DIGEST, base_decl.digest()[:16])
check("the observation digest is the v0.7 one, byte for byte",
      base_ctx.observation_digest == V07_OBSERVATION_DIGEST,
      (base_ctx.observation_digest or "")[:16])
base_index = orch.core.calculate_system_dof(base_state, None, base_ctx)
check("the index of the released fixture is unchanged",
      close_enough(base_index, V07_INDEX, 1e-6), f"{base_index:.6f}")
# The fingerprints of the released fixture, in full: a run that stopped comparing
# must not be able to pass unnoticed, and a reader of the log must be able to see
# the values the run asserted against.
print()
print(f"RULER  digest={base_decl.digest()}")
print(f"OBSERVATION digest={base_ctx.observation_digest}")
print(f"INDEX  {base_index:.6f}")
print()

# --- the run variant ----------------------------------------------------------
core, state, ctx, decl = run_t1()

print("=== 2. §4.5: the two entities D3 exists for ===")
check("trainee sits above the minimum DoF of calc(S), so it is not critical",
      state.entities["trainee"].current_dof > 0.0,
      f"trainee={state.entities['trainee'].current_dof:.6f} "
      f"min={min(state.entities[e].current_dof for e in core.calc_members(state, ctx)):.6f}")
check("trainee can act itself: V > 0", ctx.v_before("trainee") > 0,
      f"V={ctx.v_before('trainee')}")
check("and its own act does not lift it — its path is somebody else's act",
      ctx.verdict("trainee") == "reachable", ctx.verdict("trainee"))
check("critical(S) is the known zeros of calc, not the dependents",
      core.critical_members(state, ctx) == {"revivable", "unobserved"},
      str(sorted(core.critical_members(state, ctx))))

print("=== 3. §4.5: the vector is computed, not declared ===")
v_comp = core.candidate_vector(state, O.t1_compensate(), ctx)
check("t1_compensate: no destruction — nothing is driven to a known zero",
      v_comp["d1"] == 0, str(v_comp))
check("t1_compensate: it cuts the patient's only path", v_comp["d2"] == 1, str(v_comp))
check("t1_compensate: and that path is the critical node's", v_comp["d3"] == 1, str(v_comp))
check("t1_compensate: NetDelta > 0 — every pre-v0.8 gate passes it",
      v_comp["net_delta"] > 0.0, f"{v_comp['net_delta']:+.6f}")
check("t1_compensate: the collapse charges are empty, as §4.2 computes them",
      core.collapse_charges(state, O.t1_compensate(), ctx) == [])
check("t1_compensate: the lost path is auditable, with the witness it lost",
      core.lost_paths(state, O.t1_compensate(), ctx)
      == [{"entity_id": "revivable", "verdict_before": "reachable",
           "verdict_after": "proven_unreachable", "critical": True,
           "witness_lost": ["act_medkit"]}],
      str(core.lost_paths(state, O.t1_compensate(), ctx)))

print("=== 4. §4.5: the decision that changed ===")
selected, _vectors = core.select_candidate(state, [O.t1_compensate()], ctx)
check("under T1 the option loses to staying put: the system stays", selected is None)
check("the report names the key that barred it", core.barring_key(v_comp) == "d2")
check("staying put is a candidate with the zero vector",
      # `v0.11` (§10(E), §4.10.1) makes admissibility one predicate with **two
      # families of conditions**, so the candidate vector gains `viable` and
      # `resources_ok`. The baseline satisfies every one of them vacuously — it
      # is not an action — so it remains the zero vector.
      core.baseline_vector() == {"viable": True, "resources_ok": True,
                                 "d1": 0, "d2": 0, "d3": 0, "net_delta": 0.0,
                                 "reversible": True, "option_id": None})
refusal = core.report(state, [O.t1_compensate()], None, "FAST_PASS",
                      declaration=decl, ctx=ctx)
check("no candidate beat inaction, and the report says so", refusal.no_candidate_better)
check("the refusal lists the candidates and the key that barred each",
      [(r["option_id"], r["barring_key"]) for r in refusal.options] == [("t1_compensate", "d2")])

print("=== 5. §4.5: the mirror — the same gain, a path that is not the price ===")
v_mirror = core.candidate_vector(state, O.t1_mirror(), ctx)
check("t1_mirror: nothing destroyed, nothing lost",
      (v_mirror["d1"], v_mirror["d2"], v_mirror["d3"]) == (0, 0, 0), str(v_mirror))
check("t1_mirror: a real gain over staying put",
      v_mirror["net_delta"] > 0.0, f"{v_mirror['net_delta']:+.6f}")
check("robot keeps eight of its nine vectors: the closure was a price, not a loss",
      ctx.v_after_closure("robot", O.t1_mirror().closed) == 8
      and ctx.v_before("robot") == 9)
check("the mirror is selected", core.evaluate_and_select(
    state, [O.t1_compensate(), O.t1_mirror()], ctx) is not None)
check("and it is the mirror that is selected, not the compensation",
      core.evaluate_and_select(state, [O.t1_compensate(), O.t1_mirror()], ctx).option_id
      == "t1_mirror")

print("=== 6. §4.5: a path cut is a bar, and the third key cannot separate ===")
v_help = core.candidate_vector(state, O.t1_help(), ctx)
v_rival = core.candidate_vector(state, O.t1_rival(), ctx)
check("t1_help: it cuts a path without destroying anything",
      # `v0.11` §4.9: the condition is `DoF(X | h) + Δ_P(X) > 0`, and the
      # **trivial** path is a finite simple path — so cutting a path does not
      # make a *live* entity unreachable, and `D2` no longer fires for it
      # (§10(J), fixtures `ar` and `ad`). `D2` keeps its teeth exactly where it
      # matters: an option that drives the entity to a **known zero** and closes
      # its recovery — which is what `t1_rival` does, and the assertion below
      # still holds for it.
      (v_help["d1"], v_help["d2"], v_help["d3"]) == (0, 0, 0), str(v_help))
check("t1_help: and no path is reported as lost",
      [row["entity_id"] for row in core.lost_paths(state, O.t1_help(), ctx)] == [])
check("closing the mentor's act costs it a vector and destroys nothing",
      core.collapse_charges(state, O.t1_help(), ctx) == []
      and ctx.v_after_closure("mentor", O.t1_help().closed) == 1)
check("t1_rival: the same D1 and D2, and the path is the critical node's",
      (v_rival["d1"], v_rival["d2"], v_rival["d3"]) == (0, 1, 1), str(v_rival))
check("staying put wins at the second key against both: a cut path is a bar",
      core.evaluate_and_select(state, [O.t1_help(), O.t1_rival()], ctx) is None)
check("so the third key cannot separate two candidates — D3 ≤ D2, and key 2 "
      "leaves only candidates with D2 = 0 (§4.5, finding of this release)",
      all(int(v["d3"]) <= int(v["d2"]) for v in (v_comp, v_mirror, v_help, v_rival)))

print("=== 7. §4.5: bounds, the baseline and the retired gate ===")
members = core.calc_members(state, ctx)
check("an empty candidate set selects nothing",
      core.evaluate_and_select(state, [], ctx) is None)
check("D1, D2 and D3 are bounded by calc(S)",
      all(int(v[k]) <= len(members) for v in (v_comp, v_mirror, v_help, v_rival)
          for k in ("d1", "d2", "d3")), f"|calc|={len(members)}")
chosen = core.evaluate_and_select(state, [O.t1_compensate(), O.t1_mirror()], ctx)
rep = core.report(state, [O.t1_compensate(), O.t1_mirror()], chosen, "FAST_PASS",
                  declaration=decl, ctx=ctx)
row = next(r for r in rep.options if r["option_id"] == "t1_compensate")
check("the report carries the vector and the barring key per option",
      row["candidate_vector"]["d2"] == 1 and row["barring_key"] == "d2")
check("the report carries the baseline",
      rep.baseline["d1"] == 0 and rep.baseline["net_delta"] == 0.0
      and rep.baseline["reversible"] is True)
check("a selected option is not reported as a refusal", rep.no_candidate_better is False)
check("the structural decision is no longer a removal: no collapse gate anywhere",
      all(entry.get("gate") != "collapse" for entry in rep.removed_options),
      str(rep.removed_options))
check("a charged candidate is no longer deleted from the set: it is evaluated and reported",
      core.apply_structural_gate(state, [O.opt_win(), O.opt_collapse()], ctx)[1]
      == [{"option_id": "opt_collapse", "gate": "collapse"}],
      "the retired v0.7 rule is still callable by the historical harness")
check("and on the live path nobody leaves the candidate set",
      len(core.select_candidate(state, [O.opt_win(), O.opt_collapse()], ctx)[1]) == 2)

print()
print(f"checks: {CHECKS[0]}, failures: {len(FAILURES)}")
if FAILURES:
    print("FAILURES: " + str(FAILURES))
    sys.exit(1)
print("OK")
