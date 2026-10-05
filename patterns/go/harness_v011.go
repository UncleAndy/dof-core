package main

// Conformance harness of the Go port, DOF-SPEC v0.11 (§3.3, §3.4.2, §3.6, §4.4,
// §4.8b, §4.10).
//
// This is the release's own harness for the conditional layer. `v07`, `v08` and
// `v091` are left untouched: they are the evidence of their releases, and their
// fingerprints are asserted here again as the precondition of everything below —
// a moved digest is an error to be fixed, not a new version.
//
// Sections, in order: one ruler, two readings; the hypothesis set and its five
// validation rules; absence and emptiness reduce to the observed singleton; the
// two forms of §3.3/§4.4 and the refusal to mix them; the temporal condition of
// §4.8b; the robust selection of §4.10.

import (
	"fmt"
	"math"
)

// v07RulerDigestV011 is the **ruler-level** digest of the released v0.7 fixture:
// the same declaration with `entities`, `freeze` and `verdicts` excluded
// (§3.4.2/§3.4.3). It is the value the Python port's `ruler_digest()` produces
// for that fixture, byte for byte, and it is the comparability key every release
// must keep — which is a different claim from the declaration digest above, and
// is asserted separately.
const v07RulerDigestV011 = "81948b8b3ca9805a75624c4d136f5926b9347c42ed3c6415966fc6f43f4301ba"

