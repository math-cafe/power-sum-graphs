# Changelog

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
