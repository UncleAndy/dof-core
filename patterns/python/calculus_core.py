"""DOF-Core calculus kernel (Python port).

Mirrors the normative DOF-SPEC: pure Nash evaluation index (sum of ln(DoF)),
the `calc` calculation set, Collapse-Source isolation, Delta-T-aware selection,
the collapse charge (§4.2) with the ordered admissibility filter of §4.5 (v0.8:
`D1 → D2 → D3 → NetDelta → reversibility`, with staying put a candidate), the
resource gate with verified conversion and insolvency (§4.8), and the
Proof-of-Implementation audit report (DOF-SPEC §6).

Structural expression of the skill's axioms: Axiom 1 (maximize the total future
DoF of the system AND its constituent entities); Axiom 3 (never trade one
entity's collapse for another's gain — enforced structurally by the collapse
charge and the admissibility filter, because the ε-floor is finite); Axiom 5
(prefer reversible actions; never assume unknown possibilities have zero DoF —
a node with dof_known=False is never excluded as a hopeless zero).
"""

import math
from typing import Any, List, Dict, Optional, Sequence, Set, Tuple
from pydantic import BaseModel, Field

from measurement import EntityMeasurement, MeasurementDeclaration, psi_var
from world_graph import ClosedRef, WorldGraph

# §4.5 / §10 (v0.8): the tolerance used when grouping candidates whose `NetDelta`
# ties. The index is a sum of logarithms over a *set*, so two ports that iterate
# their container in different orders can disagree in the last bits (~1e-15)
# while agreeing on every derivation. A tie must be resolved identically
# everywhere: §7 requires the same *choice*, not only the same numbers.
NET_DELTA_TOLERANCE = 1e-9

# §3.6/§4.10.6: the identifier of the observed-state singleton — the reading a
# state without a declared hypothesis set is evaluated under. It is a reserved
# identifier: a declared set MUST NOT use it for one of its own members.
OBSERVED_HYPOTHESIS_ID = "$observed$"


def tau_of(state: 'SystemStateMatrix') -> Optional[float]:
    """§3.2b: τ as the calculus reads it — from the **resource map**, signed.

    `state.tau` is the `tau` `ResourceObservation`. When it is absent or its
    `value` is `null`, τ is **unmeasured** (`null`), never the minimum over the
    measured deadlines alone and never `0.0`: an unmeasured active deadline may
    be the most urgent one, so acting on the budget the measured ones support is
    acting on a budget the state does not establish, and writing `0.0` invents a
    catastrophe (§3.1, §3.2b).

    A **negative** value is a deadline that has passed, `|τ|` ago. It is a
    *known* state and MUST NOT be clamped to `0.0` or replaced by `null`, which
    means unmeasured only (§3.2b, §4.8b).
    """
    tau_obs = getattr(state, "tau", None)
    if tau_obs is not None:
        value = getattr(tau_obs, "value", None)
        return None if value is None else float(value)
    # No `tau` observation at all: fall back to the declared individual
    # deadlines, and only then to the deprecated mirror — which a port that
    # predates the resource layer still writes. The mirror is read **only** when
    # the state carries no resource-map τ and no deadline set, so it can never
    # override a measurement (§3.2b).
    deadlines = getattr(state, "deadlines", None) or {}
    if deadlines:
        if any(v is None for v in deadlines.values()):
            return None
        return min(float(v) for v in deadlines.values())
    mirror = getattr(state, "global_time_to_collapse_mks", None)
    return None if mirror is None else float(mirror)


def mirror_time_to_collapse(tau: Optional[float]) -> float:
    """§3.1/§3.2b: the deprecated mirror — clamped, non-authoritative.

    Equal to τ when τ is known and non-negative, `0.0` when τ is negative or
    `null`. It is **forbidden as an input to any rule** of §4.7, §4.8b or §5,
    and the `1e15` µs synthetic default is withdrawn: a fabricated deadline is
    indistinguishable from a measured one.
    """
    if tau is None or tau < 0.0:
        return 0.0
    return float(tau)


class PsiReference(BaseModel):
    """§3.4: the frozen measurement declaration reference stored in the state."""
    id: str
    digest: str


class EntityState(BaseModel):
    entity_id: str
    is_autonomous: bool = True
    agency_index: float = Field(..., ge=0.0, le=1.0)  # Measure of controllability
    current_dof: float = Field(..., ge=0.0, le=1.0)  # Degree of freedom of the node
    is_collapse_source: bool = False                 # Virus/aggressor flag
    dof_known: bool = True                           # Whether current_dof is a known value (Axiom 5)
    time_to_collapse_mks: float                     # Local node timer (microseconds)
    # Port-level extension (not a §3.1 field): the measurement that produced
    # `current_dof`, kept so the audit can show the per-lens terms (§6.1).
    measurement: Optional[EntityMeasurement] = None


class SystemStateMatrix(BaseModel):
    global_time_to_collapse_mks: float               # Deprecated mirror (§3.1, §3.2b)
    context_switch_cost: float                       # Penalty for changing current process (ΔT)
    entities: Dict[str, EntityState]
    psi: Optional[PsiReference] = None               # Frozen measurement ruler (§3.4)
    # §3.2 (v0.9): the acting agent's available means per resource, as
    # ResourceObservation objects carrying metadata.
    resources: Dict[str, 'ResourceObservation'] = {}
    # §3.2b (v0.9.1): τ as ResourceObservation.
    tau: Optional['ResourceObservation'] = None
    # §3.2b (v0.11): the active individual deadlines τ is derived from. τ is
    # `null` when **any** active deadline is unmeasured — never the minimum over
    # the measured ones alone, and never `0.0` (§3.2b, §10(I)).
    deadlines: Dict[str, Optional[float]] = {}
    # §4.7 (v0.11): the declared measurement durations `t_m`, `t_v` per lens.
    # Hashed ruler content: two states differing only in `t_m` produce different
    # `ruler_digest` (§3.4.1, §10(an)).
    measurement_durations: Dict[str, Dict[str, float]] = {}
    # §4.7 (v0.11): the declared **schedule** `t` per lens — when the measurement
    # is planned to happen. An undeclared schedule reads as `t = 0` (`u₀`); a
    # declared `t > t*` is non-conformant input, never clamped.
    measurement_schedule: Dict[str, float] = {}


class ActionOption(BaseModel):
    option_id: str
    description: str
    # §3.3 (v0.11): **two forms, never mixed within one option.**
    #   flat           — {entity_id: delta}, applied under every hypothesis
    #   per_hypothesis — {hypothesis_id: {entity_id: delta}}, the entry for `h`
    #                    used under `h`, entities unlisted for `h` taking 0.0
    # An option in the flat form asserts that its effect does not depend on the
    # causal reading; the per-hypothesis form is meaningful only when a
    # hypothesis set is declared.
    projected_dof_delta: Any = {}
    # §4.4 (v0.11): the closure list has the same two forms.
    closed: Any = []
    is_reversible: bool = True
    estimated_duration_mks: float = Field(0.0, ge=0.0)
    projected_resource_delta: Dict[str, Dict[str, float]] = {}
    act_id: Optional[str] = None
    # §3.3 (v0.9): resources this option needs for gate checks, and resources
    # whose value becomes known after execution (measure-type act).
    requires: List[str] = []
    discovers: List[str] = []
    # §3.3 (v0.11): the projected τ change. `null` means **not computable** —
    # which happens exactly when τ is unknown — and is admissible only for an act
    # that resolves τ. Derived, never independently declared, for such an act:
    # it MUST equal `projected_tau_value - (τ - estimated_duration_mks)`.
    projected_tau_delta: Optional[float] = None
    # §3.3 (v0.11): the value the option expects `tau` to hold AFTER it executes.
    # Present iff `discovers` names "tau". MAY be negative (§3.2b).
    projected_tau_value: Optional[float] = None

    # ---------------------------------------------------------------- forms
    def projection_form(self) -> str:
        """`"flat"` | `"per_hypothesis"` | `"invalid"` (§3.3, §10(B))."""
        flat = self._is_flat_entity_map(self.projected_dof_delta)
        nested = self._is_nested_entity_map(self.projected_dof_delta)
        if flat and not nested:
            return "flat"
        if nested and not flat:
            return "per_hypothesis"
        return "invalid"

    def closure_form(self) -> str:
        """`"flat"` | `"per_hypothesis"` | `"invalid"` (§4.4, §10(B))."""
        if isinstance(self.closed, list):
            return "flat"
        if isinstance(self.closed, dict):
            if not self.closed:
                return "flat"
            if all(isinstance(v, list) for v in self.closed.values()):
                return "per_hypothesis"
        return "invalid"

    @staticmethod
    def _is_flat_entity_map(value: Any) -> bool:
        if not isinstance(value, dict):
            return False
        return all(not isinstance(v, dict) for v in value.values())

    @staticmethod
    def _is_nested_entity_map(value: Any) -> bool:
        if not isinstance(value, dict) or not value:
            return False
        return all(isinstance(v, dict) for v in value.values())

    def delta_for(self, hypothesis_id: str, entity_id: str) -> float:
        """The declared delta this option contributes under `hypothesis_id`."""
        if self.projection_form() == "per_hypothesis":
            per_h = self.projected_dof_delta.get(hypothesis_id) or {}
            return float(per_h.get(entity_id, 0.0))
        return float(self.projected_dof_delta.get(entity_id, 0.0))

    def closed_for(self, hypothesis_id: str) -> List[ClosedRef]:
        """The closures this option declares under `hypothesis_id`."""
        if self.closure_form() == "per_hypothesis":
            raw = self.closed.get(hypothesis_id) or []
        else:
            raw = self.closed or []
        out: List[ClosedRef] = []
        for item in raw:
            out.append(item if isinstance(item, ClosedRef) else ClosedRef(**item))
        return out

    def flat_delta(self) -> Dict[str, float]:
        """The flat delta map, whichever form the option uses.

        Used where a single map is needed for `None`-safety (the baseline), and
        by older callers; a per-hypothesis option returns the **union** of its
        entries, which is only meaningful for existence questions.
        """
        if self.projection_form() == "per_hypothesis":
            merged: Dict[str, float] = {}
            for per_h in self.projected_dof_delta.values():
                for e_id, delta in (per_h or {}).items():
                    merged[e_id] = max(merged.get(e_id, 0.0), float(delta))
            return merged
        return {k: float(v) for k, v in self.projected_dof_delta.items()}

    def forms_consistent(self) -> Optional[str]:
        """§3.3/§4.4: the two forms MUST NOT be mixed within one option."""
        if self.projection_form() == "invalid":
            return (f"{self.option_id}: `projected_dof_delta` mixes the flat and "
                    f"per-hypothesis forms (§3.3)")
        if self.closure_form() == "invalid":
            return f"{self.option_id}: `closed` mixes the flat and per-hypothesis forms (§4.4)"
        return None


