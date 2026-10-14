//! The bases encoding, `doc/formats.md` section 3.
//!
//! Each step `v -> w` of a canonical cycle is written as the base `b` of the
//! power `v + w = b^k`; decoding starts at 1 and applies `w = b^k - v`. The
//! closing step back to 1 is implied, so a cycle of `n` vertices has `n - 1`
//! bases. Bases are about `n^(1/k)`, a few digits where vertices have many.

use crate::cycle::{self, CycleError};
use crate::graph::{Graph, Vertex};
use crate::power::root;

/// The `n - 1` bases of a canonical Hamiltonian cycle of `graph`.
///
/// # Errors
///
/// When `cycle` is not a canonical Hamiltonian cycle of `graph`.
pub fn encode(graph: &Graph, cycle: &[Vertex]) -> Result<Vec<u64>, CycleError> {
    cycle::check_canonical(graph, cycle)?;
    Ok(cycle
        .windows(2)
        .map(|w| {
            let base = root(u128::from(w[0]) + u128::from(w[1]), graph.k());
            u64::try_from(base).expect("a base is at most 2n")
        })
        .collect())
}

/// The canonical Hamiltonian cycle of `graph` written by `bases`.
///
/// # Errors
///
/// When a base decodes outside `1..=n`, or the decoded sequence is not a
/// canonical Hamiltonian cycle of `graph`. Positions in
/// [`CycleError::Range`] count the vertex 1 that starts the cycle.
pub fn decode(graph: &Graph, bases: &[u64]) -> Result<Vec<Vertex>, CycleError> {
    let mut cycle = Vec::with_capacity(bases.len() + 1);
    let mut v: Vertex = 1;
    cycle.push(v);
    for (i, &b) in bases.iter().enumerate() {
        let value = u128::from(b)
            .checked_pow(graph.k())
            .and_then(|p| p.checked_sub(u128::from(v)));
        let w = value
            .and_then(|w| Vertex::try_from(w).ok())
            .filter(|&w| graph.contains(w))
            .ok_or(CycleError::Range {
                position: i + 1,
                value: value.unwrap_or(u128::MAX),
            })?;
        cycle.push(w);
        v = w;
    }
    cycle::check_canonical(graph, &cycle)?;
    Ok(cycle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cycle::tests::G2_32;

    #[test]
    fn round_trip() {
        let g = Graph::new(2, 32).unwrap();
        let bases = encode(&g, &G2_32).unwrap();
        assert_eq!(bases.len(), 31);
        assert_eq!(&bases[..4], &[3, 6, 7, 5]);
        assert_eq!(decode(&g, &bases).unwrap(), G2_32);
    }

    #[test]
    fn matches_the_test_vector() {
        let g = Graph::new(2, 32).unwrap();
        let mut text = Vec::new();
        crate::format::write_bases(&mut text, &encode(&g, &G2_32).unwrap()).unwrap();
        assert_eq!(text, include_bytes!("../../../tests/vectors/g2-32.bases"));
    }

    #[test]
    fn defects() {
        let g = Graph::new(2, 32).unwrap();
        let mut bases = encode(&g, &G2_32).unwrap();
        bases[0] = 10;
        assert_eq!(
            decode(&g, &bases),
            Err(CycleError::Range {
                position: 1,
                value: 99
            })
        );
        bases[0] = 1;
        assert_eq!(
            decode(&g, &bases),
            Err(CycleError::Range {
                position: 1,
                value: 0
            })
        );
        assert!(decode(&g, &[3, 6]).is_err());
        let mut reversed = G2_32;
        reversed[1..].reverse();
        assert_eq!(encode(&g, &reversed), Err(CycleError::NotCanonical));
    }
}
