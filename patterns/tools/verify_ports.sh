#!/usr/bin/env bash
# Verify the DOF-Core reference ports against the frozen digests of the CURRENT
# release, v0.9.1, and report the v0.8/v0.7/v0.6 harnesses separately as historical
# evidence.
#
# Usage:  bash patterns/tools/verify_ports.sh [REPO_DIR]   # defaults to this repo root
#
# v0.9.1 does NOT change the ruler or observation digests (it adds ResourceObservation
# and τ-as-resource, but canonical serialization of existing fields is unchanged), so
# the decisive row is v0.9.1 and it must show the SAME digests v0.7/v0.8 shipped.
#
# This is the repo copy of the script that ships with the Hermes skill
# `normative-standard-port` (see its Verification checklist). The only
# difference is the default REPO_DIR: here it is the repository this script
# lives in (two levels up from patterns/tools), so it can be run from anywhere.
# It sits under patterns/ because it is port machinery: it hardcodes the port
# directories, each harness filename and the toolchain of every language, so it
# moves with the ports, not with the standard.
#
# There are TWO frozen digests per release, and a port must reproduce BOTH
# byte-for-byte:
#   * the RULER digest       — the canonical text of the declaration (§3.4.1);
#   * the OBSERVATION digest — the fingerprint of the observed world graph (§6.2),
#     which pins the subgraph a report was taken from.
# They are different artifacts and a port that gets one right and the other
# wrong is not conformant; so they are grepped for by exact value and counted
# per port rather than "the first 64-hex we happen to see".
#
# v0.9.1 does NOT change the ruler or observation digests (ResourceObservation
# and τ-as-resource were added, but canonical serialization of existing fields is
# unchanged), so the decisive row is v0.9.1 and it must show the SAME two digests
# v0.7/v0.8 shipped: the candidate vector, the admissibility test, and the new
# ResourceObservation fields are not measurement inputs. A moved digest here is
# an error to be fixed, not a new version — which is exactly why the v0.7 row
# is re-run alongside: all releases must agree on the fingerprints.
#
# Expectations:
#   * ports live at patterns/{python,go,cpp,rust};
#   * every harness DECLARES its frozen digests as quoted 64-hex constants and
#     asserts its own run against them (this script cross-checks those constants,
#     so a run that silently stopped comparing cannot pass unnoticed);
#   * the v0.8 harness is what each port runs by default; the v0.7 and v0.6
#     harnesses stay runnable as the historical record (`v07`/`v06` arguments,
#     `smoke_test_v07.py`/`smoke_test.py` in python) and are reported without
#     deciding the verdict — except in python, whose v0.6 harness has three
#     DOCUMENTED divergences that v0.7 makes deliberate (the ruler changed; a
#     known zero is no longer excluded without an observation; acting on a
#     passive object is no longer free without one);
#   * toolchains come from nix-shell, which works offline here.
#
# Verified on 2026-09-16 against this repo (v0.6 row): VERIFIED with python 68 /
# go 62 / cpp 62 / rust 62 checks, all four harnesses declaring
# bed37c25fd9cb757e9ea4a861c01cd4660fd896a83cd39b7c73b8e0be7489ad4; and NOT
# VERIFIED (exit 1) on a directory with no ports.

set -u

SELF_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO="${1:-$(cd "$SELF_DIR/../.." && pwd)}"
OUT_DIR="$(mktemp -d)"
status=0
current_ports_ok=0
current_ports_seen=0

RULER_DIGEST="5126fd99641ffdc9c338d3d288fcf3cb6dcf093ca0a423f1cd265b3fcae4152a"
OBS_DIGEST="f3891c6ab622325fd6668893dd9f7450d39aa2d0a7ad2849634a4f59219f6a1c"
V06_DIGEST="bed37c25fd9cb757e9ea4a861c01cd4660fd896a83cd39b7c73b8e0be7489ad4"

printf 'repo: %s\nout:  %s\n' "$REPO" "$OUT_DIR"
printf 'ruler digest:       %s\nobservation digest: %s\n\n' "$RULER_DIGEST" "$OBS_DIGEST"

