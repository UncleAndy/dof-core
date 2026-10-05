package main

import (
	"fmt"
	"math"
	"sort"
)

type RawObservation struct {
	IsAutonomous      bool
	AgencyIndex       float64
	IsCollapseSource  bool
	TimeToCollapseMks float64
	Lenses            LensObservation
}

type GraphMapper struct {
	ContextSwitchCost float64
	PsiID             string
	U0PriorQ          *float64
	LastDeclaration   *MeasurementDeclaration
	// The observation the state was decided over (§3.5/§4.9). Kept beside the
	// state, never inside it: a world graph is a Perception artifact, exactly
	// like the derived groups and the observed rates.
	LastObservation *ObservationContext
	// Mismatches between the declared derived numbers and what the named
	// procedures recompute over the observation (§4.6/§4.9). Empty means the
	// ruler is honest; a non-empty list means the declaration claimed a counter
	// its own observation does not support.
	LastGraphProblems []string
}

// Reserved keys of `raw` that describe the world/agent, not an entity.
var reservedKeys = map[string]bool{"resource_layer": true, "world": true}

func NewGraphMapper(contextSwitchCost float64) *GraphMapper {
	return &GraphMapper{ContextSwitchCost: contextSwitchCost, PsiID: "perception-v1"}
}

// parseWorld builds the §3.5 observation out of the reserved `world` key. It is
// an *observation*, so it arrives with the measurement and not inside the state:
// the same state plus a different observation is a different decision, and the
// report has to say which one was used.
func parseWorld(worldRaw map[string]interface{}) (*WorldGraph, []string, map[string]float64, *string, *float64, string) {
	graph := &WorldGraph{Entities: map[string]EntityNode{}, Means: []string{},
		Acts: []ActEdge{}, Exchanges: []ExchangeEdge{}}
	if ents, ok := worldRaw["entities"].(map[string]interface{}); ok {
		for eid, specRaw := range ents {
			spec, ok := specRaw.(map[string]interface{})
			if !ok {
				continue
			}
			node := EntityNode{ID: eid, Observation: "complete"}
			if v, ok := spec["id"].(string); ok {
				node.ID = v
			}
			if v, ok := spec["observation"].(string); ok {
				node.Observation = v
			}
			if v, ok := spec["current_dof"].(float64); ok {
				node.CurrentDoF = v
			}
			graph.Entities[eid] = node
		}
	}
	if rawMeans, ok := worldRaw["means"].([]interface{}); ok {
		for _, m := range rawMeans {
			if s, ok := m.(string); ok {
				graph.Means = append(graph.Means, s)
			}
		}
	}
	if rawActs, ok := worldRaw["acts"].([]interface{}); ok {
		for _, aRaw := range rawActs {
			a, ok := aRaw.(map[string]interface{})
			if !ok {
				continue
			}
			act := ActEdge{Effect: map[string]float64{}, Resources: map[string]float64{}}
			act.ID, _ = a["id"].(string)
			act.Source, _ = a["source"].(string)
			act.Target, _ = a["target"].(string)
			act.Category, _ = a["category"].(string)
			if req, ok := a["requires"].([]interface{}); ok {
				for _, r := range req {
					if s, ok := r.(string); ok {
						act.Requires = append(act.Requires, s)
					}
				}
			}
			if eff, ok := a["effect"].(map[string]interface{}); ok {
				for k, v := range eff {
					if f, ok := v.(float64); ok {
						act.Effect[k] = f
					}
				}
			}
			if res, ok := a["resources"].(map[string]interface{}); ok {
				for k, v := range res {
					if f, ok := v.(float64); ok {
						act.Resources[k] = f
					}
				}
			}
			act.DurationMks, _ = a["duration_mks"].(float64)
			graph.Acts = append(graph.Acts, act)
		}
	}
	if rawEx, ok := worldRaw["exchanges"].([]interface{}); ok {
		for _, eRaw := range rawEx {
			e, ok := eRaw.(map[string]interface{})
			if !ok {
				continue
			}
			ex := ExchangeEdge{Gives: map[string]float64{}, Wants: map[string]float64{}}
			ex.ID, _ = e["id"].(string)
			if g, ok := e["gives"].(map[string]interface{}); ok {
				for k, v := range g {
					if f, ok := v.(float64); ok {
						ex.Gives[k] = f
					}
				}
			}
			if w, ok := e["wants"].(map[string]interface{}); ok {
				for k, v := range w {
					if f, ok := v.(float64); ok {
						ex.Wants[k] = f
					}
				}
			}
			ex.DurationMks, _ = e["duration_mks"].(float64)
			graph.Exchanges = append(graph.Exchanges, ex)
		}
	}
	meansClass := []string{}
	if mc, ok := worldRaw["means_class"].([]interface{}); ok {
		for _, c := range mc {
			if s, ok := c.(string); ok {
				meansClass = append(meansClass, s)
			}
		}
	}
	tRec := map[string]float64{}
	if tr, ok := worldRaw["t_rec"].(map[string]interface{}); ok {
		for k, v := range tr {
			if f, ok := v.(float64); ok {
				tRec[k] = f
			}
		}
	}
	var numeraire *string
	if n, ok := worldRaw["numeraire"].(string); ok {
		numeraire = &n
	}
	var horizon *float64
	if ch, ok := worldRaw["counting_horizon_mks"].(float64); ok {
		horizon = &ch
	}
	procedure, _ := worldRaw["procedure"].(string)
	return graph, meansClass, tRec, numeraire, horizon, procedure
}

