//! Chains of forced edges contracted to their ends, and expanded back.
//!
//! After propagation, the forced edges form disjoint paths, the chains. A
//! vertex inside a chain has both its cycle edges decided, so only the chain's
//! two ends matter to a solver: each chain becomes one fixed edge between its
//! ends, and its interior disappears. The solver sees the **nodes** — chain
//! ends and vertices on no forced edge, numbered from 1 in increasing vertex
//! order — joined by the free edges and by one link per chain. At `k = 6` this
//! cuts LKH's memory by 42 %.
//!
//! Every free edge joins two nodes, since rule 2 of propagation forbids the
//! free edges of an interior vertex, and none joins the two ends of one
//! chain, since rule 3 forbids those. A tour of the nodes through every link
//! expands to a Hamiltonian cycle of `G_k(n)` by putting each chain's interior
//! back between its ends.

use std::fmt;

use crate::graph::Vertex;
use crate::propagation::{EdgeState, Propagation};

/// Why a contraction or an expansion failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractionError {
    /// Propagation already forced a Hamiltonian cycle: there is nothing to
    /// contract or search.
    Solved,
    /// A node tour of the wrong length.
    Length {
        /// Nodes given.
        found: usize,
        /// Nodes of the contraction.
        expected: usize,
    },
    /// A value that is not a node, or a node listed twice.
    Node(u32),
    /// A chain whose two ends are not consecutive in the tour.
    Link(u32, u32),
}

impl fmt::Display for ContractionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solved => write!(f, "propagation alone forces a Hamiltonian cycle"),
            Self::Length { found, expected } => write!(f, "{found} nodes, expected {expected}"),
            Self::Node(x) => write!(f, "node {x} is out of range or repeated"),
            Self::Link(x, y) => {
                write!(f, "nodes {x} and {y} end one chain but are not consecutive")
            }
        }
    }
}

impl std::error::Error for ContractionError {}

/// The contraction of the forced chains of a [`Propagation`].
#[derive(Clone, Debug)]
pub struct Contraction {
    /// Node of each vertex, 0 for an interior vertex; index 0 unused.
    node_of: Vec<u32>,
    /// Vertex of each node; index 0 unused.
    vertex_of: Vec<Vertex>,
    /// The other end of each chain end's chain, 0 for a node on no chain.
    partner: Vec<u32>,
    /// One link per chain, between the nodes of its ends, first end smaller.
    links: Vec<(u32, u32)>,
    /// Chains with an interior, end to end from their smaller end, flattened.
    chain_vertices: Vec<Vertex>,
    /// Where each such chain starts in `chain_vertices`, plus a final bound.
    chain_starts: Vec<usize>,
    /// For a node ending a chain with an interior: `2i + 1` if it is the first
    /// vertex of chain `i`, `2i + 2` if the last; 0 otherwise.
    chain_of: Vec<u32>,
}

impl Contraction {
    /// Contracts the forced chains of `propagation`.
    ///
    /// # Errors
    ///
    /// [`ContractionError::Solved`] when propagation forced the whole cycle.
    ///
    /// # Panics
    ///
    /// When `n` does not fit in memory as `usize`, which `Vertex` excludes.
    pub fn new(propagation: &Propagation) -> Result<Self, ContractionError> {
        if propagation.is_cycle() {
            return Err(ContractionError::Solved);
        }
        let n = propagation.graph().n();
        let forced = |v: Vertex| propagation.forced_neighbours(v);

        let mut node_of = vec![0u32; n as usize + 1];
        let mut vertex_of = vec![0];
        for v in 1..=n {
            if forced(v).nth(1).is_none() {
                vertex_of.push(v);
                node_of[v as usize] = u32::try_from(vertex_of.len() - 1).expect("nodes fit in u32");
            }
        }

        let mut partner = vec![0u32; vertex_of.len()];
        let mut chain_of = vec![0u32; vertex_of.len()];
        let mut links = Vec::new();
        let mut chain_vertices = Vec::new();
        let mut chain_starts = Vec::new();

        // Walk each chain from its smaller end, met first in vertex order.
        for v in 1..=n {
            let x = node_of[v as usize];
            if x == 0 || partner[x as usize] != 0 {
                continue;
            }
            let Some(mut current) = forced(v).next() else {
                continue;
            };
            let start = chain_vertices.len();
            chain_vertices.push(v);
            let mut previous = v;
            while node_of[current as usize] == 0 {
                chain_vertices.push(current);
                let next = forced(current)
                    .find(|&u| u != previous)
                    .expect("interior vertices have two forced edges");
                (previous, current) = (current, next);
            }
            chain_vertices.push(current);
            let y = node_of[current as usize];
            partner[x as usize] = y;
            partner[y as usize] = x;
            links.push((x, y));
            if chain_vertices.len() - start > 2 {
                let i = u32::try_from(chain_starts.len()).expect("chains fit in u32");
                chain_of[x as usize] = 2 * i + 1;
                chain_of[y as usize] = 2 * i + 2;
                chain_starts.push(start);
            } else {
                chain_vertices.truncate(start);
            }
        }
        chain_starts.push(chain_vertices.len());

        Ok(Self {
            node_of,
            vertex_of,
            partner,
            links,
            chain_vertices,
            chain_starts,
            chain_of,
        })
    }

