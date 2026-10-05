// Measurement layer of the Rust port (DOF-SPEC §3.4, §4.6, §4.7 — v0.4).
//
// Perception side: raw lens inputs -> ψ per lens -> the product that becomes
// current_dof, plus the frozen declaration and its canonical digest.
//
//   ψ_var = V / (V + V_env)
//   ψ_opt = Π_g f_g(x_g),  f_g(x) = 4^(−x),  x_g = c_g / C_g
//   ψ_con = F / (F + F_env)
//
// Guard (§4.6): if a lens has neither a numerator nor an external clamp its
// value is 0 — uniform across lenses, so no 0/0 and no NaN can poison the sum.
// Unmeasured lenses are not zero and not ideal (§4.7): they enter the product
// as u(t), the entity's dof_known becomes false, and §4.2 keeps it in calc.

use std::collections::BTreeMap;
use std::fmt::Write;

pub const EPSILON: f64 = 1e-6;
pub const U_ALPHA: f64 = 0.25;
pub const U_MAX: f64 = 0.5;
/// ε^(1−ρ) with ρ = 0.9 (§4.7).
pub fn u_min() -> f64 {
    EPSILON.powf(1.0 - 0.9)
}

pub const LENS_ORDER: [&str; 3] = ["variety", "options", "constraint"];

/// §4.6 (v0.6): the Options blocks come from a named derivation procedure, which
/// is part of the frozen ruler (`procedures["options_blocks"]`), and every
/// option that names an entity must declare its energy draw (§3.3).
pub const DERIVE_BLOCKS_PROCEDURE: &str = "derive_blocks";
pub const MANDATORY_RESOURCE: &str = "energy";

fn clamp01(x: f64) -> f64 {
    x.max(0.0).min(1.0)
}

/// Variety lens (§4.6). V = 0 ⇒ 0, including the (0,0) case.
pub fn psi_var(v: f64, v_env: f64) -> f64 {
    if v <= 0.0 {
        return 0.0;
    }
    clamp01(v / (v + v_env.max(0.0)))
}

/// Constraint lens (§4.6). F = 0 ⇒ 0, including the (0,0) case.
pub fn psi_con(f: f64, f_env: f64) -> f64 {
    if f <= 0.0 {
        return 0.0;
    }
    clamp01(f / (f + f_env.max(0.0)))
}

/// Options lens (§4.6). `blocks` holds (c_g, C_g) per resource block: an empty
/// repertoire means no reachable transition at all ⇒ 0; a block with c_g = 0
/// does not participate; a block with c_g > 0 and C_g = 0 is dead ⇒ 0.
pub fn psi_opt(blocks: &[(f64, f64)]) -> f64 {
    if blocks.is_empty() {
        return 0.0;
    }
    let mut value = 1.0;
    for (c_g, cap_g) in blocks {
        if *c_g <= 0.0 {
            continue;
        }
        if *cap_g <= 0.0 {
            return 0.0;
        }
        value *= 4.0_f64.powf(-(c_g / cap_g));
    }
    clamp01(value)
}

/// §4.8 (v0.6): the exchange-group partition is analysis-side and canonical.
/// Declared groups are normalized (members sorted, group list sorted); every
/// resource that appears in the raw inputs but in no group forms a **singleton
/// group** of its own, so a requirement can never be silently dropped from the
/// derivation.
pub fn canonical_groups(
    groups: &[Vec<String>],
    requirements: Option<&BTreeMap<String, f64>>,
    means: Option<&BTreeMap<String, f64>>,
) -> Vec<Vec<String>> {
    let mut named: BTreeMap<String, ()> = BTreeMap::new();
    let mut normalized: Vec<Vec<String>> = Vec::new();
    for group in groups {
        let mut members: Vec<String> = group.clone();
        members.sort();
        members.dedup();
        if members.is_empty() {
            continue;
        }
        for member in members.iter() {
            named.insert(member.clone(), ());
        }
        normalized.push(members);
    }
    let mut extra: BTreeMap<String, ()> = BTreeMap::new();
    for source in [requirements, means].iter() {
        if let Some(map) = source {
            for key in map.keys() {
                if !named.contains_key(key) {
                    extra.insert(key.clone(), ());
                }
            }
        }
    }
    for key in extra.keys() {
        normalized.push(vec![key.clone()]);
    }
    normalized.sort();
    normalized
}

