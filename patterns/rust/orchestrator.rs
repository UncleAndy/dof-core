// DOF-Core Reactive Circuit with Interruption (Rust port).
// Ties the three layers; switches FAST PASS / DEEP by τ.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::dof_core::{
    tau_of, ActionOption, DofCalculusCore, DofReport, ObservationContext, RemovedOption,
    ReportInput, SystemStateMatrix,
};
use crate::generator::Generator;
use crate::graph_mapper::{GraphMapper, RawObservation};
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
    pub fn step_with_report(
        &mut self,
        raw: &HashMap<String, RawObservation>,
    ) -> (Option<ActionOption>, DofReport) {
        let state = self.mapper.poll_environment(raw);
        self.decide(&state)
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
        let options = self.generate(state, tau);
        let (options, removed) = Self::viability_gate(options, tau);
        let ctx = self.observation();
        let (options, removed_structural) = (options, Vec::new());
        // Gate order is normative (§5 → §4.8): the reason a reader needs first is
        // the one about the world, not the one about the wallet. v0.8 retired the
        // structural gate of §4.5 — a charged candidate is evaluated, reported in
        // full and barred by the candidate-vector test of §4.5.
        let (groups, rates, weights, cap) = self.gate_context();
        let (options, removed_resource) =
            self.core
                .apply_resource_gate(state, &options, groups, rates, weights, cap);
        let mut all_removed = removed;
        all_removed.extend(removed_structural);
        all_removed.extend(removed_resource);
        let selected = self.core.evaluate_and_select(state, &options, ctx);

        // §6.2 (v0.7): where the amounts a decision rests on came from — a measured
        // balance or an asserted authority — so a reader can check the ceiling against
        // a measurement instead of against a claim.
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
                means_provenance,
            },
        );
        (selected, report)
    }
}
