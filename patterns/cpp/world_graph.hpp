// DOF-SPEC v0.7 §3.5 / §4.9 — the observed world graph and the named procedures
// over it. C++ reference port; mirrors patterns/python/world_graph.py and
// patterns/go/world_graph.go exactly.
//
// Design notes that matter for cross-port equality (§3.4.3, §11.9):
//   * every comparison of a derived rate is made on the CANONICALLY QUANTIZED
//     value (6 decimals), never on the raw double — comparison and serialization
//     then use one rounding, so the result is a function of the observation and
//     not of the order in which a port happened to multiply its factors;
//   * path ties are broken canonically: cheaper quantized value, then FEWER
//     EDGES, then lexicographic order of the edge-id sequence;
//   * the reference port enumerates simple paths (a world small enough for that),
//     but any implementation MUST reproduce the same canonical choice.

#pragma once

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <functional>
#include <limits>
#include <map>
#include <optional>
#include <set>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include "measurement.hpp"

namespace dof {

constexpr int kQDecimals = 6;
constexpr std::size_t kMaxPathEdges = 8;  // reference-port bound

// Canonical quantization of §3.4.3: one rounding for comparison AND emission.
// Rendered through the decimal string, exactly as the reference does, so a tie
// at the sixth decimal cannot resolve differently.
inline double q6(double x) {
    std::ostringstream os;
    os << std::fixed << std::setprecision(kQDecimals) << x;
    return std::strtod(os.str().c_str(), nullptr);
}

// Numbers are serialized in the canonical form of §3.4.3 — fixed six-decimal
// strings — never in a language's own float notation: `1000.0` and `1000` are
// one quantity, and three ports must agree on it.
inline std::string obs_f6(double x) {
    std::ostringstream os;
    os << std::fixed << std::setprecision(kQDecimals) << x;
    return os.str();
}

inline std::string obs_quote(const std::string& s) { return "\"" + s + "\""; }

// ------------------------------------------------------------------- nodes
struct EntityNode {
    std::string id;
    std::string observation = "complete";  // "complete" | "partial"
    double current_dof = 0.0;              // mirrored from the state
    bool complete() const { return observation == "complete"; }
};

struct ActEdge {
    std::string id;
    std::string source;
    std::string target;
    std::string category;
    std::vector<std::string> requires;
    std::map<std::string, double> effect;
    std::map<std::string, double> resources;
    double duration_mks = 0.0;
    // §3.5 (v0.9.1): measure-type act resolves an unmeasured resource.
    std::optional<std::string> discovers;
};

struct ExchangeEdge {
    std::string id;
    std::map<std::string, double> gives;  // what the actor hands over
    std::map<std::string, double> wants;  // what the actor receives
    double duration_mks = 0.0;

    std::string from_resource() const { return gives.empty() ? "" : gives.begin()->first; }
    std::string to_resource() const { return wants.empty() ? "" : wants.begin()->first; }
    // Units of `wants` obtained per one unit of `gives`.
    double multiplier() const { return wants.begin()->second / gives.begin()->second; }
};

// One entry of an option's `closed` list (§4.4).
struct ClosedRef {
    std::string kind;  // "act" | "mean"
    std::string id;
};

// ----------------------------------------------------------------- results
struct RateResult {
    std::string status;  // observed | undetermined | not_covered
    std::optional<double> rate;
    std::vector<std::string> path;
    double duration_mks = 0.0;
    std::string reason;
};

struct Verdict {
    std::string entity_id;
    std::string verdict;  // reachable | proven_unreachable | undetermined
    std::vector<std::string> witness;
    int admissible_seen = 0;
    std::string reason;
};

// One finite **simple directed path** of structurally admissible acts (§4.9):
// `ids` is the edge sequence, `duration` its total duration, `delta` the sum of
// its declared effects on the entity the path is enumerated for.
//
// Named `ReachPath`, not `PathCand`: `PathCand` in this file is an *exchange*
// path (a sequence of quotes with a product), and the two are different
// procedures over different graphs. Sharing the name would invite sharing the
// sort order, which is exactly what §4.9's canonical witness must not inherit.
struct ReachPath {
    std::vector<std::string> ids;
    double duration = 0.0;
    double delta = 0.0;
};

// ------------------------------------------------------------------- graph
struct WorldGraph {
    std::map<std::string, EntityNode> entities;
    std::vector<std::string> means;
    std::vector<ActEdge> acts;
    std::vector<ExchangeEdge> exchanges;

