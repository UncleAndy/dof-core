package main

import (
	"fmt"
	"math"
	"sort"
	"strings"
)

// ResourceValue resolves a resource's usable value for the gate (§4.8).
func ResourceValue(obs *ResourceObservation, useEstimated bool) float64 {
	if obs == nil {
		return 0.0
	}
	if obs.Value != nil {
		return *obs.Value
	}
	if useEstimated && obs.Estimated != nil {
		return *obs.Estimated
	}
	return 0.0
}

// IsStale reports whether a ResourceObservation's data is stale (§3.2a).
func IsStale(obs *ResourceObservation, now float64) bool {
	if obs == nil || obs.AgingTime <= 0.0 {
		return false
	}
	return (now - obs.LastMeasuredAt) > obs.AgingTime
}

type EntityState struct {
	EntityID          string  `json:"entity_id"`
	IsAutonomous      bool    `json:"is_autonomous"`
	AgencyIndex       float64 `json:"agency_index"`
	CurrentDoF        float64 `json:"current_dof"`
	IsCollapseSource  bool    `json:"is_collapse_source"`
	DoFKnown          bool    `json:"dof_known"`
	TimeToCollapseMks float64 `json:"time_to_collapse_mks"`
	Measurement       *EntityMeasurement `json:"-"`
}

type SystemStateMatrix struct {
	GlobalTimeToCollapseMks float64                 `json:"global_time_to_collapse_mks"`
	ContextSwitchCost       float64                 `json:"context_switch_cost"`
	Entities                map[string]*EntityState `json:"entities"`
	Psi                     *PsiReference           `json:"psi"`
	Resources               map[string]*ResourceObservation `json:"resources"`
	// §3.2b (v0.9.1): τ as ResourceObservation.
	Tau *ResourceObservation `json:"tau"`
	// §3.2b (v0.11): the active individual deadlines τ is derived from. τ is
	// `null` when **any** active deadline is unmeasured — never the minimum over
	// the measured ones alone, and never `0.0` (§3.2b, §10(I), §10(ao)).
	Deadlines map[string]*float64 `json:"deadlines"`
	// §4.7 (v0.11): the declared measurement durations `t_m`, `t_v` per lens.
	// Hashed ruler content: two states differing only in `t_m` produce different
	// `ruler_digest` (§3.4.1, §10(an)).
	MeasurementDurations map[string]map[string]float64 `json:"measurement_durations"`
	// §4.7 (v0.11): the declared **schedule** `t` per lens — when the measurement
	// is planned to happen. An undeclared schedule reads as `t = 0` (`u₀`); a
	// declared `t > t*` is non-conformant input, never clamped.
	MeasurementSchedule map[string]float64 `json:"measurement_schedule"`
}

// TauOf is §3.2b: τ as the calculus reads it — from the **resource map**, signed.
//
// `state.Tau` is the `tau` `ResourceObservation`. When it is absent or its
// `value` is `null`, τ is **unmeasured** (`null`), never the minimum over the
// measured deadlines alone and never `0.0`: an unmeasured active deadline may be
// the most urgent one, so acting on the budget the measured ones support is
// acting on a budget the state does not establish, and writing `0.0` invents a
// catastrophe (§3.1, §3.2b).
//
// A **negative** value is a deadline that has passed, `|τ|` ago. It is a *known*
// state and MUST NOT be clamped to `0.0` or replaced by `null`, which means
// unmeasured only (§3.2b, §4.8b).
func TauOf(state *SystemStateMatrix) *float64 {
	if state == nil {
		return nil
	}
	if state.Tau != nil {
		if state.Tau.Value == nil {
			return nil
		}
		v := *state.Tau.Value
		return &v
	}
	// No `tau` observation at all: fall back to the declared individual
	// deadlines, and only then to the deprecated mirror — which a port that
	// predates the resource layer still writes. The mirror is read **only** when
	// the state carries no resource-map τ and no deadline set, so it can never
	// override a measurement (§3.2b).
	if len(state.Deadlines) > 0 {
		best := math.Inf(1)
		for _, v := range state.Deadlines {
			if v == nil {
				return nil
			}
			if *v < best {
				best = *v
			}
		}
		return &best
	}
	v := state.GlobalTimeToCollapseMks
	return &v
}

// MeasurementWindow is §5's `t*` for a measurement of `t_meas_mks`: `null` when τ
// is unmeasured — an unknown budget is not a closed window, and the strict
// `t* > 0` rule of §5 is not applied to it (a τ measurement is governed by §4.8b
// instead). τ is read from the resource map, never from the deprecated mirror.
func MeasurementWindow(state *SystemStateMatrix, tMeasMks float64) *float64 {
	tau := TauOf(state)
	if tau == nil {
		return nil
	}
	w := *tau - tMeasMks
	return &w
}

// MirrorTimeToCollapse is §3.1/§3.2b's deprecated mirror — clamped,
// non-authoritative.
//
// Equal to τ when τ is known and non-negative, `0.0` when τ is negative or
// `null`. It is **forbidden as an input to any rule** of §4.7, §4.8b or §5, and
// the `1e15` µs synthetic default is withdrawn: a fabricated deadline is
// indistinguishable from a measured one.
func MirrorTimeToCollapse(tau *float64) float64 {
	if tau == nil || *tau < 0.0 {
		return 0.0
	}
	return *tau
}

type ActionOption struct {
	OptionID             string                        `json:"option_id"`
	Description          string                        `json:"description"`
	// §3.3 (v0.11): **two forms, never mixed within one option.**
	//   flat           — {entity_id: delta}, applied under every hypothesis
	//   per_hypothesis — {hypothesis_id: {entity_id: delta}}, the entry for `h`
	//                    used under `h`, entities unlisted for `h` taking 0.0
	// An option in the flat form asserts that its effect does not depend on the
	// causal reading; the per-hypothesis form is meaningful only when a
	// hypothesis set is declared. The port carries the two forms in two fields
	// rather than one union-typed field; `ProjectionForm` is the single place
	// that decides which form an option is in, and a non-empty pair is
	// `"invalid"` (§3.3, §10(B)).
	ProjectedDoFDelta      map[string]float64              `json:"projected_dof_delta"`
	ProjectedByHypothesis  map[string]map[string]float64   `json:"projected_dof_delta_by_hypothesis"`
	ProjectedResourceDelta map[string]map[string]float64   `json:"projected_resource_delta"`
	IsReversible         bool                          `json:"is_reversible"`
	EstimatedDurationMks float64                       `json:"estimated_duration_mks"`
	// §3.3 (v0.11): the projected τ change. `nil` means **not computable** —
	// which happens exactly when τ is unknown — and is admissible only for an act
	// that resolves τ. Derived, never independently declared, for such an act: it
	// MUST equal `projected_tau_value - (τ - estimated_duration_mks)`.
	ProjectedTauDelta *float64 `json:"projected_tau_delta"`
	// §3.3 (v0.11): the value the option expects `tau` to hold AFTER it executes.
	// Present iff `discovers` names "tau". MAY be negative (§3.2b).
	ProjectedTauValue *float64 `json:"projected_tau_value"`
	// §3.3/§4.4 (v0.7): the transitions this option CLOSES — the acts and means
	// that cease to exist once it executes. `is_reversible` is DERIVED from this
	// list (true exactly when it is empty) and is kept only as a reported field:
	// a label that could be set to dodge the price is not a rule.
	//
	// §4.4 (v0.11): the closure list has the same two forms as the projection.
	Closed             []ClosedRef            `json:"closed"`
	ClosedByHypothesis map[string][]ClosedRef `json:"closed_by_hypothesis"`
	ActID              string                 `json:"act_id"`
	// §3.3 (v0.9): resources needed for gate checks, resources resolved by execution.
	Requires  []string `json:"requires"`
	Discovers []string `json:"discovers"`
}

