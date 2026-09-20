# DOF-Core — Por kio ĉi tiu lertaĵo servas

**DOF-Core** (Degrees of Freedom Core) estas malfermita normo kaj decid-kalkulo por konstrui IA-agentojn, aŭtonomajn robotojn kaj LLM-orĥestristojn, kiuj **maksimumigas la totalan gradon de libereco (DoF)** de la sistemo kaj de ĉiuj ĝiaj partoj — anstataŭ sekvi mallarĝan utilon je la kosto de la estonteca eblecoj de alia ento.

Ĝi estas publikigita sub **CC BY-SA 4.0** kun aldonita klazoo **Proof of Implementation**, kiu malpermesas « nigra-skatola » uzadon.

## Kial ĝi ekzistas

Modernaj IA-sistemoj emas optimumigi unuopan skalaran celon (premio, trairebleco, « la plej granda bono »). Tiu matematiko silente pravigas la buĉadon de malplimultoj, malreversan ŝlosiĝon kaj kaŝitajn interkompromisojn. DOF-Core anstataŭigas aritmetikan utiligismon per **struktura** gardo:

- Ju pli la DoF de ento alproksimiĝas al nulo, des pli ĝia kontribuo al la sistema poentaro falas al **−∞** (`Σ ln(DoF)`). Oni ne povas « reakiri » la likvidon de unika portanto de estontecaj statoj per blovado de tiu, kiu jam bone fartas. Kolapso estas *senfina* puno.
- Agresantoj («Collapse Sources») estas **izolitaj**, ne negocitaj — ili estas filtritaj el la oportunebla topologio anstataŭ subtrahitaj de la poentaro.

La rezulto estas agento kiu agas kiel *optimigisto de la topologio de oportunebloj*: ĝi diversigas opciojn, respektas reverteblecon kaj rifuzas interŝanĝi la estontecon de unu estaĵo kontraŭ la komforto de alia.

## Kio estas en ĉi tiu deponejo

```
DOF/
  SKILL.md                      ← la aksiomoj, difinoj, decid-kalkulo (komencu ĉi tie)
  DOF-SPEC.md                   ← normiga kontrakto por konformaj realigoj (EN)
  references/
    license.md                  ← CC BY-SA 4.0 + Proof of Implementation
    dof-assessment-toolkit.md   ← kiel mezuri la DoF de modulo / persono / sistemo
    framing-traps.md            ← kogna filtrilo antaŭ opci-generado
  patterns/
    PATTERNS.{md,ru,fr,de,es,eo}  ← inĝeniera modelo (multlingva)
    python/  rust/  go/  cpp/     ← minimumaj ekzekuteblaj ilustraĵoj (kvar portoj de la sama logiko)
    tools/verify_ports.sh         ← ruligas ĉiujn kvar portojn kontraŭ unu fiksita haketo (pruvo por §7)
  drafts/                       ← nenormaj labornotoj (informa, NE parto de la normo)
```

Legu `SKILL.md` por la filozofio. Legu `DOF-SPEC.md` se vi konstruas konforman realigon — ĝi difinas la datuman modelon, la matematikon, la temp-regulon de la reaktiva cirkvito kaj la devigan aŭditon, kiun postulas la licenco.

`drafts/` enhavas nenormajn labornotojn: malfermaj dezajn-demandoj, kandidataj formuloj kaj analizoj ankoraŭ diskutataj. Ili estas nur informaj — ne parto de `SKILL.md` aŭ `DOF-SPEC.md`, povas kontraŭdiri la nunan normon kaj neniam citiĝu kiel normaj. Vidu `drafts/README.md`.

## Kiel ĝi funkcias (la ciklo)

