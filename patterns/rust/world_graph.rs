//! DOF-SPEC v0.7 §3.5 / §4.9 — the observed world graph and the named procedures
//! over it. Rust reference port; mirrors patterns/python/world_graph.py,
//! patterns/go/world_graph.go and patterns/cpp/world_graph.hpp exactly.
//!
//! Design notes that matter for cross-port equality (§3.4.3, §11.9):
//!   * every comparison of a derived rate is made on the CANONICALLY QUANTIZED
//!     value (6 decimals), never on the raw `f64` — comparison and serialization
//!     then use one rounding, so the result is a function of the observation and
//!     not of the order in which a port happened to multiply its factors;
//!   * path ties are broken canonically: cheaper quantized value, then FEWER
//!     EDGES, then lexicographic order of the edge-id sequence;
//!   * the reference port enumerates simple paths (a world small enough for that),
//!     but any implementation MUST reproduce the same canonical choice.

use std::collections::{BTreeMap, BTreeSet};

use crate::measurement::sha256_hex;

const MAX_PATH_EDGES: usize = 8;

/// Canonical quantization of §3.4.3: one rounding for comparison AND emission.
/// Rendered through the decimal string, exactly as the reference does, so a tie at
/// the sixth decimal cannot resolve differently.
pub fn q6(x: f64) -> f64 {
    format!("{:.6}", x).parse::<f64>().unwrap_or(x)
}

/// Numbers are serialized in the canonical form of §3.4.3 — fixed six-decimal
/// strings — never in a language's own float notation: `1000.0` and `1000` are one
/// quantity, and four ports must agree on it.
pub fn obs_f6(x: f64) -> String {
    format!("{:.6}", x)
}

/// ---------------------------------------------------------------- nodes
#[derive(Clone, Debug)]
pub struct EntityNode {
    pub id: String,
    /// "complete" | "partial"
    pub observation: String,
    /// Mirrored from the state. Since `v0.11` §4.9 the verdict **does** read it —
    /// as `DoF(X | h)` under the reading being evaluated — so a live entity is
    /// `reachable` through the trivial path without any act.
    pub current_dof: f64,
}

impl EntityNode {
    pub fn complete(&self) -> bool {
        self.observation == "complete"
    }
}

#[derive(Clone, Debug, Default)]
pub struct ActEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub category: String,
    pub requires: Vec<String>,
    pub effect: BTreeMap<String, f64>,
    pub resources: BTreeMap<String, f64>,
    pub duration_mks: f64,
    /// §3.5 (v0.9.1): measure-type act resolves an unmeasured resource.
    pub discovers: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ExchangeEdge {
    pub id: String,
    /// What the actor hands over.
    pub gives: BTreeMap<String, f64>,
    /// What the actor receives.
    pub wants: BTreeMap<String, f64>,
    pub duration_mks: f64,
}

impl ExchangeEdge {
    pub fn from_resource(&self) -> String {
        self.gives.keys().next().cloned().unwrap_or_default()
    }
    pub fn to_resource(&self) -> String {
        self.wants.keys().next().cloned().unwrap_or_default()
    }
    /// Units of `wants` obtained per one unit of `gives`.
    pub fn multiplier(&self) -> f64 {
        let w = self.wants.values().next().copied().unwrap_or(0.0);
        let g = self.gives.values().next().copied().unwrap_or(1.0);
        w / g
    }
}

/// One quote of the observed market.
#[derive(Clone, Debug)]
pub struct Quote {
    pub src: String,
    pub dst: String,
    pub id: String,
    pub mult: f64,
    pub duration: f64,
}

/// One entry of an option's `closed` list (§4.4).
#[derive(Clone, Debug)]
pub struct ClosedRef {
    /// "act" | "mean"
    pub kind: String,
    pub id: String,
}

impl ClosedRef {
    pub fn act(id: &str) -> Self {
        ClosedRef { kind: "act".to_string(), id: id.to_string() }
    }
    pub fn mean(id: &str) -> Self {
        ClosedRef { kind: "mean".to_string(), id: id.to_string() }
    }
}

/// ------------------------------------------------------------- results
#[derive(Clone, Debug)]
pub struct RateResult {
    /// observed | undetermined | not_covered
    pub status: String,
    pub rate: Option<f64>,
    pub path: Vec<String>,
    pub duration_mks: f64,
    pub reason: String,
}

