# G_4(9641)

G_4(9641) has a Hamiltonian cycle, and no G_4(m) with m < 9641 has one:
9641 is the least n for which G_4(n) is Hamiltonian.

- `bands.txt` certifies every m < 9641: each line names a vertex of degree
  at most one in G_4(m) for every m of its range.
- 1 witness found by posum, with its provenance, cited in papers:
  `cac088f0e2a21fd7f3fe38ef3a5c9b6dd0c5b4edcc3822cb5492d7b919b4d7bc`.

Check a witness with either independent verifier of the repository, after
decompressing it:

```console
xz -dk cac088f0.txt.xz
verify/rust/target/release/posum-verify plain 4 9641 cac088f0.txt
python3 verify/python/posum_verify.py plain 4 9641 cac088f0.txt
```

The format of every file is specified in `doc/formats.md`.
