package main

// DOF-SPEC v0.11 §4.10 — robust selection over the declared readings.
// Reference port; mirrors patterns/python/calculus_core.py (`conditional_vector`,
// `conditional_vectors`, `robust_admissible`, `least_favourable`,
// `admissible_under`, `hypothesis_conflict`, `robust_reversible`,
// `select_conditional`).
//
// The shape of the rule, in one place:
//
//   - the **candidate set is the same for every hypothesis**; only the
//     projection differs. A reading never adds or removes an option;
//   - admissibility is a **universally quantified conjunction** over
//     `H_plausible` — `viable ∧ resources_ok ∧ D1 = D2 = D3 = 0` under *every*
//     plausible reading;
//   - the ordering key is the **least-favourable** conditional delta
//     `min_h NetDelta(o | h)`, never a maximum: the greatest conditional delta
//     would be exactly the optimistic aggregation §4.10 refuses;
//   - §4.5 keys 3 and 4 then apply unchanged, key 3 read **robustly**
//     (reversible under every plausible reading) because key 2 has already
//     charged the closure at its worst;
//   - an empty robust candidate set yields `none`. There is **no fallback to
//     admissible support**: ranking the survivors of an inadmissible set would
//     be the compensation Axiom 3 forbids.

import "math"

// ConditionalVector is §6.3: one candidate's vector **under one reading**.
//
// It carries the whole admissibility predicate — `viable`, `resources_ok` and
// the three dimensions — so that §4.10's aggregation reads one structure and not
// three.
type ConditionalVector struct {
	CandidateVector
	HypothesisID string `json:"hypothesis_id"`
}

// ConditionalSelection is §6.3's decision payload: everything the report needs
// to show what was decided and under which reading.
type ConditionalSelection struct {
	ConditionalVectors map[string]map[string]ConditionalVector `json:"conditional_vectors"`
	AdmissibleUnder    map[string]map[string]bool              `json:"admissible_under"`
	HypothesisConflict bool                                    `json:"hypothesis_conflict"`
	RobustAdmissible   []string                                `json:"robust_admissible"`
	NetDeltaRobust     map[string]float64                      `json:"net_delta_robust"`
}

// ConditionalVectorOf is §4.5's full vector of one candidate under one reading.
//
// The state passed in is that reading's own state, and the context is the same
// observation read under that reading's DoF (§4.9): every quantity of §4.1–§4.9
// is conditional (§4.10). The financial condition is evaluated with the **same
// declared mandate and observed rates** the cycle uses (§4.8) — passing none of
// them would make every option with a `requires` look insolvent, because a
// deficit could never be converted.
func (c *DOFCalculusCore) ConditionalVectorOf(state *SystemStateMatrix,
	option *ActionOption, ctx *ObservationContext, hypothesisID string,
	groups [][]string, rates map[string]RateInfo, weights map[string]float64,
	cap *float64) ConditionalVector {
	dofs := map[string]float64{}
	for eID, ent := range state.Entities {
		dofs[eID] = ent.CurrentDoF
	}
	hCtx := ctx.WithDoF(dofs)
	currentIndex := c.CalculateSystemDoF(state, nil, hCtx)
	viability := c.Viability(state, option)
	plan := c.PlanFunding(state, option, groups, rates, weights, cap)
	return c.candidateVectorFor(state, option, hCtx, currentIndex, hypothesisID,
		viability.Viable, plan.Covered)
}

// ConditionalVectors is §6.3: `{option_id: {hypothesis_id: vector}}`.
func (c *DOFCalculusCore) ConditionalVectors(members []*Hypothesis,
	options []*ActionOption, ctx *ObservationContext, groups [][]string,
	rates map[string]RateInfo, weights map[string]float64,
	cap *float64) map[string]map[string]ConditionalVector {
	out := map[string]map[string]ConditionalVector{}
	for _, option := range options {
		perH := map[string]ConditionalVector{}
		for _, h := range members {
			perH[h.ID] = c.ConditionalVectorOf(h.State, option, ctx, h.ID,
				groups, rates, weights, cap)
		}
		out[option.OptionID] = perH
	}
	return out
}

