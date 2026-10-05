package main

// DOF-SPEC v0.7 §3.5 / §4.9 — the observed world graph and the named procedures
// over it. Reference port; mirrors patterns/python/world_graph.py exactly.
//
// Design notes that matter for cross-port equality (§3.4.3, §11.9):
//   * every comparison of a derived rate is made on the CANONICALLY QUANTIZED
//     value (6 decimals), never on the raw float — comparison and serialization
//     then use one rounding, so the result is a function of the observation and
//     not of the order in which a port happened to multiply its factors;
//   * path ties are broken canonically: cheaper quantized value, then FEWER
//     EDGES, then lexicographic order of the edge-id sequence — so the reported
//     witness agrees too;
//   * the reference port enumerates simple paths (a world small enough for
//     that), but any implementation MUST reproduce the same canonical choice.

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"math"
	"sort"
	"strconv"
	"strings"
)

const (
	QDecimals    = 6
	MaxPathEdges = 8 // reference-port bound for simple-path enumeration
)

// q6 is the canonical quantization of §3.4.3: one rounding for comparison AND
// emission. Implemented through the decimal string, exactly as the reference
// does, so that a tie at the sixth decimal cannot resolve differently.
func q6(x float64) float64 {
	v, err := strconv.ParseFloat(fmt.Sprintf("%.6f", x), 64)
	if err != nil {
		return x
	}
	return v
}

// canonicalPayload renders a fingerprint payload in the canonical form of
// §3.4.3: dictionaries key-sorted, floats as fixed six-decimal STRINGS, ints
// left as ints, absent values as null. A hash several ports must reproduce may
// never be serialized by a language's own default notation — `1000.0` and
// `1000` are one quantity and three ports must agree on it.
func canonicalPayload(obj interface{}) interface{} {
	switch v := obj.(type) {
	case map[string]interface{}:
		res := make(map[string]interface{}, len(v))
		for k, val := range v {
			res[k] = canonicalPayload(val)
		}
		return res
	case []interface{}:
		res := make([]interface{}, len(v))
		for i, val := range v {
			res[i] = canonicalPayload(val)
		}
		return res
	case []string:
		res := make([]interface{}, len(v))
		for i, val := range v {
			res[i] = canonicalPayload(val)
		}
		return res
	case float64:
		return fmt.Sprintf("%.6f", v)
	case int:
		return v
	case bool, nil, string:
		return v
	}
	return fmt.Sprintf("%v", obj)
}

func jsonOf(payload interface{}) string {
	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	// HTML escaping MUST stay off: Go would otherwise write `>` as `\u003e`,
	// and an observed rate key or an id containing it would hash differently
	// from the identical UTF-8 bytes every other port writes.
	enc.SetEscapeHTML(false)
	_ = enc.Encode(canonicalPayload(payload))
	return strings.TrimRight(buf.String(), "\n")
}

// ------------------------------------------------------------------- nodes
type EntityNode struct {
	ID          string  `json:"id"`
	Observation string  `json:"observation"` // "complete" | "partial"
	CurrentDoF  float64 `json:"current_dof"` // mirrored from the state
}

func (n EntityNode) complete() bool { return n.Observation == "complete" }

type ActEdge struct {
	ID          string             `json:"id"`
	Source      string             `json:"source"`
	Target      string             `json:"target"`
	Category    string             `json:"category"`
	Requires    []string           `json:"requires"`
	Effect      map[string]float64 `json:"effect"`
	Resources   map[string]float64 `json:"resources"`
	DurationMks float64            `json:"duration_mks"`
}

func (a ActEdge) requiresAll(means map[string]bool) bool {
	for _, m := range a.Requires {
		if !means[m] {
			return false
		}
	}
	return true
}

type ExchangeEdge struct {
	ID          string             `json:"id"`
	Gives       map[string]float64 `json:"gives"`
	Wants       map[string]float64 `json:"wants"`
	DurationMks float64            `json:"duration_mks"`
}