    // Per-search scratch of §4.9. `enumeration_incomplete` is the only piece the
    // verdict reads: an enumeration cut short by the implementation's own safety
    // limit has NOT proved that no raising path exists, so the verdict it feeds
    // must read `undetermined` rather than `proven_unreachable`. `mutable`
    // because the verdict is a query — it is computed on a const graph, and the
    // scratch is not part of the graph's value.
    mutable bool enumeration_incomplete = false;
    mutable int enumerated = 0;
    std::optional<int> enumeration_safety_limit;

    std::set<std::string> means_set() const {
        std::set<std::string> out;
        for (const auto& m : means) out.insert(m);
        return out;
    }

    // ------------------------------------------------------ form (§3.5)
    std::vector<std::string> form_errors() const {
        std::vector<std::string> errs;
        for (const auto& e : exchanges) {
            if (e.gives.empty() || e.wants.empty()) errs.push_back(e.id + ": an exchange basket is empty");
            for (const auto& kv : e.gives) {
                if (kv.second <= 0.0) errs.push_back(e.id + ": a quote amount is not strictly positive");
            }
            for (const auto& kv : e.wants) {
                if (kv.second <= 0.0) errs.push_back(e.id + ": a quote amount is not strictly positive");
            }
            if (e.gives.size() != 1 || e.wants.size() != 1) {
                errs.push_back(e.id + ": multi-resource baskets are reserved in this revision");
            }
            for (const auto& kv : e.gives) {
                if (e.wants.count(kv.first) > 0) {
                    errs.push_back(e.id + ": a trade cannot give and want the same resource");
                }
            }
            if (e.duration_mks < 0.0) errs.push_back(e.id + ": negative duration");
        }
        const std::set<std::string> means_decl = means_set();
        for (const auto& a : acts) {
            if (a.category.empty()) errs.push_back(a.id + ": an act without an admissible-means category");
            if (a.duration_mks < 0.0) errs.push_back(a.id + ": negative duration");
            for (const auto& m : a.requires) {
                if (means_decl.count(m) == 0) errs.push_back(a.id + ": requires undeclared mean " + m);
            }
        }
        return errs;
    }

    // ------------------------------------------------ arbitrage test (§3.5)
    // Bellman-Ford on `-ln(multiplier)`: a cycle of product > 1 is a negative
    // cycle. The result is every edge that still relaxed on the final pass — a
    // superset of the offending cycle, which is what a report needs to point at.
    std::vector<std::string> arbitrage_edges() const {
        struct Edge { std::string src, dst, id; double w; };
        std::vector<Edge> edges;
        std::set<std::string> node_set;
        for (const auto& e : exchanges) {
            const std::string src = e.from_resource();
            const std::string dst = e.to_resource();
            node_set.insert(src);
            node_set.insert(dst);
            edges.push_back(Edge{src, dst, e.id, -std::log(e.multiplier())});
        }
        std::map<std::string, double> dist;
        for (const auto& n : node_set) dist[n] = 0.0;  // virtual source: all at 0
        std::vector<std::string> hot;
        for (std::size_t i = 0; i < node_set.size(); ++i) {
            hot.clear();
            for (const auto& e : edges) {
                if (dist[e.src] + e.w < dist[e.dst] - 1e-12) {
                    dist[e.dst] = dist[e.src] + e.w;
                    hot.push_back(e.id);
                }
            }
            if (hot.empty()) return {};
        }
        std::set<std::string> uniq(hot.begin(), hot.end());
        return std::vector<std::string>(uniq.begin(), uniq.end());
    }