// RobustReversible is §4.5 key 3 under a hypothesis set:
//
//	reversible_robust(o) ⇔ ∀ h ∈ H_plausible : o.closed[h] = []
//
// The direction is not a matter of taste. Key 2 worst-cases the **price** of a
// closure (`min_h NetDelta(o | h)`), so a closure invisible at one reading but
// real at another is already charged at its worst. If this preference were read
// from a single reading — the observed one included — that same closure would be
// charged at its worst *and* rewarded as if it did not exist, and the robust
// ordering could be reversed by the very closure the worst case exists to weigh.
// So the preference is taken over the same set, in the same direction, as the
// price. With no hypothesis set this reduces to the flat `closed = []` of §4.5.
func (c *DOFCalculusCore) RobustReversible(option *ActionOption,
	members []*Hypothesis) bool {
	if len(members) == 0 {
		return c.IsReversible(option)
	}
	for _, h := range members {
		if !c.IsReversibleFor(option, h.ID) {
			return false
		}
	}
	return true
}

// barred is the §4.10.1 admissibility predicate of one reading's vector.
func barred(vector ConditionalVector) bool {
	return !vector.Viable || !vector.ResourcesOK ||
		vector.D1 > 0 || vector.D2 > 0 || vector.D3 > 0
}

// RobustAdmissible is §4.10.1: admissible under **every** plausible hypothesis.
//
// The predicate is both families of conditions — the temporal one of §4.8b, the
// financial one of §4.8 and the three dimensions of §4.5 — and the aggregation is
// a universally quantified conjunction. Neither is a maximum over hypotheses.
func (c *DOFCalculusCore) RobustAdmissible(perH map[string]ConditionalVector,
	members []*Hypothesis) bool {
	for _, h := range members {
		vector, ok := perH[h.ID]
		if !ok || barred(vector) {
			return false
		}
	}
	return true
}

// LeastFavourable is `NetDelta_robust(o) = min_{h ∈ H_plausible} NetDelta(o | h)`
// (§4.10.2). An empty set yields 0.0: staying put.
func (c *DOFCalculusCore) LeastFavourable(perH map[string]ConditionalVector,
	members []*Hypothesis) float64 {
	values := []float64{}
	for _, h := range members {
		if v, ok := perH[h.ID]; ok {
			values = append(values, v.NetDelta)
		}
	}
	if len(values) == 0 {
		return 0.0
	}
	least := values[0]
	for _, v := range values {
		if v < least {
			least = v
		}
	}
	return least
}

// AdmissibleUnder is §6.3: per reading, whether the option is admissible.
func (c *DOFCalculusCore) AdmissibleUnder(perH map[string]ConditionalVector,
	members []*Hypothesis) map[string]bool {
	out := map[string]bool{}
	for _, h := range members {
		vector, ok := perH[h.ID]
		out[h.ID] = ok && !barred(vector)
	}
	return out
}

// HypothesisConflict is §4.10.5: an unresolved conflict the report MUST surface.
//
// Fires when `H_plausible` has more than one element and either (a) the robust
// **candidate** set is empty, or (b) some candidate is admissible under some
// plausible hypotheses and inadmissible under others — stated over the **whole
// predicate**, so an option executable under one reading and physically
// impossible under another is as much a conflict as one that destroys a counted
// entity under one reading only.
//
// A difference that does **not** move admissibility — the same bar, of different
// magnitude, under different readings — is **not** a conflict: the choice is the
// same under both readings, the conditional vectors are listed anyway, and the
// least-favourable key already resolves it.
func (c *DOFCalculusCore) HypothesisConflict(
	perHAll map[string]map[string]ConditionalVector, members []*Hypothesis,
	robust []*ActionOption) bool {
	if len(members) <= 1 {
		return false
	}
	if len(robust) == 0 {
		return true
	}
	robustIDs := map[string]bool{}
	for _, o := range robust {
		robustIDs[o.OptionID] = true
	}
	for optionID, perH := range perHAll {
		flags := c.AdmissibleUnder(perH, members)
		anyAdmissible, anyBarred := false, false
		for _, v := range flags {
			if v {
				anyAdmissible = true
			} else {
				anyBarred = true
			}
		}
		if anyAdmissible && anyBarred {
			return true
		}
		if !robustIDs[optionID] && anyAdmissible {
			return true
		}
	}
	return false
}

