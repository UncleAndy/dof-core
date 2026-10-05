// DOF-Core calculus kernel (C++ port).
// Mirrors patterns/calculus_core.py: pure Nash evaluation index (sum of ln(DoF)),
// the `calc` calculation set, Collapse-Source isolation, Delta-T-aware selection,
// and the Proof-of-Implementation audit report (DOF-SPEC §6).
//
// Axioms: Axiom 1 (maximize the total future DoF of the system AND its constituent
// entities); Axiom 3 (never trade one entity's collapse for another's gain);
// Axiom 5 (prefer reversible actions; never assume unknown possibilities have zero
// DoF — a node with dof_known == false is never excluded as a hopeless zero).

#pragma once
#include <string>
#include <set>
#include <utility>
#include <unordered_map>
#include <optional>
#include <vector>
#include <cmath>
#include <limits>
#include <algorithm>
#include "measurement.hpp"
#include "world_graph.hpp"

// §3.2a (v0.9.1): a resource as an observable quantity with metadata.
struct ResourceObservation {
    std::optional<double> value;
    std::string unit;
    double scale = 1.0;
    std::string source;
    double last_measured_at = 0.0;
    double aging_time = 0.0;
    std::optional<double> estimated;
    std::vector<std::string> estimation_source;

    // §3.2a (v0.9.1): check if data has exceeded aging_time.
    bool is_stale(double now) const {
        if (aging_time <= 0.0) return false;
        return (now - last_measured_at) > aging_time;
    }

    // §3.2a (v0.9.1): check if this observation is usable (has a value).
    bool is_usable() const { return value.has_value(); }
};

// §4.8 (v0.9.1): resolve a resource's usable value for the gate.
inline double resource_value(const ResourceObservation& obs, bool use_estimated = false) {
    if (obs.value) return *obs.value;
    if (use_estimated && obs.estimated) return *obs.estimated;
    return 0.0;
}

// §4.8 (v0.9.1): equality for ResourceObservation (used in report comparison).
inline bool resource_obs_equal(const ResourceObservation& a, const ResourceObservation& b) {
    if (a.value.has_value() != b.value.has_value()) return false;
    if (a.value.has_value() && *a.value != *b.value) return false;
    return a.unit == b.unit && a.scale == b.scale && a.source == b.source &&
           a.last_measured_at == b.last_measured_at && a.aging_time == b.aging_time &&
           a.estimated == b.estimated && a.estimation_source == b.estimation_source;
}

// Compare two resource maps (used in reports).
inline bool resource_map_equal(
    const std::map<std::string, ResourceObservation>& a,
    const std::map<std::string, ResourceObservation>& b) {
    if (a.size() != b.size()) return false;
    for (const auto& [key, val] : a) {
        auto it = b.find(key);
        if (it == b.end() || !resource_obs_equal(val, it->second)) return false;
    }
    return true;
}

struct EntityState {
    std::string entity_id;
    bool is_autonomous = true;
    double agency_index = 0.0;   // 0..1
    double current_dof = 0.0;     // 0..1
    bool is_collapse_source = false;
    bool dof_known = true;        // unknown DoF is never treated as 0 (Axiom 5)
    double time_to_collapse_mks = 0.0;
    // Port-level extension (not a §3.1 field): the measurement that produced
    // current_dof, kept so the audit can show the per-lens terms (§6.1).
    std::optional<dof::EntityMeasurement> measurement;
};

struct SystemStateMatrix {
    double global_time_to_collapse_mks = 0.0;
    double context_switch_cost = 0.0;
    std::unordered_map<std::string, EntityState> entities;
    std::optional<dof::PsiReference> psi;  // frozen measurement ruler (§3.4)
    std::unordered_map<std::string, ResourceObservation> resources;
    // §3.2b (v0.11): τ as a signed ResourceObservation. An absent **value** means
    // unmeasured — which is neither a measured zero (a passed deadline keeps its
    // magnitude) nor an absent timer.
    std::optional<ResourceObservation> tau;
    // §3.2b (v0.11): the active individual deadlines τ is derived from. An entry
    // whose value is `nullopt` is an UNMEASURED deadline: τ is then unknown, and
    // taking the minimum over the measured ones alone would read an unknown timer
    // as absent.
    std::map<std::string, std::optional<double>> deadlines;
    // §4.7 (v0.11): the declared measurement durations `t_m`, `t_v` per lens —
    // hashed ruler content of the same observation (§3.4.1).
    std::map<std::string, std::map<std::string, double>> measurement_durations;
    // §4.7 (v0.11): the declared **schedule** `t` per lens — when the measurement
    // is planned to happen. An undeclared schedule reads as `t = 0` (`u₀`).
    std::map<std::string, double> measurement_schedule;
};

// §3.2b (v0.11): τ as the calculus reads it — from the **resource map**, signed.
//
// A `null` value is **unmeasured**, never the minimum over the measured deadlines
// alone and never `0.0`: an unmeasured active deadline may be the most urgent one,
// so acting on the budget the measured ones support is acting on a budget the state
// does not establish, and writing `0.0` invents a catastrophe (§3.1, §3.2b).
//
// A **negative** value is a deadline that has passed, `|τ|` ago. It is a *known*
// state and MUST NOT be clamped to `0.0` or replaced by `null`, which means
// unmeasured only (§3.2b, §4.8b).
//
// The order is the reference's: the map's `tau` observation — as `resolve_tau`
// left it in the state — then the declared deadlines, and only then the deprecated
// mirror, which a port that predates the resource layer still writes. The mirror is
// therefore read **only** when the state carries neither, so it can never override a
// measurement.
inline std::optional<double> tau_of(const SystemStateMatrix& state) {
    if (state.tau) return state.tau->value;
    if (!state.deadlines.empty()) {
        double best = std::numeric_limits<double>::infinity();
        for (const auto& kv : state.deadlines) {
            if (!kv.second) return std::nullopt;  // an active deadline is unmeasured
            if (*kv.second < best) best = *kv.second;
        }
        return best;
    }
    return state.global_time_to_collapse_mks;
}

// §5's `t*` for a measurement of `t_meas_mks`: `nullopt` when τ is unmeasured — an
// unknown budget is not a closed window, and the strict `t* > 0` rule of §5 is not
// applied to it (a τ measurement is governed by §4.8b instead).
inline std::optional<double> measurement_window(const SystemStateMatrix& state,
                                                double t_meas_mks) {
    const std::optional<double> tau = tau_of(state);
    if (!tau) return std::nullopt;
    return *tau - t_meas_mks;
}

// §3.2b/§10(s): the deprecated mirror is clamped and is **not** τ. Equal to τ when
// τ is known and non-negative, and `0.0` when τ is negative or unmeasured.
//
// It exists because the historical harnesses read it, and because a report field
// named "time to collapse" showing a raw negative τ would read as time running
// backwards. It is never an input to a rule (§3.1, §4.7, §4.8b).
inline double mirror_time_to_collapse(const std::optional<double>& tau) {
    if (!tau || *tau < 0.0) return 0.0;
    return *tau;
}

// §3.2b's three-case resolution of τ over an observation, stated once. It mirrors
// the reference port exactly:
//
//  1. declared individual deadlines — their minimum, and **unknown** when any of
//     them is unmeasured. Taking the minimum over the measured ones alone would
//     read an unknown timer as absent, and the unmeasured one may be the most
//     urgent (§10(w), §10(ao));
//  2. otherwise the `tau` observation of the map — signed, so a passed deadline
//     keeps its magnitude, and unmeasured when the map says the value is unknown;
//  3. otherwise the legacy entity-minimum, so the historical fixtures of
//     `v0.6`–`v0.9.1` still read as they did.
inline std::optional<double> resolve_tau(
    const std::map<std::string, std::optional<double>>& deadlines,
    const std::optional<std::optional<double>>& tau_obs, double legacy) {
    if (!deadlines.empty()) {
        double best = std::numeric_limits<double>::infinity();
        for (const auto& kv : deadlines) {
            if (!kv.second) return std::nullopt;  // an active deadline is unmeasured
            if (*kv.second < best) best = *kv.second;
        }
        return best;
    }
    if (tau_obs) return *tau_obs;
    return legacy;
}

