package main

// Fixture helpers of the v0.11 harness (§3.6, §4.10).
//
// The readings are built as **transformations of one observed state**, never as
// hand-written states: scaling a copy's lens values, zeroing a copy's τ, removing
// an entity. Every transformation keeps the lens identity of §4.1 exactly — the
// copy's stated DoF is set to the product of the copy's own counters — so a
// reading is refused by `ValidateSet` only when the harness deliberately breaks
// it.

import "math"

// v011SetProduct scales an entity's lens values so that their product becomes
// `target`, then states that product as the entity's DoF (§4.1).
//
// Setting the DoF to the product **as computed** is what makes the identity hold
// exactly: the harness never asserts a number the port would compute differently.
func v011SetProduct(entity *EntityState, target float64) {
	if entity.Measurement == nil {
		return
	}
	product := LensProduct(entity)
	if product == nil || *product <= 0.0 || target <= 0.0 {
		return
	}
	ratio := math.Pow(target/(*product), 1.0/float64(len(lensOrder)))
	for lens, v := range entity.Measurement.Psi {
		if v == nil {
			continue
		}
		scaled := *v * ratio
		entity.Measurement.Psi[lens] = &scaled
	}
	if p := LensProduct(entity); p != nil {
		entity.CurrentDoF = *p
		entity.Measurement.CurrentDoF = *p
	}
}

// v011Copy is a deep-enough copy of a state: entities, their measurement objects
// and the psi maps are copied, so a transformation never touches the original.
func v011Copy(state *SystemStateMatrix) *SystemStateMatrix {
	clone := *state
	clone.Entities = make(map[string]*EntityState, len(state.Entities))
	for eID, ent := range state.Entities {
		copied := *ent
		if ent.Measurement != nil {
			m := *ent.Measurement
			m.Psi = map[string]*float64{}
			for lens, v := range ent.Measurement.Psi {
				if v == nil {
					m.Psi[lens] = nil
					continue
				}
				copiedValue := *v
				m.Psi[lens] = &copiedValue
			}
			counters := map[string]float64{}
			for k, v := range ent.Measurement.VarietyCounters {
				counters[k] = v
			}
			m.VarietyCounters = counters
			copied.Measurement = &m
		}
		clone.Entities[eID] = &copied
	}
	return &clone
}

// v011ScaledScene scales the declared Variety counter of every entity in a scene
// by `k`.
//
// The counter is a *measured* input, so scaling it produces a genuinely different
// reading, while the ruler — the procedures, the lens set, the units, the means
// class — is untouched. The resulting state's stated DoF is reconciled to the
// product of its own counters by `v011Reconcile`, which is what §4.1 requires of
// any reading.
func v011ScaledScene(scene map[string]interface{}, k float64) map[string]interface{} {
	out := map[string]interface{}{}
	for key, value := range scene {
		out[key] = value
	}
	for key, value := range scene {
		spec, ok := value.(map[string]interface{})
		if !ok {
			continue
		}
		lenses, ok := spec["lenses"].(map[string]interface{})
		if !ok {
			continue
		}
		variety, ok := lenses["variety"].(map[string]interface{})
		if !ok {
			continue
		}
		v, ok := variety["V"].(float64)
		if !ok {
			if iv, isInt := variety["V"].(int); isInt {
				v, ok = float64(iv), true
			}
		}
		if !ok {
			continue
		}
		newLenses := map[string]interface{}{}
		for lk, lv := range lenses {
			newLenses[lk] = lv
		}
		newVariety := map[string]interface{}{}
		for vk, vv := range variety {
			newVariety[vk] = vv
		}
		newVariety["V"] = v * k
		newLenses["variety"] = newVariety
		newSpec := map[string]interface{}{}
		for sk, sv := range spec {
			newSpec[sk] = sv
		}
		newSpec["lenses"] = newLenses
		out[key] = newSpec
	}
	return out
}

// v011Reconcile states each entity's DoF as the product of its own counters, so
// that a mapped copy satisfies the lens identity of §4.1 by construction.
func v011Reconcile(state *SystemStateMatrix) *SystemStateMatrix {
	clone := v011Copy(state)
	for _, ent := range clone.Entities {
		if product := LensProduct(ent); product != nil {
			ent.CurrentDoF = *product
			if ent.Measurement != nil {
				ent.Measurement.CurrentDoF = *product
			}
		}
	}
	return clone
}

// v011ScaleAll scales every entity's lens values by `k`, keeping the lens
// identity exact.
func v011ScaleAll(state *SystemStateMatrix, k float64) *SystemStateMatrix {
	clone := v011Copy(state)
	for _, ent := range clone.Entities {
		if ent.Measurement == nil {
			continue
		}
		for lens, v := range ent.Measurement.Psi {
			if v == nil {
				continue
			}
			scaled := *v * k
			ent.Measurement.Psi[lens] = &scaled
		}
		if product := LensProduct(ent); product != nil {
			ent.CurrentDoF = *product
			ent.Measurement.CurrentDoF = *product
		}
	}
	return clone
}

