// DOF-Core Perception & Mapping layer (C++ port).
// Builds a SystemStateMatrix **through the measurement layer** (§4.6–§4.8):
// raw lens inputs -> ψ per lens -> the product that becomes current_dof, plus
// the frozen declaration and its digest (§3.4), plus the acting agent's means
// and the exchange layer the resource gate of §4.8 decides against.

#pragma once

#include <algorithm>
#include <cmath>
#include <limits>
#include <map>
#include <optional>
#include <string>
#include <unordered_map>
#include <utility>

#include "dof_core.hpp"
#include "measurement.hpp"

// The reserved top-level observation that carries the resource layer (§3.2,
// §4.8) instead of describing an entity. It is part of the ruler: its units,
// groups and rates enter the hashed declaration, so a ruler that declares
// different units is a different ruler.
inline const std::string kResourceLayerKey = "resource_layer";

// The reserved key that carries the §3.5 observation of the world. Like the
// resource layer it is an *observation*, not an entity: the same state plus a
// different observation is a different decision, and the report says which one.
inline const std::string kWorldKey = "world";

// §4.6/§4.9: declared derived numbers MUST equal what their procedures compute.
// Returns the mismatches (empty = the ruler is honest). A declaration that claims
// a counter its own observation does not support is exactly the "declared, not
// derived" defect this revision removes.
inline std::vector<std::string> verify_graph_derived(
    const dof::MeasurementDeclaration& declaration, const dof::WorldGraph& graph,
    const std::optional<double>& counting_horizon) {
    std::vector<std::string> problems;
    for (const auto& kv : declaration.verdicts) {
        const std::string& entity_id = kv.first;
        const dof::VerdictRecord& declared = kv.second;
        const int v_here = graph.v_count(entity_id, declaration.means_class, counting_horizon);
        if (declared.v != v_here) {
            problems.push_back(entity_id + ": declared V=" + std::to_string(declared.v) +
                               " but the counting procedure gives " + std::to_string(v_here));
        }
        const std::string verdict_here =
            graph.verdict(entity_id, declaration.means_class, declared.t_rec_mks).verdict;
        if (declared.verdict != verdict_here) {
            problems.push_back(entity_id + ": declared verdict " + declared.verdict +
                               " but the verdict procedure returns " + verdict_here);
        }
    }
    return problems;
}

struct ResourceLayer {
    std::map<std::string, double> means;                          // the agent's stock
    std::vector<std::vector<std::string>> groups;                 // derived exchange groups
    std::map<std::string, dof::Rate> rates;                       // "from->to" -> {rate, duration_mks}
    std::vector<dof::ResourceUnit> resources;                     // declared units
    std::map<std::string, dof::MandateValue> mandate;             // declared mandate + limits
    // §3.2b (v0.11): the active individual deadlines declared alongside the map.
    // A `nullopt` entry is an UNMEASURED deadline and makes τ unknown — it is not
    // absent, and the measured ones alone must not decide (§10(w), §10(ao)).
    std::map<std::string, std::optional<double>> deadlines;
    // §3.2b (v0.11): the `tau` observation of the resource map. A disengaged
    // option means the map declares no τ at all; an engaged option with no value
    // means the map declares τ **unmeasured** — which is neither absent nor zero.
    // `std::optional<std::optional<double>>` is the three-case shape, and the three
    // cases are exactly what §3.2b distinguishes.
    std::optional<std::optional<double>> tau;
};

// Raw observation of one entity. A lens left empty is **unmeasured**: u(t)
// applies to it, dof_known becomes false, and §4.2 keeps the entity in calc.
struct RawObservation {
    bool is_autonomous = true;
    double agency_index = 0.0;
    bool is_collapse_source = false;
    double time_to_collapse_mks = 0.0;
    dof::LensObservation lenses;
    // Set on the reserved `resource_layer` entry only.
    std::optional<ResourceLayer> resource_layer;
    // Set on the reserved `world` entry only: the §3.5 observation of the world.
    std::optional<dof::WorldObservation> world;
    // Set on the reserved `measurement_durations` entry only (§4.7, v0.11): the
    // declared `t_m`, `t_v` per lens — ruler content, hashed by §3.4.1.
    std::optional<std::map<std::string, std::map<std::string, double>>> measurement_durations;
    // Set on the reserved `measurement_schedule` entry only (§4.7, v0.11): the
    // declared `t` per lens.
    std::optional<std::map<std::string, double>> measurement_schedule;
};