    bool is_arbitrage_free() const { return arbitrage_edges().empty(); }

    // ------------------------------------ fingerprint of the observation (§6.2)
    // It covers what was observed — nodes with completeness, means, acts, quotes,
    // M(S), T_rec and the counting horizon — and deliberately not the candidate
    // set: a decision that moved with the options offered would not be
    // reproducible (§4.2).
    std::string observation_digest(const std::vector<std::string>& means_class,
                                   const std::map<std::string, double>& t_rec,
                                   const std::optional<double>& counting_horizon_mks) const {
        std::vector<std::string> sorted_acts;
        for (const auto& a : acts) sorted_acts.push_back(a.id);
        std::sort(sorted_acts.begin(), sorted_acts.end());
        std::vector<std::string> sorted_ex;
        for (const auto& e : exchanges) sorted_ex.push_back(e.id);
        std::sort(sorted_ex.begin(), sorted_ex.end());
        std::vector<std::string> cls = means_class;
        std::sort(cls.begin(), cls.end());
        std::vector<std::string> sorted_means = means;
        std::sort(sorted_means.begin(), sorted_means.end());

        // Keys are emitted in sorted order, as the canonical form requires:
        // acts, counting_horizon_mks, entities, exchanges, means, means_class, t_rec.
        std::ostringstream os;
        os << "{\"acts\":[";
        bool first = true;
        for (const auto& id : sorted_acts) {
            const ActEdge* a = nullptr;
            for (const auto& cand : acts) {
                if (cand.id == id) { a = &cand; break; }
            }
            if (a == nullptr) continue;
            if (!first) os << ",";
            first = false;
            std::vector<std::string> req = a->requires;
            std::sort(req.begin(), req.end());
            os << "{\"category\":" << obs_quote(a->category)
               << ",\"duration_mks\":" << obs_quote(obs_f6(a->duration_mks))
               << ",\"effect\":{";
            bool ef = true;
            for (const auto& kv : a->effect) {
                if (!ef) os << ",";
                ef = false;
                os << obs_quote(kv.first) << ":" << obs_quote(obs_f6(kv.second));
            }
            os << "},\"id\":" << obs_quote(a->id) << ",\"requires\":[";
            bool rf = true;
            for (const auto& r : req) {
                if (!rf) os << ",";
                rf = false;
                os << obs_quote(r);
            }
            os << "],\"source\":" << obs_quote(a->source)
               << ",\"target\":" << obs_quote(a->target) << "}";
        }
        os << "],\"counting_horizon_mks\":";
        if (counting_horizon_mks) {
            os << obs_quote(obs_f6(*counting_horizon_mks));
        } else {
            os << "null";
        }
        os << ",\"entities\":{";
        bool ent_first = true;
        for (const auto& kv : entities) {
            if (!ent_first) os << ",";
            ent_first = false;
            os << obs_quote(kv.first) << ":{\"current_dof\":" << obs_quote(obs_f6(kv.second.current_dof))
               << ",\"observation\":" << obs_quote(kv.second.observation) << "}";
        }
        os << "},\"exchanges\":[";
        bool ex_first = true;
        for (const auto& id : sorted_ex) {
            const ExchangeEdge* e = nullptr;
            for (const auto& cand : exchanges) {
                if (cand.id == id) { e = &cand; break; }
            }
            if (e == nullptr) continue;
            if (!ex_first) os << ",";
            ex_first = false;
            os << "{\"duration_mks\":" << obs_quote(obs_f6(e->duration_mks)) << ",\"gives\":{";
            bool gf = true;
            for (const auto& kv : e->gives) {
                if (!gf) os << ",";
                gf = false;
                os << obs_quote(kv.first) << ":" << obs_quote(obs_f6(kv.second));
            }
            os << "},\"id\":" << obs_quote(e->id) << ",\"wants\":{";
            bool wf = true;
            for (const auto& kv : e->wants) {
                if (!wf) os << ",";
                wf = false;
                os << obs_quote(kv.first) << ":" << obs_quote(obs_f6(kv.second));
            }
            os << "}}";
        }
        os << "],\"means\":[";
        bool mf = true;
        for (const auto& m : sorted_means) {
            if (!mf) os << ",";
            mf = false;
            os << obs_quote(m);
        }
        os << "],\"means_class\":[";
        bool cf = true;
        for (const auto& c : cls) {
            if (!cf) os << ",";
            cf = false;
            os << obs_quote(c);
        }
        os << "],\"t_rec\":{";
        bool tf = true;
        for (const auto& kv : t_rec) {
            if (!tf) os << ",";
            tf = false;
            os << obs_quote(kv.first) << ":" << obs_quote(obs_f6(kv.second));
        }
        os << "}}";
        return sha256_hex(os.str());
    }

