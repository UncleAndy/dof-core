"""Measurement layer of the Python port (DOF-SPEC §3.4, §4.6, §4.7, §4.8 — v0.6).

This module is the **Perception side** of the port: it turns raw lens inputs
into `current_dof`, produces the frozen measurement declaration and its digest,
and computes the per-lens terms of the evaluation index.

Three lenses, one product (§4.6):

    ψ_var = V / (V + V_env)                                  (Variety)
    ψ_opt = Π_g f_g(x_g),  f_g(x) = 4^(−x),  x_g = c_g / C_g  (Options)
    ψ_con = F / (F + F_env)                                  (Constraint)

    current_dof = ψ_var · ψ_opt · ψ_con

Degenerate case (guard, uniform across lenses): if a lens has neither a
numerator nor an external clamp, its value is 0. The rule keeps the index
total — `0/0` would yield NaN, and one NaN poisons the whole sum — and removes
any freedom to choose a convention. A zero lens is not a verdict: it makes
`current_dof = 0`, and §4.2 then decides exclusion.

Unmeasured lenses are not zero and not ideal (§4.7): they enter the product as
the declared ignorance factor `u(t)`, the entity's `dof_known` becomes false,
and §4.2 keeps it in the calculation set.

Resource layer (v0.6). The block-level `(c_g, C_g)` the Options lens consumes
are **derived, not authored** (§4.6): they are computed by the named procedure
`derive_blocks` from the entity's raw per-resource requirements, the acting
agent's means and the derived groups. The ruler carries the resource identities
with their unit name and scale, the groups, the observed rates and the declared
mandate (§3.4.1, §4.8), so a declaration is comparable only if it declares the
same resource units.
"""

from __future__ import annotations

import hashlib
import json
import math
from typing import Dict, List, Optional, Sequence, Tuple

from pydantic import BaseModel

# --- normative constants (DOF-SPEC §4.1, §4.7) ---------------------------------
EPSILON = 1e-6
U_ALPHA = 0.25                     # caution measure of the prior quantile
U_RHO = 0.9                        # share of the collapse penalty as a floor
U_MIN = EPSILON ** (1.0 - U_RHO)   # ≈ 0.2512
U_MAX = 0.5                        # an unmeasured term is never an ideal
LENS_ORDER: Tuple[str, ...] = ("variety", "options", "constraint")

# §4.6 (v0.6): the Options blocks come from a named derivation procedure, which
# is part of the frozen ruler (`procedures["options_blocks"]`).
DERIVE_BLOCKS_PROCEDURE = "derive_blocks"
# §3.3: every option that names an entity MUST declare its energy draw.
MANDATORY_RESOURCE = "energy"


def _clamp01(x: float) -> float:
    return max(0.0, min(1.0, x))


def psi_var(V: float, V_env: float) -> float:
    """Variety lens (§4.6). `V = 0` ⇒ 0, including the (0,0) case."""
    if V <= 0.0:
        return 0.0
    return _clamp01(V / (V + max(V_env, 0.0)))


def psi_con(F: float, F_env: float) -> float:
    """Constraint lens (§4.6). `F = 0` ⇒ 0, including the (0,0) case."""
    if F <= 0.0:
        return 0.0
    return _clamp01(F / (F + max(F_env, 0.0)))


def psi_opt(blocks: Sequence[Tuple[float, float]]) -> float:
    """Options lens (§4.6).

    `blocks` is the requirement/budget pair `(c_g, C_g)` of every resource block.
    An empty repertoire means no reachable transition at all ⇒ 0. A block with
    `c_g = 0` does not participate (`f_g = 1`); a block with `c_g > 0` and
    `C_g = 0` is dead (nothing to exchange with) ⇒ 0, the gate of §4.6.
    """
    if not blocks:
        return 0.0
    value = 1.0
    for c_g, C_g in blocks:
        if c_g <= 0.0:
            continue
        if C_g <= 0.0:
            return 0.0
        value *= 4.0 ** (-(c_g / C_g))
    return _clamp01(value)


