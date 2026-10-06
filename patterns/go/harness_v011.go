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
		len(selection.RobustAdmissible) == 2, fmt.Sprintf("%v", selection.RobustAdmissible))
	check("the payload lists the robust key of every option",
		len(selection.NetDeltaRobust) == 2)

	// §4.10.4: no fallback to admissible support.
	chosenNone, noneSelection := core.SelectConditional(observed,
		[]*ActionOption{sink}, barMembers, ctx, nil, nil, nil, nil)
	check("an empty robust candidate set yields no action at all",
		chosenNone == nil, fmt.Sprintf("%v", chosenNone))
	check("…and the payload still reports what it refused",
		len(noneSelection.RobustAdmissible) == 0 && noneSelection.HypothesisConflict)

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

	// --- 7. §6.1/§6.2/§6.3: the report under a declared set -----------------
	fmt.Println("=== 7. §6.1/§6.2/§6.3: the audit report is per hypothesis ===")
	// The layer a port can compute but cannot publish is not landed: the
	// conditional vectors existed in this port's core and in this harness, and
	// nothing a consumer could call returned them. The report is the release's
	// output, so it is asserted here on the **report** and not on the core.
	//
	// The fixture is section 5's: one entity read at 0.25 and at 0.30, so the two
	// readings really differ, and an option that crosses into the zero under the
	// second one only.
	// The fixture: one entity read just above the zero under `h_bar` and well
	// above it under the observed reading, so an option that drains it is barred
	// under one reading and admissible under the other — the asymmetry §6.3 exists
	// to report rather than average away. The readings really differ, so the index
	// map has two different values.
	reportCandidates := []*ActionOption{strong, sink}
	reportSelected, reportSelection := core.SelectConditional(observed, reportCandidates,
		barMembers, ctx, nil, nil, nil, nil)
	var reportRuler *string
	if decl != nil {
		ruler := decl.RulerDigest()
		reportRuler = &ruler
	}
	report := core.Report(observed, reportCandidates, reportSelected, "FAST_PASS",
		ReportInput{
			Ctx:         ctx,
			Readings:    barMembers,
			Declared:    barMembers,
			Coverage:    "partial",
			RulerDigest: reportRuler,
			Selection:   &reportSelection,
		})

	check("§6.2: the scalar total names the reading it belongs to",
		report.TotalSystemDoFReading == ObservedHypothesisID,
		report.TotalSystemDoFReading)
	check("§6.2: the report names the declared set and the plausible readings",
		len(report.Hypotheses) == 2 && len(report.PlausibleHypotheses) == 2,
		fmt.Sprintf("%d declared, %d plausible",
			len(report.Hypotheses), len(report.PlausibleHypotheses)))
	check("§6.2: an undeclared coverage is reported as `partial`",
		report.HypothesisCoverage == "partial", report.HypothesisCoverage)
	check("§6.2: the index under a set is a map, one entry per reading",
		len(report.TotalSystemDoFByHypothesis) == 2 &&
			report.TotalSystemDoFByHypothesis[ObservedHypothesisID] !=
				report.TotalSystemDoFByHypothesis["h_bar"],
		fmt.Sprintf("%v", report.TotalSystemDoFByHypothesis))

	// Every candidate carries its whole vector per reading, computed from that
	// reading's own triple — the report is the place §6.3 says an asymmetry is a
	// result rather than an intermediate. The vectors are compared against the
	// core's own per-reading computation, so a report that drifted from the
	// decision it publishes is caught here and nowhere else.
	vecOK := true
	vecDetail := ""
	for _, row := range report.Options {
		if len(row.ConditionalVectors) != len(barMembers) {
			vecOK = false
			vecDetail = row.OptionID + ": " + fmt.Sprint(len(row.ConditionalVectors)) + " readings"
			break
		}
		option := optionByID(reportCandidates, row.OptionID)
		for _, h := range barMembers {
			direct := core.ConditionalVectorOf(h.State, option, ctx, h.ID, nil, nil, nil, nil)
			got := row.ConditionalVectors[h.ID]
			if got.D1 != direct.D1 || got.D2 != direct.D2 || got.D3 != direct.D3 ||
				math.Abs(got.NetDelta-direct.NetDelta) > netDeltaTolerance {
				vecOK = false
				vecDetail = row.OptionID + " under " + h.ID
			}
		}
	}
	check("§6.3: every candidate carries its conditional vector per reading", vecOK, vecDetail)

	// The barring condition must be visible **under the reading that barred it**:
	// a candidate that fails a condition of admissibility in one reading is barred
	// there, and that is what §4.10.5's flag names.
	barRow := findRow(report.Options, "sink")
	check("§6.3: a candidate barred under one reading is visible as barred there",
		barRow != nil && !barRow.AdmissibleUnder["h_bar"] &&
			len(barRow.ConditionalVectors) == 2,
		func() string {
			if barRow == nil {
				return "no row"
			}
			return fmt.Sprintf("%v", barRow.AdmissibleUnder)
		}())
	check("§6.3: the key that barred it is reported per reading",
		barRow != nil && barRow.BarringKeyByHypothesis["h_bar"] != nil,
		func() string {
			if barRow == nil {
				return "no row"
			}
			return fmt.Sprintf("%v", barRow.BarringKeyByHypothesis)
		}())
	check("§4.10.5: the split admissibility is surfaced as a conflict",
		report.HypothesisConflict, fmt.Sprintf("%v", report.HypothesisConflict))

	// §6.1: the reason a value is what it is belongs to a reading too — there is
	// no shared `ψ` to print once the readings differ.
	entRow := findEntityRow(report.Entities, entity)
	check("§6.1: the lens terms and the verdict are reported per reading",
		entRow != nil && len(entRow.LensTermsByHypothesis) == 2 &&
			len(entRow.RecoverabilityByHypothesis) == 2,
		func() string {
			if entRow == nil {
				return "no entity row"
			}
			return fmt.Sprintf("%d terms-rows, %d verdict-rows",
				len(entRow.LensTermsByHypothesis), len(entRow.RecoverabilityByHypothesis))
		}())

	// The layer must be reachable for a **consumer of the port**, not only for this
	// harness: the orchestrator's set entry publishes the per-reading report, while
	// its flat entry keeps publishing the flat one — the reduction of §4.10.6 read
	// from the outside.
	orchSelected, orchReport := orchObs.DecideOnSet(observed, &HypothesisSet{Members: members})
	_, flatOrchReport := orchObs.DecideOnSet(observed, nil)
	perReading := len(orchReport.Options) > 0
	for _, row := range orchReport.Options {
		if len(row.ConditionalVectors) != 2 {
			perReading = false
		}
	}
	check("§6.2/§6.3: the orchestrator publishes the per-reading report for a set",
		len(orchReport.Hypotheses) == 2 && perReading &&
			orchReport.TotalSystemDoFReading == ObservedHypothesisID &&
			orchReport.HypothesisCoverage == "partial",
		fmt.Sprintf("%d readings, %d options", len(orchReport.Hypotheses),
			len(orchReport.Options)))
	// §4.10.6: the set entry without a set is the observed-state **singleton** — one
	// reading, one entry in each map — and not the `v0.9.1`-shaped flat report of
	// `core.Report`, which carries no per-reading surface at all.
	check("§4.10.6: the set entry without a set publishes the observed singleton",
		len(flatOrchReport.Hypotheses) == 1 &&
			flatOrchReport.Hypotheses[0].ID == ObservedHypothesisID &&
			len(flatOrchReport.PlausibleHypotheses) == 1 &&
			len(flatOrchReport.TotalSystemDoFByHypothesis) == 1,
		fmt.Sprintf("%d readings, %d plausible", len(flatOrchReport.Hypotheses),
			len(flatOrchReport.PlausibleHypotheses)))
	// §4.10: the published decision is the one the report's own robust support
	// licenses. A report describing a decision other than the one made is the defect
	// the single conditional pass exists to prevent.
	check("§4.10: the published decision is robustly admissible in its own report",
		(orchSelected == nil && len(orchReport.RobustAdmissible) == 0) ||
			(orchSelected != nil && containsString(orchReport.RobustAdmissible,
				orchSelected.OptionID)),
		fmt.Sprintf("selected=%v robust=%v", orchSelected, orchReport.RobustAdmissible))
	// §3.6: a set whose observed reading is not the state it is evaluated on is
	// **refused**, not repaired — the entry point is where that obligation lives.
	func() {
		defer func() {
			rec := recover()
			check("§3.6: the entry point refuses a set whose observed reading is absent",
				rec != nil, fmt.Sprintf("%v", rec))
		}()
		orchObs.DecideOnSet(observed, &HypothesisSet{Members: barMembers})
	}()
	// §6.2/§6.3: the set-aware entry is reachable from a **bare scene** as well as from a
	// measured state — this is the call a v0.11 consumer makes, and without a set it must
	// publish the observed singleton rather than the empty surface the flat entry gives.
	// A layer only the harness can reach has not been landed.
	bareOrch := NewDOFOrchestrator(0.05)
	_, bareReport := bareOrch.StepWithReportOnSet(FixtureT1Scene(), nil)
	check("§4.10.6: the set entry on a bare scene publishes the observed singleton",
		len(bareReport.Hypotheses) == 1 &&
			bareReport.Hypotheses[0].ID == ObservedHypothesisID &&
			len(bareReport.PlausibleHypotheses) == 1 &&
			len(bareReport.TotalSystemDoFByHypothesis) == 1 &&
			bareReport.HypothesisCoverage == "partial" &&
			len(bareReport.ConditionalVectors) > 0,
		fmt.Sprintf("%d readings, %d options", len(bareReport.Hypotheses),
			len(bareReport.Options)))

	// §4.10.6: a cycle with **no observation context** still decides and reports. A
	// scene without a graph is decidable — the §4.9 verdicts read `undetermined` and
	// are priced by `u(t)` — so an absent context changes what the quantities are,
	// never whether the calculus answers; and the refusal the entry publishes must be
	// the arithmetic's refusal: the robustly admissible set it reports must equal the
	// set its **own published vectors** license. A fresh orchestrator has no context
	// until it polls, which is exactly the case this asserts.
	nocOrch := NewDOFOrchestrator(0.05)
	nocSel, nocReport := nocOrch.DecideOnSet(observed, nil)
	nocSingleton := ResolvedMembers(observed, nil)
	nocSurface := len(nocReport.Hypotheses) == 1 && len(nocReport.Options) > 0
	licensed := []string{}
	for _, row := range nocReport.Options {
		if len(row.ConditionalVectors) != 1 {
			nocSurface = false
			continue
		}
		if _, ok := row.ConditionalVectors[ObservedHypothesisID]; !ok {
			nocSurface = false
			continue
		}
		if nocOrch.core.RobustAdmissible(row.ConditionalVectors, nocSingleton) {
			licensed = append(licensed, row.OptionID)
		}
	}
	decidedOK := (nocSel == nil) == (len(licensed) == 0) &&
		(nocSel == nil || containsString(licensed, nocSel.OptionID))
	check("§4.10.6: a cycle with no observation context still decides and reports",
		nocSurface && sameStringSet(nocReport.RobustAdmissible, licensed) && decidedOK,
		fmt.Sprintf("surface=%v, published %d vs licensed %d, decided=%v",
			nocSurface, len(nocReport.RobustAdmissible), len(licensed), nocSel != nil))

	// §4.10.6: with no declared set the observed state alone is the answer, and the
	// per-reading surface is **absent** rather than a one-entry map.
	flatReport := core.Report(observed, reportCandidates, reportSelected, "FAST_PASS",
		ReportInput{Ctx: ctx})
	check("§4.10.6: without a declared set the flat report carries no per-reading surface",
		len(flatReport.Hypotheses) == 0 && len(flatReport.TotalSystemDoFByHypothesis) == 0 &&
			flatReport.Options[0].ConditionalVectors == nil,
		fmt.Sprintf("%d hypotheses, %d totals",
			len(flatReport.Hypotheses), len(flatReport.TotalSystemDoFByHypothesis)))
	fmt.Println()
}

// sameStringSet reports whether two id lists name the same set (order-free).
func sameStringSet(a []string, b []string) bool {
	if len(a) != len(b) {
		return false
	}
	seen := map[string]bool{}
	for _, item := range a {
		seen[item] = true
	}
	for _, item := range b {
		if !seen[item] {
			return false
		}
	}
	return true
}

// containsString reports whether the list holds this id.
func containsString(list []string, id string) bool {
	for _, item := range list {
		if item == id {
			return true
		}
	}
	return false
}

// findRow returns the option row with this id, or nil.
func findRow(rows []OptionReportRow, optionID string) *OptionReportRow {
	for i := range rows {
		if rows[i].OptionID == optionID {
			return &rows[i]
		}
	}
	return nil
}

// optionByID returns the candidate with this id, or nil.
func optionByID(options []*ActionOption, optionID string) *ActionOption {
	for _, option := range options {
		if option.OptionID == optionID {
			return option
		}
	}
	return nil
}

// findEntityRow returns the entity row with this id, or nil.
func findEntityRow(rows []EntityReportRow, entityID string) *EntityReportRow {
	for i := range rows {
		if rows[i].EntityID == entityID {
			return &rows[i]
		}
	}
	return nil
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
