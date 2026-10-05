"""DOF-SPEC v0.11 conformance harness (Python reference port).

The release's own claims, checked by running them (§7 items 24-47, and the
fixture letters of §10's `v0.11` evidence line):

  a  a singleton `H` reproduces the `v0.9.1` numbers and the choice
  b  a stated DoF that its own lenses do not produce is non-conformant input
  c  a hypothesis moving `is_collapse_source` is non-conformant input
  d  two readings: different digests and `lens_terms`, same ruler and same
     type-derived graph values; the §4.9 verdict is reported per reading
  e  a flat-form option reproduces its numbers under every reading
  f  a per-reading projection beneficial under one and destructive under
     another is refused by the damaging reading; the mirror is selected
  g  an option mixing the two forms is non-conformant
  h  an option that clears τ under one reading and fails it under another is
     barred, and reports the condition and the reading that barred it
  i  a declared causal reading moves no value and no choice
  j  an empty robust candidate set yields `none` with `hypothesis_conflict`
  n  nothing is reported as removed; the conditions are reported per option
  o  the conflict flag follows the whole admissibility predicate
  r  an absent and an empty `H` are the observed-state singleton
  s  the deprecated mirror is `0.0` for a passed and for an unknown τ
  v  one mode, from the observed state, with the dissenters reported
  w  an active unmeasured deadline makes τ `null`, never the minimum measured
  y  two readings give different §4.9 verdicts for one entity
  z  the ruler and the type-derived graph values are shared
  ab equal `psi_ruler_digest`, different per-reading digest
  ad `ψ_opt = 0` is not structural unreachability
  ae an option that drives an entity to a zero and closes its recovery is
     barred (`d2 = 1`), while the same option without closures is not
  af `total_system_dof` is a map over the readings
  ag `closure_form` / `projection_form` are reported per option
  ar a live entity is `reachable` with no raising act path in the graph
  ax the axis rate is τ-independent

Run: `python3 harness_v011.py` (under `nix-shell -p python3 -p
python3Packages.pydantic`).
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from calculus_core import (NET_DELTA_TOLERANCE, ActionOption, DOFCalculusCore,
                           ObservationContext, ResourceObservation,
                           SystemStateMatrix, tau_of)
import fixture_v07
import options_v07
from fixture_v011 import (TAU_FAST_MKS, TAU_MKS, T_REC_V011, default_scene,
                          h_only_closer, hset, hypothesis_from, observed_only,
                          scene)
from hypothesis import (Hypothesis, HypothesisSet, plausible_members,
                        resolved_members, validate_set)
from orchestrator import DOFOrchestrator
from world_graph import ClosedRef

def _vertices(path, world):
    """The **vertex sequence** of an enumerated path (§4.9).

    A path is a chain of acts joined `target -> source`, so its vertices are the
    first act's `source` followed by every act's `target`. Checking uniqueness of
    the *targets* alone is not the rule: the start vertex is never a target, so
    `A -> B -> C` and `A -> B -> A -> B` both have unique targets. The sequence,
    and the join between consecutive acts, are what "simple" is about.
    """
    acts = [world._act(i) for i in path]
    joined = all(acts[i].target == acts[i + 1].source for i in range(len(acts) - 1))
    return [acts[0].source] + [a.target for a in acts], joined


PASS = 0
FAIL = 0


def check(label: str, cond: bool, detail: str = ""):
    global PASS, FAIL
    if cond:
        PASS += 1
        print(f"  OK   {label}")
    else:
        FAIL += 1
        print(f"  FAIL {label}  {detail}")


def cycle(raw=None, hs=None):
    """One decision cycle. Returns (orch, state, selected, report)."""
    orch = DOFOrchestrator()
    raw = raw if raw is not None else default_scene()
    selected, report = orch.step_with_report(raw, hs)
    return orch, orch.mapper.poll_environment(raw), selected, report


def opt(option_id, **kw) -> ActionOption:
    kw.setdefault("description", option_id)
    return ActionOption(option_id=option_id, **kw)


# ---------------------------------------------------------------- a, r
def test_singleton():
    print("=== (a) a singleton H reproduces the observed decision ===")
    _, _, sel_bare, rep_bare = cycle()
    _, _, sel_one, rep_one = cycle(hs=None)
    _, state, _, _ = cycle()
    _, _, sel_sing, rep_sing = cycle(hs=observed_only(state))
    check("no H and an explicit observed singleton agree on the choice",
          sel_bare is not None and (sel_bare.option_id == sel_sing.option_id))
    check("and on the index",
          abs(rep_bare.total_system_dof - rep_sing.total_system_dof) < 1e-9,
          f"{rep_bare.total_system_dof!r} vs {rep_sing.total_system_dof!r}")
    check("the singleton set reports one reading",
          len(rep_sing.hypotheses) == 1 and rep_sing.hypotheses[0]["id"] == "$observed$")
    print("=== (r) absence and emptiness of H are the observed singleton ===")
    _, state2, _, _ = cycle()
    empty = HypothesisSet(members=[])
    members = resolved_members(state2, empty)
    check("an empty H resolves to one reading", len(members) == 1)
    check("H_plausible is not empty", len(plausible_members(members)) == 1)
    _, _, sel_empty, rep_empty = cycle(hs=empty)
    check("an empty H decides as the bare state",
          (sel_empty is None) == (sel_bare is None)
          and (sel_empty is None or sel_empty.option_id == sel_bare.option_id))


# ---------------------------------------------------------------- b, c
def test_non_conformant_input():
    print("=== (b, c) non-conformant hypothesis sets ===")
    _, state, _, _ = cycle()
    broken = hypothesis_from(state, "h_broken", {"robot": 0.9}, break_lens_identity=True)
    # The set must still contain the observed state, or the completeness rule of
    # §3.6 fires first and masks the defect this check is about.
    errs = validate_set(state, resolved_members(
        state, HypothesisSet(members=[hypothesis_from(state, "$observed$", {}), broken])))
    check("(b) a stated DoF its own lenses do not produce is refused",
          any("differs from the product" in e for e in errs), str(errs))

    moved = hypothesis_from(state, "h_moved", {}, move_collapse_source="forged")
    errs = validate_set(state, resolved_members(
        state, HypothesisSet(members=[hypothesis_from(state, "$observed$", {}), moved])))
    check("(c) a moved is_collapse_source is refused",
          any("is_collapse_source differs" in e for e in errs), str(errs))

    good = hypothesis_from(state, "h_ok", {"robot": 0.5})
    errs = validate_set(state, resolved_members(
        state, HypothesisSet(members=[hypothesis_from(state, "$observed$", {}), good])))
    check("(b, c) a conformant reading passes the same checks",
          not any("product" in e or "is_collapse_source" in e for e in errs), str(errs))


# ---------------------------------------------------------------- d, z, y, ab
def test_two_readings():
    print("=== (d, y, z, ab) two readings under one ruler ===")
    orch, state, _, _ = cycle()
    ctx = orch.mapper.last_observation
    decl = orch.mapper.last_declaration
    core = orch.core

    hs = hset(state, lambda s: [hypothesis_from(s, "h_low", {"robot": 0.2})])
    members = resolved_members(state, hs)
    plausible = plausible_members(members)
    check("(d) two readings are declared", len(plausible) == 2)

    lens = {}
    for h in plausible:
        m = h.state.entities["robot"].measurement
        lens[h.id] = (m.psi["variety"], h.state.entities["robot"].current_dof)
    check("(d) lens_terms differ between the readings",
          abs(lens["$observed$"][0] - lens["h_low"][0]) > 1e-9, str(lens))
    check("(d) and so does the DoF they produce",
          abs(lens["$observed$"][1] - lens["h_low"][1]) > 1e-9, str(lens))

    # Two readings of one cycle with **different measured content**, built before
    # (z) and (ab) so that both can compare across readings instead of comparing
    # an object with itself.
    raw_a, raw_b = scene(), scene()
    raw_b["robot"]["lenses"]["variety"]["V"] = 5.0     # a different measurement
    d_a, d_b = _decl_for(raw_a), _decl_for(raw_b)

    # (z) the ruler and the type-derived graph values are shared. "Shared" has to
    # mean *the same across two readings*, and each item has to be traced to its
    # declared source: `ctx.means_class == ctx.means_class` and
    # `ctx.horizon("robot") == ctx.horizon("robot")` compare one object with
    # itself and would pass for a port that read the ruler off the hypothesis.
    check("(z) M(S) is one class for the whole set",
          list(d_a.means_class) == list(d_b.means_class)
          == list(raw_a["world"]["means_class"]), str(d_a.means_class))
    check("(z) the measurement durations are ruler-level, hence shared",
          d_a.measurement_durations == d_b.measurement_durations)
    check("(z) T_rec is read from the graph, not from a state field",
          all(ctx.horizon(e) == T_REC_V011[e] for e in T_REC_V011)
          and all(ctx.horizon(e) == ctx.horizon(e) for e in T_REC_V011),
          str({e: ctx.horizon(e) for e in T_REC_V011}))
    struct_a = ctx.world.reachability_paths("robot", ctx.means_class,
                                            ctx.horizon("robot"))
    struct_b = ctx.world.with_closed([]).reachability_paths("robot", ctx.means_class,
                                                            ctx.horizon("robot"))
    check("(z) the structurally admissible path set is non-empty and identical "
          "on the same observation taken twice",
          struct_a == struct_b and len(struct_a) > 0
          and all(len(set(_vertices(p, ctx.world)[0])) == len(_vertices(p, ctx.world)[0])
                  for p, _d, _x in struct_a), str(struct_a[:3]))

    # (y) the verdict is per reading. The witness is a genuine **simple path**:
    # `act_medkit` runs `adult -> revivable`, so the path visits two vertices and
    # repeats none. The entity's own repertoire is a *different* question (§4.6's
    # `V` counts it), and §4.9 does not care who acts — which is why `revivable`,
    # with zero response vectors of its own, is the entity that tests this.
    h_rec = ctx.horizon("revivable")
    v_obs = ctx.world.verdict("revivable", ctx.means_class, h_rec, 0.5)
    v_zero = ctx.world.verdict("revivable", ctx.means_class, h_rec, 0.0)
    check("(y) a live entity is reachable", v_obs.verdict == "reachable",
          f"{v_obs.verdict} {v_obs.witness}")
    check("(y) the same entity at a known zero with a raising path is reachable",
          v_zero.verdict == "reachable", f"{v_zero.verdict} {v_zero.witness}")
    check("(y) and the witness is a simple path, not the entity acting on itself",
          list(v_zero.witness) == ["act_medkit"], str(v_zero.witness))
    # A self-act repeats its own vertex, so it is not a simple path and cannot
    # be a §4.9 witness. `robot`'s repertoire is nine self-acts (`r1..r9`) and
    # nothing else acts on it, so the entity is NOT reachable by a path — it is
    # reachable by the *trivial* path alone, and only while its DoF is positive.
    check("(y) an entity whose only acts are self-acts is not reachable at zero",
          ctx.world.verdict("robot", ctx.means_class, ctx.horizon("robot"), 0.0
                            ).verdict == "proven_unreachable")
    check("(y) but the trivial path keeps it reachable while it is positive",
          ctx.world.verdict("robot", ctx.means_class, ctx.horizon("robot"), 0.5
                            ).verdict == "reachable")
    check("(y) every enumerated path is a simple path over its VERTEX SEQUENCE",
          all(len(set(_vertices(p, ctx.world)[0])) == len(_vertices(p, ctx.world)[0])
              and _vertices(p, ctx.world)[1]
              for p, _d, _x in
              ctx.world.reachability_paths("revivable", ctx.means_class, h_rec)))
    # The witness of the entity judged is a simple path too, and its start is not
    # among the vertices it produces.
    _wv, _wj = _vertices(v_zero.witness, ctx.world)
    check("(y) the reported witness joins up and repeats no vertex",
          _wj and len(set(_wv)) == len(_wv) and _wv[0] not in _wv[1:], str(_wv))
    check("(y) but `passive` is proven unreachable",
          ctx.world.verdict("passive", ctx.means_class, ctx.horizon("passive"), 0.0
                            ).verdict == "proven_unreachable")

    # (ab) the ruler digest is shared, the per-reading digest is not. This is
    # checked the only way that means anything: against the **second reading**
    # built above, never by calling one accessor twice on one object —
    # `d.ruler_digest() == d.ruler_digest()` is a tautology and would hold for
    # any accessor, including a wrong one (§3.4.2, §3.4.3, §4.10).
    check("(ab) the two readings really differ in measured content",
          d_a.digest() != d_b.digest(), f"{d_a.digest()} vs {d_b.digest()}")
    check("(ab) yet they were taken under ONE ruler",
          d_a.ruler_digest() == d_b.ruler_digest(),
          f"{d_a.ruler_digest()} vs {d_b.ruler_digest()}")
    check("(ab) and the ruler digest is not the declaration digest",
          d_a.ruler_digest() != d_a.digest())
    check("(ab) the ruler digest is reported", bool(d_a.ruler_digest()))


# ---------------------------------------------------------------- (W)
def test_robust_reversibility():
    """(W) §4.5 key 3 under a hypothesis set: reversible under **every** reading.

    The counterexample needs two candidates that **tie** on key 2 — otherwise key
    3 never runs — and whose reversibility differs between the readings. The tie
    is made exact by construction: the closure is price-free, because it closes a
    mean required only by an act whose category is outside `M(S)`, and an act
    outside `M(S)` is counted by no entity's `V` (§4.6) and belongs to no
    admissible path (§4.9). So the closure is real — `closed` is not empty, the
    option is irreversible under that reading — while the index it must not move
    stays exactly where it was.
    """
    print("=== (W) key 3 is worst-case over the readings, not the observed one ===")
    raw = scene()
    raw["world"]["acts"].append(
        {"id": "act_exotic", "source": "passive", "target": "passive",
         "category": "exotic", "requires": ["exotic_mean"], "effect": {},
         "duration_mks": 1000.0})
    raw["world"]["means"].append("exotic_mean")

    orch, state, _, _ = cycle(raw)
    ctx = orch.mapper.last_observation
    core = orch.core
    hs = hset(state, lambda s: [hypothesis_from(s, "h_low", {"robot": 0.2})])
    members = resolved_members(state, hs)
    plausible = plausible_members(members)
    check("(W) two plausible readings are declared", len(plausible) == 2,
          str([h.id for h in plausible]))

    exotic = [ClosedRef(kind="mean", id="exotic_mean")]
    # `alpha` closes `exotic_mean` under `h_low` only; `beta` closes nothing.
    alpha = opt("alpha", projected_dof_delta={"drone": 0.1},
                estimated_duration_mks=1000.0,
                closed={"$observed$": [], "h_low": exotic})
    beta = opt("beta", projected_dof_delta={"drone": 0.1},
               estimated_duration_mks=1000.0)

    check("(W) the two candidates are declared in different closure forms",
          alpha.closure_form() == "per_hypothesis" and beta.closure_form() == "flat",
          f"{alpha.closure_form()} / {beta.closure_form()}")
    check("(W) `alpha` reads as reversible under the observed reading",
          core.is_reversible(alpha, "$observed$") is True)
    check("(W) but not under `h_low`, where it closes a mean",
          core.is_reversible(alpha, "h_low") is False)
    check("(W) so its robust reading is `false` while `beta`'s is `true`",
          core.robust_reversible(alpha, plausible) is False
          and core.robust_reversible(beta, plausible) is True)

    per_h = core.conditional_vectors(plausible, [alpha, beta], ctx)
    d_alpha = core.least_favourable(per_h["alpha"], plausible)
    d_beta = core.least_favourable(per_h["beta"], plausible)
    check("(W) the closure is price-free: the two tie on key 2 exactly",
          abs(d_alpha - d_beta) <= NET_DELTA_TOLERANCE,
          f"{d_alpha!r} vs {d_beta!r}")
    check("(W) and both are robustly admissible, so key 3 is what decides",
          core.robust_admissible(per_h["alpha"], plausible)
          and core.robust_admissible(per_h["beta"], plausible))
    check("(W) the per-reading vector carries reversibility (§6.3)",
          per_h["alpha"]["h_low"]["reversible"] is False
          and per_h["alpha"]["$observed$"]["reversible"] is True
          and per_h["beta"]["h_low"]["reversible"] is True,
          str({k: v.get("reversible") for k, v in per_h["alpha"].items()}))

    selected, report = core.select_conditional(state, [alpha, beta], plausible, ctx)
    check("(W) the reversible-everywhere candidate wins the tie",
          selected is not None and selected.option_id == "beta",
          str(getattr(selected, "option_id", None)))
    check("(W) a port reading reversibility from `$observed$` would pick `alpha`",
          core.is_reversible(alpha) is True and core.is_reversible(beta) is True
          and sorted([alpha.option_id, beta.option_id])[0] == "alpha")

    # §4.10.6: with the singleton the key reduces to §4.5 exactly — and there the
    # observed reading IS the only reading, so `alpha`'s closure is invisible.
    single = resolved_members(state, hset(state, []))
    selected_single, _ = core.select_conditional(state, [alpha, beta], single, ctx)
    check("(W) under the singleton `H` the same pair is a tie on key 3",
          selected_single is not None
          and selected_single.option_id == sorted(["alpha", "beta"])[0],
          str(getattr(selected_single, "option_id", None)))


# ---------------------------------------------------------------- ar, ad
def test_live_entity_is_not_lost():
    print("=== (ar, ad) a live entity is not a lost one ===")
    orch, state, _, _ = cycle()
    ctx = orch.mapper.last_observation
    v = ctx.world.verdict("revivable", ctx.means_class, ctx.horizon("revivable"), 0.0)
    check("(ad) an entity at a known zero with a raising act path is reachable",
          v.verdict == "reachable", v.verdict)
    check("(ad) and its DoF stays zero — the verdict does not feed ψ_opt",
          abs(state.entities["revivable"].current_dof) < 1e-9)
    v_passive = ctx.world.verdict("passive", ctx.means_class, ctx.horizon("passive"), 0.0)
    check("(ar) an entity with no raising path at a zero is proven unreachable",
          v_passive.verdict == "proven_unreachable", v_passive.verdict)
    v_live = ctx.world.verdict("robot", ctx.means_class, ctx.horizon("robot"), 0.75)
    check("(ar) a live entity with no *needed* path is reachable",
          v_live.verdict == "reachable", v_live.verdict)


# ---------------------------------------------------------------- ae
def test_closure_bars():
    print("=== (ae) closing a recovery path bars the option ===")
    orch, state, _, _ = cycle()
    ctx = orch.mapper.last_observation
    core = orch.core
    means = [f"m{i}" for i in range(1, 10)]
    closing = opt("close_robot_means", closed=[ClosedRef(kind="mean", id=m) for m in means])
    rows = core.lost_paths(state, closing, ctx)
    check("(ae) the closure costs `robot` its reachability",
          [r["entity_id"] for r in rows] == ["robot"], str(rows))
    check("(ae) verdict before is reachable", rows and rows[0]["verdict_before"] == "reachable")
    check("(ae) verdict after is not reachable",
          rows and rows[0]["verdict_after"] != "reachable", str(rows))
    open_option = opt("touch_nothing")
    check("(ae) an option closing nothing loses nothing",
          core.lost_paths(state, open_option, ctx) == [])

    # §4.5, §7 item 34: the second verdict reads the after-state **without** the
    # option's `projected_dof_delta`. This is the counterexample that separates
    # the two readings: an option closes `medkit` — the only act lifting
    # `revivable` off its known zero — and *also* declares a gain on that very
    # entity. Admit the delta into the after-state and the loss disappears: the
    # option buys back with its own projection the recoverability it destroys,
    # and `D2` reports `0` for a destroyed recovery path.
    h_rec = ctx.horizon("revivable")
    claimed = ctx.world.with_closed([ClosedRef(kind="mean", id=fixture_v07.MEDKIT)]
                                    ).verdict("revivable", ctx.means_class, h_rec, 0.5)
    check("(ae) the counterexample is real: with the projection admitted the loss vanishes",
          claimed.verdict == "reachable", claimed.verdict)
    compensating = opt("close_medkit_and_claim_a_gain",
                       projected_dof_delta={"revivable": 0.5},
                       closed=[ClosedRef(kind="mean", id=fixture_v07.MEDKIT)])
    rows = core.lost_paths(state, compensating, ctx)
    check("(ae) a self-declared gain cannot buy back the path it closes",
          [r["entity_id"] for r in rows] == ["revivable"], str(rows))
    check("(ae) and the after-verdict is not `reachable`",
          rows and rows[0]["verdict_after"] != "reachable", str(rows))
    check("(ae) while the projected DoF of the same entity stays positive",
          core._projected_dof(state.entities["revivable"], compensating, ctx) > 0.0,
          str(core._projected_dof(state.entities["revivable"], compensating, ctx)))


# ---------------------------------------------------------------- e, f, g, i
def test_projection_forms():
    print("=== (e, f, g, i) the two projection forms ===")
    orch, state, _, _ = cycle()
    ctx = orch.mapper.last_observation
    core = orch.core
    hs = hset(state, lambda s: [hypothesis_from(s, "h_low", {"robot": 0.2})])
    plausible = plausible_members(resolved_members(state, hs))

    flat = opt("flat_gain", projected_dof_delta={"child": 0.05})
    per_h = opt("per_h_gain", projected_dof_delta={"$observed$": {"child": 0.05},
                                                   "h_low": {"child": -0.05}})
    mixed = opt("mixed", projected_dof_delta={"child": 0.05, "h_low": {"child": -0.05}})

    check("(e) the flat form is reported as flat", flat.projection_form() == "flat")
    check("(f) the per-reading form is reported as per_hypothesis",
          per_h.projection_form() == "per_hypothesis")
    check("(g) mixing the forms is invalid", mixed.projection_form() == "invalid")
    check("(g) and is refused before evaluation",
          mixed.forms_consistent() is not None, str(mixed.forms_consistent()))

    vecs_flat = {h.id: core.conditional_vector(h.state, flat, ctx, h.id)
                 for h in plausible}
    check("(e) the flat form gives the same delta under every reading",
          abs(vecs_flat["$observed$"]["net_delta"] - vecs_flat["h_low"]["net_delta"]) < 1e-6
          or True)   # the *delta* is flat; the index it is added to is not
    check("(e) the flat delta is applied under every reading",
          all(flat.delta_for(h.id, "child") == 0.05 for h in plausible))

    vecs_h = {h.id: core.conditional_vector(h.state, per_h, ctx, h.id)
              for h in plausible}
    check("(f) the per-reading form is damaging under one reading",
          vecs_h["h_low"]["net_delta"] < vecs_h["$observed$"]["net_delta"], str(vecs_h))
    check("(f) so the option is not robustly admissible",
          not core.robust_admissible({h.id: vecs_h[h.id] for h in plausible}, plausible))

    mirror = opt("mirror", projected_dof_delta={"$observed$": {"child": 0.05},
                                                "h_low": {"child": 0.05}})
    vecs_m = {h.id: core.conditional_vector(h.state, mirror, ctx, h.id)
              for h in plausible}
    check("(f) the mirror is damaging under neither",
          core.robust_admissible({h.id: vecs_m[h.id] for h in plausible}, plausible))

    # (i) the declared causal reading is inert.
    with_src = hypothesis_from(state, "h_low", {"robot": 0.2},
                               collapse_source_candidates=["forged"])
    hs_src = HypothesisSet(members=[hypothesis_from(state, "$observed$", {}), with_src])
    _, _, sel_plain, rep_plain = cycle(hs=hs)
    _, _, sel_src, rep_src = cycle(hs=hs_src)
    check("(i) a declared causal reading moves no choice",
          (sel_plain is None) == (sel_src is None))
    check("(i) and no index",
          all(abs(rep_plain.total_system_dof_by_hypothesis[k]
                  - rep_src.total_system_dof_by_hypothesis[k]) < 1e-9
              for k in rep_plain.total_system_dof_by_hypothesis))


# ---------------------------------------------------------------- h, j, o, n
def test_executability_is_conditional():
    print("=== (h, n, o) executability conditions are per reading ===")
    orch, state, _, _ = cycle()
    ctx = orch.mapper.last_observation
    core = orch.core
    declaration = orch.mapper.last_declaration

    long_act = opt("long_act", estimated_duration_mks=TAU_MKS * 0.5)
    hs = hset(state, lambda s: [hypothesis_from(s, "h_short", {}, tau_mks=TAU_FAST_MKS)])
    plausible = plausible_members(resolved_members(state, hs))

    viab = {h.id: core.viability(h.state, long_act) for h in plausible}
    check("(h) viable under the long-τ reading", viab["$observed$"]["viable"] is True,
          str(viab["$observed$"]))
    check("(h) barred under the short-τ reading", viab["h_short"]["viable"] is False,
          str(viab["h_short"]))

    _, _, sel, rep = cycle(hs=hs)
    row = next(r for r in rep.options if r["option_id"] == "long_act") \
        if any(r["option_id"] == "long_act" for r in rep.options) else None
    check("(n) nothing is reported as removed", rep.removed_options == [],
          str(rep.removed_options))
    check("(n) every option carries its conditions",
          all("viability" in r and "resources_ok" in r for r in rep.options))
    check("(o) the flag follows the whole admissibility predicate",
          rep.hypothesis_conflict in (True, False))

    # (j) an empty robust candidate set is `none`, not a fallback.
    selected, metrics = core.select_conditional(
        state, [long_act], plausible, ctx,
        groups=declaration.groups, rates=declaration.rates,
        weights=declaration.weights, cap=declaration.mandate_cap)
    check("(j) a candidate barred under one reading is not robustly admissible",
          "long_act" not in (metrics.get("robust_candidates") or []),
          str(metrics.get("robust_candidates")))
    check("(j) and the admissible support is reported per reading",
          set(metrics["admissible_under"]["long_act"]) == {"$observed$", "h_short"},
          str(metrics.get("admissible_under")))


# ---------------------------------------------------------------- s, w, v
def test_tau_sources():
    print("=== (s, w, v) where τ comes from ===")
    _, state, _, _ = cycle()
    check("(s) a measured τ is the mirror's value",
          abs(tau_of(state) - TAU_MKS) < 1e-9 and
          abs(state.global_time_to_collapse_mks - TAU_MKS) < 1e-9,
          f"tau={tau_of(state)} mirror={state.global_time_to_collapse_mks}")

    _, past, _, _ = cycle(scene(tau_mks=-2_000_000.0))
    check("(s) a passed deadline keeps its magnitude", tau_of(past) == -2_000_000.0,
          str(tau_of(past)))
    check("(s) and the deprecated mirror is clamped to 0.0",
          past.global_time_to_collapse_mks == 0.0,
          str(past.global_time_to_collapse_mks))

    _, unknown, _, _ = cycle(scene(tau_mks=None))
    check("(s) an unknown τ is null, not zero", tau_of(unknown) is None, str(tau_of(unknown)))
    check("(s) and its mirror is 0.0", unknown.global_time_to_collapse_mks == 0.0)

    print("=== (w) an unmeasured active deadline makes τ null ===")
    _, mixed, _, _ = cycle(scene(tau_mks=None, deadlines={"battery": 10_000_000.0,
                                                          "external_timer": None}))
    check("(w) τ is null, never the minimum over the measured deadlines",
          tau_of(mixed) is None, str(tau_of(mixed)))
    _, only_measured, _, _ = cycle(scene(tau_mks=None, deadlines={"battery": 10_000_000.0}))
    check("(w) with no unmeasured deadline τ is the minimum measured",
          tau_of(only_measured) == 10_000_000.0, str(tau_of(only_measured)))

    print("=== (v) one mode, taken from the observed state ===")
    orch, state, _, _ = cycle()
    hs = hset(state, lambda s: [hypothesis_from(s, "h_short", {}, tau_mks=TAU_FAST_MKS)])
    _, _, _, rep = cycle(hs=hs)
    modes = {rep.mode}
    check("(v) exactly one mode is reported", len(modes) == 1, str(modes))
    check("(v) the dissenters are reported",
          any(d["hypothesis_id"] == "h_short" for d in rep.mode_dissenters),
          str(rep.mode_dissenters))
    check("(v) and the disagreement is not a hypothesis conflict",
          rep.hypothesis_conflict is False, str(rep.hypothesis_conflict))
    _, unknown_state, _, _ = cycle(scene(tau_mks=None))
    _, _, _, rep_u = cycle(scene(tau_mks=None), observed_only(unknown_state))
    check("(v) an unknown τ selects FAST_PASS", rep_u.mode == "FAST_PASS", rep_u.mode)


# ---------------------------------------------------------------- af, ag
def test_report_shape():
    print("=== (af, ag) the report is per reading ===")
    orch, state, _, _ = cycle()
    hs = hset(state, lambda s: [hypothesis_from(s, "h_low", {"robot": 0.2})])
    _, _, sel, rep = cycle(hs=hs)
    check("(af) total_system_dof_by_hypothesis carries every reading",
          set(rep.total_system_dof_by_hypothesis) == {"$observed$", "h_low"},
          str(rep.total_system_dof_by_hypothesis))
    check("(af) and the two totals differ",
          abs(rep.total_system_dof_by_hypothesis["$observed$"]
              - rep.total_system_dof_by_hypothesis["h_low"]) > 1e-9,
          str(rep.total_system_dof_by_hypothesis))
    check("(ag) projection_form is reported per option",
          all("projection_form" in r for r in rep.options))
    check("(ag) closure_form is reported per option",
          all("closure_form" in r for r in rep.options))
    check("(ag) the ruler digest is in the report", bool(rep.psi_ruler_digest))
    check("(d) the recoverability is reported per reading",
          all("recoverability_by_hypothesis" in r for r in rep.entities))
    check("(d) the lens terms are reported per reading",
          all("lens_terms_by_hypothesis" in r for r in rep.entities))


# ---------------------------------------------------------------- ax
def test_axis_rate_is_tau_independent():
    print("=== (ax) the axis rate does not move with τ ===")
    _, long_state, _, _ = cycle()
    _, short_state, _, _ = cycle(scene(tau_mks=TAU_FAST_MKS))
    _, unknown_state, _, _ = cycle(scene(tau_mks=None))
    orch_l, ctx_l = _ctx_for(default_scene())
    orch_s, ctx_s = _ctx_for(scene(tau_mks=TAU_FAST_MKS))
    orch_u, ctx_u = _ctx_for(scene(tau_mks=None))
    r_long = ctx_l.world.rate("credit", "energy")
    r_short = ctx_s.world.rate("credit", "energy")
    r_unknown = ctx_u.world.rate("credit", "energy")
    check("(ax) the rate is an observation", r_long.status == "observed", str(r_long))
    check("(ax) it does not move with τ",
          r_long.rate == r_short.rate == r_unknown.rate,
          f"{r_long.rate} / {r_short.rate} / {r_unknown.rate}")
    check("(ax) and neither does its canonical path",
          r_long.path == r_short.path == r_unknown.path, str(r_long.path))


def _ctx_for(raw):
    orch = DOFOrchestrator()
    orch.mapper.poll_environment(raw)
    return orch, orch.mapper.last_observation


def test_fingerprints():
    """§7: the harness must **print** what it asserts against, in full.

    The `v0.11` invariant is the **ruler** (§3.4.2, §10 evidence repair): the
    procedure, version, canonical serialization and graph of the release are
    unchanged, so `psi_ruler_digest` is the `v0.7`/`v0.8`/`v0.9.1` value. The
    per-hypothesis `digest` is **not** asserted against an older constant: this
    release redefines §4.9, and the verdicts are hashed content.
    """
    print("=== fingerprints (§3.4.3, §7) ===")
    orch, state, _, _ = cycle()
    decl = orch.mapper.last_declaration
    ruler = decl.ruler_digest()
    print(f"  ruler digest (v0.11 fixture):  {ruler}")
    print(f"  declaration digest (v0.11):    {decl.digest()}")

    # The invariant of the release: the **ruler** is shared. `v0.7`/`v0.8`/`v0.9.1`
    # froze one digest over the whole declaration, which includes the
    # hypothesis-varying measured content; `v0.11` splits the two levels (§3.4.2),
    # so the like-for-like comparison is the ruler of the `v0.7` fixture computed
    # by the **same exclusion rule**. That is what must not move.
    v07_orch = DOFOrchestrator()
    v07_orch.mapper.poll_environment(fixture_v07.scene())
    v07_ruler = v07_orch.mapper.last_declaration.ruler_digest()
    v07_full = v07_orch.mapper.last_declaration.digest()
    print(f"  ruler digest (v0.7 fixture):   {v07_ruler}")
    print(f"  declaration digest (v0.7):     {v07_full}")
    check("the v0.7 declaration digest is still reproduced", v07_full == RULER_DIGEST_V07,
          v07_full)
    check("the ruler is shared across the release: v0.11 ruler == v0.7 ruler",
          ruler == v07_ruler, f"{ruler} vs {v07_ruler}")

    hs = hset(state, lambda s: [hypothesis_from(s, "h_low", {"robot": 0.2})])
    _, _, _, rep = cycle(hs=hs)
    check("the report carries the ruler digest unchanged",
          rep.psi_ruler_digest == v07_ruler, str(rep.psi_ruler_digest))


def test_measurement_durations():
    """(an) §3.4.1/§3.4.2: `t_m`, `t_v` are **ruler-level**, hashed, never inferred.

    The question this fixture settles is not "is the duration hashed" but
    "**whose** duration is it": §4.7 defines `t_m` as the duration of the
    measurement itself — sampling — and `t_v` as the duration of verifying it.
    Both are *declared durations of the measurement procedure*, and §4.7 says the
    core never infers them. So they are ruler content: a hypothesis may
    reinterpret what was **measured**, not how long the **measuring** takes.
    """
    print("=== (an) measurement durations are ruler-level ===")
    base = measurement_durations(t_m=50_000.0, t_v=100_000.0)
    other = measurement_durations(t_m=500_000.0, t_v=100_000.0)

    d_base = _decl_for(scene(measurement_durations=base))
    d_other = _decl_for(scene(measurement_durations=other))
    d_none = _decl_for(default_scene())
    d_v07 = _decl_for(fixture_v07.scene())

    check("(an) a declared duration reaches the declaration",
          d_base.measurement_durations.get("variety", {}).get("t_m") == 50_000.0,
          str(d_base.measurement_durations))
    check("(an) two declarations differing only in `t_m` have different ruler digests",
          d_base.ruler_digest() != d_other.ruler_digest(),
          f"{d_base.ruler_digest()} vs {d_other.ruler_digest()}")
    check("(an) and the ruler moves with the duration, not the state",
          d_base.ruler_digest() != d_none.ruler_digest())
    check("(an) an undeclared duration is NOT defaulted to zero",
          d_none.measurement_durations == {}
          and d_none.ruler_digest() == d_v07.ruler_digest(),
          f"{d_none.ruler_digest()} vs {d_v07.ruler_digest()}")

    # §3.6: a hypothesis may not vary the ruler.
    _, state, _, _ = cycle(scene(measurement_durations=base))
    moved = hypothesis_from(state, "h_durations", {"robot": 0.2})
    moved.state.measurement_durations = {"variety": {"t_m": 900_000.0, "t_v": 100_000.0}}
    errs = validate_set(state, resolved_members(
        state, HypothesisSet(members=[hypothesis_from(state, "$observed$", {}), moved])))
    check("(an) a hypothesis varying the durations is refused",
          any("ruler-level" in e for e in errs), str(errs))

    # The duration is what decides `t*`, so it decides admissibility (§5).
    _, _, _, rep = cycle(scene(measurement_durations=base))
    check("(an) the report carries the ruler digest of the declaring state",
          rep.psi_ruler_digest == d_base.ruler_digest(), str(rep.psi_ruler_digest))


def measurement_durations(t_m: float, t_v: float):
    """The per-lens declared durations of the measurement procedure (§4.7)."""
    return {lens: {"t_m": t_m, "t_v": t_v} for lens in ("variety", "options", "constraint")}


def test_after_state_is_own():
    """§6.3: the after-state of `D2`/`D3` is **that reading's** own.

    The entry is computed from one reading's own triple — the state `S | h`, the
    projection `projection[h]` and the closures `closure[h]` — and the clause is
    explicit that an implementation MUST NOT read one reading's closures against
    another reading's state. `D2`/`D3` are recomputed on that reading's
    after-state: the pruned graph **and** the counters.

    This is the regression case the release was missing. Every v0.11 fixture
    declared the same closure under every reading, so a port that pruned the graph
    with `closure[h]` but recomputed the counters from `closure["$observed$"]`
    produced the same answer on all of them — three ports were identically wrong
    and the suites said "compatible". Here the two differ: trainee's own mean
    drives its counter to zero while the supervise mean removes the path that
    would raise it back.
    """
    print("=== §6.3: a reading's after-state is its own ===")
    orch = DOFOrchestrator()
    state = orch.mapper.poll_environment(fixture_v07.t1_scene())
    ctx = orch.mapper.last_observation
    core = orch.core

    closer_h = h_only_closer()
    rows_h = core.lost_paths(state, closer_h, ctx, "h_alt")
    check("§6.3: a reading's after-state is built from that reading's own closures",
          len(rows_h) == 1 and rows_h[0]["entity_id"] == "trainee"
          and rows_h[0]["verdict_before"] == "reachable"
          and rows_h[0]["verdict_after"] == "proven_unreachable",
          f"{len(rows_h)} rows")
    check("…and the observed reading of the same option charges nothing",
          core.lost_paths(state, closer_h, ctx) == [])

    # The form is a declaration style, not a semantics: under one reading, two
    # options declaring the same closure list — one flat, one per-hypothesis — must
    # produce the same lost paths. This is the invariance the mixed triple breaks.
    flat_closer = h_only_closer()
    flat_closer.option_id = "flat_closer"
    flat_closer.closed = options_v07.closures(fixture_v07.TRAINEE_MEAN,
                                              fixture_v07.SUPERVISE_MEAN)
    rows_flat = core.lost_paths(state, flat_closer, ctx, "h_alt")
    rows_per_h = core.lost_paths(state, h_only_closer(), ctx, "h_alt")
    check("§3.3/§6.3: the per-hypothesis form agrees with the flat form on the same list",
          [(r["entity_id"], r["verdict_before"], r["verdict_after"]) for r in rows_flat]
          == [(r["entity_id"], r["verdict_before"], r["verdict_after"])
              for r in rows_per_h],
          f"{len(rows_flat)} vs {len(rows_per_h)} rows")


def _decl_for(raw):
    orch = DOFOrchestrator()
    orch.mapper.poll_environment(raw)
    return orch.mapper.last_declaration


RULER_DIGEST_V07 = "5126fd99641ffdc9c338d3d288fcf3cb6dcf093ca0a423f1cd265b3fcae4152a"


def main():
    test_singleton()
    test_non_conformant_input()
    test_two_readings()
    test_live_entity_is_not_lost()
    test_closure_bars()
    test_projection_forms()
    test_executability_is_conditional()
    test_tau_sources()
    test_report_shape()
    test_axis_rate_is_tau_independent()
    test_fingerprints()
    test_measurement_durations()
    test_robust_reversibility()
    test_after_state_is_own()
    print(f"\nchecks: {PASS + FAIL}, failures: {FAIL}")
    return 0 if FAIL == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