/// §4.6 (v0.7): the derived `(c_g, C_g)` pair of every resource block. Named
/// procedure: resources inside a group are mutually exchangeable, so they share
/// one block — `c_g` is what the transition draws from the group, `C_g` is what
/// the agent can commit to it. Zero is legal on both sides; `psi_opt` then
/// applies the `c_g > 0 ∧ C_g = 0` gate. The derivation is total: every resource
/// of the inputs lands in exactly one group.
///
/// `weights` are the observed prices of each resource in the group's numeraire.
/// Without them the sum adds credits to joules, and the value of the lens starts to
/// depend on the unit a resource happens to be declared in: the lens would measure
/// notation instead of the world. A resource with no path to the numeraire is its
/// own singleton group and carries weight 1.0 — with no exchange available, its own
/// unit IS its nominal.
///
/// `cap` is the mandate: permission, never possibility. It can only lower `C_g`.
pub fn derive_blocks(
    requirements: &BTreeMap<String, f64>,
    means: &BTreeMap<String, f64>,
    groups: &[Vec<String>],
    weights: Option<&BTreeMap<String, f64>>,
    cap: Option<f64>,
) -> Vec<(f64, f64)> {
    let weight_of = |r: &str| -> f64 {
        match weights {
            Some(w) => w.get(r).copied().unwrap_or(1.0),
            None => 1.0,
        }
    };
    let mut blocks = Vec::new();
    for group in canonical_groups(groups, Some(requirements), Some(means)) {
        let mut c_g = 0.0;
        let mut cap_g = 0.0;
        for resource in group.iter() {
            if let Some(v) = requirements.get(resource) {
                c_g += weight_of(resource) * v.max(0.0);
            }
            if let Some(v) = means.get(resource) {
                cap_g += weight_of(resource) * v.max(0.0);
            }
        }
        if let Some(c) = cap {
            cap_g = cap_g.min(c.max(0.0));
        }
        blocks.push((c_g, cap_g));
    }
    blocks
}

/// Raw lens inputs of one entity. `None` = the lens was never measured. The
/// Options lens takes exactly one of two inputs: `options` (the blocks
/// themselves) or `requirements` (raw per-resource demands, from which the
/// blocks are derived against the agent's means and the groups).
#[derive(Clone, Debug, Default)]
pub struct LensObservation {
    pub variety: Option<(f64, f64)>,          // (V, V_env)
    pub options: Option<Vec<(f64, f64)>>,     // [(c_g, C_g)]
    pub constraint: Option<(f64, f64)>,       // (F, F_env)
    pub requirements: Option<BTreeMap<String, f64>>,  // {"energy": 4.0} (§4.6)
    // §4.6 (v0.7): the numeraire weights and the mandate cap. Declared per
    // observation as an alternative to passing them in; the caller's values win.
    pub weights: Option<BTreeMap<String, f64>>,
    pub cap: Option<f64>,
}

impl LensObservation {
    pub fn psi(
        &self,
        lens: &str,
        means: Option<&BTreeMap<String, f64>>,
        groups: Option<&Vec<Vec<String>>>,
        call_weights: Option<&BTreeMap<String, f64>>,
        call_cap: Option<f64>,
    ) -> Option<f64> {
        let eff_weights = call_weights.or(self.weights.as_ref());
        let eff_cap = call_cap.or(self.cap);
        match lens {
            "variety" => self.variety.map(|(v, ve)| psi_var(v, ve)),
            "options" => {
                if let Some(blocks) = self.options.as_ref() {
                    return Some(psi_opt(blocks));
                }
                if let Some(reqs) = self.requirements.as_ref() {
                    let no_means: BTreeMap<String, f64> = BTreeMap::new();
                    let no_groups: Vec<Vec<String>> = Vec::new();
                    return Some(psi_opt(&derive_blocks(
                        reqs,
                        means.unwrap_or(&no_means),
                        groups.unwrap_or(&no_groups),
                        eff_weights,
                        eff_cap,
                    )));
                }
                None
            }
            "constraint" => self.constraint.map(|(f, fe)| psi_con(f, fe)),
            _ => None,
        }
    }
}

