#!/bin/sh
# Decode a bases file (doc/formats.md section 3) to the plain cycle list.
#
#   contrib/awk/decode-bases.sh <k> <file.bases> > cycle.txt
#
# Starting from 1, each base b gives the next vertex b^k - v. The output is
# the plain list; check it with verify-plain.sh or either verifier. awk
# computes in double precision, exact below 2^53, which the script checks.

[ $# -eq 2 ] || { echo "usage: decode-bases.sh <k> <file.bases>" >&2; exit 2; }
[ -r "$2" ] || { echo "decode-bases: cannot read $2" >&2; exit 2; }

LC_ALL=C awk -v k="$1" '
BEGIN { v = 1; print v }
{
    if ($0 !~ /^[1-9][0-9]*$/) { print "decode-bases: line " NR ": not a base" > "/dev/stderr"; exit 1 }
    p = 1; for (i = 0; i < k; i++) p *= $0
    if (p >= 2 ^ 53) { print "decode-bases: values too large for awk" > "/dev/stderr"; exit 2 }
    v = p - v
    print v
}' "$2"