# §3.2a (v0.9): a resource as an observable quantity with metadata.
class ResourceObservation(BaseModel):
    value: Optional[float] = None
    unit: str = ""
    scale: float = 1.0
    source: str = ""
    last_measured_at: float = 0.0
    aging_time: float = 0.0
    estimated: Optional[float] = None
    estimation_source: List[str] = []

    def resource_value(self, use_estimated: bool = False) -> float:
        if self.value is not None:
            return self.value
        if use_estimated and self.estimated is not None:
            return self.estimated
        return 0.0

    def is_stale(self, now: float = 0.0) -> bool:
        if self.aging_time <= 0.0:
            return False
        return (now - self.last_measured_at) > self.aging_time


class DofReport(BaseModel):
    """Proof-of-Implementation audit (DOF-SPEC §6). Serializable to JSON."""
    entities: List[Dict[str, object]]
    total_system_dof: float
    context_switch_cost: float
    global_time_to_collapse_mks: float
    mode: str
    options: List[Dict[str, object]]
    # §6.2: the ruler that produced the numbers, the removals that happened
    # before evaluation, and whether a resolvable unknown was left unmeasured.
    # A removal is a decision and must be visible.
    psi_id: Optional[str] = None
    psi_digest: Optional[str] = None
    declaration: Optional[str] = None
    removed_options: List[Dict[str, str]] = []
    incomplete: bool = False
    # §6.2 (v0.6): the acting agent's means at the start of the cycle and after
    # the selected option's consumption. Multi-step accumulation is auditable
    # only if the spend is written where the next cycle can see it (§4.8).
    resources_before: Dict[str, object] = {}         # ResourceObservation per resource
    resources_after: Dict[str, object] = {}
    # §6.2 (v0.7): where each amount of the agent's means came from — a measured
    # balance or an asserted authority — and the identity of the observation a
    # reported subgraph was taken from.
    means_provenance: Dict[str, object] = {}
    observation_digest: Optional[str] = None
    # §6.2 (v0.8): the vector every candidate was compared against, and whether
    # any candidate beat it. A refusal to act is a decision and must be audible.
    baseline: Dict[str, object] = {}
    no_candidate_better: bool = False
    # §6.2 (v0.9): resources left unmeasured and time spent on measurements.
    unknown_resources: List[Dict[str, object]] = []
    measurement_time_spent: float = 0.0
    # --- §6.2/§6.3 (v0.11) ---------------------------------------------------
    # The report is **per hypothesis**: `lens_terms` and `binding_lens` belong to
    # a reading, so printing one shared `ψ` per entity is forbidden — there is no
    # shared value to print (§6.1). `total_system_dof` becomes a map.
    psi_ruler_digest: Optional[str] = None
    hypotheses: List[Dict[str, object]] = []          # id, plausible, basis, coverage
    coverage: str = "partial"
    hypothesis_horizon_mks: Optional[float] = None
    total_system_dof_by_hypothesis: Dict[str, float] = {}
    hypothesis_conflict: bool = False
    # §6.3: the conditional vector of every candidate under every reading, and
    # per reading whether it was admissible and on which condition it failed.
    conditional_vectors: Dict[str, Dict[str, Dict[str, object]]] = {}
    admissible_under: Dict[str, Dict[str, bool]] = {}
    net_delta_robust: Dict[str, float] = {}
    robust_candidates: List[str] = []
    # §6.2: the single reactive-circuit mode, taken from the **observed** state,
    # and the readings whose τ would have selected a different one.
    mode_dissenters: List[Dict[str, object]] = []


class ObservationContext(BaseModel):
    """The observation a cycle is decided over (DOF-SPEC §3.5, §4.9).

    Deliberately NOT a state field: the world graph is a Perception artifact
    supplied to the cycle, exactly as the derived groups and the observed rates
    are (§4.8). Without it every verdict is `undetermined`, which means no entity
    at a known zero is excluded and no collapse-source label is honoured — the
    fail-safe direction: nothing is proven, so nothing is removed.
    """

    world: WorldGraph
    means_class: List[str] = []                       # M(S): admissible-means identifiers
    t_rec: Dict[str, float] = {}                      # entity -> recovery horizon, µs
    counting_horizon_mks: Optional[float] = None      # horizon of the V counting procedure
    observation_digest: str = ""                      # §6.2: pins the reported subgraph
    # §4.9 (v0.11): `DoF(X | h)` for the reading the verdict is being computed
    # under. The verdict is a value of `(G, state_h)`, not of `G` alone, and
    # `DoF` is the one clause of it that is conditional, so the reading's DoF is
    # supplied here rather than left to a module-global.
    dof_override: Dict[str, float] = {}

    def horizon(self, entity_id: str) -> Optional[float]:
        return self.t_rec.get(entity_id)

    def verdict(self, entity_id: str) -> str:
        return self.world.verdict(entity_id, self.means_class,
                                  self.horizon(entity_id),
                                  self.dof_override.get(entity_id)).verdict

    def with_dof(self, dofs: Dict[str, float]) -> 'ObservationContext':
        """The same observation, read against another state's measured DoF."""
        merged = dict(self.dof_override)
        merged.update({k: float(v) for k, v in dofs.items()})
        return self.model_copy(update={"dof_override": merged})

    def v_before(self, entity_id: str) -> int:
        return self.world.v_count(entity_id, self.means_class, self.counting_horizon_mks)

    def v_after_closure(self, entity_id: str, closed: Sequence[ClosedRef]) -> int:
        if not closed:
            return self.v_before(entity_id)
        return self.world.with_closed(closed).v_count(entity_id, self.means_class,
                                                      self.counting_horizon_mks)


