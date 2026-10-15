#!/usr/bin/env bash
#
# The pre-publication gate: everything a release must pass.
#
# Usage:
#   scripts/check.sh [--quick]
#
#   --quick   skip posum check on the witnesses, the longest step (minutes)
#
# Steps, each of which must pass:
#
#  1. formatting, lints and tests of the posum workspace;
#  2. lints and tests of the Rust verifier, tests of the Python verifier;
#  3. both verifiers, and the awk one, against the test vectors;
#  4. the claims that need no witness: the bands certificates for
#     n_min(k), k = 3..6, and for the lower bound n_min(7) >= 587761723,
#     computed by posum and accepted by both independent verifiers;
#  5. posum check on witness/: every bundle, both verifiers.

set -uo pipefail
cd "$(dirname "$0")/.."

quick=0
[ "${1:-}" = "--quick" ] && quick=1

failures=0
run() {  # description command...
    local description="$1"; shift
    printf '%-62s ' "$description"
    local log
    if log="$("$@" 2>&1)"; then
        echo "ok"
    else
        echo "FAILED"
        printf '%s\n' "$log" | tail -20 | sed 's/^/    /'
        failures=$((failures + 1))
    fi
}

run "posum: rustfmt" cargo fmt --check
run "posum: clippy, warnings denied" cargo clippy --release --locked --all-targets -- -D warnings
run "posum: tests" cargo test --release --locked
run "posum: build" cargo build --release --locked

verify="verify/rust/Cargo.toml"
run "Rust verifier: clippy, warnings denied" cargo clippy --release --locked --manifest-path "$verify" --all-targets -- -D warnings
run "Rust verifier: tests" cargo test --release --locked --manifest-path "$verify"
run "Python verifier: tests" python3 -m unittest discover -s verify/python
run "all verifiers against the test vectors" tests/run-vectors.sh

posum=target/release/posum
scratch="$(mktemp -d "${TMPDIR:-/tmp}/posum-gate.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT
bands() {  # k n: posum's certificate, accepted by both verifiers
    "$posum" bands "$1" "$2" >"$scratch/bands-$1.txt" &&
        verify/rust/target/release/posum-verify bands "$1" "$2" "$scratch/bands-$1.txt" &&
        python3 verify/python/posum_verify.py bands "$1" "$2" "$scratch/bands-$1.txt"
}
run "bands: n_min(3) = 473" bands 3 473
run "bands: n_min(4) = 9641" bands 4 9641
run "bands: n_min(5) = 246877" bands 5 246877
run "bands: n_min(6) = 15748961" bands 6 15748961
run "bands: n_min(7) >= 587761723" bands 7 587761723

if [ "$quick" -eq 0 ]; then
    run "posum check: every bundle, both verifiers" "$posum" check
fi

echo
if [ "$failures" -eq 0 ]; then
    echo "gate: all steps passed"
else
    echo "gate: $failures step(s) failed"
    exit 1
fi
