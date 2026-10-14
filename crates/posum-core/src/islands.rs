//! The chain of blocking vertices: admissible islands and bands certificates.
//!
//! A vertex blocks `G_k(m)` when its degree is below two, which rules out a
//! Hamiltonian cycle. Along `v`, the degree
//!
//! ```text
//! deg(v) = floor((v+m)^(1/k)) - floor(v^(1/k)) - [2v is a k-th power]
//! ```
//!
//! only drops where `v` reaches a power or a loop position, so the smallest
//! blocking vertex is always `1`, some `a^k` or some `a^k/2`. Beyond the safe
//! base `a* = ceil(3 / (2^(1/k) - 1))` three powers lie in `(x, 2x]` and no
//! vertex blocks, which leaves `O(k)` candidates. Following the smallest
//! blocking vertex as `m` grows splits `m >= 1` into bands, each held by one
//! vertex at one degree, and admissible islands, where every degree is at
//! least two; the last island is unbounded.
//!
//! The whole chain is computed once, in exact `u128` arithmetic, by
//! [`Chain::new`]; everything else reads it.

use std::fmt;

use crate::power::{is_power, root};

/// The degree of `v` in `G_k(m)`, for `1 <= v <= m`.
///
/// # Panics
///
/// When `2v` or `v + m` overflows `u128`.
#[must_use]
pub fn degree(k: u32, v: u128, m: u128) -> u128 {
    let sum = v.checked_add(m).expect("v + m fits in u128");
    let double = v.checked_mul(2).expect("2v fits in u128");
    root(sum, k) - root(v, k) - u128::from(is_power(double, k))
}

/// A range of `m` over which one vertex has one degree, at most one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Band {
    /// First `m` of the band.
    pub inf: u128,
    /// Last `m` of the band, `sup >= inf`.
    pub sup: u128,
    /// The smallest vertex of degree at most one in every `G_k(m)` of the band.
    pub vertex: u128,
    /// Its degree, 0 or 1, the same throughout the band.
    pub degree: u8,
}

impl fmt::Display for Band {
    /// The line of a bands file, `doc/formats.md` section 4.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.inf == self.sup {
            write!(f, "{}", self.inf)?;
        } else {
            write!(f, "{} - {}", self.inf, self.sup)?;
        }
        write!(f, " vertex={} degree={}", self.vertex, self.degree)
    }
}

/// A maximal range of `m` where every degree of `G_k(m)` is at least two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Island {
    /// First `m` of the island.
    pub inf: u128,
    /// Last `m`, or `None` for the final, unbounded island.
    pub sup: Option<u128>,
}

impl Island {
    /// Whether `m` lies in the island.
    #[must_use]
    pub fn contains(&self, m: u128) -> bool {
        m >= self.inf && self.sup.is_none_or(|sup| m <= sup)
    }
}

impl fmt::Display for Island {
    /// The line of an islands listing, `doc/formats.md` section 5.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.sup {
            Some(sup) => write!(f, "{} - {sup}", self.inf),
            None => write!(f, "{} - ...", self.inf),
        }
    }
}

/// Why no chain or no certificate can be given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IslandError {
    /// `k < 2`.
    Exponent(u32),
    /// The candidates of this `k` overflow `u128`; `k <= 18` always fits.
    TooLarge(u32),
    /// A bands certificate needs `n >= 2`.
    Order(u128),
    /// `G_k(m)` has every degree at least two for this `m < n`, so no bands
    /// file can certify `n`.
    Admissible(u128),
}

impl fmt::Display for IslandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exponent(k) => write!(f, "k = {k}: the exponent must be at least 2"),
            Self::TooLarge(k) => write!(f, "k = {k}: the chain exceeds 128-bit arithmetic"),
            Self::Order(n) => write!(f, "n = {n}: a bands certificate needs n >= 2"),
            Self::Admissible(m) => write!(
                f,
                "every degree of G_k({m}) is at least two, so no bands file covers it"
            ),
        }
    }
}

impl std::error::Error for IslandError {}

#[derive(Clone, Copy, Debug)]
enum Segment {
    Band(Band),
    Island(u128, u128),
}

/// The chain of blocking vertices of `G_k`, for every `m >= 1`.
#[derive(Clone, Debug)]
pub struct Chain {
    k: u32,
    segments: Vec<Segment>,
    final_island: u128,
}