// v011SetEntityDoF sets one entity's DoF to `target` by scaling its own counters,
// which is the only way a reading may arrive at a different DoF (§4.1).
func v011SetEntityDoF(state *SystemStateMatrix, entityID string, target float64) *SystemStateMatrix {
	clone := v011Copy(state)
	if ent, ok := clone.Entities[entityID]; ok {
		v011SetProduct(ent, target)
	}
	return clone
}

// v011UnknownTau removes every trace of τ: the resource-map observation is
// present but **unmeasured**, which is what §3.2b calls an unknown τ. Zeroing the
// fields instead would leave the deprecated mirror in place and read as a
// **measured** τ of zero — a passed deadline, not an unmeasured budget.
func v011UnknownTau(state *SystemStateMatrix) *SystemStateMatrix {
	clone := v011Copy(state)
	clone.Tau = &ResourceObservation{Value: nil, Unit: "mks", Source: "sensor"}
	clone.Deadlines = nil
	clone.GlobalTimeToCollapseMks = 0.0
	return clone
}

// v011WithoutEntity drops an entity from a copy: the state-completeness defect of
// §3.6.
func v011WithoutEntity(state *SystemStateMatrix, entityID string) *SystemStateMatrix {
	clone := v011Copy(state)
	delete(clone.Entities, entityID)
	return clone
}

// v011WithExtraEntity adds an entity the observed state does not declare.
func v011WithExtraEntity(state *SystemStateMatrix) *SystemStateMatrix {
	clone := v011Copy(state)
	clone.Entities["v011_extra"] = &EntityState{
		EntityID: "v011_extra", CurrentDoF: 0.5, DoFKnown: true,
	}
	return clone
}

// v011BreakLensProduct states a DoF its own counters do not produce.
func v011BreakLensProduct(state *SystemStateMatrix, entityID string) *SystemStateMatrix {
	clone := v011Copy(state)
	if ent, ok := clone.Entities[entityID]; ok {
		ent.CurrentDoF = ent.CurrentDoF*2.0 + 0.5
	}
	return clone
}

// v011MoveCollapseLabel flips the collapse-source label of one entity: the defect
// §3.6 refuses, because the label is honoured through an act of the shared graph.
func v011MoveCollapseLabel(state *SystemStateMatrix, entityID string) *SystemStateMatrix {
	clone := v011Copy(state)
	if ent, ok := clone.Entities[entityID]; ok {
		ent.IsCollapseSource = !ent.IsCollapseSource
	}
	return clone
}

// v011ChangeDurations gives a reading its own measurement durations: the defect
// §3.4.1/§3.4.2 refuse, because the durations are ruler-level.
func v011ChangeDurations(state *SystemStateMatrix) *SystemStateMatrix {
	clone := v011Copy(state)
	durations := map[string]map[string]float64{}
	for lens, durs := range clone.MeasurementDurations {
		inner := map[string]float64{}
		for k, v := range durs {
			inner[k] = v
		}
		durations[lens] = inner
	}
	durations["variety"] = map[string]float64{"t_m": 123456.0, "t_v": 654321.0}
	clone.MeasurementDurations = durations
	return clone
}

// v011Option is a candidate that adds `delta` to one entity, requires nothing and
// closes nothing — the simplest well-formed option, in the flat form of §3.3.
func v011Option(id, entityID string, delta float64) *ActionOption {
	return &ActionOption{
		OptionID:             id,
		Description:          "v0.11 fixture candidate",
		ProjectedDoFDelta:    map[string]float64{entityID: delta},
		IsReversible:         true,
		EstimatedDurationMks: 1000.0,
	}
}

// v011Entity returns the `skip`-th entity id with a positive DoF, in sorted order,
// so the harness does not hard-code a fixture name for its arithmetic.
func v011Entity(state *SystemStateMatrix, skip int) string {
	for _, id := range sortedKeys(state.Entities) {
		if state.Entities[id].CurrentDoF > 0.0 {
			if skip == 0 {
				return id
			}
			skip--
		}
	}
	return ""
}

// hasSubstring reports whether any of the messages contains `needle`.
func hasSubstring(messages []string, needle string) bool {
	for _, message := range messages {
		if len(message) >= len(needle) {
			for i := 0; i+len(needle) <= len(message); i++ {
				if message[i:i+len(needle)] == needle {
					return true
				}
			}
			continue
		}
	}
	return false
}