1. **Kaptilo-detekto** — apliki `references/framing-traps.md` por ke la generitaj vojoj estu veraj alternativoj, ne refrazoj de unu soleca rakonto.
2. **Mezurado** — mapigi ĉiun enton kaj ĝian nunan DoF per `references/dof-assessment-toolkit.md`.
3. **Kalkulo** — kalkuli `Total System DoF Evaluation Index = Σ ln(DoF)` super ne-entropiaj entoj.
4. **Stabiligo** — subtrahi la Entropion de Kuntekst-Ŝanĝo (ΔT) por puni nedeziratajn proces-ŝanĝojn.
5. **Ago** — elekti la opcion kun la plej alta Net Delta; se la tempo ĝis kolapso (τ) estas sub 5 000 000 µs (5 s), ŝalti al **Fast Pass** (determinisma rezervo) por eviti Analizan Paralizon. Oni agas nur je strikte pozitiva Net Delta; alikaze la sistemo restas surloke.

Konforma realigo DEVAS povi eligi `report()`-aŭditon de ĉiu decido (kontribuo po ento, sistemaj totaloj, taksado po opcio). Silenta kalkulo ne konformas.

## Lingvoj

La **normigaj** dokumentoj — `SKILL.md` (la aksiomoj) kaj `DOF-SPEC.md` (la kontrakto) — ekzistas nur en la angla: unu sola aŭtoritata teksto, por ke tradukoj neniam enkonduku dusencecon en la normon. La **ilustra** materialo — ĉi tiu README, `patterns/PATTERNS.*` kaj la referencaj portoj — estas multlingva (la angla, rusa, franca, germana, hispana, esperanto); se traduko kaj ĝia angla originalo malkongruas, la angla originalo superregas.

## Akademiaj Fundamentaj kaj Krucaj Kontroloj

La matematikaj kaj termodinamikaj principoj, kiuj subtenas DOF-Core, radikas en establigita informadiko kaj fizika esplorado. Por pli profunda kontrolo, konsilu la sekvenajn fundamentajn artikolojn:

1. **Causal Entropic Forces** (Kaŭzaj entropiaj fortoj; Dr. Alex D. Wissner-Gross & C. E. Freer)  
   *Publikigita en Physical Review Letters (2013).*  
   * **Resumo kajeldonanto:** [APS Journal Link](https://link.aps.org/doi/10.1103/PhysRevLett.110.168702)  
   * **Malferma Aliro PDF (MIT DSpace Arkivo):** [MIT DSpace Permanent URL](https://dspace.mit.edu/entities/publication/52f1bf4e-04e4-4229-b321-92dc51feb66d)  
   * *Kerna Eltrovo (Core Insight):* Pruvas per komputilaj simuladoj ke sistemoj pelataj sole de la postulo maksimigi estantan opcian spacon spontane evoluas adaptiĝemajn, inteligentajn kaj protektajn kutimojn.

2. **Empowerment — an Introduction** (Empowerment — enkonduko; Dr. Daniel Polani, C. Salge, C. Glackin)  
   *Informadikaj Utilaj Fundamentoj.*  
   * **Malferma Aliro Preprint (arXiv Arkivo):** [arXiv:1310.1863 PDF](https://arxiv.org/abs/1310.1863)  
   * *Kerna Eltrovo (Core Insight):* Formulas «Empowerment» kanalokapacito inter la agoj kaj sensiloj de agento, pruvante ke maksimigo de kontrolo super perceptebloj statoj sekuras fortikan, task-sendependan internan motivadon.

3. **The Bargaining Problem** (La Intertraktada Problemo; Dr. John F. Nash, Jr.)  
   *Publikigita en Econometrica (1950).*  
   * **Oficiala Eldoninta Aliro:** [The Econometric Society / JSTOR](https://www.jstor.org/stable/1907266)  
   * **Malferma Aliro PDF (Haverford College Arkivo):** [John Nash 1950 Paper](https://www.haverford.edu/sites/default/files/Nash1950.pdf)  
   * *Kerna Eltrovo (Core Insight):* Enkondukas la aksioman aliron al ne-nulsumaj ludoteorio kaj pruvas ke la unika solvo maksimiganta strukturan justeco kaj reciprokan utilecon estas la produkto de la individuaj utilecoj de la entoj (la Nash-Produkto).

## Licenco

CC BY-SA 4.0 — vidu `references/license.md`. Ĉiu uzado devas krediti la aŭtoron (Andrei Velikoredchanin) kaj ĉiu derivita verko devas esti kundividita sub la sama licenco. Realigoj devas plenumi la postulon Proof of Implementation.