// §3.6: the id the observed state carries when it is read as a member of `H`.
// Declared here — next to the option — because the per-hypothesis accessors below
// need it, and `hypothesis.hpp` (the artifact's home) includes this header.
inline const std::string kObservedHypothesisId = "$observed$";

struct ActionOption {
    std::string option_id;
    std::string description;
    // §3.3 (v0.11): **two forms, never mixed within one option.**
    //   flat           — {entity_id: delta}, applied under every hypothesis;
    //   per_hypothesis — {hypothesis_id: {entity_id: delta}}, the entry for `h`
    //                    used under `h`, entities unlisted for `h` taking 0.0.
    //
    // An option in the flat form asserts that its effect does not depend on the
    // causal reading; the per-hypothesis form is meaningful only when a hypothesis
    // set is declared. The two forms live in two fields rather than one
    // union-typed field; `projection_form()` is the single place that decides which
    // form an option is in, and a non-empty pair is `invalid` (§3.3, §10(B)).
    std::unordered_map<std::string, double> projected_dof_delta;
    std::unordered_map<std::string, std::unordered_map<std::string, double>>
        projected_by_hypothesis;
    bool is_reversible = true;
    double estimated_duration_mks = 0.0;  // execution time, microseconds (DOF-SPEC §3.3)
    // §3.3 (v0.6): what the option draws from the acting agent, attributed to the
    // entity whose transitions consume it. Negative = consumption, positive =
    // production. `energy` MUST be present (as 0.0) for every entity named in
    // projected_dof_delta.
    std::unordered_map<std::string, std::unordered_map<std::string, double>> projected_resource_delta;
    // §3.3/§4.4 (v0.7): the transitions this option CLOSES — the acts and means
    // that cease to exist once it executes. `is_reversible` is DERIVED from this
    // list (true exactly when it is empty) and is kept only as a reported field:
    // a label that could be set to dodge the price is not a rule.
    //
    // §4.4 (v0.11): the closure list has the same two forms as the projection.
    std::vector<dof::ClosedRef> closed;
    std::unordered_map<std::string, std::vector<dof::ClosedRef>> closed_by_hypothesis;
    std::string act_id;  // the graph act implementing this option
    // §3.3 (v0.11): the projected τ change. `nullopt` means **not computable** —
    // which happens exactly when τ is unknown — and is admissible only for an act
    // that resolves τ. Derived, never independently declared, for such an act: it
    // MUST equal `projected_tau_value - (τ - estimated_duration_mks)`.
    std::optional<double> projected_tau_delta;
    // §3.3 (v0.11): the value the option expects `tau` to hold AFTER it executes.
    // Present iff `discovers` names "tau". MAY be negative (§3.2b).
    std::optional<double> projected_tau_value;
    // §3.3 (v0.9.1): resources whose value becomes known after this option executes.
    std::vector<std::string> discovers;
    // §3.3 (v0.9.1): resources needed for gate checks.
    std::vector<std::string> requires;
};

// `"flat"` | `"per_hypothesis"` | `"invalid"` (§3.3, §10(B)).
inline std::string projection_form(const ActionOption& option) {
    const bool flat = !option.projected_dof_delta.empty();
    const bool nested = !option.projected_by_hypothesis.empty();
    if (flat && !nested) return "flat";
    if (nested && !flat) return "per_hypothesis";
    // An option declaring no delta at all is the flat form: the empty map is a flat
    // map, and reading it as "invalid" would refuse every baseline.
    if (!flat && !nested) return "flat";
    // Both filled: **invalid**, never a merge. A merge would silently pick one of
    // two contradictory statements about the same option — and the choice would
    // decide the index.
    return "invalid";
}

// `"flat"` | `"per_hypothesis"` | `"invalid"` (§4.4, §10(B)).
inline std::string closure_form(const ActionOption& option) {
    const bool flat = !option.closed.empty();
    const bool nested = !option.closed_by_hypothesis.empty();
    if (flat && !nested) return "flat";
    if (nested && !flat) return "per_hypothesis";
    if (!flat && !nested) return "flat";
    return "invalid";
}

// The declared delta this option contributes under `hypothesis_id`.
inline double delta_for(const ActionOption& option, const std::string& hypothesis_id,
                        const std::string& entity_id) {
    if (projection_form(option) == "per_hypothesis") {
        auto it = option.projected_by_hypothesis.find(hypothesis_id);
        if (it == option.projected_by_hypothesis.end()) return 0.0;
        auto eit = it->second.find(entity_id);
        return eit == it->second.end() ? 0.0 : eit->second;
    }
    auto it = option.projected_dof_delta.find(entity_id);
    return it == option.projected_dof_delta.end() ? 0.0 : it->second;
}

// The closures this option declares under `hypothesis_id`.
inline std::vector<dof::ClosedRef> closed_for(const ActionOption& option,
                                              const std::string& hypothesis_id) {
    if (closure_form(option) == "per_hypothesis") {
        auto it = option.closed_by_hypothesis.find(hypothesis_id);
        if (it == option.closed_by_hypothesis.end()) return {};
        return it->second;
    }
    return option.closed;
}

// §4.5 key 3, under one named reading.
inline bool is_reversible_for(const ActionOption& option, const std::string& hypothesis_id) {
    return closed_for(option, hypothesis_id).empty();
}

// The flat delta map, whichever form the option uses.
//
// Used where a single map is needed for `nullopt`-safety (the baseline); a
// per-hypothesis option returns the **union** of its entries, which is only
// meaningful for existence questions.
inline std::unordered_map<std::string, double> flat_delta(const ActionOption& option) {
    std::unordered_map<std::string, double> out;
    if (projection_form(option) == "per_hypothesis") {
        for (const auto& per_h : option.projected_by_hypothesis) {
            for (const auto& kv : per_h.second) {
                auto it = out.find(kv.first);
                if (it == out.end() || kv.second > it->second) out[kv.first] = kv.second;
            }
        }
        return out;
    }
    return option.projected_dof_delta;
}

// §3.3/§4.4: the two forms MUST NOT be mixed within one option. An empty result
// means the option is well formed. A non-empty one is non-conformant **input**:
// the option is refused, never repaired.
inline std::string forms_consistent(const ActionOption& option) {
    if (projection_form(option) == "invalid") {
        return option.option_id +
               ": `projected_dof_delta` mixes the flat and per-hypothesis forms (§3.3)";
    }
    if (closure_form(option) == "invalid") {
        return option.option_id + ": `closed` mixes the flat and per-hypothesis forms (§4.4)";
    }
    return "";
}

// §3.3/§4.8b: does this option resolve `resource`? The question is asked by §4.8b
// (a τ measurement is governed by a different rule than a lens measurement), by
// the τ-gate of §5 (with an unknown τ only a τ measurement survives) and by the
// viability gate — so it is stated once, where the option is defined. A second
// copy is how the gates come to disagree about what counts as a measurement.
inline bool option_discovers(const ActionOption& option, const std::string& resource) {
    return std::find(option.discovers.begin(), option.discovers.end(), resource) !=
           option.discovers.end();
}

// The observation a cycle is decided over (§3.5, §4.9).
//
// Deliberately NOT a state field: the world graph is a Perception artifact
// supplied to the cycle, exactly as the derived groups and the observed rates
// are (§4.8). Without it every verdict is `undetermined`, which means no entity
// at a known zero is excluded and no collapse-source label is honoured — the
// fail-safe direction: nothing is proven, so nothing is removed.
struct ObservationContext {
    dof::WorldGraph world;
    std::vector<std::string> means_class;
    std::map<std::string, double> t_rec;
    std::optional<double> counting_horizon_mks;
    std::string observation_digest;
    // §3.6/§4.9/§4.10 (v0.11): `DoF(X | h)` — the per-entity degrees of freedom
    // **under one reading**, which the §4.9 verdict consumes. `nullopt` means "read
    // the graph's own value", which is what the observed reading does; a hypothesis
    // reading supplies its own numbers here. Structural inputs — `G`, the paths and
    // their admissibility, `M(S)`, `T_rec(X)` — stay shared: only the DoF is
    // conditional (§3.6, §7 п.25).
    std::optional<std::unordered_map<std::string, double>> dof_override;

