# posum formats — specification

Status: draft 1, normative once release 1.0 is tagged. Format version
`posum-bundle 1`.

This document fixes every file posum writes or reads well enough for two
independent implementations — the Rust and Python verifiers, and posum itself —
to produce and accept exactly the same bytes. The key words MUST, MUST NOT,
SHOULD and MAY are to be read as in RFC 2119. Rationale lives in
[`design.md`](design.md); this document only states the rules.

---

## 1. Common conventions

These rules apply to every text format below unless a section says otherwise.

- **Encoding.** ASCII only. No byte above `0x7F`, no byte order mark.
- **Lines.** Every line, the last one included, MUST end with a single line
  feed (`0x0A`). Carriage returns MUST NOT appear. Empty lines MUST NOT appear
  unless a section allows them.
- **Integers.** Decimal digits only: no sign, no leading zero (`0` itself is
  never a valid value in these formats), no separator, no exponent. Integers
  are unbounded; implementations MUST NOT overflow silently.
- **Whitespace.** Where a format separates fields, it uses exactly one space
  (`0x20`). No tab, no trailing space, no leading space.
- **Hashes.** SHA-256, written as 64 lowercase hexadecimal digits. A `sha8` is
  the first 8 of those digits.
- **Powers.** "`x` is a `k`-th power" means `x = b^k` for an integer `b >= 1`,
  decided in exact integer arithmetic. Floating point MAY be used to guess `b`
  but MUST NOT decide the answer.

### 1.1 The graph

For integers `k >= 2` and `n >= 1`, `G_k(n)` has vertices `1..n`; two distinct
vertices `a` and `b` are adjacent exactly when `a + b` is a `k`-th power. The
degree of `v <= n` is the number of `k`-th powers `p` with `v < p <= v + n`
and `p != 2v`.

---

## 2. Plain cycle files

A plain cycle file lists a Hamiltonian cycle of `G_k(n)`, `n >= 3`.

**Syntax.** Exactly `n` lines, each one integer.

**Validity.** A file is a valid witness for `(k, n)` when:

1. it has exactly `n` lines, each an integer in `1..n`;
2. no integer appears twice, so every integer of `1..n` appears once;
3. every two consecutive lines sum to a `k`-th power;
4. the last and the first line sum to a `k`-th power.

**Canonical form.** A plain cycle file MUST be canonical:

5. the first line is `1`;
6. the second line is smaller than the last line.

Each cycle has exactly one canonical file, so the file identifies the cycle.

**Identity.** The **identity** of a witness is the SHA-256 of its canonical
plain file, uncompressed, final line feed included. It is the same whichever
encoding the witness is stored in.

---

## 3. Bases files

A bases file stores the same cycle as a sequence of `k`-th roots.

**Syntax.** Exactly `n - 1` lines, each one integer `b >= 2`.

**Decoding.** Start from `v_1 = 1`. Line `i`, holding `b_i`, gives the next
vertex `v_(i+1) = b_i^k - v_i`. The decoded cycle is `v_1, ..., v_n`.

**Validity.** A bases file is a valid witness for `(k, n)` when the decoded
sequence:

1. has every `v_i` in `1..n` (a decoded value outside that range, zero or
   negative included, makes the file invalid at that line);
2. repeats no value, so every integer of `1..n` appears once;
3. closes: `v_n + 1` is a `k`-th power.

Consecutive sums are `k`-th powers by construction and need no check. The
decoded sequence starts at 1 by construction; it MUST also satisfy rule 6 of
section 2 (`v_2 < v_n`), so that it decodes to the canonical plain file.

**Identity.** The identity of a bases file is the identity of the plain file it
decodes to (section 2). The SHA-256 of the bases file itself is never an
identity.

---

## 4. Bands files

A bands file is the lower-bound certificate for `n`: for every `m` in
`1..n-1`, it names a vertex of degree at most one in `G_k(m)`, which rules out
a Hamiltonian cycle of `G_k(m)`.

**Syntax.** One band per line, in one of two forms:

```text
<inf> - <sup> vertex=<v> degree=<d>
<m> vertex=<v> degree=<d>
```

The first form covers `inf..sup` with `inf < sup`; the second covers the single
value `m`. The dash is surrounded by exactly one space on each side; `vertex=`
and `degree=` are literal. Example, `k = 4`, `n = 9641`:

```text
1 - 14 vertex=1 degree=0
15 - 79 vertex=1 degree=1
80 - 247 vertex=8 degree=1
5904 - 9640 vertex=5000 degree=1
```

A single value MUST use the second form: `5 - 5` is invalid.

**Validity.** A bands file is a valid certificate for `(k, n)`, `n >= 2`, when:

1. the first band starts at 1, each band starts right after the previous one
   ends, and the last band ends at `n - 1`: the bands cover `1..n-1` exactly,
   without gap or overlap;
2. for each band, `1 <= v <= inf` (the vertex exists in every graph of the
   band) and `d` is 0 or 1;
3. for each band, the degree of `v` is `d` in `G_k(inf)` and in `G_k(sup)`.

Rule 3 settles every `m` in between, since the degree of a fixed vertex never
decreases as `m` grows. A bands file never contains an unbounded band.

**Canonical form.** Consecutive values with the same vertex and degree MUST be
in the same band, so the file for `(k, n)` is unique given the vertex chosen
for each value. posum chooses the smallest vertex of degree at most one.

---

## 5. Islands listings

`posum islands <k>` prints the admissible islands of `G_k`, the maximal ranges
of `n` with every degree at least two. It is an output format, not part of a
bundle.

**Syntax.** One island per line, in increasing order:

```text
<inf> - <sup>
<inf> - ...
```

Every line but the last uses the first form; the last island is always
unbounded and uses `...`. Example, `k = 4`:

```text
9641 - 9999
18193 - ...
```

---

## 6. Compression

Cycle files, plain and bases, are stored compressed with xz (`.xz` streams as
produced by XZ Utils). Readers MUST accept any valid xz stream. Writers SHOULD
use `xz -9e -T1`, which makes the compressed bytes independent of the machine's
core count, but the compressed bytes are never an identity: only the SHA-256 of
the decompressed content is.

Bands files, manifests and provenance files are not compressed.

---

## 7. Bundles

A bundle holds the witnesses of one result. There are two kinds: a **value
bundle**, for one `(k, n)`, and an **island bundle**, for every `n` of one
admissible island.

### 7.1 Trees

Bundles live in two parallel trees, `witness/plain/` and `witness/bases/`. The
same bundle exists in both under the same name; the two copies differ only in
the encoding of their cycle files. Manifests, provenance files, `bands.txt` and
`README.md` MUST be byte-identical in both trees.

### 7.2 Value bundles

Directory name `k<k>-n<n>`, for example `k6-n15748961`.

```text
witness/plain/k6-n15748961/
├── manifest.toml
├── README.md
├── bands.txt                     only if the bundle claims minimality
├── <sha8>.txt.xz                 one per witness
└── <sha8>.provenance.toml        one per witness
```

In `witness/bases/`, each `<sha8>.txt.xz` is replaced by `<sha8>.bases.xz`.

### 7.3 Island bundles

Directory name `k<k>-n<inf>-<sup>`, for example `k5-n246877-248831`, where
`inf..sup` MUST be exactly one finite admissible island of `G_k`.

```text
witness/plain/k5-n246877-248831/
├── manifest.toml
├── README.md
├── n246877/
│   ├── <sha8>.txt.xz
│   └── <sha8>.provenance.toml
├── n246878/
└── ...                           one directory n<n> per value of the island
```

An island bundle claims Hamiltonicity for every `n` of the island and never
contains `bands.txt`.

### 7.4 Names

`<sha8>` is the first 8 digits of the witness's identity (section 2), hence the
same in both trees. Within one directory two witnesses MUST NOT share a
`<sha8>`; should it happen, the writer MUST refuse rather than overwrite.

---

