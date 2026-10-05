package main

// DOF-SPEC v0.11 §3.6 — the hypothesis-set artifact and its validation rules.
// Reference port; mirrors patterns/python/hypothesis.py exactly.
//
// `v0.10` made the *state* conditional; `v0.11` repaired the artifact so a
// hypothesis is a **complete alternative state** under the **shared ruler**
// (§3.6, §10(A)):
//
//   - a hypothesis supplies the *measured* content — the per-entity lens
//     counters, the measurement durations and the resource map, from which τ
//     follows; `current_dof`, `dof_known` and τ are **computed** from it by the
//     same named procedures, and a stated DoF that its own counters do not
//     produce is non-conformant input;
//   - the **ruler** (§3.4.2) and the observed graph `G` (§3.5) are **shared** —
//     structural uncertainty is *priced, not branched*;
//   - `is_collapse_source` is **not** hypothesis-local: the label is honoured
//     through an observed act, the act comes from the shared graph, and a set
//     that moves the label between readings is non-conformant input;
//   - absence and emptiness are the **observed-state singleton**, with
//     `plausible = true`, so `H_plausible` is never empty and the worst-case
//     operators of §4.10 are total.
//
// The core accepts `H` as supplied: it MUST NOT add, merge, split, drop, reorder
// or re-weight a hypothesis, and MUST NOT compute the `plausible` flag.

import (
	"fmt"
	"math"
	"sort"
)

// ObservedHypothesisID is the identifier of the observed reading (§3.6).
const ObservedHypothesisID = "$observed$"

// Hypothesis is §3.6: one declared interpretation of the same observed state.
type Hypothesis struct {
	ID        string             `json:"id"`
	Plausible bool               `json:"plausible"`
	State     *SystemStateMatrix `json:"state"`
	// Report context (§3.4.1): the declared causal reading. Inert — it MUST NOT
	// affect `calc`, the collapse charges, the §4.9 verdicts or the Axiom-3
	// exemption. An empty list reads as "names no possible source", never as
	// "asserts that no source exists".
	CollapseSourceCandidates []string `json:"collapse_source_candidates"`
	Basis                    string   `json:"basis"`
}

// HypothesisSet is §3.6: the artifact. `coverage` is a claim, and its default is
// cautious: absent means `"partial"`, never `"complete"`.
type HypothesisSet struct {
	Coverage   string        `json:"coverage"`
	Members    []*Hypothesis `json:"members"`
	HorizonMks *float64      `json:"horizon_mks"`
}

// CoverageOf reads a declared set's coverage with the cautious default.
func CoverageOf(hset *HypothesisSet) string {
	if hset == nil || hset.Coverage == "" {
		return "partial"
	}
	return hset.Coverage
}

// ObservedSingleton is §3.6/§4.10.6: an absent or empty `H` **is** the
// observed-state singleton.
func ObservedSingleton(state *SystemStateMatrix) []*Hypothesis {
	return []*Hypothesis{{
		ID: ObservedHypothesisID, Plausible: true, State: state,
		Basis: "absence or emptiness of H is the observed state",
	}}
}

// ResolvedMembers is `H` as the core reads it, including the absence/emptiness
// reduction.
func ResolvedMembers(state *SystemStateMatrix, hset *HypothesisSet) []*Hypothesis {
	if hset == nil || len(hset.Members) == 0 {
		return ObservedSingleton(state)
	}
	return hset.Members
}

// PlausibleMembers is `H_plausible = { h ∈ H : h.plausible }`.
//
// Never empty: the set is either the observed singleton or a declared set whose
// observed member MUST be present and plausible (§3.6), so a set that resolves
// to nothing is non-conformant input caught by `ValidateSet`.
func PlausibleMembers(members []*Hypothesis) []*Hypothesis {
	out := []*Hypothesis{}
	for _, h := range members {
		if h.Plausible {
			out = append(out, h)
		}
	}
	if len(out) == 0 && len(members) > 0 {
		return members[:1]
	}
	return out
}

// LensProduct is the entity's DoF as the product of its **own** lens values
// (§4.1, §4.6).
//
// `nil` when the entity carries no measurement declaration: the port-level
// `measurement` object is the carrier of the counters, and without it the
// identity cannot be checked. A missing declaration is not a contradiction.
func LensProduct(entity *EntityState) *float64 {
	m := entity.Measurement
	if m == nil {
		return nil
	}
	product := 1.0
	for _, lens := range lensOrder {
		value := m.Psi[lens]
		if value == nil {
			return nil
		}
		product *= *value
	}
	return &product
}

