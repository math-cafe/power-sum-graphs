# power-sum-graphs 1.0 — design note

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

**Names (decided):**

- the repository is `power-sum-graphs`: explicit, and free on GitHub, crates.io
  and PyPI;
- the software is **posum**, after the Latin *possum*, "I can": the binary
  `posum`, the crates `posum-core`, `posum-lkh` and `posum-cli`, the last
  producing the binary. All were free on crates.io and
  PyPI on 13 October 2026; `posum` alone is taken on crates.io, which is why the
  binary's crate is `posum-cli`;
- in papers the object is a *power-sum graph*; "posum" names the software.

Rejected: `psg`, taken on crates.io and PyPI, shared with thousands of
repositories and with a common shell alias for `ps aux | grep`, and PΣG with it;
`possum`, taken on crates.io and PyPI, as is `possum-core`.

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
│   ├── posum-core/   G_k(n) by formula, islands in O(k), bands certificate,
│   │                 canonical order, bases encoding, degree-two propagation,
│   │                 chain contraction; no heavy dependency
│   ├── posum-lkh/    TSPLIB export and tour import, LKH runs as a separate
│   │                 process, run directories, resumption
│   └── posum-cli/    the single binary, `posum`
├── verify/           outside the workspace, independent of everything above
│   ├── rust/         no dependency at all
│   └── python/       standard library only
├── lean/             after 1.0: Lean 4 project, pinned Mathlib (section 7)
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

**Built clean, not moved (decided).** Moving the lab notebook's code with new
names would bring its disorder along, which is precisely what this repository
must avoid. Each part is rebuilt to this design, reusing proven algorithms and
tests, not their structure.

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
posum islands <k>                         admissible islands of G_k
posum bands <k> <n>                       lower-bound certificate for n
posum search <k> <n>                      find a Hamiltonian cycle with LKH
             [--time-limit T] [--run-dir D] [--force]
