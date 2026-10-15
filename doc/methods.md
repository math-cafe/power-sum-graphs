# Methods

The algorithms behind posum, in the order a search uses them. Code references
are to `crates/posum-core` and `crates/posum-lkh`.

---

## 1. The graph by formula (`graph.rs`)

The neighbours of `v` in `G_k(n)` are the `b^k - v` for the bases `b` with
`v < b^k <= v + n`, except `b^k = 2v`, which would be a loop. Hence

```text
deg(v) = floor((v+n)^(1/k)) - floor(v^(1/k)) - [2v is a k-th power]
```

and nothing needs storing: listing neighbours, testing an edge or counting a
degree costs a few `k`-th roots. Roots are guessed in floating point and
decided in exact integer arithmetic. An edge's position among the neighbours of
`v` — its *slot* — is also a formula, which lets propagation keep one byte per
edge end and no adjacency list.

## 2. The chain of blocking vertices (`islands.rs`)

A vertex *blocks* `G_k(m)` when its degree is below two; then `G_k(m)` has no
Hamiltonian cycle. At fixed `m`, the degree only drops along `v` where `v`
reaches a power (which leaves the window `(v, v+m]`) or a loop position
`a^k/2`. The smallest blocking vertex is therefore always `1`, some `a^k` or
some `a^k/2`. Beyond the safe base `a* = ceil(3 / (2^(1/k) - 1))` three powers
lie in `(x, 2x]` and nothing blocks, so `O(k)` candidates suffice.

The degree of a fixed vertex never decreases as `m` grows. Following the
smallest blocking vertex `x` from `m = 1` therefore splits `m >= 1` into:

- **bands**: `x` holds from the `m` where it starts blocking until
  `p2 - x - 1`, where `p1 < p2` are the first two powers above `x` other than
  `2x`; its degree is 0 until `p1 - x - 1`, then 1;
- **admissible islands**: ranges where no candidate blocks. The next blocker
  can only be a vertex entering the graph, the next candidate, which bounds
  each island.

The last island is unbounded. The whole chain, about `2k` segments, is computed
once in exact `u128` arithmetic. A **bands certificate** for `n` is its bands
up to `n - 1`, written one per line (`doc/formats.md` section 4); it exists
exactly when no island starts below `n`. Each band is checkable at its two
ends, by monotonicity.

## 3. Degree-two propagation (`propagation.rs`)

A Hamiltonian cycle uses exactly two edges at every vertex. Three rules,
applied with a work queue until nothing changes:

1. a vertex with exactly two edges left uses both: they are **forced**;
2. a vertex with two forced edges uses no other: the rest are **forbidden**;
3. **chain closing**: forced edges form disjoint paths; an edge joining the two
   ends of one path is forbidden unless the path already holds all `n`
   vertices, since it would close a cycle too short.

Rules 1 and 2 hold for every 2-factor; rule 3 makes the result specific to
Hamiltonian cycles. Path ends are tracked with one pointer per vertex, joined in
constant time when an edge is forced. The rules can also prove that no cycle
exists — a vertex needing a third forced edge, a vertex left with one edge, a
forced subcycle — and on `G_2(32)` they force most of the unique cycle.

Memory: one byte per edge end, plus about 25 bytes per vertex. `G_6(15748961)`
propagates in 3 s; 9 756 661 of its 26 281 979 edges end up forced and
2 480 898 forbidden.

## 4. Chain contraction (`contraction.rs`)

A vertex inside a chain of forced edges has nothing left to choose. Each chain
is replaced by its two ends joined by one fixed edge, the **link**; its interior
disappears. The solver sees the **nodes** — chain ends and vertices on no forced
edge, numbered in increasing vertex order — joined by the free edges and the
links. Rule 2 guarantees that no free edge touches an interior vertex, rule 3
that none joins the two ends of one chain. A tour of the nodes through every
link expands back to a Hamiltonian cycle of `G_k(n)` by putting each chain's
interior between its ends. At `k = 6`, 2 821 104 chains leave 8 813 404 nodes,
56 % of `n`, and LKH's memory falls in proportion.

## 5. LKH (`posum-lkh`)

The contracted problem is written as a TSPLIB HCP instance — Hamiltonian cycle
— with the links in `FIXED_EDGES_SECTION`, and solved by LKH-3 as a separate
process. The parameters (`witness/engines/lkh-posum-1.par`):

- `OPTIMUM = 0`, `STOP_AT_OPTIMUM = YES`: a Hamiltonian cycle is a tour of
  cost 0, and LKH stops at the first;
- `SUBGRADIENT = NO`: every edge costs 0, so the default ascent has nothing to
  learn, and at `k = 5` it turned a 5 s run into ten minutes;
- `RUNS = 1`: one run per **segment**.

A search runs segments until a tour of cost 0 is found: each with its own seed,
each starting from the best tour so far (`INITIAL_TOUR_FILE`), LKH writing its
best tour at every improvement. Under a time limit, posum stops LKH past the
deadline and the run directory can be resumed. After 100 consecutive segments
without improvement the search gives up: LKH cannot prove that no cycle exists,
and `G_2(31)`, admissible yet not Hamiltonian, would loop forever.

The tour found is expanded, checked as a Hamiltonian cycle of `G_k(n)`, and put
in canonical order.

LKH is built by `scripts/build-lkh.sh` with `TREE_TYPE = THREE_LEVEL_TREE`,
3.4 times faster than the default on `G_6(15748961)`; the build is
byte-reproducible on one machine. `G_6(15748961)` takes one trial, 6 min 49 and
6.0 GB on the development machine.

## 6. Canonical form and encodings (`cycle.rs`, `bases.rs`)

A cycle has `2n` sequences, one per starting point and direction. The
**canonical** one starts at 1 and goes in the direction where the second vertex
is smaller than the last. Its plain list, one vertex per line, identifies the
witness by its SHA-256.

The **bases** encoding writes each step `v -> w` as the base `b` of
`v + w = b^k`, `n - 1` lines, the closing step implied. Decoding starts at 1 and
applies `w = b^k - v`, without the graph; every sum is a `k`-th power by
construction, so a verifier only checks that the `n` vertices are distinct and
in range, and that the last one closes. Under xz, bases are 15 to 35 times
smaller than plain lists, which is why the repository stores them and derives
the plain lists.
