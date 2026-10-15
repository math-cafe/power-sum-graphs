# G_5(600000)

G_5(600000) has a Hamiltonian cycle.

- 1 witness found by posum, with its provenance, cited in papers:
  `4f10d95c684b47e8d2cf7fd1f8bc5d05d5d25c774dd70bc3f41fa7786ea07084`.

Check a witness with either independent verifier of the repository, after
decompressing it:

```console
xz -dk 4f10d95c.txt.xz
verify/rust/target/release/posum-verify plain 5 600000 4f10d95c.txt
python3 verify/python/posum_verify.py plain 5 600000 4f10d95c.txt
```

The format of every file is specified in `doc/formats.md`.
