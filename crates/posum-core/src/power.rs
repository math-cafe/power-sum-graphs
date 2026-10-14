//! Exact `k`-th roots.
//!
//! Floating point gives a first guess; exact integer arithmetic decides, as
//! `doc/formats.md` requires.

/// `floor(x^(1/k))`, exactly, for `k >= 1`.
#[must_use]
pub fn root(x: u128, k: u32) -> u128 {
    if x < 2 || k == 1 {
        return x;
    }
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let mut r = (x as f64).powf(1.0 / f64::from(k)) as u128;
    while r.checked_pow(k).is_none_or(|p| p > x) {
        r -= 1;
    }
    while (r + 1).checked_pow(k).is_some_and(|p| p <= x) {
        r += 1;
    }
    r
}

/// `floor(x^(1/k))` for `u64`, the fast path of the graph.
#[must_use]
pub fn root_u64(x: u64, k: u32) -> u64 {
    if x < 2 || k == 1 {
        return x;
    }
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let mut r = (x as f64).powf(1.0 / f64::from(k)) as u64;
    while r.checked_pow(k).is_none_or(|p| p > x) {
        r -= 1;
    }
    while (r + 1).checked_pow(k).is_some_and(|p| p <= x) {
        r += 1;
    }
    r
}

/// Whether `x` is `b^k` for some integer `b >= 1`.
#[must_use]
pub fn is_power(x: u128, k: u32) -> bool {
    x > 0 && root(x, k).checked_pow(k) == Some(x)
}

/// The base `b` with `b^k = x`, if there is one.
#[must_use]
pub fn base_of(x: u64, k: u32) -> Option<u64> {
    let r = root_u64(x, k);
    (x > 0 && r.checked_pow(k) == Some(x)).then_some(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_are_exact_at_and_around_powers() {
        for k in 2..=12 {
            for b in [2u128, 3, 7, 1000, 1_000_003] {
                let Some(p) = b.checked_pow(k) else { continue };
                assert_eq!(root(p, k), b);
                assert_eq!(root(p - 1, k), b - 1);
                assert_eq!(root(p + 1, k), b);
                assert!(is_power(p, k) && !is_power(p + 1, k) && !is_power(p - 1, k));
            }
        }
    }

    #[test]
    fn roots_near_the_top_of_the_range() {
        assert_eq!(root(u128::MAX, 2), u128::from(u64::MAX));
        assert_eq!(root_u64(u64::MAX, 2), u64::from(u32::MAX));
        assert_eq!(root(u128::MAX, 64), 3);
        assert_eq!(root_u64((1 << 63) - 1, 63), 1);
    }

    #[test]
    fn small_values() {
        assert_eq!(root(0, 3), 0);
        assert_eq!(root(1, 3), 1);
        assert!(!is_power(0, 2));
        assert!(is_power(1, 5));
        assert_eq!(base_of(243, 5), Some(3));
        assert_eq!(base_of(244, 5), None);
    }
}