def canonical_groups(groups: Optional[Sequence[Sequence[str]]],
                     requirements: Optional[Dict[str, float]] = None,
                     means: Optional[Dict[str, float]] = None) -> List[List[str]]:
    """§4.8: the exchange-group partition is analysis-side and canonical.

    Declared groups are normalized (members sorted, group list sorted); every
    resource that appears in the raw inputs but in no group forms a **singleton
    group** of its own, so a requirement can never be silently dropped from the
    derivation. Keeping the normalization here — and not in each caller — is
    what makes two implementations derive the same blocks from the same inputs.
    """
    named: set = set()
    normalized: List[List[str]] = []
    for group in (groups or []):
        members = sorted({str(r) for r in group})
        if not members:
            continue
        normalized.append(members)
        named.update(members)
    extra = sorted((set(requirements or {}) | set(means or {})) - named)
    normalized.extend([[r] for r in extra])
    return sorted(normalized)


def derive_blocks(requirements: Dict[str, float], means: Dict[str, float],
                  groups: Optional[Sequence[Sequence[str]]] = None,
                  weights: Optional[Dict[str, float]] = None,
                  cap: Optional[float] = None
                  ) -> List[Tuple[float, float]]:
    """§4.6 (v0.7): the derived `(c_g, C_g)` pair of every resource block.

    Named procedure. For each derived group `g`:

        c_g = Σ_{r ∈ g} w_r · requirement_r
        C_g = min( Σ_{r ∈ g} w_r · means_r , cap )

    v0.9: ``means`` may be a dict of floats (legacy) or a dict of
    ResourceObservation dicts. We normalize to floats here.
    """
    means_flat: Dict[str, float] = {}
    for k, v in means.items():
        if isinstance(v, dict):
            means_flat[k] = float(v.get("value") or 0.0)
        else:
            means_flat[k] = float(v)
    w = {str(k): float(v) for k, v in (weights or {}).items()}
    blocks: List[Tuple[float, float]] = []
    for group in canonical_groups(groups, requirements, means_flat):
        c_g = sum(w.get(r, 1.0) * max(0.0, float(requirements.get(r, 0.0))) for r in group)
        C_g = sum(w.get(r, 1.0) * max(0.0, float(means_flat.get(r, 0.0))) for r in group)
        if cap is not None:
            C_g = min(C_g, max(0.0, float(cap)))
        blocks.append((c_g, C_g))
    return blocks


class LensObservation(BaseModel):
    """Raw lens inputs of one entity, as produced by named Perception procedures.

    A lens left as `None` is **unmeasured**: it is not zero and not ideal, and
    §4.7 applies `u(t)` to it.

    The Options lens takes exactly one of two inputs. `options` are the blocks
    themselves (admissible only when they equal what `derive_blocks` computes);
    `requirements` are the raw per-resource requirements, from which the blocks
    are derived against the agent's means and the groups. Raw requirements are
    the honest input of a v0.6 ruler: they cannot be tuned to the agent's own
    stock.
    """

    variety: Optional[Dict[str, float]] = None        # {"V": float, "V_env": float}
    options: Optional[List[Tuple[float, float]]] = None   # [(c_g, C_g), ...]
    constraint: Optional[Dict[str, float]] = None     # {"F": float, "F_env": float}
    requirements: Optional[Dict[str, float]] = None   # {"energy": 4.0, ...} (§4.6)
    # §4.6 (v0.7): the observed rates to the numeraire, and the mandate cap that
    # limits what may be spent. Both belong to the ruler's resource layer.
    weights: Optional[Dict[str, float]] = None
    cap: Optional[float] = None

    def psi(self, lens: str, means: Optional[Dict[str, float]] = None,
            groups: Optional[Sequence[Sequence[str]]] = None,
            weights: Optional[Dict[str, float]] = None,
            cap: Optional[float] = None) -> Optional[float]:
        if lens == "variety":
            if self.variety is None:
                return None
            return psi_var(self.variety.get("V", 0.0), self.variety.get("V_env", 0.0))
        if lens == "options":
            if self.options is not None:
                return psi_opt(self.options)
            if self.requirements is not None:
                return psi_opt(derive_blocks(self.requirements, means or {}, groups,
                                             weights, cap))
            return None
        if lens == "constraint":
            if self.constraint is None:
                return None
            return psi_con(self.constraint.get("F", 0.0), self.constraint.get("F_env", 0.0))
        raise KeyError(lens)


