#!/usr/bin/env python3
"""Independent verifier for power-sum graph witnesses and certificates.

    posum_verify.py <plain|bases|bands> <k> <n> <file>

Decides one uncompressed file against doc/formats.md sections 1 to 4, and
exits with 0 when it is valid, 1 when it is not, 2 on a usage error. It reads
nothing but the file, as a stream: its memory is one byte per vertex.

Standard library only, and no code shared with posum or the Rust verifier.
"""

import sys


class Invalid(Exception):
    """The file does not meet the specification."""


# --- Lines (section 1) -----------------------------------------------------


def lines(path):
    """Yield (number, text) for each line, checked against the common
    conventions: ASCII, every line ending with a line feed, no carriage
    return, no empty line."""
    with open(path, "rb") as f:
        for number, raw in enumerate(f, start=1):
            if not raw.endswith(b"\n"):
                raise Invalid(f"line {number} has no final line feed")
            raw = raw[:-1]
            if not raw:
                raise Invalid(f"line {number} is empty")
            if b"\r" in raw:
                raise Invalid(f"line {number} contains a carriage return")
            if any(byte >= 0x80 for byte in raw):
                raise Invalid(f"line {number} contains a non-ASCII byte")
            yield number, raw.decode("ascii")


def integer(text, number, allow_zero=False):
    """A decimal integer: digits only, no leading zero, 0 only if allowed."""
    if not text or not all("0" <= c <= "9" for c in text):
        raise Invalid(f"line {number}: {text!r} is not a decimal integer")
    if text[0] == "0" and (len(text) > 1 or not allow_zero):
        raise Invalid(f"line {number}: {text!r} has a leading zero or is zero")
    return int(text)


# --- Powers ----------------------------------------------------------------


def root(x, k):
    """floor(x^(1/k)), exactly, by bisection on integers."""
    lo, hi = 0, 1
    while hi**k <= x:
        hi *= 2
    while hi - lo > 1:
        mid = (lo + hi) // 2
        if mid**k <= x:
            lo = mid
        else:
            hi = mid
    return lo


def is_power(x, k):
    return x > 0 and root(x, k) ** k == x


def degree(v, m, k):
    """Degree of v in G_k(m), for 1 <= v <= m."""
    return root(v + m, k) - root(v, k) - (1 if is_power(2 * v, k) else 0)


# --- Plain cycle files (section 2) -----------------------------------------


def verify_plain(path, k, n):
    if n < 3:
        raise Invalid("a cycle needs n >= 3")
    seen = bytearray(n + 1)
    first = second = previous = None
    count = 0
    for number, text in lines(path):
        v = integer(text, number)
        if v > n:
            raise Invalid(f"line {number}: {v} is outside 1..{n}")
        if seen[v]:
            raise Invalid(f"line {number}: {v} appears twice")
        seen[v] = 1
        if previous is not None and not is_power(previous + v, k):
            raise Invalid(f"line {number}: {previous} + {v} is not a {k}-th power")
        count += 1
        if count > n:
            raise Invalid(f"more than {n} lines")
        if count == 1:
            first = v
        elif count == 2:
            second = v
        previous = v
    if count != n:
        raise Invalid(f"{count} lines, expected {n}")
    if not is_power(previous + first, k):
        raise Invalid(f"closing {previous} + {first} is not a {k}-th power")
    if first != 1:
        raise Invalid(f"not canonical: starts with {first}, not 1")
    if second >= previous:
        raise Invalid(f"not canonical: second vertex {second} is not smaller than the last {previous}")


# --- Bases files (section 3) -----------------------------------------------