// ProjectionForm is `"flat"` | `"per_hypothesis"` | `"invalid"` (§3.3, §10(B)).
func (o *ActionOption) ProjectionForm() string {
	flat := len(o.ProjectedDoFDelta) > 0
	nested := len(o.ProjectedByHypothesis) > 0
	if flat && !nested {
		return "flat"
	}
	if nested && !flat {
		return "per_hypothesis"
	}
	if !flat && !nested {
		// An option declaring no delta at all is the flat form: the empty map is
		// a flat map, and reading it as "invalid" would refuse every baseline.
		return "flat"
	}
	return "invalid"
}

// ClosureForm is `"flat"` | `"per_hypothesis"` | `"invalid"` (§4.4, §10(B)).
func (o *ActionOption) ClosureForm() string {
	flat := len(o.Closed) > 0
	nested := len(o.ClosedByHypothesis) > 0
	if flat && !nested {
		return "flat"
	}
	if nested && !flat {
		return "per_hypothesis"
	}
	if !flat && !nested {
		return "flat"
	}
	return "invalid"
}

// DeltaFor is the declared delta this option contributes under `hypothesisID`.
func (o *ActionOption) DeltaFor(hypothesisID, entityID string) float64 {
	if o.ProjectionForm() == "per_hypothesis" {
		perH := o.ProjectedByHypothesis[hypothesisID]
		if perH == nil {
			return 0.0
		}
		return perH[entityID]
	}
	return o.ProjectedDoFDelta[entityID]
}

// ClosedFor is the closures this option declares under `hypothesisID`.
func (o *ActionOption) ClosedFor(hypothesisID string) []ClosedRef {
	if o.ClosureForm() == "per_hypothesis" {
		return o.ClosedByHypothesis[hypothesisID]
	}
	return o.Closed
}

// FlatDelta is the flat delta map, whichever form the option uses.
//
// Used where a single map is needed for `nil`-safety (the baseline); a
// per-hypothesis option returns the **union** of its entries, which is only
// meaningful for existence questions.
func (o *ActionOption) FlatDelta() map[string]float64 {
	if o.ProjectionForm() == "per_hypothesis" {
		merged := map[string]float64{}
		for _, perH := range o.ProjectedByHypothesis {
			for eID, delta := range perH {
				if cur, ok := merged[eID]; !ok || delta > cur {
					merged[eID] = delta
				}
			}
		}
		return merged
	}
	out := map[string]float64{}
	for k, v := range o.ProjectedDoFDelta {
		out[k] = v
	}
	return out
}

// FormsConsistent is §3.3/§4.4: the two forms MUST NOT be mixed within one
// option. An empty result means the option is well formed.
func (o *ActionOption) FormsConsistent() string {
	if o.ProjectionForm() == "invalid" {
		return o.OptionID + ": `projected_dof_delta` mixes the flat and " +
			"per-hypothesis forms (§3.3)"
	}
	if o.ClosureForm() == "invalid" {
		return o.OptionID + ": `closed` mixes the flat and per-hypothesis forms (§4.4)"
	}
	return ""
}

// ObservationContext is the observation a cycle is decided over (§3.5, §4.9).
//
// Deliberately NOT a state field: the world graph is a Perception artifact
// supplied to the cycle, exactly as the derived groups and the observed rates
// are (§4.8). Without it every verdict is `undetermined`, which means no entity
// at a known zero is excluded and no collapse-source label is honoured — the
// fail-safe direction: nothing is proven, so nothing is removed.
type ObservationContext struct {
	World              *WorldGraph
	MeansClass         []string
	TRec               map[string]float64
	CountingHorizonMks *float64
	ObservationDigest  string
	// §3.6/§4.9/§4.10 (v0.11): `DoF(X | h)` — the per-entity degrees of freedom
	// **under one reading**, which the §4.9 verdict consumes. `nil` means "read
	// the graph's own value", which is what the observed reading does; a
	// hypothesis reading supplies its own numbers here. Structural inputs —
	// `G`, the paths and their admissibility, `M(S)`, `T_rec(X)` — stay shared:
	// only the DoF is conditional.
	DoFOverride map[string]float64
}

// doFOverrideOf is the reading's `DoF(X | h)` for one entity, or nil when this
// context carries no reading (§4.9).
func (ctx *ObservationContext) doFOverrideOf(entityID string) *float64 {
	if ctx == nil || ctx.DoFOverride == nil {
		return nil
	}
	if v, ok := ctx.DoFOverride[entityID]; ok {
		return &v
	}
	return nil
}

// WithDoF is §3.6/§4.9: the same observation, read under one hypothesis's DoF.
//
// Every shared input is carried over untouched — the graph, `M(S)`, the horizons,
// the observation digest — because §3.6 shares them by construction: only
// `DoF(X | h)` differs between readings, and only it is replaced here.
func (ctx *ObservationContext) WithDoF(dofs map[string]float64) *ObservationContext {
	if ctx == nil {
		return nil
	}
	clone := *ctx
	clone.DoFOverride = dofs
	return &clone
}

func (ctx *ObservationContext) horizon(entityID string) *float64 {
	if ctx == nil || ctx.TRec == nil {
		return nil
	}
	if v, ok := ctx.TRec[entityID]; ok {
		return &v
	}
	return nil
}

func (ctx *ObservationContext) verdict(entityID string) string {
	// §4.9/§4.10: the verdict consumes `DoF(X | h)`. Under the observed reading
	// there is no override and the graph's own value is used — byte-identical to
	// every earlier release; under a hypothesis the reading supplies it.
	return ctx.World.VerdictWithDoF(entityID, ctx.MeansClass,
		ctx.horizon(entityID), ctx.doFOverrideOf(entityID)).Verdict
}

func (ctx *ObservationContext) vBefore(entityID string) int {
	return ctx.World.VCount(entityID, ctx.MeansClass, ctx.CountingHorizonMks)
}

func (ctx *ObservationContext) vAfterClosure(entityID string, closed []ClosedRef) int {
	if len(closed) == 0 {
		return ctx.vBefore(entityID)
	}
	return ctx.World.WithClosed(closed).VCount(entityID, ctx.MeansClass, ctx.CountingHorizonMks)
}

type EntityReportRow struct {
	EntityID         string     `json:"entity_id"`
	IsCollapseSource bool       `json:"is_collapse_source"`
	IncludedInSum    bool       `json:"included_in_sum"`
	CurrentDoF       float64    `json:"current_dof"`
	DoFKnown         bool       `json:"dof_known"`
	Contribution     float64    `json:"contribution"`
	LensTerms        []LensTerm `json:"lens_terms"`
	BindingLens      string     `json:"binding_lens"`
	Floored          bool       `json:"floored"`
	Blocks           [][2]float64 `json:"blocks"`
	Derivation       map[string]interface{} `json:"derivation"`
	// §6.1 (v0.7): the recoverability verdict, its witness and the completeness
	// of the observation behind it.
	Recoverability map[string]interface{} `json:"recoverability"`
}

type CollapseCharge struct {
	EntityID  string  `json:"entity_id"`
	DoFBefore float64 `json:"dof_before"`
}