    /// Number of nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.vertex_of.len() - 1
    }

    /// Number of chains, one link each.
    #[must_use]
    pub fn chain_count(&self) -> usize {
        self.links.len()
    }

    /// The node of `v`, or `None` for a vertex inside a chain.
    #[must_use]
    pub fn node(&self, v: Vertex) -> Option<u32> {
        Some(self.node_of[v as usize]).filter(|&x| x != 0)
    }

    /// The vertex of node `x`.
    #[must_use]
    pub fn vertex(&self, x: u32) -> Vertex {
        self.vertex_of[x as usize]
    }

    /// The other end of the chain ending at node `x`, if any.
    #[must_use]
    pub fn partner(&self, x: u32) -> Option<u32> {
        Some(self.partner[x as usize]).filter(|&y| y != 0)
    }

    /// The links, one per chain, as node pairs: the solver's fixed edges.
    #[must_use]
    pub fn links(&self) -> &[(u32, u32)] {
        &self.links
    }

    /// The chains with at least one interior vertex, end to end.
    pub fn chains(&self) -> impl Iterator<Item = &[Vertex]> {
        self.chain_starts
            .windows(2)
            .map(|w| &self.chain_vertices[w[0]..w[1]])
    }

    /// The free edges of `propagation`, as node pairs.
    ///
    /// # Panics
    ///
    /// When a free edge touches an interior vertex or closes a chain, which
    /// propagation rules out; `propagation` must be the one contracted.
    pub fn free_edges<'a>(
        &'a self,
        propagation: &'a Propagation,
    ) -> impl Iterator<Item = (u32, u32)> + 'a {
        propagation
            .edges()
            .filter(|&(_, _, state)| state == EdgeState::Free)
            .map(|(a, b, _)| {
                let (x, y) = (self.node(a), self.node(b));
                let (Some(x), Some(y)) = (x, y) else {
                    panic!("free edge {a}-{b} at an interior vertex")
                };
                assert_ne!(self.partner(x), Some(y), "free edge {a}-{b} closes a chain");
                (x, y)
            })
    }

    /// The Hamiltonian cycle of `G_k(n)` given by a tour of the nodes, which
    /// must take every link. Not canonical, not checked against the graph.
    ///
    /// # Errors
    ///
    /// When `tour` is not a permutation of the nodes, or skips a link.
    pub fn expand(&self, tour: &[u32]) -> Result<Vec<Vertex>, ContractionError> {
        let m = self.node_count();
        if tour.len() != m {
            return Err(ContractionError::Length {
                found: tour.len(),
                expected: m,
            });
        }
        let mut seen = vec![false; m + 1];
        for &x in tour {
            if x == 0 || x as usize > m || seen[x as usize] {
                return Err(ContractionError::Node(x));
            }
            seen[x as usize] = true;
        }

        let mut cycle = Vec::with_capacity(self.node_of.len() - 1);
        for (i, &x) in tour.iter().enumerate() {
            let (before, after) = (tour[(i + m - 1) % m], tour[(i + 1) % m]);
            cycle.push(self.vertex(x));
            let Some(y) = self.partner(x) else { continue };
            if y == after {
                if let Some(interior) = self.interior_from(x) {
                    cycle.extend(interior);
                }
            } else if y != before {
                return Err(ContractionError::Link(x, y));
            }
        }
        Ok(cycle)
    }

    /// The interior of the chain ending at `x`, read from `x`.
    fn interior_from(&self, x: u32) -> Option<Box<dyn Iterator<Item = Vertex> + '_>> {
        let code = self.chain_of[x as usize];
        if code == 0 {
            return None;
        }
        let i = ((code - 1) / 2) as usize;
        let chain = &self.chain_vertices[self.chain_starts[i]..self.chain_starts[i + 1]];
        let interior = &chain[1..chain.len() - 1];
        Some(if code % 2 == 1 {
            Box::new(interior.iter().copied())
        } else {
            Box::new(interior.iter().rev().copied())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cycle;
    use crate::graph::Graph;
    use crate::propagation::tests::hamiltonian_cycles;

    /// The node tour a solver would return for `cycle`.
    fn contract_cycle(c: &Contraction, cycle: &[Vertex]) -> Vec<u32> {
        cycle.iter().filter_map(|&v| c.node(v)).collect()
    }

    #[test]
    fn every_cycle_contracts_and_expands_back() {
        let mut checked = 0;
        for (k, range) in [(2, 33..=48)] {
            for n in range {
                let g = Graph::new(k, n).unwrap();
                let Ok(p) = Propagation::run(&g) else {
                    continue;
                };
                let Ok(c) = Contraction::new(&p) else {
                    continue;
                };
                let interior: usize = c.chains().map(|chain| chain.len() - 2).sum();
                assert_eq!(c.node_count() + interior, n as usize);
                assert_eq!(c.chain_count() as u64, p.forced_edges() - interior as u64);
                assert!(c.free_edges(&p).all(|(x, y)| x != y));
                for original in hamiltonian_cycles(&g, 500) {
                    let tour = contract_cycle(&c, &original);
                    for reflect in [false, true] {
                        let mut tour = tour.clone();
                        if reflect {
                            tour.reverse();
                        }
                        let mut expanded = c.expand(&tour).unwrap();
                        assert_eq!(cycle::check(&g, &expanded), Ok(()));
                        cycle::canonicalize(&mut expanded);
                        assert_eq!(expanded, original);
                    }
                    checked += 1;
                }
            }
        }
        assert!(checked > 500, "only {checked} cycles checked");
    }

    #[test]
    fn broken_tours_are_refused() {
        let g = Graph::new(2, 48).unwrap();
        let p = Propagation::run(&g).unwrap();
        let c = Contraction::new(&p).unwrap();
        let original = hamiltonian_cycles(&g, 1).pop().unwrap();
        let tour = contract_cycle(&c, &original);
        let m = tour.len();
        assert!(matches!(
            c.expand(&tour[1..]),
            Err(ContractionError::Length { .. })
        ));
        let mut repeated = tour.clone();
        repeated[1] = repeated[0];
        assert!(matches!(
            c.expand(&repeated),
            Err(ContractionError::Node(_))
        ));
        // Moving one end of a link elsewhere breaks it.
        let (x, y) = c.links()[0];
        let mut broken = tour.clone();
        let i = broken.iter().position(|&z| z == x).unwrap();
        let j = broken.iter().position(|&z| z == y).unwrap();
        assert!((i + 1) % m == j || (j + 1) % m == i);
        let far = (i + m / 2) % m;
        broken.swap(i, far);
        assert!(matches!(c.expand(&broken), Err(ContractionError::Link(..))));
    }

    #[test]
    fn a_solved_graph_is_not_contracted() {
        let g = Graph::new(2, 32).unwrap();
        let p = Propagation::run(&g).unwrap();
        if p.is_cycle() {
            assert_eq!(Contraction::new(&p).unwrap_err(), ContractionError::Solved);
        }
    }
}
