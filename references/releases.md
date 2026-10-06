# Released versions of `DOF-SPEC`

Every release of the standard is a git tag in this repository. The fingerprints below are the SHA-256 of `DOF-SPEC.md` **as released** — the value §10 of the specification asks to publish alongside a release, so that a silent modification of the file is detectable.

| Tag | Commit | Date (UTC+02:00) | SHA-256 of `DOF-SPEC.md` |
|---|---|---|---|
| `v0.11` | `9d66541` | 2026-10-06 02:46 | `4211950fc708cbc6404ad47c2cfbbddadd7d22f5ff3b5b2647a5f3859449c873` |
| `v0.9.1` | `6a03443` | 2026-09-18 07:00 | `8e1d6e00022b79f41cd9d0787d804b5d43ac0325dd581368f5e1fbde13683897` |
| `v0.8` | `c7b5f06` | 2026-09-16 15:10 | `6b1948ca7dc32aaa32ae1729f45570ed8f5103d59afda9baaeddd99cf611d0de` |
| `v0.7` | `c9dbbe8` | 2026-09-16 12:44 | `8c8c4580452dac93d35af73c877c485a61d3e8677d20ead836c4bbdc2e042741` |

## Verifying a copy

```sh
git checkout v0.11
sha256sum DOF-SPEC.md                   # must equal the value in the table above
bash patterns/tools/verify_ports.sh     # → VERIFIED
```

The digest covers `DOF-SPEC.md` alone, not the whole tree: the normative text is the artifact a release claims, and the reference ports are checked by **running** them rather than by hashing them. `verify_ports.sh` runs all four ports against the frozen ruler digest and reports `VERIFIED` for the current release together with the historical rows; the `v0.11` rows report Python 112 checks, Go 83, Rust 83 and C++ 84, none failing.

Because a release is tagged, editing `DOF-SPEC.md` after it is published changes the file and the value in the table stops matching it. That is the point of publishing the digest — and the reason a documentation change after a tag belongs either to the next version or to the files that are not the normative text.

## Two versions that were never released

`v0.9` and `v0.10` have **no tag and no file of their own**: both are transitional steps of the path to `v0.11`, and both were superseded in form before they were ever carried into the reference ports.

- **`v0.9` — budget accounting: observable resources.** Prepared on 17 September 2026 (commit `c2adc11`). It introduced `ResourceObservation` with its metadata, `requires` and `discovers` on `ActionOption`, the `measure` act type, the null-resource branch of the resource gate and the matching audit fields — and it kept τ a **rigid** deadline. Its log entry was folded into `v0.9.1` the same night (commit `bc36da5`, tagged 18 September 07:00 at `6a03443`), which added τ as a **consumable** resource. No `v0.9` tag exists and no port ships a `v0.9` row; the release's rules are asserted by the `v0.9.1` rows of all four ports, which exercise the resource layer and the τ-budget layer together.
- **`v0.10` — conditional evaluation under declared hypotheses.** Its hypothesis artifact was superseded in the same cycle by `v0.11`, where a hypothesis is a **complete alternative state** rather than a set of DoF assignments read against a shared ruler: assigning the product directly contradicts the lens identity of §4.1, so the `v0.10` form had no derivation the standard could give. There is no `v0.10` tag and no `v0.10` harness in any port; the release's surviving claims — the singleton reduction, the reading that refuses an option its mirror selects, the empty robust set — are asserted by the `v0.11` fixtures.

The normative record of what each release changed, including how `v0.11` treated these two transitional steps, is §10 of `DOF-SPEC.md`. This file is **informative**: it repeats fingerprints and history for convenience and never overrides the specification.