    // The reading's `DoF(X | h)` for one entity, or `nullopt` when this context
    // carries no reading.
    std::optional<double> dof_override_of(const std::string& entity_id) const {
        if (!dof_override) return std::nullopt;
        auto it = dof_override->find(entity_id);
        if (it == dof_override->end()) return std::nullopt;
        return it->second;
    }

    // §3.6/§4.9: the same observation, read under one hypothesis's DoF.
    //
    // Every shared input is carried over untouched — the graph, `M(S)`, the
    // horizons, the observation digest — because §3.6 shares them by construction:
    // only `DoF(X | h)` differs between readings, and only it is replaced here.
    ObservationContext with_dof(std::unordered_map<std::string, double> dofs) const {
        ObservationContext clone = *this;
        clone.dof_override = std::move(dofs);
        return clone;
    }

    std::optional<double> horizon(const std::string& entity_id) const {
        auto it = t_rec.find(entity_id);
        if (it == t_rec.end()) return std::nullopt;
        return it->second;
    }
    // §4.9: the verdict **under this reading**. The observed reading carries no
    // override and is therefore byte-identical to the `v0.9.1` call — the
    // conditional form is the general one and the observed form is its instance,
    // not a second rule.
    std::string verdict(const std::string& entity_id) const {
        return world
            .verdict_with_dof(entity_id, means_class, horizon(entity_id),
                              dof_override_of(entity_id))
            .verdict;
    }
    int v_before(const std::string& entity_id) const {
        return world.v_count(entity_id, means_class, counting_horizon_mks);
    }
    int v_after_closure(const std::string& entity_id,
                        const std::vector<dof::ClosedRef>& closed) const {
        if (closed.empty()) return v_before(entity_id);
        return world.with_closed(closed).v_count(entity_id, means_class, counting_horizon_mks);
    }
};

// Audit report rows and container (DOF-SPEC §6)
// §6.1 (v0.7): the recoverability verdict, its witness, and the completeness
// claim behind it. A `proven_unreachable` verdict without a witness is not a
// verdict, so the report carries both — and names the observation, because "no
// path" is only meaningful together with "and the observation was complete for
// this entity".
struct RecoverabilityRow {
    std::string verdict;
    std::vector<std::string> witness;
    std::optional<double> horizon_mks;
    std::string observation = "unobserved";
    int admissible_seen = 0;
    std::string reason;
};

struct EntityReportRow {
    std::string entity_id;
    bool is_collapse_source = false;
    bool included_in_sum = false;
    double current_dof = 0.0;
    bool dof_known = true;
    double contribution = 0.0;
    // §6.1: why, not only what.
    std::vector<dof::LensTerm> lens_terms;
    std::optional<std::string> binding_lens;
    bool floored = false;
    // §4.6 (v0.6): the derived blocks and the derivation behind them, so a
    // reader can recompute (c_g, C_g) from the raw requirements.
    std::vector<std::pair<double, double>> blocks;
    std::optional<dof::DerivationInfo> derivation;
    // §6.1 (v0.7): the recoverability verdict and its witness.
    RecoverabilityRow recoverability;
};

// One entity a candidate drove from a counted state to a known zero (§4.2, §6.3):
// the audit line that makes the price of destruction explicit.
struct CollapseCharge {
    std::string entity_id;
    double dof_before = 0.0;
};

// The v0.8 candidate vector (§4.5): three counts of entities — the protected
// dimensions — plus the index and the reversibility preference.
struct CandidateVector {
    int d1 = 0;
    int d2 = 0;
    int d3 = 0;
    double net_delta = 0.0;
    bool reversible = true;
    std::string option_id;
    // §6.3: `viable` and `resources_ok` are **conditions of admissibility**, not
    // annotations — a `false` in either bars the option exactly as a positive `d`
    // does — so they are part of `candidate_vector` and not of the row's context.
    // Both default to true, and the layer that evaluates the condition sets them:
    // the financial one here (`plan_funding` is this header's own), the temporal one
    // in the layer that reads τ.
    bool viable = true;
    bool resources_ok = true;
};

// One entity this option drops out of a `reachable` verdict, with the witness it
// lost (§4.5, §6.3). A path loss counts even where no exclusion follows from it.
struct LostPathEntry {
    std::string entity_id;
    std::string verdict_before;
    std::string verdict_after;
    bool critical = false;
    std::vector<std::string> witness_lost;
};

// A deficit covered by an exchange (§4.8): the audit line that shows the price
// was paid by trade, at an observed rate, and how long the trade itself took.
struct Conversion {
    std::string from;
    std::string to;
    double amount_from = 0.0;
    double amount_to = 0.0;
    double rate = 0.0;
    double duration_mks = 0.0;
};

struct OptionReportRow {
    std::string option_id;
    bool is_reversible = true;
    double projected_dof = 0.0;
    double net_delta = 0.0;
    bool selected = false;
    double estimated_duration_mks = 0.0;
    // §6.3: every collapse this option causes, as an auditable line of the ledger.
    std::vector<CollapseCharge> collapse_charges;
    // §6.3 (v0.6): what the option draws, and how "affordable" was established —
    // by cash in hand or by an observed trade — plus whatever stayed uncovered.
    std::unordered_map<std::string, std::unordered_map<std::string, double>> resource_consumption;
    std::vector<Conversion> conversion_applied;
    std::map<std::string, double> resources_uncovered;
    // §4.8/§6.3 (v0.7): how much of the mandate the spend would have used, and
    // what the closure decomposes into.
    double mandate_exceeded = 0.0;
    std::vector<dof::ClosedRef> closed;
    std::map<std::string, double> closure_share;
    // §6.3 (v0.8): the protected dimensions, the dimension that barred the
    // candidate (empty when nothing did), and the path losses line by line.
    CandidateVector candidate_vector;
    std::optional<std::string> barring_key;
    std::vector<LostPathEntry> lost_paths;
};

// The result of §4.8's funding decision: what the option needs, what actually
// leaves the agent's stock (the spend ledger — a deficit bought from another
// resource spends *that* resource), the trades that were performed, and the
// deficit that survived full verified conversion.
struct FundingPlan {
    bool covered = true;
    std::map<std::string, double> need;
    std::map<std::string, double> spend;
    std::vector<Conversion> conversions;
    std::map<std::string, double> uncovered;
    // §4.8 (v0.7): the part of the spend above the mandate ceiling, in the group
    // numeraire. It can only remove an option a larger balance would have paid for.
    double mandate_exceeded = 0.0;
    double total_duration_mks = 0.0;
};

// A candidate removed before evaluation (§6.2).
struct RemovedOption {
    std::string option_id;
    std::string gate;
};

struct DofReport {
    std::vector<EntityReportRow> entities;
    double total_system_dof = 0.0;
    double context_switch_cost = 0.0;
    double global_time_to_collapse_mks = 0.0;
    std::string mode;
    std::vector<OptionReportRow> options;
    std::string psi_id;
    std::string psi_digest;
    std::string declaration;
    std::vector<RemovedOption> removed_options;
    // §6.2: a resolvable unknown was left unmeasured in every candidate, so the
    // decision is declared incomplete rather than presented as informed.
    bool incomplete = false;
    // §6.2 (v0.6): the acting agent's means at the start of the cycle and after
    // the selected option's consumption. Multi-step accumulation is auditable
    // only if the spend is written where the next cycle can see it (§4.8).
    std::map<std::string, ResourceObservation> resources_before;
    std::map<std::string, ResourceObservation> resources_after;
    // §6.2 (v0.7): the identity of the observation a reported subgraph was taken
    // from, and where the amounts a decision rests on came from — a measured
    // balance or an asserted authority.
    std::optional<std::string> observation_digest;
    std::map<std::string, dof::MandateValue> means_provenance;
    // §6.2 (v0.8): the vector every candidate was compared against, and whether
    // any candidate beat it. A refusal to act is a decision and must be audible.
    CandidateVector baseline;
    bool no_candidate_better = false;
};