func (e ExchangeEdge) fromResource() string { return firstKey(e.Gives) }
func (e ExchangeEdge) toResource() string   { return firstKey(e.Wants) }

// multiplier is the units of `wants` obtained per one unit of `gives`.
func (e ExchangeEdge) multiplier() float64 {
	return firstValue(e.Wants) / firstValue(e.Gives)
}

func firstKey(m map[string]float64) string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	if len(keys) == 0 {
		return ""
	}
	return keys[0]
}

func firstValue(m map[string]float64) float64 { return m[firstKey(m)] }

type ClosedRef struct {
	Kind string `json:"kind"` // "act" | "mean"
	ID   string `json:"id"`
}

// ----------------------------------------------------------------- results
type RateResult struct {
	Status      string   // observed | undetermined | not_covered
	Rate        *float64
	Path        []string
	DurationMks float64
	Reason      string
}

type Verdict struct {
	EntityID       string
	Verdict        string // reachable | proven_unreachable | undetermined
	Witness        []string
	AdmissibleSeen int
	Reason         string
}

// ------------------------------------------------------------------- graph
type WorldGraph struct {
	Entities  map[string]EntityNode
	Means     []string
	Acts      []ActEdge
	Exchanges []ExchangeEdge

	// Per-search scratch of §4.9. `enumerationIncomplete` is the only piece the
	// verdict reads, and it defaults to false on every fresh graph — the honest
	// default, because a search that was never cut short proved what it claims.
	enumerationIncomplete  bool
	enumerated             int
	enumerationSafetyLimit *int
}

// PathCand is one finite simple directed path of structurally admissible acts:
// its act identifiers, its total duration and its cumulative declared effect on
// the entity the verdict is about (§3.5, §4.9).
type PathCand struct {
	Ids      []string
	Duration float64
	Delta    float64
}

func (g *WorldGraph) meansSet() map[string]bool {
	out := make(map[string]bool, len(g.Means))
	for _, m := range g.Means {
		out[m] = true
	}
	return out
}

// ---------------------------------------------------------------- form (§3.5)
func (g *WorldGraph) FormErrors() []string {
	errs := []string{}
	for _, e := range g.Exchanges {
		if len(e.Gives) == 0 || len(e.Wants) == 0 {
			errs = append(errs, e.ID+": an exchange basket is empty")
		}
		for _, v := range e.Gives {
			if v <= 0 {
				errs = append(errs, e.ID+": a quote amount is not strictly positive")
			}
		}
		for _, v := range e.Wants {
			if v <= 0 {
				errs = append(errs, e.ID+": a quote amount is not strictly positive")
			}
		}
		if len(e.Gives) != 1 || len(e.Wants) != 1 {
			errs = append(errs, e.ID+": multi-resource baskets are reserved in this revision")
		}
		for k := range e.Gives {
			if _, both := e.Wants[k]; both {
				errs = append(errs, e.ID+": a trade cannot give and want the same resource")
			}
		}
		if e.DurationMks < 0 {
			errs = append(errs, e.ID+": negative duration")
		}
	}
	means := g.meansSet()
	for _, a := range g.Acts {
		if a.Category == "" {
			errs = append(errs, a.ID+": an act without an admissible-means category")
		}
		if a.DurationMks < 0 {
			errs = append(errs, a.ID+": negative duration")
		}
		for _, m := range a.Requires {
			if !means[m] {
				errs = append(errs, a.ID+": requires undeclared mean "+m)
			}
		}
	}
	return errs
}

