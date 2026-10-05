package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"math"
	"reflect"
	"sort"
	"strings"
)

const (
	Epsilon = 1e-6
	UAlpha  = 0.25
	UmaxLvl = 0.5
)

var (
	UminLvl   = math.Pow(Epsilon, 1.0-0.9)
	lensOrder = []string{"variety", "options", "constraint"}
)

func clamp01(x float64) float64 {
	if x < 0.0 {
		return 0.0
	}
	if x > 1.0 {
		return 1.0
	}
	return x
}

func PsiVar(V, VEnv float64) float64 {
	if V <= 0.0 {
		return 0.0
	}
	return clamp01(V / (V + math.Max(VEnv, 0.0)))
}

func PsiCon(F, FEnv float64) float64 {
	if F <= 0.0 {
		return 0.0
	}
	return clamp01(F / (F + math.Max(FEnv, 0.0)))
}

func PsiOpt(blocks [][2]float64) float64 {
	if len(blocks) == 0 {
		return 0.0
	}
	value := 1.0
	for _, b := range blocks {
		cG, CG := b[0], b[1]
		if cG <= 0.0 {
			continue
		}
		if CG <= 0.0 {
			return 0.0
		}
		value *= math.Pow(4.0, -(cG / CG))
	}
	return clamp01(value)
}

func canonicalGroups(groups [][]string, requirements map[string]float64, means map[string]float64) [][]string {
	named := make(map[string]bool)
	var normalized [][]string
	for _, group := range groups {
		members := make([]string, 0, len(group))
		seen := make(map[string]bool)
		for _, r := range group {
			if !seen[r] {
				members = append(members, r)
				seen[r] = true
			}
		}
		sort.Strings(members)
		if len(members) > 0 {
			normalized = append(normalized, members)
			for _, m := range members {
				named[m] = true
			}
		}
	}

	var extra []string
	for r := range requirements {
		if !named[r] {
			extra = append(extra, r)
		}
	}
	for r := range means {
		if !named[r] {
			extra = append(extra, r)
		}
	}
	sort.Strings(extra)

	var uniqueExtra []string
	seenExtra := make(map[string]bool)
	for _, r := range extra {
		if !seenExtra[r] {
			uniqueExtra = append(uniqueExtra, r)
			seenExtra[r] = true
		}
	}
	for _, r := range uniqueExtra {
		normalized = append(normalized, []string{r})
	}

	sort.Slice(normalized, func(i, j int) bool {
		if len(normalized[i]) == 0 || len(normalized[j]) == 0 {
			return len(normalized[i]) < len(normalized[j])
		}
		return normalized[i][0] < normalized[j][0]
	})
	return normalized
}

func deriveBlocks(requirements map[string]float64, means map[string]float64, groups [][]string, weights map[string]float64, cap *float64) [][2]float64 {
	// §4.6 (v0.7): c_g = Σ w_r·requirement_r and C_g = min(Σ w_r·means_r, cap).
	//
	// `w_r` is the observed price of resource r in the group numeraire. Without
	// it the sum adds credits to joules, and the value of the lens starts to
	// depend on the unit a resource happens to be declared in: the lens would
	// measure notation instead of the world. A resource with no path to the
	// numeraire is its own singleton group and carries weight 1.0 — with no
	// exchange available, its own unit IS its nominal.
	//
	// `cap` is the mandate: permission, never possibility. It can only lower
	// C_g, so a narrow mandate removes an option a large balance would have paid
	// for, and no mandate can make payable what the measured means cannot cover.
	var blocks [][2]float64
	w := func(r string) float64 {
		if weights == nil {
			return 1.0
		}
		if v, ok := weights[r]; ok {
			return v
		}
		return 1.0
	}
	for _, group := range canonicalGroups(groups, requirements, means) {
		var cG, CG float64
		for _, r := range group {
			cG += w(r) * math.Max(0.0, requirements[r])
			CG += w(r) * math.Max(0.0, means[r])
		}
		if cap != nil {
			CG = math.Min(CG, math.Max(0.0, *cap))
		}
		blocks = append(blocks, [2]float64{cG, CG})
	}
	return blocks
}

