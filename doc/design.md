# power-sum-graphs (PΣG) 1.0 — design note

Status: draft, to be discussed before any code is written. Decisions taken so
far are marked **decided**; the rest are proposals or open questions.

---

## 1. Purpose

For `k >= 2` and `n >= 1`, the power-sum graph `G_k(n)` has vertex set
`{1, ..., n}` and joins two distinct vertices whose sum is a perfect `k`-th
power. `n_min(k)`, the least `n` for which `G_k(n)` has a Hamiltonian cycle, is
OEIS A115418.

This repository is the published, citable object behind the results: it
produces and independently verifies every claim, and nothing else. The
exploration that led to it — failed optimisations, benchmarks, research notes,
experiments — lives in the lab notebook, the earlier repository
`math-cafe/powercycle`, which stays public and is cited for provenance.

**Name (decided):** `power-sum-graphs`, abbreviated PΣG in typeset text and
`psg` in every technical identifier: binary, crates, commands, file names.

---

## 2. Claims the repository must reproduce

| claim | status | prior work |
|---|---|---|
| `n_min(3) = 473`, `n_min(4) = 9641` | known values, minimality certified here | G. Resta (473); OEIS A115418, R. Tauraso, 2006 |
| `n_min(5) = 246877`, `n_min(6) = 15748961` | new, as far as is known | — |
| `n_min(7) >= 587761723` | lower bound | — |
| admissible islands `I_k = { n : delta(G_k(n)) >= 2 }` for `k = 2..12` | characterisation and `O(k)` computation | — |
| `G_2(31)` is not Hamiltonian | known (B. Dobbelaere, A071984); explicit certificate here | — |
| every `n` of the finite islands of `k = 4` and `k = 5` is Hamiltonian | evidence for the conjecture below | — |

**Conjecture:** for `k >= 3`, `G_k(n)` is Hamiltonian exactly when
`n ∈ I_k`. Stated, with its evidence; not claimed.

Each claim needs a reproduction path in `doc/reproduce.md` and a check in the
pre-publication gate.

---

## 3. Architecture (proposal)

```text
power-sum-graphs/
├── README.md, README.fr.md        results, verification, attributions
├── CITATION.cff, CHANGELOG.md, LICENSE-MIT, LICENSE-APACHE
├── Cargo.toml, rust-toolchain.toml
├── crates/
│   ├── psg-core/     G_k(n) by formula, islands in O(k), bands certificate,
│   │                 canonical order, bases encoding; no heavy dependency
│   ├── psg-search/   preflight and propagation, chain contraction,
│   │                 SAT + repair engine (CaDiCaL), checkpoints
│   ├── psg-lkh/      TSPLIB export (contracted), tour import, LKH driver
│   └── psg/          the single binary
├── verify/           outside the workspace, independent of everything above
│   ├── rust/         no dependency at all
│   └── python/       standard library only
├── lean/             Lean 4 project, pinned Mathlib
├── witness/
│   ├── plain/        bundles with the plain cycle list
│   └── bases/        the same bundles with the bases encoding
├── scripts/
│   ├── check.sh      pre-publication gate
│   └── build-lkh.sh  fetch, verify and build the pinned LKH
├── doc/
│   ├── design.md     this note
│   ├── formats.md    specification of bundles, manifests, encodings
│   ├── methods.md    algorithms
│   └── reproduce.md  one path per claim
└── .github/workflows/
```

### What carries over from the lab notebook, and what does not

Carried over, rewritten where needed: the graph and the preflight, degree-two
propagation (with the chain-closing rule it lacks today), chain contraction,
the SAT + repair engine, the TSPLIB bridge, canonical order, the bands
certificate, both verifiers, the bundle builder and the pre-publication gate.

Left behind: the per-`n` scan (bands come from the island chain directly), the
separate `pc-patch` binary (folded into `search`), the dump format as a
published artefact (kept internal, for checkpoints), the nice-pair and glue
experiments, the build benchmarks, the research notes.