// ValidateSet is §3.6's non-conformance checks. An empty result means the input
// is admissible.
//
// Every rule here is a *check*, not a hope: each one corresponds to a way a
// manipulated or careless hypothesis set could otherwise move the index, the
// choice or the collapse-source label without leaving a trace.
func ValidateSet(state *SystemStateMatrix, members []*Hypothesis) []string {
	errs := []string{}
	seen := map[string]bool{}
	duplicate := false
	for _, h := range members {
		if seen[h.ID] {
			duplicate = true
		}
		seen[h.ID] = true
	}
	if duplicate {
		errs = append(errs, "hypothesis ids are not unique")
	}
	observedPresent := false
	for _, h := range members {
		hs := h.State
		if hs == nil {
			errs = append(errs, fmt.Sprintf("%s: carries no state (§3.6)", h.ID))
			continue
		}
		// (1) State completeness: every entity of `S` appears in every reading,
		//     with an explicit value, never omitted.
		missing := missingKeys(state.Entities, hs.Entities)
		if len(missing) > 0 {
			errs = append(errs, fmt.Sprintf(
				"%s: omits entities %v (§3.6 state completeness)", h.ID, missing))
		}
		extra := missingKeys(hs.Entities, state.Entities)
		if len(extra) > 0 {
			errs = append(errs, fmt.Sprintf(
				"%s: declares entities not in the observed state %v", h.ID, extra))
		}
		// (2) The lens identity holds under every hypothesis: the stated DoF must
		//     equal the product of that reading's OWN counters (§4.1, §3.6).
		for _, eID := range sortedKeys(hs.Entities) {
			ent := hs.Entities[eID]
			product := LensProduct(ent)
			if product == nil {
				continue // no counters carried: nothing to check
			}
			if math.Abs(*product-ent.CurrentDoF) > 1e-9 {
				errs = append(errs, fmt.Sprintf(
					"%s/%s: stated DoF %v differs from the product of its own lens "+
						"values %v (§4.1, §3.6)", h.ID, eID, ent.CurrentDoF, *product))
			}
		}
		// (3) `is_collapse_source` is not hypothesis-local: the label is honoured
		//     through an act of the SHARED graph, so it must equal the observed
		//     value under every reading (§3.6).
		for _, eID := range sortedKeys(hs.Entities) {
			ent := hs.Entities[eID]
			obs, ok := state.Entities[eID]
			if ok && ent.IsCollapseSource != obs.IsCollapseSource {
				errs = append(errs, fmt.Sprintf(
					"%s/%s: is_collapse_source differs from the observed value — the "+
						"label is not hypothesis-local (§3.6)", h.ID, eID))
			}
		}
		// (5) §3.4.1/§3.4.2 (v0.11): the measurement durations are
		//     **ruler-level**. A hypothesis reinterprets what was *measured*; it
		//     may not reinterpret how long the *measuring* takes. If it could,
		//     `T_meas` and hence `t*` would differ between readings that claim one
		//     ruler, and `min_h NetDelta(o | h)` would compare numbers produced by
		//     different measuring systems — which is what the shared
		//     `psi_ruler_digest` exists to make impossible.
		if !sameDurations(hs.MeasurementDurations, state.MeasurementDurations) {
			errs = append(errs, fmt.Sprintf(
				"%s: measurement durations differ from the observed procedure — the "+
					"durations are ruler-level, not hypothesis-level (§3.4.1, §3.4.2)",
				h.ID))
		}
		// (4) The observed state MUST be one of the readings (§3.6).
		if sameState(hs, state) {
			observedPresent = true
		}
	}
	if !observedPresent {
		errs = append(errs,
			"the observed state is absent from H (§3.6: it MUST be present)")
	}
	anyPlausible := false
	for _, h := range members {
		if h.Plausible {
			anyPlausible = true
		}
	}
	if !anyPlausible && len(members) > 1 {
		// A single-member set is the observed singleton and is plausible by
		// construction; a larger set that marks everything implausible leaves
		// `H_plausible` empty, which §3.6 declares an invalid input.
		errs = append(errs,
			"every hypothesis is implausible (§3.6: H_plausible would be empty)")
	}
	return errs
}

// missingKeys returns the sorted keys of `a` that `b` does not carry.
func missingKeys(a, b map[string]*EntityState) []string {
	out := []string{}
	for k := range a {
		if _, ok := b[k]; !ok {
			out = append(out, k)
		}
	}
	sort.Strings(out)
	return out
}

func sameDurations(a, b map[string]map[string]float64) bool {
	if len(a) != len(b) {
		return false
	}
	for lens, durs := range a {
		other, ok := b[lens]
		if !ok || len(durs) != len(other) {
			return false
		}
		for k, v := range durs {
			ov, ok := other[k]
			if !ok || math.Abs(v-ov) > 1e-9 {
				return false
			}
		}
	}
	return true
}

// sameState reports whether two matrices are the same *measured* state.
//
// Only the measured content is compared: the per-entity DoF, the lens counters,
// the resource map and τ. Report context and provenance are not part of the
// comparison — two readings that differ only in a `basis` string are the same
// state.
func sameState(a, b *SystemStateMatrix) bool {
	if a == nil || b == nil {
		return a == b
	}
	if len(a.Entities) != len(b.Entities) {
		return false
	}
	for eID, ea := range a.Entities {
		eb, ok := b.Entities[eID]
		if !ok {
			return false
		}
		if math.Abs(ea.CurrentDoF-eb.CurrentDoF) > 1e-9 {
			return false
		}
		if ea.DoFKnown != eb.DoFKnown {
			return false
		}
		ma, mb := ea.Measurement, eb.Measurement
		if (ma == nil) != (mb == nil) {
			return false
		}
		if ma != nil {
			for _, lens := range lensOrder {
				va, vb := ma.Psi[lens], mb.Psi[lens]
				if (va == nil) != (vb == nil) {
					return false
				}
				if va != nil && math.Abs(*va-*vb) > 1e-9 {
					return false
				}
			}
		}
	}
	if len(a.Resources) != len(b.Resources) {
		return false
	}
	for r, ra := range a.Resources {
		rb, ok := b.Resources[r]
		if !ok {
			return false
		}
		if (ra.Value == nil) != (rb.Value == nil) {
			return false
		}
		if ra.Value != nil && math.Abs(*ra.Value-*rb.Value) > 1e-9 {
			return false
		}
	}
	if (a.Tau == nil) != (b.Tau == nil) {
		return false
	}
	if a.Tau != nil && b.Tau != nil {
		if (a.Tau.Value == nil) != (b.Tau.Value == nil) {
			return false
		}
		if a.Tau.Value != nil && math.Abs(*a.Tau.Value-*b.Tau.Value) > 1e-9 {
			return false
		}
	}
	return true
}
