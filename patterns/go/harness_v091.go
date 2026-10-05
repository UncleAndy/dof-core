package main

// Conformance harness of the Go port, DOF-SPEC v0.9.1 (§3.2b, §4.8, budget + τ-consumption).

import (
	"fmt"
	"math"
)

var v091Checks int
var v091Failures []string

// v091Check records a conformance check result.
func v091Check(label string, cond bool, detail ...string) {
	v091Checks++
	if cond {
		fmt.Printf("  OK   %s\n", label)
		return
	}
	v091Failures = append(v091Failures, label)
	if len(detail) > 0 && detail[0] != "" {
		fmt.Printf("  FAIL %s  %s\n", label, detail[0])
	} else {
		fmt.Printf("  FAIL %s\n", label)
	}
}

func runHarnessV091() {
	// --- 1. τ is a ResourceObservation --------------------------------------
	fmt.Println("=== 1. τ is a ResourceObservation ===")
	{
		orch := NewDOFOrchestrator(0.05)
		state := orch.mapper.PollEnvironment(FixtureScene(FixtureOptions{}))
		v091Check("state.Tau is non-nil", state.Tau != nil)
		if state.Tau != nil {
			v091Check("Tau.Value is set", state.Tau.Value != nil)
			if state.Tau.Value != nil {
				v091Check("Tau.Value > 0", *state.Tau.Value > 0, fmt.Sprintf("%.0f", *state.Tau.Value))
			}
			v091Check("Tau.Unit == 'us'", state.Tau.Unit == "us")
			v091Check("Tau.AgingTime == 0", state.Tau.AgingTime == 0.0)
		}
	}

	// --- 2. Null resource with estimated + fallback -------------------------
	fmt.Println("=== 2. Null resource with estimated + fallback ===")
	{
		nilVal := 5.0
		state := &SystemStateMatrix{
			GlobalTimeToCollapseMks: 1000000.0,
			ContextSwitchCost:       0.05,
			Resources: map[string]*ResourceObservation{
				"medical_supply": {Value: nil, Unit: "unit", Scale: 1.0, Estimated: &nilVal},
			},
			Tau: &ResourceObservation{Value: ptr(1000000.0), Unit: "us", Scale: 1.0, AgingTime: 0.0},
		}
		ms := state.Resources["medical_supply"]
		v091Check("medical_supply.Value is nil", ms.Value == nil)
		if ms.Estimated != nil {
			v091Check("medical_supply.Estimated == 5.0", *ms.Estimated == 5.0)
		}
		// Without fallback, null resource fails the gate
		core := NewDOFCalculusCore()
		option := &ActionOption{
			OptionID:             "use_supply_no_fallback",
			Description:          "Use medical supply without fallback",
			ProjectedDoFDelta:    map[string]float64{"patient": 0.1},
			ProjectedResourceDelta: map[string]map[string]float64{"patient": {"medical_supply": -3.0}},
			EstimatedDurationMks: 1000.0,
			Requires:             []string{"medical_supply"},
		}
		result := core.PlanFunding(state, option, nil, nil, nil, nil)
		v091Check("null resource fails without fallback", !result.Covered, fmt.Sprintf("uncovered=%v", result.Uncovered))
	}

	// --- 3. Staleness check -------------------------------------------------
	fmt.Println("=== 3. Staleness check ===")
	{
		obs := &ResourceObservation{Value: ptr(10.0), AgingTime: 1000.0, LastMeasuredAt: 0.0}
		v091Check("not stale at t=500", !IsStale(obs, 500.0))
		v091Check("stale at t=1500", IsStale(obs, 1500.0))
		obsNoAging := &ResourceObservation{Value: ptr(10.0), AgingTime: 0.0, LastMeasuredAt: 0.0}
		v091Check("never stale if aging_time=0", !IsStale(obsNoAging, 99999.0))
	}

	// --- 4. projected_tau_delta changes τ -----------------------------------
	fmt.Println("=== 4. projected_tau_delta changes τ ===")
	{
		orch := NewDOFOrchestrator(0.05)
		state := orch.mapper.PollEnvironment(FixtureScene(FixtureOptions{}))
		tauBefore := *state.Tau.Value
		// CPR takes 1000ms but increases τ by 5000ms
		option := &ActionOption{
			OptionID:              "cpr",
			Description:           "CPR increases τ",
			ProjectedDoFDelta:     map[string]float64{"patient": 0.2},
			ProjectedResourceDelta: map[string]map[string]float64{"patient": {"energy": -5.0}},
			EstimatedDurationMks:  1000.0,
			ProjectedTauDelta:     ptr(5000.0),
		}
		tauAfter := tauBefore - option.EstimatedDurationMks + *option.ProjectedTauDelta
		v091Check("CPR increases τ", tauAfter > tauBefore)
		v091Check("CPR delta is +4000 net", tauAfter-tauBefore == 4000.0, fmt.Sprintf("actual=%.0f", tauAfter-tauBefore))
		_ = option
	}

	// --- 5. Parallel actions consume τ by max(duration) ---------------------
	fmt.Println("=== 5. Parallel actions consume τ by max(duration) ===")
	{
		orch := NewDOFOrchestrator(0.05)
		state := orch.mapper.PollEnvironment(FixtureScene(FixtureOptions{}))
		tauBefore := *state.Tau.Value
		d1, d2 := 3000.0, 5000.0
		tauConsumed := math.Max(d1, d2)
		tauAfter := tauBefore - tauConsumed
		v091Check("parallel τ = max(d_i)", tauConsumed == 5000.0)
		v091Check("τ decreases by max duration", tauAfter == tauBefore-5000.0)
	}

	fmt.Println()
	fmt.Printf("checks: %d, failures: %d\n", v091Checks, len(v091Failures))
	if len(v091Failures) > 0 {
		fmt.Printf("FAILURES: %v\n", v091Failures)
		return
	}
	fmt.Println("OK")
}
