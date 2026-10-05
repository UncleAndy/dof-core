// DOF-Core Reactive Circuit with Interruption (Go port).
package main

type DOFOrchestrator struct {
	FastPassThreshold float64
	mapper            *GraphMapper
	generator         *Generator
	core              *DOFCalculusCore
}

func NewDOFOrchestrator(contextSwitchCost float64) *DOFOrchestrator {
	return &DOFOrchestrator{
		FastPassThreshold: 5000000.0,
		mapper:            NewGraphMapper(contextSwitchCost),
		generator:         NewGenerator(),
		core:              NewDOFCalculusCore(),
	}
}

// applyViabilityGate is the §5 viability gate of `v0.9.1`, **retired in `v0.11`**
// — kept for the historical harness, whose fixtures must stay reproducible.
//
// τ is `TauOf(state)`, never the deprecated mirror: a **null** τ is an unknown
// budget, and §4.8b's null case admits only an option that measures τ. An unknown
// budget never licenses acting on it, so every other candidate is barred.
func applyViabilityGate(options []*ActionOption, tau *float64) ([]*ActionOption, []RemovedOption) {
	viable := []*ActionOption{}
	removed := []RemovedOption{}
	for _, o := range options {
		if tau == nil {
			if discovers(o, "tau") {
				viable = append(viable, o)
			} else {
				removed = append(removed, RemovedOption{OptionID: o.OptionID, Gate: "viability"})
			}
			continue
		}
		if o.EstimatedDurationMks <= *tau {
			viable = append(viable, o)
		} else {
			removed = append(removed, RemovedOption{OptionID: o.OptionID, Gate: "viability"})
		}
	}
	return viable, removed
}

// discovers reports whether the option resolves the named resource (§4.7).
func discovers(o *ActionOption, resource string) bool {
	for _, d := range o.Discovers {
		if d == resource {
			return true
		}
	}
	return false
}

// modeFor is §5's reactive-circuit mode, evaluated **once** on the observed τ
// (§4.10): the threshold is deliberately not hypothesis-conditional, because its
// only consequence is the mode and the Generator runs once.
//
// An unknown budget selects `FAST_PASS`: an unknown budget never licenses the
// expensive path. This is the rule `TauOf`'s `null` feeds — reading the clamped
// mirror instead would make an unknown τ indistinguishable from a passed one.
func (o *DOFOrchestrator) modeFor(tau *float64) string {
	if tau == nil || *tau < o.FastPassThreshold {
		return "FAST_PASS"
	}
	return "DEEP_DIVERSIFICATION"
}

func (o *DOFOrchestrator) generate(state *SystemStateMatrix, tau *float64) []*ActionOption {
	if tau == nil || *tau < o.FastPassThreshold {
		return o.generator.SafeFallback(state, 1)
	}
	return o.generator.Synthesize(state, 5)
}

// gates runs §5 → §4.8, with every removal recorded.
//
// v0.8 RETIRED the structural gate of §4.5: a charged candidate is no longer
// removed from the set. It is evaluated, reported in full and made inadmissible
// by the candidate-vector test, so the structural decision is visible per option
// as `d1` instead of as a deletion here. Two gates remain: viability (§5) and
// insolvency (§4.8).
func (o *DOFOrchestrator) gates(state *SystemStateMatrix, options []*ActionOption) ([]*ActionOption, []RemovedOption) {
	decl := o.mapper.LastDeclaration
	// §3.2b (v0.11): τ comes from the resource map — signed, `null` when
	// unmeasured. The deprecated `global_time_to_collapse_mks` mirror is never an
	// input to a rule (§3.1, §4.7, §4.8b).
	tau := TauOf(state)
	viable, removedViability := applyViabilityGate(options, tau)
	var groups [][]string
	var rates map[string]RateInfo
	var weights map[string]float64
	var cap *float64
	if decl != nil {
		groups, rates, weights, cap = decl.Groups, decl.Rates, decl.Weights, decl.MandateCap
	}
	affordable, removedResource := o.core.ApplyResourceGate(state, viable, groups, rates, weights, cap)
	return affordable, append(removedViability, removedResource...)
}

func (o *DOFOrchestrator) Step(raw map[string]interface{}) *ActionOption {
	state := o.mapper.PollEnvironment(raw)
	ctx := o.mapper.LastObservation
	options, _ := o.gates(state, o.generate(state, TauOf(state)))
	return o.core.EvaluateAndSelect(state, options, ctx)
}

