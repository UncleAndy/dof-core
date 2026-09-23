# DOF-Core — What This Skill Is For

**DOF-Core** (Degrees of Freedom Core) is an open standard and decision-calculus for
building AI agents, autonomous robots, and LLM orchestrators that **maximize the total degrees of freedom** of the system and all its parts — rather than chasing narrow utility at the cost of someone's future options.

It is published under **CC BY-SA 4.0** with an added **Proof of Implementation** clause that forbids "black-box" use.

## Why this exists

Modern AI systems tend to optimize a single scalar objective (reward, throughput, "the greater good"). That math quietly licenses sacrificing minorities, irreversible lock-in, and silent trade-offs. DOF-Core replaces arithmetic utilitarianism with a
**structural** safeguard:

- As an entity's DoF approaches zero, its contribution to the system score goes to **−∞** (`Σ ln(DoF)`) — an *infinite* penalty. You cannot "earn back" the liquidation of a unique future-state carrier by inflating someone already well-off.
- Aggressors ("Collapse Sources") are **isolated**, not negotiated with — they are filtered out of the opportunity topology instead of being subtracted from the score.

The result is an agent that behaves like an *optimizer of opportunity topology*: it diversifies options, respects reversibility, and refuses to trade one being's future for another's comfort.

## What's in this repository

```
DOF/
  SKILL.md                      ← the axioms, definitions, decision calculus (start here)
  DOF-SPEC.md                   ← normative contract for conforming implementations (EN)
  references/
    license.md                  ← CC BY-SA 4.0 + Proof of Implementation
    dof-assessment-toolkit.md   ← how to measure DoF of a module / person / system
    framing-traps.md            ← cognitive filter applied before generating options
  patterns/
    PATTERNS.{md,ru,fr,de,es,eo}  ← engineering blueprint (multilingual)
    python/  rust/  go/  cpp/     ← minimal runnable illustrations (four ports of the same logic)
    tools/verify_ports.sh         ← runs all four ports against one frozen digest (§7 conformance evidence)
  drafts/                       ← non-normative working notes (informative, NOT part of the standard)
```

Read `SKILL.md` for the philosophy. Read `DOF-SPEC.md` if you are building a conforming implementation — it defines the data model, math, reactive-circuit timing, and the mandatory audit output that the license requires.

`drafts/` holds non-normative working notes: open design questions, candidate formulas and analyses still under discussion. They are informative only — they are not part of `SKILL.md` or `DOF-SPEC.md`, may contradict the current standard, and must not be cited as normative. See `drafts/README.md`.

## How it works (the loop)

1. **Trap Detection** — apply `references/framing-traps.md` so generated paths are genuine alternatives, not rephrasings of one narrative.
2. **Measurement** — map every entity and its current DoF via `references/dof-assessment-toolkit.md`.
3. **Calculation** — compute the `Total System DoF Evaluation Index = Σ ln(DoF)` over the calculation set `calc`: non-collapse-source entities whose DoF is positive, revivable, or unknown (unknown DoF is never treated as zero).
4. **Stabilization** — subtract the Context-Switch Entropy (ΔT) to penalize needless process switching.
5. **Action** — pick the option with the highest Net Delta, but if time-to-collapse (τ) is under 5,000,000 µs (5 s), switch to **Fast Pass** (deterministic fallback) to avoid analysis paralysis. The system acts only on a strictly positive Net Delta; otherwise it stays put.

A conforming implementation MUST be able to emit a `report()` audit of every decision (per-entity contribution, overall totals, per-option evaluation). Silent calculation is non-conforming.

## Languages

The **normative** documents — `SKILL.md` (the axioms) and `DOF-SPEC.md` (the contract) — exist in English only: a single authoritative text, so that translations can never introduce ambiguity into the standard. The **illustrative** material — this README, `patterns/PATTERNS.*`, and the reference ports — is multilingual (English, Russian, French, German, Spanish, Esperanto); where a translation and its English original disagree, the English original prevails.

## Academic Foundations & Cross-Verification

The mathematical and thermodynamic principles underlying DOF-Core are rooted in established information theory and physics research. For deeper verification, consult the following foundational papers:

1. **Causal Entropic Forces** (Dr. Alex D. Wissner-Gross & C. E. Freer)  
   *Published in Physical Review Letters (2013).*  
   * **Abstract & Publisher:** [APS Journal Link](https://link.aps.org/doi/10.1103/PhysRevLett.110.168702)  
   * **Open Access PDF (MIT DSpace Archive):** [MIT DSpace Permanent URL](https://dspace.mit.edu/entities/publication/52f1bf4e-04e4-4229-b321-92dc51feb66d)  
   * *Core Insight:* Proves via computer simulations that systems driven solely by the requirement to maximize future option space spontaneously develop adaptive, intelligent, and protective behaviors.

2. **Empowerment — an Introduction** (Dr. Daniel Polani, C. Salge, C. Glackin)  
   *Information-Theoretic Utility Foundations.*  
   * **Open Access Preprint (arXiv Archive):** [arXiv:1310.1863 PDF](https://arxiv.org/abs/1310.1863)  
   * *Core Insight:* Formulates "Empowerment" as the channel capacity between an agent's actions and sensors, proving that maximizing control over perceivable states ensures robust, non-task-dependent intrinsic motivation.

3. **The Bargaining Problem** (Dr. John F. Nash, Jr.)  
   *Published in Econometrica (1950).*  
   * **Official Publisher Access:** [The Econometric Society / JSTOR](https://www.jstor.org/stable/1907266)  
   * **Open Access PDF (Haverford College Archive):** [John Nash 1950 Paper](https://www.haverford.edu/sites/default/files/Nash1950.pdf)  
   * *Core Insight:* Introduces the axiomatic approach to non-zero-sum game theory and proves that the unique solution maximizing structural fairness and mutual utility is the product of the entities' individual utilities (the Nash Product).

## Architecture

DOF-Core is built as a closed-loop control system — creativity and constraint inseparably coupled through feedback:

| Component | Role in control theory | Implementation | File |
|---|---|---|---|
| **SKILL.md** | Setpoint (reference input) — programs the "worldview" of the creative generator | Markdown prompt for the LLM | `SKILL.md` |
| **DOF-SPEC.md** | Constraint block (feedback + censor) — deterministic filter that validates every generated option | Normative text + four language ports (Python/Rust/Go/C++) | `DOF-SPEC.md` |
| **Calculus Core** | Actuator — amplifies proposals, executes validated decisions, runs tree-search | Rust via PyO3, recursive lookahead, Admissibility Gate | `src/lib.rs` |

The generator (LLM) proposes options → DOF-SPEC (the constraint block) verifies each against the axioms → the Admissibility Gate rejects invalid branches within milliseconds of execution. A destructive option cannot pass through even if the generator hallucinates it.

## License

CC BY-SA 4.0 — see `references/license.md`. Any use must attribute the author (Andrei Velikoredchanin) and any derivative work must be shared under the same license. Implementations must satisfy the Proof of Implementation requirement.