    // -------------------------------------------- the rate as an observation
    struct Quote {
        std::string src, dst, id;
        double mult = 1.0;
        double duration = 0.0;
    };

    std::vector<Quote> quotes() const {
        std::vector<Quote> out;
        for (const auto& e : exchanges) {
            out.push_back(Quote{e.from_resource(), e.to_resource(), e.id, e.multiplier(), e.duration_mks});
        }
        std::sort(out.begin(), out.end(), [](const Quote& a, const Quote& b) { return a.id < b.id; });
        return out;
    }

    struct PathCand {
        std::vector<std::string> ids;
        double product = 1.0;
        double duration = 0.0;
    };

    std::vector<PathCand> simple_paths(const std::string& a, const std::string& b) const {
        std::map<std::string, std::vector<Quote>> adj;
        for (const auto& q : quotes()) adj[q.src].push_back(q);
        for (auto& kv : adj) {
            std::sort(kv.second.begin(), kv.second.end(),
                      [](const Quote& x, const Quote& y) { return x.id < y.id; });
        }
        std::vector<PathCand> out;
        struct Walk {
            const std::map<std::string, std::vector<Quote>>* adj;
            const std::string* goal;
            std::vector<PathCand>* out;
            void operator()(const std::string& node, std::set<std::string> seen,
                            std::vector<std::string> ids, double prod, double dur) const {
                if (ids.size() > kMaxPathEdges) return;
                if (node == *goal && !ids.empty()) {
                    out->push_back(PathCand{ids, prod, dur});
                    return;
                }
                auto it = adj->find(node);
                if (it == adj->end()) return;
                for (const auto& q : it->second) {
                    if (seen.count(q.dst) > 0) continue;
                    std::set<std::string> next = seen;
                    next.insert(q.dst);
                    std::vector<std::string> next_ids = ids;
                    next_ids.push_back(q.id);
                    (*this)(q.dst, next, next_ids, prod * q.mult, dur + q.duration);
                }
            }
        };
        Walk{&adj, &b, &out}(a, std::set<std::string>{a}, {}, 1.0, 0.0);
        return out;
    }