def u0_from_prior(prior_q: Optional[float] = None) -> float:
    """Base level of the ignorance penalty (§4.7).

    `prior_q` is `exp(Q_α(ln D))` — the declared prior quantile in log space.
    The prior is optional and defaults to the point value 0.5, which reduces
    `u₀` to 0.5. The band is a hard limit: the prior is fitted into it, never
    the reverse.
    """
    q = 0.5 if prior_q is None else float(prior_q)
    return max(U_MIN, min(U_MAX, q))


def total_budget_mks(t_m: float, t_v: float, t_a_plus: float = 0.0,
                     t_a_minus: float = 0.0) -> float:
    """`T_meas = t_m + t_v` (§4.7) — the measurement's **own** duration.

    `v0.11` removes the `max(t_a⁺, t_a⁻)` term that `v0.9.1` carried. Bundling the
    duration of a *subsequent* action into the measurement window made the window
    depend on a candidate the Core has not chosen yet, which inverts the
    separation `measurement → obtains information` / `decision → chooses action`:
    the Core is stepwise, and the action that follows a measurement is evaluated
    in the next cycle through its own `estimated_duration_mks`.

    `t_a_plus` / `t_a_minus` are kept in the signature for callers of the older
    revision and are **deliberately ignored**; a port that adds them is
    non-conformant (§4.7, §10(T)).
    """
    return t_m + t_v


def u_of_t(u0: float, tau_mks: Optional[float], t_meas_mks: float,
           t_mks: float = 0.0) -> float:
    """Ignorance penalty `u(t)` (§4.7), continuous at the boundary `t* = 0`.

    ```text
    u(t) = u₀                                        if τ = null
         = ε                                         if τ ≠ null and t* ≤ 0
         = u₀^(1 − t/t*) · ε^(t/t*)                  if τ ≠ null and t* > 0
    ```

    `t* = τ − T_meas` is the point of no return for measurement. The two branches
    are separated deliberately: `lim_{t*→0⁺} u(t*) = ε`, so the closed window must
    price at `ε` (≈ `ln ε = −13.8` nats) rather than at `u₀` (≈ `−0.69`). Pricing
    it at `u₀` produced a jump of ~13 nats exactly where measurement stops being
    possible; the `v0.9.1` wording asserted the `ε` limit while the branch
    returned `u₀`.

    An **undeclared** schedule is read as `t = 0` and therefore as `u₀`; a
    declared `t > t*` is non-conformant input, not a value to clamp.
    """
    if tau_mks is None:
        return u0
    t_star = tau_mks - t_meas_mks
    if t_star <= 0.0:
        return EPSILON
    t = max(0.0, min(t_mks, t_star))
    w = t / t_star
    return (u0 ** (1.0 - w)) * (EPSILON ** w)


class EntityMeasurement(BaseModel):
    """Result of measuring one entity: the terms of the index and the product."""

    entity_id: str
    psi: Dict[str, Optional[float]]          # lens -> value, or None if unmeasured
    terms: List[Dict[str, object]]           # {lens, psi, dof_known, contribution}
    current_dof: float
    dof_known: bool
    contribution: float                      # ln(max(current_dof, ε))
    terms_sum: float                         # Σ of the terms (diagnostics)
    floored: bool                            # the ε-floor was applied at entity level
    binding_lens: Optional[str]              # lowest measured lens; ties → LENS_ORDER
    blocks: List[Tuple[float, float]] = []   # §4.6: the derived (c_g, C_g) actually used
    derivation: Optional[Dict[str, object]] = None  # the named procedure and its inputs
    variety_counters: Optional[Dict[str, float]] = None  # §4.6 (v0.7): the declared counters


