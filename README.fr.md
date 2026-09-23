# DOF-Core — À quoi sert cette compétence

**DOF-Core** (Degrees of Freedom Core) est une norme ouverte et un calcul décisionnel pour construire des agents IA, des robots autonomes et des orchestrateurs LLM qui **maximisent le degré de liberté (DoF) total** du système et de toutes ses parties — au lieu de poursuivre une utilité étroite au détriment des options futures de quelqu'un d'autre.

Elle est publiée sous **CC BY-SA 4.0** avec une clause supplémentaire **Proof of Implementation** qui interdit l'usage « boîte noire ».

## Pourquoi cela existe

Les systèmes d'IA modernes tendent à optimiser un objectif scalaire unique (récompense, débit, « le plus grand bien »). Cette mathématique justifie en silence le sacrifice des minorités, le verrouillage irréversible et les compromis cachés. DOF-Core remplace l'utilitarisme arithmétique par une protection **structurelle** :

- Au fur et à mesure que le DoF d'une entité tend vers zéro, sa contribution au score système chute vers **−∞** (`Σ ln(DoF)`). On ne peut « récupérer » la liquidation d'un porteur unique d'états futurs en gonflant celui qui est déjà bien loti. Un effondrement est une pénalité *infinie*.
- Les agresseurs («Collapse Sources») sont **isolés**, pas négociés — ils sont filtrés de la topologie d'opportunité au lieu d'être soustraits du score.

Le résultat est un agent qui se comporte comme un *optimiseur de topologie d'opportunités* : il diversifie les options, respecte la réversibilité et refuse d'échanger l'avenir d'un être contre le confort d'un autre.

## Ce qu'il y a dans ce dépôt

```
DOF/
  SKILL.md                      ← les axiomes, définitions, calcul décisionnel (commencez ici)
  DOF-SPEC.md                   ← contrat normatif pour les implémentations conformes (EN)
  references/
    license.md                  ← CC BY-SA 4.0 + Proof of Implementation
    dof-assessment-toolkit.md   ← comment mesurer le DoF d'un module / d'une personne / d'un système
    framing-traps.md            ← filtre cognitif appliqué avant de générer des options
  patterns/
    PATTERNS.{md,ru,fr,de,es,eo}  ← plan directeur d'ingénierie (multilingue)
    python/  rust/  go/  cpp/     ← illustrations minimales exécutables (quatre ports de la même logique)
    tools/verify_ports.sh         ← exécute les quatre ports contre une empreinte figée (preuve du §7)
  drafts/                       ← notes de travail non normatives (informative, PAS le standard)
```

Lisez `SKILL.md` pour la philosophie. Lisez `DOF-SPEC.md` si vous construisez une implémentation conforme — il définit le modèle de données, les mathématiques, la temporisation du circuit réactif et l'audit obligatoire exigé par la licence.

`drafts/` contient des notes de travail non normatives : questions de conception ouvertes, formules candidates et analyses encore en discussion. Elles sont purement informatives — elles ne font pas partie de `SKILL.md` ni de `DOF-SPEC.md`, peuvent contredire le standard actuel et ne doivent jamais être citées comme normatives. Voir `drafts/README.md`.

## Comment cela fonctionne (la boucle)

1. **Détection de pièges** — appliquer `references/framing-traps.md` pour que les chemins générés soient de vraies alternatives, pas des paraphrases d'un seul récit.
2. **Mesure** — cartographier chaque entité et son DoF actuel via `references/dof-assessment-toolkit.md`.
3. **Calcul** — calculer `Total System DoF Evaluation Index = Σ ln(DoF)` sur les entités non-entropiques.
4. **Stabilisation** — soustraire l'Entropie de changement de contexte (ΔT) pour pénaliser les changements de processus superflus.
5. **Action** — choisir l'option au Net Delta le plus élevé, mais si le temps avant effondrement (τ) est sous 5 000 000 µs (5 s), basculer en **Fast Pass** (repli déterministe) pour éviter la paralysie par l'analyse. On n'agit que sur un Net Delta strictement positif ; sinon le système reste sur place.

Une implémentation conforme DOIT pouvoir émettre un audit `report()` de chaque décision (contribution par entité, totaux système, évaluation par option). Un calcul silencieux n'est pas conforme.

## Langues

