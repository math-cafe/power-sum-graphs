#!/bin/sh
# Verify a plain cycle file (doc/formats.md section 2) with awk.
#
#   contrib/awk/verify-plain.sh <k> <n> <file>
#
# Exit 0 if valid, 1 if not, 2 on a usage error. Not an official verifier: a
# demonstration that checking a witness takes twenty lines in any language.
# awk computes in double precision, exact for integers below 2^53, which the
# script checks; that covers every sum up to n near 4.5 * 10^15. Its memory is
# an associative array, about 230 bytes per vertex: 3.6 GB for G_6(15748961).

[ $# -eq 3 ] || { echo "usage: verify-plain.sh <k> <n> <file>" >&2; exit 2; }
k=$1 n=$2 file=$3
[ -r "$file" ] || { echo "verify-plain: cannot read $file" >&2; exit 2; }

# awk cannot see a missing final line feed; check it here.
if [ -s "$file" ] && [ "$(tail -c 1 "$file" | od -An -c | tr -d ' ')" != '\n' ]; then
    echo "INVALID plain k=$k n=$n: no final line feed"; exit 1
fi

LC_ALL=C awk -v k="$k" -v n="$n" '
function bad(why) { print "INVALID plain k=" k " n=" n ": " why; failed = 1; exit 1 }
function power(x,    r, p, i) {          # is x an exact k-th power?
    r = int(exp(log(x) / k) + 0.5)
    for (d = -1; d <= 1; d++) { p = 1; for (i = 0; i < k; i++) p *= (r + d); if (p == x) return 1 }
    return 0
}
BEGIN { if (2 * n >= 2 ^ 53) { print "verify-plain: n too large for awk" > "/dev/stderr"; exit 2 } }
{
    if ($0 !~ /^[1-9][0-9]*$/) bad("line " NR ": " $0 " is not a canonical decimal integer")
    v = $0 + 0
    if (v > n) bad("line " NR ": " v " is outside 1.." n)
    if (v in seen) bad("line " NR ": " v " appears twice")
    seen[v] = 1
    if (NR > 1 && !power(prev + v)) bad("line " NR ": " prev " + " v " is not a " k "-th power")
    if (NR == 1) first = v; if (NR == 2) second = v
    prev = v
}
END {
    if (failed) exit 1
    if (NR != n) bad(NR " lines, expected " n)
    if (!power(prev + first)) bad("closing " prev " + " first " is not a " k "-th power")
    if (first != 1) bad("not canonical: starts with " first)
    if (second >= prev) bad("not canonical: second vertex " second " is not smaller than the last " prev)
    print "VALID plain k=" k " n=" n
}' "$file"
