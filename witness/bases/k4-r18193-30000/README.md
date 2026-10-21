# G_4(n) for every n of the admissible range 18193 - 30000

Every G_4(n) with 18193 <= n <= 30000, 11808 consecutive admissible values of
G_4, has a Hamiltonian cycle. This bundle holds witnesses for 11808 of them,
11808 in all, one directory `n<n>/` per value, each witness with its
provenance; `manifest.toml` marks one reference per value.

Check a witness with either independent verifier of the repository, after
decompressing it:

```console
xz -dk n<n>/<sha8>.txt.xz
verify/rust/target/release/posum-verify plain 4 <n> n<n>/<sha8>.txt
python3 verify/python/posum_verify.py plain 4 <n> n<n>/<sha8>.txt
```

The format of every file is specified in `doc/formats.md`.
