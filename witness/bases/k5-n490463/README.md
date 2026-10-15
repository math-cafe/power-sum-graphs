# G_5(490463)

G_5(490463) has a Hamiltonian cycle.

- 1 witness found by posum, with its provenance, cited in papers:
  `b201a9b4cfb109049b0ef1b5cdd56204e17a054f6c1d46c52706ccb5e716bd7e`.

Check a witness with either independent verifier of the repository, after
decompressing it:

```console
xz -dk b201a9b4.txt.xz
verify/rust/target/release/posum-verify plain 5 490463 b201a9b4.txt
python3 verify/python/posum_verify.py plain 5 490463 b201a9b4.txt
```

The format of every file is specified in `doc/formats.md`.
