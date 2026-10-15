# G_5(246877)

G_5(246877) has a Hamiltonian cycle, and no G_5(m) with m < 246877 has one:
246877 is the least n for which G_5(n) is Hamiltonian.

- `bands.txt` certifies every m < 246877: each line names a vertex of degree
  at most one in G_5(m) for every m of its range.
- 1 witness found by posum, with its provenance, cited in papers:
  `25b320b3c2bbcd0467d470f75ff03404ad0025d057be9ce0537eb3b2d7cf7843`.

Check a witness with either independent verifier of the repository, after
decompressing it:

```console
xz -dk 25b320b3.txt.xz
verify/rust/target/release/posum-verify plain 5 246877 25b320b3.txt
python3 verify/python/posum_verify.py plain 5 246877 25b320b3.txt
```

The format of every file is specified in `doc/formats.md`.