/// Base level of the ignorance penalty (§4.7).
pub fn u0_from_prior(prior_q: Option<f64>) -> f64 {
    let q = prior_q.unwrap_or(0.5);
    q.max(u_min()).min(U_MAX)
}

/// `T_meas = t_m + t_v + max(t_a⁺, t_a⁻)` (§4.7).
pub fn total_budget_mks(t_m: f64, t_v: f64, t_a_plus: f64, t_a_minus: f64) -> f64 {
    t_m + t_v + t_a_plus.max(t_a_minus)
}

/// `u(t) = u₀^(1 − t/t*) · ε^(t/t*)` on `t ∈ [0, t*]` (§4.7).
///
/// §4.7/§10(au): an **unmeasured** τ prices the ignorance at `u₀` — no window is
/// computable, so no deadline is being spent. A τ that is known but leaves no
/// window (`t* <= 0`) prices it at `ε`, not at `u₀`: the `v0.9.1` branch returned
/// `u₀` here, which put a jump of ~13 nats exactly where measurement stops being
/// possible.
pub fn u_of_t(u0: f64, tau_mks: Option<f64>, t_meas_mks: f64, t_mks: f64) -> f64 {
    let tau = match tau_mks {
        Some(v) => v,
        None => return u0,
    };
    let t_star = tau - t_meas_mks;
    if t_star <= 0.0 {
        return EPSILON;
    }
    let t = t_mks.max(0.0).min(t_star);
    let w = t / t_star;
    u0.powf(1.0 - w) * EPSILON.powf(w)
}

/// One row of the (entity × lens) ledger.
#[derive(Clone, Debug)]
pub struct LensTerm {
    pub lens: String,
    pub psi: Option<f64>,
    pub dof_known: bool,
    pub contribution: f64,
}

/// The named derivation behind an entity's blocks (§4.6), kept so the audit can
/// show the raw inputs a reader needs to recompute `(c_g, C_g)`.
#[derive(Clone, Debug)]
pub struct DerivationInfo {
    pub procedure: String,
    pub requirements: BTreeMap<String, f64>,
    pub means: BTreeMap<String, f64>,
    pub groups: Vec<Vec<String>>,
    /// §4.6 (v0.7): the numeraire weights and the mandate cap are part of the
    /// derivation, so a reader can recompute `(c_g, C_g)` and see that the sum is
    /// not adding different physical units together.
    pub weights: BTreeMap<String, f64>,
    pub cap: Option<f64>,
}

/// Result of measuring one entity.
#[derive(Clone, Debug)]
pub struct EntityMeasurement {
    pub entity_id: String,
    pub psi: BTreeMap<String, Option<f64>>,
    pub terms: Vec<LensTerm>,
    pub current_dof: f64,
    pub dof_known: bool,
    pub contribution: f64,
    pub terms_sum: f64,
    pub floored: bool,
    pub binding_lens: Option<String>,
    /// §4.6 (v0.6): the derived blocks actually used, and the derivation itself.
    pub blocks: Vec<(f64, f64)>,
    pub derivation: Option<DerivationInfo>,
    /// §4.6 (v0.7): the declared counters behind the Variety share — kept because
    /// the price of a closure is recomputed from them (§4.4), not from the lens.
    pub variety_counters: Option<BTreeMap<String, f64>>,
}