type VarietyObs struct {
	V    float64 `json:"V"`
	VEnv float64 `json:"V_env"`
}

type ConstraintObs struct {
	F    float64 `json:"F"`
	FEnv float64 `json:"F_env"`
}

type LensObservation struct {
	Variety      *VarietyObs        `json:"variety"`
	Options      *[][2]float64       `json:"options"`
	Constraint   *ConstraintObs     `json:"constraint"`
	Requirements map[string]float64 `json:"requirements"`
	// §4.6 (v0.7): the numeraire weights and the mandate cap. Declared per
	// observation as an alternative to passing them in, exactly as the reference
	// does; the caller's values win when both are present.
	Weights map[string]float64 `json:"weights"`
	Cap     *float64           `json:"cap"`
}

func (o LensObservation) psi(lens string, means map[string]float64, groups [][]string, weights map[string]float64, cap *float64) *float64 {
	if weights == nil {
		weights = o.Weights
	}
	if cap == nil {
		cap = o.Cap
	}
	switch lens {
	case "variety":
		if o.Variety == nil {
			return nil
		}
		v := PsiVar(o.Variety.V, o.Variety.VEnv)
		return &v
	case "options":
		if o.Options != nil {
			v := PsiOpt(*o.Options)
			return &v
		}
		if o.Requirements != nil {
			v := PsiOpt(deriveBlocks(o.Requirements, means, groups, weights, cap))
			return &v
		}
		return nil
	case "constraint":
		if o.Constraint == nil {
			return nil
		}
		v := PsiCon(o.Constraint.F, o.Constraint.FEnv)
		return &v
	}
	return nil
}

func U0FromPrior(priorQ *float64) float64 {
	q := 0.5
	if priorQ != nil {
		q = *priorQ
	}
	return math.Max(UminLvl, math.Min(UmaxLvl, q))
}

func TotalBudgetMks(tm, tv, taPlus, taMinus float64) float64 {
	return tm + tv + math.Max(taPlus, taMinus)
}

func UOfT(u0 float64, tauMks *float64, tMeasMks, tMks float64) float64 {
	// §4.7/§10(au): an **unmeasured** τ prices the ignorance at `u₀` — no window
	// is computable, so no deadline is being spent. A τ that is known but leaves
	// no window (`t* <= 0`) prices it at `ε`, not at `u₀`: the `v0.9.1` branch
	// returned `u₀` here, which put a jump of ~13 nats exactly where measurement
	// stops being possible.
	if tauMks == nil {
		return u0
	}
	tStar := *tauMks - tMeasMks
	if tStar <= 0.0 {
		return Epsilon
	}
	t := math.Max(0.0, math.Min(tMks, tStar))
	w := t / tStar
	return math.Pow(u0, 1.0-w) * math.Pow(Epsilon, w)
}

// tauJSON renders τ for the canonical dump: six-decimal string when measured,
// `null` when not. An unmeasured τ MUST NOT be written as a number — `0.0` would
// be a measured catastrophe and the `1e15` synthetic default a fabricated
// deadline (§3.2b).
func tauJSON(tau *float64) interface{} {
	if tau == nil {
		return nil
	}
	return canonFloat(*tau)
}

type LensTerm struct {
	Lens         string   `json:"lens"`
	Psi          *float64 `json:"psi"`
	DoFKnown     bool     `json:"dof_known"`
	Contribution float64  `json:"contribution"`
}

type EntityMeasurement struct {
	EntityID     string              `json:"entity_id"`
	Psi          map[string]*float64 `json:"psi"`
	Terms        []LensTerm          `json:"terms"`
	CurrentDoF   float64             `json:"current_dof"`
	DoFKnown     bool                `json:"dof_known"`
	Contribution float64             `json:"contribution"`
	TermsSum     float64             `json:"terms_sum"`
	Floored      bool                `json:"floored"`
	BindingLens  string              `json:"binding_lens"`
	Blocks       [][2]float64        `json:"blocks"`
	Derivation   map[string]interface{} `json:"derivation"`
	// §4.6 (v0.7): the declared counters behind the Variety share. Kept because
	// the price of a closure is recomputed from them (§4.4), not from the lens.
	VarietyCounters map[string]float64 `json:"variety_counters"`
}

