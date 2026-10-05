// DOF-SPEC v0.11 §3.3/§4.8b — the temporal condition, stated once (Rust port).
// Mirrors patterns/python/calculus_core.py (`viability`, `derived_tau_delta`) and
// patterns/go/viability.go exactly.

use crate::dof_core::{option_discovers, tau_of, ActionOption, DofCalculusCore, SystemStateMatrix};

/// §4.8b's temporal condition, as a **condition** and not a removal: `v0.11`
/// retired the pre-evaluation removal pipeline, so a candidate that fails this test
/// is reported per option with the condition it failed rather than deleted from the
/// set (§10(E)).
#[derive(Clone, Debug)]
pub struct ViabilityResult {
    pub viable: bool,
    pub tau: Option<f64>,
    pub tau_after: Option<f64>,
    pub projected_tau_delta: Option<f64>,
    pub reason: String,
}

impl DofCalculusCore {
    /// §3.3/§4.8b: `projected_tau_delta` is **derived** for a τ measurement, never
    /// independently declared.
    ///
    /// The declared field is the whole projection for every other act. For an act
    /// that resolves τ it MUST equal
    /// `projected_tau_value − (τ − estimated_duration)` — `τ'` computed from the
    /// value the act expects to leave behind — and it MUST be `None` when τ is
    /// unknown, because the difference is not computable; the measurement is still
    /// admitted and evaluated from `projected_tau_value` directly.
    pub fn derived_tau_delta(
        &self,
        state: &SystemStateMatrix,
        option: &ActionOption,
    ) -> Option<f64> {
        if !option_discovers(option, "tau") {
            return option.projected_tau_delta;
        }
        let tau = tau_of(state)?;
        let value = option.projected_tau_value?;
        Some(value - (tau - option.estimated_duration_mks))
    }

    /// §4.8b: `τ' = τ − estimated_duration_mks + projected_tau_delta >= 0` is the
    /// whole τ arithmetic of an action — the exchange duration is already inside the
    /// field (§3.3), so a second term would charge τ twice.
    ///
    /// Two cases are named apart:
    ///
    /// * an act that resolves **τ** is viable iff it can complete (`τ >= t_m`, or
    ///   `τ` is unknown) **and** its declared `projected_tau_value` is live (`>= 0`);
    /// * an act that resolves a **lens or another resource** is governed by §4.7 and
    ///   the strict window of §5, and with an unknown τ it is not viable at all: an
    ///   unknown budget is not a licence for an action, and it is not a passed
    ///   deadline either — only a measurement of τ resolves it.
    ///
    /// The old disjunction `τ >= t_m` OR `τ = null` is withdrawn: as a disjunction
    /// it admitted a measurement whose own declared projection is negative.
    pub fn viability(&self, state: &SystemStateMatrix, option: &ActionOption) -> ViabilityResult {
        let d = option.estimated_duration_mks;
        let tau = tau_of(state);
        let resolves_tau = option_discovers(option, "tau");
        let delta = self.derived_tau_delta(state, option);
        if resolves_tau {
            let can_complete = match tau {
                None => true,
                Some(t) => t >= d,
            };
            let live = matches!(option.projected_tau_value, Some(v) if v >= 0.0);
            return ViabilityResult {
                viable: can_complete && live,
                tau,
                tau_after: option.projected_tau_value,
                projected_tau_delta: delta,
                reason: "a τ measurement is viable iff it can complete and its declared \
                         result is live (§4.8b)"
                    .to_string(),
            };
        }
        let tau = match tau {
            None => {
                return ViabilityResult {
                    viable: false,
                    tau: None,
                    tau_after: None,
                    projected_tau_delta: delta,
                    reason: "τ is unmeasured; measure it first (§4.8b)".to_string(),
                }
            }
            Some(t) => t,
        };
        let tau_after = tau - d + delta.unwrap_or(0.0);
        ViabilityResult {
            viable: tau_after >= 0.0,
            tau: Some(tau),
            tau_after: Some(tau_after),
            projected_tau_delta: delta,
            reason: "τ' = τ − d + projected_tau_delta must be >= 0 (§4.8b)".to_string(),
        }
    }
}