def measure_entity(entity_id: str, obs: LensObservation, u_value: float,
                   means: Optional[Dict[str, float]] = None,
                   groups: Optional[Sequence[Sequence[str]]] = None,
                   weights: Optional[Dict[str, float]] = None,
                   cap: Optional[float] = None
                   ) -> EntityMeasurement:
    """Apply §4.6–§4.7 to one entity."""
    if weights is None:
        weights = obs.weights
    if cap is None:
        cap = obs.cap
    psi: Dict[str, Optional[float]] = {}
    terms: List[Dict[str, object]] = []
    product = 1.0
    known_all = True
    terms_sum = 0.0

    for lens in LENS_ORDER:
        value = obs.psi(lens, means, groups, weights, cap)
        psi[lens] = value
        if value is None:
            known_all = False
            contribution = math.log(u_value)
            product *= u_value
        else:
            contribution = math.log(max(value, EPSILON))
            product *= value
        terms_sum += contribution
        terms.append({
            "lens": lens,
            "psi": value,
            "dof_known": value is not None,
            "contribution": contribution,
        })

    contribution = math.log(max(product, EPSILON))
    measured = [(lens, psi[lens]) for lens in LENS_ORDER if psi[lens] is not None]
    binding = min(measured, key=lambda pair: (pair[1], LENS_ORDER.index(pair[0])))[0] if measured else None

    # §4.6: the derived blocks and the derivation itself are reported, so a
    # reader can recompute `(c_g, C_g)` from the raw requirements.
    blocks: List[Tuple[float, float]] = []
    derivation: Optional[Dict[str, object]] = None
    if obs.requirements is not None:
        blocks = derive_blocks(obs.requirements, means or {}, groups, weights, cap)
        derivation = {
            "procedure": DERIVE_BLOCKS_PROCEDURE,
            "requirements": dict(obs.requirements),
            "means": dict(means or {}),
            "groups": canonical_groups(groups, obs.requirements, means),
            # §4.6 (v0.7): the numeraire weights and the mandate cap are part of
            # the derivation, so a reader can recompute `(c_g, C_g)` and see that
            # the sum is not adding different physical units together.
            "weights": {str(k): float(v) for k, v in (weights or {}).items()},
            "cap": cap,
        }

    return EntityMeasurement(
        entity_id=entity_id,
        psi=psi,
        terms=terms,
        current_dof=_clamp01(product),
        dof_known=known_all,
        contribution=contribution,
        terms_sum=terms_sum,
        floored=product < EPSILON,
        binding_lens=binding,
        blocks=blocks,
        derivation=derivation,
        variety_counters=dict(obs.variety) if obs.variety else None,
    )