// What a report needs beyond the state, the candidates and the selection. It keeps
// the report call site readable now that the report is where the release's reasons
// are written down (§6).
struct ReportInput {
    std::optional<dof::MeasurementDeclaration> declaration;
    std::vector<RemovedOption> removed;
    const std::vector<std::vector<std::string>>* groups = nullptr;
    const std::map<std::string, dof::Rate>* rates = nullptr;
    const std::map<std::string, double>* weights = nullptr;
    std::optional<double> cap;
    const ObservationContext* ctx = nullptr;
    std::map<std::string, dof::MandateValue> means_provenance;
};

class DOFCalculusCore {
    double epsilon_;
public:
    DOFCalculusCore(double epsilon = 1e-6) : epsilon_(epsilon) {}

    // v0.8 (§10): the tolerance that decides whether two candidates' NetDelta are
    // tied. The index is a sum of logarithms over a SET, so two ports that iterate
    // their container in different orders can disagree in the last bits (~1e-15)
    // while agreeing on every derivation — and a tie must be resolved identically
    // everywhere, because §7 requires the same CHOICE, not only the same numbers.
    static constexpr double net_delta_tolerance = 1e-9;

    // Whether an entity belongs to the calculation set `calc` (DOF-SPEC §4.2).
    // Excluded if it is a **witnessed** collapse source, or if its DoF is a known
    // zero whose recoverability verdict is `proven_unreachable` (§4.2/§4.9). A
    // node with unknown DoF is never excluded (Axiom 5), and neither is a node
    // whose verdict is `reachable` or `undetermined` — incompleteness of an
    // observation is never read as proof.
    //
    // The witness of exclusion MUST NOT be the Generator's candidate set (§4.2),
    // and a verdict is computed from the observation, never asserted.
    bool is_included(const EntityState& e, const ObservationContext* ctx = nullptr,
                     const SystemStateMatrix* state = nullptr) const {
        if (e.is_collapse_source && label_witnessed(e, ctx, state)) return false;
        return is_included_without_label(e, ctx);
    }

    // `calc` membership with the collapse-source label NOT honoured (§4.2). Used
    // in two places, and it must be the same rule in both: deciding who is
    // counted, and deciding whether a label has a witness. The witness question is
    // "would this entity be counted if its own label were ignored" — asking it
    // with the label already applied would be circular, and would make every
    // label unfalsifiable.
    bool is_included_without_label(const EntityState& e, const ObservationContext* ctx = nullptr) const {
        if (e.current_dof > 0.0) return true;
        if (!e.dof_known) return true;
        if (ctx == nullptr) return true;  // fail-safe: no observation, no proof
        return ctx->verdict(e.entity_id) != "proven_unreachable";
    }

    // A label is honoured only with a machine-verifiable act (§4.2): an act
    // performed by this entity that drives an entity which would otherwise be
    // counted to a known zero.
    bool label_witnessed(const EntityState& e, const ObservationContext* ctx,
                         const SystemStateMatrix* state) const {
        if (ctx == nullptr || state == nullptr) return false;
        std::set<std::string> counted;
        std::map<std::string, double> dof_before;
        for (const auto& kv : state->entities) {
            if (is_included_without_label(kv.second, ctx)) counted.insert(kv.first);
            dof_before[kv.first] = kv.second.current_dof;
        }
        if (counted.count(e.entity_id) == 0) return false;
        std::set<std::string> acts;
        for (const auto& id : ctx->world.collapse_acts(counted, dof_before)) acts.insert(id);
        for (const auto& a : ctx->world.acts) {
            if (a.source == e.entity_id && acts.count(a.id) > 0) return true;
        }
        return false;
    }

    // calc(S), frozen for the whole cycle (§4.2): computed once, on S, and the same
    // entities are summed in S and in S', so a term cannot appear or disappear
    // between the two sides of NetDelta.
    std::set<std::string> calc_members(const SystemStateMatrix& state,
                                       const ObservationContext* ctx = nullptr) const {
        std::set<std::string> members;
        for (const auto& kv : state.entities) {
            if (is_included(kv.second, ctx, &state)) members.insert(kv.first);
        }
        return members;
    }

    // Evaluation index: pure Nash product (sum of ln(DoF)) over the frozen calc set.
    // Values are negative; only their ordering matters. See DOF-SPEC §4.1.
    // Pass nullptr for `members` to use calc(state) itself.
    double calculate_system_dof(const SystemStateMatrix& state,
                                const std::set<std::string>* members = nullptr,
                                const ObservationContext* ctx = nullptr) const {
        std::set<std::string> owned;
        if (members == nullptr) {
            owned = calc_members(state, ctx);
            members = &owned;
        }
        double total = 0.0;
        for (const auto& eid : *members) {
            auto it = state.entities.find(eid);
            if (it == state.entities.end()) continue;
            total += std::log(std::max(it->second.current_dof, epsilon_));
        }
        return total;
    }

    static double coerce_dof(double v) { return std::max(0.0, std::min(1.0, v)); }

    // §4.3/§4.4: DoF recomputed from the counters after the option's closure. Only
    // the Variety share moves, so the whole product moves by its ratio. A nullopt
    // means the entity is not affected or its Variety lens was unmeasured.
    //
    // §4.4 (v0.11): the closures read are the ones the option declares **under this
    // reading** — the closure list has the same two forms as the projection.
    std::optional<double> dof_after_closure_for(const EntityState& e, const ActionOption& option,
                                                const ObservationContext* ctx,
                                                const std::string& hypothesis_id) const {
        if (!e.measurement || !e.measurement->variety_counters) return std::nullopt;
        auto var_it = e.measurement->psi_by_lens.find("variety");
        if (var_it == e.measurement->psi_by_lens.end() || !var_it->second) return std::nullopt;
        const double v_env = e.measurement->variety_counters->count("V_env")
                                 ? e.measurement->variety_counters->at("V_env")
                                 : 0.0;
        const double var_before = *var_it->second;
        const int v_before = ctx->v_before(e.entity_id);
        const int v_after =
            ctx->v_after_closure(e.entity_id, closed_for(option, hypothesis_id));
        if (v_after == v_before) return std::nullopt;  // this entity is not affected
        return coerce_dof(e.current_dof / var_before * dof::psi_var(static_cast<double>(v_after), v_env));
    }

    // The observed-reading entry point. Not a second rule: the observed reading is
    // `H = {$observed$}`, so the conditional form above is the general one and this
    // is its instance.
    std::optional<double> dof_after_closure(const EntityState& e, const ActionOption& option,
                                            const ObservationContext* ctx) const {
        return dof_after_closure_for(e, option, ctx, kObservedHypothesisId);
    }

    // The DoF this option would leave the entity with, closure included (§4.3).
    // ONE definition, used by both `simulate` and `collapse_charges`: if the charge
    // were computed from the raw delta while the index was computed from the
    // closure-aware value, an option that destroys an entity BY CLOSING ITS
    // TRANSITIONS would be scored as a collapse and charged as nothing — the
    // structural gate of §4.5 would then pass exactly the option it exists to stop.
    double projected_dof_for(const EntityState& e, const ActionOption& option,
                             const ObservationContext* ctx,
                             const std::string& hypothesis_id) const {
        const double add = delta_for(option, hypothesis_id, e.entity_id);
        double nd = coerce_dof(e.current_dof + add);
        if (ctx != nullptr && !closed_for(option, hypothesis_id).empty()) {
            std::optional<double> recomputed = dof_after_closure_for(e, option, ctx, hypothesis_id);
            if (recomputed) nd = *recomputed;
        }
        return nd;
    }

    double projected_dof(const EntityState& e, const ActionOption& option,
                         const ObservationContext* ctx) const {
        return projected_dof_for(e, option, ctx, kObservedHypothesisId);
    }