posum search --resume <D> [--time-limit T] [--force]
posum encode <k> <n> cycle.txt            plain list -> bases
posum decode <k> <n> cycle.bases          bases -> plain list
posum bundle <k> <n> ...                  build or verify a bundle
posum check                               the whole pre-publication gate
```

`bands` computes the certificate from the chain of blocking vertices, in
`O(k)`, instead of scanning every `n`.

---

## 5. Search engines

**Decided: LKH-3 is the only engine.** Measured in the lab notebook, one run
each: `G_6(15748961)` in 12 min with the three-level LKH build against about
28 h for SAT + repair. No published claim depends on how a witness was found —
the verifiers check every witness whatever produced it — so the search engine
serves finding, never proving. The SAT + repair engine, about 10 000 of the lab
notebook's 15 000 lines, stays there, frozen with its measurements; the
computational paper and the provenance of the witnesses it found refer to it.

Consequences, decided:

- **Searching requires LKH; verifying never does.** The README says so plainly,
  with LKH's licence.
- **The interface stays engine-agnostic:** `posum` writes a standard TSPLIB HCP
  problem with fixed edges and reads a standard TSPLIB tour, so any solver
  speaking TSPLIB can replace LKH.
- **LKH runs as a separate process**, never linked into `posum`: it keeps its
  state in global variables and calls `exit()` on errors, which would end the
  whole program. A separate process also lets the memory be measured and ties
  each provenance to the SHA-256 of one binary. The `lkh-rs` crate (0.1.0, July
  2026) was looked at and rejected: it links LKH in-process, builds the
  two-level tree, and bundles LKH's full source in a crate labelled MIT on
  crates.io, which a dependency would pull on every build — redistributing LKH
  indirectly, exactly what this repository does not do.

LKH-3 is distributed "for academic and non-commercial use; the author reserves
all rights". Our use is academic and non-commercial; redistribution is not
granted. Therefore (decided):

- the repository never contains LKH;
- `scripts/build-lkh.sh` (written, commit `4625ef5`) downloads LKH-3.0.14 from
  the author's page, checks the archive's SHA-256
  (`a9a79b5b8632a843d42ed93ab77fa10c5237aca16d5c1bf31287e214e931f3ca`), sets
  `TREE_TYPE = THREE_LEVEL_TREE`, keeps LKH's own compiler flags (LTO, PGO and
  clang measured within noise), maps the build directory to a fixed name so
  that two builds on one machine are byte-identical, smoke-tests the binary on
  `G_2(32)` and installs it;
- LKH, its version and its licence are stated in the README, the papers and
  every manifest whose witness it found;
- a permission request to include the unmodified archive in the Zenodo deposit
  has been drafted for Keld Helsgaun. If granted, the archive joins the deposit.

Parameters that matter, from the measurements: `SUBGRADIENT = NO` (the default
ascent is useless for 0/1 instances and very slow), `OPTIMUM = 0`,
`STOP_AT_OPTIMUM = YES`, `OUTPUT_TOUR_FILE` so a stopped run keeps its best
tour, and chain contraction, which cuts LKH's memory by 42 % at `k = 6`.

### 5.1 Time budget and resumption (decided)

LKH cannot resume inside a run,
but it can start from a tour (`INITIAL_TOUR_FILE`) and writes its best tour at
every improvement (`OUTPUT_TOUR_FILE`). For LKH, resuming means restarting from
the best tour so far: its internal state is lost, the tour's quality is kept. A
reboot during a search of several days costs at most the work since the last
improvement.

- **One run directory per search** holds its whole state: the problem files
  (`.hcp`, `.chains`), the best tour and its cost or the last SAT checkpoint, a
  log, and the provenance so far.
- **`--time-limit`** is wall-clock time over the whole search. When it runs
  out, the search stops cleanly and keeps its best state.
- **Exit codes:** `0` a cycle found and verified; `2` time ran out, the search
  can be resumed; `1` an error.
- **`--resume <dir>`** restarts LKH from the best tour in the run directory.
- **The provenance records every segment**: start, end, duration, seed, and
  the SHA-256 of the tour it started from. A cycle found over three segments
  says so; an interrupted search is reproducible only by replaying the same
  segments, and the provenance states it.
- **A new seed per segment**: the initial seed plus the segment number, so that
  a resumed LKH does not walk the same path again.

### 5.2 Memory guard (decided)

Before starting, `posum search` estimates the memory it needs: the graph, plus
about 600 bytes per vertex for LKH, or per node once chains are contracted. It
refuses to start when the estimate exceeds the available memory, and warns
above half of it. This is the guard the lab notebook recorded as missing since
August; at `k = 7` it avoids finding out after six hours that the machine is
swapping.

**`--force` overrides the refusal (decided).** The estimate is arithmetic, not
a measurement — the `k = 7` preflight was measured at about 1.6 to 1.8 times
the graph estimate — and the user may know better: a machine with large swap,
an estimate known to be pessimistic, a deliberate test. With `--force` the
search starts anyway, the warning is still printed, and the provenance records
that the guard was overridden.

### 5.3 Not planned: a hybrid engine

If LKH stops at a low cost — three defects, as in the first trial on `G_6` —
its paths could be handed to the alternating repair. It would need the repair
engine, which stays in the lab notebook; not planned.

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

**Island bundles (decided).** A result that covers a whole range of `n` — the
finite islands of `k = 4` and `k = 5`, every `n` of which is Hamiltonian — is
one bundle per island and per tree, with one manifest listing every `n` and its
witnesses, instead of one directory and manifest per `n`: two manifests for the
`k = 5` island instead of about 3 900. The exact layout is for
`doc/formats.md`.

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

### 6.3 Manifest (decided)

**One manifest per `(k, n)`, byte-identical in both trees.** Each witness lists
both encodings, so the gate checks that the trees agree by comparing two files,
and each bundle stays self-contained: it holds its own encoding and the
manifest says where the other one is.

```toml
format = "posum-bundle 1"
k = 6
n = 15748961
claims = ["hamiltonian", "minimal"]   # "minimal" requires [bands]
reference = "5f2b4192388caf36..."     # full SHA-256 of the cited witness

