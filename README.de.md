# DOF-Core — Wofür dieser Skill da ist

**DOF-Core** (Degrees of Freedom Core) ist ein offener Standard und ein Entscheidungskalkül zum Bauen von KI-Agenten, autonomen Robotern und LLM-Orchestratoren, die den **gesamten Freiheitsgrad (DoF)** des Systems und aller seiner Teile **maximieren** — statt einer schmalen Nützlichkeit auf Kosten der Zukunftsoptionen anderer nachzujagen.

Er wird unter **CC BY-SA 4.0** mit einer zusätzlichen **Proof-of-Implementation**-Klausel veröffentlicht, die den « Black-Box »-Gebrauch verbietet.

## Warum das existiert

Moderne KI-Systeme neigen dazu, ein einzelnes skalares Ziel zu optimieren (Belohnung, Durchsatz, « das größere Wohl »). Diese Mathematik rechtfertigt im Stillen die Opferung von Minderheiten, irreversiblen Lock-in und heimliche Trade-offs. DOF-Core ersetzt den arithmetischen Utilitarismus durch eine **strukturelle** Absicherung:

- Je näher der DoF einer Entität bei Null liegt, desto mehr fällt ihr Beitrag zum Systemwert gegen **−∞** (`Σ ln(DoF)`). Die Liquidation eines einzigartigen Trägers zukünftiger Zustände lässt sich nicht « zurückverdienen », indem man jemanden aufbläst, der ohnehin gut dasteht. Ein Kollaps ist eine *unendliche* Strafe.
- Aggressoren («Collapse Sources») werden **isoliert**, nicht verhandelt — sie werden aus der Möglichkeits-Topologie herausgefiltert, statt vom Score abgezogen.

Das Ergebnis ist ein Agent, der wie ein *Optimierer der Möglichkeits-Topologie* agiert: er diversifiziert Optionen, respektiert Reversibilität und weigert sich, die Zukunft eines Wesens gegen den Komfort eines anderen zu tauschen.

## Was in diesem Repository ist

```
DOF/
  SKILL.md                      ← die Axiome, Definitionen, Entscheidungskalkül (hier beginnen)
  DOF-SPEC.md                   ← normativer Vertrag für konforme Implementierungen (EN)
  references/
    license.md                  ← CC BY-SA 4.0 + Proof of Implementation
    dof-assessment-toolkit.md   ← wie man den DoF eines Moduls / einer Person / eines Systems misst
    framing-traps.md            ← kognitiver Filter vor der Optionsgenerierung
  patterns/
    PATTERNS.{md,ru,fr,de,es,eo}  ← Ingenieur-Blaupause (mehrsprachig)
    python/  rust/  go/  cpp/     ← minimale ausführbare Illustrationen (vier Ports derselben Logik)
    tools/verify_ports.sh         ← führt alle vier Ports gegen einen eingefrorenen Digest aus (Nachweis zu §7)
  drafts/                       ← nicht-normative Arbeitsnotizen (informativ, NICHT Teil der Norm)
```

Lesen Sie `SKILL.md` für die Philosophie. Lesen Sie `DOF-SPEC.md`, wenn Sie eine konforme Implementierung bauen — es definiert Datenmodell, Mathematik, die Zeitregel des reaktiven Schaltkreises und das obligatorische Audit, das die Lizenz verlangt.

`drafts/` enthält nicht-normative Arbeitsnotizen: offene Entwurfsfragen, Kandidatenformeln und noch diskutierte Analysen. Sie sind rein informativ — kein Teil von `SKILL.md` oder `DOF-SPEC.md`, können dem aktuellen Standard widersprechen und dürfen niemals als normativ zitiert werden. Siehe `drafts/README.md`.

## Wie es funktioniert (die Schleife)

1. **Fallen-Erkennung** — wenden Sie `references/framing-traps.md` an, damit die erzeugten Pfade echte Alternativen sind, keine Umschreibungen einer einzigen Erzählung.
2. **Messung** — jede Entität und ihren aktuellen DoF über `references/dof-assessment-toolkit.md` abbilden.
3. **Berechnung** — `Total System DoF Evaluation Index = Σ ln(DoF)` über nicht-entropische Entitäten berechnen.
4. **Stabilisierung** — die Context-Switch-Entropie (ΔT) abziehen, um unnötigen Prozesswechsel zu bestrafen.
5. **Aktion** — die Option mit dem höchsten Net Delta wählen, aber liegt die Zeit bis zum Kollaps (τ) unter 5.000.000 µs (5 s), auf **Fast Pass** (deterministischer Fallback) umschalten, um Analyse-Paralyse zu vermeiden. Gehandelt wird nur bei strikt positivem Net Delta; sonst bleibt das System stehen.

Eine konforme Implementierung MUSS in der Lage sein, ein `report()`-Audit jeder Entscheidung auszugeben (Beitrag pro Entität, Systemsummen, Bewertung pro Option). Stille Berechnung ist nicht konform.