/// Apply §4.6–§4.7 to one entity.
pub fn measure_entity(
    entity_id: &str,
    obs: &LensObservation,
    u: f64,
    means: Option<&BTreeMap<String, f64>>,
    groups: Option<&Vec<Vec<String>>>,
    call_weights: Option<&BTreeMap<String, f64>>,
    call_cap: Option<f64>,
) -> EntityMeasurement {
    let eff_weights = call_weights.or(obs.weights.as_ref());
    let eff_cap = call_cap.or(obs.cap);
    let mut psi: BTreeMap<String, Option<f64>> = BTreeMap::new();
    let mut terms: Vec<LensTerm> = Vec::new();
    let mut product = 1.0;
    let mut known_all = true;
    let mut terms_sum = 0.0;
    let mut binding: Option<String> = None;
    let mut binding_value = f64::INFINITY;

    for lens in LENS_ORDER.iter() {
        let value = obs.psi(lens, means, groups, eff_weights, eff_cap);
        psi.insert((*lens).to_string(), value);
        let contribution = match value {
            None => {
                known_all = false;
                product *= u;
                u.ln()
            }
            Some(v) => {
                product *= v;
                if v < binding_value {
                    binding_value = v;
                    binding = Some((*lens).to_string());
                }
                v.max(EPSILON).ln()
            }
        };
        terms_sum += contribution;
        terms.push(LensTerm {
            lens: (*lens).to_string(),
            psi: value,
            dof_known: value.is_some(),
            contribution,
        });
    }

    let no_means: BTreeMap<String, f64> = BTreeMap::new();
    let no_groups: Vec<Vec<String>> = Vec::new();
    let effective_means = means.unwrap_or(&no_means);
    let effective_groups = groups.unwrap_or(&no_groups);
    let (blocks, derivation) = match obs.requirements.as_ref() {
        Some(reqs) => (
            derive_blocks(reqs, effective_means, effective_groups, eff_weights, eff_cap),
            Some(DerivationInfo {
                procedure: DERIVE_BLOCKS_PROCEDURE.to_string(),
                requirements: reqs.clone(),
                means: effective_means.clone(),
                groups: canonical_groups(effective_groups, Some(reqs), Some(effective_means)),
                weights: eff_weights.cloned().unwrap_or_default(),
                cap: eff_cap,
            }),
        ),
        None => (Vec::new(), None),
    };

    // §4.6 (v0.7): the declared counters are kept with the measurement, because the
    // price of a closure (§4.4) is recomputed FROM them rather than from the lens.
    let variety_counters = obs.variety.map(|(v, v_env)| {
        let mut m: BTreeMap<String, f64> = BTreeMap::new();
        m.insert("V".to_string(), v);
        m.insert("V_env".to_string(), v_env);
        m
    });

    EntityMeasurement {
        entity_id: entity_id.to_string(),
        psi,
        terms,
        current_dof: clamp01(product),
        dof_known: known_all,
        contribution: product.max(EPSILON).ln(),
        terms_sum,
        floored: product < EPSILON,
        binding_lens: binding,
        blocks,
        derivation,
        variety_counters,
    }
}

/// §3.4.1 (v0.6): the resource layer of the ruler — identities with unit name
/// and scale, the observed rates, and the declared mandate. Two implementations
/// that declare the same resource name with different scales are measurably
/// different rulers and will produce different digests (§4.8).
#[derive(Clone, Debug)]
pub struct ResourceUnit {
    pub id: String,
    pub unit: String,
    pub scale: f64,
}

/// An observed exchange rate: the key is `"from->to"` (one unit of `from` yields
/// `rate` units of `to`), plus the exchange's own duration, which the gate of
/// §4.8 charges to the same τ as the option itself.
#[derive(Clone, Debug)]
pub struct Rate {
    pub rate: f64,
    pub duration_mks: f64,
}

/// Mandate entries are either numbers (rendered like every other number, as a
/// fixed six-decimal string) or free text.
#[derive(Clone, Debug)]
pub enum MandateValue {
    Number(f64),
    Text(String),
}

/// One entity's declared verdict together with the counters and the horizon it was
/// computed with, so the declaration can be checked against the observation it came
/// from (`verify_graph_derived`).
#[derive(Clone, Debug)]
pub struct VerdictRecord {
    pub verdict: String,
    pub t_rec_mks: Option<f64>,
    /// A count, not a measurement: serialized as an INTEGER.
    pub v: i64,
}