// ------------------------------------------------------- arbitrage test (§3.5)
// ArbitrageEdges returns the edge ids involved in a cycle whose product exceeds
// 1 (empty = healthy). Bellman-Ford on `-ln(multiplier)`: a cycle of product > 1
// is a negative cycle. The result is every edge that still relaxed on the final
// pass — a superset of the offending cycle, which is what a report needs to
// point at.
func (g *WorldGraph) ArbitrageEdges() []string {
	type edge struct {
		src, dst string
		w        float64
		id       string
	}
	var edges []edge
	nodeSet := map[string]bool{}
	for _, e := range g.Exchanges {
		src, dst := e.fromResource(), e.toResource()
		nodeSet[src] = true
		nodeSet[dst] = true
		edges = append(edges, edge{src: src, dst: dst, w: -math.Log(e.multiplier()), id: e.ID})
	}
	nodes := make([]string, 0, len(nodeSet))
	for n := range nodeSet {
		nodes = append(nodes, n)
	}
	sort.Strings(nodes)
	dist := make(map[string]float64, len(nodes))
	for _, n := range nodes {
		dist[n] = 0.0 // virtual source: all nodes at 0
	}
	var hot []string
	for i := 0; i < len(nodes); i++ {
		hot = nil
		for _, e := range edges {
			if dist[e.src]+e.w < dist[e.dst]-1e-12 {
				dist[e.dst] = dist[e.src] + e.w
				hot = append(hot, e.id)
			}
		}
		if len(hot) == 0 {
			return []string{}
		}
	}
	sort.Strings(hot)
	out := []string{}
	for i, id := range hot {
		if i == 0 || hot[i-1] != id {
			out = append(out, id)
		}
	}
	return out
}

func (g *WorldGraph) IsArbitrageFree() bool { return len(g.ArbitrageEdges()) == 0 }

// ---------------------------------------------- fingerprint of the observation
// ObservationDigest is the §6.2 fingerprint of the OBSERVATION, not of the
// ruler: it covers what was observed (nodes with completeness, means, acts,
// quotes, M(S), T_rec and the counting horizon) and deliberately not the
// candidate set — a decision that moved with the options offered would not be
// reproducible (§4.2).
func (g *WorldGraph) ObservationDigest(meansClass []string, tRec map[string]float64,
	countingHorizonMks *float64) string {
	entities := map[string]interface{}{}
	for id, node := range g.Entities {
		entities[id] = map[string]interface{}{
			"observation": node.Observation,
			"current_dof": node.CurrentDoF,
		}
	}
	means := append([]string{}, g.Means...)
	sort.Strings(means)
	acts := make([]interface{}, 0, len(g.Acts))
	sortedActs := append([]ActEdge{}, g.Acts...)
	sort.Slice(sortedActs, func(i, j int) bool { return sortedActs[i].ID < sortedActs[j].ID })
	for _, a := range sortedActs {
		req := append([]string{}, a.Requires...)
		sort.Strings(req)
		effect := map[string]interface{}{}
		for k, v := range a.Effect {
			effect[k] = v
		}
		acts = append(acts, map[string]interface{}{
			"id": a.ID, "source": a.Source, "target": a.Target, "category": a.Category,
			"requires": req, "effect": effect, "duration_mks": a.DurationMks,
		})
	}
	exs := make([]interface{}, 0, len(g.Exchanges))
	sortedEx := append([]ExchangeEdge{}, g.Exchanges...)
	sort.Slice(sortedEx, func(i, j int) bool { return sortedEx[i].ID < sortedEx[j].ID })
	for _, e := range sortedEx {
		gives := map[string]interface{}{}
		for k, v := range e.Gives {
			gives[k] = v
		}
		wants := map[string]interface{}{}
		for k, v := range e.Wants {
			wants[k] = v
		}
		exs = append(exs, map[string]interface{}{
			"id": e.ID, "gives": gives, "wants": wants, "duration_mks": e.DurationMks,
		})
	}
	cls := append([]string{}, meansClass...)
	sort.Strings(cls)
	tr := map[string]interface{}{}
	for k, v := range tRec {
		tr[k] = v
	}
	var horizon interface{}
	if countingHorizonMks != nil {
		horizon = *countingHorizonMks
	}
	payload := map[string]interface{}{
		"entities": entities, "means": means, "acts": acts, "exchanges": exs,
		"means_class": cls, "t_rec": tr, "counting_horizon_mks": horizon,
	}
	sum := sha256.Sum256([]byte(jsonOf(payload)))
	return hex.EncodeToString(sum[:])
}

