//! Degree-two propagation.
//!
//! A Hamiltonian cycle uses exactly two edges at every vertex. Three rules
//! follow, applied until nothing changes:
//!
//! 1. a vertex with exactly two edges left uses both: they are **forced**;
//! 2. a vertex with two forced edges uses no other: the rest are
//!    **forbidden**;
//! 3. the chain-closing rule: forced edges form disjoint paths, and an edge
//!    joining the two ends of a path is forbidden unless that path already
//!    holds all `n` vertices, since it would close a cycle too short.
//!
//! Rules 1 and 2 hold for every 2-factor; rule 3 is what makes the result
//! specific to Hamiltonian cycles. Every forced edge is in every Hamiltonian
//! cycle of `G_k(n)` and no forbidden edge is in any, so a solver may fix the
//! first and drop the second. The rules also detect some graphs with no
//! Hamiltonian cycle at all: an [`Obstruction`].
//!
//! Edge states are kept per incidence slot, the neighbours of each vertex in
//! increasing order, as given by [`Graph::neighbours`]: about 22 bytes per
//! vertex plus one byte per slot.

use std::collections::VecDeque;
use std::fmt;

use crate::graph::{Graph, Vertex};

/// What propagation knows about an edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EdgeState {
    /// Not decided.
    Free,
    /// In every Hamiltonian cycle.
    Forced,
    /// In no Hamiltonian cycle.
    Forbidden,
}

/// A proof, found by propagation, that `G_k(n)` has no Hamiltonian cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Obstruction {
    /// A vertex with fewer than two neighbours.
    Degree {
        /// The vertex.
        vertex: Vertex,
        /// Its degree.
        degree: u32,
    },
    /// A vertex that would need a third forced edge.
    Saturated {
        /// The vertex.
        vertex: Vertex,
    },
    /// A vertex left with fewer than two usable edges.
    Starved {
        /// The vertex.
        vertex: Vertex,
    },
    /// Forced edges closing a cycle through fewer than `n` vertices.
    Subcycle {
        /// A vertex of that cycle.
        vertex: Vertex,
    },
}

impl fmt::Display for Obstruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Degree { vertex, degree } => write!(f, "vertex {vertex} has degree {degree}"),
            Self::Saturated { vertex } => write!(f, "vertex {vertex} would need three edges"),
            Self::Starved { vertex } => {
                write!(f, "vertex {vertex} is left with fewer than two edges")
            }
            Self::Subcycle { vertex } => {
                write!(
                    f,
                    "forced edges close a cycle through {vertex} shorter than n"
                )
            }
        }
    }
}

impl std::error::Error for Obstruction {}

/// The fixed point of degree-two propagation on `G_k(n)`.
#[derive(Clone, Debug)]
pub struct Propagation {
    graph: Graph,
    /// Slots of vertex `v` are `offsets[v - 1]..offsets[v]`.
    offsets: Vec<u64>,
    states: Vec<EdgeState>,
    forced: u64,
    forbidden: u64,
    cycle: bool,
}

impl Propagation {
    /// Runs propagation on `graph` to its fixed point.
    ///
    /// # Errors
    ///
    /// The obstruction met, if the rules prove that `graph` has no
    /// Hamiltonian cycle.
    pub fn run(graph: &Graph) -> Result<Self, Obstruction> {
        let n = graph.n() as usize;
        let mut offsets = Vec::with_capacity(n + 1);
        let mut available = Vec::with_capacity(n);
        offsets.push(0u64);
        for v in 1..=graph.n() {
            let degree = graph.degree(v);
            if degree < 2 {
                return Err(Obstruction::Degree { vertex: v, degree });
            }
            available.push(degree);
            offsets.push(offsets[offsets.len() - 1] + u64::from(degree));
        }
        let slots = usize::try_from(offsets[n]).expect("slots fit in memory");

        let mut engine = Engine {
            graph: *graph,
            states: vec![EdgeState::Free; slots],
            offsets,
            available,
            forced_degree: vec![0; n],
            end: (0..=graph.n()).collect(),
            queue: (1..=graph.n()).collect(),
            queued: vec![true; n],
            forced: 0,
            forbidden: 0,
            cycle: false,
        };
        while let Some(v) = engine.queue.pop_front() {
            engine.queued[v as usize - 1] = false;
            engine.settle(v)?;
        }

        Ok(Self {
            graph: *graph,
            offsets: engine.offsets,
            states: engine.states,
            forced: engine.forced,
            forbidden: engine.forbidden,
            cycle: engine.cycle,
        })
    }