    // §4.4 guards: closing one's own execution path, or a false label with nothing
    // closed. Returns an empty string when the option is conformant.
    //
    // §4.4 (v0.11): the guards look at **every** closure list the option carries —
    // the flat one and each per-hypothesis one. An option declares a closure once
    // per reading, but an act of the shared graph closes the same path under every
    // reading, so a declaration that passes under `h₁` and closes its own path
    // under `h₂` is a false declaration, not a conditional one.
    std::string closure_error(const ActionOption& option) const {
        std::vector<std::vector<dof::ClosedRef>> lists;
        lists.push_back(option.closed);
        for (const auto& kv : option.closed_by_hypothesis) lists.push_back(kv.second);
        bool any_closed = false;
        for (const auto& list : lists) {
            if (list.empty()) continue;
            any_closed = true;
            if (!option.act_id.empty()) {
                for (const auto& ref : list) {
                    if (ref.kind == "act" && ref.id == option.act_id) {
                        return option.option_id + ": closes its own execution path (§4.4 guard 1)";
                    }
                }
            }
        }
        // The false-label guard is a statement about the option as a whole: an
        // option that closes nothing under **any** reading while claiming
        // `is_reversible = false` is mislabelled. It is asked of the flat form
        // alone, because the per-hypothesis form's empty entries are ordinary.
        if (!any_closed && closure_form(option) == "flat" && !option.is_reversible) {
            return option.option_id + ": is_reversible=false with an empty closure list (§4.4 guard 2)";
        }
        return "";
    }

    // §4.4: the reported flag is DERIVED — true exactly when nothing is closed
    // under the reading being evaluated.
    static bool is_reversible(const ActionOption& option) {
        return is_reversible_for(option, kObservedHypothesisId);
    }

    // §6.3: the per-entity decomposition of a closure's price. A DECOMPOSITION of
    // the loss already inside `NetDelta` (§4.3/§4.4), never an extra charge.
    std::map<std::string, double> closure_share(const SystemStateMatrix& state,
                                                const ActionOption& option,
                                                const ObservationContext* ctx) const {
        std::map<std::string, double> out;
        if (ctx == nullptr || option.closed.empty()) return out;
        for (const auto& kv : state.entities) {
            const EntityState& e = kv.second;
            if (!e.measurement || !e.measurement->variety_counters) continue;
            auto var_it = e.measurement->psi_by_lens.find("variety");
            if (var_it == e.measurement->psi_by_lens.end() || !var_it->second) continue;
            const double v_env = e.measurement->variety_counters->count("V_env")
                                     ? e.measurement->variety_counters->at("V_env")
                                     : 0.0;
            const int v_after = ctx->v_after_closure(e.entity_id, option.closed);
            const int v_before = ctx->v_before(e.entity_id);
            if (v_after == v_before) continue;
            const double after = std::max(dof::psi_var(static_cast<double>(v_after), v_env), epsilon_);
            const double before = std::max(dof::psi_var(static_cast<double>(v_before), v_env), epsilon_);
            out[e.entity_id] = dof::q6(std::log(after) - std::log(before));
        }
        return out;
    }

    // §6.1: the verdict, its witness and the completeness claim behind it.
    RecoverabilityRow recoverability_row(const std::string& entity_id,
                                         const ObservationContext* ctx) const {
        RecoverabilityRow row;
        if (ctx == nullptr) {
            row.verdict = "undetermined";
            row.observation = "unobserved";
            row.reason = "no observation was supplied for this cycle";
            return row;
        }
        dof::Verdict v = ctx->world.verdict(entity_id, ctx->means_class, ctx->horizon(entity_id));
        row.verdict = v.verdict;
        row.witness = v.witness;
        row.horizon_mks = ctx->horizon(entity_id);
        auto it = ctx->world.entities.find(entity_id);
        row.observation = (it == ctx->world.entities.end()) ? "unobserved" : it->second.observation;
        row.admissible_seen = v.admissible_seen;
        row.reason = v.reason;
        return row;
    }

    // Simulate an option's projected deltas into a new state and return it with the
    // **frozen** member set of calc(S) (§4.2).
    //
    // §3.6/§4.10 (v0.11): the projected DoF is the one the option declares **under
    // this reading** — the same graph read under `h`, not a second graph.
    std::pair<SystemStateMatrix, std::set<std::string>> simulate_for(
        const SystemStateMatrix& current, const ActionOption& option,
        const ObservationContext* ctx, const std::string& hypothesis_id) const {
        std::set<std::string> members = calc_members(current, ctx);
        SystemStateMatrix sim = current;
        for (auto& kv : sim.entities) {
            kv.second.current_dof = projected_dof_for(kv.second, option, ctx, hypothesis_id);
        }
        return {sim, members};
    }

    // The observed-reading entry point.
    std::pair<SystemStateMatrix, std::set<std::string>> simulate(
        const SystemStateMatrix& current, const ActionOption& option,
        const ObservationContext* ctx = nullptr) const {
        return simulate_for(current, option, ctx, kObservedHypothesisId);
    }

    // §4.2: the counted entities a candidate drives to a known zero. The charge
    // depends on neither the Generator's candidate set nor the victim's prospects.
    std::vector<CollapseCharge> collapse_charges_for(const SystemStateMatrix& current,
                                                     const ActionOption& option,
                                                     const ObservationContext* ctx,
                                                     const std::string& hypothesis_id) const {
        std::vector<CollapseCharge> charges;
        for (const auto& eid : calc_members(current, ctx)) {
            auto it = current.entities.find(eid);
            if (it == current.entities.end()) continue;
            const EntityState& e = it->second;
            if (!e.dof_known) continue;  // unknown DoF is never a collapse (§4.2)
            // The projected value is the closure-aware one (§4.3): an option can
            // destroy a counted entity by closing its transitions while declaring
            // no delta at all, and that is exactly the case §4.5 must catch.
            const double nd = projected_dof_for(e, option, ctx, hypothesis_id);
            // §4.2: a charge requires a *transition* into the zero, not a stay at
            // it — charging an entity that was already at zero would make every
            // option destructive in any state containing a recoverable zero.
            if (nd == 0.0 && e.current_dof > 0.0) charges.push_back(CollapseCharge{eid, e.current_dof});
        }
        return charges;
    }

    // The observed-reading entry point.
    std::vector<CollapseCharge> collapse_charges(const SystemStateMatrix& current,
                                                 const ActionOption& option,
                                                 const ObservationContext* ctx = nullptr) const {
        return collapse_charges_for(current, option, ctx, kObservedHypothesisId);
    }

    // RETIRED in v0.8: the live path no longer calls this. A charged candidate is
    // evaluated, reported in full, and made inadmissible by the candidate-vector
    // test of §4.5 (`select_candidate`), so `removed_options` carries no structural
    // removal. Kept because the v0.6 harness asserts the rule that was in force
    // then, and history must stay reproducible.
    //
    // §4.5 (v0.7 rule): removes options that destroy a counted entity while a
    // charge-free candidate exists (Axiom 3). Every removal is recorded as
    // gate = "collapse".
    std::pair<std::vector<ActionOption>, std::vector<RemovedOption>> apply_structural_gate(
        const SystemStateMatrix& current, const std::vector<ActionOption>& options,
        const ObservationContext* ctx = nullptr) const {
        if (options.empty()) return {{}, {}};
        bool charge_free_exists = false;
        for (const auto& opt : options) {
            if (collapse_charges(current, opt, ctx).empty()) {
                charge_free_exists = true;
                break;
            }
        }
        if (!charge_free_exists) {
            // No alternative exists: the ladder decides among the destructive candidates.
            return {options, {}};
        }
        std::vector<ActionOption> admissible;
        std::vector<RemovedOption> removed;
        for (const auto& opt : options) {
            if (collapse_charges(current, opt, ctx).empty()) {
                admissible.push_back(opt);
            } else {
                removed.push_back(RemovedOption{opt.option_id, "collapse"});
            }
        }
        return {admissible, removed};
    }

    // --- §4.8 resource gate ---------------------------------------------------

    static double means_of(const SystemStateMatrix& state, const std::string& resource) {
        auto it = state.resources.find(resource);
        return it == state.resources.end() ? 0.0 : resource_value(it->second);
    }

    // §4.8: the option's net draw on the agent, per resource. Consumption is the
    // negative component of the declared delta summed over the entities the
    // option names; a resource produced more than consumed yields no requirement.
    std::map<std::string, double> requirement(const ActionOption& option) const {
        std::map<std::string, double> net;
        for (const auto& per_entity : option.projected_resource_delta) {
            for (const auto& rv : per_entity.second) net[rv.first] += rv.second;
        }
        std::map<std::string, double> need;
        for (const auto& kv : net) {
            if (kv.second < 0.0) need[kv.first] = -kv.second;
        }
        return need;
    }