// SelectConditional is §4.10: robust selection over the declared readings.
//
// The candidate set is the same for every hypothesis; only the projection
// differs. Among the robustly admissible candidates take the greatest
// `NetDelta_robust`, then apply §4.5 keys 3 and 4 (reversibility — read
// robustly — then the comparison origin), then the lexicographically smallest
// `option_id`.
//
// **There is no fallback to admissible support.** An empty robust candidate set
// yields `nil` — every proposed action is barred under at least one plausible
// reading, and ranking the survivors of an inadmissible set would be the
// compensation Axiom 3 forbids (§4.10.4).
func (c *DOFCalculusCore) SelectConditional(state *SystemStateMatrix,
	options []*ActionOption, members []*Hypothesis, ctx *ObservationContext,
	groups [][]string, rates map[string]RateInfo, weights map[string]float64,
	cap *float64) (*ActionOption, ConditionalSelection) {
	perHAll := c.ConditionalVectors(members, options, ctx, groups, rates, weights, cap)

	robust := []*ActionOption{}
	for _, option := range options {
		if c.RobustAdmissible(perHAll[option.OptionID], members) {
			robust = append(robust, option)
		}
	}

	selection := ConditionalSelection{
		ConditionalVectors: perHAll,
		AdmissibleUnder:    map[string]map[string]bool{},
		HypothesisConflict: c.HypothesisConflict(perHAll, members, robust),
		RobustAdmissible:   []string{},
		NetDeltaRobust:     map[string]float64{},
	}
	for _, option := range options {
		selection.AdmissibleUnder[option.OptionID] =
			c.AdmissibleUnder(perHAll[option.OptionID], members)
		selection.NetDeltaRobust[option.OptionID] =
			c.LeastFavourable(perHAll[option.OptionID], members)
	}
	for _, option := range robust {
		selection.RobustAdmissible = append(selection.RobustAdmissible, option.OptionID)
	}
	if len(robust) == 0 {
		return nil, selection
	}

	// §4.5 key 2: the greatest least-favourable delta, ties by tolerance.
	best := selection.NetDeltaRobust[robust[0].OptionID]
	for _, option := range robust {
		if v := selection.NetDeltaRobust[option.OptionID]; v > best {
			best = v
		}
	}
	survivors := []*ActionOption{}
	for _, option := range robust {
		if math.Abs(selection.NetDeltaRobust[option.OptionID]-best) <= netDeltaTolerance {
			survivors = append(survivors, option)
		}
	}

	// §4.5 key 3, under its **robust** reading (§4.10.2): revert only if some
	// survivor is reversible under every plausible reading.
	anyReversible := false
	for _, option := range survivors {
		if c.RobustReversible(option, members) {
			anyReversible = true
			break
		}
	}
	if anyReversible {
		kept := []*ActionOption{}
		for _, option := range survivors {
			if c.RobustReversible(option, members) {
				kept = append(kept, option)
			}
		}
		survivors = kept
	}

	// §4.5 key 4: the survivor must beat the comparison origin.
	kept := []*ActionOption{}
	for _, option := range survivors {
		if selection.NetDeltaRobust[option.OptionID] > 0.0 {
			kept = append(kept, option)
		}
	}
	survivors = kept
	if len(survivors) == 0 {
		return nil, selection
	}

	// §4.5 final key: the lexicographically smallest `option_id`.
	chosen := survivors[0]
	for _, option := range survivors[1:] {
		if option.OptionID < chosen.OptionID {
			chosen = option
		}
	}
	return chosen, selection
}

// HypothesisCoverage is §6.2: the declared coverage of a set, reported next to
// the decision it produced. An absent claim reads as `"partial"` — the cautious
// default — and is never inferred to be complete.
func HypothesisCoverage(hset *HypothesisSet) string {
	return CoverageOf(hset)
}
