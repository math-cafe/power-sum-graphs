# power-sum-graphs

Hamiltonian cycles of power-sum graphs: the results, their certificates, the
software that finds them, **posum**, and two independent verifiers.

For integers `k >= 2` and `n >= 1`, the **power-sum graph** `G_k(n)` has
vertices `1, ..., n` and joins two distinct vertices whose sum is a perfect
`k`-th power. A Hamiltonian cycle of `G_k(n)` arranges `1..n` around a circle
so that every two neighbours sum to a `k`-th power; for `k = 2` this is the
classical square-sum problem. `n_min(k)` is the least `n` for which such an
arrangement exists, OEIS [A115418](https://oeis.org/A115418).

[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.23403537.svg)](https://doi.org/10.5281/zenodo.23403537)

[Français](README.fr.md)

## Results

| k | `n_min(k)` | closed form | status | bundle |
|---|---|---|---|---|
| 2 | 32 | — | classical ([A071984](https://oeis.org/A071984)); cycle here, minimality cited | [`k2-n32`](witness/bases/k2-n32/) |
| 3 | **473** | `9^3 - 8^3/2` | found by G. Resta; minimality certified here | [`k3-n473`](witness/bases/k3-n473/) |
| 4 | **9641** | `11^4 - 10^4/2` | A115418, R. Tauraso, 2006; minimality certified here | [`k4-n9641`](witness/bases/k4-n9641/) |
| 5 | **246877** | `13^5 - 12^5/2` | new, as far as is known | [`k5-n246877`](witness/bases/k5-n246877/) |
| 6 | **15748961** | `17^6 - 16^6/2` | new, as far as is known | [`k6-n15748961`](witness/bases/k6-n15748961/) |
| 7 | `>= 587761723` | `19^7 - 18^7/2` | lower bound, certified here | — |

Each value in bold is a theorem: its bundle holds a Hamiltonian cycle **and** a
bands certificate ruling out every smaller `n`, one vertex of degree at most
one per range of `n`. For `k = 2` no such certificate exists: `G_2(31)` has
every degree at least two and is not Hamiltonian for a subtler reason.

**Admissible islands.** A vertex of degree below two rules a cycle out, so
`G_k(n)` can be Hamiltonian only when every degree is at least two. For
`k >= 4` the `n` where that holds form finite **islands** before a last,
unbounded one: `9641 - 9999` then `18193 - ...` at `k = 4`, `246877 - 248831`
then `490463 - ...` at `k = 5` (`posum islands <k>`). Every `n` of the finite
islands of `k = 4` and `k = 5` is Hamiltonian, witnesses in
[`k4-n9641-9999`](witness/bases/k4-n9641-9999/) and
[`k5-n246877-248831`](witness/bases/k5-n246877-248831/). Since 1.1.0, range
bundles add runs of consecutive values at the start of the unbounded interval:
every `n` from 473 to 10000 at `k = 3`
([`k3-r473-10000`](witness/bases/k3-r473-10000/)), every `n` from 18193 to
30000 at `k = 4` ([`k4-r18193-30000`](witness/bases/k4-r18193-30000/)), and
`n = 30039769`, the start of the unbounded interval at `k = 6`
([`k6-n30039769`](witness/bases/k6-n30039769/)). So Hamiltonicity is not
monotone in `n`: at `k = 4`, `5` and `6` it holds in the first island (at its
first value for `k = 6`), fails on the gap that follows, and holds again. All
of this supports:

> **Conjecture.** For `k >= 3`, `G_k(n)` is Hamiltonian exactly when every
> vertex has degree at least two.

Stated, with its evidence; not claimed.

## Verify a result

Nothing here asks you to trust the search. Each witness is a file of `n - 1`
**bases**: starting from 1, each base `b` gives the next vertex `b^k - v`, and
the cycle closes back to 1. From a clone:

```console
cargo build --release --manifest-path verify/rust/Cargo.toml
xz -dk witness/bases/k5-n246877/25b320b3.bases.xz

verify/rust/target/release/posum-verify bases 5 246877 witness/bases/k5-n246877/25b320b3.bases
python3 verify/python/posum_verify.py bases 5 246877 witness/bases/k5-n246877/25b320b3.bases
python3 verify/python/posum_verify.py bands 5 246877 witness/bases/k5-n246877/bands.txt
```

The two verifiers share no code with posum or with each other. The Rust one
has **no dependency at all**, the Python one uses only the standard library;
each reads one file and checks it against `doc/formats.md`: a permutation of
`1..n` whose consecutive sums, closure included, are `k`-th powers, or bands
covering `1..n-1` with the stated degrees. `contrib/awk/` decodes a bases file
to the plain list and checks plain lists, in a few lines of awk.

The plain lists, one vertex per line, are derived from the bases (`posum
bundle plain`) and published with each release on Zenodo. A witness's identity
is the SHA-256 of its plain list.

## Regenerate the witnesses

Every witness was found by posum with LKH-3, and its provenance says how:
binary, seed, segments, time. To find them all again:

```console
scripts/regenerate.sh            # about 5 h on 16 cores; --no-k6 to skip the two G_6
```

It builds posum and LKH, sweeps the islands, runs the single searches, files
every witness into `witness-regenerated/`, checks it with both verifiers and
compares it with the published bundles. LKH is built for the local processor,
so another machine may find other cycles: they are checked all the same.

`scripts/check.sh` is the pre-publication gate: lints, tests, the verifiers
against the test vectors, the bands certificates for `k = 3..7` and `posum
check` on every bundle.

## posum

```console
posum islands <k>                     admissible islands of G_k
posum bands <k> <n>                   lower-bound certificate for n
posum search <k> <n>                  find a Hamiltonian cycle with LKH
posum sweep <k> island                every n of every finite island, in parallel
posum encode | decode                 plain list <-> bases
posum bundle add | plain              file witnesses, derive the plain tree
posum check                           every bundle, both verifiers
```

A search refuses an `n` outside the islands, runs degree-two propagation,
contracts the chains of forced edges and hands the rest to LKH as a TSPLIB
problem, in resumable segments under a time limit. `doc/methods.md` describes
the algorithms, `doc/formats.md` every file, `doc/reproduce.md` one path per
claim, `doc/design.md` the decisions behind them.

**LKH** is Keld Helsgaun's [LKH-3](http://webhotel4.ruc.dk/~keld/research/LKH-3/),
distributed for academic and non-commercial use. It is not part of this
repository: `scripts/build-lkh.sh` downloads the official archive, checks its
SHA-256 and builds it on your machine.

## History and credits

- Giovanni Resta found the cycle of length 473 for cubes
  ([Puzzle 311](http://www.primepuzzles.net/puzzles/puzz_311.htm)); Roberto
  Tauraso submitted A115418 with its terms up to `k = 4` in 2006.
- 32 is the classical answer to the square-sum problem; in
  [A071984](https://oeis.org/A071984) (2018) Bert Dobbelaere noted the degree
  argument for `n <= 30` and that `n = 31` can be ruled out by hand.
- The exploration behind this release — the SAT-based engine that first found
  `n_min(5)` and `n_min(6)`, measurements, failures, research notes — is the
  lab notebook [math-cafe/powercycle](https://github.com/math-cafe/powercycle),
  kept public as provenance and archived on Software Heritage
  ([swh:1:snp:3ffd408d237801defd00401bfb20b55b1677ffa9](https://archive.softwareheritage.org/swh:1:snp:3ffd408d237801defd00401bfb20b55b1677ffa9)).
  Its witnesses are valid and independently
  verified; release 1.0 publishes only witnesses found again by posum.

## Licences and citation

Code: MIT or Apache-2.0, at your option ([LICENSE](LICENSE)). Witnesses:
public domain, CC0 1.0 ([witness/LICENSE](witness/LICENSE)).

Release 1.0.0 — the source and both witness trees — is archived on Zenodo:
[doi:10.5281/zenodo.23403537](https://doi.org/10.5281/zenodo.23403537); the
DOI [10.5281/zenodo.23403536](https://doi.org/10.5281/zenodo.23403536) always
points to the latest version. Release 1.1.0 will be deposited there as a new
version. To cite, see [CITATION.cff](CITATION.cff); for 1.0.0:

> Nouvier, F. (2026). *power-sum-graphs 1.0.0: Hamiltonian cycles of
> power-sum graphs, with certificates* (Version 1.0.0). Zenodo.
> https://doi.org/10.5281/zenodo.23403537