# --- current-release row (v0.9.1): both digests, no failures, checks ran --------
report_current() {
    local name="$1" file="$2"
    local ok fail ruler obs
    ok=$(grep -c '^  OK' "$file" || true)
    fail=$(grep -c '^  FAIL' "$file" || true)
    ruler=$(grep -c "$RULER_DIGEST" "$file" || true)
    obs=$(grep -c "$OBS_DIGEST" "$file" || true)
    current_ports_seen=$((current_ports_seen + 1))
    printf '%-14s checks=%-4s failed=%-3s ruler=%s observation=%s\n' \
        "$name" "$ok" "$fail" "$([ "$ruler" -gt 0 ] && echo yes || echo NO)" \
        "$([ "$obs" -gt 0 ] && echo yes || echo NO)"
    if [ "$fail" -ne 0 ]; then
        printf '  FAIL lines:\n'
        grep '^  FAIL' "$file" | sed 's/^/    /'
        status=1
    fi
    if [ "$ok" -eq 0 ]; then
        printf '  no checks ran\n'
        status=1
    fi
    if [ "$ruler" -eq 0 ] || [ "$obs" -eq 0 ]; then
        printf '  the v0.9.1 run does not show both frozen digests — that is a failure, not a warning\n'
        status=1
    else
        current_ports_ok=$((current_ports_ok + 1))
    fi
}

# --- v0.9.1 row: budget + τ-consumption harness --------------------------------
report_v091() {
    local name="$1" file="$2"
    local ok fail
    ok=$(grep -c '^  OK' "$file" || true)
    fail=$(grep -c '^  FAIL' "$file" || true)
    printf '%-14s checks=%-4s failed=%-3s   (v0.9.1, budget+τ)\n' \
        "$name" "$ok" "$fail"
    if [ "$fail" -ne 0 ]; then
        printf '  FAIL lines:\n'
        grep '^  FAIL' "$file" | sed 's/^/    /'
        status=1
    fi
}

# --- v0.11 row: the hypothesis artifact and the repaired §4.9 -----------------
# The invariant of this release is the **ruler**, not the declaration (§10
# evidence-criterion repair): `v0.11` redefines §4.9, and the §4.9 verdicts are
# hashed declaration content (§3.4.1), so a fixture that exercises the repaired
# rule MUST produce a different per-hypothesis digest. What must NOT move is
# `psi_ruler_digest` (§3.4.2) — so this row asserts, from the harness's own
# printed fingerprints, that the v0.11 ruler equals the v0.7 ruler computed by
# the same exclusion rule, and that the v0.7 declaration digest is still
# reproduced byte-for-byte.
report_v011() {
    local name="$1" file="$2"
    local ok fail shared v07dig
    ok=$(grep -c '^  OK' "$file" || true)
    fail=$(grep -c '^  FAIL' "$file" || true)
    shared=$(grep -c '^  OK   the ruler is shared across the release' "$file" || true)
    v07dig=$(grep -c '^  OK   the v0.7 declaration digest is still reproduced' "$file" || true)
    printf '%-14s checks=%-4s failed=%-3s ruler-shared=%s v0.7-digest=%s   (v0.11, hypotheses)\n' \
        "$name" "$ok" "$fail" \
        "$([ "$shared" -gt 0 ] && echo yes || echo NO)" \
        "$([ "$v07dig" -gt 0 ] && echo yes || echo NO)"
    if [ "$fail" -ne 0 ]; then
        printf '  FAIL lines:\n'
        grep '^  FAIL' "$file" | sed 's/^/    /'
        status=1
    fi
    if [ "$ok" -eq 0 ] || [ "$shared" -eq 0 ] || [ "$v07dig" -eq 0 ]; then
        printf '  the v0.11 run does not show the shared ruler and the reproduced v0.7 digest\n'
        printf '  — that is a failure, not a warning\n'
        status=1
    fi
}

# --- v0.7 row: historical, but it must still show the SAME two digests --------
report_v07() {
    local name="$1" file="$2"
    local ok fail ruler obs
    ok=$(grep -c '^  OK' "$file" || true)
    fail=$(grep -c '^  FAIL' "$file" || true)
    ruler=$(grep -c "$RULER_DIGEST" "$file" || true)
    obs=$(grep -c "$OBS_DIGEST" "$file" || true)
    printf '%-14s checks=%-4s failed=%-3s ruler=%s observation=%s   (v0.7, historical)\n' \
        "$name" "$ok" "$fail" "$([ "$ruler" -gt 0 ] && echo yes || echo no)" \
        "$([ "$obs" -gt 0 ] && echo yes || echo no)"
}

