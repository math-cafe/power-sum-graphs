# G_6(15748961)

G_6(15748961) has a Hamiltonian cycle, and no G_6(m) with m < 15748961 has one:
15748961 is the least n for which G_6(n) is Hamiltonian.

- `bands.txt` certifies every m < 15748961: each line names a vertex of degree
  at most one in G_6(m) for every m of its range.
- 1 witness found by posum, with its provenance, cited in papers:
  `4141453d12d7f53e1c63d3aa8373884eb5ed5da5fc59c1fcd78921c8bf53dd70`.

Check a witness with either independent verifier of the repository, after
decompressing it:

```console
xz -dk 4141453d.txt.xz
verify/rust/target/release/posum-verify plain 6 15748961 4141453d.txt
python3 verify/python/posum_verify.py plain 6 15748961 4141453d.txt
```

The format of every file is specified in `doc/formats.md`.