[bands]
file = "bands.txt"
sha256 = "015b5be7015d2f6d..."
count = 16                            # bands covering 1..n-1

[[witness]]
id = "5f2b4192388caf36..."            # SHA-256 of the canonical plain list
plain = { file = "5f2b4192/cycle.txt.xz", url = "https://github.com/.../k6-plain.tar" }
bases = { file = "5f2b4192/cycle.bases.xz" }
found = "2026-08-23"
engine = "sat-repair"
provenance = "powercycle 0.1.0, pc-search --max-depth=9, about 27 h 45"

[[witness]]
id = "6c575cc2..."
plain = { file = "6c575cc2/cycle.txt.xz", url = "https://github.com/.../k6-plain.tar" }
bases = { file = "6c575cc2/cycle.bases.xz" }
found = "2026-10-11"
engine = "lkh"
cpu = "AMD Ryzen 9 7940HS"
system = "Linux 7.2.9"

  [witness.lkh]
  version = "3.0.14"
  binary_sha256 = "bb05ca46843ccdbb..."
  tree = "three-level"
  contracted = true
  seed = 1
  seconds = 351
  trials = 1
  parameters = "witness/engines/lkh-3.0.14-default.par"
  parameters_sha256 = "..."
```

A non-minimal value has `claims = ["hamiltonian"]` and no `[bands]`. A witness
whose provenance was not recorded, such as the August `k = 5` island cycles,
says so in plain words:

```toml
[[witness]]
id = "..."
found = "2026-08"
engine = "unknown"
provenance = "archive H5_246877-248831.tar, 21 August 2026; engine and parameters not recorded"
```

Rules (decided):

- **Machine.** Only the processor and the operating system are published, as
  context for the running time. The host name is not: it has no scientific
  value and describes the author's environment.
- **No verification results.** The manifest says what to check — `k`, `n`,
  each SHA-256 — never that it was checked. Verification is a process that
  `posum check` repeats; a recorded "valid" proves nothing.
- **Engine parameters by reference.** A parameter file is stored once, under
  `witness/engines/`, and each witness points to it with its SHA-256, rather
  than repeating it thousands of times across the `k = 5` island.
- **The independent verifiers do not read manifests.** They stay free of any
  TOML parsing — the Rust one has no dependency at all — and take `k`, `n` and
  a file. `posum check` reads the manifest, calls both verifiers and compares the
  SHA-256 itself.
- **The reference witness** is the one cited in papers: the first published
  where one exists, otherwise a witness with recorded provenance.

### 6.3b Formats and names, refined (decided 2026-10-14)

These supersede the examples above where they differ; `doc/formats.md` will
specify them normatively, with valid and invalid test vectors both verifiers
must agree on.

- **Bases are ASCII decimal**, one per line. A `uint8` would cap bases at 255,
  which `k = 2` passes from `n` near 32 000, and saves only 7 % after xz.
- **Bands:** `inf - sup` for a range, `n` for a single value, one space on
  each side of the dash, then the blocking vertex and its degree. Every line
  is an obstruction, so `IMPOSSIBLE` is dropped:

  ```text
  1 - 14         vertex=1 degree=0
  15 - 79        vertex=1 degree=1
  5904 - 9640    vertex=5000 degree=1
  ```

  A bands file never contains an unbounded range: it covers exactly `1..n-1`.
- **Islands** (`posum islands`) use the same notation, with `inf - ...` for the
  final, unbounded island:

  ```text
  9641 - 9999
  18193 - ...
  ```

- **Manifest:** TOML, always written by the tool with fixed serialisation rules
  so that the copies in both trees are byte-identical. It stays short — the
  claims, the certificates, the list of witnesses with their identifiers — and
  **each witness's provenance is a separate file** next to it. Required fields
  are the minimum: format, `k`, `n` or the island range, the claims, and each
  witness's identifier; engine, duration and processor are optional, since
  older witnesses lack them.
- **Names, flattened.** No subdirectory per witness: the provenance file makes
  it pointless, and an island would otherwise nest three levels deep.

  ```text
  witness/plain/k6-n15748961/            witness/plain/k5-n246877-248831/
  ├── manifest.toml                      ├── manifest.toml
  ├── bands.txt                          ├── README.md
  ├── README.md                          ├── n246877/
  ├── 5f2b4192.txt.xz                    │   ├── <sha8>.txt.xz
  ├── 5f2b4192.provenance.toml           │   ├── <sha8>.provenance.toml
  └── ...                                │   └── c458203e.txt.xz
                                         └── ... one directory per n
  ```

  `k<k>-n<n>` for a fixed `n`, `k<k>-n<inf>-<sup>` for an island, `n<n>/`
  inside an island, `<sha8>.txt.xz` or `<sha8>.bases.xz` for a cycle and
  `<sha8>.provenance.toml` for its provenance, the prefix taken from the
  SHA-256 of the canonical plain list. An island bundle has no `bands.txt`: it
  claims Hamiltonicity for every `n`, not minimality.

### 6.3c Provenance of historical witnesses (decided)

The five witnesses published by the lab notebook were found by its SAT + repair
engine; their detailed provenance is in each `final.pcdump.json` (producer,
session, the factors, every repair move) and in each bundle README (the chain
of tours and moves, with timings). That file is not carried over. Each such
witness gets a `<sha8>.provenance.toml` with:

1. the engine, `sat-repair`, and its producer, `pc-search 0.1.0`;
2. a readable summary of the chain, taken from the README, with the timings
   available;
3. a verifiable pointer to the lab notebook: repository, tag
   (`witness-k6-n15748961`, ...), path of the dump, SHA-256 of its decompressed
   content, and its `dump_id`.

**The lab notebook is archived on Software Heritage before 1.0 (decided)**, and
the pointers give its SWHID, a permanent identifier that outlives GitHub. The
SAT + repair engine and the detailed provenance stay reachable even if the
repository is moved or deleted.

### 6.4 Verification (decided)

Each verifier, Rust and Python, independently: reads either encoding, checks
that there are exactly `n` values, none repeated, none missing, that
consecutive sums and the closing sum are `k`-th powers, and that the plain
SHA-256 matches the manifest; checks `bands.txt` band by band. The gate also
checks that the two trees agree: same manifests, bases decoding to the plain
lists, identical `bands.txt`.

### 6.5 Hosting (decided)

**Git is the home of the witnesses.** Measured on the cycles collected so far,
plain xz against the estimated bases size:

| group | cycles | plain, xz | bases, estimated |
|---|---|---|---|
| `k = 3`, `n = 473..800` | 328 | 0.4 MB | negligible |
| `k = 4` finite island | 359 | 4.5 MB | about 0.2 MB |
| `k = 5` finite island | 1 959, twice that after the provenance sweep | 531 MB, about 1 GB after | 15 to 30 MB |
| `k = 5`, other `n` | 5 | 7.4 MB | about 0.3 MB |
| `k = 6` | 4 | about 100 MB | about 6 MB |

- The whole `witness/bases/` tree is in Git: about 50 MB in all.
- `witness/plain/` is in Git for everything except **two groups**: the `k = 5`
  finite island and `k = 6`. In Git they would weigh on every clone forever,
  since history keeps them.
- Those two groups are attached as **assets of the GitHub release** (up to
  2 GB per file, outside Git history, tied to the tag), and the same files go
  into the **Zenodo deposit** of that release, which gives the DOI the OEIS
  and the papers need.
- In the manifest, an offloaded file has a `url` next to its `sha256`;
  `posum bundle` downloads and checks it. Integrity never depends on the host.

**Rejected: Google Drive.** Considered for the large files and dropped: no DOI,
tied to a personal account, links that break when a file moves, download quotas
on popular large files, and no link to a repository version.

**Rejected: Git LFS.** GitHub's free quota, 1 GB of storage and of monthly
bandwidth, is too small for these files and their downloads.

---

## 7. Lean (decided: after 1.0)

Lean is not needed to publish: every claim already rests on certificates that
two independent programs check. Where the mathematics is elementary, Lean would
only move a computation from those programs to its kernel:

- that a Hamiltonian cycle gives every vertex degree at least two — its edges
  are edges of the graph, and each vertex has a predecessor and a successor,
  distinct for `n >= 3` — is a connecting lemma, absent from Mathlib, not a
  result;
- that degrees never decrease as `n` grows — `G_k(n)` is an induced subgraph
  of `G_k(n+1)` — is all the bands certificate uses; checking the 8 to 18
  bands of a given `k` is a finite computation the verifiers already do;
- existence is data, checked by the verifiers; a formal existence proof would
  add display, not trust.

Lean is kept for the two places where it would add real assurance, as a
separate project after 1.0 (decided):

1. **`G_2(31)` is not Hamiltonian**: the only non-Hamiltonicity argument that
   does not reduce to degree — finite but intricate, propagation then two
   branches each closing on two cycles, the kind of reasoning where a mistake
   goes unnoticed.
2. **The islands theory**: the characterisation of `I_k`, the nesting of the
   obstructions, the termination bound `a*`. It is the only general,
   non-trivial reasoning behind the results, so far checked only by
   computations agreeing up to `k = 12`. Once proved, the lower bound follows
   for every `k` from a theorem instead of a certificate per `k`, and the
   degree lemmas come with it. It is the harder of the two.

Dropped: formal existence proofs, at any size.

Statements would use Mathlib's vocabulary (`SimpleGraph.IsHamiltonian`,
`Walk.IsHamiltonianCycle`), so a reader need not trust our definition of a
Hamiltonian cycle; computations would go through Boolean checkers with
soundness lemmas, evaluated by the kernel. Lean and Mathlib pinned, in a
separate CI workflow using Mathlib's cache. Lean is not installed on the
development machine yet. A successful formalisation of the islands theory could
strengthen the results paper, or become a short paper of its own.

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

1. ~~The Git / Zenodo threshold~~ **decided** (section 6.5): Git for all but
   the plain `k = 5` island and `k = 6`, which go to release assets and Zenodo;
   no Google Drive.
2. ~~The manifest fields~~ **decided** (section 6.3): one manifest per
   `(k, n)`, identical in both trees; processor and system but no host name;
   no verification results; engine parameters by reference.
3. ~~Time budget and resumption~~ **decided** (sections 5.1 to 5.3): run
   directories, wall-clock `--time-limit`, exit codes 0 / 2 / 1, `--resume`,
   per-segment provenance and seeds, a memory guard that `--force` overrides,
   the hybrid engine after 1.0.
4. ~~Lean~~ **decided** (section 7): after 1.0, limited to `G_2(31)` and the
   islands theory; no formal existence proofs.
5. ~~The provenance of the 1 955 `k = 5` island cycles~~ **decided:** they are
   recomputed with LKH by a sweep that records full provenance per `n`
   (`scripts/witness-sweep.py` in the lab notebook, commit `d23c0ab`, to
   become a `posum` command). The new cycles become the island's reference
   witnesses; the August cycles are kept as additional witnesses, flagged as
   having no recorded provenance.
6. CI cost of verifying the larger witnesses (Lean's own CI is separate, after
   1.0).