---

## 4. Commands (proposal)

```console
psg islands <k>                         admissible islands of G_k
psg bands <k> <n>                       lower-bound certificate for n
psg search <k> <n> [--engine lkh|sat]   find a Hamiltonian cycle
psg encode <k> <n> cycle.txt            plain list -> bases
psg decode <k> <n> cycle.bases          bases -> plain list
psg bundle <k> <n> ...                  build or verify a bundle
psg check                               the whole pre-publication gate
```

`bands` computes the certificate from the chain of blocking vertices, in
`O(k)`, instead of scanning every `n`.

---

## 5. Search engines

**Decided:** LKH-3 is the main engine, the SAT + repair engine the
self-contained fallback. Measured in the lab notebook, one run each:
`G_6(15748961)` in 12 min with the three-level LKH build against about 28 h
for SAT + repair.

LKH-3 is distributed "for academic and non-commercial use; the author reserves
all rights". Our use is academic and non-commercial; redistribution is not
granted. Therefore (decided):

- the repository never contains LKH;
- `scripts/build-lkh.sh` downloads LKH-3.0.14 from the author's page, checks
  the archive's SHA-256
  (`a9a79b5b8632a843d42ed93ab77fa10c5237aca16d5c1bf31287e214e931f3ca`),
  sets `TREE_TYPE = THREE_LEVEL_TREE` and builds it;
- LKH, its version and its licence are stated in the README, the papers and
  every manifest whose witness it found;
- a permission request to include the unmodified archive in the Zenodo deposit
  has been drafted for Keld Helsgaun. If granted, the archive joins the deposit.

Parameters that matter, from the measurements: `SUBGRADIENT = NO` (the default
ascent is useless for 0/1 instances and very slow), `OPTIMUM = 0`,
`STOP_AT_OPTIMUM = YES`, `OUTPUT_TOUR_FILE` so a stopped run keeps its best
tour, and chain contraction, which cuts LKH's memory by 42 % at `k = 6`.

---

## 6. Witness bundles

### 6.1 Layout (decided)

Two parallel trees, one per encoding, so that a reader downloads only one:

```text
witness/plain/k6-n15748961/          witness/bases/k6-n15748961/
├── manifest.toml                    ├── manifest.toml
├── bands.txt                        ├── bands.txt
├── README.md                        ├── README.md
├── 5f2b4192/cycle.txt.xz            ├── 5f2b4192/cycle.bases.xz
└── .../cycle.txt.xz                 └── .../cycle.bases.xz
```

- One directory per `(k, n)`; the statement and the minimality certificate
  belong to `(k, n)`, not to a cycle.
- **All witnesses are kept (decided)**, several per `(k, n)` when several were
  found, and non-minimal `n` too (existence only, no `bands.txt`).
- Each witness directory is named by the first 8 hex digits of the SHA-256 of
  its canonical plain list, identical in both trees.
- The manifest names one **reference witness**, the one cited in papers.

### 6.2 Encodings (decided)

**Plain.** One vertex per line, canonical: rotated so that 1 comes first,
oriented so that the smaller neighbour of 1 comes second. Its SHA-256 is the
witness's identity, unchanged from the lab notebook's 0.2.0 bundles. Meant for
anyone writing their own verifier.

**Bases.** `n - 1` lines, one base `b` per line: starting from `v = 1`, the next
vertex is `b^k - v`. The cycle closes back to 1. Decoding needs no graph, and
every sum is a `k`-th power by construction. Measured on the published
witnesses, against the plain list under xz:

| k | plain list, xz | bases, xz | ratio |
|---|---|---|---|
| 4 | 10 300 B | 568 B | 18x |
| 5 | 258 744 B | 7 524 B | 34x |
| 6 | 24 744 912 B | 1 576 820 B | 15.7x |

A pure binary encoding was measured at only 7 % smaller and rejected.