## 8. Manifests

A manifest is a TOML 1.0 document that states what a bundle claims and lists
its witnesses. posum writes it; the independent verifiers never read it.

### 8.1 Value bundle manifest

Keys, in this order:

| key | type | required | meaning |
|---|---|---|---|
| `format` | string | yes | `"posum-bundle 1"` |
| `k` | integer | yes | the exponent, `>= 2` |
| `n` | integer | yes | the number of vertices, `>= 3` |
| `claims` | array of strings | yes | `["hamiltonian"]` or `["hamiltonian", "minimal"]` |
| `reference` | string | yes | identity of the witness cited in papers; MUST be one of the witnesses |

then, if and only if `claims` contains `"minimal"`, a table:

```toml
[bands]
file = "bands.txt"
sha256 = "<sha256 of bands.txt>"
count = <number of bands>
```

then one table per witness, in increasing order of identity:

```toml
[[witness]]
id = "<identity>"
plain = { file = "<sha8>.txt.xz" }
bases = { file = "<sha8>.bases.xz" }
provenance = "<sha8>.provenance.toml"
```

`plain` and `bases` MAY carry a `url`, after `file`, when the file is hosted
outside Git: `{ file = "...", url = "https://..." }`. A file with a `url` is
fetched and then checked like any other; the `url` never replaces the check.

### 8.2 Island bundle manifest

```toml
format = "posum-bundle 1"
k = 5
island = [246877, 248831]
claims = ["hamiltonian"]
witnesses = [
  { n = 246877, id = "<identity>", reference = true },
  { n = 246878, id = "<identity>", reference = true },
  ...
]
```

`island` replaces `n`. `witnesses` lists every witness of every value, one
inline table per line, in increasing order of `n` and then of identity. Every
`n` of the island MUST appear at least once, and exactly one witness per `n`
MUST carry `reference = true`. The files of each witness are, by rule,
`n<n>/<sha8>.txt.xz` (or `.bases.xz`) and `n<n>/<sha8>.provenance.toml`; a
`url`, when needed, applies to an archive of the whole tree and is given in a
top-level `archive` table:

```toml
[archive]
plain = { file = "k5-n246877-248831-plain.tar", url = "https://...", sha256 = "<sha256>" }
```

### 8.3 Canonical serialisation

So that the copies in both trees are byte-identical, a manifest MUST be
serialised exactly as follows:

1. keys in the order given in sections 8.1 and 8.2, top-level keys first, then
   tables in that order;
2. one `key = value` per line, one space on each side of `=`;
3. strings in double quotes, ASCII, no escape sequence (no value needs one);
4. integers in decimal (section 1);
5. arrays of strings and integers on one line: `["a", "b"]`, `[1, 2]`;
6. inline tables with one space inside the braces and after each comma:
   `{ file = "a", url = "b" }`;
7. exactly one empty line before each table header (`[bands]`, `[[witness]]`,
   `[archive]`), none elsewhere; the island `witnesses` array opens with
   `witnesses = [` on its line, one inline table per line indented by two
   spaces and followed by a comma, and closes with `]` alone;
8. no comment, no trailing space, final line feed.

`posum check` MUST re-serialise every manifest it reads and reject one whose
bytes differ from the canonical form.

---

## 9. Provenance files

One per witness, `<sha8>.provenance.toml`, TOML 1.0, serialised under the rules
of section 8.3. It records how the witness was found; no field is ever a
verification result.

| key | type | required | meaning |
|---|---|---|---|
| `id` | string | yes | the witness's identity |
| `k`, `n` | integers | yes | as in the manifest |
| `found` | string | yes | date and time found, ISO 8601, UTC, to the second |
| `posum` | string | yes | posum version, e.g. `"1.0.0"` |
| `engine` | string | yes | `"lkh"` |
| `cpu` | string | no | processor model |
| `system` | string | no | operating system and kernel |

then a table describing the engine run:

```toml
[lkh]
version = "3.0.14"
binary_sha256 = "<sha256 of the LKH binary>"
tree = "three-level"
contracted = true
seed = 1
segments = 1
seconds = 351
trials = 1
parameters = "witness/engines/<file>.par"
parameters_sha256 = "<sha256>"
```

`segments` counts the runs of a resumed search; when it exceeds 1, one
`[[lkh.segment]]` table per segment follows, with `seed`, `started`, `seconds`
and `initial_tour_sha256`. The host name MUST NOT be recorded.

Parameter files live once under `witness/engines/` and are referenced by path
and SHA-256.

---

## 10. Verification

### 10.1 Independent verifiers

Each of the two verifiers takes `k`, `n` and one file, and decides:

- a plain cycle file: sections 2, rules 1 to 6;
- a bases file: section 3;
- a bands file: section 4.

They print a verdict and exit with status 0 when the file is valid, 1 when it is
not, 2 on a usage error. They read nothing else: no manifest, no provenance, no
network.

### 10.2 posum check

For every bundle, in both trees, `posum check` MUST:

1. re-serialise the manifest and compare it byte for byte (section 8.3);
2. check that the directory name agrees with `k`, `n` or `island`;
3. for each witness, decompress the cycle file, have both verifiers accept it,
   and compare its identity with the manifest's `id`;
4. if `minimal` is claimed, check the SHA-256 and the count of `bands.txt` and
   have both verifiers accept it;
5. check every provenance file against section 9;
6. check that the two trees agree: same file names, byte-identical manifests,
   provenance files, `bands.txt` and `README.md`, and every bases file decoding
   to the plain file of the same identity.

---

## 11. Test vectors

`tests/vectors/` holds small files with known verdicts, which both verifiers and
posum MUST agree on. Each vector is a file plus a line in
`tests/vectors/index.tsv`:

```text
<file>	<kind>	<k>	<n>	<verdict>	<reason>
```

with `kind` among `plain`, `bases`, `bands`, `manifest`, and `verdict` either
`valid` or `invalid`. The first set, to be written with the verifiers:

| file | kind | k | n | verdict | reason |
|---|---|---|---|---|---|
| `g2-32.txt` | plain | 2 | 32 | valid | the unique cycle |
| `g2-32.bases` | bases | 2 | 32 | valid | the same cycle |
| `g3-473-bands.txt` | bands | 3 | 473 | valid | the lower bound |
| `g4-9641-bands.txt` | bands | 4 | 9641 | valid | the lower bound |
| `dup.txt` | plain | 2 | 32 | invalid | a vertex repeated |
| `short.txt` | plain | 2 | 32 | invalid | 31 lines |
| `nonedge.txt` | plain | 2 | 32 | invalid | two neighbours not summing to a square |
| `open.txt` | plain | 2 | 32 | invalid | last and first not summing to a square |
| `rotated.txt` | plain | 2 | 32 | invalid | valid cycle, not starting at 1 |
| `reversed.txt` | plain | 2 | 32 | invalid | valid cycle, second line larger than the last |
| `crlf.txt` | plain | 2 | 32 | invalid | carriage returns |
| `nofinal.txt` | plain | 2 | 32 | invalid | no final line feed |
| `leading0.txt` | plain | 2 | 32 | invalid | an integer with a leading zero |
| `open.bases` | bases | 2 | 32 | invalid | does not close |
| `out.bases` | bases | 2 | 32 | invalid | decodes outside `1..n` |
| `gap-bands.txt` | bands | 4 | 9641 | invalid | a missing value |
| `long-bands.txt` | bands | 4 | 9641 | invalid | a band past its vertex's degree |
| `dash-bands.txt` | bands | 4 | 9641 | invalid | a single value written `m - m` |
| `noncanon.toml` | manifest | 6 | 15748961 | invalid | keys out of order |

---

## 12. Versioning

`format = "posum-bundle 1"` names this specification. Any change a reader of
version 1 could misread MUST change the number. Adding an optional key that
readers may ignore does not. The verifiers do not depend on the manifest
format, only on sections 1 to 4.