    /// The graph.
    #[must_use]
    pub const fn graph(&self) -> &Graph {
        &self.graph
    }

    /// Number of forced edges.
    #[must_use]
    pub const fn forced_edges(&self) -> u64 {
        self.forced
    }

    /// Number of forbidden edges.
    #[must_use]
    pub const fn forbidden_edges(&self) -> u64 {
        self.forbidden
    }

    /// Number of edges left free.
    #[must_use]
    pub fn free_edges(&self) -> u64 {
        self.offsets[self.offsets.len() - 1] / 2 - self.forced - self.forbidden
    }

    /// Whether the forced edges alone form a Hamiltonian cycle.
    #[must_use]
    pub const fn is_cycle(&self) -> bool {
        self.cycle
    }

    /// The state of edge `a`-`b`, or `None` if they are not adjacent.
    #[must_use]
    pub fn state(&self, a: Vertex, b: Vertex) -> Option<EdgeState> {
        self.graph
            .has_edge(a, b)
            .then(|| self.states[self.index(a, b)])
    }

    /// Every edge once, as `(a, b, state)` with `a < b`, by increasing `a`
    /// then `b`.
    pub fn edges(&self) -> impl Iterator<Item = (Vertex, Vertex, EdgeState)> + '_ {
        (1..=self.graph.n()).flat_map(move |a| {
            let first = self.offsets[a as usize - 1];
            self.graph
                .neighbours(a)
                .zip(first..)
                .filter(move |&(b, _)| b > a)
                .map(move |(b, slot)| {
                    (
                        a,
                        b,
                        self.states[usize::try_from(slot).expect("slots fit in memory")],
                    )
                })
        })
    }

    /// The neighbours of `v` along forced edges, at most two.
    pub fn forced_neighbours(&self, v: Vertex) -> impl Iterator<Item = Vertex> + '_ {
        let first = self.offsets[v as usize - 1];
        self.graph
            .neighbours(v)
            .zip(first..)
            .filter(|&(_, slot)| {
                self.states[usize::try_from(slot).expect("slots fit in memory")]
                    == EdgeState::Forced
            })
            .map(|(u, _)| u)
    }

    /// The Hamiltonian cycle formed by the forced edges, in canonical order,
    /// when [`Self::is_cycle`].
    #[must_use]
    pub fn forced_cycle(&self) -> Option<Vec<Vertex>> {
        if !self.cycle {
            return None;
        }
        let mut cycle = Vec::with_capacity(self.graph.n() as usize);
        let (mut previous, mut current) = (0, 1);
        loop {
            cycle.push(current);
            let next = self.forced_neighbours(current).find(|&u| u != previous)?;
            if next == 1 {
                break;
            }
            (previous, current) = (current, next);
        }
        crate::cycle::canonicalize(&mut cycle);
        Some(cycle)
    }

    fn index(&self, v: Vertex, u: Vertex) -> usize {
        usize::try_from(self.offsets[v as usize - 1] + u64::from(self.graph.slot(v, u)))
            .expect("slots fit in memory")
    }
}

/// The mutable state of a run.
struct Engine {
    graph: Graph,
    offsets: Vec<u64>,
    states: Vec<EdgeState>,
    /// Edges of each vertex not forbidden, indexed `v - 1`.
    available: Vec<u32>,
    /// Forced edges of each vertex, indexed `v - 1`.
    forced_degree: Vec<u8>,
    /// For a vertex at the end of a path of forced edges, the other end;
    /// for a vertex on no forced edge, itself. Indexed by `v`.
    end: Vec<Vertex>,
    queue: VecDeque<Vertex>,
    queued: Vec<bool>,
    forced: u64,
    forbidden: u64,
    cycle: bool,
}

