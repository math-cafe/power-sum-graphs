# G_3(473)

G_3(473) has a Hamiltonian cycle, and no G_3(m) with m < 473 has one:
473 is the least n for which G_3(n) is Hamiltonian.

- `bands.txt` certifies every m < 473: each line names a vertex of degree
  at most one in G_3(m) for every m of its range.
- 1 witness found by posum, with its provenance, cited in papers:
  `48a0b7a5f9b83e65f8ad998fe91321fc38b2676e9bfde2674553049c1598fdb6`.

Check a witness with either independent verifier of the repository, after
decompressing it:

```console
xz -dk 48a0b7a5.txt.xz
verify/rust/target/release/posum-verify plain 3 473 48a0b7a5.txt
python3 verify/python/posum_verify.py plain 3 473 48a0b7a5.txt
```

The format of every file is specified in `doc/formats.md`.