// ------------------------------------------------- the rate as an observation
type quote struct {
	src, dst string
	mult     float64
	duration float64
	id       string
}

func (g *WorldGraph) quotes() []quote {
	out := make([]quote, 0, len(g.Exchanges))
	for _, e := range g.Exchanges {
		out = append(out, quote{src: e.fromResource(), dst: e.toResource(),
			mult: e.multiplier(), duration: e.DurationMks, id: e.ID})
	}
	sort.Slice(out, func(i, j int) bool { return out[i].id < out[j].id })
	return out
}

type pathCand struct {
	ids      []string
	product  float64
	duration float64
}

// simplePaths enumerates every simple path a -> b: (edge ids, product, duration)
// in a deterministic order.
func (g *WorldGraph) simplePaths(a, b string) []pathCand {
	adj := map[string][]quote{}
	for _, q := range g.quotes() {
		adj[q.src] = append(adj[q.src], q)
	}
	for k := range adj {
		list := adj[k]
		sort.Slice(list, func(i, j int) bool { return list[i].id < list[j].id })
		adj[k] = list
	}
	out := []pathCand{}
	var dfs func(node string, seen map[string]bool, ids []string, prod, dur float64)
	dfs = func(node string, seen map[string]bool, ids []string, prod, dur float64) {
		if len(ids) > MaxPathEdges {
			return
		}
		if node == b && len(ids) > 0 {
			out = append(out, pathCand{ids: append([]string{}, ids...), product: prod, duration: dur})
			return
		}
		for _, q := range adj[node] {
			if seen[q.dst] {
				continue
			}
			next := map[string]bool{}
			for k, v := range seen {
				next[k] = v
			}
			next[q.dst] = true
			dfs(q.dst, next, append(ids, q.id), prod*q.mult, dur+q.duration)
		}
	}
	dfs(a, map[string]bool{a: true}, []string{}, 1.0, 0.0)
	return out
}

// Rate is the axis rate (§3.5/§4.8): the best product of quotes along a path.
// Selection is canonical — quantized value first, then fewer edges, then the
// lexicographically smallest edge-id sequence. Unknown and absent are different
// answers: an incomplete observation yields `undetermined`, never a price.
func (g *WorldGraph) Rate(a, b string, observationComplete bool) RateResult {
	if a == b {
		one := 1.0
		return RateResult{Status: "observed", Rate: &one, Path: []string{}, Reason: "identity"}
	}
	if !g.IsArbitrageFree() {
		return RateResult{Status: "undetermined", Reason: "observation is not arbitrage-free"}
	}
	cands := g.simplePaths(a, b)
	if len(cands) == 0 {
		if observationComplete {
			return RateResult{Status: "not_covered", Reason: "no exchange path " + a + "->" + b}
		}
		return RateResult{Status: "undetermined",
			Reason: "no observed exchange path " + a + "->" + b + ", observation partial"}
	}
	sort.SliceStable(cands, func(i, j int) bool {
		qi, qj := q6(cands[i].product), q6(cands[j].product)
		if qi != qj {
			return qi > qj // the largest product wins
		}
		if len(cands[i].ids) != len(cands[j].ids) {
			return len(cands[i].ids) < len(cands[j].ids) // then FEWER edges
		}
		return strings.Join(cands[i].ids, "\x00") < strings.Join(cands[j].ids, "\x00")
	})
	best := cands[0]
	rate := q6(best.product)
	return RateResult{Status: "observed", Rate: &rate, Path: best.ids,
		DurationMks: best.duration, Reason: "canonical best path"}
}

