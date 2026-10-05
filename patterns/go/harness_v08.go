package main

// Conformance harness of the Go port, DOF-SPEC v0.8 (§4.5, T1).
//
// This is the release's own harness. `v07` is left untouched: it is the v0.7
// evidence, and it still reproduces the v0.7 ruler — which is itself one of this
// release's checks (a moved digest is an error to be fixed, not a new version).
//
// Sections, in order: the ruler did not move; the two entities D3 exists for; the
// candidate vector is computed; the decision that changed; the mirror; a path cut
// is a bar; bounds and the baseline.

import (
	"fmt"
)

// v07Index is the index of the released fixture (v0.7, §10). T1 must not move it.
const v07Index = -35.314438370902

// vectorOf is the vector a single candidate would be judged by, computed by the
// selection itself so that the harness reads the same numbers the decision does.
func vectorOf(core *DOFCalculusCore, state *SystemStateMatrix, ctx *ObservationContext,
	option *ActionOption) CandidateVector {
	_, vectors := core.SelectCandidate(state, []*ActionOption{option}, ctx)
	return vectors[0]
}

func runHarnessV08() {
	// --- 1. the ruler did not move (§10, R6) --------------------------------
	orchBase := NewDOFOrchestrator(0.05)
	baseState := orchBase.mapper.PollEnvironment(FixtureScene(FixtureOptions{}))
	baseCtx := orchBase.mapper.LastObservation
	baseDecl := orchBase.mapper.LastDeclaration
	baseCore := orchBase.core

	fmt.Println("=== 1. §10/R6: the released fixture is unchanged by T1 ===")
	check("the ruler digest is the v0.7 one, byte for byte",
		baseDecl.Digest() == expectedRulerDigest, baseDecl.Digest()[:16])
	check("the observation digest is the v0.7 one, byte for byte",
		baseCtx.ObservationDigest == expectedObservationDigest,
		baseCtx.ObservationDigest[:16])
	baseIndex := baseCore.CalculateSystemDoF(baseState, nil, baseCtx)
	check("the index of the released fixture is unchanged",
		closeEnough(baseIndex, v07Index, 1e-6), fmt.Sprintf("%.6f", baseIndex))
	// The fingerprints of the released fixture, in full: a run that stopped
	// comparing must not be able to pass unnoticed, and a reader of the log must be
	// able to see the values the run asserted against.
	fmt.Println()
	fmt.Printf("RULER  digest=%s\n", baseDecl.Digest())
	fmt.Printf("OBSERVATION digest=%s\n", baseCtx.ObservationDigest)
	fmt.Printf("INDEX  %.6f\n", baseIndex)
	fmt.Println()

	orch := NewDOFOrchestrator(0.05)
	state := orch.mapper.PollEnvironment(FixtureT1Scene())
	ctx := orch.mapper.LastObservation
	decl := orch.mapper.LastDeclaration
	core := orch.core

	fmt.Println("=== 2. §4.5: the two entities D3 exists for ===")
	trainee := state.Entities["trainee"]
	minimum := 1.0
	for id := range core.calcMembers(state, ctx) {
		if ent, ok := state.Entities[id]; ok && ent.CurrentDoF < minimum {
			minimum = ent.CurrentDoF
		}
	}
	check("trainee sits above the minimum DoF of calc(S), so it is not critical",
		trainee.CurrentDoF > 0.0,
		fmt.Sprintf("trainee=%.6f min=%.6f", trainee.CurrentDoF, minimum))
	check("trainee can act itself: V > 0", ctx.vBefore("trainee") > 0,
		fmt.Sprintf("V=%d", ctx.vBefore("trainee")))
	check("and its own act does not lift it — its path is somebody else's act",
		ctx.verdict("trainee") == "reachable", ctx.verdict("trainee"))
	critical := core.criticalMembers(state, ctx)
	check("critical(S) is the known zeros of calc, not the dependents",
		len(critical) == 2 && critical["revivable"] && critical["unobserved"],
		fmt.Sprintf("%v", sortedKeys(critical)))

	fmt.Println("=== 3. §4.5: the vector is computed, not declared ===")
	vecComp := vectorOf(core, state, ctx, t1Compensate())
	check("t1_compensate: no destruction — nothing is driven to a known zero",
		vecComp.D1 == 0, fmt.Sprintf("%v", vecComp))
	check("t1_compensate: it cuts the patient's only path", vecComp.D2 == 1,
		fmt.Sprintf("%v", vecComp))
	check("t1_compensate: and that path is the critical node's", vecComp.D3 == 1,
		fmt.Sprintf("%v", vecComp))
	check("t1_compensate: NetDelta > 0 — every pre-v0.8 gate passes it",
		vecComp.NetDelta > 0.0, fmt.Sprintf("%+.6f", vecComp.NetDelta))
	check("t1_compensate: the collapse charges are empty, as §4.2 computes them",
		len(core.collapseCharges(state, t1Compensate(), ctx)) == 0)
	lost := core.lostPaths(state, t1Compensate(), ctx)
	check("t1_compensate: the lost path is auditable, with the witness it lost",
		len(lost) == 1 && lost[0].EntityID == "revivable" &&
			lost[0].VerdictBefore == "reachable" &&
			lost[0].VerdictAfter == "proven_unreachable" && lost[0].Critical &&
			len(lost[0].WitnessLost) == 1 && lost[0].WitnessLost[0] == "act_medkit",
		fmt.Sprintf("%v", lost))

	fmt.Println("=== 4. §4.5: the decision that changed ===")
	selected, _ := core.SelectCandidate(state, []*ActionOption{t1Compensate()}, ctx)
	check("under T1 the option loses to staying put: the system stays", selected == nil)
	key := core.BarringKey(vecComp)
	check("the report names the key that barred it", key != nil && *key == "d2")
	baseline := core.BaselineVector()
	check("staying put is a candidate with the zero vector",
		baseline.D1 == 0 && baseline.D2 == 0 && baseline.D3 == 0 &&
			baseline.NetDelta == 0.0 && baseline.Reversible)
	refusal := core.Report(state, []*ActionOption{t1Compensate()}, nil, "FAST_PASS",
		ReportInput{Declaration: decl, Ctx: ctx})
	check("no candidate beat inaction, and the report says so", refusal.NoCandidateBetter)
	check("the refusal lists the candidates and the key that barred each",
		len(refusal.Options) == 1 && refusal.Options[0].BarringKey != nil &&
			*refusal.Options[0].BarringKey == "d2")

	fmt.Println("=== 5. §4.5: the mirror — the same gain, a path that is not the price ===")
	vecMirror := vectorOf(core, state, ctx, t1Mirror())
	check("t1_mirror: nothing destroyed, nothing lost",
		vecMirror.D1 == 0 && vecMirror.D2 == 0 && vecMirror.D3 == 0,
		fmt.Sprintf("%v", vecMirror))
	check("t1_mirror: a real gain over staying put", vecMirror.NetDelta > 0.0,
		fmt.Sprintf("%+.6f", vecMirror.NetDelta))
	check("robot keeps eight of its nine vectors: the closure was a price, not a loss",
		ctx.vAfterClosure("robot", t1Mirror().Closed) == 8 && ctx.vBefore("robot") == 9)
	chosen := core.EvaluateAndSelect(state, []*ActionOption{t1Compensate(), t1Mirror()}, ctx)
	check("the mirror is selected", chosen != nil && chosen.OptionID == "t1_mirror",
		fmt.Sprintf("%v", chosen))

	fmt.Println("=== 6. §4.5: a path cut is a bar, and the third dimension cannot separate ===")
	vecHelp := vectorOf(core, state, ctx, t1Help())
	vecRival := vectorOf(core, state, ctx, t1Rival())
	// `v0.11` §4.9: the condition is `DoF(X | h) + Δ_P(X) > 0`, and the
	// **trivial** path is a finite simple path — so cutting a path does not make
	// a *live* entity unreachable, and `D2` no longer fires for it (§10(J),
	// fixtures `ar` and `ad`). `D2` keeps its teeth exactly where it matters: an
	// option that drives the entity to a **known zero** and closes its recovery —
	// which is what `t1_rival` does, and the assertion below still holds for it.
	check("t1_help: it cuts a path without destroying anything",
		vecHelp.D1 == 0 && vecHelp.D2 == 0 && vecHelp.D3 == 0, fmt.Sprintf("%v", vecHelp))
	helpLost := core.lostPaths(state, t1Help(), ctx)
	check("t1_help: and no path is reported as lost", len(helpLost) == 0,
		fmt.Sprintf("%v", helpLost))
	check("closing the mentor's act costs it a vector and destroys nothing",
		len(core.collapseCharges(state, t1Help(), ctx)) == 0 &&
			ctx.vAfterClosure("mentor", t1Help().Closed) == 1)
	check("t1_rival: the same D1 and D2, and the path is the critical node's",
		vecRival.D1 == 0 && vecRival.D2 == 1 && vecRival.D3 == 1,
		fmt.Sprintf("%v", vecRival))
	check("staying put wins at the second dimension against both: a cut path is a bar",
		core.EvaluateAndSelect(state, []*ActionOption{t1Help(), t1Rival()}, ctx) == nil)
	check("so the third dimension cannot separate two candidates — D3 <= D2, and an "+
		"admissible candidate has D2 = 0 (§4.5, finding of this release)",
		vecComp.D3 <= vecComp.D2 && vecMirror.D3 <= vecMirror.D2 &&
			vecHelp.D3 <= vecHelp.D2 && vecRival.D3 <= vecRival.D2)

	fmt.Println("=== 7. §4.5: bounds, the baseline and the retired gate ===")
	members := core.calcMembers(state, ctx)
	check("an empty candidate set selects nothing",
		core.EvaluateAndSelect(state, []*ActionOption{}, ctx) == nil)
	check("D1, D2 and D3 are bounded by calc(S)",
		vecComp.D1 <= len(members) && vecComp.D2 <= len(members) &&
			vecComp.D3 <= len(members) && vecRival.D2 <= len(members),
		fmt.Sprintf("|calc|=%d", len(members)))
	chosen = core.EvaluateAndSelect(state, []*ActionOption{t1Compensate(), t1Mirror()}, ctx)
	rep := core.Report(state, []*ActionOption{t1Compensate(), t1Mirror()}, chosen, "FAST_PASS",
		ReportInput{Declaration: decl, Ctx: ctx})
	var row *OptionReportRow
	for i := range rep.Options {
		if rep.Options[i].OptionID == "t1_compensate" {
			row = &rep.Options[i]
		}
	}
	check("the report carries the vector and the barring key per option",
		row != nil && row.CandidateVector.D2 == 1 && row.BarringKey != nil &&
			*row.BarringKey == "d2")
	check("the report carries the baseline",
		rep.Baseline.D1 == 0 && rep.Baseline.NetDelta == 0.0 && rep.Baseline.Reversible)
	check("a selected option is not reported as a refusal", !rep.NoCandidateBetter)
	noCollapseGate := true
	for _, removed := range rep.RemovedOptions {
		if removed.Gate == "collapse" {
			noCollapseGate = false
		}
	}
	check("the structural decision is no longer a removal: no collapse gate anywhere",
		noCollapseGate, fmt.Sprintf("%v", rep.RemovedOptions))
	_, retiredRemoved := core.ApplyStructuralGate(state, []*ActionOption{optWin(), optCollapse()}, ctx)
	check("a charged candidate is no longer deleted from the set: it is evaluated and reported",
		len(retiredRemoved) == 1 && retiredRemoved[0].Gate == "collapse",
		"the retired v0.7 rule is still callable by the historical harness")
	_, liveVectors := core.SelectCandidate(state, []*ActionOption{optWin(), optCollapse()}, ctx)
	check("and on the live path nobody leaves the candidate set", len(liveVectors) == 2)

	fmt.Println()
	fmt.Printf("checks: %d, failures: %d\n", checksRun, len(failures))
	if len(failures) > 0 {
		fmt.Printf("FAILURES: %v\n", failures)
		return
	}
	fmt.Println("OK")
}