func MeasureEntity(eid string, obs LensObservation, u float64, means map[string]float64, groups [][]string, weights map[string]float64, cap *float64) EntityMeasurement {
	if weights == nil {
		weights = obs.Weights
	}
	if cap == nil {
		cap = obs.Cap
	}
	psi := map[string]*float64{}
	terms := []LensTerm{}
	product := 1.0
	knownAll := true
	termsSum := 0.0
	binding := ""
	bindingValue := math.Inf(1)

	for _, lens := range lensOrder {
		value := obs.psi(lens, means, groups, weights, cap)
		psi[lens] = value
		var contribution float64
		if value == nil {
			knownAll = false
			contribution = math.Log(u)
			product *= u
		} else {
			contribution = math.Log(math.Max(*value, Epsilon))
			product *= *value
			if *value < bindingValue {
				bindingValue = *value
				binding = lens
			}
		}
		termsSum += contribution
		terms = append(terms, LensTerm{Lens: lens, Psi: value, DoFKnown: value != nil, Contribution: contribution})
	}

	var blocks [][2]float64
	var derivation map[string]interface{}
	if obs.Requirements != nil {
		blocks = deriveBlocks(obs.Requirements, means, groups, weights, cap)
		derivation = map[string]interface{}{
			"procedure":    "derive_blocks",
			"requirements": obs.Requirements,
			"means":        means,
			"groups":       canonicalGroups(groups, obs.Requirements, means),
			// §4.6 (v0.7): the numeraire weights and the mandate cap are part of
			// the derivation, so a reader can recompute (c_g, C_g) and see that
			// the sum is not adding different physical units together.
			"weights": weights,
			"cap":     cap,
		}
	}

	var counters map[string]float64
	if obs.Variety != nil {
		counters = map[string]float64{"V": obs.Variety.V, "V_env": obs.Variety.VEnv}
	}

	return EntityMeasurement{
		EntityID:        eid,
		Psi:             psi,
		Terms:           terms,
		CurrentDoF:      clamp01(product),
		DoFKnown:        knownAll,
		Contribution:    math.Log(math.Max(product, Epsilon)),
		TermsSum:        termsSum,
		Floored:         product < Epsilon,
		BindingLens:     binding,
		Blocks:          blocks,
		Derivation:      derivation,
		VarietyCounters: counters,
	}
}

type ResourceInfo struct {
	ID    string
	Unit  string
	Scale float64
}

type RateInfo struct {
	Rate        float64
	DurationMks float64
}

type MeasurementDeclaration struct {
	PsiID      string
	LensOrder  []string
	U0PriorQ   *float64
	Entities   map[string]LensObservation
	TauMks     *float64
	Resources  []ResourceInfo
	Groups     [][]string
	Rates      map[string]RateInfo
	Mandate    map[string]interface{}
	Procedures map[string]string
	// §3.4.1 hashed content (v0.7): the graph-derived values of §4.9 and the
	// numeraire the group amounts are expressed in. Only what determines numbers
	// is here — the graph itself, the witness paths and the observation digest
	// are report context (§6.2), and an option's closure list is a per-option
	// input like `projected_dof_delta`, not ruler content.
	Numeraire      *string
	Weights        map[string]float64
	MandateCap     *float64
	Verdicts       map[string]VerdictRecord
	MeansClass     []string
	GraphProcedure string
	// §4.7 (v0.11): the declared measurement durations per lens, hashed ruler
	// content. Omitted from the canonical dump when empty, so a state that
	// declares none hashes exactly as it did before the field existed (§3.4.3).
	MeasurementDurations map[string]map[string]float64
}

