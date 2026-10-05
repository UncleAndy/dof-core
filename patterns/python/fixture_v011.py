"""DOF-SPEC v0.11 fixture — the hypothesis set, and the repaired reachability.

`v0.11` is the first release whose central object is **not** a single state: a
decision is taken over a declared set `H` of complete alternative states under
one shared ruler (§3.6), and the reachability verdict of §4.9 is computed **per
hypothesis** while the graph it is computed on stays shared.

This fixture therefore builds, from the `v0.7` world (one world for all four
ports, §11.10) and a resource layer carrying `τ`:

* a **scene** — the raw observation, with `τ` as a `ResourceObservation` in the
  resource map and the deprecated `time_to_collapse_mks` present but read by no
  rule;
* **hypotheses** — complete alternative states derived from the observed one by
  changing the *measured* content and recomputing `current_dof` from the
  entity's **own** counters, so the lens identity of §4.1 holds under every
  reading by construction rather than by assertion;
* the **counter-scenes** the evidence list of §10 names: a stated DoF that its
  own lenses do not produce (b), a moved collapse-source label (c), a `τ` that
  clears under one reading and fails under another (h), a resolvable unknown
  whose only measuring candidate is barred (t), a deadline pair with one
  unmeasured member (w), and a passed deadline whose magnitude must survive (p).

Nothing here depends on the candidate set: the fixture is an **observation**,
and an observation that changed with the options offered would make the decision
unreproducible (§4.2).
"""
from __future__ import annotations

import copy
from typing import Dict, List, Optional, Sequence

from fixture_v07 import (COUNTING_HORIZON_MKS, EXCHANGES, M_S, MANDATE_CAP,
                         MEANS, NUMERAIRE, RESOURCES, T_REC, entity_specs,
                         graph_acts, graph_entities, graph_means,
                         make_means_resource_obs)
from measurement import LENS_ORDER

# --- §3.2b: τ is a resource, and the deadlines it is derived from -------------
TAU_MKS: float = 10_000_000.0        # 10 s: DEEP_DIVERSIFICATION on its own
TAU_FAST_MKS: float = 1_000_000.0    # 1 s: FAST_PASS


# The per-entity recovery horizon. `v0.11` reads it for **every** entity, not
# only for one sitting at a zero: the verdict of §4.9 is computed per hypothesis
# and compared before and after an option, so an undeclared horizon would make
# the verdict `undetermined` on both sides and hide a lost witness. The value is
# the `v0.7` fixture's `T_rec`, extended to the entities this fixture exercises.
T_REC_V011: Dict[str, float] = {
    "adult": T_REC.get("adult", 4_000_000.0),
    "child": T_REC.get("child", 4_000_000.0),
    "drone": T_REC.get("drone", 4_000_000.0),
    "forged": T_REC.get("forged", 4_000_000.0),
    "robot": T_REC.get("robot", 4_000_000.0),
    "passive": T_REC.get("passive", 4_000_000.0),
    "revivable": T_REC.get("revivable", 4_000_000.0),
    "unobserved": T_REC.get("unobserved", 4_000_000.0),
}


def resource_layer(tau_mks: Optional[float] = TAU_MKS,
                   deadlines: Optional[Dict[str, Optional[float]]] = None,
                   means: Optional[Dict[str, float]] = None,
                   cap: Optional[float] = MANDATE_CAP) -> dict:
    """The resource layer with `τ` in the map (§3.2b, §3.4.1).

    `tau_mks = None` declares τ **unmeasured** — which is not the same as a
    passed deadline, and the two admit different actions (§4.8b, §10(p)).
    """
    out: dict = {
        "means": make_means_resource_obs(dict(means if means is not None else MEANS)),
        "groups": [list(GROUP)],
        "resources": copy.deepcopy(RESOURCES),
        "rates": {
            "credit->energy": {"rate": 0.5, "duration_mks": 1000.0},
            "energy->credit": {"rate": 2.0, "duration_mks": 1000.0},
        },
    }
    # §3.2b: τ **is** an entry of the resource map, and an unmeasured τ is an
    # entry whose `value` is `null` — not an absent key, which would read as
    # "this state tracks no deadline" and fall back to the legacy entity timer.
    tau_obs = make_means_resource_obs({"tau": tau_mks if tau_mks is not None else 0.0})["tau"]
    tau_obs["value"] = None if tau_mks is None else float(tau_mks)
    tau_obs["unit"] = "us"
    out["means"]["tau"] = tau_obs
    # The individual deadlines τ is derived from (§3.2b). `tau` itself is the
    # derived singleton when no individual deadline is tracked.
    out["deadlines"] = dict(deadlines if deadlines is not None else {})
    mandate: dict = {"scope": "household", "external_limit_credit": 100.0}
    if cap is not None:
        mandate["cap"] = cap
    out["mandate"] = mandate
    return out


