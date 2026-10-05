from typing import Dict, List, Optional, Sequence, Tuple

from calculus_core import (OBSERVED_HYPOTHESIS_ID, ActionOption, DofReport,
                           DOFCalculusCore, SystemStateMatrix, tau_of)
from graph_mapper import GraphMapper
from generator import Generator
from hypothesis import (HypothesisSet, plausible_members, resolved_members,
                        validate_set)


class DOFOrchestrator:
    """Reactive Circuit with Interruption (Time-Bounded Interrupter).

    Ties the three layers together and links compute cycles to the physical
    time remaining before collapse (τ).

    **`v0.11` retires the removal mechanism.** The gates of §4.8, §4.8b and §5
    are no longer a pipeline that deletes candidates before evaluation: they are
    **conditions of admissibility** (§4.5, §4.10.1), computed per option and
    reported per option with the condition it failed. Two reasons, both
    normative:

    * under a declared hypothesis set there is **no single state** on which a
      pre-evaluation removal could run, and §4.10 gives no rule for composing
      removals across readings — so the sentence "a gate may clear under one
      reading and fail under another" had no operational meaning at all;
    * a removal and an admissibility condition expressing the same rule is the
      double protection §4.5 refuses, and `admissible_under` — "an option barred
      under some hypothesis lists the hypotheses that bar it" — presupposes that
      the option **is** evaluated.

    So `removed_options` is **kept and marked, never filled**: it stays in the
    report so a reader of an older report still knows what it meant.
    """

    FAST_PASS_THRESHOLD = 5000000.0  # microseconds (DOF-SPEC §5)

    def __init__(self, context_switch_cost: float = 0.05, llm_client=None,
                 psi_id: str = "perception-v1", u0_prior_q: Optional[float] = None):
        self.mapper = GraphMapper(context_switch_cost=context_switch_cost,
                                  psi_id=psi_id, u0_prior_q=u0_prior_q)
        self.generator = Generator(llm_client=llm_client)
        self.core = DOFCalculusCore()

    # ---------------------------------------------------------------- §5 mode
    # --- HISTORICAL (v0.6–v0.9.1), retired in v0.11 ---------------------------
    @staticmethod
    def _apply_viability_gate(options: List[ActionOption], tau: float
                              ) -> Tuple[List[ActionOption], List[Dict[str, str]]]:
        """§5 **v0.9.1 rule, retired in `v0.11`** — kept for the historical harness.

        The gate is no longer a *removal*: it is a **condition** of admissibility
        (`viable`, §4.8b), computed per option and reported per option with the
        condition it failed, because under a hypothesis set there is no single
        state for a pre-evaluation removal to run on and `admissible_under`
        presupposes that the option is evaluated (§10(E)). Nothing on the live
        path calls this; the `v0.6` reference harness asserts the rule that was in
        force then, and history must stay reproducible.
        """
        viable: List[ActionOption] = []
        removed: List[Dict[str, str]] = []
        for option in options:
            if option.estimated_duration_mks <= tau:
                viable.append(option)
            else:
                removed.append({"option_id": option.option_id, "gate": "viability"})
        return viable, removed

    def _gates(self, state: SystemStateMatrix, options: List[ActionOption]
               ) -> Tuple[List[ActionOption], List[Dict[str, str]]]:
        """**`v0.9.1` removal pipeline, retired in `v0.11`** — historical only.

        Two gates ran here (viability §5, then insolvency §4.8) and each recorded
        its own deletions. They are now conditions of one admissibility predicate
        (§4.5, §4.10.1) and no candidate leaves the set: it is evaluated, reported
        in full and loses to the comparison origin on the key that barred it, so
        the decision stays per option and visible.
        """
        ctx = self.mapper.last_observation
        declaration = self.mapper.last_declaration
        tau = state.global_time_to_collapse_mks
        viable, removed_viability = self._apply_viability_gate(options, tau)
        affordable, removed_resource = self.core.apply_resource_gate(
            state, viable,
            groups=declaration.groups if declaration else None,
            rates=declaration.rates if declaration else None,
            weights=declaration.weights if declaration else None,
            cap=declaration.mandate_cap if declaration else None)
        return affordable, removed_viability + removed_resource

    def mode_for(self, tau: Optional[float]) -> str:
        """§5: the reactive-circuit mode, evaluated **once**, on observed τ.

        The threshold is deliberately **not** hypothesis-conditional: its only
        consequence is the mode, the mode governs candidate generation, the
        Generator runs once, and the candidate set is the same for every
        hypothesis — so a per-hypothesis threshold would be a quantity with no
        effect (§4.10). An unknown budget selects `FAST_PASS`: an unknown budget
        never licenses the expensive path.
        """
        if tau is None:
            return "FAST_PASS"
        return ("FAST_PASS" if tau < self.FAST_PASS_THRESHOLD
                else "DEEP_DIVERSIFICATION")

    def _generate(self, state: SystemStateMatrix, tau: Optional[float]) -> List[ActionOption]:
        if tau is None or tau < self.FAST_PASS_THRESHOLD:
            return self.generator.safe_fallback(state, n_options=1)
        return self.generator.synthesize(state, n_options=5)

    def _mode_dissenters(self, state: SystemStateMatrix,
                         members: Sequence) -> List[Dict[str, object]]:
        """§5/§6.2: the readings whose τ would have selected a different mode.

        Reported as **context**: the disagreement is not a `hypothesis_conflict`
        (§4.10.5), because it moves no admissibility — the mode and the candidate
        set are identical under every reading.
        """
        observed_mode = self.mode_for(tau_of(state))
        out: List[Dict[str, object]] = []
        for h in members:
            h_mode = self.mode_for(tau_of(h.state))
            if h_mode != observed_mode:
                out.append({"hypothesis_id": h.id, "would_select": h_mode,
                            "tau": tau_of(h.state)})
        return out

    # ------------------------------------------------------------- the cycle
    def step(self, raw_observations: dict,
             hset: Optional[HypothesisSet] = None) -> Optional[ActionOption]:
        """Run one decision cycle and return the verified safe vector."""
        return self.step_with_report(raw_observations, hset)[0]

    def step_with_report(self, raw_observations: dict,
                         hset: Optional[HypothesisSet] = None
                         ) -> Tuple[Optional[ActionOption], DofReport]:
        """Like step(), but also returns the Proof-of-Implementation audit."""
        state: SystemStateMatrix = self.mapper.poll_environment(raw_observations)
        ctx = self.mapper.last_observation
        declaration = self.mapper.last_declaration
        # §3.2b (v0.11): τ is read from the resource map — signed, and `null`
        # when unmeasured. The deprecated `global_time_to_collapse_mks` mirror is
        # never an input to a rule.
        tau = tau_of(state)
        mode = self.mode_for(tau)

        members = resolved_members(state, hset)
        errors = validate_set(state, members)
        if errors:
            raise ValueError("non-conformant hypothesis set: " + "; ".join(errors))
        readings = plausible_members(members)

        options = self._generate(state, tau)
        for option in options:
            problem = option.forms_consistent()
            if problem is not None:
                raise ValueError(problem)

        selected, metrics = self.core.select_conditional(
            state, options, readings, ctx,
            groups=declaration.groups if declaration else None,
            rates=declaration.rates if declaration else None,
            weights=declaration.weights if declaration else None,
            cap=declaration.mandate_cap if declaration else None)

        # §6.2 (v0.7): where the amounts a decision rests on came from — a
        # measured balance or an asserted authority — so a reader can check the
        # ceiling against a measurement instead of against a claim.
        means_provenance: Dict[str, object] = {
            "source": "measured balance (§4.8)",
            "measured": dict(state.resources),
            "numeraire": declaration.numeraire if declaration else None,
            "weights": dict(declaration.weights) if declaration else {},
            "mandate_cap": declaration.mandate_cap if declaration else None,
        }
        report = self.core.report(
            state, options, selected, mode,
            declaration=declaration,
            # §10(E): the removal mechanism is retired; the list is kept empty
            # and the per-option conditions carry what it used to carry.
            removed_options=[],
            groups=declaration.groups if declaration else None,
            rates=declaration.rates if declaration else None,
            ctx=ctx,
            weights=declaration.weights if declaration else None,
            cap=declaration.mandate_cap if declaration else None,
            means_provenance=means_provenance,
            readings=readings,
            metrics=metrics,
            coverage=(hset.coverage if hset else "partial"),
            horizon_mks=(hset.horizon_mks if hset else None),
            ruler_digest=(declaration.ruler_digest() if declaration else None),
            mode_dissenters=self._mode_dissenters(state, readings))
        return selected, report
