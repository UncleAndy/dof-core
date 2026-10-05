"""Hypothesis set artifact (DOF-SPEC §3.6) and its validation rules.

`v0.10` made the *state* conditional; `v0.11` repaired the artifact so a
hypothesis is a **complete alternative state** under the **shared ruler**
(§3.6, §10(A)):

* a hypothesis supplies the *measured* content — the per-entity lens counters,
  the measurement durations and the resource map, from which τ follows;
  `current_dof`, `dof_known` and τ are **computed** from it by the same named
  procedures, and a stated DoF that its own counters do not produce is
  non-conformant input;
* the **ruler** (§3.4.2) and the observed graph `G` (§3.5) are **shared** —
  structural uncertainty is *priced, not branched*;
* `is_collapse_source` is **not** hypothesis-local: the label is honoured
  through an observed act, the act comes from the shared graph, and a set that
  moves the label between readings is non-conformant input;
* absence and emptiness are the **observed-state singleton**, with
  `plausible = true`, so `H_plausible` is never empty and the worst-case
  operators of §4.10 are total.

The core accepts `H` as supplied: it MUST NOT add, merge, split, drop, reorder
or re-weight a hypothesis, and MUST NOT compute the `plausible` flag.
"""

import math
from typing import Dict, List, Optional, Sequence

from pydantic import BaseModel, Field

from calculus_core import SystemStateMatrix, tau_of, mirror_time_to_collapse
from measurement import LENS_ORDER


class Hypothesis(BaseModel):
    """§3.6: one declared interpretation of the same observed state."""

    id: str
    plausible: bool = True
    state: SystemStateMatrix
    # Report context (§3.4.1): the declared causal reading. Inert — it MUST NOT
    # affect `calc`, the collapse charges, the §4.9 verdicts or the Axiom-3
    # exemption. An empty list reads as "names no possible source", never as
    # "asserts that no source exists".
    collapse_source_candidates: List[str] = Field(default_factory=list)
    basis: str = ""


class HypothesisSet(BaseModel):
    """§3.6: the artifact. `coverage` is a claim, and its default is cautious."""

    coverage: str = "partial"          # absent => "partial", never "complete"
    members: List[Hypothesis] = Field(default_factory=list)
    horizon_mks: Optional[float] = None   # the analysis horizon declared with the set


def observed_singleton(state: SystemStateMatrix) -> List[Hypothesis]:
    """§3.6/§4.10.6: an absent or empty `H` **is** the observed-state singleton."""
    return [Hypothesis(id="$observed$", plausible=True, state=state,
                       basis="absence or emptiness of H is the observed state")]


def resolved_members(state: SystemStateMatrix,
                     hset: Optional[HypothesisSet]) -> List[Hypothesis]:
    """`H` as the core reads it, including the absence/emptiness reduction."""
    if hset is None or not hset.members:
        return observed_singleton(state)
    return list(hset.members)


def plausible_members(members: Sequence[Hypothesis]) -> List[Hypothesis]:
    """`H_plausible = { h ∈ H : h.plausible }`.

    Never empty: the set is either the observed singleton or a declared set
    whose observed member MUST be present and plausible (§3.6), so a set that
    resolves to nothing is non-conformant input caught by `validate_set`.
    """
    out = [h for h in members if h.plausible]
    return out or list(members[:1])


def _lens_product(entity) -> Optional[float]:
    """The entity's DoF as the product of its **own** lens values (§4.1, §4.6).

    `None` when the entity carries no measurement declaration: the port-level
    `measurement` object is the carrier of the counters, and without it the
    identity cannot be checked. A missing declaration is not a contradiction.
    """
    m = getattr(entity, "measurement", None)
    if m is None:
        return None
    product = 1.0
    for lens in LENS_ORDER:
        value = m.psi.get(lens)
        if value is None:
            return None
        product *= float(value)
    return product