// ------------------------------------------------------------ variety (§4.6)
// AdmissibleActs returns the acts admissible in this observation: category in
// `M(S)`, inside the horizon, every required mean declared.
func (g *WorldGraph) AdmissibleActs(categories []string, horizonMks *float64) []ActEdge {
	if len(categories) == 0 || horizonMks == nil {
		return []ActEdge{}
	}
	cat := map[string]bool{}
	for _, c := range categories {
		cat[c] = true
	}
	means := g.meansSet()
	out := []ActEdge{}
	for _, a := range g.Acts {
		if cat[a.Category] && a.DurationMks <= *horizonMks && a.requiresAll(means) {
			out = append(out, a)
		}
	}
	return out
}

// PathEdges returns the **structurally admissible** acts of §4.9, in canonical
// order. Structurally admissible means `category ∈ M(S)`, every `requires` mean
// declared, and `duration_mks <= T_rec(X)` — a *structural* property of the graph
// and the entity type. It is deliberately NOT the option predicate `admissible(o)`
// of §4.5: running that over a path's acts would let the τ and the resource map
// of one reading leak into a verdict §4.9 wants independent of the option set.
func (g *WorldGraph) PathEdges(categories []string, horizonMks *float64) []ActEdge {
	out := append([]ActEdge{}, g.AdmissibleActs(categories, horizonMks)...)
	sort.SliceStable(out, func(i, j int) bool { return out[i].ID < out[j].ID })
	return out
}

// ReachabilityPaths enumerates every finite **simple directed path** of
// structurally admissible acts.
//
// A path is a chain of acts joined by `target → source` (an act's outcome is what
// the next act starts from), with no node and no edge repeated — a *simple* path,
// never an arbitrary walk: a zero-duration cycle with a positive effect would
// otherwise pump `Δ_P(X)` without spending time, and `duration(P) = 0` would
// satisfy every horizon (§3.5, item 45).
//
// The enumeration is exhaustive over the finite simple paths and carries no edge
// bound. §4.9's condition is an existential over finite simple directed paths and
// states no length limit, so an implementation that silently stopped at a fixed
// depth could report `proven_unreachable` for an entity a longer path restores — a
// verdict §4.2 turns into an exclusion. A `safetyLimit`, when an implementation
// sets one, bounds the number of **explored nodes** for resource protection only;
// an enumeration that hits it is marked incomplete and `Verdict` then reads
// `undetermined`, never a proof of unreachability. The depth is bounded anyway by
// simplicity (`|P| <= |V(G)| - 1`) and by the horizon.
//
// Results are returned in canonical order — greater `Δ_P` first, then fewer edges,
// then the lexicographically smallest identifier sequence — so two conformant
// implementations report the same witness.
func (g *WorldGraph) ReachabilityPaths(entityID string, categories []string,
	horizonMks *float64, safetyLimit *int) []PathCand {
	g.enumerationIncomplete = false
	g.enumerated = 0
	if len(categories) == 0 || horizonMks == nil {
		return []PathCand{}
	}
	edges := g.PathEdges(categories, horizonMks)
	bySource := map[string][]ActEdge{}
	for _, a := range edges {
		bySource[a.Source] = append(bySource[a.Source], a)
	}
	results := []PathCand{}
	var walk func(node string, produced, seenEdges map[string]bool,
		ids []string, duration, delta float64)
	walk = func(node string, produced, seenEdges map[string]bool,
		ids []string, duration, delta float64) {
		// Record every prefix: a path need not be maximal, and a shorter prefix
		// may be the one that lifts the entity off a known zero.
		results = append(results, PathCand{Ids: append([]string{}, ids...),
			Duration: duration, Delta: delta})
		g.enumerated++
		if safetyLimit != nil && g.enumerated > *safetyLimit {
			g.enumerationIncomplete = true
			return
		}
		for _, a := range bySource[node] {
			if seenEdges[a.ID] {
				continue // no edge twice
			}
			// No NODE twice, the origin included. `produced` is seeded with the
			// start vertex at every launch, so a path can neither return to its
			// origin (`A -> B -> A`) nor act on itself (`A -> A`): both repeat a
			// vertex and are therefore not simple paths. Without the seed a
			// zero-duration cycle with a positive effect would satisfy
			// `duration(P) = 0 <= T_rec` and pump `Δ_P(X)` for free, and the
			// verdict would report `reachable` where no simple path exists.
			if produced[a.Target] {
				continue
			}
			if duration+a.DurationMks > *horizonMks {
				continue // Σ duration ≤ T_rec(X)
			}
			np := make(map[string]bool, len(produced)+1)
			for k, v := range produced {
				np[k] = v
			}
			np[a.Target] = true
			ne := make(map[string]bool, len(seenEdges)+1)
			for k, v := range seenEdges {
				ne[k] = v
			}
			ne[a.ID] = true
			walk(a.Target, np, ne, append(ids, a.ID), duration+a.DurationMks,
				delta+a.Effect[entityID])
		}
	}
	startsSet := map[string]bool{}
	for _, a := range edges {
		startsSet[a.Source] = true
	}
	starts := make([]string, 0, len(startsSet))
	for s := range startsSet {
		starts = append(starts, s)
	}
	sort.Strings(starts)
	for _, s := range starts {
		walk(s, map[string]bool{s: true}, map[string]bool{}, []string{}, 0.0, 0.0)
	}
	// Non-empty paths only: the empty path changes nothing and cannot raise a DoF.
	out := []PathCand{}
	for _, r := range results {
		if len(r.Ids) > 0 {
			out = append(out, r)
		}
	}
	sort.SliceStable(out, func(i, j int) bool {
		qi, qj := q6(out[i].Delta), q6(out[j].Delta)
		if qi != qj {
			return qi > qj
		}
		if len(out[i].Ids) != len(out[j].Ids) {
			return len(out[i].Ids) < len(out[j].Ids)
		}
		return strings.Join(out[i].Ids, "\x00") < strings.Join(out[j].Ids, "\x00")
	})
	return out
}