func (o *DOFOrchestrator) StepWithReport(raw map[string]interface{}) (*ActionOption, *DofReport) {
	state := o.mapper.PollEnvironment(raw)
	ctx := o.mapper.LastObservation
	// §3.2b (v0.11): τ is read from the resource map — signed, and `null` when
	// unmeasured. The deprecated mirror is never an input to a rule.
	tau := TauOf(state)
	mode := o.modeFor(tau)
	options, allRemoved := o.gates(state, o.generate(state, tau))
	selected := o.core.EvaluateAndSelect(state, options, ctx)

	decl := o.mapper.LastDeclaration
	// §6.2 (v0.7): where the amounts a decision rests on came from — a measured
	// balance or an asserted authority — so a reader can check the ceiling
	// against a measurement instead of against a claim.
	measured := map[string]interface{}{}
	for k, v := range state.Resources {
		measured[k] = v
	}
	var numeraire interface{}
	weights := map[string]interface{}{}
	var mandateCap interface{}
	if decl != nil {
		if decl.Numeraire != nil {
			numeraire = *decl.Numeraire
		}
		for k, v := range decl.Weights {
			weights[k] = v
		}
		if decl.MandateCap != nil {
			mandateCap = *decl.MandateCap
		}
	}
	provenance := map[string]interface{}{
		"source":      "measured balance (§4.8)",
		"measured":    measured,
		"numeraire":   numeraire,
		"weights":     weights,
		"mandate_cap": mandateCap,
	}

	report := o.core.Report(state, options, selected, mode, ReportInput{
		Declaration:     decl,
		Removed:         allRemoved,
		Ctx:             ctx,
		MeansProvenance: provenance,
		Groups: func() [][]string {
			if decl != nil {
				return decl.Groups
			}
			return nil
		}(),
		Rates: func() map[string]RateInfo {
			if decl != nil {
				return decl.Rates
			}
			return nil
		}(),
		Weights: func() map[string]float64 {
			if decl != nil {
				return decl.Weights
			}
			return nil
		}(),
		Cap: func() *float64 {
			if decl != nil {
				return decl.MandateCap
			}
			return nil
		}(),
	})
	return selected, report
}

// StepWithReportOnSet is §6.2/§6.3 (v0.11): the cycle run over a **declared
// hypothesis set**. It is the entry point that makes the per-reading report
// reachable in use — without it the conditional vectors exist in the core and in
// the harness, and no consumer of this port can ever obtain them.
//
// The shape is §4.10's, and three points of it are decisions rather than
// mechanics:
//
//   - the **candidate set is generated once**, from the observed state's τ and
//     mode (§4.10.5), and is then shared: a reading never adds or removes an
//     option;
//   - the set is an **input** and is validated as such — a hypothesis whose
//     stated DoF is not the product of its own counters, or whose collapse-source
//     label differs from the observed one, is non-conformant input and is
//     refused here rather than silently normalized (§3.6);
//   - the selection is `SelectConditional`, so the same payload that decided the
//     cycle is what the report publishes — a report that recomputed the vectors
//     could show numbers no decision rested on.
func (o *DOFOrchestrator) StepWithReportOnSet(raw map[string]interface{},
	hset *HypothesisSet) (*ActionOption, *DofReport) {
	return o.DecideOnSet(o.mapper.PollEnvironment(raw), hset)
}

// DecideOnSet is the same cycle on an already measured state — the entry a caller
// uses when the state comes from somewhere other than the mapper, and the one the
// harness exercises. It is not a second implementation: the raw-level entry above
// delegates here, so the two cannot diverge on which options are evaluated.
func (o *DOFOrchestrator) DecideOnSet(state *SystemStateMatrix,
	hset *HypothesisSet) (*ActionOption, *DofReport) {
	ctx := o.mapper.LastObservation
	tau := TauOf(state)
	mode := o.modeFor(tau)
	options, allRemoved := o.gates(state, o.generate(state, tau))

	members := ResolvedMembers(state, hset)
	if errs := ValidateSet(state, members); len(errs) > 0 {
		errText := ""
		for _, e := range errs {
			errText += e + "; "
		}
		panic("non-conformant hypothesis set: " + errText)
	}
	readings := PlausibleMembers(members)

	decl := o.mapper.LastDeclaration
	var groups [][]string
	var rates map[string]RateInfo
	var weights map[string]float64
	var cap *float64
	if decl != nil {
		groups = decl.Groups
		rates = decl.Rates
		weights = decl.Weights
		cap = decl.MandateCap
	}

	selected, selection := o.core.SelectConditional(state, options, readings, ctx,
		groups, rates, weights, cap)

	measured := map[string]interface{}{}
	for k, v := range state.Resources {
		measured[k] = v
	}
	var numeraire interface{}
	declWeights := map[string]interface{}{}
	var mandateCap interface{}
	if decl != nil {
		if decl.Numeraire != nil {
			numeraire = *decl.Numeraire
		}
		for k, v := range decl.Weights {
			declWeights[k] = v
		}
		if decl.MandateCap != nil {
			mandateCap = *decl.MandateCap
		}
	}
	provenance := map[string]interface{}{
		"source":      "measured balance (§4.8)",
		"measured":    measured,
		"numeraire":   numeraire,
		"weights":     declWeights,
		"mandate_cap": mandateCap,
	}

	in := ReportInput{
		Declaration:     decl,
		Removed:         allRemoved,
		Groups:          groups,
		Rates:           rates,
		Weights:         weights,
		Cap:             cap,
		Ctx:             ctx,
		MeansProvenance: provenance,
		Readings:        readings,
		Declared:        members,
		Coverage:        CoverageOf(hset),
		HorizonMks:      hsetHorizon(hset),
		Selection:       &selection,
	}
	if decl != nil {
		ruler := decl.RulerDigest()
		in.RulerDigest = &ruler
	}
	report := o.core.Report(state, options, selected, mode, in)
	return selected, report
}

// hsetHorizon names the declared analysis horizon, or nil when the set declares
// none — §3.6 reports the horizon that bounds causal coverage, and an undeclared
// horizon is reported as nothing rather than as a zero.
func hsetHorizon(hset *HypothesisSet) *float64 {
	if hset == nil {
		return nil
	}
	return hset.HorizonMks
}
