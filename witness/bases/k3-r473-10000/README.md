# G_3(n) for every n of the admissible range 473 - 10000

Every G_3(n) with 473 <= n <= 10000, 9528 consecutive admissible values of
G_3, has a Hamiltonian cycle. This bundle holds witnesses for 9528 of them,
9528 in all, one directory `n<n>/` per value, each witness with its
provenance; `manifest.toml` marks one reference per value.

Check a witness with either independent verifier of the repository, after
decompressing it:

```console
xz -dk n<n>/<sha8>.txt.xz
verify/rust/target/release/posum-verify plain 3 <n> n<n>/<sha8>.txt
python3 verify/python/posum_verify.py plain 3 <n> n<n>/<sha8>.txt
```

The format of every file is specified in `doc/formats.md`.