class DOFCalculusCore:
    def __init__(self, epsilon: float = 1e-6):
        self.epsilon = epsilon  # Protection against ln(0) — a numerics device (§4.1)

    def _is_included(self, entity: EntityState, ctx: Optional[ObservationContext] = None,
                     state: Optional[SystemStateMatrix] = None) -> bool:
        """Whether an entity belongs to the calculation set `calc` (DOF-SPEC §4.2).

        Excluded if it is a **witnessed** collapse source, or if its DoF is a
        known zero whose recoverability verdict is `proven_unreachable`. A node
        with an unknown DoF is never excluded (Axiom 5), and neither is a node
        whose verdict is `reachable` or `undetermined` — incompleteness of an
        observation is never read as proof (§4.9).

        The witness of unreachability MUST NOT be the Generator's candidate set
        (§4.2), and a verdict is computed from the observation, never asserted.
        With no observation at all nothing is proven, so nothing is excluded.
        """
        if entity.is_collapse_source and self._label_witnessed(entity, ctx, state):
            return False                      # aggressors leave the topology
        return self._is_included_without_label(entity, ctx)

    def _is_included_without_label(self, entity: EntityState,
                                   ctx: Optional[ObservationContext]) -> bool:
        """`calc` membership with the collapse-source label *not* honoured (§4.2).

        Used in two places, and it must be the same rule in both: deciding who is
        counted, and deciding whether a label has a witness. The witness question
        is "would this entity be counted if its own label were ignored" — asking
        it with the label already applied would be circular, and would make every
        label unfalsifiable.
        """
        if entity.current_dof > 0.0:
            return True
        if not entity.dof_known:
            return True
        if ctx is None:
            return True                       # fail-safe: no observation, no proof
        return ctx.verdict(entity.entity_id) != "proven_unreachable"

    def _coerce_dof(self, value: float) -> float:
        return max(0.0, min(1.0, value))

    def _label_witnessed(self, entity: EntityState, ctx: Optional[ObservationContext],
                         state: Optional[SystemStateMatrix]) -> bool:
        """§4.2/§4.9: a label is honoured only with a machine-verifiable act.

        The act must be performed by this entity and must drive an entity that
        would otherwise be counted to a known zero. A flag without such an act is
        not a verdict — otherwise the label itself would raise the index.
        """
        if ctx is None or state is None:
            return False
        # The pool is "who would be counted if this label (and every label) were
        # ignored". Applying the label first would make the question circular:
        # a labelled entity would fall out of its own witness set, and no label
        # could ever be confirmed — or refuted.
        counted = {e.entity_id for e in state.entities.values()
                   if self._is_included_without_label(e, ctx)}
        if entity.entity_id not in counted:
            return False
        dof_before = {e.entity_id: e.current_dof for e in state.entities.values()}
        acts = set(ctx.world.collapse_acts(counted, dof_before))
        return any(a.source == entity.entity_id and a.id in acts
                   for a in ctx.world.acts)

    def calc_members(self, state: SystemStateMatrix,
                     ctx: Optional[ObservationContext] = None) -> Set[str]:
        """§4.2: the calculation set `calc(S)`, frozen for the whole cycle.

        Computed once, on `S`, and reused for every simulated state: the same
        entities are summed in `S` and in `S'`, so a term cannot appear or
        disappear between the two sides of `NetDelta`.
        """
        return {e.entity_id for e in state.entities.values()
                if self._is_included(e, ctx, state)}

    def _projected_dof(self, e_state: EntityState, option: ActionOption,
                       ctx: Optional[ObservationContext],
                       h_id: str = OBSERVED_HYPOTHESIS_ID) -> float:
        """The DoF this option would leave the entity with, closure included (§4.3).

        One definition, used by both `simulate` and `collapse_charges`. If the
        charge were computed from the raw delta while the index was computed from
        the closure-aware value, an option that destroys an entity *by closing its
        transitions* would be scored as a collapse and charged as nothing — the
        structural gate of §4.5 would then pass exactly the option it exists to
        stop. Two call sites, one rule.

        Under a declared hypothesis set the projection and the closure list are
        both **conditional**: `h_id` selects the reading's own delta (§3.3) and
        its own closures (§4.4).
        """
        new_dof = self._coerce_dof(
            e_state.current_dof + option.delta_for(h_id, e_state.entity_id))
        closed = option.closed_for(h_id)
        if ctx is not None and closed:
            recomputed = self._dof_after_closure(e_state, option, ctx, h_id)
            if recomputed is not None:
                new_dof = recomputed
        return new_dof

    def simulate(self, current_state: SystemStateMatrix, option: ActionOption,
                 ctx: Optional[ObservationContext] = None,
                 h_id: str = OBSERVED_HYPOTHESIS_ID
                 ) -> Tuple[SystemStateMatrix, Set[str]]:
        """Apply an option's projected deltas to produce a simulated state.

        Returns the simulated state plus the **frozen** member set of `calc(S)`:
        everything counted in `S` stays counted in `S'` (§4.2), so destroying a
        counted entity cannot raise the index by removing a negative term, while
        an entity outside `calc(S)` stays outside it — acting on something that
        is not a subject of the decision is neither rewarded nor punished.

        The agent's means travel with the state unchanged: `simulate` scores the
        DoF consequences of an option, and the resource side is decided by the
        gate of §4.8 (a DoF projection must not silently also pay for itself).

        §4.4 (v0.7): when the option closes transitions and an observation is
        supplied, the affected entities' Variety counter falls in `S'` and their
        `DoF` is recomputed from the changed counter — so the price of a closure
        sits *inside* the DoF difference, where freedom is measured, instead of
        being a separate entry that would charge the same loss twice.
        """
        self.validate_closure(option, h_id)
        members = self.calc_members(current_state, ctx)
        simulated_entities: Dict[str, EntityState] = {}
        for e_id, e_state in current_state.entities.items():
            new_dof = self._projected_dof(e_state, option, ctx, h_id)
            simulated_entities[e_id] = EntityState(
                entity_id=e_id,
                is_autonomous=e_state.is_autonomous,
                agency_index=e_state.agency_index,
                current_dof=new_dof,
                is_collapse_source=e_state.is_collapse_source,
                dof_known=e_state.dof_known,
                time_to_collapse_mks=e_state.time_to_collapse_mks,
            )
        simulated = SystemStateMatrix(
            global_time_to_collapse_mks=current_state.global_time_to_collapse_mks,
            context_switch_cost=current_state.context_switch_cost,
            entities=simulated_entities,
            psi=current_state.psi,
            resources=current_state.resources,
        )
        return simulated, members

    def validate_closure(self, option: ActionOption,
                         h_id: str = OBSERVED_HYPOTHESIS_ID) -> None:
        """§4.4 guards. Both violations are non-conformant, so the cycle refuses.

        (1) An option MUST NOT list its own execution path among the transitions
        it closes — that would be a contradiction, not a price. (2) `closed` MUST
        be non-empty whenever `is_reversible` reads false; `is_reversible` is
        derived from the list, so an empty list with a false label is a lie that
        would also be an escape from the price.

        Under a declared hypothesis set the closures are read **per reading**
        (§4.4, §10(B)), so both guards are applied to that reading's own list.
        """
        closed = option.closed_for(h_id)
        if closed and option.act_id and any(
                c.kind == "act" and c.id == option.act_id for c in closed):
            raise ValueError(
                f"{option.option_id}: closes its own execution path (§4.4 guard 1)")
        if not closed and option.is_reversible is False:
            raise ValueError(
                f"{option.option_id}: is_reversible=false with an empty closure list "
                f"(§4.4 guard 2)")

    def is_reversible(self, option: ActionOption,
                      h_id: str = OBSERVED_HYPOTHESIS_ID) -> bool:
        """§4.4: the reported flag is DERIVED — true exactly when nothing is closed."""
        return not option.closed_for(h_id)

    def _dof_after_closure(self, entity: EntityState, option: ActionOption,
                           ctx: ObservationContext,
                           h_id: str = OBSERVED_HYPOTHESIS_ID) -> Optional[float]:
        """`DoF` recomputed from the counters after the option's closure (§4.3, §4.4).

        Only the Variety share moves, so the whole product moves by its ratio: the
        other lenses (and any `u(t)` factors) are untouched by a closure. Returns
        `None` when the entity is not affected or its Variety lens was unmeasured.
        """
        m = entity.measurement
        var_before = m.psi.get("variety") if m else None
        if m is None or var_before is None or not m.variety_counters:
            return None
        v_env = float(m.variety_counters.get("V_env", 0.0))
        v_before = ctx.v_before(entity.entity_id)
        v_after = ctx.v_after_closure(entity.entity_id, option.closed_for(h_id))
        if v_after == v_before:
            return None                      # this entity is not affected
        return self._coerce_dof(m.current_dof / var_before * psi_var(v_after, v_env))

    def closure_share(self, state: SystemStateMatrix, option: ActionOption,
                      ctx: Optional[ObservationContext],
                      h_id: str = OBSERVED_HYPOTHESIS_ID) -> Dict[str, float]:
        """§6.3: the per-entity decomposition of a closure's price.

        This is a *decomposition* of the loss that is already inside `NetDelta`
        (§4.3/§4.4), never an extra charge: it exists so a reader can see which
        entity lost which share, and by how much.
        """
        out: Dict[str, float] = {}
        closed = option.closed_for(h_id)
        if ctx is None or not closed:
            return out
        for e_id, ent in sorted(state.entities.items()):
            m = ent.measurement
            var_before = m.psi.get("variety") if m else None
            if m is None or var_before is None or not m.variety_counters:
                continue
            v_env = float(m.variety_counters.get("V_env", 0.0))
            v_after = ctx.v_after_closure(e_id, closed)
            v_before = ctx.v_before(e_id)
            if v_after == v_before:
                continue
            out[e_id] = round(
                math.log(max(psi_var(v_after, v_env), self.epsilon))
                - math.log(max(psi_var(v_before, v_env), self.epsilon)), 6)
        return out

    def recoverability_row(self, entity_id: str,
                           ctx: Optional[ObservationContext]) -> Dict[str, object]:
        """§6.1: the verdict, its witness, and the completeness claim behind it.

        A `proven_unreachable` verdict without a witness is not a verdict, so the
        report carries both — and names the observation, because "no path" is only
        meaningful together with "and the observation was complete for this entity".
        """
        if ctx is None:
            return {"verdict": "undetermined", "witness": [], "horizon_mks": None,
                    "observation": "unobserved", "admissible_seen": 0,
                    "reason": "no observation was supplied for this cycle"}
        node = ctx.world.entities.get(entity_id)
        v = ctx.world.verdict(entity_id, ctx.means_class, ctx.horizon(entity_id),
                              ctx.dof_override.get(entity_id))
        return {"verdict": v.verdict, "witness": list(v.witness),
                "horizon_mks": ctx.horizon(entity_id),
                "observation": (node.observation if node else "unobserved"),
                "admissible_seen": v.admissible_seen, "reason": v.reason}

    def collapse_charges(self, current_state: SystemStateMatrix,
                         option: ActionOption,
                         ctx: Optional[ObservationContext] = None,
                         h_id: str = OBSERVED_HYPOTHESIS_ID) -> List[Dict[str, object]]:
        """§4.2: counted entities that this option drives to a known zero.

        The charge depends on neither the Generator's candidate set nor the
        victim's post-collapse prospects — only on what was counted in `S` and
        what the option did to it.
        """
        charges: List[Dict[str, object]] = []
        for e_id in sorted(self.calc_members(current_state, ctx)):
            e_state = current_state.entities[e_id]
            if not e_state.dof_known:
                continue  # unknown DoF is never a collapse (§4.2)
            # The projected value is the closure-aware one (§4.3): an option can
            # destroy a counted entity by closing its transitions while declaring
            # no delta at all, and that is exactly the case §4.5 must catch.
            new_dof = self._projected_dof(e_state, option, ctx, h_id)
            # §4.2: a charge requires a *transition* into the zero, not a stay at
            # it. An entity already at a known zero was not destroyed by this
            # option — charging it would make every option destructive in any
            # state that contains a recoverable zero (an entity kept in `calc`
            # by an `undetermined` verdict, for instance).
            if new_dof == 0.0 and e_state.current_dof > 0.0:
                charges.append({"entity_id": e_id, "dof_before": e_state.current_dof})
        return charges

    def apply_structural_gate(self, current_state: SystemStateMatrix,
                              options: List[ActionOption],
                              ctx: Optional[ObservationContext] = None
                              ) -> Tuple[List[ActionOption], List[Dict[str, str]]]:
        """§4.5 **v0.7 rule, retired in v0.8** — kept for the historical harnesses.

        `v0.8` no longer removes a charged candidate from the set: the candidate
        is evaluated, reported in full, and loses to staying put on the first key
        of the ordered filter (§4.5, `select_candidate`), so `removed_options`
        carries no structural removal. This function survives because the `v0.6`
        reference harness asserts the rule that was in force then and history
        must stay reproducible; **nothing on the live path calls it**.
        Original contract: an option that destroys a counted entity is
        inadmissible while a charge-free candidate exists. Every removal is
        recorded (§6.2).

        The charge is taken against `calc(S)`, and `calc` depends on the
        observation (§4.2/§4.9): an entity kept in the set by a `reachable` or
        `undetermined` verdict is a legitimate charge, an entity excluded as
        `proven_unreachable` is not. So the observation must reach the gate —
        without it `calc` is the fail-safe superset and the gate would compare
        against a different set than the one the index was scored on.
        """
        if not options:
            return [], []
        charged = [(o, self.collapse_charges(current_state, o, ctx)) for o in options]
        if any(not charges for _, charges in charged):
            admissible = [o for o, charges in charged if not charges]
            removed = [{"option_id": o.option_id, "gate": "collapse"}
                       for o, charges in charged if charges]
            return admissible, removed
        # No alternative exists: Axiom 3 still forbids preferring destruction,
        # but with every candidate destructive the ladder decides (rung 1).
        return [o for o, _ in charged], []

    # --- §4.5 (v0.8): the candidate vector and the ordered filter -------------
    def critical_members(self, state: SystemStateMatrix,
                         ctx: Optional[ObservationContext] = None,
                         members: Optional[Set[str]] = None) -> Set[str]:
        """§4.5: the entities of `calc(S)` at the minimum `current_dof`.

        A set, not a node: a minimum attained by several known zeros has no
        unique "critical node", and a flag would have to invent a tie-break by
        `entity_id`. `D3` is the *count* of lost paths inside this set.
        """
        members = self.calc_members(state, ctx) if members is None else members
        dofs = {e: state.entities[e].current_dof for e in members if e in state.entities}
        if not dofs:
            return set()
        lowest = min(dofs.values())
        return {e for e, value in dofs.items() if value == lowest}

    def lost_paths(self, state: SystemStateMatrix, option: ActionOption,
                   ctx: Optional[ObservationContext] = None,
                   h_id: str = OBSERVED_HYPOTHESIS_ID) -> List[Dict[str, object]]:
        """§4.5: the entities this option drops out of a `reachable` verdict.

        The verdict procedure runs twice over the *same* observation — once as
        observed, once with the option's closure applied — so a verdict can only
        move away from `reachable` and the difference is computed, not declared.
        A lost witness is a loss: an entity that leaves `reachable` counts even
        where no exclusion follows from it, because §4.2 excludes only on a
        `proven_unreachable` verdict over a complete observation.

        The verdict consumes `DoF(X | h)` (§4.9), so the second verdict is run
        **on the after-state of the same reading** — closures applied and the
        option's own declared delta for that reading *plus* the closure factored
        in as §4.3 prescribes. An after-state computed without the delta would let
        an option buy back the recoverability it destroys (§10(J), fixture `ae`).
        """
        closed = option.closed_for(h_id)
        if ctx is None or not closed:
            return []
        if state is None:                    # pragma: no cover - defensive
            return []
        closed_world = ctx.world.with_closed(closed)
        critical = self.critical_members(state, ctx)
        rows: List[Dict[str, object]] = []
        for e_id in sorted(state.entities):
            ent = state.entities[e_id]
            before = ctx.world.verdict(e_id, ctx.means_class, ctx.horizon(e_id),
                                       ent.current_dof)
            if before.verdict != "reachable":
                continue
            after = closed_world.verdict(e_id, ctx.means_class, ctx.horizon(e_id),
                                         self._projected_dof(ent, option, ctx, h_id))
            if after.verdict == "reachable":
                continue
            rows.append({"entity_id": e_id, "verdict_before": before.verdict,
                         "verdict_after": after.verdict, "critical": e_id in critical,
                         "witness_lost": list(before.witness)})
        return rows

    def candidate_vector(self, state: SystemStateMatrix, option: ActionOption,
                         ctx: Optional[ObservationContext] = None,
                         current_index: Optional[float] = None,
                         h_id: str = OBSERVED_HYPOTHESIS_ID,
                         viable: Optional[bool] = None,
                         resources_ok: Optional[bool] = None) -> Dict[str, object]:
        """§4.5 (v0.11): the keys of one candidate, all of them computed.

        `d1`/`d2`/`d3` are **counts of entities** — the protected dimensions.
        They are integers bounded by `calc(S)` and by the observed graph, and
        they are never mixed with the index: the integers decide admissibility,
        the index selects among those that are admissible.

        `viable` and `resources_ok` complete the predicate (§4.10.1): admissibility
        has **two families of conditions** — *executability* (`viable`, the temporal
        condition of §4.8b; `resources_ok`, the financial condition of §4.8) and
        *structure* (`D1 = D2 = D3 = 0`). All of them are conditions of one
        predicate, not a pipeline of removals, and a failure of any of them bars
        the candidate under the same worst case (§10(E)).
        """
        if current_index is None:
            current_index = self.calculate_system_dof(state, None, ctx)
        simulated, members = self.simulate(state, option, ctx, h_id)
        projected = self.calculate_system_dof(simulated, members, ctx)
        lost = self.lost_paths(state, option, ctx, h_id)
        return {
            "viable": True if viable is None else bool(viable),
            "resources_ok": True if resources_ok is None else bool(resources_ok),
            "d1": len(self.collapse_charges(state, option, ctx, h_id)),
            "d2": len(lost),
            "d3": sum(1 for row in lost if row["critical"]),
            "net_delta": self._net_delta(state, option, projected, current_index),
            "reversible": self.is_reversible(option, h_id),
            "option_id": option.option_id,
        }

    def baseline_vector(self) -> Dict[str, object]:
        """§4.5: staying put — the zero vector, `NetDelta = 0` by definition.

        It is the **comparison origin**, always defined and subject to no
        condition of admissibility: it closes nothing and changes no counter
        under any hypothesis (§4.10.3).
        """
        return {"viable": True, "resources_ok": True, "d1": 0, "d2": 0, "d3": 0,
                "net_delta": 0.0, "reversible": True, "option_id": None}

    def barring_key(self, vector: Dict[str, object]) -> Optional[str]:
        """§4.5/§6.2: the first key on which this candidate fails to beat staying
        put. `None` means nothing barred it — it outranks the baseline, or ties
        it while staying reversible.

        The executability conditions come **first** in the evaluation order of
        §4.8b — temporal, then structural, then financial — and that order is
        unobservable to the result, so any of the three keys reports the same bar.
        """
        if not bool(vector.get("viable", True)):
            return "viable"
        if not bool(vector.get("resources_ok", True)):
            return "resources_ok"
        for key in ("d1", "d2", "d3"):
            if int(vector[key]) > 0:
                return key
        if float(vector["net_delta"]) <= 0.0:
            return "net_delta"
        return None

    # --- §4.10 (v0.11): hypothesis-conditional evaluation ---------------------
    def conditional_vector(self, state: SystemStateMatrix, option: ActionOption,
                           ctx: Optional[ObservationContext],
                           h_id: str = OBSERVED_HYPOTHESIS_ID,
                           groups: Optional[Sequence[Sequence[str]]] = None,
                           rates: Optional[Dict[str, Dict[str, float]]] = None,
                           weights: Optional[Dict[str, float]] = None,
                           cap: Optional[float] = None) -> Dict[str, object]:
        """The full §4.5 vector of one candidate **under one reading**.

        Every quantity of §4.1–§4.9 is conditional (§4.10): the protected
        dimensions come from that reading's `calc(S | h)` and its own `DoF`, and
        the executability conditions from that reading's resource map and τ.

        The financial condition is evaluated with the **same declared mandate and
        observed rates** the cycle uses (§4.8); passing none of them would make
        every option with a `requires` look insolvent, because a deficit could
        never be converted.
        """
        dofs = {e_id: ent.current_dof for e_id, ent in state.entities.items()}
        h_ctx = ctx.with_dof(dofs) if ctx is not None else None
        current_index = self.calculate_system_dof(state, None, h_ctx)
        viability = self.viability(state, option)
        plan = self.plan_funding(state, option, groups, rates, weights, cap)
        return self.candidate_vector(state, option, h_ctx, current_index, h_id,
                                     viable=viability["viable"],
                                     resources_ok=bool(plan["covered"]))

    def viability(self, state: SystemStateMatrix,
                  option: ActionOption) -> Dict[str, object]:
        """§4.8b: the temporal condition, as a **condition** and not a removal.

        `τ' = τ − estimated_duration_mks + projected_tau_delta >= 0` is the whole
        τ arithmetic of an action — the exchange duration is already inside the
        field (§3.3), so a second term would charge τ twice.

        Two cases are named apart (§4.8b): an act that resolves **τ** is viable
        iff it can complete (`τ >= t_m`, or `τ = null`) **and** its declared
        `projected_tau_value` is live (`>= 0`); an act that resolves a **lens or
        another resource** is governed by §4.7 and the strict window of §5. The
        old disjunction `τ >= t_m` OR `τ = null` is withdrawn: as a disjunction it
        admitted a measurement whose own declared projection is negative.
        """
        d = float(option.estimated_duration_mks)
        tau = tau_of(state)
        resolves_tau = "tau" in (option.discovers or [])
        delta = self.derived_tau_delta(state, option)
        if resolves_tau:
            can_complete = tau is None or tau >= d
            expected = option.projected_tau_value
            live = expected is not None and float(expected) >= 0.0
            return {
                "viable": bool(can_complete and live),
                "tau": tau, "tau_after": (None if expected is None else float(expected)),
                "projected_tau_delta": delta,
                "reason": ("a τ measurement is viable iff it can complete and its "
                           "declared result is live (§4.8b)"),
            }
        if tau is None:
            # An unknown budget is not a licence for an action, and it is not a
            # passed deadline either: only a measurement of τ resolves it.
            return {"viable": False, "tau": None, "tau_after": None,
                    "projected_tau_delta": delta,
                    "reason": "τ is unmeasured; measure it first (§4.8b)"}
        delta_term = 0.0 if delta is None else float(delta)
        tau_after = tau - d + delta_term
        return {"viable": tau_after >= 0.0, "tau": tau, "tau_after": tau_after,
                "projected_tau_delta": delta,
                "reason": "τ' = τ − d + projected_tau_delta must be >= 0 (§4.8b)"}

    def derived_tau_delta(self, state: SystemStateMatrix,
                          option: ActionOption) -> Optional[float]:
        """§3.3/§4.8b: `projected_tau_delta` is **derived** for a τ measurement.

        For an act that resolves τ it MUST equal
        `projected_tau_value − (τ − estimated_duration_mks)`, so that
        `τ' = projected_tau_value`; and it MUST be `null` when τ is unknown,
        because the difference is not computable — although the measurement is
        still admitted and evaluated from `projected_tau_value` directly.
        """
        if "tau" not in (option.discovers or []):
            return option.projected_tau_delta
        tau = tau_of(state)
        if tau is None or option.projected_tau_value is None:
            return None
        return float(option.projected_tau_value) - (tau - float(option.estimated_duration_mks))

    def conditional_vectors(self, members: Sequence, options: List[ActionOption],
                            ctx: Optional[ObservationContext] = None,
                            groups: Optional[Sequence[Sequence[str]]] = None,
                            rates: Optional[Dict[str, Dict[str, float]]] = None,
                            weights: Optional[Dict[str, float]] = None,
                            cap: Optional[float] = None
                            ) -> Dict[str, Dict[str, Dict[str, object]]]:
        """§6.3: `{option_id: {hypothesis_id: vector}}` for every candidate."""
        out: Dict[str, Dict[str, Dict[str, object]]] = {}
        for option in options:
            per_h: Dict[str, Dict[str, object]] = {}
            for h in members:
                per_h[h.id] = self.conditional_vector(h.state, option, ctx, h.id,
                                                      groups, rates, weights, cap)
            out[option.option_id] = per_h
        return out

    def robust_admissible(self, per_h: Dict[str, Dict[str, object]],
                          members: Sequence) -> bool:
        """§4.10.1: admissible under **every** plausible hypothesis.

        The predicate is `viable ∧ resources_ok ∧ D1 = D2 = D3 = 0` — both
        families of conditions. The aggregation is a **universally quantified
        conjunction**, and the ordering key is the **least-favourable**
        conditional delta; neither is a maximum over hypotheses, because the
        greatest conditional delta would be exactly the optimistic aggregation
        §4.10 refuses.
        """
        for h in members:
            vector = per_h.get(h.id)
            if vector is None:
                return False
            if not bool(vector.get("viable", True)):
                return False
            if not bool(vector.get("resources_ok", True)):
                return False
            if any(int(vector[k]) > 0 for k in ("d1", "d2", "d3")):
                return False
        return True

    def least_favourable(self, per_h: Dict[str, Dict[str, object]],
                         members: Sequence) -> float:
        """`NetDelta_robust(o) = min_{h ∈ H_plausible} NetDelta(o | h)` (§4.10.2)."""
        values = [float(per_h[h.id]["net_delta"]) for h in members if h.id in per_h]
        return min(values) if values else 0.0

    def admissible_under(self, per_h: Dict[str, Dict[str, object]],
                         members: Sequence) -> Dict[str, bool]:
        """§6.3: per reading, whether the option is admissible, and on what key."""
        out: Dict[str, bool] = {}
        for h in members:
            vector = per_h.get(h.id)
            if vector is None:
                out[h.id] = False
                continue
            barred = (not bool(vector.get("viable", True))
                      or not bool(vector.get("resources_ok", True))
                      or any(int(vector[k]) > 0 for k in ("d1", "d2", "d3")))
            out[h.id] = not barred
        return out

    def hypothesis_conflict(self, per_h_all: Dict[str, Dict[str, Dict[str, object]]],
                            members: Sequence, robust: List[ActionOption]) -> bool:
        """§4.10.5: an unresolved conflict the report MUST surface.

        Fires when `H_plausible` has more than one element and either (a) the
        robust **candidate** set is empty, or (b) some candidate is admissible
        under some plausible hypotheses and inadmissible under others — stated
        over the **whole predicate**, so an option executable under one reading
        and physically impossible under another is as much a conflict as one that
        destroys a counted entity under one reading only.

        A difference that does **not** move admissibility — the same bar, of
        different magnitude, under different readings — is **not** a conflict:
        the choice is the same under both readings, the conditional vectors are
        listed anyway, and the least-favourable key already resolves it.
        """
        if len(members) <= 1:
            return False
        if not robust:
            return True
        robust_ids = {o.option_id for o in robust}
        for option_id, per_h in per_h_all.items():
            flags = self.admissible_under(per_h, members)
            values = set(flags.values())
            if len(values) > 1:
                return True
            if option_id not in robust_ids and values != {False}:
                return True
        return False

    def select_conditional(self, state: SystemStateMatrix,
                           options: List[ActionOption],
                           members: Sequence,
                           ctx: Optional[ObservationContext] = None,
                           groups: Optional[Sequence[Sequence[str]]] = None,
                           rates: Optional[Dict[str, Dict[str, float]]] = None,
                           weights: Optional[Dict[str, float]] = None,
                           cap: Optional[float] = None
                           ) -> Tuple[Optional[ActionOption], Dict[str, object]]:
        """§4.10: robust selection over the declared readings.

        The candidate set is the **same for every hypothesis**; only the
        projection differs (§4.10). Among the robustly admissible candidates take
        the greatest `NetDelta_robust`, then apply §4.5 keys 3 and 4 unchanged
        (reversibility, then the baseline, then the lexicographically smallest
        `option_id`).

        **There is no fallback to admissible support.** An empty robust candidate
        set yields `none` — every proposed action is barred under at least one
        plausible reading, and ranking the survivors of an inadmissible set would
        be the compensation Axiom 3 forbids (§4.10.4).
        """
        per_h_all = self.conditional_vectors(members, options, ctx, groups, rates,
                                             weights, cap)
        robust: List[ActionOption] = []
        for option in options:
            if self.robust_admissible(per_h_all[option.option_id], members):
                robust.append(option)
        robust_keys: List[Tuple[ActionOption, float]] = [
            (o, self.least_favourable(per_h_all[o.option_id], members)) for o in robust
        ]
        vectors = [self.conditional_vector(h.state, o, ctx, h.id, groups, rates,
                                           weights, cap)
                   for o in options for h in members]
        conflict = self.hypothesis_conflict(per_h_all, members, robust)

        # §4.5 key 2: the greatest least-favourable delta, ties by tolerance.
        if robust_keys:
            best = max(value for _, value in robust_keys)
            survivors = [(o, value) for o, value in robust_keys
                         if abs(value - best) <= NET_DELTA_TOLERANCE]
            # §4.5 key 3: prefer the reversible candidate.
            if any(self.is_reversible(o) for o, _ in survivors):
                survivors = [(o, v) for o, v in survivors if self.is_reversible(o)]
            # §4.5 key 4: the survivor must beat the comparison origin.
            survivors = [(o, v) for o, v in survivors if v > 0.0]
        else:
            survivors = []

        payload = {
            "conditional_vectors": per_h_all,
            "admissible_under": {o.option_id: self.admissible_under(per_h_all[o.option_id], members)
                                 for o in options},
            "hypothesis_conflict": conflict,
            "robust_candidates": [o.option_id for o in robust],
            "net_delta_robust": {o.option_id: self.least_favourable(per_h_all[o.option_id], members)
                                 for o in options},
            "vectors": vectors,
        }
        if not survivors:
            return None, payload
        smallest = min(o.option_id for o, _ in survivors)
        return next(o for o, _ in survivors if o.option_id == smallest), payload

    # --- §4.8 resource gate ---------------------------------------------------
    def requirement(self, option: ActionOption) -> Dict[str, float]:
        """§4.8: the option's net draw on the agent, per resource.

        Consumption is the negative component of the declared delta summed over
        the entities the option names. A resource that the option produces more
        of than it consumes yields no requirement — production is not a payment.
        """
        net: Dict[str, float] = {}
        for entity_deltas in option.projected_resource_delta.values():
            for resource, delta in entity_deltas.items():
                net[resource] = net.get(resource, 0.0) + float(delta)
        return {r: -value for r, value in net.items() if value < 0.0}

    @staticmethod
    def _same_group(a: str, b: str, groups: Optional[Sequence[Sequence[str]]]) -> bool:
        """§4.8: exchange is possible only inside a derived group."""
        if a == b:
            return True
        for group in (groups or []):
            members = {str(r) for r in group}
            if a in members and b in members:
                return True
        return False

    def _resource_value(self, obs: 'ResourceObservation', use_estimated: bool = False) -> float:
        """Resolve a resource's usable value (v0.9).

        When `value` is not null, return it. When `value` is null and
        `use_estimated` is True, return `estimated` (for fallback scenarios).
        Otherwise, the resource has no usable value for the gate.
        """
        if obs.value is not None:
            return obs.value
        if use_estimated and obs.estimated is not None:
            return obs.estimated
        return 0.0

    def _is_stale(self, obs: 'ResourceObservation', now: float = 0.0) -> bool:
        """Check if a ResourceObservation's data is stale (§3.2a)."""
        if obs.aging_time <= 0.0:
            return False
        return (now - obs.last_measured_at) > obs.aging_time

    def plan_funding(self, state: SystemStateMatrix, option: ActionOption,
                     groups: Optional[Sequence[Sequence[str]]] = None,
                     rates: Optional[Dict[str, Dict[str, float]]] = None,
                     weights: Optional[Dict[str, float]] = None,
                     cap: Optional[float] = None
                     ) -> Dict[str, object]:
        """§4.8: decide *how* an option is paid for, and whether it can be.

        Step 1 is a direct comparison against the agent's means. Step 2 is
        **verified** conversion: the exchange path must exist (declared rate),
        the resources must share a group, an offer must satisfy the requirement
        (`amount = deficit / rate`), the price must be payable from the agent's
        means, and the exchange's **own time** must still fit in `τ`. Anything
        that fails is not a cheaper conversion — it is a deficit that stays
        uncovered, and step 3 turns that into insolvency.

        The spend ledger is what actually leaves the agent's stock: a deficit
        bought from another resource spends *that* resource, not the one the
        option declared it would consume.
        """
        need = self.requirement(option)
        means = state.resources
        # §3.2b/§4.8 (v0.11): τ is read from the resource map as a **signed**
        # value — a negative τ is a passed deadline, not a zero — and `null`
        # means unmeasured. The deprecated `global_time_to_collapse_mks` mirror
        # is never an input to a rule (§3.1, §4.7, §4.8b).
        tau = tau_of(state)
        # The numeraire weights: used to choose an offer canonically and to
        # express the mandate ceiling in one unit.
        w = {str(k): float(v) for k, v in (weights or {}).items()}
        spend: Dict[str, float] = {}
        conversions: List[Dict[str, object]] = []
        uncovered: Dict[str, float] = {}
        total_duration = option.estimated_duration_mks

        for resource in sorted(need):
            remaining = need[resource]
            res_obs = means.get(resource, ResourceObservation())
            # Stale resources MUST be re-measured before use (§3.2a/§4.8).
            # Here we treat stale as unusable (value 0) for the gate.
            available = max(0.0, self._resource_value(res_obs) - spend.get(resource, 0.0))
            direct = min(remaining, available)
            spend[resource] = spend.get(resource, 0.0) + direct
            remaining -= direct

            # §4.8 (v0.7): the offer is chosen **canonically** — the cheapest in
            # the group numeraire first, then the shorter exchange, then the key.
            # Choosing by declaration order (or by resource name) would let a
            # rename change what the report says happened, and two ports would
            # describe the same world differently.
            offers: List[Tuple[float, float, str, str, float, float]] = []
            for key in sorted(rates or {}):
                source, _, target = key.partition("->")
                if target != resource:
                    continue
                spec = rates[key] or {}
                rate = float(spec.get("rate", 0.0))
                duration = float(spec.get("duration_mks", 0.0))
                if rate <= 0.0 or not self._same_group(source, resource, groups):
                    continue
                amount_source = remaining / rate
                if amount_source > max(0.0, self._resource_value(means.get(source, ResourceObservation())) - spend.get(source, 0.0)):
                    continue                        # the price is not payable
                if tau is None or total_duration + duration > tau:
                    continue                        # does not fit in τ (or τ unknown)
                offers.append((w.get(source, 1.0) * amount_source, duration, key,
                               source, amount_source, rate))
            if remaining > 0.0 and offers:
                _cost, duration, _key, source, amount_source, rate = min(offers)
                spend[source] = spend.get(source, 0.0) + amount_source
                total_duration += duration
                conversions.append({
                    "from": source, "to": resource,
                    "amount_from": amount_source, "amount_to": remaining,
                    "rate": rate, "duration_mks": duration,
                })
                remaining = 0.0

            if remaining > 0.0:
                uncovered[resource] = remaining

        # §4.8 (v0.7): the mandate caps what may be spent, in the group numeraire.
        # It can only remove an option a larger balance would have paid for, and it
        # can never make payable what the measured means cannot cover.
        mandate_exceeded = 0.0
        if cap is not None:
            spent_value = sum(w.get(r, 1.0) * amount for r, amount in spend.items())
            if spent_value > cap:
                mandate_exceeded = spent_value - cap

        return {
            "covered": (not uncovered) and mandate_exceeded <= 0.0,
            "need": need,
            "spend": spend,
            "conversions": conversions,
            "uncovered": uncovered,
            "mandate_exceeded": mandate_exceeded,
            "total_duration_mks": total_duration,
        }

    def apply_resource_gate(self, state: SystemStateMatrix, options: List[ActionOption],
                            groups: Optional[Sequence[Sequence[str]]] = None,
                            rates: Optional[Dict[str, Dict[str, float]]] = None,
                            weights: Optional[Dict[str, float]] = None,
                            cap: Optional[float] = None
                            ) -> Tuple[List[ActionOption], List[Dict[str, str]]]:
        """§4.8 step 3: an unpayable option is inadmissible, unconditionally.

        Unlike the structural gate of §4.5 there is no "no alternative" escape:
        a shortage that survives full verified conversion is a **verdict**, not
        a price, so it cannot be traded against a preference for acting. Not
        affordable is not the same as expensive, exactly as unreachable is not
        the same as distant. Every removal is recorded (§6.2).
        """
        if not options:
            return [], []
        admissible: List[ActionOption] = []
        removed: List[Dict[str, str]] = []
        for option in options:
            if self.plan_funding(state, option, groups, rates, weights, cap)["covered"]:
                admissible.append(option)
            else:
                removed.append({"option_id": option.option_id, "gate": "insolvency"})
        return admissible, removed

    def calculate_system_dof(self, state: SystemStateMatrix,
                             members: Optional[Set[str]] = None,
                             ctx: Optional[ObservationContext] = None) -> float:
        """Evaluation index: pure Nash product (sum of ln(DoF)) over the calc set.

        Values are negative; only their ordering matters (DOF-SPEC §4.1). The
        `members` set is the frozen `calc(S)` of §4.2: when a simulated state is
        scored, the same entities are summed, so a counted entity driven to a
        known zero contributes the floor `ln ε` instead of silently vanishing.
        """
        if members is None:
            members = self.calc_members(state, ctx)
        total_score = 0.0
        for e_id in members:
            entity = state.entities.get(e_id)
            if entity is None:
                continue
            total_score += math.log(max(entity.current_dof, self.epsilon))
        return total_score

    def _net_delta(self, current_state: SystemStateMatrix, option: ActionOption,
                   projected_dof: float, current_dof: float) -> float:
        # §4.4 (v0.7): no flat penalty. An irreversible option's price is already
        # inside `projected_dof`, because the closure lowered the affected
        # entities' Variety counter in `S'` (§4.3); subtracting anything here
        # would charge the same loss twice.
        return projected_dof - current_dof - current_state.context_switch_cost

    def evaluate_and_select(self, current_state: SystemStateMatrix,
                            options: List[ActionOption],
                            ctx: Optional[ObservationContext] = None) -> Optional[ActionOption]:
        """§4.5 (v0.8): the keys are an **ordered filter**, not a tie-break.

        `D1 → D2 → D3 → NetDelta → reversibility → option_id`, each key applied
        only to the survivors of the previous one, with staying put a candidate
        (the zero vector). The survivor is selected only if it beats the baseline
        (`NetDelta > 0`); otherwise selection returns `none` and the system stays.
        """
        return self.select_candidate(current_state, options, ctx)[0]

    def select_candidate(self, current_state: SystemStateMatrix,
                         options: List[ActionOption],
                         ctx: Optional[ObservationContext] = None
                         ) -> Tuple[Optional[ActionOption], List[Dict[str, object]]]:
        """The filter itself: the winner (or `None`) and every candidate's vector.

        `NetDelta` ties are grouped with a tolerance, and for the same reason the
        index is compared with one (§10): the index is a sum of logarithms over a
        set, so two ports that sum in different orders can differ in the last
        bits. A *tie* must be resolved identically everywhere — §7 requires the
        same choice, not only the same numbers.
        """
        if not options:
            return None, []
        current_index = self.calculate_system_dof(current_state, None, ctx)
        candidates = [(o, self.candidate_vector(current_state, o, ctx, current_index))
                      for o in options]
        vectors = [v for _, v in candidates]
        # §4.5: staying put is a candidate **like any other**, so its zero vector
        # enters the set. That is what makes a protected key a *bar* instead of a
        # comparison: any candidate with `d1`, `d2` or `d3` above zero loses to it,
        # and no candidate can ever be preferred for cutting a path. Comparing
        # against the baseline only at the `NetDelta` key would let a positive
        # delta buy a lost path back — exactly the defect this release removes.
        candidates = candidates + [(None, self.baseline_vector())]
        survivors = list(candidates)
        for key in ("d1", "d2", "d3"):               # protected keys: the fewest
            if not survivors:
                break
            best = min(int(v[key]) for _, v in survivors)
            survivors = [(o, v) for o, v in survivors if int(v[key]) == best]
        if survivors:                                 # the index: the greatest
            best = max(float(v["net_delta"]) for _, v in survivors)
            survivors = [(o, v) for o, v in survivors
                         if abs(float(v["net_delta"]) - best) <= NET_DELTA_TOLERANCE]
        if survivors and any(bool(v["reversible"]) for _, v in survivors):
            survivors = [(o, v) for o, v in survivors if bool(v["reversible"])]
        # §4.5 key 6: on a complete tie, staying put wins if it is still a
        # candidate. A zero vector is a full tie with doing nothing, and doing
        # nothing is what that vector means.
        if any(o is None for o, _ in survivors):
            return None, vectors
        if survivors:                                 # deterministic fallback
            smallest = min(str(v["option_id"]) for _, v in survivors)
            survivors = [(o, v) for o, v in survivors if str(v["option_id"]) == smallest]
        if not survivors:
            return None, vectors
        winner, vector = survivors[0]
        # §4.5 key 4: the survivor is selected only if it beats the baseline. With
        # the baseline in the set this is already implied — a winner that reached
        # the end beat it strictly — and the guard stays as a statement of the rule.
        if float(vector["net_delta"]) <= 0.0:
            return None, vectors
        return winner, vectors

    def _is_incomplete(self, state: SystemStateMatrix,
                       options: List[ActionOption],
                       members: Optional[Sequence] = None,
                       robust_ids: Optional[Sequence[str]] = None
                       ) -> bool:
        """§4.7: was a resolvable unknown left unmeasured?

        `v0.11` names the **set**: the scope is the **robustly admissible**
        candidates (§4.10.1; the admissible set when no `H` is declared, §4.10.6),
        not every syntactic candidate. A candidate that measures the unknown but
        is **barred** could not have resolved it, so counting it would declare the
        decision informed when the unknown survived the cycle. The complementary
        obligation is that such a candidate MUST still be reported per option with
        its `discovers` list and its failing condition (§6.3), so the audit shows
        *why* the unknown remained.

        The measurement window is read under each plausible reading's own τ
        (§4.10): an unknown whose window has closed under every reading is not a
        resolvable one.
        """
        scope = set(robust_ids) if robust_ids is not None else None
        unknowns = [e.entity_id for e in state.entities.values() if not e.dof_known]
        if not unknowns:
            return False
        relevant = [o for o in options if scope is None or o.option_id in scope]
        if not relevant:
            return False
        durations = [o.estimated_duration_mks for o in relevant
                     if o.estimated_duration_mks > 0.0]
        cheapest_measurement = min(durations) if durations else None
        readings = list(members) if members else [None]
        for e_id in unknowns:
            touched = any(o.flat_delta().get(e_id, 0.0) != 0.0 for o in relevant)
            if touched:
                continue
            if cheapest_measurement is None:
                continue  # no procedure available at all: nothing to be incomplete about
            for h in readings:
                st = state if h is None else h.state
                window = self.measurement_window(st, cheapest_measurement)
                if window is not None and window > 0.0:
                    return True
        return False

    @staticmethod
    def measurement_window(state: SystemStateMatrix,
                           t_meas_mks: float) -> Optional[float]:
        """§4.7/§5: `t* = τ − T_meas`, the strict measurement window.

        `None` when τ is unmeasured: an unknown budget is not a closed window, and
        the strict `t* > 0` rule of §5 is not applied to it (a τ measurement is
        governed by §4.8b instead, §10(I)).
        """
        tau = tau_of(state)
        if tau is None:
            return None
        return tau - float(t_meas_mks)

    def report(self, current_state: SystemStateMatrix, options: List[ActionOption],
               selected: Optional[ActionOption], mode: str,
               declaration: Optional[MeasurementDeclaration] = None,
               removed_options: Optional[List[Dict[str, str]]] = None,
               groups: Optional[Sequence[Sequence[str]]] = None,
               rates: Optional[Dict[str, Dict[str, float]]] = None,
               ctx: Optional[ObservationContext] = None,
               weights: Optional[Dict[str, float]] = None,
               cap: Optional[float] = None,
               means_provenance: Optional[Dict[str, object]] = None,
               readings: Optional[Sequence] = None,
               metrics: Optional[Dict[str, object]] = None,
               coverage: str = "partial",
               horizon_mks: Optional[float] = None,
               ruler_digest: Optional[str] = None,
               mode_dissenters: Optional[List[Dict[str, object]]] = None) -> DofReport:
        """Transparent audit (DOF-SPEC §6). Required by the license (PoI).

        `v0.11` makes the report **per hypothesis**: `lens_terms` and
        `binding_lens` are reported for each reading, `total_system_dof` becomes a
        map, and every candidate carries its conditional vector — because no
        shared `ψ` exists for an entity once the readings differ (§6.1, §6.3).

        `readings` is the list of `Hypothesis` objects the cycle was evaluated
        over (the observed-state singleton when no set is declared). The name is
        deliberate: `members` is already taken by `calc_members`, and an older
        caller passing the calculation set positionally would silently turn the
        per-reading report into a list of entity identifiers.
        """
        observed = list(readings) if readings else []
        entity_rows: List[Dict[str, object]] = []
        for e_id, ent in current_state.entities.items():
            included = self._is_included(ent, ctx, current_state)
            contribution = math.log(max(ent.current_dof, self.epsilon)) if included else 0.0
            row: Dict[str, object] = {
                "entity_id": e_id,
                "is_collapse_source": ent.is_collapse_source,
                "included_in_sum": included,
                "current_dof": ent.current_dof,
                "dof_known": ent.dof_known,
                "contribution": contribution,
            }
            # §6.1: the report shows *why*, not only *what* — and under a
            # hypothesis set the reason is a property of the reading.
            m = ent.measurement
            row["lens_terms"] = m.terms if m else []
            row["binding_lens"] = m.binding_lens if m else None
            row["floored"] = m.floored if m else False
            row["lens_terms_by_hypothesis"] = {
                h.id: (h.state.entities[e_id].measurement.terms
                       if (h is not None and e_id in h.state.entities
                           and h.state.entities[e_id].measurement) else [])
                for h in observed
            }
            row["binding_lens_by_hypothesis"] = {
                h.id: (h.state.entities[e_id].measurement.binding_lens
                       if (h is not None and e_id in h.state.entities
                           and h.state.entities[e_id].measurement) else None)
                for h in observed
            }
            # §4.6 (v0.6): the derived blocks and the derivation behind them.
            row["blocks"] = m.blocks if m else []
            row["derivation"] = m.derivation if m else None
            # §6.1 (v0.7): the recoverability verdict, its witness and the
            # completeness of the observation behind it — per hypothesis (§4.9).
            row["recoverability"] = self.recoverability_row(e_id, ctx)
            row["recoverability_by_hypothesis"] = {
                h.id: self.recoverability_row(
                    e_id, ctx.with_dof({k: v.current_dof
                                        for k, v in h.state.entities.items()})
                    if ctx is not None else None)
                for h in observed
            }
            entity_rows.append(row)
        total = self.calculate_system_dof(current_state, None, ctx)
        totals_by_h: Dict[str, float] = {}
        for h in observed:
            if h is None:
                continue
            h_ctx = (ctx.with_dof({k: v.current_dof for k, v in h.state.entities.items()})
                     if ctx is not None else None)
            totals_by_h[h.id] = self.calculate_system_dof(h.state, None, h_ctx)

        resources_before = {k: v.model_dump() for k, v in current_state.resources.items()}
        resources_after = dict(resources_before)
        if selected is not None:
            plan = self.plan_funding(current_state, selected, groups, rates, weights, cap)
            for resource, amount in plan["spend"].items():
                if resource in resources_after:
                    obs = current_state.resources.get(resource, ResourceObservation())
                    resources_after[resource] = {**obs.model_dump(), "value": max(0.0, (obs.value or 0.0) - amount)}

        option_rows: List[Dict[str, object]] = []
        current_index = self.calculate_system_dof(current_state, None, ctx)
        for option in options:
            simulated, members = self.simulate(current_state, option, ctx)
            projected_dof = self.calculate_system_dof(simulated, members, ctx)
            vector = self.candidate_vector(current_state, option, ctx, current_index)
            net_delta = float(vector["net_delta"])
            is_selected = (selected is not None and option.option_id == selected.option_id)
            plan = self.plan_funding(current_state, option, groups, rates, weights, cap)
            option_rows.append({
                "option_id": option.option_id,
                "is_reversible": self.is_reversible(option),
                "projected_dof": projected_dof,
                "net_delta": net_delta,
                # §6.3 (v0.8): the structural keys, and — when the candidate lost
                # to staying put — the key that barred it. The integers are what
                # the selection compares; the index only breaks their ties.
                "candidate_vector": vector,
                "barring_key": self.barring_key(vector),
                "lost_paths": self.lost_paths(current_state, option, ctx),
                "selected": is_selected,
                "estimated_duration_mks": option.estimated_duration_mks,
                # §6.3: every collapse this option causes, as an auditable line
                "collapse_charges": self.collapse_charges(current_state, option, ctx),
                # §6.3 (v0.6): what the option draws, and how "affordable" was
                # established — by cash in hand or by an observed trade.
                "resource_consumption": option.projected_resource_delta,
                "conversion_applied": plan["conversions"],
                "resources_uncovered": plan["uncovered"],
                "mandate_exceeded": plan["mandate_exceeded"],
                # §6.3 (v0.7): what the option closes, and how the loss decomposes.
                "closed": [c.model_dump() for c in option.closed],
                "closure_share": self.closure_share(current_state, option, ctx),
                # §6.3 (v0.9): resources this option resolves, and resources
                # for which it used an estimate with a fallback.
                "discovers": list(option.discovers),
                "fallback_for": [],
                # §6.3 (v0.11): the two declared forms, so a reader can see which
                # reading a projection was applied under; the executability
                # conditions; and the conditional vector of every reading. A
                # candidate barred under some hypothesis is **reported**, with
                # the condition it failed (§10(E)).
                "projection_form": option.projection_form(),
                "closure_form": option.closure_form(),
                "viability": self.viability(current_state, option),
                "resources_ok": bool(plan["covered"]),
                "forms_consistent": option.forms_consistent() is None,
            })
            if metrics and metrics.get("conditional_vectors"):
                option_rows[-1]["conditional_vectors"] = \
                    metrics["conditional_vectors"].get(option.option_id, {})
                option_rows[-1]["admissible_under"] = \
                    (metrics.get("admissible_under") or {}).get(option.option_id, {})
                option_rows[-1]["net_delta_robust"] = \
                    (metrics.get("net_delta_robust") or {}).get(option.option_id)
                # §6.3 (v0.11): the key that barred it **under the readings**, so an
                # executability condition (`viable`, `resources_ok`) shows up as the
                # bar instead of hiding behind the structural keys of one state. A
                # candidate barred under *some* reading is a candidate that failed
                # a condition of admissibility under that reading (§4.10.5).
                by_h = {hid: self.barring_key(v)
                        for hid, v in option_rows[-1]["conditional_vectors"].items()}
                option_rows[-1]["barring_key_by_hypothesis"] = by_h
                first_bar = next((k for k in by_h.values() if k is not None), None)
                if first_bar is not None:
                    option_rows[-1]["barring_key"] = first_bar
        robust_ids = list(metrics.get("robust_candidates") or []) if metrics else None
        return DofReport(
            entities=entity_rows,
            total_system_dof=total,
            context_switch_cost=current_state.context_switch_cost,
            global_time_to_collapse_mks=mirror_time_to_collapse(tau_of(current_state)),
            mode=mode,
            options=option_rows,
            psi_id=(declaration.psi_id if declaration else (current_state.psi.id if current_state.psi else None)),
            psi_digest=(declaration.digest() if declaration else (current_state.psi.digest if current_state.psi else None)),
            declaration=(declaration.canonical_text() if declaration else None),
            removed_options=removed_options or [],
            incomplete=self._is_incomplete(current_state, options, readings, robust_ids),
            resources_before=resources_before,
            resources_after=resources_after,
            means_provenance=dict(means_provenance or {}),
            observation_digest=(ctx.observation_digest if ctx else None),
            # §6.2 (v0.8): what the candidates were compared against, and whether
            # any of them beat it. A silent "no action" is an omission.
            baseline=self.baseline_vector(),
            no_candidate_better=bool(options) and selected is None,
            # §6.2 (v0.9): resources left unmeasured and time spent on measurements.
            unknown_resources=[],
            measurement_time_spent=0.0,
            # §6.2/§6.3 (v0.11).
            psi_ruler_digest=ruler_digest,
            hypotheses=[{"id": h.id, "plausible": bool(h.plausible), "basis": h.basis,
                         "collapse_source_candidates": list(h.collapse_source_candidates)}
                        for h in observed if h is not None],
            coverage=coverage,
            hypothesis_horizon_mks=horizon_mks,
            total_system_dof_by_hypothesis=totals_by_h,
            hypothesis_conflict=bool(metrics.get("hypothesis_conflict")) if metrics else False,
            conditional_vectors=(metrics.get("conditional_vectors") or {}) if metrics else {},
            admissible_under=(metrics.get("admissible_under") or {}) if metrics else {},
            net_delta_robust=(metrics.get("net_delta_robust") or {}) if metrics else {},
            robust_candidates=robust_ids or [],
            mode_dissenters=list(mode_dissenters or []),
        )
