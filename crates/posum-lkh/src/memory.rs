//! The memory guard of `doc/design.md` section 5.2.
//!
//! Estimates are arithmetic, from measurements, not promises: LKH takes about
//! 600 bytes per node, measured at `k = 6`; posum's own propagation about 25
//! bytes per vertex plus one per incidence slot, and the contraction it keeps
//! during the search about 20 bytes per vertex.

/// Bytes LKH takes per node.
pub const LKH_BYTES_PER_NODE: u64 = 600;

/// Bytes propagation takes per vertex, slots included at the density of
/// `k = 6`, about 3.3 slots per vertex, rounded up.
pub const PROPAGATION_BYTES_PER_VERTEX: u64 = 30;

/// Bytes the contraction keeps per vertex during the search.
pub const CONTRACTION_BYTES_PER_VERTEX: u64 = 20;

/// The memory available for a new process, from `/proc/meminfo`, or `None`
/// where that is unknown.
#[must_use]
pub fn available() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = text
        .lines()
        .find(|line| line.starts_with("MemAvailable:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

/// What the guard says about an estimate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Below half the available memory, or the available memory is unknown.
    Fine,
    /// Above half the available memory: warn, go on.
    Warn,
    /// Above the available memory: refuse unless forced.
    Refuse,
}

/// The guard's verdict on `estimate` bytes against `available` bytes.
#[must_use]
pub fn judge(estimate: u64, available: Option<u64>) -> Verdict {
    match available {
        Some(available) if estimate > available => Verdict::Refuse,
        Some(available) if estimate > available / 2 => Verdict::Warn,
        _ => Verdict::Fine,
    }
}

/// Bytes, in GB with one decimal, for messages.
#[must_use]
pub fn gb(bytes: u64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let value = bytes as f64 / 1e9;
    format!("{value:.1} GB")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdicts() {
        assert_eq!(judge(10, Some(100)), Verdict::Fine);
        assert_eq!(judge(60, Some(100)), Verdict::Warn);
        assert_eq!(judge(101, Some(100)), Verdict::Refuse);
        assert_eq!(judge(u64::MAX, None), Verdict::Fine);
        assert_eq!(gb(5_700_000_000), "5.7 GB");
    }
}