// VerdictRecord is one entity's declared verdict together with the counters and
// the horizon it was computed with, so the declaration can be checked against
// the observation it came from (`verifyGraphDerived`).
type VerdictRecord struct {
	Verdict string
	TRecMks *float64
	V       int
}

func NewDeclaration(psiID string, entities map[string]LensObservation, tauMks *float64, u0PriorQ *float64, resources []ResourceInfo, groups [][]string, rates map[string]RateInfo, mandate map[string]interface{}, numeraire *string, weights map[string]float64, mandateCap *float64, verdicts map[string]VerdictRecord, meansClass []string, graphProcedure string, measurementDurations map[string]map[string]float64) *MeasurementDeclaration {
	procs := make(map[string]string)
	for _, lens := range lensOrder {
		procs[lens] = psiID + ":" + lens
	}
	procs["options_blocks"] = psiID + ":derive_blocks"
	cls := append([]string{}, meansClass...)
	sort.Strings(cls)
	return &MeasurementDeclaration{
		PsiID:                psiID,
		LensOrder:            lensOrder,
		U0PriorQ:             u0PriorQ,
		Entities:             entities,
		TauMks:               tauMks,
		Resources:            resources,
		Groups:               canonicalGroups(groups, nil, nil),
		Rates:                rates,
		Mandate:              mandate,
		Procedures:           procs,
		Numeraire:            numeraire,
		Weights:              weights,
		MandateCap:           mandateCap,
		Verdicts:             verdicts,
		MeansClass:           cls,
		GraphProcedure:       graphProcedure,
		MeasurementDurations: measurementDurations,
	}
}

func (d *MeasurementDeclaration) U0() float64 { return U0FromPrior(d.U0PriorQ) }

func canonFloat(x float64) string { return fmt.Sprintf("%.6f", x) }

// perEntityWeights / perEntityFloat render the optional per-entity ruler fields
// as the reference does: a map of canonical strings, or null.
func perEntityWeights(m map[string]float64) interface{} {
	if m == nil {
		return nil
	}
	out := make(map[string]interface{}, len(m))
	for k, v := range m {
		out[k] = canonFloat(v)
	}
	return out
}

func perEntityFloat(p *float64) interface{} {
	if p == nil {
		return nil
	}
	return canonFloat(*p)
}

func canonicalize(obj interface{}) interface{} {
	switch v := obj.(type) {
	case map[string]interface{}:
		res := make(map[string]interface{})
		for k, val := range v {
			res[k] = canonicalize(val)
		}
		return res
	case []interface{}:
		res := make([]interface{}, len(v))
		for i, val := range v {
			res[i] = canonicalize(val)
		}
		return res
	case float64:
		return canonFloat(v)
	case int:
		return v
	case bool:
		return v
	case nil:
		return nil
	}
	// Typed collections (`[][]string` for groups, `map[string]string` for
	// procedures, …) MUST be walked as well: falling through to `fmt.Sprintf`
	// would emit Go syntax (`[[credit energy]]`) instead of JSON, and two ports
	// would report the same ruler under different digests (§3.4.3).
	rv := reflect.ValueOf(obj)
	switch rv.Kind() {
	case reflect.Slice, reflect.Array:
		res := make([]interface{}, rv.Len())
		for i := 0; i < rv.Len(); i++ {
			res[i] = canonicalize(rv.Index(i).Interface())
		}
		return res
	case reflect.Map:
		res := make(map[string]interface{}, rv.Len())
		for _, key := range rv.MapKeys() {
			res[fmt.Sprintf("%v", key.Interface())] = canonicalize(rv.MapIndex(key).Interface())
		}
		return res
	}
	return fmt.Sprintf("%v", obj)
}

