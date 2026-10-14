#!/bin/sh
# Run every verifier against the test vectors and compare its exit code with
# the verdict of tests/vectors/index.tsv: valid is 0, invalid is 1.
#
#   tests/run-vectors.sh
#
# The Rust verifier is built if needed. The awk verifier is run on plain files
# only, the one kind it handles. Manifest vectors are for posum itself.

set -u
root=$(cd "$(dirname "$0")/.." && pwd)
vectors="$root/tests/vectors"

cargo build --quiet --release --manifest-path "$root/verify/rust/Cargo.toml" || exit 2
rust="$root/verify/rust/target/release/posum-verify"
python="python3 $root/verify/python/posum_verify.py"
awk="$root/contrib/awk/verify-plain.sh"

runs=0 failures=0
check() {  # name verifier-label expected command...
    name=$1 label=$2 expected=$3; shift 3
    "$@" >/dev/null 2>&1
    code=$?
    runs=$((runs + 1))
    if [ "$code" -ne "$expected" ]; then
        failures=$((failures + 1))
        echo "FAIL $label $name: exit $code, expected $expected"
    fi
}

while IFS="$(printf '\t')" read -r file kind k n verdict reason; do
    case "$file" in '#'*|'') continue ;; esac
    [ "$kind" = manifest ] && continue
    case "$verdict" in valid) expected=0 ;; *) expected=1 ;; esac
    check "$file" rust "$expected" "$rust" "$kind" "$k" "$n" "$vectors/$file"
    check "$file" python "$expected" $python "$kind" "$k" "$n" "$vectors/$file"
    [ "$kind" = plain ] && check "$file" awk "$expected" "$awk" "$k" "$n" "$vectors/$file"
done < "$vectors/index.tsv"

echo "$runs runs, $failures failures"
[ "$failures" -eq 0 ]