// verifyGraphDerived: §4.6/§4.9. Derived numbers MUST equal what their procedure
// computes. Recomputes the Variety counter and the reachability verdict of every
// entity against the declaration's M(S) and T_rec, and returns the mismatches
// (empty = the ruler is honest). A declaration that claims a counter its own
// observation does not support is exactly the "declared, not derived" defect this
// revision removes.
func verifyGraphDerived(declaration *MeasurementDeclaration, graph *WorldGraph, countingHorizon *float64) []string {
	problems := []string{}
	cats := declaration.MeansClass
	eids := make([]string, 0, len(declaration.Verdicts))
	for eid := range declaration.Verdicts {
		eids = append(eids, eid)
	}
	sort.Strings(eids)
	for _, entityID := range eids {
		declared := declaration.Verdicts[entityID]
		vHere := graph.VCount(entityID, cats, countingHorizon)
		if declared.V != vHere {
			problems = append(problems, fmt.Sprintf(
				"%s: declared V=%d but the counting procedure gives %d", entityID, declared.V, vHere))
		}
		var declaredVerdict string
		if d, ok := declaration.Verdicts[entityID]; ok {
			declaredVerdict = d.Verdict
		}
		verdictHere := graph.Verdict(entityID, cats, declared.TRecMks).Verdict
		if declaredVerdict != verdictHere {
			problems = append(problems, fmt.Sprintf(
				"%s: declared verdict %q but the verdict procedure returns %q",
				entityID, declaredVerdict, verdictHere))
		}
	}
	return problems
}

