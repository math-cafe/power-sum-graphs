#!/usr/bin/env bash
#
# Regenerate every witness of this release from scratch, with posum and LKH.
#
# Usage:
#   scripts/regenerate.sh [--root DIR] [--runs DIR] [--jobs J] [--lkh PATH] [--no-k6]
#
#   --root DIR   witness root to write (default: witness-regenerated); the
#                published witness/ is never touched
#   --runs DIR   run directories (default: runs)
#   --jobs J     searches at once in the island sweeps (default: all cores)
#   --lkh PATH   LKH binary with its LKH.toml (default: lkh/bin/LKH, built by
#                scripts/build-lkh.sh if missing)
#   --no-k6      skip G_6(15748961), which needs about 6 GB and 7 minutes
#
# The script builds posum and the Rust verifier, builds LKH if needed, runs the
# island sweeps and the single searches, files every witness into its bundle,
# derives the plain tree, runs posum check on the result, and compares the
# identities found with the published ones.
#
# It can be interrupted and run again: sweeps and searches resume where they
# stopped. On the development machine (16 cores) the whole run takes about
# 1 h 20, nearly all of it the k = 5 island.
#
# Identical identities are not expected everywhere: scripts/build-lkh.sh builds
# with -march=native, so another processor gives another LKH binary, which may
# find other cycles. Every cycle found is checked all the same; the claims rest
# on verification, not on reproducing the same bytes.

set -euo pipefail

root="witness-regenerated"
runs="runs"
jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)"
lkh="lkh/bin/LKH"
with_k6=1

while [ $# -gt 0 ]; do
    case "$1" in
        --root) root="$2"; shift 2 ;;
        --runs) runs="$2"; shift 2 ;;
        --jobs) jobs="$2"; shift 2 ;;
        --lkh) lkh="$2"; shift 2 ;;
        --no-k6) with_k6=0; shift ;;
        -h|--help) sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "regenerate: unknown option $1" >&2; exit 2 ;;
    esac
done

cd "$(dirname "$0")/.."
if [ "$root" = "witness" ]; then
    echo "regenerate: refusing to write into the published witness/; choose another --root" >&2
    exit 2
fi

step() { printf '\n== %s\n' "$*"; }

step "building posum and the Rust verifier"
cargo build --release --locked --quiet
cargo build --release --locked --quiet --manifest-path verify/rust/Cargo.toml
posum="$PWD/target/release/posum"

if [ ! -x "$lkh" ]; then
    step "building LKH with scripts/build-lkh.sh"
    scripts/build-lkh.sh --prefix "$(dirname "$lkh")"
fi
export POSUM_LKH="$lkh"

# Every n with a value bundle; n_min(k) for k = 2..6, then the other values.
singles=("2 32" "3 473" "4 9641" "5 246877" "5 490463" "5 600000" "5 2000000")
[ "$with_k6" -eq 1 ] && singles+=("6 15748961")

step "sweeping the finite islands of k = 4 and k = 5"
for k in 4 5; do
    "$posum" sweep "$k" island --jobs "$jobs" --run-root "$runs"
done

step "single searches"
for kn in "${singles[@]}"; do
    read -r k n <<<"$kn"
    dir="$runs/k$k-n$n"
    if compgen -G "$dir/*.provenance.toml" >/dev/null; then
        echo "G_$k($n): already found"
    elif [ -f "$dir/run.toml" ]; then
        "$posum" search --resume "$dir" >/dev/null
    else
        "$posum" search "$k" "$n" --run-dir "$dir" >/dev/null
    fi
done

step "filing the witnesses into $root/"
island_runs=()
for k in 4 5; do
    while read -r inf sup; do
        for ((n = inf; n <= sup; n++)); do island_runs+=("$runs/k$k-n$n"); done
    done < <("$posum" islands "$k" | grep -v '\.\.\.' | tr -d '-' | awk '{print $1, $2}')
done
"$posum" bundle add --island --root "$root" "${island_runs[@]}" | tail -1
value_runs=()
for kn in "${singles[@]}"; do
    read -r k n <<<"$kn"
    value_runs+=("$runs/k$k-n$n")
done
"$posum" bundle add --root "$root" "${value_runs[@]}"

step "deriving the plain tree"
"$posum" bundle plain --root "$root" --jobs "$jobs"

step "checking $root/ with both verifiers"
"$posum" check --root "$root" --jobs "$jobs"

step "comparing with the published witnesses"
same=0; different=0
while IFS= read -r manifest; do
    relative="${manifest#"$root"/}"
    published="witness/$relative"
    if [ -f "$published" ] && cmp -s "$manifest" "$published"; then
        same=$((same + 1))
    else
        different=$((different + 1))
        echo "differs from the published bundle: ${relative%/manifest.toml}"
    fi
done < <(find "$root/bases" -name manifest.toml | sort)
echo "$same bundles identical to the published ones, $different different"
echo "regenerate: done; the regenerated witnesses are in $root/, all checked"
