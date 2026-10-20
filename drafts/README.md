# drafts/ — non-normative working notes

**These files are NOT part of the DOF-Core standard.**

Everything in this directory is informative scratch work: open design questions, candidate
formulas, half-finished analyses and argument drafts. Statements here may contradict
`SKILL.md` or `DOF-SPEC.md`, may be superseded, and may never have been accepted at all.

Rules for this directory:

- It is **not** a normative reference. Nothing here can be cited to claim conformance.
- The only normative texts are `SKILL.md` (axioms, definitions, decision calculus) and
  `DOF-SPEC.md` (the contract for conforming implementations). Where a draft disagrees with
  either of them, the draft is wrong by construction.
- Content is kept here precisely because it was worth not losing: questions that are still
  open, alternatives that were considered, and reasoning that has not yet been turned into
  impersonal specification prose. When an item matures it moves into `DOF-SPEC.md` /
  `SKILL.md`, and this directory stops being its home.
- Style here is deliberately conversational and first-person; the standard is written in
  impersonal normative form. Do not copy wording from `drafts/` into the standard.

## Contents

| File | What it is |
|---|---|
| `measurement-normalization.md` | The measurement & normalization contract: the three lenses, the Variety formula `ψ_var = V/(V + V_env)`, the unknown-contribution defect (`dof_known = false`), the ignorance penalty `u(t)`, time budget and action gates (§8.6), and the open question list (A–I). |
| `real-idea-1.md` | DoF as a space of future possibilities: local estimation instead of world modeling, why ΔDoF matters more than the absolute value, and why the action space matters more than the state space. |
| `real-idea-2.md` | The three terms / the state-DoF vs action-DoF distinction, and why a single one of them is not enough. |
| `justification-article.md` | Plan and chapter-by-chapter sketch of the public article that justifies DOF from scratch: universal well-being as the goal, the Nash product as the only aggregation that refuses to pay for a loss, well-being identified with the number of real paths, then the consequences and where they are stricter than ordinary morality. Draft prose, not the article. |
| `justification-article-text.md` | Full text of the public article «Моральная теорема» (*The Moral Theorem*): preface plus 7 parts and 17 chapters, 223 lines and about 20.5k characters, one formula plate. Written from the plan above, opens with the Strugatsky line and its part-by-part accounting, marks the "growth over time" claim as the author's own extension rather than the standard's rule, and closes by reducing the whole argument to two input statements plus Nash's proven theorem. |
| `justification-article-text.en.md` | English version of the same article, "The Moral Theorem": paragraph-for-paragraph match with the Russian text (223 lines, the same 8 sections, 17 chapters and formula plate), held to the same typographic rules (no em dashes, straight quotes, ASCII spaces only). |

---

# drafts/ — ненормативные рабочие заметки

**Эти файлы НЕ являются частью стандарта DOF-Core.**

Всё в этом каталоге — informative-черновики: открытые вопросы проектирования, формулы-кандидаты,
незаконченные разборы и наброски аргументации. Утверждения здесь могут противоречить
`SKILL.md` или `DOF-SPEC.md`, могут быть устаревшими и могут вообще не быть принятыми.

Правила каталога:

- Это **не** нормативная ссылка. Ссылаться на него, чтобы обосновать соответствие стандарту, нельзя.
- Нормативны только `SKILL.md` (аксиомы, определения, исчисление решений) и `DOF-SPEC.md`
  (контракт для совместимых реализаций). Где черновик расходится с ними — черновик неверен по построению.
- Содержимое хранится здесь именно потому, что его не хотелось потерять: ещё открытые вопросы,
  рассмотренные альтернативы и рассуждения, не переведённые в безличную нормативную форму.
  Когда пункт дозревает, он переезжает в `DOF-SPEC.md` / `SKILL.md`, и этот каталог перестаёт быть его домом.
- Стиль здесь намеренно разговорный и от первого лица; стандарт пишется в безличной нормативной форме.
  Переносить формулировки из `drafts/` в стандарт дословно нельзя.