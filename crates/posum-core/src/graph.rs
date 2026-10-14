//! `G_k(n)` by formula.
//!
//! No edge is stored. The neighbours of `v` are the `b^k - v` for the bases
//! `b` with `v < b^k <= v + n`, except `b^k = 2v`, the loop: listing them,
//! testing an edge or counting a degree costs a few root computations.

use std::fmt;

use crate::power::{base_of, root_u64};

/// A vertex of `G_k(n)`, in `1..=n`.
///
/// `u32` bounds `n` by `2^32 - 1`, far beyond what a solver can take: the
/// admissible island of `k = 7` starts at 587 761 723. Islands and bands,
/// which need no graph, are computed in `u128` by [`crate::islands`].
pub type Vertex = u32;

/// The power-sum graph `G_k(n)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Graph {
    k: u32,
    n: Vertex,
}

/// Parameters no `G_k(n)` of this crate can have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphError {
    /// `k < 2`.
    Exponent(u32),
    /// `n = 0` or `n > 2^32 - 1`.
    Order(u128),
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exponent(k) => write!(f, "k = {k}: the exponent must be at least 2"),
            Self::Order(n) => write!(f, "n = {n}: the order must be in 1..={}", Vertex::MAX),
        }
    }
}

impl std::error::Error for GraphError {}

impl Graph {
    /// `G_k(n)`.
    ///
    /// # Errors
    ///
    /// When `k < 2`, or `n` is outside `1..=2^32 - 1`.
    pub fn new(k: u32, n: u128) -> Result<Self, GraphError> {
        if k < 2 {
            return Err(GraphError::Exponent(k));
        }
        match Vertex::try_from(n) {
            Ok(n) if n >= 1 => Ok(Self { k, n }),
            _ => Err(GraphError::Order(n)),
        }
    }

    /// The exponent `k`.
    #[must_use]
    pub const fn k(&self) -> u32 {
        self.k
    }

    /// The order `n`.
    #[must_use]
    pub const fn n(&self) -> Vertex {
        self.n
    }

    /// Whether `v` is a vertex, `1 <= v <= n`.
    #[must_use]
    pub const fn contains(&self, v: Vertex) -> bool {
        v >= 1 && v <= self.n
    }

    /// The degree of `v`: the `k`-th powers in `(v, v + n]` other than `2v`.
    ///
    /// # Panics
    ///
    /// When `v` is not a vertex.
    #[must_use]
    pub fn degree(&self, v: Vertex) -> u32 {
        assert!(
            self.contains(v),
            "{v} is not a vertex of G_{}({})",
            self.k,
            self.n
        );
        let (v, n) = (u64::from(v), u64::from(self.n));
        let count = root_u64(v + n, self.k) - root_u64(v, self.k);
        let count = count - u64::from(base_of(2 * v, self.k).is_some());
        u32::try_from(count).expect("a degree is at most n")
    }

    /// The neighbours of `v`, in increasing order.
    ///
    /// # Panics
    ///
    /// When `v` is not a vertex.
    #[must_use]
    pub fn neighbours(&self, v: Vertex) -> Neighbours {
        assert!(
            self.contains(v),
            "{v} is not a vertex of G_{}({})",
            self.k,
            self.n
        );
        let (v64, n) = (u64::from(v), u64::from(self.n));
        Neighbours {
            k: self.k,
            v: v64,
            base: root_u64(v64, self.k) + 1,
            last: root_u64(v64 + n, self.k),
        }
    }

    /// Whether `a` and `b` are adjacent.
    #[must_use]
    pub fn has_edge(&self, a: Vertex, b: Vertex) -> bool {
        self.contains(a)
            && self.contains(b)
            && a != b
            && base_of(u64::from(a) + u64::from(b), self.k).is_some()
    }

    /// The position of `u` in the increasing list of neighbours of `v`.
    ///
    /// `u` must be a neighbour of `v`.
    pub(crate) fn slot(&self, v: Vertex, u: Vertex) -> u32 {
        let (v64, u64_) = (u64::from(v), u64::from(u));
        let base = root_u64(v64 + u64_, self.k);
        let mut slot = base - root_u64(v64, self.k) - 1;
        if base_of(2 * v64, self.k).is_some_and(|c| c < base) {
            slot -= 1;
        }
        u32::try_from(slot).expect("a slot is below the degree")
    }
}

/// The neighbours of a vertex, see [`Graph::neighbours`].
#[derive(Clone, Debug)]
pub struct Neighbours {
    k: u32,
    v: u64,
    base: u64,
    last: u64,
}

impl Iterator for Neighbours {
    type Item = Vertex;

    fn next(&mut self) -> Option<Vertex> {
        while self.base <= self.last {
            let power = self.base.pow(self.k);
            self.base += 1;
            if power != 2 * self.v {
                return Some(Vertex::try_from(power - self.v).expect("a neighbour is at most n"));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brute_neighbours(k: u32, n: Vertex, v: Vertex) -> Vec<Vertex> {
        (1..=n)
            .filter(|&u| u != v && crate::power::is_power(u128::from(u + v), k))
            .collect()
    }

    #[test]
    fn rejects_invalid_parameters() {
        assert_eq!(Graph::new(1, 10), Err(GraphError::Exponent(1)));
        assert_eq!(Graph::new(2, 0), Err(GraphError::Order(0)));
        assert_eq!(Graph::new(2, 1 << 32), Err(GraphError::Order(1 << 32)));
        assert!(Graph::new(2, u128::from(u32::MAX)).is_ok());
    }

    #[test]
    fn formula_agrees_with_brute_force() {
        for k in 2..=5 {
            for n in [1, 2, 7, 31, 32, 100, 473] {
                let g = Graph::new(k, u128::from(n)).unwrap();
                for v in 1..=n {
                    let expected = brute_neighbours(k, n, v);
                    let listed: Vec<_> = g.neighbours(v).collect();
                    assert_eq!(listed, expected, "k={k} n={n} v={v}");
                    assert_eq!(g.degree(v) as usize, expected.len());
                    for (slot, &u) in expected.iter().enumerate() {
                        assert!(g.has_edge(v, u) && g.has_edge(u, v));
                        assert_eq!(g.slot(v, u) as usize, slot);
                    }
                }
            }
        }
    }

    #[test]
    fn g2_10() {
        let g = Graph::new(2, 10).unwrap();
        assert_eq!(g.neighbours(1).collect::<Vec<_>>(), [3, 8]);
        assert_eq!(g.neighbours(4).collect::<Vec<_>>(), [5]);
        assert_eq!(g.degree(4), 1);
        assert!(!g.has_edge(2, 2) && !g.has_edge(0, 4) && !g.has_edge(1, 2));
    }

    /// Figures of the lab notebook's stored graph, for `G_5(490463)`.
    #[test]
    fn g5_490463_as_in_the_lab_notebook() {
        let g = Graph::new(5, 490_463).unwrap();
        let degrees: Vec<u32> = (1..=g.n()).map(|v| g.degree(v)).collect();
        let total: u64 = degrees.iter().map(|&d| u64::from(d)).sum();
        assert_eq!(total / 2, 833_318);
        assert_eq!(degrees.iter().min(), Some(&2));
        assert_eq!(degrees.iter().max(), Some(&12));
        assert_eq!(degrees.iter().filter(|&&d| d == 2).count(), 139_252);
    }
}