Les documents **normatifs** — `SKILL.md` (les axiomes) et `DOF-SPEC.md` (le contrat) — n'existent qu'en anglais : un seul texte faisant autorité, afin que les traductions ne puissent introduire d'ambiguïté dans le standard. Le matériel **illustratif** — ce README, `patterns/PATTERNS.*` et les ports de référence — est multilingue (anglais, russe, français, allemand, espagnol, espéranto) ; en cas de divergence entre une traduction et son original anglais, l'original anglais prévaut.

## Fondements Académiques et Vérification Croisée

Les principes mathématiques et thermodynamiques sous-jacents à DOF-Core sont ancrés dans la théorie de l'information et la recherche en physique établies. Pour une vérification approfondie, consultez les articles fondamentaux suivants :

1. **Causal Entropic Forces** (Forces entropiques causales ; Dr. Alex D. Wissner-Gross & C. E. Freer)  
   *Publié dans Physical Review Letters (2013).*  
   * **Résumé et éditeur :** [APS Journal Link](https://link.aps.org/doi/10.1103/PhysRevLett.110.168702)  
   * **PDF Open Access (Archive MIT DSpace) :** [MIT DSpace Permanent URL](https://dspace.mit.edu/entities/publication/52f1bf4e-04e4-4229-b321-92dc51feb66d)  
   * *Aperçu fondamental (Core Insight) :* Démontre par des simulations informatiques que des systèmes guidés uniquement par l'exigence de maximiser l'espace d'option futur développent spontanément des comportements adaptatifs, intelligents et protecteurs.

2. **Empowerment — an Introduction** (Empowerment — une introduction ; Dr. Daniel Polani, C. Salge, C. Glackin)  
   *Fondements de l'utilité issus de la théorie de l'information.*  
   * **Préimpression Open Access (Archive arXiv) :** [arXiv:1310.1863 PDF](https://arxiv.org/abs/1310.1863)  
   * *Aperçu fondamental (Core Insight) :* Formule l'« Empowerment » comme la capacité de canal entre les actions et les capteurs d'un agent, démontrant que la maximisation du contrôle sur les états perceptibles assure une motivation intrinsèque robuste et indépendante de la tâche.

3. **The Bargaining Problem** (Le problème de la négociation ; Dr. John F. Nash, Jr.)  
   *Publié dans Econometrica (1950).*  
   * **Accès officiel chez l'éditeur :** [The Econometric Society / JSTOR](https://www.jstor.org/stable/1907266)  
   * **PDF Open Access (Archive Haverford College) :** [John Nash 1950 Paper](https://www.haverford.edu/sites/default/files/Nash1950.pdf)  
   * *Aperçu fondamental (Core Insight) :* Introduit l'approche axiomatique de la théorie des jeux à somme non nulle et démontre que l'unique solution maximisant l'équité structurelle et l'utilité mutuelle est le produit des utilités individuelles des entités (le produit de Nash).

## Architecture

DOF-Core est conçu comme un système de commande en boucle fermée — créativité et contrainte inextricablement couplées par rétroaction :

| Composant | Rôle en théorie de commande | Implémentation | Fichier |
|---|---|---|---|
| **SKILL.md** | Consigne (valeur de référence) — programme la « vision du monde » du générateur créatif | Prompt Markdown pour le LLM | `SKILL.md` |
| **DOF-SPEC.md** | Bloc de contraintes (rétroaction + censeur) — filtre déterministe validant chaque option | Texte normatif + 4 ports linguistiques (Python/Rust/Go/C++) | `DOF-SPEC.md` |
| **Calculus Core** | Actionneur — amplifie les propositions, exécute les décisions validées | Rust via PyO3, recherche arborescente récursive, Admissibility Gate | `src/lib.rs` |

Le générateur (LLM) propose des options → DOF-SPEC (bloc de contraintes) vérifie chacune contre les axiomes → l'Admissibility Gate rejette les branches invalides en quelques millisecondes. Une option destructrice ne peut pas passer, même si le générateur hallucine.

## Licence

CC BY-SA 4.0 — voir `references/license.md`. Toute utilisation doit créditer l'auteur (Andrei Velikoredchanin) et toute œuvre dérivée doit être partagée sous la même licence. Les implémentations doivent satisfaire l'exigence Proof of Implementation.