    // §4.8: exchange is possible only inside a derived group.
    static bool same_group(const std::string& a, const std::string& b,
                           const std::vector<std::vector<std::string>>* groups) {
        if (a == b) return true;
        if (groups == nullptr) return false;
        for (const auto& group : *groups) {
            bool has_a = false;
            bool has_b = false;
            for (const auto& r : group) {
                if (r == a) has_a = true;
                if (r == b) has_b = true;
            }
            if (has_a && has_b) return true;
        }
        return false;
    }

    // §4.8: decide *how* an option is paid for, and whether it can be. Step 1 is a
    // direct comparison against the agent's means. Step 2 is **verified**
    // conversion: the exchange path must exist (declared rate), the resources must
    // share a group, an offer must satisfy the requirement (deficit / rate), the
    // price must be payable from the agent's means, and the exchange's **own
    // time** must still fit in τ. Anything that fails is not a cheaper conversion
    // — it is a deficit that stays uncovered, and step 3 turns that into
    // insolvency.
    FundingPlan plan_funding(const SystemStateMatrix& state, const ActionOption& option,
                             const std::vector<std::vector<std::string>>* groups = nullptr,
                             const std::map<std::string, dof::Rate>* rates = nullptr,
                             const std::map<std::string, double>* weights = nullptr,
                             const std::optional<double>& cap = std::nullopt) const {
        FundingPlan plan;
        plan.need = requirement(option);
        plan.total_duration_mks = option.estimated_duration_mks;
        // §3.2b/§4.8 (v0.11): τ is read from the resource map as a **signed** value
        // — a negative τ is a passed deadline, not a zero — and `null` means
        // unmeasured. The deprecated `global_time_to_collapse_mks` mirror is never
        // an input to a rule (§3.1, §4.7, §4.8b): an unknown budget licenses no
        // spending beyond the measured balance, so no exchange is used to cover a
        // deficit.
        const std::optional<double> tau = tau_of(state);
        // The numeraire weights: used to choose an offer canonically and to express
        // the mandate ceiling in one unit.
        auto weight_of = [weights](const std::string& r) {
            if (weights == nullptr) return 1.0;
            auto it = weights->find(r);
            return it == weights->end() ? 1.0 : it->second;
        };

        for (const auto& need_entry : plan.need) {
            const std::string& resource = need_entry.first;
            double remaining = need_entry.second;
            double available = std::max(0.0, means_of(state, resource) - plan.spend[resource]);
            double direct = std::min(remaining, available);
            plan.spend[resource] += direct;
            remaining -= direct;

            if (rates != nullptr) {
                // §4.8 (v0.7): the offer is chosen CANONICALLY — the cheapest in
                // the group numeraire first, then the shorter exchange, then the
                // key. Choosing by declaration order (or by resource name) would
                // let a rename change what the report says happened, and two ports
                // would describe the same world differently.
                struct Offer {
                    double cost;
                    double duration;
                    std::string key;
                    std::string source;
                    double amount_source;
                    double rate;
                };
                std::vector<Offer> offers;
                for (const auto& rate_entry : *rates) {
                    const std::string& key = rate_entry.first;
                    std::size_t arrow = key.find("->");
                    if (arrow == std::string::npos) continue;
                    const std::string source = key.substr(0, arrow);
                    const std::string target = key.substr(arrow + 2);
                    if (target != resource) continue;
                    const double rate = rate_entry.second.rate;
                    const double duration = rate_entry.second.duration_mks;
                    if (rate <= 0.0 || !same_group(source, resource, groups)) continue;
                    const double amount_source = remaining / rate;
                    if (amount_source > std::max(0.0, means_of(state, source) - plan.spend[source])) {
                        continue;  // the price is not payable
                    }
                    // §3.2b/§4.8 (v0.11): the exchange's **own time** must fit in τ,
                    // and with an unknown τ there is no budget to spend — so no
                    // exchange is used to cover a deficit.
                    if (!tau || plan.total_duration_mks + duration > *tau) {
                        continue;  // does not fit in τ (or τ is unknown)
                    }
                    offers.push_back(Offer{weight_of(source) * amount_source, duration, key,
                                           source, amount_source, rate});
                }
                if (remaining > 0.0 && !offers.empty()) {
                    std::sort(offers.begin(), offers.end(), [](const Offer& a, const Offer& b) {
                        if (a.cost != b.cost) return a.cost < b.cost;
                        if (a.duration != b.duration) return a.duration < b.duration;
                        return a.key < b.key;
                    });
                    const Offer& best = offers.front();
                    plan.spend[best.source] += best.amount_source;
                    plan.total_duration_mks += best.duration;
                    plan.conversions.push_back(
                        Conversion{best.source, resource, best.amount_source, remaining,
                                   best.rate, best.duration});
                    remaining = 0.0;
                }
            }
            if (remaining > 0.0) plan.uncovered[resource] = remaining;
        }

        // §4.8 (v0.7): the mandate caps what may be spent, in the group numeraire.
        // It can only remove an option a larger balance would have paid for, and it
        // can never make payable what the measured means cannot cover.
        if (cap) {
            double spent = 0.0;
            for (const auto& kv : plan.spend) spent += weight_of(kv.first) * kv.second;
            if (spent > *cap) plan.mandate_exceeded = spent - *cap;
        }
        plan.covered = plan.uncovered.empty() && plan.mandate_exceeded <= 0.0;
        return plan;
    }

    // §4.8 step 3: an unpayable option is inadmissible, unconditionally. Unlike
    // the structural gate of §4.5 there is no "no alternative" escape: a shortage
    // that survives full verified conversion is a verdict, not a price.
    std::pair<std::vector<ActionOption>, std::vector<RemovedOption>> apply_resource_gate(
        const SystemStateMatrix& state, const std::vector<ActionOption>& options,
        const std::vector<std::vector<std::string>>* groups = nullptr,
        const std::map<std::string, dof::Rate>* rates = nullptr,
        const std::map<std::string, double>* weights = nullptr,
        const std::optional<double>& cap = std::nullopt) const {
        if (options.empty()) return {{}, {}};
        std::vector<ActionOption> admissible;
        std::vector<RemovedOption> removed;
        for (const auto& option : options) {
            if (plan_funding(state, option, groups, rates, weights, cap).covered) {
                admissible.push_back(option);
            } else {
                removed.push_back(RemovedOption{option.option_id, "insolvency"});
            }
        }
        return {admissible, removed};
    }

    // ---------------------------------------------------------------------
    // §4.5 (v0.8): the candidate vector and the ordered test
    // ---------------------------------------------------------------------

    // The set of entities of calc(S) whose current_dof is the minimum over calc(S)
    // (§4.5). A set, not a node: a minimum attained by several known zeros has no
    // unique "critical node", and a flag would have to invent a tie-break.
    std::set<std::string> critical_members(const SystemStateMatrix& state,
                                           const ObservationContext* ctx = nullptr) const {
        std::set<std::string> out;
        std::set<std::string> members = calc_members(state, ctx);
        bool found = false;
        double lowest = std::numeric_limits<double>::infinity();
        for (const auto& id : members) {
            auto it = state.entities.find(id);
            if (it == state.entities.end()) continue;
            lowest = std::min(lowest, it->second.current_dof);
            found = true;
        }
        if (!found) return out;
        for (const auto& id : members) {
            auto it = state.entities.find(id);
            if (it != state.entities.end() && it->second.current_dof == lowest) out.insert(id);
        }
        return out;
    }