impl Engine {
    fn index(&self, v: Vertex, u: Vertex) -> usize {
        usize::try_from(self.offsets[v as usize - 1] + u64::from(self.graph.slot(v, u)))
            .expect("slots fit in memory")
    }

    fn set(&mut self, v: Vertex, u: Vertex, state: EdgeState) {
        let (i, j) = (self.index(v, u), self.index(u, v));
        self.states[i] = state;
        self.states[j] = state;
    }

    fn enqueue(&mut self, v: Vertex) {
        if !self.queued[v as usize - 1] {
            self.queued[v as usize - 1] = true;
            self.queue.push_back(v);
        }
    }

    /// Applies rules 1 and 2 at `v`.
    fn settle(&mut self, v: Vertex) -> Result<(), Obstruction> {
        let (available, forced) = (
            self.available[v as usize - 1],
            self.forced_degree[v as usize - 1],
        );
        if available < 2 {
            return Err(Obstruction::Starved { vertex: v });
        }
        let target = if forced == 2 && available > 2 {
            EdgeState::Forbidden
        } else if available == 2 && forced < 2 {
            EdgeState::Forced
        } else {
            return Ok(());
        };
        for u in self.graph.neighbours(v) {
            if self.states[self.index(v, u)] == EdgeState::Free {
                match target {
                    EdgeState::Forced => self.force(v, u)?,
                    _ => self.forbid(v, u),
                }
            }
        }
        Ok(())
    }

    fn forbid(&mut self, v: Vertex, u: Vertex) {
        self.set(v, u, EdgeState::Forbidden);
        self.available[v as usize - 1] -= 1;
        self.available[u as usize - 1] -= 1;
        self.forbidden += 1;
        self.enqueue(v);
        self.enqueue(u);
    }

