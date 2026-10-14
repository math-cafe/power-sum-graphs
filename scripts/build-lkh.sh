#!/usr/bin/env bash
#
# Download, check, configure and build LKH-3, the external solver posum uses.
#
# Usage:
#   scripts/build-lkh.sh [--prefix DIR] [--two-level] [--keep-build]
#
#   --prefix DIR   install the binary as DIR/LKH (default: ./lkh/bin)
#   --two-level    keep LKH's default TWO_LEVEL_TREE instead of THREE_LEVEL_TREE
#   --keep-build   keep the build directory instead of removing it
#
# LKH is not part of this repository and is not redistributed by it. Its
# author, Keld Helsgaun, distributes it "for academic and non-commercial use"
# and reserves all rights; this script only fetches the official archive from
# his page, checks it against the SHA-256 the results were obtained with, and
# builds it on your machine.
#
# The one functional change is TREE_TYPE = THREE_LEVEL_TREE in SRC/Makefile: a
# three-level tour list, made 3.4 times faster on G_6(15748961), 15.7 million
# nodes, in the measurements behind these results. Compiler flags are LKH's
# own, -O3 -march=native: LTO, PGO and clang were measured within noise. With
# -march=native the binary suits the machine that builds it, and its SHA-256
# differs from one machine to another; posum records it in every provenance.
# The build directory is mapped to a fixed name, so that rebuilding on the same
# machine gives the same binary.

set -euo pipefail

VERSION="3.0.14"
URL="http://webhotel4.ruc.dk/~keld/research/LKH-3/LKH-${VERSION}.tgz"
SHA256="a9a79b5b8632a843d42ed93ab77fa10c5237aca16d5c1bf31287e214e931f3ca"

prefix="./lkh/bin"
tree="THREE_LEVEL_TREE"
keep=0

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix) prefix="$2"; shift 2 ;;
        --two-level) tree="TWO_LEVEL_TREE"; shift ;;
        --keep-build) keep=1; shift ;;
        -h|--help) sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "build-lkh: unknown option $1" >&2; exit 2 ;;
    esac
done

for tool in make cc tar; do
    command -v "$tool" >/dev/null || { echo "build-lkh: $tool is required" >&2; exit 1; }
done

if command -v sha256sum >/dev/null; then
    sha() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null; then
    sha() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
    echo "build-lkh: sha256sum or shasum is required" >&2; exit 1
fi

if command -v curl >/dev/null; then
    fetch() { curl -fsSL --retry 3 -o "$2" "$1"; }
elif command -v wget >/dev/null; then
    fetch() { wget -q -O "$2" "$1"; }
else
    echo "build-lkh: curl or wget is required" >&2; exit 1
fi

build="$(mktemp -d "${TMPDIR:-/tmp}/build-lkh.XXXXXX")"
cleanup() { [ "$keep" -eq 1 ] && echo "build-lkh: build kept in $build" || rm -rf "$build"; }
trap cleanup EXIT

echo "build-lkh: downloading LKH-${VERSION} from the author's page"
fetch "$URL" "$build/LKH-${VERSION}.tgz"

actual="$(sha "$build/LKH-${VERSION}.tgz")"
if [ "$actual" != "$SHA256" ]; then
    echo "build-lkh: SHA-256 mismatch, refusing to build" >&2
    echo "  expected: $SHA256" >&2
    echo "  found:    $actual" >&2
    exit 1
fi
echo "build-lkh: archive SHA-256 verified"

tar -xzf "$build/LKH-${VERSION}.tgz" -C "$build"
src="$build/LKH-${VERSION}"

# The only change: the tour representation.
makefile="$src/SRC/Makefile"
grep -q '^TREE_TYPE = TWO_LEVEL_TREE$' "$makefile" \
    || { echo "build-lkh: unexpected SRC/Makefile, refusing to patch" >&2; exit 1; }
sed -i.orig "s/^TREE_TYPE = TWO_LEVEL_TREE\$/TREE_TYPE = ${tree}/" "$makefile"
echo "build-lkh: TREE_TYPE = ${tree}"

# Reproducible builds: LKH compiles with -g, which writes the build directory,
# a random temporary path, into the binary. Mapping it to a fixed name makes
# two builds on the same machine byte-identical. Behaviour is unaffected.
if cc -ffile-prefix-map=a=b -x c -c -o /dev/null /dev/null 2>/dev/null; then
    sed -i "s|^CFLAGS = \(.*\)\$|CFLAGS = \1 -ffile-prefix-map=${src}=/LKH-${VERSION}|" "$makefile"
    echo "build-lkh: build path mapped to /LKH-${VERSION}, for a reproducible binary"
fi

jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)"
make -C "$src" -j"$jobs" >"$build/make.log" 2>&1 \
    || { echo "build-lkh: compilation failed, see $build/make.log" >&2; keep=1; exit 1; }

# Smoke test: G_2(32) has exactly one Hamiltonian cycle; LKH must find a tour
# of cost 0 through its 46 edges.
test_dir="$build/test"
mkdir -p "$test_dir"
{
    echo "NAME : G2_32"
    echo "TYPE : HCP"
    echo "DIMENSION : 32"
    echo "EDGE_DATA_FORMAT : EDGE_LIST"
    echo "EDGE_DATA_SECTION"
    for a in $(seq 1 32); do
        for b in $(seq $((a + 1)) 32); do
            s=$((a + b)); r=1
            while [ $((r * r)) -lt "$s" ]; do r=$((r + 1)); done
            [ $((r * r)) -eq "$s" ] && echo "$a $b"
        done
    done
    echo "-1"
    echo "EOF"
} >"$test_dir/g.hcp"
printf 'PROBLEM_FILE = g.hcp\nTOUR_FILE = g.tour\nSUBGRADIENT = NO\nOPTIMUM = 0\nSTOP_AT_OPTIMUM = YES\nRUNS = 1\nTRACE_LEVEL = 0\n' >"$test_dir/g.par"
( cd "$test_dir" && "$src/LKH" g.par >/dev/null 2>&1 ) || true
if ! grep -q 'Length = 0' "$test_dir/g.tour" 2>/dev/null; then
    echo "build-lkh: smoke test failed: no Hamiltonian cycle of G_2(32) found" >&2
    keep=1; exit 1
fi
echo "build-lkh: smoke test passed, the unique Hamiltonian cycle of G_2(32) found"

mkdir -p "$prefix"
install -m 755 "$src/LKH" "$prefix/LKH"
echo "build-lkh: installed $prefix/LKH"
echo "build-lkh: binary SHA-256 $(sha "$prefix/LKH")"
echo "build-lkh: LKH is (c) Keld Helsgaun, for academic and non-commercial use only."