    // The entities this option drops out of a `reachable` verdict, line by line
    // (§4.5, §6.3).
    //
    // The verdict procedure runs twice over the SAME observation — once as
    // observed, once with the option's closure applied — so a verdict can only
    // move away from `reachable`, and the difference is computed rather than
    // declared. A lost witness is a loss: an entity that leaves `reachable` counts
    // even where no exclusion follows from it, because §4.2 excludes only on a
    // proven_unreachable verdict over a complete observation.
    std::vector<LostPathEntry> lost_paths_for(const SystemStateMatrix& state,
                                              const ActionOption& option,
                                              const ObservationContext* ctx,
                                              const std::string& hypothesis_id) const {
        std::vector<LostPathEntry> out;
        const std::vector<dof::ClosedRef> closed = closed_for(option, hypothesis_id);
        if (ctx == nullptr || closed.empty()) return out;
        dof::WorldGraph closed_world = ctx->world.with_closed(closed);
        std::set<std::string> critical = critical_members(state, ctx);
        for (const auto& kv : state.entities) {
            const std::string& id = kv.first;
            const EntityState& entity = kv.second;
            // The verdict procedure runs twice over the SAME observation — once as
            // observed, once with the option's closure applied — so a verdict can
            // only move away from `reachable`, and the difference is computed
            // rather than declared.
            //
            // The before-state DoF is the one the **state being read** carries:
            // under a hypothesis reading the caller passes that reading's state,
            // whose `current_dof` is its own `DoF(X | h)`.
            dof::Verdict before = ctx->world.verdict_with_dof(
                id, ctx->means_class, ctx->horizon(id), entity.current_dof);
            if (before.verdict != "reachable") continue;
            // The second verdict reads the state the option LEAVES BEHIND — the
            // counters the closure changed, with the lens values recomputed from
            // them — and reads them **without** the option's `projected_dof_delta`.
            // D2 asks "did the closure destroy a recovery path?", not "is the
            // entity better off after the option's promised effect?" — the latter
            // is NetDelta. Were the projection admitted here, an option could raise
            // the after-state DoF with its own promise and buy back the very
            // recoverability it destroys. That is why the base DoF of the second
            // run is `dof_after_closure` and never `projected_dof`; a nullopt means
            // the closure did not touch this entity, which then keeps its DoF.
            //
            // §6.3: the after-state is **this reading's** own, so the counters are
            // recomputed from the same `closure[h]` the graph above was pruned by.
            // Reading `closure[$observed$]` here — as the reference did before this
            // correction — built a state no hypothesis produces: one reading's graph
            // with another reading's counters, which is what §6.3's "MUST NOT read
            // one reading's closures against another reading's state" forbids.
            std::optional<double> after_dof =
                dof_after_closure_for(entity, option, ctx, hypothesis_id);
            dof::Verdict after = closed_world.verdict_with_dof(
                id, ctx->means_class, ctx->horizon(id), after_dof);
            if (after.verdict == "reachable") continue;
            LostPathEntry row;
            row.entity_id = id;
            row.verdict_before = before.verdict;
            row.verdict_after = after.verdict;
            row.critical = critical.count(id) > 0;
            row.witness_lost = before.witness;
            out.push_back(row);
        }
        return out;
    }

    // The observed-reading entry point.
    std::vector<LostPathEntry> lost_paths(const SystemStateMatrix& state,
                                          const ActionOption& option,
                                          const ObservationContext* ctx = nullptr) const {
        return lost_paths_for(state, option, ctx, kObservedHypothesisId);
    }

    // The keys of one candidate (§4.5): all of them, from quantities the earlier
    // releases already produce.
    //
    // §4.10 (v0.11): every key comes from **one** reading — `d1`, `d2`, `d3`,
    // `NetDelta` and `is_reversible` are the numbers of the same `h`. Mixing them
    // (a worst-case `d1` with an observed `NetDelta`) would compare two different
    // worlds under one name.
    CandidateVector candidate_vector_for(const SystemStateMatrix& state,
                                         const ActionOption& option,
                                         const ObservationContext* ctx, double current_index,
                                         const std::string& hypothesis_id) const {
        auto sim = simulate_for(state, option, ctx, hypothesis_id);
        double projected = calculate_system_dof(sim.first, &sim.second, ctx);
        std::vector<LostPathEntry> lost = lost_paths_for(state, option, ctx, hypothesis_id);
        int d3 = 0;
        for (const auto& row : lost) {
            if (row.critical) ++d3;
        }
        CandidateVector v;
        v.d1 = static_cast<int>(collapse_charges_for(state, option, ctx, hypothesis_id).size());
        v.d2 = static_cast<int>(lost.size());
        v.d3 = d3;
        v.net_delta = net_delta(state, option, projected, current_index);
        v.reversible = is_reversible_for(option, hypothesis_id);
        v.option_id = option.option_id;
        // §6.3: the financial condition of admissibility belongs to the vector. The
        // temporal one is set by the layer that reads τ — this header has no
        // visibility of it — and defaults to admissible, never to barred.
        v.resources_ok = plan_funding(state, option).covered;
        return v;
    }

    CandidateVector candidate_vector(const SystemStateMatrix& state, const ActionOption& option,
                                     const ObservationContext* ctx, double current_index) const {
        return candidate_vector_for(state, option, ctx, current_index, kObservedHypothesisId);
    }

    // Staying put: the zero vector, NetDelta = 0 by definition.
    static CandidateVector baseline_vector() { return CandidateVector{}; }

    // The first dimension on which a candidate fails to beat staying put (§4.5,
    // §6.2). Empty means nothing barred it: it outranks the baseline, or ties it
    // while staying reversible.
    //
    // The executability conditions come first, in the order §4.8b evaluates them —
    // temporal (`viable`), then structural, then financial (`resources_ok`). The
    // order is unobservable to the result, so any of the keys reports the same bar;
    // leaving them out would let an option that cannot be executed be reported as
    // barred for a structural reason it never reached.
    static std::optional<std::string> barring_key(const CandidateVector& v) {
        if (!v.viable) return std::string("viable");
        if (!v.resources_ok) return std::string("resources_ok");
        if (v.d1 > 0) return std::string("d1");
        if (v.d2 > 0) return std::string("d2");
        if (v.d3 > 0) return std::string("d3");
        if (v.net_delta <= 0.0) return std::string("net_delta");
        return std::nullopt;
    }

    static int dimension(const CandidateVector& v, const std::string& key) {
        if (key == "d1") return v.d1;
        if (key == "d2") return v.d2;
        return v.d3;
    }

    double net_delta(const SystemStateMatrix& current, const ActionOption& option,
                     double projected, double current_dof) const {
        // §4.4 (v0.7): no flat penalty. An irreversible option's price is already
        // inside `projected`, because the closure lowered the affected entities'
        // Variety counter in S' (§4.3); subtracting anything here would charge the
        // same loss twice.
        (void)option;
        return projected - current_dof - current.context_switch_cost;
    }