    // The axis rate (§3.5/§4.8): the best product of quotes along a path.
    // Selection is canonical — quantized value first, then fewer edges, then the
    // lexicographically smallest edge-id sequence. Unknown and absent are
    // different answers: an incomplete observation yields `undetermined`, never a
    // price.
    RateResult rate(const std::string& a, const std::string& b,
                    bool observation_complete = true) const {
        if (a == b) {
            RateResult r;
            r.status = "observed";
            r.rate = 1.0;
            r.reason = "identity";
            return r;
        }
        if (!is_arbitrage_free()) {
            RateResult r;
            r.status = "undetermined";
            r.reason = "observation is not arbitrage-free";
            return r;
        }
        std::vector<PathCand> cands = simple_paths(a, b);
        if (cands.empty()) {
            RateResult r;
            if (observation_complete) {
                r.status = "not_covered";
                r.reason = "no exchange path " + a + "->" + b;
            } else {
                r.status = "undetermined";
                r.reason = "no observed exchange path " + a + "->" + b + ", observation partial";
            }
            return r;
        }
        std::sort(cands.begin(), cands.end(), [](const PathCand& x, const PathCand& y) {
            const double qx = q6(x.product), qy = q6(y.product);
            if (qx != qy) return qx > qy;  // the largest product wins
            if (x.ids.size() != y.ids.size()) return x.ids.size() < y.ids.size();  // fewer edges
            return x.ids < y.ids;
        });
        const PathCand& best = cands.front();
        RateResult r;
        r.status = "observed";
        r.rate = q6(best.product);
        r.path = best.ids;
        r.duration_mks = best.duration;
        r.reason = "canonical best path";
        return r;
    }

    // -------------------------------------------------------- variety (§4.6)
    std::vector<ActEdge> admissible_acts(const std::vector<std::string>& categories,
                                         const std::optional<double>& horizon_mks) const {
        if (categories.empty() || !horizon_mks) return {};
        std::set<std::string> cat(categories.begin(), categories.end());
        const std::set<std::string> means_decl = means_set();
        std::vector<ActEdge> out;
        for (const auto& a : acts) {
            if (cat.count(a.category) == 0) continue;
            if (a.duration_mks > *horizon_mks) continue;
            bool ok = true;
            for (const auto& m : a.requires) {
                if (means_decl.count(m) == 0) { ok = false; break; }
            }
            if (ok) out.push_back(a);
        }
        return out;
    }

    // The entity's RESPONSE VECTORS (§4.6): admissible acts THIS entity can
    // perform. Recoverability is a different question — there the pool is every
    // admissible act whose effect raises the entity's DoF, whoever performs it.
    std::vector<std::string> reachable_acts(const std::string& entity_id,
                                            const std::vector<std::string>& categories,
                                            const std::optional<double>& horizon_mks) const {
        std::vector<std::string> out;
        for (const auto& a : admissible_acts(categories, horizon_mks)) {
            if (a.source == entity_id) out.push_back(a.id);
        }
        std::sort(out.begin(), out.end());
        return out;
    }

    int v_count(const std::string& entity_id, const std::vector<std::string>& categories,
                const std::optional<double>& horizon_mks) const {
        return static_cast<int>(reachable_acts(entity_id, categories, horizon_mks).size());
    }

    // --------------------------------------------- numeraire weights (§4.6)
    // `w_r` is the price of one unit of `r`, in the numeraire: what one unit
    // COSTS TO ACQUIRE (the inverse of the best product from the numeraire to
    // `r`). When no path *from* the numeraire exists the weight falls back to
    // what one unit FETCHES — the only price the observation supports. A resource
    // with neither direction carries NO weight, and must not silently take 1.0.
    std::map<std::string, double> weights_to(const std::string& numeraire,
                                             const std::vector<std::string>& resources) const {
        std::set<std::string> uniq(resources.begin(), resources.end());
        std::map<std::string, double> out;
        for (const auto& r : uniq) {
            if (r == numeraire) {
                out[r] = 1.0;
                continue;
            }
            RateResult buy = rate(numeraire, r, true);
            if (buy.status == "observed" && buy.rate && *buy.rate != 0.0) {
                out[r] = q6(1.0 / *buy.rate);
                continue;
            }
            RateResult sell = rate(r, numeraire, true);
            if (sell.status == "observed" && sell.rate) out[r] = q6(*sell.rate);
        }
        return out;
    }