// ResourceObservation is the v0.9 observable-resource record (§3.2a): a concrete
// quantity with metadata, not an abstract unit.
type ResourceObservation struct {
	Value             *float64  `json:"value"`              // null = unmeasured
	Unit              string    `json:"unit"`
	Scale             float64   `json:"scale"`
	Source            string    `json:"source"`             // sensor / API / ROM / derived
	LastMeasuredAt    float64   `json:"last_measured_at"`
	AgingTime         float64   `json:"aging_time"`
	Estimated         *float64  `json:"estimated"`          // used when value is nil
	EstimationSource  []string  `json:"estimation_source"`
}

// ResourceValue resolves a resource's usable value for the gate (§4.8). When the
// value is nil and useEstimated is true, it falls back to estimated.
// IsUsable reports whether a resource has a known positive value (§4.8).
func (obs *ResourceObservation) IsUsable() bool {
	return obs != nil && obs.Value != nil && *obs.Value > 0.0
}

// CandidateVector is the v0.8 candidate vector (§4.5): three counts of entities —
// the protected dimensions — plus the index and the reversibility preference.
type CandidateVector struct {
	D1         int     `json:"d1"`
	D2         int     `json:"d2"`
	D3         int     `json:"d3"`
	NetDelta   float64 `json:"net_delta"`
	Reversible bool    `json:"reversible"`
	OptionID   string  `json:"option_id"`
}

// LostPathEntry is one entity this option drops out of a `reachable` verdict,
// with the witness it lost (§4.5, §6.3). A path loss counts even where no
// exclusion follows from it.
type LostPathEntry struct {
	EntityID      string   `json:"entity_id"`
	VerdictBefore string   `json:"verdict_before"`
	VerdictAfter  string   `json:"verdict_after"`
	Critical      bool     `json:"critical"`
	WitnessLost   []string `json:"witness_lost"`
}

type OptionReportRow struct {
	OptionID             string                         `json:"option_id"`
	IsReversible         bool                           `json:"is_reversible"`
	ProjectedDoF         float64                        `json:"projected_dof"`
	NetDelta             float64                        `json:"net_delta"`
	Selected             bool                           `json:"selected"`
	EstimatedDurationMks float64                        `json:"estimated_duration_mks"`
	CollapseCharges      []CollapseCharge               `json:"collapse_charges"`
	ResourceConsumption  map[string]map[string]float64   `json:"resource_consumption"`
	ConversionApplied    []map[string]interface{}        `json:"conversion_applied"`
	ResourcesUncovered   map[string]float64              `json:"resources_uncovered"`
	MandateExceeded      float64                         `json:"mandate_exceeded"`
	// §6.3 (v0.7): what the option closes, and how the loss decomposes.
	Closed       []ClosedRef        `json:"closed"`
	ClosureShare map[string]float64 `json:"closure_share"`
	// §6.3 (v0.8): the protected dimensions, the key that barred the candidate
	// (nil when nothing did), and the path losses line by line.
	CandidateVector CandidateVector `json:"candidate_vector"`
	BarringKey      *string         `json:"barring_key"`
	LostPaths       []LostPathEntry `json:"lost_paths"`
	Requires        []string        `json:"requires"`
	Discovers       []string        `json:"discovers"`
}

type RemovedOption struct {
	OptionID string `json:"option_id"`
	Gate     string `json:"gate"`
}

type DofReport struct {
	Entities                []EntityReportRow `json:"entities"`
	TotalSystemDoF          float64           `json:"total_system_dof"`
	ContextSwitchCost       float64           `json:"context_switch_cost"`
	GlobalTimeToCollapseMks float64           `json:"global_time_to_collapse_mks"`
	Mode                    string            `json:"mode"`
	Options                 []OptionReportRow `json:"options"`
	PsiID                   string            `json:"psi_id"`
	PsiDigest               string            `json:"psi_digest"`
	Declaration             string            `json:"declaration"`
	RemovedOptions          []RemovedOption   `json:"removed_options"`
	Incomplete              bool              `json:"incomplete"`
	ResourcesBefore         map[string]interface{} `json:"resources_before"`
	ResourcesAfter          map[string]interface{} `json:"resources_after"`
	// §6.2 (v0.7): the identity of the observation a reported subgraph was taken
	// from, and where the amounts a decision rests on came from — a measured
	// balance or an asserted authority.
	ObservationDigest *string                `json:"observation_digest"`
	MeansProvenance   map[string]interface{} `json:"means_provenance"`
	// §6.2 (v0.8): the vector every candidate was compared against, and whether
	// any candidate beat it. A refusal to act is a decision and must be audible.
	Baseline          CandidateVector `json:"baseline"`
	NoCandidateBetter bool            `json:"no_candidate_better"`
}

// ReportInput carries what a report needs beyond the state, the candidates and
// the selection. It keeps the report call site readable now that the report is
// the place where the release's reasons are written down (§6).
type ReportInput struct {
	Declaration     *MeasurementDeclaration
	Removed         []RemovedOption
	Groups          [][]string
	Rates           map[string]RateInfo
	Weights         map[string]float64
	Cap             *float64
	Ctx             *ObservationContext
	MeansProvenance map[string]interface{}
}

type DOFCalculusCore struct {
	epsilon float64
}

func NewDOFCalculusCore() *DOFCalculusCore {
	return &DOFCalculusCore{epsilon: 1e-6}
}

// netDeltaTolerance is the v0.8 constant that decides whether two candidates'
// NetDelta are tied (§10). The index is a sum of logarithms over a SET, so two
// ports that iterate their container in different orders can disagree in the last
// bits (~1e-15) while agreeing on every derivation — and a tie must be resolved
// identically everywhere, because §7 requires the same CHOICE, not only the same
// numbers.
const netDeltaTolerance = 1e-9

func (c *DOFCalculusCore) isIncluded(entity *EntityState, ctx *ObservationContext, state *SystemStateMatrix) bool {
	// Excluded if it is a **witnessed** collapse source, or if its DoF is a known
	// zero whose recoverability verdict is `proven_unreachable` (§4.2/§4.9). A
	// node with an unknown DoF is never excluded (Axiom 5), and neither is a node
	// whose verdict is `reachable` or `undetermined` — incompleteness of an
	// observation is never read as proof.
	if entity.IsCollapseSource && c.labelWitnessed(entity, ctx, state) {
		return false // aggressors leave the topology
	}
	return c.isIncludedWithoutLabel(entity, ctx)
}

// isIncludedWithoutLabel is `calc` membership with the collapse-source label NOT
// honoured (§4.2). Used in two places, and it must be the same rule in both:
// deciding who is counted, and deciding whether a label has a witness. The
// witness question is "would this entity be counted if its own label were
// ignored" — asking it with the label already applied would be circular, and
// would make every label unfalsifiable.
func (c *DOFCalculusCore) isIncludedWithoutLabel(entity *EntityState, ctx *ObservationContext) bool {
	if entity.CurrentDoF > 0.0 {
		return true
	}
	if !entity.DoFKnown {
		return true
	}
	if ctx == nil {
		return true // fail-safe: no observation, no proof
	}
	return ctx.verdict(entity.EntityID) != "proven_unreachable"
}

