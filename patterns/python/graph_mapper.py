from typing import Dict, List, Optional

from calculus_core import EntityState, ObservationContext, PsiReference, ResourceObservation, SystemStateMatrix
from measurement import (
    LensObservation,
    MeasurementDeclaration,
    build_declaration,
    measure_entity,
    verify_graph_derived,
)
from world_graph import WorldGraph


class GraphMapper:
    """Perception & Mapping Layer (Graph Mapper).

    Polls the environment on every cycle and builds a SystemStateMatrix **through
    the measurement layer** (DOF-SPEC §4.6–§4.7): raw lens inputs → ψ per lens →
    the product that becomes `current_dof`, plus the frozen declaration and its
    digest (§3.4).

    raw_observations: dict of entity_id -> dict with keys:
        is_autonomous (bool), agency_index (float 0..1),
        is_collapse_source (bool), time_to_collapse_mks (float microseconds),
        lenses (dict) — raw lens inputs, see `measurement.LensObservation`:
            {"variety":    {"V": <float>, "V_env": <float>},
             "options":    [[c_g, C_g], ...],
             "requirements": {"energy": <float>, ...},
             "constraint": {"F": <float>, "F_env": <float>}}
        A lens omitted or set to None is **unmeasured**: `u(t)` applies to it,
        the entity's `dof_known` becomes false, and §4.2 keeps the entity in
        `calc` — ignorance is never treated as zero and never as ideal.

    One reserved top-level key carries the resource layer (§3.2, §4.8):

        "resource_layer": {
            "means":     {"energy": <float>, ...},        # the acting agent's stock
            "groups":    [["credit", "energy"], ...],     # derived exchange groups
            "rates":     {"credit->energy": {"rate": <float>, "duration_mks": <float>}},
            "resources": [{"id": "energy", "unit": "joule", "scale": 1.0}, ...],
            "mandate":   {"external_limit_credit": <float>, ...}}

    The layer is what makes `(c_g, C_g)` derivable (§4.6) and what the gate of
    §4.8 decides against; it enters the hashed declaration, so a ruler that
    declares different units or rates is a different ruler.

    psi_id: name and version of the measurement procedure set. It is part of the
    frozen declaration, so two implementations measuring the same state with the
    same procedure produce the same digest (§3.4.3).
    """

    # Keys of `raw_observations` that describe the world/agent, not an entity.
    RESERVED_KEYS = ("resource_layer", "world")

    def __init__(self, context_switch_cost: float = 0.05,
                 psi_id: str = "perception-v1",
                 u0_prior_q: Optional[float] = None):
        self.context_switch_cost = context_switch_cost
        self.psi_id = psi_id
        self.u0_prior_q = u0_prior_q
        # The declaration frozen on the state being built; the orchestrator
        # hands it to the audit report (§6.2).
        self.last_declaration: Optional[MeasurementDeclaration] = None
        # The observation the state was decided over (§3.5/§4.9). Kept beside
        # the state, never inside it: a world graph is a Perception artifact,
        # exactly like the derived groups and the observed rates.
        self.last_observation: Optional[ObservationContext] = None
        # Mismatches between the declared derived numbers and what the named
        # procedures recompute over the observation (§4.6/§4.9). Empty means the
        # ruler is honest; a non-empty list means the declaration claimed a
        # counter its own observation does not support.
        self.last_graph_problems: List[str] = []

    def poll_environment(self, raw_observations: Dict[str, dict]) -> SystemStateMatrix:
        """Build a SystemStateMatrix from raw observations."""
        layer = raw_observations.get("resource_layer") or {}
        means_raw = layer.get("means") or {}
        # v0.9: means can be a dict of floats or a dict of ResourceObservation dicts.
        # We normalize to ResourceObservation dicts here.
        means_obs: Dict[str, dict] = {}
        for k, v in means_raw.items():
            if isinstance(v, (int, float)):
                means_obs[k] = {
                    "value": float(v), "unit": "unknown", "scale": 1.0,
                    "source": "sensor", "last_measured_at": 0.0, "aging_time": 3600.0,
                    "estimated": None, "estimation_source": []
                }
            else:
                means_obs[k] = v
        groups = layer.get("groups") or []
        rates = layer.get("rates") or {}
        units = layer.get("resources") or []
        mandate = layer.get("mandate") or {}

        # §3.5 (v0.7): the observed world graph, when the cycle was given one.
        # It is an *observation*, so it arrives with the measurement and not
        # inside the state: the same state plus a different observation is a
        # different decision, and the report has to say which one was used.
        world_obs = raw_observations.get("world") or {}
        graph: Optional[WorldGraph] = None
        if world_obs:
            nodes: Dict[str, dict] = {}
            for eid, spec in (world_obs.get("entities") or {}).items():
                node = dict(spec)
                node.setdefault("id", str(eid))
                nodes[str(eid)] = node
            graph = WorldGraph(
                entities=nodes,
                means=[str(m) for m in (world_obs.get("means") or [])],
                acts=list(world_obs.get("acts") or []),
                exchanges=list(world_obs.get("exchanges") or []),
            )
        # §4.9: the admissible-means class and the recovery horizon are part of
        # the observation, so a verdict can never be asserted — only computed.
        means_class = [str(c) for c in (world_obs.get("means_class") or [])]
        t_rec = {str(k): float(v) for k, v in (world_obs.get("t_rec") or {}).items()}
        numeraire = world_obs.get("numeraire")
        graph_procedure = str(world_obs.get("procedure")
                              or f"{self.psi_id}:world_verdicts")

        observations: Dict[str, LensObservation] = {}
        min_ttc = float("inf")

        # Pass 1: raw lens inputs and the local deadlines.
        for eid, obs in raw_observations.items():
            if eid in self.RESERVED_KEYS:
                continue
            observations[eid] = LensObservation(**(obs.get("lenses") or {}))
            ttc = float(obs.get("time_to_collapse_mks", float("inf")))
            if not obs.get("is_collapse_source", False) and ttc < min_ttc:
                min_ttc = ttc

        # Global τ is driven by the most urgent non-collapse-source entity
        # (§3.2). A safe large value is used when none exists.
        global_ttc = min_ttc if min_ttc != float("inf") else 1e15

        # §3.2b (v0.11): τ is read from the **resource map**, and the individual
        # deadlines declared alongside it govern it. Three cases, in this order:
        #
        #   1. declared individual deadlines — τ is their minimum, and is `null`
        #      when **any** active deadline is unmeasured: taking the minimum over
        #      the measured ones alone would read an unknown timer as absent
        #      (§10(w), §10(ao));
        #   2. otherwise the `tau` observation of the map, which may be negative
        #      (a passed deadline keeps its magnitude, §10(p)) or `null`;
        #   3. otherwise the legacy entity-minimum, kept so the historical
        #      fixtures of `v0.6`-`v0.9.1` still read as they did.
        declared_deadlines: Dict[str, Optional[float]] = {
            str(k): (None if v is None else float(v))
            for k, v in (layer.get("deadlines") or {}).items()
        }
        tau_obs_raw = means_obs.get("tau")
        if declared_deadlines:
            tau_value: Optional[float] = (
                None if any(v is None for v in declared_deadlines.values())
                else min(v for v in declared_deadlines.values() if v is not None))
        elif tau_obs_raw is not None:
            raw_value = tau_obs_raw.get("value")
            tau_value = None if raw_value is None else float(raw_value)
        else:
            tau_value = global_ttc
        # §3.2b/§10(s): the deprecated mirror is clamped and is **not** τ. It is
        # `0.0` for an unknown and for a passed deadline, and equals τ otherwise.
        mirror_ttc = 0.0 if (tau_value is None or tau_value < 0.0) else float(tau_value)

        # §4.9 (v0.7): the counting horizon of the Variety procedure. A response
        # vector must be executable inside it, so the default is the cycle's own
        # τ — the observation may declare a different one, but never an implicit
        # one: an unpacked horizon would change `V` without appearing anywhere.
        counting_horizon = (float(world_obs.get("counting_horizon_mks", global_ttc))
                            if world_obs else None)

        # §4.6 (v0.7): the numeraire weights and the mandate cap are DERIVED over
        # the observation, not authored. Without a declared numeraire there is no
        # unit for a scalar cap, so neither applies — which is what keeps a
        # ruler without a world graph reading exactly as it did in v0.6.
        weights: Dict[str, float] = {}
        cap: Optional[float] = None
        if graph is not None and numeraire:
            members = sorted({str(r) for grp in (groups or []) for r in grp})
            weights = graph.weights_to(str(numeraire), members)
            # §3.5/§4.8: the axis rates are the *output* of the observation
            # procedure, so with a graph in hand the table is derived rather than
            # read from the layer. A declared table next to an observed graph
            # would be a second ruler for the same quantity, free to drift.
            derived: Dict[str, Dict[str, float]] = {}
            for a in members:
                for b in members:
                    if a == b:
                        continue
                    res = graph.rate(a, b)
                    if res.status == "observed" and res.rate:
                        derived[f"{a}->{b}"] = {"rate": float(res.rate),
                                                "duration_mks": float(res.duration_mks)}
            if derived:
                rates = derived
            limits: List[float] = []
            for key, value in sorted((mandate or {}).items()):
                if key == "cap":
                    limits.append(float(value))
                elif key == "external_limit_credit":
                    # Declared in credits, applied in the numeraire: converted
                    # through the *observed* weight, never a hard-coded 1.0.
                    w_credit = weights.get("credit")
                    if w_credit is not None:
                        limits.append(float(value) * float(w_credit))
            cap = min(limits) if limits else None

        # §4.6/§4.9: the verdicts and the counters are computed by the named
        # procedures and then *declared*, so the declaration can be checked
        # against the observation it came from (`verify_graph_derived`).
        verdicts: Dict[str, Dict[str, object]] = {}
        if graph is not None:
            for eid in sorted(observations):
                v = graph.verdict(eid, means_class, t_rec.get(eid))
                verdicts[eid] = {"verdict": v.verdict,
                                 "t_rec_mks": t_rec.get(eid),
                                 "v": graph.v_count(eid, means_class, counting_horizon)}

        # §3.4.1/§3.4.2 (v0.11): the measurement durations are **declared ruler
        # content**. They are read from the observation, never inferred (§4.7),
        # and they are the same for every reading of the cycle — a hypothesis
        # reinterprets what was measured, not how long the measuring takes.
        durations_raw = raw_observations.get("measurement_durations") or {}
        measurement_durations: Dict[str, Dict[str, float]] = {
            str(lens): {str(k): float(v) for k, v in (durs or {}).items()}
            for lens, durs in durations_raw.items()
        }
        measurement_schedule: Dict[str, float] = {
            str(lens): float(t)
            for lens, t in (raw_observations.get("measurement_schedule") or {}).items()
        }

        # Pass 2: the declaration is frozen on S, so τ is known before measuring.
        declaration = build_declaration(self.psi_id, observations, tau_value, self.u0_prior_q,
                                        resources=units, groups=groups, rates=rates,
                                        mandate=mandate,
                                        numeraire=(str(numeraire) if numeraire else None),
                                        weights=weights, mandate_cap=cap,
                                        verdicts=verdicts,
                                        means_class=means_class,
                                        graph_procedure=graph_procedure,
                                        measurement_durations=measurement_durations)
        self.last_declaration = declaration
        u0 = declaration.u0()   # at t = 0 the schedule of §4.7 gives u₀

        entities: Dict[str, EntityState] = {}
        for eid, obs in raw_observations.items():
            if eid in self.RESERVED_KEYS:
                continue
            measurement = measure_entity(eid, observations[eid], u0,
                                         means=means_obs, groups=groups,
                                         weights=weights, cap=cap)
            entities[eid] = EntityState(
                entity_id=eid,
                is_autonomous=obs.get("is_autonomous", True),
                agency_index=max(0.0, min(1.0, float(obs.get("agency_index", 0.0)))),
                current_dof=measurement.current_dof,
                is_collapse_source=obs.get("is_collapse_source", False),
                dof_known=measurement.dof_known,
                time_to_collapse_mks=float(obs.get("time_to_collapse_mks", float("inf"))),
                measurement=measurement,
            )

        # §3.5/§4.9: the observation itself, pinned by its own digest (§6.2), and
        # the self-check that the declared derived numbers are the ones the named
        # procedures actually return over it.
        self.last_observation = None
        self.last_graph_problems = []
        if graph is not None:
            self.last_observation = ObservationContext(
                world=graph, means_class=means_class, t_rec=t_rec,
                counting_horizon_mks=counting_horizon,
                observation_digest=graph.observation_digest(means_class, t_rec,
                                                            counting_horizon))
            self.last_graph_problems = verify_graph_derived(declaration, graph,
                                                            counting_horizon)

        return SystemStateMatrix(
            global_time_to_collapse_mks=mirror_ttc,
            context_switch_cost=self.context_switch_cost,
            entities=entities,
            psi=PsiReference(id=declaration.psi_id, digest=declaration.digest()),
            resources=means_obs,
            # §3.2b (v0.11): τ lives in the resource map — signed, and `null`
            # when unmeasured. The `tau` entry of `means` **is** this object, so a
            # rule that reads τ reads the map and not a field beside it.
            tau=ResourceObservation(
                value=tau_value,
                unit="us",
                scale=1.0,
                source="resource_map",
                last_measured_at=0.0,
                aging_time=0.0,
            ),
            deadlines=declared_deadlines,
            # §3.4.1/§3.4.2 (v0.11): ruler-level declared durations and the
            # declared measurement schedule. A hypothesis carries them because a
            # hypothesis is a complete `SystemStateMatrix`; it MUST NOT vary
            # them, and `validate_set` refuses a set that does (§3.6).
            measurement_durations=measurement_durations,
            measurement_schedule=measurement_schedule,
        )