/// The frozen ruler (§3.4). `BTreeMap` keeps entity keys sorted, which the
/// canonical form requires.
#[derive(Clone, Debug)]
pub struct MeasurementDeclaration {
    pub psi_id: String,
    pub u0_prior_q: Option<f64>,
    pub entities: BTreeMap<String, LensObservation>,
    /// §3.2b (v0.11): τ as the ruler froze it — `None` when **unmeasured**, which
    /// is `null` and not `0.0`. A negative τ is a passed deadline and keeps its
    /// magnitude.
    pub tau_mks: Option<f64>,
    // §3.4.1 hashed content (v0.6): the ruler now includes the resource layer.
    pub resources: Vec<ResourceUnit>,
    pub groups: Vec<Vec<String>>,
    pub rates: BTreeMap<String, Rate>,
    pub mandate: BTreeMap<String, MandateValue>,
    // §3.4.1 hashed content (v0.7): the graph-derived values of §4.9 and the
    // numeraire the group amounts are expressed in. Only what determines numbers is
    // here — the graph itself, the witness paths and the observation digest are
    // report context (§6.2), and an option's closure list is a per-option input like
    // `projected_dof_delta`, not ruler content.
    pub numeraire: Option<String>,
    pub weights: BTreeMap<String, f64>,
    pub mandate_cap: Option<f64>,
    pub verdicts: BTreeMap<String, VerdictRecord>,
    pub means_class: Vec<String>,
    pub graph_procedure: String,
    /// §3.4.1/§3.4.3 (v0.11): the declared measurement durations `t_m`, `t_v` per
    /// lens. **Additive** hashed content: the key is omitted when the map is
    /// empty, so a state that declares no duration hashes exactly as it did before
    /// the field existed and the `v0.7`/`v0.8`/`v0.9.1` fingerprints are untouched,
    /// while a state that *does* declare durations hashes them.
    pub measurement_durations: BTreeMap<String, BTreeMap<String, f64>>,
}