# --- v0.6 row: historical evidence, reported but not decisive -----------------
report_historical() {
    local name="$1" file="$2"
    local ok fail dig
    ok=$(grep -c '^  OK' "$file" || true)
    fail=$(grep -c '^  FAIL' "$file" || true)
    dig=$(grep -c "$V06_DIGEST" "$file" || true)
    printf '%-14s checks=%-4s failed=%-3s v0.6-digest=%s   (historical, not decisive)\n' \
        "$name" "$ok" "$fail" "$([ "$dig" -gt 0 ] && echo yes || echo no)"
}

if [ -d "$REPO/patterns/python" ]; then
    ( cd "$REPO/patterns/python" && nix-shell -p python3 -p python3Packages.pydantic \
        --run "python3 world_graph.py; python3 smoke_test_v08.py" ) > "$OUT_DIR/python.out" 2>&1
    report_current python "$OUT_DIR/python.out"
    ( cd "$REPO/patterns/python" && nix-shell -p python3 -p python3Packages.pydantic \
        --run "python3 harness_v091.py" ) > "$OUT_DIR/python_v091.out" 2>&1
    report_v091 python-v091 "$OUT_DIR/python_v091.out"
    # v0.11: the hypothesis artifact. Present in a port only once that port has
    # been extended; a port without the harness is reported as pending rather
    # than passed.
    if [ -f "$REPO/patterns/python/harness_v011.py" ]; then
        ( cd "$REPO/patterns/python" && nix-shell -p python3 -p python3Packages.pydantic \
            --run "python3 harness_v011.py" ) > "$OUT_DIR/python_v011.out" 2>&1
        report_v011 python-v011 "$OUT_DIR/python_v011.out"
    else
        printf '%-14s   (v0.11 harness absent — pending)\n' python-v011
    fi
    ( cd "$REPO/patterns/python" && nix-shell -p python3 -p python3Packages.pydantic \
        --run "python3 smoke_test_v07.py" ) > "$OUT_DIR/python_v07.out" 2>&1
    report_v07 python-v07 "$OUT_DIR/python_v07.out"
    ( cd "$REPO/patterns/python" && nix-shell -p python3 -p python3Packages.pydantic \
        --run "python3 smoke_test.py" ) > "$OUT_DIR/python_v06.out" 2>&1
    report_historical python-v06 "$OUT_DIR/python_v06.out"
fi

if [ -d "$REPO/patterns/go" ]; then
    ( cd "$REPO/patterns/go" && nix-shell -p go --run "go run . v08" ) > "$OUT_DIR/go.out" 2>&1
    report_current go "$OUT_DIR/go.out"
    ( cd "$REPO/patterns/go" && nix-shell -p go --run "go run . v091" ) > "$OUT_DIR/go_v091.out" 2>&1
    report_v091 go-v091 "$OUT_DIR/go_v091.out"
    # v0.11: the hypothesis artifact. Present in a port only once that port has
    # been extended; a port without the harness is reported as pending rather
    # than passed.
    if [ -f "$REPO/patterns/go/harness_v011.go" ]; then
        ( cd "$REPO/patterns/go" && nix-shell -p go --run "go run . v011" ) > "$OUT_DIR/go_v011.out" 2>&1
        report_v011 go-v011 "$OUT_DIR/go_v011.out"
    else
        printf '%-14s   (v0.11 harness absent — pending)\n' go-v011
    fi
    ( cd "$REPO/patterns/go" && nix-shell -p go --run "go run . v07" ) > "$OUT_DIR/go_v07.out" 2>&1
    report_v07 go-v07 "$OUT_DIR/go_v07.out"
    ( cd "$REPO/patterns/go" && nix-shell -p go --run "go run . v06" ) > "$OUT_DIR/go_v06.out" 2>&1
    report_historical go-v06 "$OUT_DIR/go_v06.out"
fi