// §4.7 (v0.11): the reserved keys that carry ruler content instead of describing
// an entity. Stated once: the mapper skips them in **every** entity pass, and a
// pass that skips them by listing two names is how the third one ends up measured
// as if it were an entity with a default τ of zero.
inline const std::string kMeasurementDurationsKey = "measurement_durations";
inline const std::string kMeasurementScheduleKey = "measurement_schedule";

inline bool is_reserved_key(const std::string& name) {
    return name == kResourceLayerKey || name == kWorldKey ||
           name == kMeasurementDurationsKey || name == kMeasurementScheduleKey;
}

class GraphMapper {
    double context_switch_cost_;

public:
    std::string psi_id = "perception-v1";
    std::optional<double> u0_prior_q;
    // The declaration frozen on the state being built; the orchestrator hands
    // it to the audit report (§6.2).
    mutable std::optional<dof::MeasurementDeclaration> last_declaration;
    // The observation the state was decided over (§3.5/§4.9), kept beside the
    // state and never inside it: a world graph is a Perception artifact, exactly
    // like the derived groups and the observed rates.
    mutable std::optional<ObservationContext> last_observation;
    // Mismatches between the declared derived numbers and what the named
    // procedures recompute over the observation (§4.6/§4.9). Empty = honest.
    mutable std::vector<std::string> last_graph_problems;

    explicit GraphMapper(double context_switch_cost = 0.05)
        : context_switch_cost_(context_switch_cost) {}

