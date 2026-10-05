package main

// DOF-SPEC v0.11 §3.3/§4.8b — the temporal condition, stated once.
// Reference port; mirrors patterns/python/calculus_core.py (`viability`,
// `derived_tau_delta`) exactly.

// ViabilityResult is §4.8b's temporal condition, as a **condition** and not a
// removal: `v0.11` retired the pre-evaluation removal pipeline, so a candidate
// that fails this test is reported per option with the condition it failed
// rather than deleted from the set (§10(E)).
type ViabilityResult struct {
	Viable            bool     `json:"viable"`
	Tau               *float64 `json:"tau"`
	TauAfter          *float64 `json:"tau_after"`
	ProjectedTauDelta *float64 `json:"projected_tau_delta"`
	Reason            string   `json:"reason"`
}

// DerivedTauDelta is §3.3/§4.8b: `projected_tau_delta` is **derived** for a τ
// measurement, never independently declared.
//
// The declared field is the whole projection for every other act. For an act
// that resolves τ it MUST equal `projected_tau_value - (τ - estimated_duration)`
// — `τ'` computed from the value the act expects to leave behind — and it MUST be
// `nil` when τ is unknown, because the difference is not computable; the
// measurement is still admitted and evaluated from `projected_tau_value`
// directly.
func (c *DOFCalculusCore) DerivedTauDelta(state *SystemStateMatrix,
	option *ActionOption) *float64 {
	if !discovers(option, "tau") {
		return option.ProjectedTauDelta
	}
	tau := TauOf(state)
	if tau == nil || option.ProjectedTauValue == nil {
		return nil
	}
	delta := *option.ProjectedTauValue - (*tau - option.EstimatedDurationMks)
	return &delta
}

// Viability is §4.8b: `τ' = τ − estimated_duration_mks + projected_tau_delta >= 0`
// is the whole τ arithmetic of an action — the exchange duration is already
// inside the field (§3.3), so a second term would charge τ twice.
//
// Two cases are named apart:
//
//   - an act that resolves **τ** is viable iff it can complete (`τ >= t_m`, or
//     `τ` is unknown) **and** its declared `projected_tau_value` is live
//     (`>= 0`);
//   - an act that resolves a **lens or another resource** is governed by §4.7
//     and the strict window of §5, and with an unknown τ it is not viable at all:
//     an unknown budget is not a licence for an action, and it is not a passed
//     deadline either — only a measurement of τ resolves it.
//
// The old disjunction `τ >= t_m` OR `τ = null` is withdrawn: as a disjunction it
// admitted a measurement whose own declared projection is negative.
func (c *DOFCalculusCore) Viability(state *SystemStateMatrix,
	option *ActionOption) ViabilityResult {
	d := option.EstimatedDurationMks
	tau := TauOf(state)
	resolvesTau := discovers(option, "tau")
	delta := c.DerivedTauDelta(state, option)
	if resolvesTau {
		canComplete := tau == nil || *tau >= d
		live := option.ProjectedTauValue != nil && *option.ProjectedTauValue >= 0.0
		return ViabilityResult{
			Viable:            canComplete && live,
			Tau:               tau,
			TauAfter:          option.ProjectedTauValue,
			ProjectedTauDelta: delta,
			Reason: "a τ measurement is viable iff it can complete and its " +
				"declared result is live (§4.8b)",
		}
	}
	if tau == nil {
		return ViabilityResult{Viable: false, Tau: nil, TauAfter: nil,
			ProjectedTauDelta: delta,
			Reason:            "τ is unmeasured; measure it first (§4.8b)"}
	}
	deltaTerm := 0.0
	if delta != nil {
		deltaTerm = *delta
	}
	tauAfter := *tau - d + deltaTerm
	return ViabilityResult{Viable: tauAfter >= 0.0, Tau: tau, TauAfter: &tauAfter,
		ProjectedTauDelta: delta,
		Reason:            "τ' = τ − d + projected_tau_delta must be >= 0 (§4.8b)"}
}
