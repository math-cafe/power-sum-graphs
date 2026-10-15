# Reproducing each claim

One path per claim of `doc/design.md` section 2: how to check it from a clone,
and which step of the pre-publication gate, `scripts/check.sh`, checks it on
every release. Commands run from the repository root, after

```console
cargo build --release --locked
cargo build --release --locked --manifest-path verify/rust/Cargo.toml
```

`V` below stands for either verifier, used the same way:

```console
verify/rust/target/release/posum-verify <kind> <k> <n> <file>
python3 verify/python/posum_verify.py   <kind> <k> <n> <file>
```

Both exit with 0 for a valid file, 1 for an invalid one.

---

## 1. `n_min(k)` for `k = 3, 4, 5, 6`

`n_min(k) = n` takes two facts: `G_k(n)` has a Hamiltonian cycle, and no
`G_k(m)` with `m < n` has one. The bundle `witness/bases/k<k>-n<n>/` holds both.

```console
cd witness/bases/k5-n246877
xz -dk 25b320b3.bases.xz
V bases 5 246877 25b320b3.bases        # the cycle
V bands 5 246877 bands.txt             # every m < 246877 ruled out
```

Each line of `bands.txt` names, for a range `inf..sup` of `m`, a vertex whose
degree in `G_k(m)` is at most one, checked at both ends of the range since a
degree never decreases as `m` grows. The reference witnesses are `48a0b7a5`
(`k = 3`), `cac088f0` (`k = 4`), `25b320b3` (`k = 5`) and `4141453d` (`k = 6`).

**Gate:** steps *bands: n_min(k)*, which recompute each bands file with posum
and have both verifiers accept it, and *posum check*, which checks every
witness and every `bands.txt` with both verifiers.

## 2. `n_min(7) >= 587761723`

No witness: the claim is the bands certificate alone.

```console
target/release/posum bands 7 587761723 > bands-7.txt
V bands 7 587761723 bands-7.txt
```

It takes 18 lines. **Gate:** step *bands: n_min(7) >= 587761723*.

## 3. `n_min(2) = 32`

The bundle `witness/bases/k2-n32/` holds a cycle of `G_2(32)`
(`V bases 2 32 ...`). Minimality is not certified: `G_2(31)` has every degree
at least two, so no bands file exists. It is the classical answer to the
square-sum problem, cited from OEIS A071984, where B. Dobbelaere notes how
`n = 31` is ruled out; a certificate is planned after 1.0.

**Gate:** *posum check* for the cycle.

## 4. The admissible islands, `k = 2..12`

```console
target/release/posum islands 5
```

prints `246877 - 248831` then `490463 - ...`. The islands are computed by
following the smallest blocking vertex, which is always 1, a power `a^k` or a
loop position `a^k/2`, so only `O(k)` candidates are examined
(`doc/methods.md` section 2). The computation is posum's own; it is tested
against a scan of every vertex for every `m` up to 3 000 (12 000 at `k = 4`),
and against the lab notebook's independent script for `k = 2..7` and `13`
(`crates/posum-core/src/islands.rs`). Each island's left end is the first `n`
the bands of section 1 do not rule out, which the verifiers check.

**Gate:** *posum: tests*.

## 5. Every `n` of the finite islands of `k = 4` and `k = 5` is Hamiltonian

The island bundles `witness/bases/k4-n9641-9999/` and
`witness/bases/k5-n246877-248831/` hold one witness per `n`, in `n<n>/`. To
check them all:

```console
target/release/posum check
```

or one at a time with `V bases`. **Gate:** *posum check*.

## 6. Regenerating the witnesses

Not a claim — the claims rest on verification — but a check that the software
does what it says:

```console
scripts/regenerate.sh
```

finds every witness again with posum and LKH, files it into
`witness-regenerated/`, runs `posum check` on it and compares it with the
published bundles. With an LKH binary built on another processor, other
cycles may be found; they are checked the same way.