    // ----------------------------------------------------- verdicts (§4.9)
    // The structurally admissible acts, in canonical id order. Separate from
    // `admissible_acts` so that the path procedure has one entry point and the
    // pool count has another: the pool is what `admissible_seen` reports, the
    // edges are what the enumeration walks.
    std::vector<ActEdge> path_edges(const std::vector<std::string>& categories,
                                    const std::optional<double>& horizon_mks) const {
        std::vector<ActEdge> out = admissible_acts(categories, horizon_mks);
        std::stable_sort(out.begin(), out.end(),
                         [](const ActEdge& a, const ActEdge& b) { return a.id < b.id; });
        return out;
    }

    // Enumerates every finite **simple directed path** of structurally admissible
    // acts (§4.9).
    //
    // A path is a chain of acts joined by `target → source` (an act's outcome is
    // what the next act starts from), with no node and no edge repeated — a
    // *simple* path, never an arbitrary walk: a zero-duration cycle with a
    // positive effect would otherwise pump `Δ_P(X)` without spending time, and
    // `duration(P) = 0` would satisfy every horizon.
    //
    // The enumeration is exhaustive over the finite simple paths and carries no
    // edge bound. §4.9's condition is an existential over finite simple directed
    // paths and states no length limit, so an implementation that silently
    // stopped at a fixed depth could report `proven_unreachable` for an entity a
    // longer path restores — a verdict §4.2 turns into an exclusion. A
    // `safety_limit`, when an implementation sets one, bounds the number of
    // **explored nodes** for resource protection only; an enumeration that hits
    // it is marked incomplete and the verdict then reads `undetermined`, never a
    // proof of unreachability. The depth is bounded anyway by simplicity
    // (`|P| <= |V(G)| - 1`) and by the horizon.
    //
    // Results are returned in canonical order — greater `Δ_P` first, then fewer
    // edges, then the lexicographically smallest identifier sequence — so two
    // conformant implementations report the same witness.
    std::vector<ReachPath> reachability_paths(
        const std::string& entity_id, const std::vector<std::string>& categories,
        const std::optional<double>& horizon_mks,
        const std::optional<int>& safety_limit) const {
        enumeration_incomplete = false;
        enumerated = 0;
        std::vector<ReachPath> out;
        if (categories.empty() || !horizon_mks) return out;
        std::vector<ActEdge> edges = path_edges(categories, horizon_mks);
        std::map<std::string, std::vector<ActEdge>> by_source;
        for (const auto& a : edges) by_source[a.source].push_back(a);

        std::vector<ReachPath> results;
        std::function<void(const std::string&, std::set<std::string>,
                           std::set<std::string>, std::vector<std::string>, double, double)>
            walk;
        walk = [&](const std::string& node, std::set<std::string> produced,
                   std::set<std::string> seen_edges, std::vector<std::string> ids,
                   double duration, double delta) {
            // Record every prefix: a path need not be maximal, and a shorter
            // prefix may be the one that lifts the entity off a known zero.
            results.push_back(ReachPath{ids, duration, delta});
            enumerated++;
            if (safety_limit && enumerated > *safety_limit) {
                enumeration_incomplete = true;
                return;
            }
            auto it = by_source.find(node);
            if (it == by_source.end()) return;
            for (const auto& a : it->second) {
                if (seen_edges.count(a.id) > 0) continue;  // no edge twice
                // No NODE twice, the origin included. `produced` is seeded with
                // the start vertex at every launch, so a path can neither return
                // to its origin (`A -> B -> A`) nor act on itself (`A -> A`):
                // both repeat a vertex and are therefore not simple paths.
                if (produced.count(a.target) > 0) continue;
                if (duration + a.duration_mks > *horizon_mks) continue;  // Σ ≤ T_rec
                std::set<std::string> next_produced = produced;
                next_produced.insert(a.target);
                std::set<std::string> next_edges = seen_edges;
                next_edges.insert(a.id);
                std::vector<std::string> next_ids = ids;
                next_ids.push_back(a.id);
                double step = 0.0;
                auto eff = a.effect.find(entity_id);
                if (eff != a.effect.end()) step = eff->second;
                walk(a.target, next_produced, next_edges, next_ids,
                     duration + a.duration_mks, delta + step);
            }
        };
        std::set<std::string> starts;
        for (const auto& a : edges) starts.insert(a.source);
        for (const auto& s : starts) {
            std::set<std::string> seeded;
            seeded.insert(s);
            walk(s, seeded, {}, {}, 0.0, 0.0);
        }
        // Non-empty paths only: the empty path changes nothing and cannot raise
        // a DoF.
        for (const auto& r : results) {
            if (!r.ids.empty()) out.push_back(r);
        }
        std::stable_sort(out.begin(), out.end(), [](const ReachPath& x, const ReachPath& y) {
            const double qx = q6(x.delta), qy = q6(y.delta);
            if (qx != qy) return qx > qy;
            if (x.ids.size() != y.ids.size()) return x.ids.size() < y.ids.size();
            return join_ids(x.ids) < join_ids(y.ids);
        });
        return out;
    }

