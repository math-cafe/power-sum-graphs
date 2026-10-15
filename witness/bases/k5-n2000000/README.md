# G_5(2000000)

G_5(2000000) has a Hamiltonian cycle.

- 1 witness found by posum, with its provenance, cited in papers:
  `755066596f41055c9d3fb7c14cc120146d6d0239afb3c16ea7876af48c8d330d`.

Check a witness with either independent verifier of the repository, after
decompressing it:

```console
xz -dk 75506659.txt.xz
verify/rust/target/release/posum-verify plain 5 2000000 75506659.txt
python3 verify/python/posum_verify.py plain 5 2000000 75506659.txt
```

The format of every file is specified in `doc/formats.md`.