    // The v0.8 selection (§4.5): admissibility first, the index second. Returns
    // the winner, or empty when the system stays — a decision and not an absence
    // of one.
    //
    // Staying put is a candidate LIKE ANY OTHER, so its zero vector enters the set:
    // that is what makes a protected dimension a BAR instead of a comparison. Any
    // candidate with d1, d2 or d3 above zero loses to it, and no candidate can ever
    // be preferred for cutting a path. Comparing against the baseline only at the
    // NetDelta step would let a positive delta buy a lost path back — exactly the
    // defect this release removes.
    std::pair<std::optional<ActionOption>, std::vector<CandidateVector>> select_candidate(
        const SystemStateMatrix& current_state,
        const std::vector<ActionOption>& options,
        const ObservationContext* ctx = nullptr) const
    {
        std::vector<CandidateVector> vectors;
        if (options.empty()) return {std::nullopt, vectors};
        struct Entry {
            std::optional<ActionOption> option;
            CandidateVector vector;
            bool is_baseline = false;
        };
        double current = calculate_system_dof(current_state, nullptr, ctx);
        std::vector<Entry> survivors;
        for (const auto& option : options) {
            CandidateVector v = candidate_vector(current_state, option, ctx, current);
            vectors.push_back(v);
            Entry e;
            e.option = option;
            e.vector = v;
            survivors.push_back(e);
        }
        Entry baseline;
        baseline.is_baseline = true;
        baseline.vector = baseline_vector();
        survivors.push_back(baseline);

        // 1. Structural admissibility: d1 = d2 = d3 = 0. Inadmissible candidates
        //    are never compared with one another.
        const std::vector<std::string> keys{"d1", "d2", "d3"};
        for (const auto& key : keys) {
            if (survivors.empty()) break;
            int best = dimension(survivors.front().vector, key);
            for (const auto& e : survivors) best = std::min(best, dimension(e.vector, key));
            std::vector<Entry> kept;
            for (const auto& e : survivors) {
                if (dimension(e.vector, key) == best) kept.push_back(e);
            }
            survivors = kept;
        }
        // 2. The index, ties grouped with the tolerance of §10.
        if (!survivors.empty()) {
            double best = survivors.front().vector.net_delta;
            for (const auto& e : survivors) best = std::max(best, e.vector.net_delta);
            std::vector<Entry> kept;
            for (const auto& e : survivors) {
                if (std::fabs(e.vector.net_delta - best) <= net_delta_tolerance) kept.push_back(e);
            }
            survivors = kept;
        }
        // 3. Reversibility: a preference among equals, not a penalty (§4.4).
        if (!survivors.empty()) {
            bool any_reversible = false;
            for (const auto& e : survivors) {
                if (e.vector.reversible) any_reversible = true;
            }
            if (any_reversible) {
                std::vector<Entry> kept;
                for (const auto& e : survivors) {
                    if (e.vector.reversible) kept.push_back(e);
                }
                survivors = kept;
            }
        }
        // 4. A complete tie goes to staying put, if it is still a candidate.
        for (const auto& e : survivors) {
            if (e.is_baseline) return {std::nullopt, vectors};
        }
        if (!survivors.empty()) {
            std::string best_id = survivors.front().vector.option_id;
            for (const auto& e : survivors) best_id = std::min(best_id, e.vector.option_id);
            std::vector<Entry> kept;
            for (const auto& e : survivors) {
                if (e.vector.option_id == best_id) kept.push_back(e);
            }
            survivors = kept;
        }
        if (survivors.empty()) return {std::nullopt, vectors};
        // The survivor is selected only if it beats the baseline. With the baseline
        // in the set this is already implied; the guard states the rule.
        if (survivors.front().vector.net_delta <= 0.0) return {std::nullopt, vectors};
        return {survivors.front().option, vectors};
    }

    std::optional<ActionOption> evaluate_and_select(
        const SystemStateMatrix& current_state,
        const std::vector<ActionOption>& options,
        const ObservationContext* ctx = nullptr) const
    {
        return select_candidate(current_state, options, ctx).first;
    }

    // §4.7: a resolvable unknown left unmeasured in every candidate.
    bool is_incomplete(const SystemStateMatrix& state,
                       const std::vector<ActionOption>& options) const {
        double cheapest = std::numeric_limits<double>::infinity();
        for (const auto& o : options) {
            if (o.estimated_duration_mks > 0.0 && o.estimated_duration_mks < cheapest) {
                cheapest = o.estimated_duration_mks;
            }
        }
        if (!std::isfinite(cheapest)) return false; // no procedure available at all
        for (const auto& kv : state.entities) {
            const EntityState& e = kv.second;
            if (e.dof_known) continue;
            bool touched = false;
            for (const auto& o : options) {
                auto it = o.projected_dof_delta.find(e.entity_id);
                if (it != o.projected_dof_delta.end() && it->second != 0.0) { touched = true; break; }
            }
            if (touched) continue;
            // §5/§3.2b (v0.11): the window is computed from τ itself — a **null** τ
            // yields no window at all. An unknown budget is not a closed window, so
            // it never licenses leaving a resolvable unknown unmeasured.
            const std::optional<double> window = measurement_window(state, cheapest);
            if (window && *window > 0.0) return true;
        }
        return false;
    }

    // Transparent audit (DOF-SPEC §6). Required by the license (PoI).
    DofReport report(const SystemStateMatrix& current_state,
                     const std::vector<ActionOption>& options,
                     const std::optional<ActionOption>& selected,
                     const std::string& mode,
                     const ReportInput& in = ReportInput{}) const
    {
        DofReport rep;
        const ObservationContext* ctx = in.ctx;
        for (const auto& kv : current_state.entities) {
            const EntityState& e = kv.second;
            bool included = is_included(e, ctx, &current_state);
            double contribution = included ? std::log(std::max(e.current_dof, epsilon_)) : 0.0;
            rep.entities.push_back(EntityReportRow{e.entity_id, e.is_collapse_source, included, e.current_dof, e.dof_known, contribution});
            EntityReportRow& row = rep.entities.back();
            if (e.measurement) {
                row.lens_terms = e.measurement->terms;
                row.binding_lens = e.measurement->binding_lens;
                row.floored = e.measurement->floored;
                row.blocks = e.measurement->blocks;
                row.derivation = e.measurement->derivation;
            }
            // §6.1 (v0.7): the verdict, its witness and the completeness behind it.
            row.recoverability = recoverability_row(e.entity_id, ctx);
        }
        double total = calculate_system_dof(current_state, nullptr, ctx);

        // §6.2 (v0.6): the means before the cycle and after the selected option's
        // spend ledger — what actually left the stock, not what was declared.
        for (const auto& kv : current_state.resources) rep.resources_before[kv.first] = kv.second;
        rep.resources_after = rep.resources_before;
        if (selected) {
            FundingPlan plan = plan_funding(current_state, *selected, in.groups, in.rates,
                                            in.weights, in.cap);
            for (const auto& kv : plan.spend) {
                auto it = rep.resources_after.find(kv.first);
                ResourceObservation before = (it == rep.resources_after.end()) ? ResourceObservation{} : it->second;
                ResourceObservation after = before;
                after.value = std::max(0.0, resource_value(before) - kv.second);
                rep.resources_after[kv.first] = after;
            }
        }

        for (const auto& option : options) {
            auto sim_result = simulate(current_state, option, ctx);
            double projected = calculate_system_dof(sim_result.first, &sim_result.second, ctx);
            CandidateVector vec = candidate_vector(current_state, option, ctx, total);
            double net = vec.net_delta;
            bool is_selected = selected.has_value() && selected->option_id == option.option_id;
            rep.options.push_back(OptionReportRow{option.option_id, is_reversible(option), projected, net, is_selected, option.estimated_duration_mks});
            // §6.3: every collapse this option causes, as an auditable line
            rep.options.back().collapse_charges = collapse_charges(current_state, option, ctx);
            // §6.3 (v0.6): what it draws, and how "affordable" was established.
            FundingPlan plan = plan_funding(current_state, option, in.groups, in.rates,
                                            in.weights, in.cap);
            rep.options.back().resource_consumption = option.projected_resource_delta;
            rep.options.back().conversion_applied = plan.conversions;
            rep.options.back().resources_uncovered = plan.uncovered;
            // §6.3 (v0.7): the mandate that would have been exceeded, what the
            // option closes, and how the closure's loss decomposes per entity.
            rep.options.back().mandate_exceeded = plan.mandate_exceeded;
            rep.options.back().closed = option.closed;
            rep.options.back().closure_share = closure_share(current_state, option, ctx);
            // §6.3 (v0.8): the protected dimensions, the dimension that barred the
            // candidate (empty when nothing did), and the path losses line by line.
            rep.options.back().candidate_vector = vec;
            rep.options.back().barring_key = barring_key(vec);
            rep.options.back().lost_paths = lost_paths(current_state, option, ctx);
        }
        rep.total_system_dof = total;
        rep.context_switch_cost = current_state.context_switch_cost;
        rep.global_time_to_collapse_mks = current_state.global_time_to_collapse_mks;
        rep.mode = mode;
        rep.removed_options = in.removed;
        rep.incomplete = is_incomplete(current_state, options);
        rep.means_provenance = in.means_provenance;
        // §6.2 (v0.8): what the candidates were compared against, and whether any
        // of them beat it. A silent "no action" is an omission.
        rep.baseline = baseline_vector();
        rep.no_candidate_better = !options.empty() && !selected.has_value();
        if (ctx != nullptr && !ctx->observation_digest.empty()) {
            rep.observation_digest = ctx->observation_digest;
        }
        if (in.declaration) {
            rep.psi_id = in.declaration->psi_id;
            rep.psi_digest = in.declaration->digest();
            rep.declaration = in.declaration->canonical_text();
        } else if (current_state.psi) {
            rep.psi_id = current_state.psi->id;
            rep.psi_digest = current_state.psi->digest;
        }
        return rep;
    }
};