// EnumerationComplete reports whether the last ReachabilityPaths call finished.
// False means the implementation's own safety limit cut the search short, so the
// absence of a raising path is NOT a proof of unreachability.
func (g *WorldGraph) EnumerationComplete() bool { return !g.enumerationIncomplete }

// SetEnumerationSafetyLimit sets this implementation's own protection bound for
// §4.9's search. `nil` (the default, and the normative behaviour of §4.9)
// enumerates every finite simple path: the existential the section states is over
// all of them and names no depth. An implementation that sets a bound does so for
// its own resource protection, and a verdict computed under a bound that was hit
// reads `undetermined` — never `proven_unreachable`, which is a claim of
// completeness the interrupted search has not earned.
func (g *WorldGraph) SetEnumerationSafetyLimit(limit *int) {
	g.enumerationSafetyLimit = limit
}

// ReachableActs is the entity's RESPONSE VECTORS (§4.6): admissible acts THIS
// entity can perform. Deliberately filtered by `source`. Recoverability is a
// different question — there the pool is every admissible act whose effect
// raises the entity's DoF, whoever performs it (a medic revives a patient).
func (g *WorldGraph) ReachableActs(entityID string, categories []string,
	horizonMks *float64) []string {
	out := []string{}
	for _, a := range g.AdmissibleActs(categories, horizonMks) {
		if a.Source == entityID {
			out = append(out, a.ID)
		}
	}
	sort.Strings(out)
	return out
}

func (g *WorldGraph) VCount(entityID string, categories []string, horizonMks *float64) int {
	return len(g.ReachableActs(entityID, categories, horizonMks))
}

