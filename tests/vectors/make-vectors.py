#!/usr/bin/env python3

# Generate the test vectors of doc/formats.md section 11, with the standard
# library only. The vectors are committed; this script records how each was
# made and rebuilds them byte for byte.
#
# Usage: tests/vectors/make-vectors.py   (writes next to itself)

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))


def iroot(x, k):
    """floor(x^(1/k)) for x >= 0, exactly."""
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
    return x > 0 and iroot(x, k) ** k == x


def degree(v, m, k):
    return iroot(v + m, k) - iroot(v, k) - (1 if is_power(2 * v, k) else 0)


def bands(k, n):
    """Lower-bound bands for 1..n-1: smallest vertex of degree <= 1 per m, merged."""
    out = []
    for m in range(1, n):
        v = next(v for v in range(1, m + 1) if degree(v, m, k) <= 1)
        d = degree(v, m, k)
        if out and out[-1][2:] == [v, d] and out[-1][1] == m - 1:
            out[-1][1] = m
        else:
            out.append([m, m, v, d])
    return out


def band_lines(bs):
    return "".join(
        (f"{a} - {b}" if a < b else f"{a}") + f" vertex={v} degree={d}\n" for a, b, v, d in bs
    )


def lines(values):
    return "".join(f"{v}\n" for v in values)


def bases_of(path, k):
    return [iroot(a + b, k) for a, b in zip(path, path[1:])]


def hamiltonian_path_not_closing(k, n):
    """A canonical-looking Hamiltonian path of G_k(n) from 1 whose end is not
    adjacent to 1 (second vertex smaller than the last), by backtracking."""
    adj = {v: [u for u in range(1, n + 1) if u != v and is_power(u + v, k)] for v in range(1, n + 1)}
    path, used = [1], {1}

    def extend():
        if len(path) == n:
            return not is_power(path[-1] + 1, k) and path[1] < path[-1]
        for u in sorted(adj[path[-1]], key=lambda u: len(adj[u])):
            if u not in used:
                used.add(u)
                path.append(u)
                if extend():
                    return True
                path.pop()
                used.discard(u)
        return False

    sys.setrecursionlimit(10000)
    assert extend()
    return path


def write(name, content):
    """Write exactly these bytes: str as ASCII with no newline translation."""
    data = content if isinstance(content, bytes) else content.encode("ascii")
    with open(os.path.join(HERE, name), "wb") as f:
        f.write(data)


