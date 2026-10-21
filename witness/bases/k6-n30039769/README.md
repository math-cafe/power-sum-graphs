# G_6(30039769)

G_6(30039769) has a Hamiltonian cycle.

- 1 witness found by posum, with its provenance, cited in papers:
  `a7aa7317ddc6a3d38164b0d0372c0ab862eab57b3132f7a514a0e262491d4eb6`.

Check a witness with either independent verifier of the repository, after
decompressing it:

```console
xz -dk a7aa7317.txt.xz
verify/rust/target/release/posum-verify plain 6 30039769 a7aa7317.txt
python3 verify/python/posum_verify.py plain 6 30039769 a7aa7317.txt
```

The format of every file is specified in `doc/formats.md`.