// ---------------------------------------------------- numeraire weights (§4.6)
// WeightsTo is `w_r` — the price of one unit of `r`, in the numeraire.
//
// The weight is what one unit of `r` COSTS TO ACQUIRE: the numeraire spent, i.e.
// the inverse of the canonical best product of observed quotes along a path from
// the numeraire to `r`. When no path *from* the numeraire exists — the resource
// cannot be bought at all — the weight falls back to what one unit FETCHES
// (`rate(r, numeraire)`), which is the only price the observation supports.
//
// The distinction matters: `credit->energy = 2.0` and `energy->credit = 0.25`
// are both in this world, and they disagree. A sum that mixed the two directions
// without saying so would produce a balance nobody could reproduce.
//
// A resource with neither direction observed carries NO weight and must not
// silently fall back to 1.0: `deriveBlocks` then treats it as its own singleton
// block, where its own unit IS its nominal.
func (g *WorldGraph) WeightsTo(numeraire string, resources []string) map[string]float64 {
	out := map[string]float64{}
	uniq := map[string]bool{}
	for _, r := range resources {
		uniq[r] = true
	}
	keys := make([]string, 0, len(uniq))
	for r := range uniq {
		keys = append(keys, r)
	}
	sort.Strings(keys)
	for _, r := range keys {
		if r == numeraire {
			out[r] = 1.0
			continue
		}
		buy := g.Rate(numeraire, r, true) // units of r per one numeraire
		if buy.Status == "observed" && buy.Rate != nil && *buy.Rate != 0 {
			out[r] = q6(1.0 / *buy.Rate)
			continue
		}
		sell := g.Rate(r, numeraire, true) // numeraire per one unit of r
		if sell.Status == "observed" && sell.Rate != nil {
			out[r] = q6(*sell.Rate)
		}
	}
	return out
}

// --------------------------------------------------------- verdicts (§4.9)
// Verdict is §4.9's verdict with the caller's own `DoF(X | h)`; `dofBefore` is
// nil when the caller does not supply it and the verdict falls back to the DoF
// the graph mirrors for that entity (the degenerate case, not the rule: the
// verdict is a value of `(G, state_h)`).
func (g *WorldGraph) Verdict(entityID string, categories []string,
	horizonMks *float64) Verdict {
	return g.VerdictWithDoF(entityID, categories, horizonMks, nil)
}

