# G_2(32)

G_2(32) has a Hamiltonian cycle.

32 is also the least n for which G_2(n) is Hamiltonian, a known result
(B. Dobbelaere, OEIS A071984) cited here, not certified: G_2(31) has every
degree at least two, so no bands file can rule it out.

- 1 witness found by posum, with its provenance, cited in papers:
  `7ace7e14d665b3d285c1948f068eaeaf26ea74270c63b329bbea41735371dc0c`.

Check a witness with either independent verifier of the repository, after
decompressing it:

```console
xz -dk 7ace7e14.txt.xz
verify/rust/target/release/posum-verify plain 2 32 7ace7e14.txt
python3 verify/python/posum_verify.py plain 2 32 7ace7e14.txt
```

The format of every file is specified in `doc/formats.md`.