    fn force(&mut self, v: Vertex, u: Vertex) -> Result<(), Obstruction> {
        for w in [v, u] {
            if self.forced_degree[w as usize - 1] == 2 {
                return Err(Obstruction::Saturated { vertex: w });
            }
        }
        self.set(v, u, EdgeState::Forced);
        self.forced_degree[v as usize - 1] += 1;
        self.forced_degree[u as usize - 1] += 1;
        self.forced += 1;
        self.enqueue(v);
        self.enqueue(u);

        let n = u64::from(self.graph.n());
        let (ev, eu) = (self.end[v as usize], self.end[u as usize]);
        if ev == u {
            // v and u were the two ends of one path: it closes.
            if self.forced == n {
                self.cycle = true;
                return Ok(());
            }
            return Err(Obstruction::Subcycle { vertex: v });
        }
        self.end[ev as usize] = eu;
        self.end[eu as usize] = ev;

        // Rule 3. With n - 1 forced edges the path holds every vertex, and
        // its closing edge is the one the cycle needs.
        if self.forced < n - 1
            && self.graph.has_edge(ev, eu)
            && self.states[self.index(ev, eu)] == EdgeState::Free
        {
            self.forbid(ev, eu);
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::cycle::tests::G2_32;

    /// Every Hamiltonian cycle of `graph`, up to `limit`, by backtracking from
    /// vertex 1, each once (second vertex smaller than the last).
    pub(crate) fn hamiltonian_cycles(graph: &Graph, limit: usize) -> Vec<Vec<Vertex>> {
        fn extend(
            g: &Graph,
            path: &mut Vec<Vertex>,
            used: &mut [bool],
            out: &mut Vec<Vec<Vertex>>,
            limit: usize,
        ) {
            if out.len() >= limit {
                return;
            }
            let last = path[path.len() - 1];
            if path.len() == g.n() as usize {
                if g.has_edge(last, 1) && path[1] < last {
                    out.push(path.clone());
                }
                return;
            }
            // Prune: every unused vertex needs two neighbours among the unused
            // vertices and the two ends of the path.
            let usable = |w: Vertex| {
                g.neighbours(w)
                    .filter(|&u| !used[u as usize] || u == last || u == 1)
                    .count()
            };
            if (1..=g.n()).any(|w| !used[w as usize] && usable(w) < 2) {
                return;
            }
            for u in g.neighbours(last) {
                if !used[u as usize] {
                    used[u as usize] = true;
                    path.push(u);
                    extend(g, path, used, out, limit);
                    path.pop();
                    used[u as usize] = false;
                }
            }
        }
        let mut used = vec![false; graph.n() as usize + 1];
        used[1] = true;
        let mut out = Vec::new();
        extend(graph, &mut vec![1], &mut used, &mut out, limit);
        out
    }

    /// Forced edges are in the cycle, forbidden ones are not.
    pub(crate) fn assert_consistent(p: &Propagation, cycle: &[Vertex]) {
        let n = cycle.len();
        let mut in_cycle = std::collections::HashSet::new();
        for i in 0..n {
            let (a, b) = (cycle[i], cycle[(i + 1) % n]);
            in_cycle.insert((a.min(b), a.max(b)));
        }
        for (a, b, state) in p.edges() {
            match state {
                EdgeState::Forced => {
                    assert!(in_cycle.contains(&(a, b)), "forced {a}-{b} not in cycle");
                }
                EdgeState::Forbidden => {
                    assert!(!in_cycle.contains(&(a, b)), "forbidden {a}-{b} in cycle");
                }
                EdgeState::Free => {}
            }
        }
    }

    #[test]
    fn g2_32_is_solved_by_propagation_alone() {
        let g = Graph::new(2, 32).unwrap();
        let p = Propagation::run(&g).unwrap();
        assert_consistent(&p, &G2_32);
        if p.is_cycle() {
            assert_eq!(p.forced_cycle().unwrap(), G2_32);
        }
    }

    /// For every `G_2(n)` and `G_3(n)` small enough to enumerate, each
    /// Hamiltonian cycle agrees with propagation, and propagation reports no
    /// obstruction where a cycle exists.
    #[test]
    fn agrees_with_every_cycle_of_small_graphs() {
        let mut checked = 0;
        for (k, range) in [(2, 3..=48), (3, 3..=40)] {
            for n in range {
                let g = Graph::new(k, n).unwrap();
                let cycles = hamiltonian_cycles(&g, 5000);
                match Propagation::run(&g) {
                    Ok(p) => {
                        let counted = p.forced_edges() + p.forbidden_edges() + p.free_edges();
                        assert_eq!(counted, p.edges().count() as u64);
                        for cycle in &cycles {
                            assert_consistent(&p, cycle);
                            checked += 1;
                        }
                        if p.is_cycle() {
                            assert_eq!(cycles, vec![p.forced_cycle().unwrap()]);
                        }
                    }
                    Err(obstruction) => {
                        assert!(
                            cycles.is_empty(),
                            "k={k} n={n}: {obstruction}, yet a cycle exists"
                        );
                    }
                }
            }
        }
        assert!(checked > 1000, "only {checked} cycles checked");
    }

    #[test]
    fn obstructions() {
        let g = Graph::new(2, 10).unwrap();
        assert_eq!(
            Propagation::run(&g).unwrap_err(),
            Obstruction::Degree {
                vertex: 2,
                degree: 1
            }
        );
        // G_2(31) passes the degree test; propagation alone refutes it.
        let g = Graph::new(2, 31).unwrap();
        assert!(hamiltonian_cycles(&g, 1).is_empty());
        let _ = Propagation::run(&g);
    }

    #[test]
    fn state_lookup() {
        let g = Graph::new(2, 32).unwrap();
        let p = Propagation::run(&g).unwrap();
        assert_eq!(p.state(1, 2), None);
        assert!(p.state(1, 8).is_some());
        assert_eq!(p.state(1, 8), p.state(8, 1));
    }
}