func (m *GraphMapper) PollEnvironment(raw map[string]interface{}) *SystemStateMatrix {
	var means_obs map[string]*ResourceObservation
	var groups [][]string
	var rates map[string]RateInfo
	var units []ResourceInfo
	var mandate map[string]interface{}
	// §3.2b (v0.11): the active individual deadlines. An entry whose value is
	// `null` is an UNMEASURED deadline and makes τ unknown — it is not absent.
	var declaredDeadlines map[string]*float64

	if layer, ok := raw["resource_layer"].(map[string]interface{}); ok {
		if m_raw, ok := layer["means"].(map[string]interface{}); ok {
			means_obs = make(map[string]*ResourceObservation)
			for k, v := range m_raw {
				switch val := v.(type) {
				case float64:
					means_obs[k] = &ResourceObservation{
						Value: &val, Unit: "unknown", Scale: 1.0,
						Source: "sensor", LastMeasuredAt: 0.0, AgingTime: 3600.0,
					}
				case map[string]interface{}:
					obs := &ResourceObservation{Unit: "unknown", Scale: 1.0}
					if vv, ok := val["value"]; ok {
						if f, ok := vv.(float64); ok {
							obs.Value = &f
						}
					}
					if u, ok := val["unit"].(string); ok {
						obs.Unit = u
					}
					if s, ok := val["scale"].(float64); ok {
						obs.Scale = s
					}
					if src, ok := val["source"].(string); ok {
						obs.Source = src
					}
					if l, ok := val["last_measured_at"].(float64); ok {
						obs.LastMeasuredAt = l
					}
					if a, ok := val["aging_time"].(float64); ok {
						obs.AgingTime = a
					}
					if e, ok := val["estimated"].(float64); ok {
						obs.Estimated = &e
					}
					means_obs[k] = obs
				}
			}
		}
		// §3.2b (v0.11): the individual deadlines declared alongside the map. A
		// `null` entry is an unmeasured ACTIVE deadline, not an absent one: it
		// makes τ unknown rather than letting the measured deadlines decide.
		if dl_raw, ok := layer["deadlines"].(map[string]interface{}); ok {
			declaredDeadlines = make(map[string]*float64, len(dl_raw))
			for k, v := range dl_raw {
				if f, ok := v.(float64); ok {
					fv := f
					declaredDeadlines[k] = &fv
				} else {
					declaredDeadlines[k] = nil
				}
			}
		}
		if g_raw, ok := layer["groups"].([]interface{}); ok {
			groups = make([][]string, 0, len(g_raw))
			for _, g := range g_raw {
				if g_list, ok := g.([]interface{}); ok {
					members := make([]string, 0, len(g_list))
					for _, m := range g_list {
						members = append(members, m.(string))
					}
					groups = append(groups, members)
				}
			}
		}
		if r_raw, ok := layer["rates"].(map[string]interface{}); ok {
			rates = make(map[string]RateInfo)
			for k, v := range r_raw {
				if v_map, ok := v.(map[string]interface{}); ok {
					rates[k] = RateInfo{
						Rate:        v_map["rate"].(float64),
						DurationMks: v_map["duration_mks"].(float64),
					}
				}
			}
		}
		if u_raw, ok := layer["resources"].([]interface{}); ok {
			units = make([]ResourceInfo, 0, len(u_raw))
			for _, u := range u_raw {
				if u_map, ok := u.(map[string]interface{}); ok {
					units = append(units, ResourceInfo{
						ID:    u_map["id"].(string),
						Unit:  u_map["unit"].(string),
						Scale: u_map["scale"].(float64),
					})
				}
			}
		}
		if mandate_raw, ok := layer["mandate"].(map[string]interface{}); ok {
			mandate = mandate_raw
		}
	}

	observations := make(map[string]LensObservation)
	minTTC := math.Inf(1)

	for eid, obsRaw := range raw {
		if reservedKeys[eid] {
			continue
		}
		obs := obsRaw.(map[string]interface{})

		var lenses LensObservation
		if l_raw, ok := obs["lenses"].(map[string]interface{}); ok {
			if v_raw, ok := l_raw["variety"].(map[string]interface{}); ok {
				lenses.Variety = &VarietyObs{V: v_raw["V"].(float64), VEnv: v_raw["V_env"].(float64)}
			}
			if c_raw, ok := l_raw["constraint"].(map[string]interface{}); ok {
				lenses.Constraint = &ConstraintObs{F: c_raw["F"].(float64), FEnv: c_raw["F_env"].(float64)}
			}
			if o_raw, ok := l_raw["options"].([]interface{}); ok {
				blocks := make([][2]float64, 0, len(o_raw))
				for _, b := range o_raw {
					if b_list, ok := b.([]interface{}); ok {
						blocks = append(blocks, [2]float64{b_list[0].(float64), b_list[1].(float64)})
					}
				}
				lenses.Options = &blocks
			}
			if r_raw, ok := l_raw["requirements"].(map[string]interface{}); ok {
				reqs := make(map[string]float64)
				for k, v := range r_raw {
					reqs[k] = v.(float64)
				}
				lenses.Requirements = reqs
			}
		}

		observations[eid] = lenses
		ttc, _ := obs["time_to_collapse_mks"].(float64)
		if !obs["is_collapse_source"].(bool) && ttc < minTTC {
			minTTC = ttc
		}
	}

	// §3.4.1/§3.4.2 (v0.11): the measurement durations are **declared ruler
	// content**. They are read from the observation, never inferred (§4.7), and
	// they are the same for every reading of the cycle — a hypothesis
	// reinterprets what was measured, not how long the measuring takes.
	measurementDurations := map[string]map[string]float64{}
	if d_raw, ok := raw["measurement_durations"].(map[string]interface{}); ok {
		for lens, dursRaw := range d_raw {
			durs, ok := dursRaw.(map[string]interface{})
			if !ok {
				continue
			}
			inner := map[string]float64{}
			for k, v := range durs {
				if f, ok := v.(float64); ok {
					inner[k] = f
				}
			}
			measurementDurations[lens] = inner
		}
	}
	measurementSchedule := map[string]float64{}
	if s_raw, ok := raw["measurement_schedule"].(map[string]interface{}); ok {
		for lens, v := range s_raw {
			if f, ok := v.(float64); ok {
				measurementSchedule[lens] = f
			}
		}
	}

	globalTTC := minTTC
	if math.IsInf(globalTTC, 1) {
		globalTTC = 1e15
	}

	// §3.2b (v0.11): τ is read from the **resource map**, and the individual
	// deadlines declared alongside it govern it. Three cases, in this order:
	//
	//   1. declared individual deadlines — τ is their minimum, and is `null` when
	//      **any** active deadline is unmeasured: taking the minimum over the
	//      measured ones alone would read an unknown timer as absent (§10(w),
	//      §10(ao));
	//   2. otherwise the `tau` observation of the map, which may be negative (a
	//      passed deadline keeps its magnitude, §10(p)) or `null`;
	//   3. otherwise the legacy entity-minimum, kept so the historical fixtures of
	//      `v0.6`-`v0.9.1` still read as they did.
	tauValue := resolveTau(declaredDeadlines, means_obs["tau"], globalTTC)
	// §3.2b/§10(s): the deprecated mirror is clamped and is **not** τ. It is `0.0`
	// for an unknown and for a passed deadline, and equals τ otherwise.
	mirrorTTC := MirrorTimeToCollapse(tauValue)

	// §3.5 (v0.7): the observed world graph, when the cycle was given one.
	var graph *WorldGraph
	var meansClass []string
	var tRec map[string]float64
	var numeraire *string
	var countingHorizon *float64
	graphProcedure := ""
	if worldRaw, ok := raw["world"].(map[string]interface{}); ok {
		graph, meansClass, tRec, numeraire, countingHorizon, graphProcedure = parseWorld(worldRaw)
	}
	if graph != nil && graphProcedure == "" {
		graphProcedure = m.PsiID + ":world_verdicts"
	}
	// §4.9: the counting horizon of the Variety procedure. A response vector must
	// be executable inside it, so the default is the cycle's own τ — the
	// observation may declare a different one, but never an implicit one.
	if graph != nil && countingHorizon == nil {
		h := globalTTC
		countingHorizon = &h
	}

	// §4.6 (v0.7): the numeraire weights, the observed rate table and the mandate
	// cap are DERIVED over the observation, not authored. Without a declared
	// numeraire there is no unit for a scalar cap, so neither applies — which is
	// what keeps a ruler without a world graph reading exactly as it did in v0.6.
	weights := map[string]float64{}
	var capValue *float64
	if graph != nil && numeraire != nil {
		members := map[string]bool{}
		for _, grp := range groups {
			for _, r := range grp {
				members[r] = true
			}
		}
		memberList := make([]string, 0, len(members))
		for r := range members {
			memberList = append(memberList, r)
		}
		sort.Strings(memberList)
		weights = graph.WeightsTo(*numeraire, memberList)
		// §3.5/§4.8: the axis rates are the *output* of the observation
		// procedure, so with a graph in hand the table is derived rather than read
		// from the layer. A declared table next to an observed graph would be a
		// second ruler for the same quantity, free to drift.
		derived := map[string]RateInfo{}
		for _, a := range memberList {
			for _, b := range memberList {
				if a == b {
					continue
				}
				res := graph.Rate(a, b, true)
				if res.Status == "observed" && res.Rate != nil {
					derived[a+"->"+b] = RateInfo{Rate: *res.Rate, DurationMks: res.DurationMks}
				}
			}
		}
		if len(derived) > 0 {
			rates = derived
		}
		limitKeys := make([]string, 0, len(mandate))
		for k := range mandate {
			limitKeys = append(limitKeys, k)
		}
		sort.Strings(limitKeys)
		limits := []float64{}
		for _, key := range limitKeys {
			switch key {
			case "cap":
				if v, ok := mandate[key].(float64); ok {
					limits = append(limits, v)
				}
			case "external_limit_credit":
				// Declared in credits, applied in the numeraire: converted
				// through the *observed* weight, never a hard-coded 1.0.
				if v, ok := mandate[key].(float64); ok {
					if wCredit, seen := weights["credit"]; seen {
						limits = append(limits, v*wCredit)
					}
				}
			}
		}
		if len(limits) > 0 {
			smallest := limits[0]
			for _, v := range limits[1:] {
				if v < smallest {
					smallest = v
				}
			}
			capValue = &smallest
		}
	}

	// §4.6/§4.9: the verdicts and the counters are computed by the named
	// procedures and then *declared*, so the declaration can be checked against
	// the observation it came from (`verifyGraphDerived`).
	verdicts := map[string]VerdictRecord{}
	if graph != nil {
		eids := make([]string, 0, len(observations))
		for eid := range observations {
			eids = append(eids, eid)
		}
		sort.Strings(eids)
		for _, eid := range eids {
			var horizon *float64
			if v, ok := tRec[eid]; ok {
				horizon = &v
			}
			verdict := graph.Verdict(eid, meansClass, horizon)
			verdicts[eid] = VerdictRecord{Verdict: verdict.Verdict, TRecMks: horizon,
				V: graph.VCount(eid, meansClass, countingHorizon)}
		}
	}

	// Pass 2: the declaration is frozen on S, so τ is known before measuring.
	declaration := NewDeclaration(m.PsiID, observations, tauValue, m.U0PriorQ, units, groups,
		rates, mandate, numeraire, weights, capValue, verdicts, meansClass, graphProcedure,
		measurementDurations)
	m.LastDeclaration = declaration
	u0 := declaration.U0()

	entities := make(map[string]*EntityState)
	for eid, obsRaw := range raw {
		if reservedKeys[eid] {
			continue
		}
		obs := obsRaw.(map[string]interface{})
		// v0.9: MeasureEntity expects map[string]float64, so we flatten ResourceObservation to floats.
		meansFlat := make(map[string]float64)
		for k, v := range means_obs {
			if v != nil && v.Value != nil {
				meansFlat[k] = *v.Value
			}
		}
		mz := MeasureEntity(eid, observations[eid], u0, meansFlat, groups, weights, capValue)
		entities[eid] = &EntityState{
			EntityID:          eid,
			IsAutonomous:      obs["is_autonomous"].(bool),
			AgencyIndex:       clamp01(obs["agency_index"].(float64)),
			CurrentDoF:        mz.CurrentDoF,
			IsCollapseSource:  obs["is_collapse_source"].(bool),
			DoFKnown:          mz.DoFKnown,
			TimeToCollapseMks: obs["time_to_collapse_mks"].(float64),
			Measurement:       &mz,
		}
	}

	// §3.5/§4.9: the observation itself, pinned by its own digest (§6.2), and the
	// self-check that the declared derived numbers are the ones the named
	// procedures actually return over it.
	m.LastObservation = nil
	m.LastGraphProblems = []string{}
	if graph != nil {
		m.LastObservation = &ObservationContext{
			World:              graph,
			MeansClass:         meansClass,
			TRec:               tRec,
			CountingHorizonMks: countingHorizon,
			ObservationDigest:  graph.ObservationDigest(meansClass, tRec, countingHorizon),
		}
		m.LastGraphProblems = verifyGraphDerived(declaration, graph, countingHorizon)
	}

	return &SystemStateMatrix{
		GlobalTimeToCollapseMks: mirrorTTC,
		ContextSwitchCost:       m.ContextSwitchCost,
		Entities:                entities,
		Psi:                     &PsiReference{ID: declaration.PsiID, Digest: declaration.Digest()},
		Resources:               means_obs,
		// §3.2b (v0.11): τ lives in the resource map — signed, and `null` when
		// unmeasured. The `tau` entry of `means` **is** this object, so a rule that
		// reads τ reads the map and not a field beside it.
		Tau: &ResourceObservation{
			Value: tauValue, Unit: "us", Scale: 1.0,
			Source: "resource_map", LastMeasuredAt: 0.0, AgingTime: 0.0,
		},
		Deadlines:            declaredDeadlines,
		MeasurementDurations: measurementDurations,
		MeasurementSchedule:  measurementSchedule,
	}
}

// resolveTau is §3.2b's three-case resolution of τ over an observation, stated
// once. It mirrors the reference port exactly:
//
//  1. declared individual deadlines — their minimum, and `null` when **any** of
//     them is unmeasured. Taking the minimum over the measured ones alone would
//     read an unknown timer as absent, and the unmeasured one may be the most
//     urgent (§10(w), §10(ao));
//  2. otherwise the `tau` observation of the map — signed, so a passed deadline
//     keeps its magnitude, and `null` when the map says the value is unknown;
//  3. otherwise the legacy entity-minimum, so the historical fixtures of
//     `v0.6`-`v0.9.1` still read as they did.
func resolveTau(deadlines map[string]*float64, tauObs *ResourceObservation,
	legacy float64) *float64 {
	if len(deadlines) > 0 {
		best := math.Inf(1)
		for _, v := range deadlines {
			if v == nil {
				return nil // an active deadline is unmeasured: τ is unknown
			}
			if *v < best {
				best = *v
			}
		}
		return &best
	}
	if tauObs != nil {
		if tauObs.Value == nil {
			return nil
		}
		v := *tauObs.Value
		return &v
	}
	v := legacy
	return &v
}