class MeasurementDeclaration(BaseModel):
    """The frozen ruler (§3.4): identity, raw lens inputs, and the freeze.

    The declaration is an *input*, frozen on `S`, and its text is echoed in the
    audit report so that a reader can reproduce the numbers.
    """

    psi_id: str = "perception-v1"
    lens_order: List[str] = list(LENS_ORDER)
    procedures: Dict[str, str] = {}                       # lens -> named procedure
    u0_prior_q: Optional[float] = None                    # declared prior quantile
    entities: Dict[str, Dict[str, object]] = {}           # entity_id -> raw lens inputs
    freeze: Dict[str, object] = {}                        # τ, budgets, rates, blocks
    # §3.4.1 hashed content (v0.6) — the ruler now includes the resource layer:
    # unit names and scales, the derived groups, the observed rates and the
    # declared mandate. Two implementations that declare the same resource name
    # with different scales are measurably different rulers and produce
    # different digests (§4.8).
    resources: List[Dict[str, object]] = []               # [{"id","unit","scale"}] sorted
    groups: List[List[str]] = []                          # derived exchange groups
    rates: Dict[str, Dict[str, float]] = {}               # "from->to" -> {rate, duration_mks}
    mandate: Dict[str, object] = {}                       # declared mandate + limits
    # §3.4.1 hashed content (v0.7) — the graph-derived values of §4.9 and the
    # numeraire the group amounts are expressed in. Only what determines numbers
    # is here: the graph itself, the witness paths and the observation digest are
    # report context (§6.2), and an option's closure list is a per-option input
    # like `projected_dof_delta`, not ruler content.
    numeraire: Optional[str] = None                       # §4.6: the declared unit of account
    weights: Dict[str, float] = {}                        # resource -> observed rate to the numeraire
    mandate_cap: Optional[float] = None                   # §4.8: the mandate ceiling, in the numeraire
    verdicts: Dict[str, Dict[str, object]] = {}           # entity -> {verdict, t_rec_mks, v}
    means_class: List[str] = []                           # §4.9: identifiers of M(S), canonical order
    graph_procedure: str = ""                             # §4.9: identity and version of the verdict procedure
    # §3.4.1/§3.4.2 (v0.11): the measurement durations `t_m`, `t_v` per lens.
    # **Ruler-level**, not hypothesis-level: §4.7 defines them as *declared
    # durations of the measurement procedure* — sampling and verification — and
    # says the core never infers them. A hypothesis may reinterpret what was
    # **measured**; it may not reinterpret how long the **measuring** takes,
    # because then `T_meas` and hence `t*` would differ between readings that
    # claim to use one ruler, and `min_h NetDelta(o | h)` would compare numbers
    # produced by different measuring systems.
    measurement_durations: Dict[str, Dict[str, float]] = {}

    def u0(self) -> float:
        return u0_from_prior(self.u0_prior_q)

    # --- canonical serialization (§3.4.3) ------------------------------------
    @staticmethod
    def _canonicalize(obj):
        """Canonical form: keys sorted, no insignificant whitespace, floats as
        fixed 6-decimal strings (no exponent), integers as integers, UTF-8."""
        if isinstance(obj, dict):
            return {k: MeasurementDeclaration._canonicalize(obj[k]) for k in sorted(obj)}
        if isinstance(obj, (list, tuple)):
            return [MeasurementDeclaration._canonicalize(v) for v in obj]
        if isinstance(obj, bool) or obj is None:
            return obj
        if isinstance(obj, int):
            return obj
        if isinstance(obj, float):
            return "%.6f" % obj
        return str(obj)

    def _dump(self) -> dict:
        """The canonical dump, with §3.4.3's undeclared-procedure rule applied.

        `§4.7` treats "no duration declared" as **one** condition — the
        procedure is undeclared and the window uncomputable — so an absent
        `measurement_durations` and an empty one MUST hash alike, and the key is
        omitted when empty. That is also what keeps the field **additive**: a
        state that declares no duration hashes exactly as it did before the
        field existed, so the `v0.7`/`v0.8`/`v0.9.1` fingerprints are untouched
        while a state that *does* declare durations hashes them (§3.4.1).
        """
        dump = self.model_dump()
        if not dump.get("measurement_durations"):
            dump.pop("measurement_durations", None)
        return dump

    def canonical_text(self) -> str:
        return json.dumps(self._canonicalize(self._dump()), separators=(",", ":"), ensure_ascii=False)

    def ruler_canonical_text(self) -> str:
        """§3.4.2/§3.4.3: the **ruler-level** content of the same declaration.

        Everything that is identical for every hypothesis of a cycle and for
        every option: the procedure and its version, the lens set, the units and
        scales, the means class `M(S)`, `T_rec(X)`, the derived groups, the
        observed rates with their numeraire, the mandate, the `u₀` prior and the
        graph procedure identity.

        Three groups of fields are **excluded**, and each for a stated reason:

        * `entities` — the per-entity **lens counters**, which are precisely what
          a hypothesis varies (§3.6);
        * `freeze` — τ and the budgets, which are the hypothesis's own measured
          content;
        * `verdicts` — the §4.9 **verdict**, which consumes `DoF(X | h)` and is
          therefore computed per hypothesis (§4.9). Its horizon `T_rec(X)` is
          type-derived and shared, but it does not have to be *hashed* for the
          readings to be comparable, and the declaration carries no field for it:
          `v0.11` is additive, so the **full** `digest()` stays byte-identical to
          the `v0.7`/`v0.8` ruler digest.

        Two readings of one cycle therefore have **equal** `ruler_digest` and
        **different** `digest`, which is what makes the `min_h` of §4.10 a
        conformant output while a comparison of two different rulers is not.
        """
        dumped = self._dump()
        for field in ("entities", "freeze", "verdicts"):
            dumped.pop(field, None)
        return json.dumps(self._canonicalize(dumped), separators=(",", ":"), ensure_ascii=False)

    def digest(self) -> str:
        return hashlib.sha256(self.canonical_text().encode("utf-8")).hexdigest()

    def ruler_digest(self) -> str:
        return hashlib.sha256(self.ruler_canonical_text().encode("utf-8")).hexdigest()