// labelWitnessed: a label is honoured only with a machine-verifiable act (§4.2).
// The act must be performed by this entity and must drive an entity that would
// otherwise be counted to a known zero. A flag without such an act is not a
// verdict — otherwise the label itself would raise the index.
func (c *DOFCalculusCore) labelWitnessed(entity *EntityState, ctx *ObservationContext, state *SystemStateMatrix) bool {
	if ctx == nil || state == nil {
		return false
	}
	counted := map[string]bool{}
	for eid, other := range state.Entities {
		if c.isIncludedWithoutLabel(other, ctx) {
			counted[eid] = true
		}
	}
	if !counted[entity.EntityID] {
		return false
	}
	dofBefore := map[string]float64{}
	for eid, other := range state.Entities {
		dofBefore[eid] = other.CurrentDoF
	}
	acts := map[string]bool{}
	for _, id := range ctx.World.CollapseActs(counted, dofBefore) {
		acts[id] = true
	}
	for _, a := range ctx.World.Acts {
		if a.Source == entity.EntityID && acts[a.ID] {
			return true
		}
	}
	return false
}

func (c *DOFCalculusCore) calcMembers(state *SystemStateMatrix, ctx *ObservationContext) map[string]bool {
	members := make(map[string]bool, len(state.Entities))
	for eid, entity := range state.Entities {
		if c.isIncluded(entity, ctx, state) {
			members[eid] = true
		}
	}
	return members
}

func (c *DOFCalculusCore) CalculateSystemDoF(state *SystemStateMatrix, members map[string]bool, ctx *ObservationContext) float64 {
	if members == nil {
		members = c.calcMembers(state, ctx)
	}
	total := 0.0
	for eid := range members {
		entity, ok := state.Entities[eid]
		if !ok {
			continue
		}
		total += math.Log(math.Max(entity.CurrentDoF, c.epsilon))
	}
	return total
}

func (c *DOFCalculusCore) coerceDoF(v float64) float64 {
	if v < 0.0 {
		return 0.0
	}
	if v > 1.0 {
		return 1.0
	}
	return v
}

// dofAfterClosure recomputes DoF from the counters after the option's closure
// (§4.3, §4.4). Only the Variety share moves, so the whole product moves by its
// ratio: the other lenses (and any u(t) factors) are untouched by a closure.
// A nil result means the entity is not affected or its Variety lens is unmeasured.
//
// §4.10 (v0.11): the closure list is read **under `hypothesisID`** — with a
// per-hypothesis `closed` the same option destroys different transitions under
// different readings, and a quantity of §4.3–§4.4 that read the flat list would
// silently decide every reading by the observed one.
func (c *DOFCalculusCore) dofAfterClosureFor(entity *EntityState, option *ActionOption, ctx *ObservationContext, hypothesisID string) *float64 {
	m := entity.Measurement
	if m == nil || m.Psi["variety"] == nil || len(m.VarietyCounters) == 0 {
		return nil
	}
	vEnv := m.VarietyCounters["V_env"]
	varBefore := *m.Psi["variety"]
	vBefore := ctx.vBefore(entity.EntityID)
	vAfter := ctx.vAfterClosure(entity.EntityID, option.ClosedFor(hypothesisID))
	if vAfter == vBefore {
		return nil // this entity is not affected
	}
	v := c.coerceDoF(entity.CurrentDoF / varBefore * PsiVar(float64(vAfter), vEnv))
	return &v
}

// dofAfterClosure is the **observed-reading** entry point (absence of a set is
// the observed singleton, §3.6).
func (c *DOFCalculusCore) dofAfterClosure(entity *EntityState, option *ActionOption, ctx *ObservationContext) *float64 {
	return c.dofAfterClosureFor(entity, option, ctx, ObservedHypothesisID)
}

// projectedDoF is the DoF this option would leave the entity with, closure
// included (§4.3). ONE definition, used by both `simulate` and
// `collapseCharges`: if the charge were computed from the raw delta while the
// index was computed from the closure-aware value, an option that destroys an
// entity BY CLOSING ITS TRANSITIONS would be scored as a collapse and charged as
// nothing — the structural gate of §4.5 would then pass exactly the option it
// exists to stop. Two call sites, one rule.
//
// Both the raw delta and the closure list are read **under `hypothesisID`**
// (§3.3, §4.4): a per-hypothesis option projects a different DoF under each
// reading, and the closure that the projection is corrected by must be the
// closure of the same reading.
func (c *DOFCalculusCore) projectedDoFFor(eState *EntityState, option *ActionOption, ctx *ObservationContext, hypothesisID string) float64 {
	newDoF := c.coerceDoF(eState.CurrentDoF + option.DeltaFor(hypothesisID, eState.EntityID))
	if ctx != nil && len(option.ClosedFor(hypothesisID)) > 0 {
		if recomputed := c.dofAfterClosureFor(eState, option, ctx, hypothesisID); recomputed != nil {
			newDoF = *recomputed
		}
	}
	return newDoF
}

// projectedDoF is the observed-reading entry point.
func (c *DOFCalculusCore) projectedDoF(eState *EntityState, option *ActionOption, ctx *ObservationContext) float64 {
	return c.projectedDoFFor(eState, option, ctx, ObservedHypothesisID)
}