impl RateResult {
    fn simple(status: &str, reason: &str) -> Self {
        RateResult {
            status: status.to_string(),
            rate: None,
            path: Vec::new(),
            duration_mks: 0.0,
            reason: reason.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Verdict {
    pub entity_id: String,
    /// reachable | proven_unreachable | undetermined
    pub verdict: String,
    pub witness: Vec<String>,
    pub admissible_seen: usize,
    pub reason: String,
}

/// A candidate path of §4.9: the edge-id sequence, its total duration and its
/// accumulated effect on the entity whose recoverability is being decided.
#[derive(Clone, Debug, Default)]
pub struct ReachPath {
    pub ids: Vec<String>,
    pub duration: f64,
    pub delta: f64,
}

/// ----------------------------------------------------------------- graph
#[derive(Clone, Debug, Default)]
pub struct WorldGraph {
    pub entities: BTreeMap<String, EntityNode>,
    pub means: Vec<String>,
    pub acts: Vec<ActEdge>,
    pub exchanges: Vec<ExchangeEdge>,
    /// §4.9 (v0.11): the enumeration's own bookkeeping. `Cell` rather than a plain
    /// field because §4.9's verdict is asked through `&self` — the graph is reached
    /// through `ObservationContext`, which is shared — while the search must still
    /// record whether it was cut short. A plain `bool` would force `&mut` through
    /// every caller of `verdict` for a flag that is written in exactly one place.
    pub enumeration_incomplete: std::cell::Cell<bool>,
    pub enumerated: std::cell::Cell<usize>,
    /// `None` means "no implementation-imposed bound": the search is exhaustive.
    pub enumeration_safety_limit: std::cell::Cell<Option<usize>>,
}

/// A candidate path: the edge-id sequence, the product of its quotes and its total
/// duration.
type PathCand = (Vec<String>, f64, f64);

/// The §4.9 enumeration, as a recursion over simple paths.
///
/// Kept as its own struct rather than a nested closure so that the accumulator —
/// the explored-node count and the completeness flag — is written in one place and
/// handed back to the graph exactly once, at the end of the search.
struct PathWalk<'a> {
    by_source: &'a BTreeMap<String, Vec<ActEdge>>,
    entity_id: &'a str,
    horizon: f64,
    safety_limit: Option<usize>,
    results: Vec<ReachPath>,
    enumerated: usize,
    incomplete: bool,
}

impl<'a> PathWalk<'a> {
    fn walk(
        &mut self,
        node: &str,
        produced: &BTreeSet<String>,
        seen_edges: &BTreeSet<String>,
        ids: &Vec<String>,
        duration: f64,
        delta: f64,
    ) {
        // Record every prefix: a path need not be maximal, and a shorter prefix may
        // be the one that lifts the entity off a known zero.
        self.results.push(ReachPath {
            ids: ids.clone(),
            duration,
            delta,
        });
        self.enumerated += 1;
        if let Some(limit) = self.safety_limit {
            if self.enumerated > limit {
                self.incomplete = true;
                return;
            }
        }
        let outgoing = match self.by_source.get(node) {
            Some(v) => v.clone(),
            None => return,
        };
        for a in outgoing.iter() {
            if seen_edges.contains(&a.id) {
                continue; // no edge twice
            }
            if produced.contains(&a.target) {
                continue; // no node twice
            }
            if duration + a.duration_mks > self.horizon {
                continue; // Σ duration ≤ T_rec(X)
            }
            let mut next_produced = produced.clone();
            next_produced.insert(a.target.clone());
            let mut next_edges = seen_edges.clone();
            next_edges.insert(a.id.clone());
            let mut next_ids = ids.clone();
            next_ids.push(a.id.clone());
            self.walk(
                &a.target,
                &next_produced,
                &next_edges,
                &next_ids,
                duration + a.duration_mks,
                delta + a.effect.get(self.entity_id).copied().unwrap_or(0.0),
            );
        }
    }
}

impl WorldGraph {
    pub fn means_set(&self) -> BTreeSet<String> {
        self.means.iter().cloned().collect()
    }

    /// ---------------------------------------------------- form (§3.5)
    pub fn form_errors(&self) -> Vec<String> {
        let mut errs: Vec<String> = Vec::new();
        for e in self.exchanges.iter() {
            if e.gives.is_empty() || e.wants.is_empty() {
                errs.push(format!("{}: an exchange basket is empty", e.id));
            }
            for v in e.gives.values().chain(e.wants.values()) {
                if *v <= 0.0 {
                    errs.push(format!("{}: a quote amount is not strictly positive", e.id));
                }
            }
            if e.gives.len() != 1 || e.wants.len() != 1 {
                errs.push(format!("{}: multi-resource baskets are reserved in this revision", e.id));
            }
            for k in e.gives.keys() {
                if e.wants.contains_key(k) {
                    errs.push(format!("{}: a trade cannot give and want the same resource", e.id));
                }
            }
            if e.duration_mks < 0.0 {
                errs.push(format!("{}: negative duration", e.id));
            }
        }
        let declared = self.means_set();
        for a in self.acts.iter() {
            if a.category.is_empty() {
                errs.push(format!("{}: an act without an admissible-means category", a.id));
            }
            if a.duration_mks < 0.0 {
                errs.push(format!("{}: negative duration", a.id));
            }
            for m in a.requires.iter() {
                if !declared.contains(m) {
                    errs.push(format!("{}: requires undeclared mean {}", a.id, m));
                }
            }
        }
        errs
    }

    /// -------------------------------------------- arbitrage test (§3.5)
    /// Bellman-Ford on `-ln(multiplier)`: a cycle of product > 1 is a negative
    /// cycle. The result is every edge that still relaxed on the final pass — a
    /// superset of the offending cycle, which is what a report needs to point at.
    pub fn arbitrage_edges(&self) -> Vec<String> {
        let mut srcs: Vec<String> = Vec::new();
        let mut dsts: Vec<String> = Vec::new();
        let mut ids: Vec<String> = Vec::new();
        let mut weights: Vec<f64> = Vec::new();
        let mut nodes: BTreeSet<String> = BTreeSet::new();
        for e in self.exchanges.iter() {
            let src = e.from_resource();
            let dst = e.to_resource();
            nodes.insert(src.clone());
            nodes.insert(dst.clone());
            srcs.push(src);
            dsts.push(dst);
            ids.push(e.id.clone());
            weights.push(-e.multiplier().ln());
        }
        let mut dist: BTreeMap<String, f64> = nodes.iter().map(|n| (n.clone(), 0.0)).collect();
        let mut hot: Vec<String> = Vec::new();
        for _ in 0..nodes.len() {
            hot.clear();
            for i in 0..ids.len() {
                let ds = dist.get(&srcs[i]).copied().unwrap_or(0.0);
                let dd = dist.get(&dsts[i]).copied().unwrap_or(0.0);
                if ds + weights[i] < dd - 1e-12 {
                    dist.insert(dsts[i].clone(), ds + weights[i]);
                    hot.push(ids[i].clone());
                }
            }
            if hot.is_empty() {
                return Vec::new();
            }
        }
        hot.sort();
        hot.dedup();
        hot
    }

    pub fn is_arbitrage_free(&self) -> bool {
        self.arbitrage_edges().is_empty()
    }

    /// ---------------------------------- fingerprint of the observation (§6.2)
    /// It covers what was observed — nodes with completeness, means, acts, quotes,
    /// M(S), T_rec and the counting horizon — and deliberately not the candidate
    /// set: a decision that moved with the options offered would not be
    /// reproducible (§4.2).
    pub fn observation_digest(
        &self,
        means_class: &[String],
        t_rec: &BTreeMap<String, f64>,
        counting_horizon_mks: Option<f64>,
    ) -> String {
        sha256_hex(self.observation_payload(means_class, t_rec, counting_horizon_mks).as_bytes())
    }

    /// The canonical payload the digest is taken over, exposed so a cross-port
    /// mismatch is a `diff` instead of a mystery.
    pub fn observation_payload(
        &self,
        means_class: &[String],
        t_rec: &BTreeMap<String, f64>,
        counting_horizon_mks: Option<f64>,
    ) -> String {
        let mut cls: Vec<String> = means_class.to_vec();
        cls.sort();
        let mut sorted_means: Vec<String> = self.means.clone();
        sorted_means.sort();
        let mut sorted_acts: Vec<String> = self.acts.iter().map(|a| a.id.clone()).collect();
        sorted_acts.sort();
        let mut sorted_ex: Vec<String> = self.exchanges.iter().map(|e| e.id.clone()).collect();
        sorted_ex.sort();

        // Keys are emitted in sorted order, as the canonical form requires: acts,
        // counting_horizon_mks, entities, exchanges, means, means_class, t_rec.
        let mut s = String::from("{\"acts\":[");
        let mut first = true;
        for id in sorted_acts.iter() {
            let a = match self.acts.iter().find(|a| &a.id == id) {
                Some(a) => a,
                None => continue,
            };
            if !first {
                s.push(',');
            }
            first = false;
            let mut req: Vec<String> = a.requires.clone();
            req.sort();
            s.push_str(&format!("{{\"category\":\"{}\"", a.category));
            s.push_str(&format!(",\"duration_mks\":\"{}\"", obs_f6(a.duration_mks)));
            s.push_str(",\"effect\":{");
            for (i, (k, v)) in a.effect.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&format!("\"{}\":\"{}\"", k, obs_f6(*v)));
            }
            s.push_str(&format!("}},\"id\":\"{}\",\"requires\":[", a.id));
            for (i, r) in req.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&format!("\"{}\"", r));
            }
            s.push_str(&format!("],\"source\":\"{}\",\"target\":\"{}\"}}", a.source, a.target));
        }
        s.push_str("],\"counting_horizon_mks\":");
        match counting_horizon_mks {
            Some(h) => s.push_str(&format!("\"{}\"", obs_f6(h))),
            None => s.push_str("null"),
        }
        s.push_str(",\"entities\":{");
        for (i, (eid, node)) in self.entities.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!(
                "\"{}\":{{\"current_dof\":\"{}\",\"observation\":\"{}\"}}",
                eid,
                obs_f6(node.current_dof),
                node.observation
            ));
        }
        s.push_str("},\"exchanges\":[");
        let mut ex_first = true;
        for id in sorted_ex.iter() {
            let e = match self.exchanges.iter().find(|e| &e.id == id) {
                Some(e) => e,
                None => continue,
            };
            if !ex_first {
                s.push(',');
            }
            ex_first = false;
            s.push_str(&format!("{{\"duration_mks\":\"{}\",\"gives\":{{", obs_f6(e.duration_mks)));
            for (i, (k, v)) in e.gives.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&format!("\"{}\":\"{}\"", k, obs_f6(*v)));
            }
            s.push_str(&format!("}},\"id\":\"{}\",\"wants\":{{", e.id));
            for (i, (k, v)) in e.wants.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&format!("\"{}\":\"{}\"", k, obs_f6(*v)));
            }
            s.push_str("}}");
        }
        s.push_str("],\"means\":[");
        for (i, m) in sorted_means.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("\"{}\"", m));
        }
        s.push_str("],\"means_class\":[");
        for (i, c) in cls.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("\"{}\"", c));
        }
        s.push_str("],\"t_rec\":{");
        for (i, (k, v)) in t_rec.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("\"{}\":\"{}\"", k, obs_f6(*v)));
        }
        s.push_str("}}");
        s
    }

    /// ------------------------------------------ the rate as an observation
    pub fn quotes(&self) -> Vec<Quote> {
        let mut out: Vec<Quote> = self
            .exchanges
            .iter()
            .map(|e| Quote {
                src: e.from_resource(),
                dst: e.to_resource(),
                id: e.id.clone(),
                mult: e.multiplier(),
                duration: e.duration_mks,
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    fn walk(
        adj: &BTreeMap<String, Vec<Quote>>,
        node: &str,
        goal: &str,
        seen: &BTreeSet<String>,
        ids: &[String],
        prod: f64,
        dur: f64,
        out: &mut Vec<PathCand>,
    ) {
        if ids.len() > MAX_PATH_EDGES {
            return;
        }
        if node == goal && !ids.is_empty() {
            out.push((ids.to_vec(), prod, dur));
            return;
        }
        let edges = match adj.get(node) {
            Some(e) => e,
            None => return,
        };
        for q in edges.iter() {
            if seen.contains(&q.dst) {
                continue;
            }
            let mut next_seen = seen.clone();
            next_seen.insert(q.dst.clone());
            let mut next_ids: Vec<String> = ids.to_vec();
            next_ids.push(q.id.clone());
            Self::walk(adj, &q.dst, goal, &next_seen, &next_ids, prod * q.mult, dur + q.duration, out);
        }
    }

    pub fn simple_paths(&self, a: &str, b: &str) -> Vec<PathCand> {
        let mut adj: BTreeMap<String, Vec<Quote>> = BTreeMap::new();
        for q in self.quotes() {
            adj.entry(q.src.clone()).or_insert_with(Vec::new).push(q);
        }
        for v in adj.values_mut() {
            v.sort_by(|x, y| x.id.cmp(&y.id));
        }
        let mut out: Vec<PathCand> = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        seen.insert(a.to_string());
        Self::walk(&adj, a, b, &seen, &[], 1.0, 0.0, &mut out);
        out
    }

    /// The axis rate (§3.5/§4.8): the best product of quotes along a path.
    /// Selection is canonical — quantized value first, then fewer edges, then the
    /// lexicographically smallest edge-id sequence. Unknown and absent are
    /// different answers: an incomplete observation yields `undetermined`, never a
    /// price.
    pub fn rate(&self, a: &str, b: &str, observation_complete: bool) -> RateResult {
        if a == b {
            let mut r = RateResult::simple("observed", "identity");
            r.rate = Some(1.0);
            return r;
        }
        if !self.is_arbitrage_free() {
            return RateResult::simple("undetermined", "observation is not arbitrage-free");
        }
        let mut cands = self.simple_paths(a, b);
        if cands.is_empty() {
            return if observation_complete {
                RateResult::simple("not_covered", &format!("no exchange path {}->{}", a, b))
            } else {
                RateResult::simple(
                    "undetermined",
                    &format!("no observed exchange path {}->{}, observation partial", a, b),
                )
            };
        }
        cands.sort_by(|x, y| {
            let qx = q6(x.1);
            let qy = q6(y.1);
            if qx != qy {
                return qy.partial_cmp(&qx).unwrap_or(std::cmp::Ordering::Equal);
            }
            if x.0.len() != y.0.len() {
                return x.0.len().cmp(&y.0.len());
            }
            x.0.cmp(&y.0)
        });
        let best = &cands[0];
        let mut r = RateResult::simple("observed", "canonical best path");
        r.rate = Some(q6(best.1));
        r.path = best.0.clone();
        r.duration_mks = best.2;
        r
    }

    /// ---------------------------------------------------- variety (§4.6)
    pub fn admissible_acts(&self, categories: &[String], horizon_mks: Option<f64>) -> Vec<ActEdge> {
        let horizon = match horizon_mks {
            Some(h) => h,
            None => return Vec::new(),
        };
        if categories.is_empty() {
            return Vec::new();
        }
        let declared = self.means_set();
        let mut out: Vec<ActEdge> = Vec::new();
        for a in self.acts.iter() {
            if !categories.iter().any(|c| c == &a.category) {
                continue;
            }
            if a.duration_mks > horizon {
                continue;
            }
            if a.requires.iter().any(|m| !declared.contains(m)) {
                continue;
            }
            out.push(a.clone());
        }
        out
    }

    /// §4.9 (v0.11): the **structurally admissible** acts, in canonical order.
    ///
    /// Structurally admissible means `category ∈ M(S)`, every `requires` mean
    /// declared, and `duration_mks <= T_rec(X)` — a *structural* property of the
    /// graph and the entity type. It is deliberately NOT the option predicate
    /// `admissible(o)` of §4.5: running that over a path's acts would let the τ and
    /// the resource map of one reading leak into a verdict §4.9 wants independent
    /// of the option set.
    pub fn path_edges(&self, categories: &[String], horizon_mks: Option<f64>) -> Vec<ActEdge> {
        let mut out = self.admissible_acts(categories, horizon_mks);
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// §4.9: every finite **simple directed path** of structurally admissible acts.
    ///
    /// A path is a chain of acts joined by `target → source` (an act's outcome is
    /// what the next act starts from), with no node and no edge repeated — a
    /// *simple* path, never an arbitrary walk: a zero-duration cycle with a positive
    /// effect would otherwise pump `Δ_P(X)` without spending time, and
    /// `duration(P) = 0` would satisfy every horizon.
    ///
    /// The enumeration is exhaustive over the finite simple paths and carries no
    /// edge bound. §4.9's condition is an existential over finite simple directed
    /// paths and states no length limit, so an implementation that silently stopped
    /// at a fixed depth could report `proven_unreachable` for an entity a longer
    /// path restores — a verdict §4.2 turns into an exclusion. A `safety_limit`,
    /// when an implementation sets one, bounds the number of **explored nodes** for
    /// resource protection only; an enumeration that hits it is marked incomplete
    /// and the verdict then reads `undetermined`, never a proof of unreachability.
    /// The depth is bounded anyway by simplicity (`|P| <= |V(G)| - 1`) and by the
    /// horizon.
    ///
    /// Results are returned in canonical order — greater `Δ_P` first, then fewer
    /// edges, then the lexicographically smallest identifier sequence — so two
    /// conformant implementations report the same witness.
    pub fn reachability_paths(
        &self,
        entity_id: &str,
        categories: &[String],
        horizon_mks: Option<f64>,
        safety_limit: Option<usize>,
    ) -> Vec<ReachPath> {
        self.enumeration_incomplete.set(false);
        self.enumerated.set(0);
        let horizon = match horizon_mks {
            Some(h) => h,
            None => return Vec::new(),
        };
        if categories.is_empty() {
            return Vec::new();
        }
        let edges = self.path_edges(categories, horizon_mks);
        let mut by_source: BTreeMap<String, Vec<ActEdge>> = BTreeMap::new();
        let mut starts: BTreeSet<String> = BTreeSet::new();
        for a in edges.iter() {
            by_source
                .entry(a.source.clone())
                .or_insert_with(Vec::new)
                .push(a.clone());
            starts.insert(a.source.clone());
        }
        let mut walk = PathWalk {
            by_source: &by_source,
            entity_id,
            horizon,
            safety_limit,
            results: Vec::new(),
            enumerated: 0,
            incomplete: false,
        };
        for s in starts.iter() {
            // No NODE twice, the origin included. `produced` is seeded with the
            // start vertex at every launch, so a path can neither return to its
            // origin (`A -> B -> A`) nor act on itself (`A -> A`): both repeat a
            // vertex and are therefore not simple paths. Without the seed a
            // zero-duration cycle with a positive effect would satisfy
            // `duration(P) = 0 <= T_rec` and pump `Δ_P(X)` for free, and the
            // verdict would report `reachable` where no simple path exists.
            let mut produced: BTreeSet<String> = BTreeSet::new();
            produced.insert(s.clone());
            walk.walk(s, &produced, &BTreeSet::new(), &Vec::new(), 0.0, 0.0);
        }
        self.enumerated.set(walk.enumerated);
        self.enumeration_incomplete.set(walk.incomplete);
        let mut out: Vec<ReachPath> = walk
            .results
            .into_iter()
            .filter(|p| !p.ids.is_empty())
            .collect();
        out.sort_by(|a, b| {
            let (qa, qb) = (q6(a.delta), q6(b.delta));
            if qa != qb {
                return qb.partial_cmp(&qa).unwrap_or(std::cmp::Ordering::Equal);
            }
            if a.ids.len() != b.ids.len() {
                return a.ids.len().cmp(&b.ids.len());
            }
            a.ids.join("\u{0}").cmp(&b.ids.join("\u{0}"))
        });
        out
    }

    /// Whether the last `reachability_paths` call finished. False means the
    /// implementation's own safety limit cut the search short, so the absence of a
    /// raising path is NOT a proof of unreachability.
    pub fn enumeration_complete(&self) -> bool {
        !self.enumeration_incomplete.get()
    }

    /// Sets this implementation's own protection bound for the path search.
    pub fn set_enumeration_safety_limit(&self, limit: Option<usize>) {
        self.enumeration_safety_limit.set(limit);
    }

    /// The entity's RESPONSE VECTORS (§4.6): admissible acts THIS entity can
    /// perform. Recoverability is a different question — there the pool is every
    /// admissible act whose effect raises the entity's DoF, whoever performs it,
    /// which is exactly why `revivable` has `V = 0` and is still reachable.
    pub fn reachable_acts(
        &self,
        entity_id: &str,
        categories: &[String],
        horizon_mks: Option<f64>,
    ) -> Vec<String> {
        let mut out: Vec<String> = self
            .admissible_acts(categories, horizon_mks)
            .into_iter()
            .filter(|a| a.source == entity_id)
            .map(|a| a.id)
            .collect();
        out.sort();
        out
    }

    pub fn v_count(&self, entity_id: &str, categories: &[String], horizon_mks: Option<f64>) -> usize {
        self.reachable_acts(entity_id, categories, horizon_mks).len()
    }

    /// --------------------------------------- numeraire weights (§4.6)
    /// `w_r` is the price of one unit of `r`, in the numeraire: what one unit
    /// COSTS TO ACQUIRE (the inverse of the best product from the numeraire to
    /// `r`). When no path *from* the numeraire exists the weight falls back to
    /// what one unit FETCHES — the only price the observation supports. A resource
    /// with neither direction carries NO weight, and must not silently take 1.0.
    pub fn weights_to(&self, numeraire: &str, resources: &[String]) -> BTreeMap<String, f64> {
        let mut uniq: BTreeSet<String> = BTreeSet::new();
        for r in resources {
            uniq.insert(r.clone());
        }
        let mut out: BTreeMap<String, f64> = BTreeMap::new();
        for r in uniq {
            if r == numeraire {
                out.insert(r, 1.0);
                continue;
            }
            let buy = self.rate(numeraire, &r, true);
            if buy.status == "observed" {
                if let Some(rate) = buy.rate {
                    if rate != 0.0 {
                        out.insert(r.clone(), q6(1.0 / rate));
                        continue;
                    }
                }
            }
            let sell = self.rate(&r, numeraire, true);
            if sell.status == "observed" {
                if let Some(rate) = sell.rate {
                    out.insert(r, q6(rate));
                }
            }
        }
        out
    }

    /// -------------------------------------------------- verdicts (§4.9)
    pub fn verdict(&self, entity_id: &str, categories: &[String], horizon_mks: Option<f64>) -> Verdict {
        self.verdict_with_dof(entity_id, categories, horizon_mks, None)
    }

    /// §4.9's rule, stated once.
    ///
    /// The condition is `DoF(X | h) + Δ_P(X) > 0` over a finite **simple** path of
    /// structurally admissible acts whose total duration fits `T_rec(X)`. For an
    /// entity at a known zero that reduces to `Δ_P(X) > 0`; for one already positive
    /// it is satisfied by the **trivial** path (`P` a single vertex, `Δ_P = 0`,
    /// duration 0), so a live entity is `reachable` without any path search —
    /// reading the rule as requiring a raising path in every case would make a live
    /// entity unrecoverable by construction.
    ///
    /// `proven_unreachable` is a claim of **completeness** — §4.2 turns it into an
    /// exclusion — so it is returned only when the enumeration actually finished.
    ///
    /// `dof_before` is `DoF(X | h)` under the reading being evaluated (§4.10). It is
    /// supplied by the caller because the verdict is asked under a *reading*, while
    /// the structural inputs — `G`, the paths, `M(S)`, `T_rec(X)` — are shared.
    pub fn verdict_with_dof(
        &self,
        entity_id: &str,
        categories: &[String],
        horizon_mks: Option<f64>,
        dof_before: Option<f64>,
    ) -> Verdict {
        let mut v = Verdict {
            entity_id: entity_id.to_string(),
            verdict: "undetermined".to_string(),
            witness: Vec::new(),
            admissible_seen: 0,
            reason: String::new(),
        };
        let node = match self.entities.get(entity_id) {
            Some(n) => n,
            None => {
                v.reason = "entity not observed at all".to_string();
                return v;
            }
        };
        if !node.complete() {
            v.reason = "observation is partial for this entity".to_string();
            return v;
        }
        if categories.is_empty() {
            v.reason = "admissible-means class M(S) is not declared".to_string();
            return v;
        }
        if horizon_mks.is_none() {
            v.reason = "recovery horizon T_rec is not declared".to_string();
            return v;
        }
        let base = dof_before.unwrap_or(node.current_dof);
        // The recoverability pool is NOT the entity's own repertoire: anyone's
        // admissible act may raise X's DoF. V counts what X itself can do.
        let pool = self.admissible_acts(categories, horizon_mks);
        v.admissible_seen = pool.len();
        if base > 0.0 {
            v.verdict = "reachable".to_string();
            v.reason =
                "DoF(X | h) > 0: the trivial path satisfies the condition (§4.9)".to_string();
            return v;
        }
        let paths = self.reachability_paths(
            entity_id,
            categories,
            horizon_mks,
            self.enumeration_safety_limit.get(),
        );
        if !self.enumeration_complete() {
            v.verdict = "undetermined".to_string();
            v.reason = "the path search was stopped at the implementation's safety limit, so \
                        absence of a raising path is not a proof (§4.9)"
                .to_string();
            return v;
        }
        for p in paths.iter() {
            if base + p.delta > 0.0 {
                v.verdict = "reachable".to_string();
                v.witness = p.ids.clone();
                v.reason =
                    "a structurally admissible simple path raises DoF within T_rec".to_string();
                return v;
            }
        }
        v.verdict = "proven_unreachable".to_string();
        v.reason = "complete observation, no structurally admissible simple path raises DoF \
                    within T_rec"
            .to_string();
        v
    }

    /// ---------------------------------- collapse act witness (§4.2)
    pub fn collapse_acts(
        &self,
        counted: &BTreeSet<String>,
        dof_before: &BTreeMap<String, f64>,
    ) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for a in self.acts.iter() {
            for (eid, delta) in a.effect.iter() {
                if !counted.contains(eid) {
                    continue;
                }
                let before = dof_before.get(eid).copied().unwrap_or(0.0);
                if before + delta <= 0.0 {
                    out.push(a.id.clone());
                    break;
                }
            }
        }
        out.sort();
        out
    }

    /// ---------------------------------------------------------- closure
    /// Acts removed directly, means removed together with their acts.
    pub fn with_closed(&self, closed: &[ClosedRef]) -> WorldGraph {
        let mut acts_off: BTreeSet<String> = BTreeSet::new();
        let mut means_off: BTreeSet<String> = BTreeSet::new();
        for c in closed {
            if c.kind == "act" {
                acts_off.insert(c.id.clone());
            }
            if c.kind == "mean" {
                means_off.insert(c.id.clone());
            }
        }
        let mut g = WorldGraph {
            entities: self.entities.clone(),
            means: Vec::new(),
            acts: Vec::new(),
            exchanges: self.exchanges.clone(),
            // §4.9 (v0.11): the closure graph is a fresh search domain, so it
            // carries the same safety limit and a fresh, empty bookkeeping state.
            enumeration_incomplete: std::cell::Cell::new(false),
            enumerated: std::cell::Cell::new(0),
            enumeration_safety_limit: std::cell::Cell::new(self.enumeration_safety_limit.get()),
        };
        for m in self.means.iter() {
            if !means_off.contains(m) {
                g.means.push(m.clone());
            }
        }
        for a in self.acts.iter() {
            if acts_off.contains(&a.id) {
                continue;
            }
            if a.requires.iter().any(|m| means_off.contains(m)) {
                continue;
            }
            g.acts.push(a.clone());
        }
        g
    }

    /// The two guards of §4.4. An empty result means the closure list is admissible.
    pub fn guard_closure(&self, closed: &[ClosedRef], own_act_id: &str) -> Vec<String> {
        let mut errs: Vec<String> = Vec::new();
        if !own_act_id.is_empty() {
            for c in closed {
                if c.kind == "act" && c.id == own_act_id {
                    errs.push("the option closes its own execution path".to_string());
                }
            }
        }
        if closed.is_empty() {
            errs.push("empty closure list".to_string());
        }
        errs
    }
}

/// The §3.5 observation as it arrives with a cycle: the graph plus the class of
/// admissible means, the recovery horizons, the counting horizon and the
/// numeraire. It is an OBSERVATION, so it is supplied beside the state and never
/// inside it.
#[derive(Clone, Debug, Default)]
pub struct WorldObservation {
    pub graph: WorldGraph,
    pub means_class: Vec<String>,
    pub t_rec: BTreeMap<String, f64>,
    pub counting_horizon_mks: Option<f64>,
    pub numeraire: Option<String>,
    pub procedure: String,
}