def validate_set(state: SystemStateMatrix,
                 members: Sequence[Hypothesis]) -> List[str]:
    """§3.6 non-conformance checks. An empty list means the input is admissible.

    Every rule here is a *check*, not a hope: each one corresponds to a way a
    manipulated or careless hypothesis set could otherwise move the index, the
    choice or the collapse-source label without leaving a trace.
    """
    errs: List[str] = []
    ids = [h.id for h in members]
    if len(set(ids)) != len(ids):
        errs.append("hypothesis ids are not unique")
    observed_present = False
    for h in members:
        hs = h.state
        # (1) State completeness: every entity of `S` appears in every reading,
        #     with an explicit value, never omitted.
        missing = sorted(set(state.entities) - set(hs.entities))
        if missing:
            errs.append(f"{h.id}: omits entities {missing} (§3.6 state completeness)")
        extra = sorted(set(hs.entities) - set(state.entities))
        if extra:
            errs.append(f"{h.id}: declares entities not in the observed state {extra}")
        # (2) The lens identity holds under every hypothesis: the stated DoF must
        #     equal the product of that reading's OWN counters (§4.1, §3.6).
        for e_id, ent in sorted(hs.entities.items()):
            product = _lens_product(ent)
            if product is None:
                continue                       # no counters carried: nothing to check
            if abs(product - ent.current_dof) > 1e-9:
                errs.append(
                    f"{h.id}/{e_id}: stated DoF {ent.current_dof!r} differs from the "
                    f"product of its own lens values {product!r} (§4.1, §3.6)")
        # (3) `is_collapse_source` is not hypothesis-local: the label is honoured
        #     through an act of the SHARED graph, so it must equal the observed
        #     value under every reading (§3.6).
        for e_id, ent in sorted(hs.entities.items()):
            obs = state.entities.get(e_id)
            if obs is not None and bool(ent.is_collapse_source) != bool(obs.is_collapse_source):
                errs.append(
                    f"{h.id}/{e_id}: is_collapse_source differs from the observed "
                    f"value — the label is not hypothesis-local (§3.6)")
        # (5) §3.4.1/§3.4.2 (v0.11): the measurement durations are **ruler-level**.
        #     A hypothesis reinterprets what was *measured*; it may not
        #     reinterpret how long the *measuring* takes. If it could, `T_meas`
        #     and hence `t*` would differ between readings that claim one ruler,
        #     and `min_h NetDelta(o | h)` would compare numbers produced by
        #     different measuring systems — which is what the shared
        #     `psi_ruler_digest` exists to make impossible.
        if dict(hs.measurement_durations) != dict(state.measurement_durations):
            errs.append(
                f"{h.id}: measurement durations differ from the observed "
                f"procedure — the durations are ruler-level, not hypothesis-level "
                f"(§3.4.1, §3.4.2)")
        # (4) The observed state MUST be one of the readings (§3.6).
        if _same_state(hs, state):
            observed_present = True
    if not observed_present:
        errs.append("the observed state is absent from H (§3.6: it MUST be present)")
    if not any(h.plausible for h in members) and len(members) > 1:
        # A single-member set is the observed singleton and is plausible by
        # construction; a larger set that marks everything implausible leaves
        # `H_plausible` empty, which §3.6 declares an invalid input.
        errs.append("every hypothesis is implausible (§3.6: H_plausible would be empty)")
    return errs


def _same_state(a: SystemStateMatrix, b: SystemStateMatrix) -> bool:
    """Whether two matrices are the same *measured* state.

    Only the measured content is compared: the per-entity DoF, the lens
    counters, the resource map and τ. Report context and provenance are not
    part of the comparison — two readings that differ only in a `basis` string
    are the same state.
    """
    if set(a.entities) != set(b.entities):
        return False
    for e_id, ea in a.entities.items():
        eb = b.entities[e_id]
        if abs(ea.current_dof - eb.current_dof) > 1e-9:
            return False
        if bool(ea.dof_known) != bool(eb.dof_known):
            return False
        ma, mb = ea.measurement, eb.measurement
        if (ma is None) != (mb is None):
            return False
        if ma is not None:
            for lens in LENS_ORDER:
                va, vb = ma.psi.get(lens), mb.psi.get(lens)
                if (va is None) != (vb is None):
                    return False
                if va is not None and abs(float(va) - float(vb)) > 1e-9:
                    return False
    if set(a.resources) != set(b.resources):
        return False
    for r, ra in a.resources.items():
        rb = b.resources[r]
        if (ra.value is None) != (rb.value is None):
            return False
        if ra.value is not None and abs(float(ra.value) - float(rb.value)) > 1e-9:
            return False
    ta, tb = a.tau, b.tau
    if (ta is None) != (tb is None):
        return False
    if ta is not None and tb is not None:
        if (ta.value is None) != (tb.value is None):
            return False
        if ta.value is not None and abs(float(ta.value) - float(tb.value)) > 1e-9:
            return False
    return True
