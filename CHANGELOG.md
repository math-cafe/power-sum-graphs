# Changelog

## 1.1.0 — 2026-10-22

Not yet deposited on Zenodo; the DOI
[10.5281/zenodo.23403536](https://doi.org/10.5281/zenodo.23403536) points to
the latest deposited version.

- Range bundles, `k<k>-r<inf>-<sup>`: every `n` of an interval of consecutive
  admissible values that is not a whole island (`doc/formats.md` section 7.5,
  `doc/design.md` section 6.6). `posum bundle add --range <inf> <sup>`;
  `posum check` enforces the rules, including disjointness.
- Results, all found by posum: every `n` from 473 to 10000 is Hamiltonian at
  `k = 3` (`k3-r473-10000`, 9528 witnesses), every `n` from 18193 to 30000 at
  `k = 4` (`k4-r18193-30000`, 11808 witnesses), and `G_6(30039769)`, the start
  of the unbounded admissible interval of `k = 6` (`k6-n30039769`).
- The bundles of 1.0.0 are unchanged.

## 1.0.0 — 2026-10-15

Archived on Zenodo: [doi:10.5281/zenodo.23403537](https://doi.org/10.5281/zenodo.23403537).

First release of this repository, rebuilt from the lab notebook
[math-cafe/powercycle](https://github.com/math-cafe/powercycle), which ended
at 0.2.0.

- Results: `n_min(k)` for `k = 3..6` with cycles and bands certificates,
  `n_min(7) >= 587761723`, the cycle of `G_2(32)`, and every `n` of the finite
  islands of `k = 4` and `k = 5`, all found by posum.
- posum: islands, bands, search with LKH-3 in resumable segments, parallel
  sweeps, encodings, bundles, `posum check`.
- Two independent verifiers, Rust without dependencies and Python with the
  standard library only, plus an awk verifier and decoder in `contrib/`.
- Formats specified in `doc/formats.md`, with test vectors.
- The repository holds the bases tree; the plain tree is derived and
  published on Zenodo with each release.