func runHarnessV011() {
	// --- 0. the released evidence still stands ------------------------------
	// The released fixture's own fingerprints, asserted on the released fixture
	// (`FixtureScene`), exactly as `harness_v08.go` asserts them: `FixtureT1Scene`
	// is a different scene and its digests are legitimately its own.
	orchRel := NewDOFOrchestrator(0.05)
	orchRel.mapper.PollEnvironment(FixtureScene(FixtureOptions{}))
	relDecl := orchRel.mapper.LastDeclaration
	relCtx := orchRel.mapper.LastObservation
	check("the v0.7 declaration digest is still reproduced",
		relDecl.Digest() == expectedRulerDigest, relDecl.Digest()[:16])
	check("the released fixture still carries the v0.7 observation digest",
		relCtx.ObservationDigest == expectedObservationDigest,
		relCtx.ObservationDigest[:16])
	check("the v0.7 ruler digest is still reproduced (the exclusion rule did not move)",
		relDecl.RulerDigest() == v07RulerDigestV011, relDecl.RulerDigest()[:16])

	orchObs := NewDOFOrchestrator(0.05)
	state := orchObs.mapper.PollEnvironment(FixtureT1Scene())
	ctx := orchObs.mapper.LastObservation
	decl := orchObs.mapper.LastDeclaration
	core := orchObs.core

	fmt.Println("=== 0. §3.4.2/§4.10: one ruler, two readings ===")

	// The second reading is a **transformation of one observed state**, mapped
	// through its own orchestrator so that its declaration, its ruler and its graph
	// come from the same named procedures as the observed one.
	orchAlt := NewDOFOrchestrator(0.05)
	stateB := v011Reconcile(orchAlt.mapper.PollEnvironment(
		v011ScaledScene(FixtureT1Scene(), 1.25)))
	ctxB := orchAlt.mapper.LastObservation
	declB := orchAlt.mapper.LastDeclaration

	check("the ruler is shared across the release: v0.11 ruler == v0.7 ruler",
		decl.RulerDigest() == relDecl.RulerDigest(),
		fmt.Sprintf("%s vs %s", decl.RulerDigest()[:16], relDecl.RulerDigest()[:16]))
	check("two readings of one cycle share the ruler digest byte for byte",
		decl.RulerDigest() == declB.RulerDigest(),
		fmt.Sprintf("%s vs %s", decl.RulerDigest()[:16], declB.RulerDigest()[:16]))
	check("…while their full declaration digests differ",
		decl.Digest() != declB.Digest(),
		fmt.Sprintf("%s vs %s", decl.Digest()[:16], declB.Digest()[:16]))
	check("…and the observation digest is **shared** — §3.5's `G` is not branched",
		ctx.ObservationDigest == ctxB.ObservationDigest,
		fmt.Sprintf("%s vs %s", ctx.ObservationDigest[:16], ctxB.ObservationDigest[:16]))

	hObs := &Hypothesis{ID: ObservedHypothesisID, Plausible: true, State: state,
		Basis: "the observed reading"}
	hAlt := &Hypothesis{ID: "h_alt", Plausible: true, State: stateB,
		Basis: "declared Variety counter scaled by 1.25"}
	fmt.Println()

	// --- 1. §3.6: the hypothesis set and its validation rules ---------------
	fmt.Println("=== 1. §3.6: five validation rules, each on its own defect ===")
	wellFormed := ValidateSet(state, []*Hypothesis{hObs, hAlt})
	check("a well-formed two-member set is admissible", len(wellFormed) == 0,
		fmt.Sprintf("%v", wellFormed))

	dup := []*Hypothesis{hObs, {ID: ObservedHypothesisID, Plausible: true, State: stateB}}
	check("duplicate ids are refused",
		hasSubstring(ValidateSet(state, dup), "ids are not unique"))

	entity := v011Entity(state, 0)
	omitted := v011WithoutEntity(stateB, entity)
	check("a reading that omits an entity of S is refused",
		hasSubstring(ValidateSet(state, []*Hypothesis{hObs,
			{ID: "h_omit", Plausible: true, State: omitted}}), "omits entities"))

	extra := v011WithExtraEntity(stateB)
	check("a reading that declares an entity not in S is refused",
		hasSubstring(ValidateSet(state, []*Hypothesis{hObs,
			{ID: "h_extra", Plausible: true, State: extra}}),
			"declares entities not in the observed state"))

	broken := v011BreakLensProduct(stateB, entity)
	check("a stated DoF that its own counters do not produce is refused",
		hasSubstring(ValidateSet(state, []*Hypothesis{hObs,
			{ID: "h_broken", Plausible: true, State: broken}}),
			"differs from the product of its own lens values"))

	moved := v011MoveCollapseLabel(stateB, entity)
	check("moving the collapse-source label between readings is refused",
		hasSubstring(ValidateSet(state, []*Hypothesis{hObs,
			{ID: "h_moved", Plausible: true, State: moved}}),
			"is_collapse_source differs"))

	durations := v011ChangeDurations(stateB)
	check("measurement durations are ruler-level and may not vary per reading",
		hasSubstring(ValidateSet(state, []*Hypothesis{hObs,
			{ID: "h_dur", Plausible: true, State: durations}}),
			"durations are ruler-level"))

	check("the observed state must be present in H",
		hasSubstring(ValidateSet(state, []*Hypothesis{hAlt}),
			"the observed state is absent"))

	allImplausible := []*Hypothesis{
		{ID: ObservedHypothesisID, Plausible: false, State: state},
		{ID: "h_alt", Plausible: false, State: stateB},
	}
	check("a set that marks every reading implausible is refused",
		hasSubstring(ValidateSet(state, allImplausible), "H_plausible would be empty"))
	fmt.Println()

	// --- 2. §3.6/§4.10.6: absence and emptiness -----------------------------
	fmt.Println("=== 2. §3.6: absence and emptiness are the observed singleton ===")
	singleton := ResolvedMembers(state, nil)
	check("an absent set resolves to exactly one member", len(singleton) == 1)
	check("…whose id is the observed one", singleton[0].ID == ObservedHypothesisID)
	check("…and which is plausible", singleton[0].Plausible)
	check("an empty set resolves the same way",
		len(ResolvedMembers(state, &HypothesisSet{Members: []*Hypothesis{}})) == 1)
	check("an absent coverage claim reads as `partial`, never `complete`",
		CoverageOf(nil) == "partial" && CoverageOf(&HypothesisSet{}) == "partial")
	check("a declared coverage claim is reported as declared",
		HypothesisCoverage(&HypothesisSet{Coverage: "complete"}) == "complete")
	fmt.Println()

	// --- 3. §3.3/§4.4: the two forms, and no mixing ------------------------
	fmt.Println("=== 3. §3.3/§4.4: the two forms of an option ===")
	flat := v011Option("flat_opt", entity, 0.01)
	check("a single map of deltas is the flat form", flat.ProjectionForm() == "flat")
	check("an empty closure list is the flat form", flat.ClosureForm() == "flat")
	check("a flat option is well formed", flat.FormsConsistent() == "")

	perH := v011Option("per_h_opt", entity, 0.01)
	perH.ProjectedDoFDelta = nil
	perH.ProjectedByHypothesis = map[string]map[string]float64{
		ObservedHypothesisID: {entity: 0.01},
		"h_alt":              {entity: 0.02},
	}
	check("a map keyed by reading is the per-hypothesis form",
		perH.ProjectionForm() == "per_hypothesis")
	check("…and the delta read under a reading is that reading's",
		perH.DeltaFor(ObservedHypothesisID, entity) == 0.01 &&
			perH.DeltaFor("h_alt", entity) == 0.02)
	check("…with entities unlisted for a reading taking 0.0",
		perH.DeltaFor("h_alt", "nobody") == 0.0)

	mixed := v011Option("mixed_opt", entity, 0.01)
	mixed.ProjectedByHypothesis = map[string]map[string]float64{
		ObservedHypothesisID: {entity: 0.02}}
	check("mixing both forms in the projection is `invalid`",
		mixed.ProjectionForm() == "invalid")
	check("…and the option says so", mixed.FormsConsistent() != "")

	mixedClosure := v011Option("mixed_closed_opt", entity, 0.01)
	mixedClosure.Closed = []ClosedRef{{Kind: "mean", ID: "radio"}}
	mixedClosure.ClosedByHypothesis = map[string][]ClosedRef{
		ObservedHypothesisID: {{Kind: "mean", ID: "radio"}}}
	check("mixing both forms in the closure list is `invalid`",
		mixedClosure.ClosureForm() == "invalid")
	check("…and the option says so too", mixedClosure.FormsConsistent() != "")
	fmt.Println()

	// --- 4. §4.8b: the temporal condition ----------------------------------
	fmt.Println("=== 4. §4.8b: the temporal condition, as a condition ===")
	tau := TauOf(state)
	check("the released fixture carries a measured τ", tau != nil)

	shortAct := v011Option("short_act", entity, 0.01)
	check("an act within τ is viable", core.Viability(state, shortAct).Viable)

	longAct := v011Option("long_act", entity, 0.01)
	longAct.EstimatedDurationMks = *tau * 2.0
	check("an act that cannot complete within τ is not viable",
		!core.Viability(state, longAct).Viable)

	noTau := v011UnknownTau(state)
	check("an unknown τ is not a licence for an ordinary act",
		!core.Viability(noTau, shortAct).Viable)
	check("…and the reason names the missing measurement (§4.8b)",
		hasSubstring([]string{core.Viability(noTau, shortAct).Reason}, "unmeasured"))

	measure := v011Option("measure_tau", entity, 0.01)
	measure.Discovers = []string{"tau"}
	measure.EstimatedDurationMks = 1000.0
	measure.ProjectedTauValue = ptr(*tau + 1.0e6)
	check("a τ measurement is viable under an unknown τ when it can complete",
		core.Viability(noTau, measure).Viable)
	check("…and viable under a measured τ as well",
		core.Viability(state, measure).Viable)

	deadMeasure := v011Option("dead_measure", entity, 0.01)
	deadMeasure.Discovers = []string{"tau"}
	deadMeasure.EstimatedDurationMks = 1000.0
	deadMeasure.ProjectedTauValue = ptr(-1.0)
	check("a τ measurement whose own declared result is dead is not viable",
		!core.Viability(state, deadMeasure).Viable)
	check("…and the retired disjunction `τ = null` OR `τ >= t_m` would have admitted it",
		!core.Viability(noTau, deadMeasure).Viable)

	derived := core.DerivedTauDelta(state, measure)
	expected := *measure.ProjectedTauValue - (*tau - measure.EstimatedDurationMks)
	check("`projected_tau_delta` is derived for a τ measurement",
		derived != nil && math.Abs(*derived-expected) < 1e-6,
		fmt.Sprintf("%v vs %v", derived, expected))
	check("…and is null, not zero, when τ is unknown",
		core.DerivedTauDelta(noTau, measure) == nil)
	fmt.Println()

	// --- 5. §4.10: the robust selection ------------------------------------
	fmt.Println("=== 5. §4.10: robust selection over the declared readings ===")
	// The arithmetic needs room: `coerceDoF` clamps an entity's DoF into [0,1] and
	// the released fixture sits at the top of that interval, where a positive
	// projection cannot move anything. The observed reading of this section is the
	// same fixture with one entity's declared Variety counter scaled down — still a
	// state §4.1 accepts, since its stated DoF is the product of its own counters.
	observed := v011SetEntityDoF(state, entity, 0.25)
	other := v011SetEntityDoF(stateB, entity, 0.30)
	hObsW := &Hypothesis{ID: ObservedHypothesisID, Plausible: true, State: observed}
	hAltW := &Hypothesis{ID: "h_alt", Plausible: true, State: other}
	members := []*Hypothesis{hObsW, hAltW}
	strong := v011Option("strong", entity, 0.5)
	weak := v011Option("weak", entity, 0.1)
	candidates := []*ActionOption{strong, weak}

	check("the readings of this section are §4.1-consistent",
		len(ValidateSet(observed, members)) == 0,
		fmt.Sprintf("%v", ValidateSet(observed, members)))

	perHAll := core.ConditionalVectors(members, candidates, ctx, nil, nil, nil, nil)
	check("the conditional vectors are produced per option and per reading",
		len(perHAll) == 2 && len(perHAll["strong"]) == 2)
	check("…and they are not all alike (the readings really differ)",
		perHAll["strong"][ObservedHypothesisID].NetDelta !=
			perHAll["strong"]["h_alt"].NetDelta,
		fmt.Sprintf("%.6f vs %.6f",
			perHAll["strong"][ObservedHypothesisID].NetDelta,
			perHAll["strong"]["h_alt"].NetDelta))

	leastStrong := core.LeastFavourable(perHAll["strong"], members)
	greatestStrong := math.Max(perHAll["strong"][ObservedHypothesisID].NetDelta,
		perHAll["strong"]["h_alt"].NetDelta)
	check("the ordering key is the least-favourable delta, not the greatest",
		leastStrong == math.Min(perHAll["strong"][ObservedHypothesisID].NetDelta,
			perHAll["strong"]["h_alt"].NetDelta) && leastStrong < greatestStrong,
		fmt.Sprintf("min=%.6f max=%.6f", leastStrong, greatestStrong))

	// A reading under which one candidate crosses into the zero: the entity sits
	// just above it, and the option's negative projection drives it in. Under the
	// observed reading the same option leaves the entity positive.
	lows := v011SetEntityDoF(stateB, entity, 0.02)
	hBar := &Hypothesis{ID: "h_bar", Plausible: true, State: lows,
		Basis: "one entity sits just above the zero"}
	barMembers := []*Hypothesis{hObs, hBar}
	sink := v011Option("sink", entity, -0.05)
	perHBar := core.ConditionalVectors(barMembers, []*ActionOption{sink}, ctx, nil, nil, nil, nil)
	check("an option that crosses into the zero under one reading is charged there (D1 > 0)",
		perHBar["sink"]["h_bar"].D1 > 0,
		fmt.Sprintf("d1=%d under the bar, d1=%d under the observed reading",
			perHBar["sink"]["h_bar"].D1, perHBar["sink"][ObservedHypothesisID].D1))
	check("…and is not charged under the observed reading",
		perHBar["sink"][ObservedHypothesisID].D1 == 0)
	check("a candidate barred under one plausible reading is not robustly admissible",
		!core.RobustAdmissible(perHBar["sink"], barMembers))

	split := core.AdmissibleUnder(perHBar["sink"], barMembers)
	check("the per-reading admissibility is reported per reading",
		split[ObservedHypothesisID] != split["h_bar"], fmt.Sprintf("%v", split))
	check("and the split is surfaced as a conflict (§4.10.5)",
		core.HypothesisConflict(perHBar, barMembers, []*ActionOption{}))
	check("a conflict is not reported for a single-member set",
		!core.HypothesisConflict(perHBar, []*Hypothesis{hObs}, []*ActionOption{sink}))

	chosen, selection := core.SelectConditional(observed, candidates, members, ctx, nil, nil, nil, nil)
	check("among robustly admissible candidates the greatest robust delta wins",
		chosen != nil && chosen.OptionID == "strong", fmt.Sprintf("%v", chosen))
	check("the payload lists the robust candidates",
		len(selection.RobustCandidates) == 2, fmt.Sprintf("%v", selection.RobustCandidates))
	check("the payload lists the robust key of every option",
		len(selection.NetDeltaRobust) == 2)

	// §4.10.4: no fallback to admissible support.
	chosenNone, noneSelection := core.SelectConditional(observed,
		[]*ActionOption{sink}, barMembers, ctx, nil, nil, nil, nil)
	check("an empty robust candidate set yields no action at all",
		chosenNone == nil, fmt.Sprintf("%v", chosenNone))
	check("…and the payload still reports what it refused",
		len(noneSelection.RobustCandidates) == 0 && noneSelection.HypothesisConflict)

	// §4.5 key 3 under its robust reading: an option that closes nothing under the
	// observed reading but closes something under another reading must NOT collect
	// the preference — key 2 has already charged that closure at its worst.
	closer := v011Option("closer", entity, 0.02)
	closer.ProjectedDoFDelta = nil
	closer.ProjectedByHypothesis = map[string]map[string]float64{
		ObservedHypothesisID: {entity: 0.02},
		"h_alt":              {entity: 0.02},
	}
	closer.ClosedByHypothesis = map[string][]ClosedRef{
		ObservedHypothesisID: {},
		"h_alt":              {{Kind: "mean", ID: "radio"}},
	}
	check("a per-hypothesis closure is read per reading",
		core.IsReversibleFor(closer, ObservedHypothesisID) &&
			!core.IsReversibleFor(closer, "h_alt"))
	check("…so the observed reading alone would call it reversible",
		core.IsReversible(closer))
	check("…but the robust reading does not (§4.5 key 3, §4.10.2)",
		!core.RobustReversible(closer, members))
	check("…and the decision prefers the one that closes nothing under every reading",
		selectsReversible(core, observed, ctx, members))
	check("with no hypothesis set the robust reading is the flat one (§4.5)",
		core.RobustReversible(closer, []*Hypothesis{hObs}))

	// §4.4 guards range over every reading's closure list.
	guarded := v011Option("guarded", entity, 0.01)
	guarded.ActID = "act_x"
	guarded.ClosedByHypothesis = map[string][]ClosedRef{
		ObservedHypothesisID: {},
		"h_alt":              {{Kind: "act", ID: "act_x"}},
	}
	check("the self-closure guard (§4.4 guard 1) catches a per-hypothesis self-closure",
		core.validateClosure(guarded) != nil)
	fmt.Println()

	// --- 6. §6.3: the after-state of a reading is its own -------------------
	fmt.Println("=== 6. §6.3: a reading's after-state is its own ===")
	// `D2`/`D3` are recomputed on **that reading's** after-state — the pruned graph
	// **and** the counters. An option that closes only under `h_alt` must be charged
	// there. A port that prunes the graph with `closure[h]` but recomputes the
	// counters from `closure[$observed$]` builds a state no hypothesis produces, and
	// this fixture separates the two: trainee's own mean drives its counter to zero
	// while the supervise mean removes the path that would raise it back.
	closerH := v011HOnlyCloser()
	rowsH := core.lostPathsFor(state, closerH, ctx, "h_alt")
	check("§6.3: a reading's after-state is built from that reading's own closures",
		len(rowsH) == 1 && rowsH[0].EntityID == "trainee" &&
			rowsH[0].VerdictBefore == "reachable" &&
			rowsH[0].VerdictAfter == "proven_unreachable",
		fmt.Sprintf("%d rows", len(rowsH)))
	check("…and the observed reading of the same option charges nothing",
		len(core.lostPaths(state, closerH, ctx)) == 0)

	// The form is a declaration style, not a semantics: two options declaring the
	// same closure list — one flat, one per-hypothesis — must produce the same lost
	// paths under the same reading. This is the invariance the mixed triple breaks.
	flatCloser := t1Mirror()
	flatCloser.OptionID = "flat_closer"
	flatCloser.ProjectedDoFDelta = nil
	flatCloser.ProjectedByHypothesis = map[string]map[string]float64{
		ObservedHypothesisID: {"drone": 0.1},
		"h_alt":              {"drone": 0.1},
	}
	flatCloser.Closed = closures(TraineeMeanID, SuperviseMeanID)
	flatCloser.ClosedByHypothesis = nil
	// The per-hypothesis option declares the same list **under `h_alt`**; the flat
	// option declares it under every reading, so under `h_alt` the two are the same
	// declaration in two forms, and the answer must not depend on which form was used.
	perHCloser := v011HOnlyCloser()
	rowsFlat := core.lostPathsFor(state, flatCloser, ctx, "h_alt")
	rowsPerH := core.lostPathsFor(state, perHCloser, ctx, "h_alt")
	check("§3.3/§6.3: the per-hypothesis form agrees with the flat form on the same list",
		sameLostRows(rowsFlat, rowsPerH),
		fmt.Sprintf("%d vs %d rows", len(rowsFlat), len(rowsPerH)))
	fmt.Println()
}