def main():
    # The unique Hamiltonian cycle of G_2(32), canonical.
    g = [1, 8, 28, 21, 4, 32, 17, 19, 30, 6, 3, 13, 12, 24, 25, 11, 5, 31, 18, 7, 29, 20, 16, 9,
         27, 22, 14, 2, 23, 26, 10, 15]
    assert sorted(g) == list(range(1, 33)) and g[1] < g[-1]
    assert all(is_power(a + b, 2) for a, b in zip(g, g[1:] + g[:1]))

    index = []

    def add(name, kind, k, n, verdict, reason):
        index.append(f"{name}\t{kind}\t{k}\t{n}\t{verdict}\t{reason}\n")

    write("g2-32.txt", lines(g))
    add("g2-32.txt", "plain", 2, 32, "valid", "the unique cycle")

    write("g2-32.bases", lines(bases_of(g, 2)))
    add("g2-32.bases", "bases", 2, 32, "valid", "the same cycle")

    for k, n in ((3, 473), (4, 9641)):
        write(f"g{k}-{n}-bands.txt", band_lines(bands(k, n)))
        add(f"g{k}-{n}-bands.txt", "bands", k, n, "valid", "the lower bound")

    dup = g[:]
    dup[9] = dup[8]
    write("dup.txt", lines(dup))
    add("dup.txt", "plain", 2, 32, "invalid", "a vertex repeated")

    write("short.txt", lines(g[:-1]))
    add("short.txt", "plain", 2, 32, "invalid", "31 lines")

    swapped = g[:]
    swapped[4], swapped[5] = swapped[5], swapped[4]
    write("nonedge.txt", lines(swapped))
    add("nonedge.txt", "plain", 2, 32, "invalid", "two neighbours not summing to a square")

    path = hamiltonian_path_not_closing(2, 32)
    write("open.txt", lines(path))
    add("open.txt", "plain", 2, 32, "invalid", "last and first not summing to a square")

    write("rotated.txt", lines(g[1:] + g[:1]))
    add("rotated.txt", "plain", 2, 32, "invalid", "valid cycle, not starting at 1")

    write("reversed.txt", lines([1] + g[1:][::-1]))
    add("reversed.txt", "plain", 2, 32, "invalid", "valid cycle, second line larger than the last")

    write("crlf.txt", lines(g).replace("\n", "\r\n").encode())
    add("crlf.txt", "plain", 2, 32, "invalid", "carriage returns")

    write("nofinal.txt", lines(g)[:-1])
    add("nofinal.txt", "plain", 2, 32, "invalid", "no final line feed")

    write("leading0.txt", lines(g).replace("\n8\n", "\n08\n", 1))
    add("leading0.txt", "plain", 2, 32, "invalid", "an integer with a leading zero")

    write("open.bases", lines(bases_of(path, 2)))
    add("open.bases", "bases", 2, 32, "invalid", "does not close")

    out = bases_of(g, 2)
    out[0] = 10  # 10^2 - 1 = 99, outside 1..32
    write("out.bases", lines(out))
    add("out.bases", "bases", 2, 32, "invalid", "decodes outside 1..n")

    b4 = bands(4, 9641)
    write("gap-bands.txt", band_lines(b4[:2] + b4[3:]))
    add("gap-bands.txt", "bands", 4, 9641, "invalid", "a missing value")

    long = [b[:] for b in b4]
    long[1][1] += 1          # vertex 1 has degree 2 at 80, since 1 + 80 = 3^4
    long[2][0] += 1          # keep the coverage contiguous
    write("long-bands.txt", band_lines(long))
    add("long-bands.txt", "bands", 4, 9641, "invalid", "a band past its vertex's degree")

    # A single value written "m - m" cannot be the only defect of a vector: no
    # natural single-value band occurs for k = 2..5, and splitting a band would
    # also break the merging rule. The notation is tested by a dash without
    # spaces instead; "m - m" is left to the verifiers' unit tests.
    dash = band_lines(b4).replace("1 - 14 vertex=1", "1-14 vertex=1", 1)
    write("dash-bands.txt", dash)
    add("dash-bands.txt", "bands", 4, 9641, "invalid", "a dash without its spaces")

    witness = "5f2b4192388caf362098311b1363163bfb78b554fb2ae8109608f773f50066c5"
    bands_sha = "015b5be7015d2f6d1756a02068fa0202b5472442c0e3ca09d52a3935ed93b06e"
    canon = (
        'format = "posum-bundle 1"\n'
        "k = 6\n"
        "n = 15748961\n"
        'claims = ["hamiltonian", "minimal"]\n'
        f'reference = "{witness}"\n'
        "\n[bands]\n"
        'file = "bands.txt"\n'
        f'sha256 = "{bands_sha}"\n'
        "count = 16\n"
        "\n[[witness]]\n"
        f'id = "{witness}"\n'
        f'plain = {{ file = "{witness[:8]}.txt.xz" }}\n'
        f'bases = {{ file = "{witness[:8]}.bases.xz" }}\n'
        f'provenance = "{witness[:8]}.provenance.toml"\n'
    )
    write("canon.toml", canon)
    add("canon.toml", "manifest", 6, 15748961, "valid", "canonical manifest")

    write("noncanon.toml", canon.replace("k = 6\nn = 15748961\n", "n = 15748961\nk = 6\n", 1))
    add("noncanon.toml", "manifest", 6, 15748961, "invalid", "keys out of order")

    with open(os.path.join(HERE, "index.tsv"), "w", newline="") as f:
        f.write("# file\tkind\tk\tn\tverdict\treason\n")
        f.writelines(index)

    print(f"{len(index)} vectors written to {HERE}")


if __name__ == "__main__":
    main()