    // Whether the last `reachability_paths` call finished. False means the
    // implementation's own safety limit cut the search short, so the absence of a
    // raising path is NOT a proof of unreachability.
    bool enumeration_complete() const { return !enumeration_incomplete; }

    // Sets this implementation's own protection bound for §4.9's search.
    // `std::nullopt` (the default, and the normative behaviour of §4.9)
    // enumerates every finite simple path: the existential the section states is
    // over all of them and names no depth.
    void set_enumeration_safety_limit(const std::optional<int>& limit) {
        enumeration_safety_limit = limit;
    }

    // §4.9's verdict with the caller's own `DoF(X | h)`; `dof_before` is
    // `std::nullopt` when the caller does not supply it and the verdict falls back
    // to the DoF the graph mirrors for that entity (the degenerate case, not the
    // rule: the verdict is a value of `(G, state_h)`).
    Verdict verdict(const std::string& entity_id, const std::vector<std::string>& categories,
                    const std::optional<double>& horizon_mks) const {
        return verdict_with_dof(entity_id, categories, horizon_mks, std::nullopt);
    }

    // §4.9's rule, stated once.
    //
    // The condition is `DoF(X | h) + Δ_P(X) > 0` over a finite **simple** path of
    // structurally admissible acts whose total duration fits `T_rec(X)`. For an
    // entity at a known zero that reduces to `Δ_P(X) > 0`; for one already
    // positive it is satisfied by the **trivial** path (`P` a single vertex,
    // `Δ_P = 0`, duration 0), so a live entity is `reachable` without any path
    // search — reading the rule as requiring a raising path in every case would
    // make a live entity unrecoverable by construction.
    //
    // `proven_unreachable` is a claim of **completeness** — §4.2 turns it into an
    // exclusion — so it is returned only when the enumeration actually finished.
    Verdict verdict_with_dof(const std::string& entity_id,
                             const std::vector<std::string>& categories,
                             const std::optional<double>& horizon_mks,
                             const std::optional<double>& dof_before) const {
        Verdict v;
        v.entity_id = entity_id;
        auto it = entities.find(entity_id);
        if (it == entities.end()) {
            v.verdict = "undetermined";
            v.reason = "entity not observed at all";
            return v;
        }
        if (!it->second.complete()) {
            v.verdict = "undetermined";
            v.reason = "observation is partial for this entity";
            return v;
        }
        if (categories.empty()) {
            v.verdict = "undetermined";
            v.reason = "admissible-means class M(S) is not declared";
            return v;
        }
        if (!horizon_mks) {
            v.verdict = "undetermined";
            v.reason = "recovery horizon T_rec is not declared";
            return v;
        }
        double base = it->second.current_dof;
        if (dof_before) base = *dof_before;
        // The recoverability pool is NOT the entity's own repertoire: anyone's
        // admissible act may raise X's DoF. V counts what X itself can do.
        std::vector<ActEdge> pool = admissible_acts(categories, horizon_mks);
        v.admissible_seen = static_cast<int>(pool.size());
        if (base > 0.0) {
            v.verdict = "reachable";
            v.reason = "DoF(X | h) > 0: the trivial path satisfies the condition (§4.9)";
            return v;
        }
        std::vector<ReachPath> paths =
            reachability_paths(entity_id, categories, horizon_mks, enumeration_safety_limit);
        if (!enumeration_complete()) {
            v.verdict = "undetermined";
            v.reason = "the path search was stopped at the implementation's safety "
                       "limit, so absence of a raising path is not a proof (§4.9)";
            return v;
        }
        for (const auto& p : paths) {
            if (base + p.delta > 0.0) {
                v.verdict = "reachable";
                v.witness = p.ids;
                v.reason = "a structurally admissible simple path raises DoF within T_rec";
                return v;
            }
        }
        v.verdict = "proven_unreachable";
        v.reason = "complete observation, no structurally admissible simple path "
                   "raises DoF within T_rec";
        return v;
    }