impl MeasurementDeclaration {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        psi_id: &str,
        entities: BTreeMap<String, LensObservation>,
        tau_mks: Option<f64>,
        u0_prior_q: Option<f64>,
        resources: Vec<ResourceUnit>,
        groups: Vec<Vec<String>>,
        rates: BTreeMap<String, Rate>,
        mandate: BTreeMap<String, MandateValue>,
        measurement_durations: BTreeMap<String, BTreeMap<String, f64>>,
    ) -> Self {
        MeasurementDeclaration {
            psi_id: psi_id.to_string(),
            u0_prior_q,
            entities,
            tau_mks,
            resources,
            groups: canonical_groups(&groups, None, None),
            rates,
            mandate,
            numeraire: None,
            weights: BTreeMap::new(),
            mandate_cap: None,
            verdicts: BTreeMap::new(),
            means_class: Vec::new(),
            graph_procedure: String::new(),
            measurement_durations,
        }
    }

    pub fn u0(&self) -> f64 {
        u0_from_prior(self.u0_prior_q)
    }

    /// JSON for the resource layer. `BTreeMap` keeps keys sorted; resources are
    /// sorted by id, so the order they were declared in cannot matter.
    fn groups_json(groups: &[Vec<String>]) -> String {
        let mut s = String::from("[");
        for (i, group) in groups.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push('[');
            for (j, member) in group.iter().enumerate() {
                if j > 0 {
                    s.push(',');
                }
                let _ = write!(s, "\"{}\"", member);
            }
            s.push(']');
        }
        s.push(']');
        s
    }

    fn mandate_json(mandate: &BTreeMap<String, MandateValue>) -> String {
        let mut s = String::from("{");
        for (i, (key, value)) in mandate.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(s, "\"{}\":", key);
            match value {
                MandateValue::Number(n) => {
                    let _ = write!(s, "\"{:.6}\"", n);
                }
                MandateValue::Text(t) => {
                    let _ = write!(s, "\"{}\"", t);
                }
            }
        }
        s.push('}');
        s
    }

    fn rates_json(rates: &BTreeMap<String, Rate>) -> String {
        let mut s = String::from("{");
        for (i, (key, value)) in rates.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(
                s,
                "\"{}\":{{\"duration_mks\":\"{:.6}\",\"rate\":\"{:.6}\"}}",
                key, value.duration_mks, value.rate
            );
        }
        s.push('}');
        s
    }

    fn resources_json(resources: &[ResourceUnit]) -> String {
        let mut sorted: Vec<&ResourceUnit> = resources.iter().collect();
        sorted.sort_by(|a, b| a.id.cmp(&b.id));
        let mut s = String::from("[");
        for (i, r) in sorted.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(
                s,
                "{{\"id\":\"{}\",\"scale\":\"{:.6}\",\"unit\":\"{}\"}}",
                r.id, r.scale, r.unit
            );
        }
        s.push(']');
        s
    }

    /// JSON for the declared measurement durations: keys sorted (`BTreeMap`), every
    /// value a fixed six-decimal string, exactly as every other float of the
    /// canonical form (§3.4.1).
    fn durations_json(durations: &BTreeMap<String, BTreeMap<String, f64>>) -> String {
        let mut s = String::from("{");
        for (i, (lens, durs)) in durations.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(s, "\"{}\":{{", lens);
            for (j, (name, value)) in durs.iter().enumerate() {
                if j > 0 {
                    s.push(',');
                }
                let _ = write!(s, "\"{}\":\"{:.6}\"", name, value);
            }
            s.push('}');
        }
        s.push('}');
        s
    }

    /// Canonical form (§3.4.3): UTF-8 JSON, keys sorted, no insignificant
    /// whitespace, non-integer numbers as fixed six-decimal strings.
    pub fn canonical_text(&self) -> String {
        self.render(false)
    }

    /// §3.4.2/§3.4.3: the **ruler-level** content of the same declaration.
    ///
    /// Everything identical for every hypothesis of a cycle and for every option:
    /// the procedure and its version, the lens set, the units and scales, the means
    /// class `M(S)`, `T_rec(X)`, the derived groups, the observed rates with their
    /// numeraire, the mandate, the `u₀` prior and the graph procedure identity.
    ///
    /// Three groups of fields are **excluded**, and each for a stated reason:
    ///
    /// * `entities` — the per-entity **lens counters**, which are precisely what a
    ///   hypothesis varies (§3.6);
    /// * `freeze` — τ, the measurement durations and the budgets, which are the
    ///   hypothesis's own measured content;
    /// * `verdicts` — the §4.9 **verdict**, which consumes `DoF(X | h)` and is
    ///   therefore computed per hypothesis (§4.9). Its horizon `T_rec(X)` is
    ///   type-derived and shared, but it does not have to be *hashed* for the
    ///   readings to be comparable.
    ///
    /// The exclusion is what makes the ruler **one** object for every reading of a
    /// cycle while `digest()` is not, and it lets a reader check that two readings
    /// share the ruler byte for byte (§6.3, §7 п.25).
    pub fn ruler_canonical_text(&self) -> String {
        self.render(true)
    }

    /// §3.4.2: the conformance fingerprint of the ruler — the value a port
    /// declares against its release.
    pub fn ruler_digest(&self) -> String {
        sha256_hex(self.ruler_canonical_text().as_bytes())
    }

    /// The single builder of both documents.
    ///
    /// `ruler` drops exactly the three hypothesis-level groups; nothing else
    /// differs, so the two forms cannot drift apart in field order, escaping or
    /// numeric rendering — the reason this is one function and not two.
    fn render(&self, ruler: bool) -> String {
        let mut s = String::from("{");
        let mut started = false;
        if !ruler {
            Self::sep(&mut s, &mut started);
            s.push_str("\"entities\":{");
        let mut first = true;
        for (eid, obs) in self.entities.iter() {
            if !first {
                s.push(',');
            }
            first = false;
            // §4.6 (v0.7): the per-entity numeraire weights and mandate cap. `null`
            // when the entity does not declare them — a missing key and a null key
            // hash differently, and every port must write the same shape.
            let _ = write!(s, "\"{}\":{{\"cap\":", eid);
            match obs.cap {
                Some(c) => {
                    let _ = write!(s, "\"{:.6}\"", c);
                }
                None => s.push_str("null"),
            }
            s.push_str(",\"constraint\":");
            match obs.constraint {
                Some((f, fe)) => {
                    let _ = write!(s, "{{\"F\":\"{:.6}\",\"F_env\":\"{:.6}\"}}", f, fe);
                }
                None => s.push_str("null"),
            }
            s.push_str(",\"options\":");
            match &obs.options {
                Some(blocks) => {
                    s.push('[');
                    let mut bfirst = true;
                    for (c, cap) in blocks {
                        if !bfirst {
                            s.push(',');
                        }
                        bfirst = false;
                        let _ = write!(s, "[\"{:.6}\",\"{:.6}\"]", c, cap);
                    }
                    s.push(']');
                }
                None => s.push_str("null"),
            }
            s.push_str(",\"requirements\":");
            match &obs.requirements {
                Some(reqs) => {
                    s.push('{');
                    for (i, (key, value)) in reqs.iter().enumerate() {
                        if i > 0 {
                            s.push(',');
                        }
                        let _ = write!(s, "\"{}\":\"{:.6}\"", key, value);
                    }
                    s.push('}');
                }
                None => s.push_str("null"),
            }
            s.push_str(",\"variety\":");
            match obs.variety {
                Some((v, ve)) => {
                    let _ = write!(s, "{{\"V\":\"{:.6}\",\"V_env\":\"{:.6}\"}}", v, ve);
                }
                None => s.push_str("null"),
            }
            s.push_str(",\"weights\":");
            match &obs.weights {
                Some(w) => {
                    s.push('{');
                    for (i, (key, value)) in w.iter().enumerate() {
                        if i > 0 {
                            s.push(',');
                        }
                        let _ = write!(s, "\"{}\":\"{:.6}\"", key, value);
                    }
                    s.push('}');
                }
                None => s.push_str("null"),
            }
            s.push('}');
        }
        s.push('}'); // the `entities` map
        } // `if !ruler`: entities
        if !ruler {
            // §3.4.3 (v0.11): `measurement_durations` is hashed ruler content, and
            // an **absent** map and an **empty** map hash alike — so the field is
            // written only when it carries something, and the historical fixtures,
            // which declare none, keep the digests of their releases. Every value is
            // a fixed six-decimal string, exactly as every other float of the
            // canonical form.
            Self::sep(&mut s, &mut started);
            s.push_str("\"freeze\":{");
            if !self.measurement_durations.is_empty() {
            s.push_str("\"measurement_durations\":");
            s.push_str(&Self::durations_json(&self.measurement_durations));
            s.push(',');
        }
        match self.tau_mks {
            Some(v) => {
                let _ = write!(s, "\"tau_mks\":\"{:.6}\"", v);
            }
            // An unmeasured τ MUST NOT be written as a number: `0.0` would be a
            // measured catastrophe and a synthetic default a fabricated deadline
            // (§3.2b).
            None => s.push_str("\"tau_mks\":null"),
        }
        s.push('}');
        } // `if !ruler`: freeze
        Self::sep(&mut s, &mut started);
        let _ = write!(s, "\"graph_procedure\":\"{}\"", self.graph_procedure);
        Self::sep(&mut s, &mut started);
        s.push_str("\"groups\":");
        s.push_str(&Self::groups_json(&self.groups));
        Self::sep(&mut s, &mut started);
        s.push_str("\"lens_order\":[\"variety\",\"options\",\"constraint\"],\"mandate\":");
        s.push_str(&Self::mandate_json(&self.mandate));
        Self::sep(&mut s, &mut started);
        s.push_str("\"mandate_cap\":");
        match self.mandate_cap {
            Some(c) => {
                let _ = write!(s, "\"{:.6}\"", c);
            }
            None => s.push_str("null"),
        }
        Self::sep(&mut s, &mut started);
        s.push_str("\"means_class\":[");
        {
            let mut cls = self.means_class.clone();
            cls.sort();
            for (i, c) in cls.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                let _ = write!(s, "\"{}\"", c);
            }
        }
        s.push(']'); // the `means_class` array
        Self::sep(&mut s, &mut started);
        s.push_str("\"numeraire\":");
        match &self.numeraire {
            Some(n) => {
                let _ = write!(s, "\"{}\"", n);
            }
            None => s.push_str("null"),
        }
        Self::sep(&mut s, &mut started);
        s.push_str("\"procedures\":{");
        let _ = write!(
            s,
            "\"constraint\":\"{}:constraint\",\"options\":\"{}:options\",\"options_blocks\":\"{}:{}\",\"variety\":\"{}:variety\"",
            self.psi_id, self.psi_id, self.psi_id, DERIVE_BLOCKS_PROCEDURE, self.psi_id
        );
        s.push('}');
        Self::sep(&mut s, &mut started);
        let _ = write!(s, "\"psi_id\":\"{}\"", self.psi_id);
        Self::sep(&mut s, &mut started);
        s.push_str("\"rates\":");
        s.push_str(&Self::rates_json(&self.rates));
        Self::sep(&mut s, &mut started);
        s.push_str("\"resources\":");
        s.push_str(&Self::resources_json(&self.resources));
        Self::sep(&mut s, &mut started);
        s.push_str("\"u0_prior_q\":");
        match self.u0_prior_q {
            Some(q) => {
                let _ = write!(s, "\"{:.6}\"", q);
            }
            None => s.push_str("null"),
        }
        // §4.9 (v0.7): the verdicts and counters the declaration claims, and the
        // numeraire weights those claims were computed with. §4.9 (v0.11): the
        // verdict consumes `DoF(X | h)`, so it is hypothesis-level and is **not**
        // part of the ruler-level document.
        if !ruler {
            Self::sep(&mut s, &mut started);
            s.push_str("\"verdicts\":{");
        for (i, (eid, rec)) in self.verdicts.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(s, "\"{}\":{{\"t_rec_mks\":", eid);
            match rec.t_rec_mks {
                Some(h) => {
                    let _ = write!(s, "\"{:.6}\"", h);
                }
                None => s.push_str("null"),
            }
            // `v` is an INTEGER: a count, not a measurement.
            let _ = write!(s, ",\"v\":{},\"verdict\":\"{}\"}}", rec.v, rec.verdict);
        }
            s.push('}');
        } // `if !ruler`: verdicts
        Self::sep(&mut s, &mut started);
        s.push_str("\"weights\":{");
        for (i, (key, value)) in self.weights.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(s, "\"{}\":\"{:.6}\"", key, value);
        }
        s.push('}'); // the `weights` map
        s.push('}'); // the document
        s
    }

    /// Emits the separator before a top-level block: the document is a JSON object,
    /// so the first block carries no comma. `ruler` drops blocks, which is why the
    /// comma cannot be written by the block itself.
    fn sep(s: &mut String, started: &mut bool) {
        if *started {
            s.push(',');
        }
        *started = true;
    }

    /// Lowercase hex SHA-256 of the canonical text (§3.4.3).
    pub fn digest(&self) -> String {
        sha256_hex(self.canonical_text().as_bytes())
    }
}

/// The §3.4 reference stored in the state.
#[derive(Clone, Debug)]
pub struct PsiReference {
    pub id: String,
    pub digest: String,
}

// --- SHA-256 (std has no hashing; kept here so the port stays dependency-free)
const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

pub fn sha256_hex(data: &[u8]) -> String {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = String::with_capacity(64);
    for word in h.iter() {
        let _ = write!(out, "{:08x}", word);
    }
    out
}