func (d *MeasurementDeclaration) canonicalDoc() map[string]interface{} {
	entities := make(map[string]interface{})
	for eid, obs := range d.Entities {
		var variety interface{}
		if obs.Variety != nil {
			variety = map[string]interface{}{"V": canonFloat(obs.Variety.V), "V_env": canonFloat(obs.Variety.VEnv)}
		}
		var options interface{}
		if obs.Options != nil {
			arr := []interface{}{}
			for _, b := range *obs.Options {
				arr = append(arr, []interface{}{canonFloat(b[0]), canonFloat(b[1])})
			}
			options = arr
		}
		var constraint interface{}
		if obs.Constraint != nil {
			constraint = map[string]interface{}{"F": canonFloat(obs.Constraint.F), "F_env": canonFloat(obs.Constraint.FEnv)}
		}
		var requirements interface{}
		if obs.Requirements != nil {
			reqs := make(map[string]interface{})
			for k, v := range obs.Requirements {
				reqs[k] = canonFloat(v)
			}
			requirements = reqs
		} else {
			requirements = nil
		}
		entities[eid] = map[string]interface{}{"variety": variety, "options": options, "constraint": constraint, "requirements": requirements,
			// §4.6 (v0.7): the per-entity numeraire weights and mandate cap. Null
			// when the entity does not declare them — the same shape every other
			// port hashes, because a missing key and a null key hash differently.
			"weights": perEntityWeights(obs.Weights), "cap": perEntityFloat(obs.Cap)}
	}

	var resources []interface{}
	sort.Slice(d.Resources, func(i, j int) bool {
		return d.Resources[i].ID < d.Resources[j].ID
	})
	for _, r := range d.Resources {
		resources = append(resources, map[string]interface{}{"id": r.ID, "scale": canonFloat(r.Scale), "unit": r.Unit})
	}

	var u0 interface{}
	if d.U0PriorQ != nil {
		u0 = canonFloat(*d.U0PriorQ)
	}

	// §3.4.3: an undeclared duration set is ONE condition — the procedure is
	// undeclared and the window uncomputable — so an absent map and an empty one
	// MUST hash alike, and the key is omitted when empty. That is also what keeps
	// the field additive: a state that declares no duration hashes exactly as it
	// did before the field existed (§3.4.1, §10(an)).
	freeze := map[string]interface{}{"tau_mks": tauJSON(d.TauMks)}
	if len(d.MeasurementDurations) > 0 {
		// §3.4.1: every number of the canonical form is a fixed six-decimal string
		// (`canonFloat`), so a declared duration is hashed exactly like a scale, a
		// rate or `tau_mks`. Passing the raw floats through the encoder would render
		// them as JSON numbers and make Go's digest differ from the reference port's
		// for the same declaration.
		durations := make(map[string]interface{}, len(d.MeasurementDurations))
		for lens, durs := range d.MeasurementDurations {
			inner := make(map[string]interface{}, len(durs))
			for name, v := range durs {
				inner[name] = canonFloat(v)
			}
			durations[lens] = inner
		}
		freeze["measurement_durations"] = durations
	}

	doc := map[string]interface{}{
		"entities":   entities,
		"freeze":     freeze,
		"groups":     d.Groups,
		"lens_order": d.LensOrder,
		"mandate":    d.Mandate,
		"procedures": d.Procedures,
		"psi_id":     d.PsiID,
		"rates":      d.Rates,
		"resources":   resources,
		"u0_prior_q": u0,
	}

	ratesCanon := make(map[string]interface{})
	for k, v := range d.Rates {
		ratesCanon[k] = map[string]interface{}{"duration_mks": canonFloat(v.DurationMks), "rate": canonFloat(v.Rate)}
	}
	doc["rates"] = ratesCanon

	mandateCanon := make(map[string]interface{})
	for k, v := range d.Mandate {
		if val, ok := v.(float64); ok {
			mandateCanon[k] = canonFloat(val)
		} else {
			mandateCanon[k] = v
		}
	}
	doc["mandate"] = mandateCanon

	// §3.4.1 (v0.7): the graph-derived content. Rendered in the canonical form —
	// floats as fixed six-decimal strings, `v` as an INTEGER, absent as null —
	// because the digest must be a function of the ruler and not of a language's
	// default float notation.
	weightsCanon := make(map[string]interface{})
	for k, v := range d.Weights {
		weightsCanon[k] = canonFloat(v)
	}
	verdictsCanon := make(map[string]interface{})
	for eid, v := range d.Verdicts {
		var tRec interface{}
		if v.TRecMks != nil {
			tRec = canonFloat(*v.TRecMks)
		}
		verdictsCanon[eid] = map[string]interface{}{
			"verdict":   v.Verdict,
			"t_rec_mks": tRec,
			"v":         v.V,
		}
	}
	var numeraire interface{}
	if d.Numeraire != nil {
		numeraire = *d.Numeraire
	}
	var mandateCap interface{}
	if d.MandateCap != nil {
		mandateCap = canonFloat(*d.MandateCap)
	}
	meansClass := append([]string{}, d.MeansClass...)
	sort.Strings(meansClass)
	doc["numeraire"] = numeraire
	doc["weights"] = weightsCanon
	doc["mandate_cap"] = mandateCap
	doc["verdicts"] = verdictsCanon
	doc["means_class"] = meansClass
	doc["graph_procedure"] = d.GraphProcedure

	return doc
}

