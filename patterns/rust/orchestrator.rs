// DOF-Core Reactive Circuit with Interruption (Rust port).
// Ties the three layers; switches FAST PASS / DEEP by τ.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::conditional::ConditionalSelection;
use crate::dof_core::{
    tau_of, ActionOption, DofCalculusCore, DofReport, ObservationContext, RemovedOption,
    ReportInput, SystemStateMatrix,
};
use crate::generator::Generator;
use crate::graph_mapper::{GraphMapper, RawObservation};
use crate::hypothesis::{coverage_of, plausible_members, resolved_members, validate_set, HypothesisSet};
use crate::measurement::{MandateValue, Rate};

pub struct DofOrchestrator {
    mapper: GraphMapper,
    generator: Generator,
    core: DofCalculusCore,
}

impl DofOrchestrator {
    /// Normative constant of §5. Kept as an associated const, not a per-instance
    /// field: it is part of the versioned contract, not a tunable.
    pub const FAST_PASS_THRESHOLD_MKS: f64 = 5000000.0;

    pub fn new(context_switch_cost: f64) -> Self {
        DofOrchestrator {
            mapper: GraphMapper::new(context_switch_cost),
            generator: Generator::new(),
            core: DofCalculusCore::new(),
        }
    }

    /// §4.7: does this option resolve the named resource — is it a measurement *of* it?
    fn discovers(option: &ActionOption, resource: &str) -> bool {
        crate::dof_core::option_discovers(option, resource)
    }