impl Chain {
    /// Follows the smallest blocking vertex of `G_k(m)` from `m = 1` until
    /// the final island.
    ///
    /// # Errors
    ///
    /// When `k < 2`, or when the candidates of `k` overflow `u128`.
    pub fn new(k: u32) -> Result<Self, IslandError> {
        if k < 2 {
            return Err(IslandError::Exponent(k));
        }

        // The safe base: (a + 3)^k <= 2 a^k from there on.
        let mut safe = 1u128;
        loop {
            let three_above = (safe + 3).checked_pow(k).ok_or(IslandError::TooLarge(k))?;
            let double = safe.pow(k).checked_mul(2).ok_or(IslandError::TooLarge(k))?;
            if three_above <= double {
                break;
            }
            safe += 1;
        }
        let last = safe + 3;

        // Every value handled below is at most 2 (last + 3)^k.
        (last + 3)
            .checked_pow(k)
            .and_then(|p| p.checked_mul(2))
            .ok_or(IslandError::TooLarge(k))?;

        let mut candidates: Vec<u128> = std::iter::once(1)
            .chain((2..last).map(|a| a.pow(k)))
            .chain((2..last).step_by(2).map(|a| a.pow(k) / 2))
            .collect();
        candidates.sort_unstable();
        candidates.dedup();

        let blocking = |m: u128| {
            candidates
                .iter()
                .copied()
                .take_while(|&x| x <= m)
                .find(|&x| degree(k, x, m) < 2)
        };

        let mut segments = Vec::new();
        let mut m = 1u128;
        let mut island = None;

        loop {
            let Some(x) = blocking(m) else {
                island.get_or_insert(m);
                // Degrees never drop as m grows: the next blocker can only be
                // a vertex entering the graph, and the smallest is a candidate.
                match candidates.iter().find(|&&c| c > m) {
                    Some(&c) => m = c,
                    None => break,
                }
                continue;
            };

            if let Some(inf) = island.take() {
                segments.push(Segment::Island(inf, m - 1));
            }

            // The first two powers above x other than the loop 2x: x has
            // degree 0 until the first fits in (x, x + m], 1 until the second.
            let mut usable = (root(x, k) + 1..).map(|b| b.pow(k)).filter(|&p| p != 2 * x);
            let (first, second) = (usable.next().unwrap(), usable.next().unwrap());
            let (zero_end, end) = (first - x - 1, second - x - 1);
            assert!(end >= m, "band of {x} ends at {end}, before m = {m}");

            if m <= zero_end {
                segments.push(Segment::Band(Band {
                    inf: m,
                    sup: zero_end,
                    vertex: x,
                    degree: 0,
                }));
                m = zero_end + 1;
            }
            segments.push(Segment::Band(Band {
                inf: m,
                sup: end,
                vertex: x,
                degree: 1,
            }));
            m = end + 1;
        }

        let final_island = island.expect("the chain ends in an island");
        Ok(Self {
            k,
            segments,
            final_island,
        })
    }

    /// The exponent `k`.
    #[must_use]
    pub const fn k(&self) -> u32 {
        self.k
    }

    /// The admissible islands, in increasing order, the last unbounded.
    #[must_use]
    pub fn islands(&self) -> Vec<Island> {
        self.segments
            .iter()
            .filter_map(|segment| match *segment {
                Segment::Island(inf, sup) => Some(Island {
                    inf,
                    sup: Some(sup),
                }),
                Segment::Band(_) => None,
            })
            .chain(std::iter::once(Island {
                inf: self.final_island,
                sup: None,
            }))
            .collect()
    }

    /// The smallest vertex of degree at most one in `G_k(m)`, with its
    /// degree, or `None` when `m` lies in an admissible island.
    #[must_use]
    pub fn blocking(&self, m: u128) -> Option<(u128, u8)> {
        if m >= self.final_island {
            return None;
        }
        let i = self.segments.partition_point(|segment| match *segment {
            Segment::Band(band) => band.sup < m,
            Segment::Island(_, sup) => sup < m,
        });
        match self.segments.get(i)? {
            Segment::Band(band) => Some((band.vertex, band.degree)),
            Segment::Island(..) => None,
        }
    }

