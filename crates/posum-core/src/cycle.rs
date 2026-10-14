//! Hamiltonian cycles: check and canonical order.
//!
//! A cycle is a sequence of the `n` vertices, each adjacent to the next and
//! the last to the first. Its canonical order, `doc/formats.md` section 2,
//! starts at 1 and takes the direction in which the second vertex is smaller
//! than the last, so that every cycle has exactly one canonical sequence.

use std::fmt;

use crate::graph::{Graph, Vertex};

/// Why a sequence is not a Hamiltonian cycle of `G_k(n)`, or not canonical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CycleError {
    /// Not `n` vertices.
    Length {
        /// Vertices given.
        found: usize,
        /// `n`.
        expected: Vertex,
    },
    /// A value outside `1..=n`, at a 0-based position.
    Range {
        /// Position in the sequence.
        position: usize,
        /// The value.
        value: u128,
    },
    /// A vertex listed twice.
    Repeated(Vertex),
    /// Two consecutive vertices, or the last and the first, not adjacent.
    NotAdjacent(Vertex, Vertex),
    /// A valid cycle, not in canonical order.
    NotCanonical,
}

impl fmt::Display for CycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { found, expected } => write!(f, "{found} vertices, expected {expected}"),
            Self::Range { position, value } => {
                write!(
                    f,
                    "value {value} at position {} is not a vertex",
                    position + 1
                )
            }
            Self::Repeated(v) => write!(f, "vertex {v} appears twice"),
            Self::NotAdjacent(a, b) => write!(f, "{a} + {b} is not a k-th power"),
            Self::NotCanonical => {
                write!(
                    f,
                    "not canonical: it must start at 1, its second vertex smaller than the last"
                )
            }
        }
    }
}

impl std::error::Error for CycleError {}

/// Checks that `cycle` is a Hamiltonian cycle of `graph`, in any order.
///
/// # Errors
///
/// The first defect found, in the order: length, range and repetition while
/// reading, then adjacency.
pub fn check(graph: &Graph, cycle: &[Vertex]) -> Result<(), CycleError> {
    let n = graph.n();
    if cycle.len() != n as usize {
        return Err(CycleError::Length {
            found: cycle.len(),
            expected: n,
        });
    }
    let mut seen = vec![0u64; (n as usize).div_ceil(64) + 1];
    for (position, &v) in cycle.iter().enumerate() {
        if !graph.contains(v) {
            return Err(CycleError::Range {
                position,
                value: u128::from(v),
            });
        }
        let (word, bit) = (v as usize / 64, v % 64);
        if seen[word] >> bit & 1 == 1 {
            return Err(CycleError::Repeated(v));
        }
        seen[word] |= 1 << bit;
    }
    let closing = std::iter::once((cycle[n as usize - 1], cycle[0]));
    for (a, b) in cycle.windows(2).map(|w| (w[0], w[1])).chain(closing) {
        if !graph.has_edge(a, b) {
            return Err(CycleError::NotAdjacent(a, b));
        }
    }
    Ok(())
}

/// Checks that `cycle` is a Hamiltonian cycle of `graph` in canonical order.
///
/// # Errors
///
/// As [`check`], then [`CycleError::NotCanonical`].
pub fn check_canonical(graph: &Graph, cycle: &[Vertex]) -> Result<(), CycleError> {
    check(graph, cycle)?;
    if is_canonical(cycle) {
        Ok(())
    } else {
        Err(CycleError::NotCanonical)
    }
}

/// Whether a sequence of at least three vertices starts at 1 with its second
/// vertex smaller than its last.
#[must_use]
pub fn is_canonical(cycle: &[Vertex]) -> bool {
    cycle.len() >= 3 && cycle[0] == 1 && cycle[1] < cycle[cycle.len() - 1]
}

/// Puts a cycle containing vertex 1 in canonical order, in place: rotated to
/// start at 1, reversed after it if needed.
///
/// # Panics
///
/// When the sequence has fewer than three vertices or lacks vertex 1.
pub fn canonicalize(cycle: &mut [Vertex]) {
    assert!(cycle.len() >= 3, "a cycle has at least three vertices");
    let start = cycle
        .iter()
        .position(|&v| v == 1)
        .expect("a cycle of G_k(n) contains 1");
    cycle.rotate_left(start);
    if cycle[1] > cycle[cycle.len() - 1] {
        cycle[1..].reverse();
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The unique Hamiltonian cycle of `G_2(32)`, canonical.
    pub(crate) const G2_32: [Vertex; 32] = [
        1, 8, 28, 21, 4, 32, 17, 19, 30, 6, 3, 13, 12, 24, 25, 11, 5, 31, 18, 7, 29, 20, 16, 9, 27,
        22, 14, 2, 23, 26, 10, 15,
    ];

    #[test]
    fn the_cycle_of_g2_32() {
        let g = Graph::new(2, 32).unwrap();
        assert_eq!(check_canonical(&g, &G2_32), Ok(()));
    }

    #[test]
    fn every_rotation_and_reflection_canonicalizes_to_one_sequence() {
        let g = Graph::new(2, 32).unwrap();
        for shift in 0..32 {
            for reflect in [false, true] {
                let mut c = G2_32;
                c.rotate_left(shift);
                if reflect {
                    c.reverse();
                }
                assert_eq!(check(&g, &c), Ok(()));
                canonicalize(&mut c);
                assert_eq!(c, G2_32);
            }
        }
    }

    #[test]
    fn defects() {
        let g = Graph::new(2, 32).unwrap();
        assert_eq!(
            check(&g, &G2_32[..31]),
            Err(CycleError::Length {
                found: 31,
                expected: 32
            })
        );

        let mut c = G2_32;
        c[9] = c[8];
        assert_eq!(check(&g, &c), Err(CycleError::Repeated(30)));

        let mut c = G2_32;
        c[3] = 33;
        assert_eq!(
            check(&g, &c),
            Err(CycleError::Range {
                position: 3,
                value: 33
            })
        );

        let mut c = G2_32;
        c.swap(4, 5);
        assert_eq!(check(&g, &c), Err(CycleError::NotAdjacent(21, 32)));

        let mut c = G2_32;
        c[1..].reverse();
        assert_eq!(check(&g, &c), Ok(()));
        assert_eq!(check_canonical(&g, &c), Err(CycleError::NotCanonical));
    }
}