GROUP: List[str] = list(MEANS.keys())


def scene(tau_mks: Optional[float] = TAU_MKS,
          deadlines: Optional[Dict[str, Optional[float]]] = None,
          means: Optional[Dict[str, float]] = None,
          include_forged_kill: bool = False,
          observation_overrides: Optional[Dict[str, str]] = None,
          t_rec: Optional[Dict[str, float]] = None,
          measurement_durations: Optional[Dict[str, Dict[str, float]]] = None,
          measurement_schedule: Optional[Dict[str, float]] = None) -> dict:
    """A complete raw observation for a `v0.11` cycle."""
    out: Dict[str, dict] = copy.deepcopy(entity_specs())
    out["resource_layer"] = resource_layer(tau_mks=tau_mks, deadlines=deadlines,
                                           means=means)
    out["world"] = {
        "entities": graph_entities(observation_overrides),
        "means": graph_means(),
        "acts": graph_acts(include_forged_kill),
        "exchanges": copy.deepcopy(EXCHANGES),
        "means_class": list(M_S),
        "t_rec": dict(t_rec if t_rec is not None else T_REC_V011),
        "counting_horizon_mks": COUNTING_HORIZON_MKS,
        "numeraire": NUMERAIRE,
        "procedure": "perception-v1:world_verdicts",
    }
    if measurement_durations is not None:
        out["measurement_durations"] = measurement_durations
    if measurement_schedule is not None:
        out["measurement_schedule"] = measurement_schedule
    return out


def default_scene() -> dict:
    return scene()


# --- §3.6: hypotheses are complete alternative states -------------------------
def _rescale(entity, target_dof: float) -> None:
    """Set an entity's Variety lens so its **own** product equals `target_dof`.

    The lens identity of §4.1 is what makes a hypothesis checkable: a reading
    states the *measured* content, and `current_dof` follows from it. A fixture
    that assigned the product directly would be testing the defect `v0.11`
    repairs.
    """
    m = entity.measurement
    product = 1.0
    for lens in LENS_ORDER:
        v = m.psi.get(lens)
        if v is None:
            raise ValueError("fixture: hypothesis over a partially measured entity")
        product *= float(v)
    if product <= 0.0:
        raise ValueError("fixture: cannot rescale a zero product")
    entity.measurement.psi["variety"] = float(target_dof) / (product / float(m.psi["variety"]))
    entity.current_dof = float(target_dof)
    entity.measurement.current_dof = float(target_dof)


def hypothesis_from(observed, h_id: str, dof_overrides: Dict[str, float],
                    plausible: bool = True, tau_mks: Optional[float] = None,
                    collapse_source_candidates: Optional[Sequence[str]] = None,
                    break_lens_identity: bool = False,
                    move_collapse_source: Optional[str] = None,
                    basis: str = ""):
    """Build one reading as a **complete** copy of the observed state.

    `tau_mks` overrides that reading's τ (the measured content of the resource
    map), which is how §4.10's *executability is conditional too* is exercised:
    an option may clear τ under one reading and fail it under another.
    """
    from hypothesis import Hypothesis

    hs = observed.model_copy(deep=True)
    for e_id, dof in dof_overrides.items():
        ent = hs.entities.get(e_id)
        if ent is None:
            raise ValueError(f"fixture: no entity {e_id!r} in the observed state")
        if break_lens_identity:
            # The defect (b) of the evidence list: the stated DoF is not the
            # product of this reading's own counters.
            ent.current_dof = float(dof)
            ent.measurement.current_dof = float(dof)
        else:
            _rescale(ent, float(dof))
    if move_collapse_source is not None:
        # The defect (c): a hypothesis moving the collapse-source label. The
        # label is honoured through an act of the SHARED graph, so it is not
        # hypothesis-local (§3.6).
        ent = hs.entities[move_collapse_source]
        ent.is_collapse_source = not bool(ent.is_collapse_source)
    if tau_mks is not None:
        hs.resources["tau"].value = float(tau_mks)
        hs.tau.value = float(tau_mks)
    return Hypothesis(id=h_id, plausible=plausible, state=hs,
                      collapse_source_candidates=list(collapse_source_candidates or []),
                      basis=basis)


def hset(observed, readings, coverage: str = "partial",
         horizon_mks: Optional[float] = None):
    """A declared hypothesis set around the observed state (§3.6)."""
    from hypothesis import HypothesisSet

    members = [hypothesis_from(observed, "$observed$", {}, basis="observed")]
    members.extend(readings(observed) if callable(readings) else readings)
    return HypothesisSet(coverage=coverage, members=members, horizon_mks=horizon_mks)


def observed_only(observed):
    """The singleton `H` a bare state reads as (§4.10.6)."""
    from hypothesis import HypothesisSet

    return HypothesisSet(members=[hypothesis_from(observed, "$observed$", {},
                                                  basis="observed singleton")])