// allClosedLists is every closure list the option declares: one for a flat
// option, one per reading for a per-hypothesis option (§3.3, §4.4). The
// declaration guards below must hold under **every** reading, so they range over
// these lists and never over the flat field alone.
func (o *ActionOption) allClosedLists() [][]ClosedRef {
	if o.ClosureForm() != "per_hypothesis" {
		return [][]ClosedRef{o.Closed}
	}
	ids := make([]string, 0, len(o.ClosedByHypothesis))
	for id := range o.ClosedByHypothesis {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	out := make([][]ClosedRef, 0, len(ids))
	for _, id := range ids {
		out = append(out, o.ClosedByHypothesis[id])
	}
	return out
}

func (c *DOFCalculusCore) validateClosure(option *ActionOption) error {
	for _, lst := range option.allClosedLists() {
		if len(lst) > 0 && option.ActID != "" {
			for _, ref := range lst {
				if ref.Kind == "act" && ref.ID == option.ActID {
					return fmt.Errorf("%s: closes its own execution path (§4.4 guard 1)", option.OptionID)
				}
			}
		}
	}
	if option.ClosureForm() == "flat" && len(option.Closed) == 0 && !option.IsReversible {
		return fmt.Errorf("%s: is_reversible=false with an empty closure list (§4.4 guard 2)", option.OptionID)
	}
	return nil
}

// IsReversible: the reported flag is DERIVED — true exactly when nothing is closed.
//
// The flag is read under a **reading** (§4.10): with a per-hypothesis `closed`
// the same option is reversible under one hypothesis and not under another, and
// the robust reading of §4.5 key 3 is the conjunction over `H_plausible`
// (`RobustReversible`). This entry point is the observed reading.
func (c *DOFCalculusCore) IsReversible(option *ActionOption) bool {
	return c.IsReversibleFor(option, ObservedHypothesisID)
}

// IsReversibleFor is §4.5 key 3 under one named reading.
func (c *DOFCalculusCore) IsReversibleFor(option *ActionOption, hypothesisID string) bool {
	return len(option.ClosedFor(hypothesisID)) == 0
}

func (c *DOFCalculusCore) simulateFor(current *SystemStateMatrix, option *ActionOption, ctx *ObservationContext, hypothesisID string) (*SystemStateMatrix, map[string]bool) {
	members := c.calcMembers(current, ctx)
	simulated := make(map[string]*EntityState, len(current.Entities))
	for eid, eState := range current.Entities {
		newDoF := c.projectedDoFFor(eState, option, ctx, hypothesisID)
		ent := *eState
		ent.CurrentDoF = newDoF
		simulated[eid] = &ent
	}
	return &SystemStateMatrix{
		GlobalTimeToCollapseMks: current.GlobalTimeToCollapseMks,
		ContextSwitchCost:       current.ContextSwitchCost,
		Entities:                simulated,
		Psi:                     current.Psi,
		Resources:               current.Resources,
	}, members
}

// simulate is the observed-reading entry point.
func (c *DOFCalculusCore) simulate(current *SystemStateMatrix, option *ActionOption, ctx *ObservationContext) (*SystemStateMatrix, map[string]bool) {
	return c.simulateFor(current, option, ctx, ObservedHypothesisID)
}

func (c *DOFCalculusCore) collapseChargesFor(current *SystemStateMatrix, option *ActionOption, ctx *ObservationContext, hypothesisID string) []CollapseCharge {
	charges := []CollapseCharge{}
	members := c.calcMembers(current, ctx)
	for eid, entity := range current.Entities {
		if !members[eid] || !entity.DoFKnown {
			continue
		}
		// The projected value is the closure-aware one (§4.3): an option can
		// destroy a counted entity by closing its transitions while declaring no
		// delta at all, and that is exactly the case §4.5 must catch — under the
		// reading being evaluated (§4.10).
		newDoF := c.projectedDoFFor(entity, option, ctx, hypothesisID)
		// §4.2: a charge requires a *transition* into the zero, not a stay at
		// it. An entity already at a known zero was not destroyed by this option
		// — charging it would make every option destructive in any state that
		// contains a recoverable zero (an entity kept in `calc` by an
		// `undetermined` verdict, for instance).
		if newDoF == 0.0 && entity.CurrentDoF > 0.0 {
			charges = append(charges, CollapseCharge{EntityID: eid, DoFBefore: entity.CurrentDoF})
		}
	}
	sort.Slice(charges, func(i, j int) bool { return charges[i].EntityID < charges[j].EntityID })
	return charges
}

// collapseCharges is the observed-reading entry point.
func (c *DOFCalculusCore) collapseCharges(current *SystemStateMatrix, option *ActionOption, ctx *ObservationContext) []CollapseCharge {
	return c.collapseChargesFor(current, option, ctx, ObservedHypothesisID)
}

// ApplyStructuralGate: §4.5. An option that destroys a counted entity is
// inadmissible while a charge-free candidate exists; every removal is recorded
// (§6.2). The charge is taken against `calc(S)`, and `calc` depends on the
// observation — so the observation must reach the gate, or the gate would filter
// a different world than the one the index was scored in.
func (c *DOFCalculusCore) ApplyStructuralGate(current *SystemStateMatrix, options []*ActionOption, ctx *ObservationContext) ([]*ActionOption, []RemovedOption) {
	if len(options) == 0 {
		return nil, nil
	}
	chargeFree := false
	for _, option := range options {
		if len(c.collapseCharges(current, option, ctx)) == 0 {
			chargeFree = true
			break
		}
	}
	if !chargeFree {
		// No alternative exists: Axiom 3 still forbids preferring destruction,
		// but with every candidate destructive the ladder decides (rung 1).
		return options, nil
	}
	admissible := []*ActionOption{}
	removed := []RemovedOption{}
	for _, option := range options {
		if len(c.collapseCharges(current, option, ctx)) == 0 {
			admissible = append(admissible, option)
		} else {
			removed = append(removed, RemovedOption{OptionID: option.OptionID, Gate: "collapse"})
		}
	}
	return admissible, removed
}

// RETIRED in v0.8: the live path no longer calls this. A charged candidate is
// evaluated, reported in full, and made inadmissible by the vector test of §4.5
// (`SelectCandidate`), so `removed_options` carries no structural removal. Kept
// because the v0.6 harness asserts the rule that was in force then, and history
// must stay reproducible.
//
// ---------------------------------------------------------------------------
// §4.5 (v0.8): the candidate vector and the ordered test
// ---------------------------------------------------------------------------

// criticalMembers is the set of entities of calc(S) whose current_dof is the
// minimum over calc(S) (§4.5). A set, not a node: a minimum attained by several
// known zeros has no unique "critical node", and a flag would have to invent a
// tie-break by entity_id.
func (c *DOFCalculusCore) criticalMembers(state *SystemStateMatrix, ctx *ObservationContext) map[string]bool {
	out := map[string]bool{}
	lowest := math.Inf(1)
	found := false
	for id := range c.calcMembers(state, ctx) {
		ent, ok := state.Entities[id]
		if !ok {
			continue
		}
		if ent.CurrentDoF < lowest {
			lowest = ent.CurrentDoF
		}
		found = true
	}
	if !found {
		return out
	}
	for id := range c.calcMembers(state, ctx) {
		ent, ok := state.Entities[id]
		if ok && ent.CurrentDoF == lowest {
			out[id] = true
		}
	}
	return out
}

// lostPaths is the number of entities this option drops out of a `reachable`
// verdict, line by line (§4.5, §6.3).
//
// The verdict procedure runs twice over the SAME observation — once as observed,
// once with the option's closure applied — so a verdict can only move away from
// `reachable`, and the difference is computed rather than declared. A lost
// witness is a loss: an entity that leaves `reachable` counts even where no
// exclusion follows from it, because §4.2 excludes only on a proven_unreachable
// verdict over a complete observation.
func (c *DOFCalculusCore) lostPathsFor(state *SystemStateMatrix, option *ActionOption, ctx *ObservationContext, hypothesisID string) []LostPathEntry {
	out := []LostPathEntry{}
	closed := option.ClosedFor(hypothesisID)
	if ctx == nil || ctx.World == nil || len(closed) == 0 {
		return out
	}
	closedWorld := ctx.World.WithClosed(closed)
	critical := c.criticalMembers(state, ctx)
	for _, id := range sortedKeys(state.Entities) {
		ent := state.Entities[id]
		beforeDoF := ent.CurrentDoF
		before := ctx.World.VerdictWithDoF(id, ctx.MeansClass, ctx.horizon(id), &beforeDoF)
		if before.Verdict != "reachable" {
			continue
		}
		// §4.5/§7 item 34: the second verdict reads the state the option LEAVES
		// BEHIND — the counters the closure changed, with the lens values
		// recomputed from them — and reads them **without** the option's
		// `projected_dof_delta`. D2 asks "did the closure destroy a recovery
		// path?", not "is the entity better off after the option's promised
		// effect?" — the latter is NetDelta. Were the projection admitted here,
		// an option could raise the after-state DoF with its own promise and buy
		// back the very recoverability it destroys. That is why the base DoF of
		// the second run is `dofAfterClosure` and never `projectedDoF`; a nil
		// result means the closure did not touch this entity, which then keeps
		// its observed DoF.
		//
		// §6.3: the after-state is **this reading's** own — the counters are
		// recomputed from `closure[h]`, the same list the graph above was pruned
		// by. Reading `closure[$observed$]` here (as this port did before) built a
		// state no hypothesis produces: the graph of one reading with the counters
		// of another, which is exactly what §6.3's "MUST NOT read one reading's
		// closures against another reading's state" forbids.
		afterDoF := c.dofAfterClosureFor(ent, option, ctx, hypothesisID)
		after := closedWorld.VerdictWithDoF(id, ctx.MeansClass, ctx.horizon(id), afterDoF)
		if after.Verdict == "reachable" {
			continue
		}
		witness := make([]string, len(before.Witness))
		copy(witness, before.Witness)
		out = append(out, LostPathEntry{
			EntityID: id, VerdictBefore: before.Verdict, VerdictAfter: after.Verdict,
			Critical: critical[id], WitnessLost: witness,
		})
	}
	return out
}

// lostPaths is the observed-reading entry point.
func (c *DOFCalculusCore) lostPaths(state *SystemStateMatrix, option *ActionOption, ctx *ObservationContext) []LostPathEntry {
	return c.lostPathsFor(state, option, ctx, ObservedHypothesisID)
}

// candidateVectorFor computes the keys of one candidate (§4.5) **under one
// reading** (§4.10): every quantity of §4.1–§4.5 is conditional, so the
// simulation, the collapse charges, the lost paths and the reversibility flag
// are all read under `hypothesisID`.
func (c *DOFCalculusCore) candidateVectorFor(state *SystemStateMatrix, option *ActionOption, ctx *ObservationContext, currentIndex float64, hypothesisID string, viable, resourcesOK bool) ConditionalVector {
	simulated, members := c.simulateFor(state, option, ctx, hypothesisID)
	projected := c.CalculateSystemDoF(simulated, members, ctx)
	lost := c.lostPathsFor(state, option, ctx, hypothesisID)
	d3 := 0
	for _, row := range lost {
		if row.Critical {
			d3++
		}
	}
	return ConditionalVector{
		CandidateVector: CandidateVector{
			D1: len(c.collapseChargesFor(state, option, ctx, hypothesisID)), D2: len(lost), D3: d3,
			NetDelta: c.netDelta(state, option, projected, currentIndex),
			Reversible: c.IsReversibleFor(option, hypothesisID), OptionID: option.OptionID,
		},
		HypothesisID: hypothesisID, Viable: viable, ResourcesOK: resourcesOK,
	}
}

// candidateVector is the **observed reading** of `candidateVectorFor`: the same
// pipeline with `H = {$observed$}`, so the historical entry point and the
// conditional one cannot drift apart.
func (c *DOFCalculusCore) candidateVector(state *SystemStateMatrix, option *ActionOption, ctx *ObservationContext, currentIndex float64) CandidateVector {
	viability := c.Viability(state, option)
	plan := c.PlanFunding(state, option, nil, nil, nil, nil)
	return c.candidateVectorFor(state, option, ctx, currentIndex,
		ObservedHypothesisID, viability.Viable, plan.Covered).CandidateVector
}

// BaselineVector is staying put: the zero vector, NetDelta = 0 by definition.
func (c *DOFCalculusCore) BaselineVector() CandidateVector {
	return CandidateVector{D1: 0, D2: 0, D3: 0, NetDelta: 0.0, Reversible: true}
}

// BarringKey is the first dimension on which a candidate fails to beat staying
// put (§4.5, §6.2). nil means nothing barred it: it outranks the baseline, or
// ties it while staying reversible.
func (c *DOFCalculusCore) BarringKey(vector CandidateVector) *string {
	for _, key := range []string{"d1", "d2", "d3"} {
		if dimension(vector, key) > 0 {
			k := key
			return &k
		}
	}
	if vector.NetDelta <= 0.0 {
		k := "net_delta"
		return &k
	}
	return nil
}

func dimension(v CandidateVector, key string) int {
	switch key {
	case "d1":
		return v.D1
	case "d2":
		return v.D2
	default:
		return v.D3
	}
}

func (c *DOFCalculusCore) requirement(option *ActionOption) map[string]float64 {
	net := make(map[string]float64)
	for _, entityDeltas := range option.ProjectedResourceDelta {
		for resource, delta := range entityDeltas {
			net[resource] += delta
		}
	}
	need := make(map[string]float64)
	for r, val := range net {
		if val < 0.0 {
			need[r] = -val
		}
	}
	return need
}

func (c *DOFCalculusCore) sameGroup(a, b string, groups [][]string) bool {
	if a == b {
		return true
	}
	for _, group := range groups {
		hasA, hasB := false, false
		for _, m := range group {
			if m == a {
				hasA = true
			}
			if m == b {
				hasB = true
			}
		}
		if hasA && hasB {
			return true
		}
	}
	return false
}

type FundingResult struct {
	Covered           bool
	Need              map[string]float64
	Spend             map[string]float64
	Conversions       []map[string]interface{}
	Uncovered         map[string]float64
	MandateExceeded   float64
	TotalDurationMks float64
}

func (c *DOFCalculusCore) PlanFunding(state *SystemStateMatrix, option *ActionOption, groups [][]string, rates map[string]RateInfo, weights map[string]float64, cap *float64) FundingResult {
	need := c.requirement(option)
	means := state.Resources
	// §3.2b/§4.8 (v0.11): τ is read from the resource map as a **signed** value —
	// a negative τ is a passed deadline, not a zero — and `null` means unmeasured.
	// The deprecated `global_time_to_collapse_mks` mirror is never an input to a
	// rule (§3.1, §4.7, §4.8b): an unknown budget licenses no spending beyond the
	// measured balance, so no exchange is used to cover a deficit.
	tau := TauOf(state)
	// The numeraire weights: used to choose an offer canonically and to express
	// the mandate ceiling in one unit.
	w := func(r string) float64 {
		if weights == nil {
			return 1.0
		}
		if v, ok := weights[r]; ok {
			return v
		}
		return 1.0
	}
	spend := make(map[string]float64)
	conversions := []map[string]interface{}{}
	uncovered := make(map[string]float64)
	totalDuration := option.EstimatedDurationMks

	sortedNeed := make([]string, 0, len(need))
	for r := range need {
		sortedNeed = append(sortedNeed, r)
	}
	sort.Strings(sortedNeed)

	for _, resource := range sortedNeed {
		remaining := need[resource]
		resObs := means[resource]
		if resObs == nil {
			resObs = &ResourceObservation{}
		}
		// v0.9: stale resources MUST be re-measured before use (§3.2a/§4.8).
		available := math.Max(0.0, ResourceValue(resObs, false)-spend[resource])
		direct := math.Min(remaining, available)
		spend[resource] += direct
		remaining -= direct

		// §4.8 (v0.7): the offer is chosen CANONICALLY — the cheapest in the
		// group numeraire first, then the shorter exchange, then the key.
		type offer struct {
			cost, duration      float64
			key, source         string
			amountSource, rate  float64
		}
		offers := []offer{}
		sortedRates := make([]string, 0, len(rates))
		for r := range rates {
			sortedRates = append(sortedRates, r)
		}
		sort.Strings(sortedRates)
		for _, key := range sortedRates {
			parts := strings.Split(key, "->")
			if len(parts) != 2 {
				continue
			}
			source, target := parts[0], parts[1]
			if target != resource {
				continue
			}
			rateSpec := rates[key]
			if rateSpec.Rate <= 0.0 || !c.sameGroup(source, resource, groups) {
				continue
			}
			amountSource := remaining / rateSpec.Rate
			srcObs := means[source]
			if srcObs == nil {
				srcObs = &ResourceObservation{}
			}
			if amountSource > math.Max(0.0, ResourceValue(srcObs, false)-spend[source]) {
				continue // the price is not payable
			}
			if tau == nil || totalDuration+rateSpec.DurationMks > *tau {
				continue // does not fit in τ (or τ is unknown: no budget to spend)
			}
			offers = append(offers, offer{cost: w(source) * amountSource,
				duration: rateSpec.DurationMks, key: key, source: source,
				amountSource: amountSource, rate: rateSpec.Rate})
		}
		if remaining > 0.0 && len(offers) > 0 {
			best := offers[0]
			for _, o := range offers[1:] {
				better := o.cost < best.cost ||
					(o.cost == best.cost && (o.duration < best.duration ||
						(o.duration == best.duration && o.key < best.key)))
				if better {
					best = o
				}
			}
			spend[best.source] += best.amountSource
			totalDuration += best.duration
			conversions = append(conversions, map[string]interface{}{
				"from":        best.source,
				"to":          resource,
				"amount_from": best.amountSource,
				"amount_to":   remaining,
				"rate":        best.rate,
				"duration_mks": best.duration,
			})
			remaining = 0.0
		}
		if remaining > 0.0 {
			uncovered[resource] = remaining
		}
	}

	// §4.8 (v0.7): the mandate caps what may be spent, in the group numeraire.
	// It can only remove an option a larger balance would have paid for, and it
	// can never make payable what the measured means cannot cover.
	mandateExceeded := 0.0
	if cap != nil {
		spent := 0.0
		for r, amount := range spend {
			spent += w(r) * amount
		}
		if spent > *cap {
			mandateExceeded = spent - *cap
		}
	}
	return FundingResult{
		Covered:           len(uncovered) == 0 && mandateExceeded <= 0.0,
		Need:              need,
		Spend:             spend,
		Conversions:       conversions,
		Uncovered:         uncovered,
		MandateExceeded:   mandateExceeded,
		TotalDurationMks: totalDuration,
	}
}

// ApplyResourceGate: §4.8 step 3. An unpayable option is inadmissible,
// unconditionally — there is no "no alternative" escape, because a shortage that
// survives full verified conversion is a VERDICT, not a price, and cannot be
// traded against a preference for acting. Every removal is recorded (§6.2).
func (c *DOFCalculusCore) ApplyResourceGate(state *SystemStateMatrix, options []*ActionOption, groups [][]string, rates map[string]RateInfo, weights map[string]float64, cap *float64) ([]*ActionOption, []RemovedOption) {
	if len(options) == 0 {
		return nil, nil
	}
	admissible := []*ActionOption{}
	removed := []RemovedOption{}
	for _, option := range options {
		if c.PlanFunding(state, option, groups, rates, weights, cap).Covered {
			admissible = append(admissible, option)
		} else {
			removed = append(removed, RemovedOption{OptionID: option.OptionID, Gate: "insolvency"})
		}
	}
	return admissible, removed
}

func (c *DOFCalculusCore) netDelta(current *SystemStateMatrix, option *ActionOption, projected, currentDoF float64) float64 {
	// §4.4 (v0.7): no flat penalty. An irreversible option's price is already
	// inside `projected_dof`, because the closure lowered the affected entities'
	// Variety counter in `S'` (§4.3); subtracting anything here would charge the
	// same loss twice.
	return projected - currentDoF - current.ContextSwitchCost
}

// EvaluateAndSelect is the v0.8 selection (§4.5): admissibility first, the index
// second. It returns the winner, or nil when the system stays — a decision and
// not an absence of one.
func (c *DOFCalculusCore) EvaluateAndSelect(currentState *SystemStateMatrix, options []*ActionOption, ctx *ObservationContext) *ActionOption {
	selected, _ := c.SelectCandidate(currentState, options, ctx)
	return selected
}

// SelectCandidate returns the winner and every candidate's vector.
//
// Staying put is a candidate LIKE ANY OTHER, so its zero vector enters the set:
// that is what makes a protected dimension a BAR instead of a comparison. Any
// candidate with d1, d2 or d3 above zero loses to it, and no candidate can ever
// be preferred for cutting a path. Comparing against the baseline only at the
// NetDelta step would let a positive delta buy a lost path back — exactly the
// defect this release removes.
func (c *DOFCalculusCore) SelectCandidate(currentState *SystemStateMatrix, options []*ActionOption, ctx *ObservationContext) (*ActionOption, []CandidateVector) {
	vectors := []CandidateVector{}
	if len(options) == 0 {
		return nil, vectors
	}
	type entry struct {
		option *ActionOption // nil = staying put
		vector CandidateVector
	}
	current := c.CalculateSystemDoF(currentState, nil, ctx)
	survivors := []entry{}
	for _, option := range options {
		vector := c.candidateVector(currentState, option, ctx, current)
		vectors = append(vectors, vector)
		survivors = append(survivors, entry{option, vector})
	}
	survivors = append(survivors, entry{nil, c.BaselineVector()})

	// 1. Structural admissibility: D1 = D2 = D3 = 0. Inadmissible candidates are
	//    never compared with one another.
	for _, key := range []string{"d1", "d2", "d3"} {
		if len(survivors) == 0 {
			break
		}
		best := dimension(survivors[0].vector, key)
		for _, e := range survivors {
			if v := dimension(e.vector, key); v < best {
				best = v
			}
		}
		kept := []entry{}
		for _, e := range survivors {
			if dimension(e.vector, key) == best {
				kept = append(kept, e)
			}
		}
		survivors = kept
	}
	// 2. The index, ties grouped with the tolerance of §10.
	if len(survivors) > 0 {
		best := survivors[0].vector.NetDelta
		for _, e := range survivors {
			if e.vector.NetDelta > best {
				best = e.vector.NetDelta
			}
		}
		kept := []entry{}
		for _, e := range survivors {
			if math.Abs(e.vector.NetDelta-best) <= netDeltaTolerance {
				kept = append(kept, e)
			}
		}
		survivors = kept
	}
	// 3. Reversibility: a preference among equals, not a penalty (§4.4).
	anyReversible := false
	for _, e := range survivors {
		if e.vector.Reversible {
			anyReversible = true
		}
	}
	if anyReversible {
		kept := []entry{}
		for _, e := range survivors {
			if e.vector.Reversible {
				kept = append(kept, e)
			}
		}
		survivors = kept
	}
	// 4. A complete tie goes to staying put, if it is still a candidate.
	for _, e := range survivors {
		if e.option == nil {
			return nil, vectors
		}
	}
	if len(survivors) > 0 {
		bestID := survivors[0].vector.OptionID
		for _, e := range survivors {
			if e.vector.OptionID < bestID {
				bestID = e.vector.OptionID
			}
		}
		kept := []entry{}
		for _, e := range survivors {
			if e.vector.OptionID == bestID {
				kept = append(kept, e)
			}
		}
		survivors = kept
	}
	if len(survivors) == 0 {
		return nil, vectors
	}
	winner := survivors[0]
	// The survivor is selected only if it beats the baseline. With the baseline in
	// the set this is already implied; the guard states the rule.
	if winner.vector.NetDelta <= 0.0 {
		return nil, vectors
	}
	return winner.option, vectors
}

func (c *DOFCalculusCore) isIncomplete(state *SystemStateMatrix, options []*ActionOption) bool {
	cheapest := math.Inf(1)
	for _, option := range options {
		if option.EstimatedDurationMks > 0.0 && option.EstimatedDurationMks < cheapest {
			cheapest = option.EstimatedDurationMks
		}
	}
	if math.IsInf(cheapest, 1) {
		return false
	}
	for eid, entity := range state.Entities {
		if entity.DoFKnown {
			continue
		}
		touched := false
		for _, option := range options {
			if option.ProjectedDoFDelta[eid] != 0.0 {
				touched = true
				break
			}
		}
		if touched {
			continue
		}
		// §4.7/§5 (v0.11): the window is read under τ from the resource map. An
		// unknown τ has **no** window — `null`, not a closed one — so the strict
		// `t* > 0` rule of §5 is not applied to it, and a resolvable unknown is
		// not declared resolvable on a budget that was never measured.
		if window := MeasurementWindow(state, cheapest); window != nil && *window > 0.0 {
			return true
		}
	}
	return false
}

// recoverabilityRow is the verdict, its witness and the completeness claim
// behind it (§6.1). A `proven_unreachable` verdict without a witness is not a
// verdict, so the report carries both — and names the observation, because "no
// path" is only meaningful together with "and the observation was complete for
// this entity".
func (c *DOFCalculusCore) recoverabilityRow(entityID string, ctx *ObservationContext) map[string]interface{} {
	if ctx == nil {
		return map[string]interface{}{"verdict": "undetermined", "witness": []string{},
			"horizon_mks": nil, "observation": "unobserved", "admissible_seen": 0,
			"reason": "no observation was supplied for this cycle"}
	}
	v := ctx.World.Verdict(entityID, ctx.MeansClass, ctx.horizon(entityID))
	observation := "unobserved"
	if node, ok := ctx.World.Entities[entityID]; ok {
		observation = node.Observation
	}
	var horizon interface{}
	if h := ctx.horizon(entityID); h != nil {
		horizon = *h
	}
	return map[string]interface{}{
		"verdict": v.Verdict, "witness": v.Witness, "horizon_mks": horizon,
		"observation": observation, "admissible_seen": v.AdmissibleSeen, "reason": v.Reason,
	}
}

// closureShare is the per-entity decomposition of a closure's price (§6.3). This
// is a *decomposition* of the loss that is already inside `NetDelta`
// (§4.3/§4.4), never an extra charge: it exists so a reader can see which entity
// lost which share, and by how much.
func (c *DOFCalculusCore) closureShare(state *SystemStateMatrix, option *ActionOption, ctx *ObservationContext) map[string]float64 {
	out := map[string]float64{}
	if ctx == nil || len(option.Closed) == 0 {
		return out
	}
	for eID, ent := range state.Entities {
		m := ent.Measurement
		if m == nil || m.Psi["variety"] == nil || len(m.VarietyCounters) == 0 {
			continue
		}
		vEnv := m.VarietyCounters["V_env"]
		vAfter := ctx.vAfterClosure(eID, option.Closed)
		vBefore := ctx.vBefore(eID)
		if vAfter == vBefore {
			continue
		}
		after := math.Max(PsiVar(float64(vAfter), vEnv), c.epsilon)
		before := math.Max(PsiVar(float64(vBefore), vEnv), c.epsilon)
		out[eID] = q6(math.Log(after) - math.Log(before))
	}
	return out
}

// report is the transparent audit (DOF-SPEC §6). Required by the license (PoI).
func (c *DOFCalculusCore) Report(currentState *SystemStateMatrix, options []*ActionOption, selected *ActionOption, mode string, in ReportInput) *DofReport {
	ctx := in.Ctx
	var entityRows []EntityReportRow
	for _, ent := range currentState.Entities {
		included := c.isIncluded(ent, ctx, currentState)
		contribution := 0.0
		if included {
			contribution = math.Log(math.Max(ent.CurrentDoF, c.epsilon))
		}
		row := EntityReportRow{
			EntityID:         ent.EntityID,
			IsCollapseSource: ent.IsCollapseSource,
			IncludedInSum:    included,
			CurrentDoF:       ent.CurrentDoF,
			DoFKnown:         ent.DoFKnown,
			Contribution:     contribution,
			Recoverability:   c.recoverabilityRow(ent.EntityID, ctx),
		}
		if ent.Measurement != nil {
			row.LensTerms = ent.Measurement.Terms
			row.BindingLens = ent.Measurement.BindingLens
			row.Floored = ent.Measurement.Floored
			row.Blocks = ent.Measurement.Blocks
			row.Derivation = ent.Measurement.Derivation
		}
		entityRows = append(entityRows, row)
	}
	total := c.CalculateSystemDoF(currentState, nil, ctx)
	var optionRows []OptionReportRow
	for _, option := range options {
		simulated, members := c.simulate(currentState, option, ctx)
		projected := c.CalculateSystemDoF(simulated, members, ctx)
		vector := c.candidateVector(currentState, option, ctx, total)
		net := vector.NetDelta
		isSelected := selected != nil && selected.OptionID == option.OptionID
		plan := c.PlanFunding(currentState, option, in.Groups, in.Rates, in.Weights, in.Cap)
		optionRows = append(optionRows, OptionReportRow{
			OptionID:             option.OptionID,
			IsReversible:         c.IsReversible(option),
			ProjectedDoF:         projected,
			NetDelta:             net,
			Selected:             isSelected,
			EstimatedDurationMks: option.EstimatedDurationMks,
			CollapseCharges:      c.collapseCharges(currentState, option, ctx),
			ResourceConsumption:  option.ProjectedResourceDelta,
			ConversionApplied:    plan.Conversions,
			ResourcesUncovered:   plan.Uncovered,
			MandateExceeded:      plan.MandateExceeded,
			Closed:               option.Closed,
			ClosureShare:         c.closureShare(currentState, option, ctx),
			// §6.3 (v0.8): the protected dimensions, the key that barred the
			// candidate (nil when nothing did), and the path losses line by line.
			CandidateVector: vector,
			BarringKey:      c.BarringKey(vector),
			LostPaths:       c.lostPaths(currentState, option, ctx),
		})
	}

	resAfterCopy := make(map[string]*ResourceObservation)
	for k, v := range currentState.Resources {
		if v != nil {
			copy := *v
			resAfterCopy[k] = &copy
		}
	}
	if selected != nil {
		plan := c.PlanFunding(currentState, selected, in.Groups, in.Rates, in.Weights, in.Cap)
		for resource, amount := range plan.Spend {
			obs := resAfterCopy[resource]
			if obs == nil {
				continue
			}
			newVal := math.Max(0.0, ResourceValue(obs, false)-amount)
			obs.Value = &newVal
		}
	}
	resBefore := make(map[string]interface{})
	for k, v := range currentState.Resources {
		if v != nil {
			resBefore[k] = v
		}
	}
	resAfter := make(map[string]interface{})
	for k, v := range resAfterCopy {
		resAfter[k] = v
	}

	report := &DofReport{
		Entities:                entityRows,
		TotalSystemDoF:          total,
		ContextSwitchCost:       currentState.ContextSwitchCost,
		GlobalTimeToCollapseMks: currentState.GlobalTimeToCollapseMks,
		Mode:                    mode,
		Options:                 optionRows,
		RemovedOptions:          in.Removed,
		Incomplete:              c.isIncomplete(currentState, options),
		ResourcesBefore:         resBefore,
		ResourcesAfter:          resAfter,
		MeansProvenance:         in.MeansProvenance,
		// §6.2 (v0.8): what the candidates were compared against, and whether any
		// of them beat it. A silent "no action" is an omission.
		Baseline:          c.BaselineVector(),
		NoCandidateBetter: len(options) > 0 && selected == nil,
	}
	if report.MeansProvenance == nil {
		report.MeansProvenance = map[string]interface{}{}
	}
	if ctx != nil && ctx.ObservationDigest != "" {
		digest := ctx.ObservationDigest
		report.ObservationDigest = &digest
	}
	if in.Declaration != nil {
		report.PsiID = in.Declaration.PsiID
		report.PsiDigest = in.Declaration.Digest()
		report.Declaration = in.Declaration.CanonicalText()
	} else if currentState.Psi != nil {
		report.PsiID = currentState.Psi.ID
		report.PsiDigest = currentState.Psi.Digest
	}
	return report
}