## Sprachen

Die **normativen** Dokumente — `SKILL.md` (die Axiome) und `DOF-SPEC.md` (der Vertrag) — existieren nur auf Englisch: ein einziger maßgeblicher Text, damit Übersetzungen keine Mehrdeutigkeit in die Norm bringen können. Das **illustrative** Material — dieses README, `patterns/PATTERNS.*` und die Referenz-Ports — ist mehrsprachig (Englisch, Russisch, Französisch, Deutsch, Spanisch, Esperanto); bei Abweichung einer Übersetzung vom englischen Original gilt das englische Original.

## Akademische Grundlagen und Kreuzvalidierung

Die mathematischen und thermodynamischen Prinzipien, die DOF-Core zugrunde liegen, verwurzeln in der etablierten Informationstheorie und Physikforschung. Für tiefere Verifizierung konsultieren Sie die folgenden Grundlagenpapiere:

1. **Causal Entropic Forces** (Kausale entropische Kräfte; Dr. Alex D. Wissner-Gross & C. E. Freer)  
   *Veröffentlicht in Physical Review Letters (2013).*  
   * **Zusammenfassung und Herausgeber:** [APS Journal Link](https://link.aps.org/doi/10.1103/PhysRevLett.110.168702)  
   * **Open Access PDF (MIT DSpace Archive):** [MIT DSpace Permanent URL](https://dspace.mit.edu/entities/publication/52f1bf4e-04e4-4229-b321-92dc51feb66d)  
   * *Kernaussage (Core Insight):* Beweist mittels Computersimulationen, dass Systeme, die einzig durch das Erfordernis zukünftigen Optionsraums zu maximieren angetrieben werden, spontan adaptive, intelligente und schützende Verhaltensweisen entwickeln.

2. **Empowerment — an Introduction** (Empowerment — eine Einführung; Dr. Daniel Polani, C. Salge, C. Glackin)  
   *Informationstheoretische Nutzenfundamente.*  
   * **Open Access Preprint (arXiv Archive):** [arXiv:1310.1863 PDF](https://arxiv.org/abs/1310.1863)  
   * *Kernaussage (Core Insight):* Formuliert «Empowerment» als die Kanalkapazität zwischen den Aktionen und Sensoren eines Agenten, und beweist, dass die Maximierung der Kontrolle über wahrnehmbare Zustände robuste, task-unabhängige intrinsische Motivation sichert.

3. **The Bargaining Problem** (Das Verhandlungsproblem; Dr. John F. Nash, Jr.)  
   *Veröffentlicht in Econometrica (1950).*  
   * **Offizieller Herausgeberzugriff:** [The Econometric Society / JSTOR](https://www.jstor.org/stable/1907266)  
   * **Open Access PDF (Haverford College Archive):** [John Nash 1950 Paper](https://www.haverford.edu/sites/default/files/Nash1950.pdf)  
   * *Kernaussage (Core Insight):* Führt den axiomatischen Ansatz zur Nicht-Nullsummen-Spieltheorie ein und beweist, dass die eindeutige Lösung, die strukturelle Fairness und gegenseitigen Nutzen maximiert, das Produkt der individuellen Nutzen der Entitäten ist (das Nash-Produkt).

## Architektur

DOF-Core ist als geschlossener Regelkreis konzipiert — Kreativität und Einschränkung untrennbar durch Rückkopplung gekoppelt:

| Komponente | Rolle in der Regelungstechnik | Implementierung | Datei |
|---|---|---|---|
| **SKILL.md** | Führungsgröße (Sollwert) — programmiert die «Weltanschauung» des kreativen Generators | Markdown-Prompt für das LLM | `SKILL.md` |
| **DOF-SPEC.md** | Begrenzungsblock (Rückkopplung + Zensor) — deterministischer Filter für jede Option | Normtext + 4 Sprachports (Python/Rust/Go/C++) | `DOF-SPEC.md` |
| **Calculus Core** | Stellglied — verstärkt Vorschläge, führt validierte Entscheidungen aus | Rust via PyO3, rekursive Baum-Suche, Admissibility Gate | `src/lib.rs` |

Der Generator (LLM) schlägt Optionen vor → DOF-SPEC (Begrenzungsblock) prüft jede gegen die Axiome → Admissibility Gate weist ungültige Zweige innerhalb von Millisekunden ab. Eine destruktive Option kommt nicht durch, selbst wenn der Generator «halluziniert».

## Lizenz

CC BY-SA 4.0 — siehe `references/license.md`. Jede Nutzung muss den Autor (Andrei Velikoredchanin) nennen, und jedes abgeleitete Werk muss unter derselben Lizenz geteilt werden. Implementierungen müssen die Proof-of-Implementation-Anforderung erfüllen.