    SystemStateMatrix poll_environment(
        const std::unordered_map<std::string, RawObservation>& raw) const
    {
        // The resource layer is analysis-side data, not an entity: pull it out
        // first and skip that key in both entity passes.
        std::optional<ResourceLayer> layer;
        auto layer_it = raw.find(kResourceLayerKey);
        if (layer_it != raw.end() && layer_it->second.resource_layer) {
            layer = layer_it->second.resource_layer;
        }
        const std::map<std::string, double> no_means;
        const std::vector<std::vector<std::string>> no_groups;
        const std::map<std::string, double>& means = layer ? layer->means : no_means;
        const std::vector<std::vector<std::string>>& groups = layer ? layer->groups : no_groups;

        std::map<std::string, dof::LensObservation> observations;
        double min_ttc = std::numeric_limits<double>::infinity();

        // Pass 1: raw lens inputs and the local deadlines. BOTH reserved keys are
        // skipped: the resource layer and the world observation describe the
        // world/agent, not an entity, and a reserved entry left in this loop would
        // drag τ down to its own default of zero.
        for (const auto& kv : raw) {
            if (is_reserved_key(kv.first)) continue;
            observations[kv.first] = kv.second.lenses;
            if (!kv.second.is_collapse_source && kv.second.time_to_collapse_mks < min_ttc) {
                min_ttc = kv.second.time_to_collapse_mks;
            }
        }

        // Global τ is driven by the most urgent non-collapse-source entity (§3.2).
        const double global_ttc = std::isfinite(min_ttc) ? min_ttc : 1e15;

        // §3.4.1/§3.4.2 (v0.11): the measurement durations are **declared ruler
        // content**. They are read from the observation, never inferred (§4.7), and
        // they are the same for every reading of the cycle — a hypothesis
        // reinterprets what was measured, not how long the measuring takes.
        std::map<std::string, std::map<std::string, double>> measurement_durations;
        auto durs_it = raw.find(kMeasurementDurationsKey);
        if (durs_it != raw.end() && durs_it->second.measurement_durations) {
            measurement_durations = *durs_it->second.measurement_durations;
        }
        std::map<std::string, double> measurement_schedule;
        auto sched_it = raw.find(kMeasurementScheduleKey);
        if (sched_it != raw.end() && sched_it->second.measurement_schedule) {
            measurement_schedule = *sched_it->second.measurement_schedule;
        }

        // §3.2b (v0.11): τ is read from the **resource map**, and the individual
        // deadlines declared alongside it govern it. Three cases, in this order:
        //   1. declared individual deadlines — τ is their minimum, and is unknown
        //      when **any** active deadline is unmeasured: taking the minimum over
        //      the measured ones alone would read an unknown timer as absent;
        //   2. otherwise the `tau` observation of the map, which may be negative (a
        //      passed deadline keeps its magnitude) or unmeasured;
        //   3. otherwise the legacy entity-minimum, kept so the historical fixtures
        //      of `v0.6`–`v0.9.1` still read as they did.
        const std::map<std::string, std::optional<double>> no_deadlines;
        const std::optional<std::optional<double>> no_tau_obs;
        const std::map<std::string, std::optional<double>>& declared_deadlines =
            layer ? layer->deadlines : no_deadlines;
        const std::optional<std::optional<double>>& tau_obs = layer ? layer->tau : no_tau_obs;
        const std::optional<double> tau_value =
            resolve_tau(declared_deadlines, tau_obs, global_ttc);
        // §3.2b/§10(s): the deprecated mirror is clamped and is **not** τ. It is
        // `0.0` for an unknown and for a passed deadline, and equals τ otherwise.
        const double mirror_ttc = mirror_time_to_collapse(tau_value);

        // §3.5 (v0.7): the observed world graph, when the cycle was given one.
        std::optional<dof::WorldObservation> world;
        auto world_it = raw.find(kWorldKey);
        if (world_it != raw.end() && world_it->second.world) world = world_it->second.world;
        const dof::WorldGraph empty_graph;
        const dof::WorldGraph& graph = world ? world->graph : empty_graph;
        const std::vector<std::string> no_class;
        const std::map<std::string, double> no_trec;
        const std::vector<std::string>& means_class = world ? world->means_class : no_class;
        const std::map<std::string, double>& t_rec = world ? world->t_rec : no_trec;
        std::optional<double> counting_horizon =
            world ? world->counting_horizon_mks : std::nullopt;
        // §4.9: a response vector must be executable inside the counting horizon, so
        // the default is the cycle's own τ — never an implicit, invisible horizon.
        if (world && !counting_horizon) counting_horizon = global_ttc;

        // §4.6/§4.8 (v0.7): the numeraire weights, the observed rate table and the
        // mandate cap are DERIVED over the observation, not authored. Without a
        // declared numeraire there is no unit for a scalar cap, so neither applies
        // — which keeps a ruler without a world graph reading exactly as in v0.6.
        std::map<std::string, double> weights;
        std::map<std::string, dof::Rate> derived_rates;
        std::optional<double> mandate_cap;
        std::map<std::string, dof::VerdictRecord> verdicts;
        if (world && world->numeraire) {
            std::set<std::string> member_set;
            for (const auto& grp : groups) {
                for (const auto& r : grp) member_set.insert(r);
            }
            std::vector<std::string> members(member_set.begin(), member_set.end());
            weights = graph.weights_to(*world->numeraire, members);
            // §3.5/§4.8: the axis rates are the *output* of the observation
            // procedure, so with a graph in hand the table is derived rather than
            // read from the layer. A declared table beside an observed graph would
            // be a second ruler for the same quantity, free to drift.
            for (const auto& a : members) {
                for (const auto& b : members) {
                    if (a == b) continue;
                    dof::RateResult res = graph.rate(a, b, true);
                    if (res.status == "observed" && res.rate) {
                        derived_rates[a + "->" + b] = dof::Rate{*res.rate, res.duration_mks};
                    }
                }
            }
            if (layer) {
                std::vector<double> limits;
                for (const auto& kv : layer->mandate) {
                    if (kv.first == "cap" && !kv.second.is_string) {
                        limits.push_back(kv.second.number);
                    } else if (kv.first == "external_limit_credit" && !kv.second.is_string) {
                        // Declared in credits, applied in the numeraire: converted
                        // through the *observed* weight, never a hard-coded 1.0.
                        auto wit = weights.find("credit");
                        if (wit != weights.end()) limits.push_back(kv.second.number * wit->second);
                    }
                }
                if (!limits.empty()) {
                    double smallest = limits.front();
                    for (double v : limits) smallest = std::min(smallest, v);
                    mandate_cap = smallest;
                }
            }
            // §4.6/§4.9: the verdicts and counters are computed by the named
            // procedures and then declared, so the declaration can be checked
            // against the observation it came from.
            for (const auto& kv : observations) {
                const std::string& eid = kv.first;
                std::optional<double> horizon;
                auto trit = t_rec.find(eid);
                if (trit != t_rec.end()) horizon = trit->second;
                dof::Verdict v = graph.verdict(eid, means_class, horizon);
                verdicts[eid] = dof::VerdictRecord{v.verdict, horizon,
                                                   graph.v_count(eid, means_class, counting_horizon)};
            }
        }

        // Pass 2: the declaration is frozen on S, so τ is known before measuring.
        dof::MeasurementDeclaration declaration;
        declaration.psi_id = psi_id;
        declaration.u0_prior_q = u0_prior_q;
        declaration.entities = observations;
        // §3.2b (v0.11): τ is signed and may be unmeasured; the declaration carries
        // it as it is, and the canonical form renders an unmeasured τ as `null`.
        declaration.tau_mks = tau_value;
        declaration.measurement_durations = measurement_durations;
        if (layer) {
            declaration.resources = layer->resources;
            declaration.groups = dof::canonical_groups(layer->groups);
            declaration.rates = layer->rates;
            declaration.mandate = layer->mandate;
        }
        if (world) {
            // The derived table replaces a declared one whenever a graph is in hand.
            if (!derived_rates.empty()) declaration.rates = derived_rates;
            declaration.numeraire = world->numeraire;
            declaration.weights = weights;
            declaration.mandate_cap = mandate_cap;
            declaration.verdicts = verdicts;
            declaration.means_class = means_class;
            declaration.graph_procedure = world->procedure;
        }
        const double u0 = declaration.u0();  // at t = 0 the schedule says u₀ (§4.7)

        std::unordered_map<std::string, EntityState> entities;
        for (const auto& kv : raw) {
            if (is_reserved_key(kv.first)) continue;
            dof::EntityMeasurement m = dof::measure_entity(kv.first, kv.second.lenses, u0,
                                                           &means, &groups, &weights, mandate_cap);
            EntityState ent;
            ent.entity_id = kv.first;
            ent.is_autonomous = kv.second.is_autonomous;
            ent.agency_index = std::max(0.0, std::min(1.0, kv.second.agency_index));
            ent.current_dof = m.current_dof;
            ent.is_collapse_source = kv.second.is_collapse_source;
            ent.dof_known = m.dof_known;
            ent.time_to_collapse_mks = kv.second.time_to_collapse_mks;
            ent.measurement = m;
            entities[kv.first] = ent;
        }

        // v0.9.1: convert flat means to ResourceObservation with metadata.
        std::unordered_map<std::string, ResourceObservation> resources;
        for (const auto& [rid, val] : means) {
            ResourceObservation obs;
            obs.value = val;
            // Find the declared unit for this resource.
            for (const auto& ru : layer->resources) {
                if (ru.id == rid) {
                    obs.unit = ru.unit;
                    obs.scale = ru.scale;
                    break;
                }
            }
            obs.source = "sensor";
            obs.aging_time = 3600.0;
            resources[rid] = obs;
        }
        // v0.9.1: τ is stored as ResourceObservation under "tau".
        {
            ResourceObservation tau_obs;
            tau_obs.value = tau_value;
            tau_obs.unit = "us";
            tau_obs.scale = 1.0;
            // §3.2b (v0.11): the provenance says where τ came from. The legacy
            // `entity_min` is only one of the three cases; reporting it for a τ
            // derived from a declared deadline would misname the source.
            tau_obs.source = declared_deadlines.empty() ? "entity_min" : "resource_map";
            tau_obs.aging_time = 0.0;
            resources["tau"] = tau_obs;
        }

        SystemStateMatrix state;
        // §3.2b/§10(s): the deprecated mirror is clamped and is **not** τ; τ itself
        // is carried beside it, signed and possibly unmeasured.
        state.global_time_to_collapse_mks = mirror_ttc;
        state.context_switch_cost = context_switch_cost_;
        state.entities = std::move(entities);
        state.psi = dof::PsiReference{declaration.psi_id, declaration.digest()};
        state.resources = std::move(resources);
        state.tau = ResourceObservation{};
        state.tau->value = tau_value;
        state.tau->unit = "us";
        state.tau->scale = 1.0;
        state.tau->source = declared_deadlines.empty() ? "entity_min" : "resource_map";
        state.deadlines = declared_deadlines;
        state.measurement_durations = measurement_durations;
        state.measurement_schedule = measurement_schedule;
        last_declaration = declaration;

        // §3.5/§4.9: the observation itself, pinned by its own digest (§6.2), and
        // the self-check that the declared derived numbers are the ones the named
        // procedures actually return over it.
        last_observation.reset();
        last_graph_problems.clear();
        if (world) {
            ObservationContext ctx;
            ctx.world = graph;
            ctx.means_class = means_class;
            ctx.t_rec = t_rec;
            ctx.counting_horizon_mks = counting_horizon;
            ctx.observation_digest = graph.observation_digest(means_class, t_rec, counting_horizon);
            last_observation = ctx;
            last_graph_problems = verify_graph_derived(declaration, graph, counting_horizon);
        }
        return state;
    }
};