// sameLostRows compares two lost-path reports on the facts that carry meaning: the
// entity and the two verdicts. Order is the fixture's sorted iteration order in
// every port, so it is compared positionally.
func sameLostRows(a, b []LostPathEntry) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i].EntityID != b[i].EntityID || a[i].VerdictBefore != b[i].VerdictBefore ||
			a[i].VerdictAfter != b[i].VerdictAfter {
			return false
		}
	}
	return true
}

// selectsReversible builds the §4.10.2 counterexample the hard way: two options
// with **identical** robust deltas, one of which declares a closure under the
// second reading only. The closure names a mean that exists nowhere in the graph,
// so it costs nothing in key 2 — no charge, no lost path — and the decision is
// made by key 3 alone.
//
// The ids are chosen so the check discriminates: the closing option sorts FIRST.
// Reading key 3 from the observed reading alone would call it reversible, keep it
// as a survivor and elect it lexicographically; the robust reading drops it.
func selectsReversible(core *DOFCalculusCore, state *SystemStateMatrix,
	ctx *ObservationContext, members []*Hypothesis) bool {
	entity := v011Entity(state, 0)
	closing := v011Option("a_closing", entity, 0.5)
	closing.ProjectedDoFDelta = nil
	closing.ProjectedByHypothesis = map[string]map[string]float64{
		ObservedHypothesisID: {entity: 0.5},
		"h_alt":              {entity: 0.5},
	}
	closing.ClosedByHypothesis = map[string][]ClosedRef{
		ObservedHypothesisID: {},
		"h_alt":              {{Kind: "mean", ID: "v011_absent_mean"}},
	}
	open := v011Option("b_open", entity, 0.5)
	chosen, selection := core.SelectConditional(state, []*ActionOption{closing, open},
		members, ctx, nil, nil, nil, nil)
	if math.Abs(selection.NetDeltaRobust["a_closing"]-
		selection.NetDeltaRobust["b_open"]) > netDeltaTolerance {
		return false // key 2 separated them: this is not the key-3 counterexample
	}
	return chosen != nil && chosen.OptionID == "b_open"
}