// VerdictWithDoF is §4.9's rule, stated once.
//
// The condition is `DoF(X | h) + Δ_P(X) > 0` over a finite **simple** path of
// structurally admissible acts whose total duration fits `T_rec(X)`. For an
// entity at a known zero that reduces to `Δ_P(X) > 0`; for one already positive
// it is satisfied by the **trivial** path (`P` a single vertex, `Δ_P = 0`,
// duration 0), so a live entity is `reachable` without any path search — reading
// the rule as requiring a raising path in every case would make a live entity
// unrecoverable by construction (§10(J), fixture `ar`).
//
// `proven_unreachable` is a claim of **completeness** — §4.2 turns it into an
// exclusion — so it is returned only when the enumeration actually finished.
func (g *WorldGraph) VerdictWithDoF(entityID string, categories []string,
	horizonMks *float64, dofBefore *float64) Verdict {
	node, ok := g.Entities[entityID]
	if !ok {
		return Verdict{EntityID: entityID, Verdict: "undetermined",
			Reason: "entity not observed at all"}
	}
	if !node.complete() {
		return Verdict{EntityID: entityID, Verdict: "undetermined",
			Reason: "observation is partial for this entity"}
	}
	if len(categories) == 0 {
		return Verdict{EntityID: entityID, Verdict: "undetermined",
			Reason: "admissible-means class M(S) is not declared"}
	}
	if horizonMks == nil {
		return Verdict{EntityID: entityID, Verdict: "undetermined",
			Reason: "recovery horizon T_rec is not declared"}
	}
	base := node.CurrentDoF
	if dofBefore != nil {
		base = *dofBefore
	}
	// The recoverability pool is NOT the entity's own repertoire: anyone's
	// admissible act may raise X's DoF. V counts what X itself can do.
	pool := g.AdmissibleActs(categories, horizonMks)
	if base > 0.0 {
		return Verdict{EntityID: entityID, Verdict: "reachable", Witness: []string{},
			AdmissibleSeen: len(pool),
			Reason: "DoF(X | h) > 0: the trivial path satisfies the condition (§4.9)"}
	}
	paths := g.ReachabilityPaths(entityID, categories, horizonMks,
		g.enumerationSafetyLimit)
	if !g.EnumerationComplete() {
		return Verdict{EntityID: entityID, Verdict: "undetermined", Witness: []string{},
			AdmissibleSeen: len(pool),
			Reason: "the path search was stopped at the implementation's safety " +
				"limit, so absence of a raising path is not a proof (§4.9)"}
	}
	for _, p := range paths {
		if base+p.Delta > 0.0 {
			return Verdict{EntityID: entityID, Verdict: "reachable", Witness: p.Ids,
				AdmissibleSeen: len(pool),
				Reason: "a structurally admissible simple path raises DoF within T_rec"}
		}
	}
	return Verdict{EntityID: entityID, Verdict: "proven_unreachable", Witness: []string{},
		AdmissibleSeen: len(pool),
		Reason:         "complete observation, no structurally admissible simple path raises DoF within T_rec"}
}

// ------------------------------------------------ collapse act witness (§4.2)
// CollapseActs returns observed acts that drive a COUNTED entity to a known
// zero. This is the machine-verifiable act of collapse; a label without one of
// these is not honoured (§4.2, §4.9).
func (g *WorldGraph) CollapseActs(counted map[string]bool,
	dofBefore map[string]float64) []string {
	out := []string{}
	for _, a := range g.Acts {
		for e, delta := range a.Effect {
			if counted[e] && dofBefore[e]+delta <= 0.0 {
				out = append(out, a.ID)
				break
			}
		}
	}
	sort.Strings(out)
	return out
}

// ------------------------------------------------------------------- closure
// WithClosed applies a closure: acts removed directly, means removed together
// with their acts.
func (g *WorldGraph) WithClosed(closed []ClosedRef) *WorldGraph {
	actsOff := map[string]bool{}
	meansOff := map[string]bool{}
	for _, c := range closed {
		if c.Kind == "act" {
			actsOff[c.ID] = true
		}
		if c.Kind == "mean" {
			meansOff[c.ID] = true
		}
	}
	acts := []ActEdge{}
	for _, a := range g.Acts {
		if actsOff[a.ID] {
			continue
		}
		touched := false
		for _, m := range a.Requires {
			if meansOff[m] {
				touched = true
				break
			}
		}
		if touched {
			continue
		}
		acts = append(acts, a)
	}
	means := []string{}
	for _, m := range g.Means {
		if !meansOff[m] {
			means = append(means, m)
		}
	}
	entities := make(map[string]EntityNode, len(g.Entities))
	for k, v := range g.Entities {
		entities[k] = v
	}
	return &WorldGraph{Entities: entities, Means: means, Acts: acts,
		Exchanges: append([]ExchangeEdge{}, g.Exchanges...)}
}

// GuardClosure holds the two guards of §4.4. An empty result means the closure
// list is admissible.
func (g *WorldGraph) GuardClosure(closed []ClosedRef, ownActID string) []string {
	errs := []string{}
	if ownActID != "" {
		for _, c := range closed {
			if c.Kind == "act" && c.ID == ownActID {
				errs = append(errs, "the option closes its own execution path")
			}
		}
	}
	if len(closed) == 0 {
		errs = append(errs, "empty closure list")
	}
	return errs
}