    // The canonical join of a path's identifier sequence, used only for the
    // lexicographic tie-break: a separator that cannot occur inside an identifier
    // (`\0`) so that `["a","b"]` and `["ab"]` cannot collide.
    static std::string join_ids(const std::vector<std::string>& ids) {
        std::string out;
        for (std::size_t i = 0; i < ids.size(); ++i) {
            if (i > 0) out.push_back('\0');
            out += ids[i];
        }
        return out;
    }

    // ------------------------------------ collapse act witness (§4.2)
    std::vector<std::string> collapse_acts(const std::set<std::string>& counted,
                                           const std::map<std::string, double>& dof_before) const {
        std::vector<std::string> out;
        for (const auto& a : acts) {
            for (const auto& kv : a.effect) {
                if (counted.count(kv.first) == 0) continue;
                auto it = dof_before.find(kv.first);
                const double before = (it == dof_before.end()) ? 0.0 : it->second;
                if (before + kv.second <= 0.0) {
                    out.push_back(a.id);
                    break;
                }
            }
        }
        std::sort(out.begin(), out.end());
        return out;
    }

    // ------------------------------------------------------------- closure
    // Acts removed directly, means removed together with their acts.
    WorldGraph with_closed(const std::vector<ClosedRef>& closed) const {
        std::set<std::string> acts_off, means_off;
        for (const auto& c : closed) {
            if (c.kind == "act") acts_off.insert(c.id);
            if (c.kind == "mean") means_off.insert(c.id);
        }
        WorldGraph g;
        g.entities = entities;
        g.exchanges = exchanges;
        for (const auto& m : means) {
            if (means_off.count(m) == 0) g.means.push_back(m);
        }
        for (const auto& a : acts) {
            if (acts_off.count(a.id) > 0) continue;
            bool touched = false;
            for (const auto& m : a.requires) {
                if (means_off.count(m) > 0) { touched = true; break; }
            }
            if (!touched) g.acts.push_back(a);
        }
        return g;
    }

    // The two guards of §4.4. An empty result means the closure list is admissible.
    std::vector<std::string> guard_closure(const std::vector<ClosedRef>& closed,
                                           const std::string& own_act_id = "") const {
        std::vector<std::string> errs;
        if (!own_act_id.empty()) {
            for (const auto& c : closed) {
                if (c.kind == "act" && c.id == own_act_id) {
                    errs.push_back("the option closes its own execution path");
                }
            }
        }
        if (closed.empty()) errs.push_back("empty closure list");
        return errs;
    }
};

// The §3.5 observation as it arrives with a cycle: the graph plus the class of
// admissible means, the recovery horizons, the counting horizon and the
// numeraire. It is an OBSERVATION, so it is supplied beside the state and never
// inside it.
struct WorldObservation {
    WorldGraph graph;
    std::vector<std::string> means_class;
    std::map<std::string, double> t_rec;
    std::optional<double> counting_horizon_mks;
    std::optional<std::string> numeraire;
    std::string procedure = "perception-v1:world_verdicts";
};

}  // namespace dof