def verify_bases(path, k, n):
    if n < 3:
        raise Invalid("a cycle needs n >= 3")
    seen = bytearray(n + 1)
    seen[1] = 1
    v, second, count = 1, None, 0
    for number, text in lines(path):
        b = integer(text, number)
        if b < 2:
            raise Invalid(f"line {number}: base {b} is below 2")
        w = b**k - v
        if not 1 <= w <= n:
            raise Invalid(f"line {number}: {b}^{k} - {v} is outside 1..{n}")
        if seen[w]:
            raise Invalid(f"line {number}: decodes to {w}, which appears twice")
        seen[w] = 1
        v = w
        count += 1
        if count >= n:
            raise Invalid(f"more than {n - 1} lines")
        if count == 1:
            second = v
    if count != n - 1:
        raise Invalid(f"{count} lines, expected {n - 1}")
    if not is_power(v + 1, k):
        raise Invalid(f"does not close: {v} + 1 is not a {k}-th power")
    if second >= v:
        raise Invalid(f"not canonical: second vertex {second} is not smaller than the last {v}")


# --- Bands files (section 4) -----------------------------------------------


def parse_band(text, number):
    """<inf> - <sup> vertex=<v> degree=<d>, or <m> vertex=<v> degree=<d>."""
    fields = text.split(" ")
    if len(fields) == 5 and fields[1] == "-":
        inf, sup = integer(fields[0], number), integer(fields[2], number)
        if inf >= sup:
            raise Invalid(f"line {number}: range {inf} - {sup} is not increasing; "
                          "a single value is written alone")
        rest = fields[3:]
    elif len(fields) == 3:
        inf = sup = integer(fields[0], number)
        rest = fields[1:]
    else:
        raise Invalid(f"line {number}: malformed band {text!r}")
    if not rest[0].startswith("vertex=") or not rest[1].startswith("degree="):
        raise Invalid(f"line {number}: expected vertex= and degree=")
    vertex = integer(rest[0][len("vertex="):], number)
    deg = integer(rest[1][len("degree="):], number, allow_zero=True)
    return inf, sup, vertex, deg


def verify_bands(path, k, n):
    expected, previous = 1, None
    for number, text in lines(path):
        inf, sup, vertex, deg = parse_band(text, number)
        if inf != expected:
            raise Invalid(f"line {number}: starts at {inf}, expected {expected}")
        if sup >= n:
            raise Invalid(f"line {number}: ends at {sup}, beyond n - 1 = {n - 1}")
        if vertex > inf:
            raise Invalid(f"line {number}: vertex {vertex} is not in G_{k}({inf})")
        if deg > 1:
            raise Invalid(f"line {number}: degree {deg} is not at most 1")
        for m in (inf, sup):
            actual = degree(vertex, m, k)
            if actual != deg:
                raise Invalid(f"line {number}: vertex {vertex} has degree {actual} in G_{k}({m}), not {deg}")
        if previous == (vertex, deg):
            raise Invalid(f"line {number}: not canonical: same vertex and degree as the previous band")
        previous = (vertex, deg)
        expected = sup + 1
    if expected != n:
        raise Invalid(f"bands end at {expected - 1}, expected {n - 1}")


# --- Command line ------------------------------------------------------------

VERIFY = {"plain": verify_plain, "bases": verify_bases, "bands": verify_bands}


def main(argv):
    usage = "usage: posum_verify.py <plain|bases|bands> <k> <n> <file>"
    if len(argv) != 4 or argv[0] not in VERIFY:
        print(usage, file=sys.stderr)
        return 2
    kind, k, n, path = argv
    if not (k.isdigit() and int(k) >= 2 and n.isdigit() and int(n) >= 2):
        print(f"posum_verify: invalid k or n\n{usage}", file=sys.stderr)
        return 2
    k, n = int(k), int(n)
    try:
        VERIFY[kind](path, k, n)
    except Invalid as reason:
        print(f"INVALID {kind} k={k} n={n}: {reason}")
        return 1
    except OSError as error:
        print(f"posum_verify: cannot read {path}: {error}", file=sys.stderr)
        return 2
    print(f"VALID {kind} k={k} n={n}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
