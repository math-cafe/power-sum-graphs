# G_5(n) for every n of the admissible island 246877 - 248831

Every G_5(n) with 246877 <= n <= 248831, the 1955 values of a finite admissible
island of G_5, has a Hamiltonian cycle. This bundle holds witnesses for
1955 of them, 1955 in all, one directory `n<n>/` per value, each witness with
its provenance; `manifest.toml` marks one reference per value.

Check a witness with either independent verifier of the repository, after
decompressing it:

```console
xz -dk n<n>/<sha8>.txt.xz
verify/rust/target/release/posum-verify plain 5 <n> n<n>/<sha8>.txt
python3 verify/python/posum_verify.py plain 5 <n> n<n>/<sha8>.txt
```

The format of every file is specified in `doc/formats.md`.