    /// The bands certificate for `n`: canonical bands covering `1..n-1`,
    /// `doc/formats.md` section 4.
    ///
    /// # Errors
    ///
    /// When `n < 2`, or some `G_k(m)`, `m < n`, has every degree at least
    /// two, so that no certificate exists.
    pub fn bands(&self, n: u128) -> Result<Vec<Band>, IslandError> {
        if n < 2 {
            return Err(IslandError::Order(n));
        }
        let mut bands = Vec::new();
        for segment in &self.segments {
            match *segment {
                Segment::Island(inf, _) if inf < n => return Err(IslandError::Admissible(inf)),
                Segment::Band(band) if band.inf < n => {
                    bands.push(Band {
                        sup: band.sup.min(n - 1),
                        ..band
                    });
                }
                _ => break,
            }
        }
        if n > self.final_island {
            return Err(IslandError::Admissible(self.final_island));
        }
        Ok(bands)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The smallest vertex of degree at most one, by scanning every vertex.
    fn brute_blocking(k: u32, m: u128) -> Option<(u128, u8)> {
        (1..=m)
            .find(|&v| degree(k, v, m) < 2)
            .map(|v| (v, u8::try_from(degree(k, v, m)).unwrap()))
    }

    #[test]
    fn blocking_agrees_with_a_full_scan() {
        for (k, limit) in [(2, 3000), (3, 3000), (4, 12_000), (5, 3000)] {
            let chain = Chain::new(k).unwrap();
            for m in 1..=limit {
                assert_eq!(chain.blocking(m), brute_blocking(k, m), "k={k} m={m}");
            }
        }
    }

    /// The islands printed by the lab notebook's scripts/islands-chain.py,
    /// itself checked against a scan of every n for k = 2..6.
    #[test]
    fn islands_as_in_the_lab_notebook() {
        type Islands = &'static [(u128, Option<u128>)];
        let expected: [(u32, Islands); 7] = [
            (2, &[(31, None)]),
            (3, &[(473, None)]),
            (4, &[(9641, Some(9999)), (18193, None)]),
            (5, &[(246_877, Some(248_831)), (490_463, None)]),
            (6, &[(15_748_961, Some(16_777_215)), (30_039_769, None)]),
            (
                7,
                &[(587_761_723, Some(612_220_031)), (1_161_088_541, None)],
            ),
            (
                13,
                &[
                    (36_593_609_919_738_952_097, Some(36_893_488_147_419_103_231)),
                    (77_703_566_159_199_703_923, Some(81_138_303_245_565_435_903)),
                    (
                        158_278_360_126_292_201_269,
                        Some(170_581_728_179_578_208_255),
                    ),
                    (310_631_728_306_075_666_055, None),
                ],
            ),
        ];
        for (k, islands) in expected {
            let found: Vec<_> = Chain::new(k)
                .unwrap()
                .islands()
                .iter()
                .map(|i| (i.inf, i.sup))
                .collect();
            assert_eq!(found, islands, "k={k}");
        }
    }

    #[test]
    fn bands_match_the_test_vectors() {
        for (k, n, file) in [
            (
                3,
                473,
                include_str!("../../../tests/vectors/g3-473-bands.txt"),
            ),
            (
                4,
                9641,
                include_str!("../../../tests/vectors/g4-9641-bands.txt"),
            ),
        ] {
            let mut text = Vec::new();
            crate::format::write_bands(&mut text, &Chain::new(k).unwrap().bands(n).unwrap())
                .unwrap();
            assert_eq!(text, file.as_bytes(), "k={k} n={n}");
        }
    }

    #[test]
    fn bands_are_refused_across_an_island() {
        let chain = Chain::new(4).unwrap();
        assert!(chain.bands(9641).is_ok());
        assert_eq!(chain.bands(9642), Err(IslandError::Admissible(9641)));
        assert!(chain.bands(10_000).is_err());
        assert_eq!(chain.bands(18_193), Err(IslandError::Admissible(9641)));
        assert_eq!(
            Chain::new(3).unwrap().bands(474),
            Err(IslandError::Admissible(473))
        );
        assert_eq!(chain.bands(1), Err(IslandError::Order(1)));
    }

    #[test]
    fn single_values_are_written_alone() {
        let band = Band {
            inf: 5,
            sup: 5,
            vertex: 1,
            degree: 1,
        };
        assert_eq!(band.to_string(), "5 vertex=1 degree=1");
        assert_eq!(
            Island {
                inf: 18193,
                sup: None
            }
            .to_string(),
            "18193 - ..."
        );
    }

    #[test]
    fn limits() {
        assert_eq!(Chain::new(1).unwrap_err(), IslandError::Exponent(1));
        assert!(Chain::new(18).is_ok());
        assert_eq!(Chain::new(40).unwrap_err(), IslandError::TooLarge(40));
    }
}