    /// §5's reactive-circuit mode, evaluated **once** on the observed τ (§4.10):
    /// the threshold is deliberately not hypothesis-conditional, because its only
    /// consequence is the mode and the Generator runs once.
    ///
    /// An unknown budget selects `FAST_PASS`: an unknown budget never licenses the
    /// expensive path. This is the rule `tau_of`'s `None` feeds — reading the
    /// clamped mirror instead would make an unknown τ indistinguishable from a
    /// passed one.
    fn mode_for(&self, tau: Option<f64>) -> &'static str {
        match tau {
            Some(t) if t >= Self::FAST_PASS_THRESHOLD_MKS => "DEEP_DIVERSIFICATION",
            _ => "FAST_PASS",
        }
    }

    fn generate(&self, state: &SystemStateMatrix, tau: Option<f64>) -> Vec<ActionOption> {
        if matches!(tau, Some(t) if t >= Self::FAST_PASS_THRESHOLD_MKS) {
            self.generator.synthesize(state, 5)
        } else {
            self.generator.safe_fallback(state, 1)
        }
    }

    /// §5/§4.8b: keep the options that can complete before τ and record every
    /// removal — a removal is a decision and must be visible (§6.2).
    ///
    /// τ is `tau_of(state)`, never the deprecated mirror. An **unknown** τ is not
    /// a passed deadline: §4.8b's null case admits only the candidate that measures
    /// τ, because an unknown budget never licenses acting on it.
    fn viability_gate(
        options: Vec<ActionOption>,
        tau: Option<f64>,
    ) -> (Vec<ActionOption>, Vec<RemovedOption>) {
        let mut viable = Vec::new();
        let mut removed = Vec::new();
        for option in options {
            let keep = match tau {
                None => Self::discovers(&option, "tau"),
                Some(t) => option.estimated_duration_mks <= t,
            };
            if keep {
                viable.push(option);
            } else {
                removed.push(RemovedOption {
                    option_id: option.option_id.clone(),
                    gate: "viability".to_string(),
                });
            }
        }
        (viable, removed)
    }

    pub fn measure(&mut self, raw: &HashMap<String, RawObservation>) -> SystemStateMatrix {
        self.mapper.poll_environment(raw)
    }

    /// The three layers, exposed read-only for the conformance harness: a check must
    /// be able to ask what the mapper observed and what the core decided.
    pub fn mapper(&self) -> &GraphMapper {
        &self.mapper
    }

    pub fn core(&self) -> &DofCalculusCore {
        &self.core
    }

    /// The deterministic candidate set for a state (§4.7 coverage is a Generator duty).
    pub fn generator_fallback(&self, state: &SystemStateMatrix, n_options: usize) -> Vec<ActionOption> {
        self.generator.safe_fallback(state, n_options)
    }

    /// The derived exchange layer of the ruler frozen on this cycle: groups, observed
    /// rates, the numeraire weights and the mandate ceiling. They live in the
    /// declaration, so the gate and the report see exactly the exchange layer the
    /// digest covers.
    #[allow(clippy::type_complexity)]
    fn gate_context(
        &self,
    ) -> (
        Option<&Vec<Vec<String>>>,
        Option<&BTreeMap<String, Rate>>,
        Option<&BTreeMap<String, f64>>,
        Option<f64>,
    ) {
        match self.mapper.last_declaration.as_ref() {
            Some(d) => (Some(&d.groups), Some(&d.rates), Some(&d.weights), d.mandate_cap),
            None => (None, None, None, None),
        }
    }

    /// The observation this cycle was decided over (§3.5/§4.9). It reaches the
    /// structural gate on purpose: the charge of §4.2 is taken against `calc(S)`, and
    /// `calc` is decided by the verdicts of §4.9 — so the gate and the index must be
    /// scored against the same set.
    fn observation(&self) -> Option<&ObservationContext> {
        self.mapper.last_observation.as_ref()
    }

    pub fn step(&mut self, raw: &HashMap<String, RawObservation>) -> Option<ActionOption> {
        let state = self.mapper.poll_environment(raw);
        // §3.2b (v0.11): τ comes from the resource map — signed, `null` when
        // unmeasured. The deprecated `global_time_to_collapse_mks` mirror is never
        // an input to a rule (§3.1, §4.7, §4.8b).
        let tau = tau_of(&state);
        let options = self.generate(&state, tau);
        let (options, _removed) = Self::viability_gate(options, tau);
        let ctx = self.observation();
        // v0.8 retired the structural gate of §4.5: a charged candidate is no
        // longer removed from the set — it is evaluated, reported in full and
        // barred by the candidate-vector test.
        let (groups, rates, weights, cap) = self.gate_context();
        let (options, _removed_resource) =
            self.core
                .apply_resource_gate(&state, &options, groups, rates, weights, cap);
        self.core.evaluate_and_select(&state, &options, ctx)
    }

    /// Like step(), but also returns the Proof-of-Implementation audit.
    ///
    /// This is the **flat** audit: no declared set, no per-reading surface. It is kept
    /// because the frozen v0.6-era harness exercises it, and its answer is the one that
    /// evidence recorded. The v0.11 surface is `step_with_report_on_set`.
    pub fn step_with_report(
        &mut self,
        raw: &HashMap<String, RawObservation>,
    ) -> (Option<ActionOption>, DofReport) {
        let state = self.mapper.poll_environment(raw);
        self.decide(&state)
    }

    /// §6.2/§6.3 (v0.11): the same cycle against a **declared hypothesis set**, with the
    /// per-reading report — the entry a v0.11 consumer of this port calls.
    ///
    /// `hset == None` is not a different mode: it is the observed-state singleton of
    /// §4.10.6, one reading, and the report then carries that reading's per-reading
    /// surface (the flat entry publishes **no** per-reading surface at all, which is a
    /// different question and a different answer).
    ///
    /// A set that fails §3.6 validation is **refused**, not repaired: there is no
    /// conformant decision to publish under a set whose readings are not
    /// §4.1-consistent, and repairing it silently would publish one under a reading
    /// nobody declared.
    pub fn step_with_report_on_set(
        &mut self,
        raw: &HashMap<String, RawObservation>,
        hset: Option<&HypothesisSet>,
    ) -> (Option<ActionOption>, DofReport) {
        let state = self.mapper.poll_environment(raw);
        self.decide_on_set(&state, hset)
    }

    /// The candidate set of one cycle, after the v0.11 gates (§5, then §4.8).
    ///
    /// Shared by the flat and the per-reading paths so the two cannot diverge on
    /// **which** options are evaluated: the mode and the candidate set are derived
    /// once, from the **observed** state, and shared by every reading (§4.10.5).
    /// The removal lists are reported for provenance; under `v0.11` they are empty,
    /// because a barred candidate is evaluated and reported rather than removed.
    fn gated_candidates(
        &self,
        state: &SystemStateMatrix,
    ) -> (Vec<ActionOption>, Vec<RemovedOption>) {
        let tau = tau_of(state);
        let options = self.generate(state, tau);
        let (options, removed_viability) = Self::viability_gate(options, tau);
        // Gate order is normative (§5 → §4.8): the reason a reader needs first is the
        // one about the world, not the one about the wallet. v0.8 retired the
        // structural gate of §4.5 — a charged candidate is evaluated, reported in
        // full and barred by the candidate-vector test of §4.5.
        let (groups, rates, weights, cap) = self.gate_context();
        let (options, removed_resource) =
            self.core
                .apply_resource_gate(state, &options, groups, rates, weights, cap);
        let mut all_removed = removed_viability;
        all_removed.extend(removed_resource);
        (options, all_removed)
    }

    /// §6.2 (v0.7): where the amounts a decision rests on came from — a measured
    /// balance or an asserted authority — so a reader can check the ceiling against a
    /// measurement instead of against a claim. Shared by both report paths: the
    /// provenance of the means does not depend on how many readings the cycle has.
    fn means_provenance_of(&self, state: &SystemStateMatrix) -> BTreeMap<String, MandateValue> {
        let mut means_provenance: BTreeMap<String, MandateValue> = BTreeMap::new();
        means_provenance.insert(
            "source".to_string(),
            MandateValue::Text("measured balance (§4.8)".to_string()),
        );
        for (resource, obs) in state.resources.iter() {
            means_provenance.insert(
                format!("measured:{}", resource),
                MandateValue::Number(obs.resource_value(false)),
            );
        }
        if let Some(d) = self.mapper.last_declaration.as_ref() {
            if let Some(n) = d.numeraire.as_ref() {
                means_provenance.insert("numeraire".to_string(), MandateValue::Text(n.clone()));
            }
            if let Some(c) = d.mandate_cap {
                means_provenance.insert("mandate_cap".to_string(), MandateValue::Number(c));
            }
        }
        means_provenance
    }

    /// §6.2/§6.3 (v0.11): the cycle on an **already measured** state, over a declared
    /// hypothesis set, with the per-reading report — the half of `step_with_report`
    /// that does not touch the mapper, and the entry a caller uses when the state comes
    /// from its own pipeline.
    ///
    /// The candidate set, the gates and the mode are shared with `decide`, so the two
    /// cannot diverge on which options are evaluated; the decision and the report come
    /// from **one** conditional pass (§4.10).
    ///
    /// A set that fails §3.6 validation is **refused**, not repaired: a cycle whose
    /// readings are internally inconsistent has no conformant decision to publish,
    /// and quietly deciding over a repaired set would publish a number no reading
    /// produced.
    pub fn decide_on_set(
        &self,
        state: &SystemStateMatrix,
        hset: Option<&HypothesisSet>,
    ) -> (Option<ActionOption>, DofReport) {
        let tau = tau_of(state);
        let mode = self.mode_for(tau);
        let (options, all_removed) = self.gated_candidates(state);
        let ctx = self.observation();

        let members = resolved_members(state, hset);
        let errors = validate_set(state, &members);
        if !errors.is_empty() {
            panic!("non-conformant hypothesis set: {}", errors.join("; "));
        }
        let readings = plausible_members(&members);

        // One conditional pass, and it is the one published: the decision and the
        // report read the same `ConditionalSelection`, so the report cannot describe
        // a decision other than the one that was made (§4.10, §6.3).
        let (groups, rates, weights, cap) = self.gate_context();
        let (selected, selection) = match ctx {
            Some(c) => self.core.select_conditional(
                state, &options, &readings, c, groups, rates, weights, cap,
            ),
            None => (None, ConditionalSelection::default()),
        };

        let report = self.core.report(
            state,
            &options,
            &selected,
            mode,
            ReportInput {
                declaration: self.mapper.last_declaration.as_ref(),
                removed: all_removed,
                groups,
                rates,
                weights,
                cap,
                ctx,
                means_provenance: self.means_provenance_of(state),
                readings: readings.clone(),
                declared: members,
                coverage: coverage_of(hset),
                horizon_mks: hset.and_then(|h| h.horizon_mks),
                selection: Some(&selection),
                ..Default::default()
            },
        );
        (selected, report)
    }

    /// The decision itself, on an already measured state.
    ///
    /// The state is passed **by reference** on purpose: with this sandbox's
    /// rustc 1.95 at `-C opt-level >= 1`, moving `SystemStateMatrix` by value
    /// into this function made the τ comparison read a stale `tau` (mode and
    /// gate came out as if from a previous call), while `opt-level=0` was
    /// correct. Borrowing avoids the miscompile; Go, C++ and Python are
    /// correct at full optimization.
    pub fn decide(&self, state: &SystemStateMatrix) -> (Option<ActionOption>, DofReport) {
        // §3.2b (v0.11): τ is read from the resource map — signed, and `null` when
        // unmeasured. The deprecated mirror is never an input to a rule.
        let tau = tau_of(state);
        let mode = self.mode_for(tau);
        let (options, all_removed) = self.gated_candidates(state);
        let ctx = self.observation();
        let selected = self.core.evaluate_and_select(state, &options, ctx);
        let (groups, rates, weights, cap) = self.gate_context();
        let report = self.core.report(
            state,
            &options,
            &selected,
            mode,
            ReportInput {
                declaration: self.mapper.last_declaration.as_ref(),
                removed: all_removed,
                groups,
                rates,
                weights,
                cap,
                ctx,
                means_provenance: self.means_provenance_of(state),
                ..Default::default()
            },
        );
        (selected, report)
    }
}