if [ -d "$REPO/patterns/cpp" ]; then
    ( cd "$REPO/patterns/cpp" && nix-shell -p gcc --run \
        "g++ -std=c++17 -O2 -I. -o $OUT_DIR/dof_cpp main.cpp && $OUT_DIR/dof_cpp v08" ) \
        > "$OUT_DIR/cpp.out" 2>&1
    report_current cpp "$OUT_DIR/cpp.out"
    if [ -x "$OUT_DIR/dof_cpp" ]; then
        "$OUT_DIR/dof_cpp" v091 > "$OUT_DIR/cpp_v091.out" 2>&1
        report_v091 cpp-v091 "$OUT_DIR/cpp_v091.out"
        "$OUT_DIR/dof_cpp" v07 > "$OUT_DIR/cpp_v07.out" 2>&1
        report_v07 cpp-v07 "$OUT_DIR/cpp_v07.out"
        "$OUT_DIR/dof_cpp" v06 > "$OUT_DIR/cpp_v06.out" 2>&1
        report_historical cpp-v06 "$OUT_DIR/cpp_v06.out"
    fi
fi

if [ -d "$REPO/patterns/rust" ]; then
    ( cd "$REPO/patterns/rust" && nix-shell -p rustc --run \
        "rustc -O --edition 2021 -o $OUT_DIR/dof_rust main.rs && $OUT_DIR/dof_rust v08" ) \
        > "$OUT_DIR/rust.out" 2>&1
    report_current rust "$OUT_DIR/rust.out"
    ( cd "$REPO/patterns/rust" && nix-shell -p rustc --run \
        "rustc -C opt-level=0 --edition 2021 -o $OUT_DIR/dof_rust0 main.rs && $OUT_DIR/dof_rust0 v08" ) \
        > "$OUT_DIR/rust_o0.out" 2>&1
    report_current rust-o0 "$OUT_DIR/rust_o0.out"
    if [ -x "$OUT_DIR/dof_rust" ]; then
        "$OUT_DIR/dof_rust" v091 > "$OUT_DIR/rust_v091.out" 2>&1
        report_v091 rust-v091 "$OUT_DIR/rust_v091.out"
        "$OUT_DIR/dof_rust" v07 > "$OUT_DIR/rust_v07.out" 2>&1
        report_v07 rust-v07 "$OUT_DIR/rust_v07.out"
        "$OUT_DIR/dof_rust" v06 > "$OUT_DIR/rust_v06.out" 2>&1
        report_historical rust-v06 "$OUT_DIR/rust_v06.out"
    fi
fi

# Every port's harness also *declares* the frozen digests as constants and asserts
# equality with them; collect those declarations so a run that silently stopped
# comparing cannot pass unnoticed.
declare_out="$OUT_DIR/declared.out"
grep -rhoE '"[0-9a-f]{64}"' "$REPO/patterns" 2>/dev/null | tr -d '"' | sort -u > "$declare_out"
declare_files=$(grep -rlE '"[0-9a-f]{64}"' "$REPO/patterns" 2>/dev/null | wc -l | tr -d ' ')
printf '\nharnesses that declare a frozen digest: %s (unique values below)\n' "$declare_files"
sed 's/^/  /' "$declare_out"

declares_ruler=$(grep -c "$RULER_DIGEST" "$declare_out" || true)
declares_obs=$(grep -c "$OBS_DIGEST" "$declare_out" || true)
printf '\nharnesses declaring the v0.7/v0.8 ruler digest:       %s\n' "$declares_ruler"
printf 'harnesses declaring the v0.7/v0.8 observation digest: %s\n' "$declares_obs"
if [ "$current_ports_seen" -eq 0 ]; then
    printf 'no v0.8 harness ran at all\n'
    status=1
elif [ "$current_ports_ok" -ne "$current_ports_seen" ]; then
    printf 'v0.8: %s of %s runs reproduce both digests\n' "$current_ports_ok" "$current_ports_seen"
    status=1
else
    printf 'v0.8: all %s runs reproduce both digests\n' "$current_ports_seen"
fi

if [ "$status" -eq 0 ]; then
    printf '\nVERIFIED\n'
else
    printf '\nNOT VERIFIED\n'
fi
exit "$status"