**Bands.** The lower-bound certificate: one band per line,
`start..end<TAB>IMPOSSIBLE<TAB>degree<2 vertex=v degree=d`, covering `1..n-1`.
The degree of a vertex never decreases as `n` grows, so each band is checked at
its two ends.

### 6.3 Manifest (proposal)

```toml
format = "psg-bundle 1"
k = 6
n = 15748961
claims = ["hamiltonian", "minimal"]       # "minimal" requires bands.txt
encoding = "plain"                         # or "bases"
reference = "5f2b4192..."                  # full SHA-256

[bands]
sha256 = "015b5be7..."

[[witness]]
sha256 = "5f2b4192..."                     # of the canonical plain list
file = "5f2b4192/cycle.txt.xz"
engine = "sat-repair"                      # or "lkh-3.0.14-three-level-contracted", ...
found = "2026-08-23"
seconds = 99900
provenance = "powercycle 0.1.0, witness k6-n15748961"
```

### 6.4 Verification (decided)

Each verifier, Rust and Python, independently: reads either encoding, checks
that there are exactly `n` values, none repeated, none missing, that
consecutive sums and the closing sum are `k`-th powers, and that the plain
SHA-256 matches the manifest; checks `bands.txt` band by band. The gate also
checks that the two trees agree: same manifests, bases decoding to the plain
lists, identical `bands.txt`.

### 6.5 Hosting (decided, threshold open)

Witnesses live on Zenodo, which has a DOI and no 100 MB file limit (a `k = 7`
plain witness would be about 920 MB). The repository keeps every manifest,
every `bands.txt`, and the bundles small enough for Git. **Open:** where the
line is. A `k = 6` plain witness is 25 MB, and there are four.

---

## 7. Lean (proposal)

Lean helps most where there is reasoning, least where there is only data:

- **Phase A:** define `G_k(n)` and degree; prove that a vertex of degree at
  most one rules out a Hamiltonian cycle, and that degree is monotone in `n`;
  check the bands for `k = 3..7`; the certificate for `G_2(31)`; the cycles of
  32 and 473 vertices. Outcome: the lower bounds for `k = 3..7` formally
  verified, and `a(2) = 32`, `a(3) = 473` entirely.
- **Phase B, optional:** existence at `k = 4` if the kernel can take 9641
  vertices.
- **Phase C, later:** exact statements of the islands theory.

Existence for `k = 5, 6` stays with the two verifiers: the cycles are too large
for the kernel, and `native_decide` would weaken the guarantee being sought.
Mathlib's current graph API has not been checked yet. Building Mathlib in CI
needs its cache or a separate workflow.

---

## 8. Publications

The sources live in a separate private repository (decided). Planned:

1. the results paper: `n_min(5)`, `n_min(6)` with certificates, the islands,
   `G_2(31)`, the conjecture and its evidence, prior work credited;
2. the computational paper: SAT + repair, propagation, contraction, LKH,
   measurements and failures;
3. the OEIS: extend A115418 with `a(5)` and `a(6)`, and propose the sequence of
   least admissible `n` (31, 473, 9641, 246877, 15748961, 587761723, ...).
   **Only after release 1.0 and its Zenodo DOI (decided).**

Tables and figures in the papers are generated from this repository's data.

---

## 9. Versioning and release

Version 1.0.0 is the first release of this repository; the lab notebook ended
at 0.2.0. A GitHub release archived on Zenodo gives the DOI. Licences carried
over: MIT OR Apache-2.0 for code, CC0 for witnesses.

---

## 10. Open questions

1. The Git / Zenodo threshold for witnesses (section 6.5).
2. The manifest fields, and whether to keep run-level provenance beyond
   engine, date and duration.
3. Whether `psg search` should accept a time budget and resume from a
   checkpoint for both engines.
4. Lean phase B: worth attempting?
5. The provenance of the 1 955 `k = 5` island cycles (computed in August 2026,
   engine not recorded): rebuild them with a recorded engine before
   publishing, or publish them as they are with that caveat.
6. CI cost of Lean, and of verifying the larger witnesses.