def build_declaration(psi_id: str, lens_observations: Dict[str, LensObservation],
                      tau_mks: float, u0_prior_q: Optional[float] = None,
                      procedures: Optional[Dict[str, str]] = None,
                      freeze: Optional[Dict[str, object]] = None,
                      resources: Optional[Sequence[Dict[str, object]]] = None,
                      groups: Optional[Sequence[Sequence[str]]] = None,
                      rates: Optional[Dict[str, Dict[str, float]]] = None,
                      mandate: Optional[Dict[str, object]] = None,
                      numeraire: Optional[str] = None,
                      weights: Optional[Dict[str, float]] = None,
                      mandate_cap: Optional[float] = None,
                      verdicts: Optional[Dict[str, Dict[str, object]]] = None,
                      means_class: Optional[Sequence[str]] = None,
                      graph_procedure: str = "",
                      measurement_durations: Optional[Dict[str, Dict[str, float]]] = None
                      ) -> MeasurementDeclaration:
    """Assemble the frozen declaration for one state (§3.4.1).

    The resource layer is normalized before hashing: units sorted by resource
    id, groups canonicalized, so two implementations that declare the same layer
    in a different order produce the same digest.
    """
    units = sorted((dict(r) for r in (resources or [])), key=lambda r: str(r.get("id", "")))
    procs = dict(procedures) if procedures else {lens: f"{psi_id}:{lens}" for lens in LENS_ORDER}
    procs.setdefault("options_blocks", f"{psi_id}:{DERIVE_BLOCKS_PROCEDURE}")
    return MeasurementDeclaration(
        psi_id=psi_id,
        procedures=procs,
        u0_prior_q=u0_prior_q,
        entities={eid: obs.model_dump() for eid, obs in lens_observations.items()},
        freeze={"tau_mks": tau_mks, **(freeze or {})},
        resources=units,
        groups=canonical_groups(groups),
        rates={str(k): dict(v) for k, v in (rates or {}).items()},
        mandate=dict(mandate or {}),
        numeraire=numeraire,
        weights={str(k): float(v) for k, v in sorted((weights or {}).items())},
        mandate_cap=(None if mandate_cap is None else float(mandate_cap)),
        verdicts={str(e): dict(v) for e, v in sorted((verdicts or {}).items())},
        means_class=sorted(str(c) for c in (means_class or [])),
        graph_procedure=str(graph_procedure),
        # §3.4.1 (v0.11): hashed **ruler** content — a duration is a property of
        # the measurement procedure, not of the reading that uses it.
        measurement_durations={
            str(lens): {str(k): float(v) for k, v in sorted(durs.items())}
            for lens, durs in sorted((measurement_durations or {}).items())
        },
    )


def verify_graph_derived(declaration: MeasurementDeclaration, graph,
                         counting_horizon_mks: Optional[float]) -> List[str]:
    """§4.6/§4.9: derived numbers MUST equal what their procedure computes.

    Recomputes, over the supplied graph, the Variety counter and the reachability
    verdict of every entity against the declaration's `M(S)` and `T_rec`, and
    returns a list of mismatches (empty = the ruler is honest). A declaration
    that claims a counter its own observation does not support is exactly the
    "declared, not derived" defect this revision removes.
    """
    problems: List[str] = []
    cats = list(declaration.means_class)
    for entity_id, declared in sorted(declaration.verdicts.items()):
        v_declared = declared.get("v")
        if v_declared is not None:
            v_here = graph.v_count(entity_id, cats, counting_horizon_mks)
            if int(v_declared) != int(v_here):
                problems.append(
                    f"{entity_id}: declared V={v_declared} but the counting procedure gives {v_here}")
        t_rec = declared.get("t_rec_mks")
        verdict_here = graph.verdict(entity_id, cats, t_rec).verdict
        if str(declared.get("verdict")) != verdict_here:
            problems.append(
                f"{entity_id}: declared verdict {declared.get('verdict')!r} "
                f"but the verdict procedure returns {verdict_here!r}")
    return problems