// renderCanonical serializes a canonical payload exactly as every port must.
//
// HTML escaping MUST be off: Go's default encoder writes `>` as `\u003e`, which
// would make an observed rate key (`credit->energy`) hash differently from the
// identical UTF-8 bytes every other port writes.
func renderCanonical(doc map[string]interface{}) string {
	canonDoc := canonicalize(doc)
	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	enc.SetEscapeHTML(false)
	_ = enc.Encode(canonDoc)
	return strings.TrimRight(buf.String(), "\n")
}

func (d *MeasurementDeclaration) CanonicalText() string {
	return renderCanonical(d.canonicalDoc())
}

// RulerCanonicalText is §3.4.2/§3.4.3: the **ruler-level** content of the same
// declaration — everything identical for every hypothesis of a cycle and for
// every option: the procedure and its version, the lens set, the units and
// scales, the means class `M(S)`, the derived groups, the observed rates with
// their numeraire, the mandate, the `u₀` prior and the graph procedure identity.
//
// Three groups of fields are **excluded**, each for a stated reason:
//
//   - `entities` — the per-entity **lens counters**, which are precisely what a
//     hypothesis varies (§3.6);
//   - `freeze` — τ and the budgets, which are the hypothesis's own measured
//     content;
//   - `verdicts` — the §4.9 **verdict**, which consumes `DoF(X | h)` and is
//     therefore computed per hypothesis (§4.9). Its horizon `T_rec(X)` is
//     type-derived and shared, but it does not have to be *hashed* for the
//     readings to be comparable, and the declaration carries no field for it:
//     `v0.11` is additive, so the **full** `Digest()` stays byte-identical to
//     the `v0.7`/`v0.8` ruler digest.
//
// Two readings of one cycle therefore have **equal** `RulerDigest()` and
// **different** `Digest()`, which is what makes the `min_h` of §4.10 a
// conformant output while a comparison of two different rulers is not.
func (d *MeasurementDeclaration) RulerCanonicalText() string {
	doc := d.canonicalDoc()
	for _, field := range []string{"entities", "freeze", "verdicts"} {
		delete(doc, field)
	}
	return renderCanonical(doc)
}

func (d *MeasurementDeclaration) Digest() string {
	sum := sha256.Sum256([]byte(d.CanonicalText()))
	return hex.EncodeToString(sum[:])
}

// RulerDigest is the shared comparability key of §3.4.2: equal for every
// hypothesis of one cycle, and the only digest under which two readings may be
// aggregated. Two states whose rulers differ MUST NOT be compared, however
// close their per-hypothesis digests happen to be.
func (d *MeasurementDeclaration) RulerDigest() string {
	sum := sha256.Sum256([]byte(d.RulerCanonicalText()))
	return hex.EncodeToString(sum[:])
}

type PsiReference struct {
	ID     string
	Digest string
}
