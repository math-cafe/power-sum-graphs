//! Independent verifier for power-sum graph witnesses and certificates.
//!
//!     posum-verify <plain|bases|bands> <k> <n> <file>
//!
//! Decides one uncompressed file against `doc/formats.md` sections 1 to 4, and
//! exits with 0 when it is valid, 1 when it is not, 2 on a usage error or a
//! value beyond 128-bit arithmetic. It reads nothing but the file, as a stream:
//! its memory is one bit per vertex.
//!
//! No dependency, and no code shared with posum.

use std::env;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();

    let (kind, k, n, path) = match parse_arguments(&arguments) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("posum-verify: {message}");
            eprintln!("usage: posum-verify <plain|bases|bands> <k> <n> <file>");
            return ExitCode::from(2);
        }
    };

    let file = match File::open(&path) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("posum-verify: cannot open {path}: {error}");
            return ExitCode::from(2);
        }
    };

    let mut lines = Lines::new(BufReader::new(file));

    let verdict = match kind.as_str() {
        "plain" => verify_plain(&mut lines, k, n),
        "bases" => verify_bases(&mut lines, k, n),
        "bands" => verify_bands(&mut lines, k, n),
        _ => unreachable!("kind checked by parse_arguments"),
    };

    match verdict {
        Ok(()) => {
            println!("VALID {kind} k={k} n={n}");
            ExitCode::SUCCESS
        }
        Err(Failure::Invalid(reason)) => {
            println!("INVALID {kind} k={k} n={n}: {reason}");
            ExitCode::from(1)
        }
        Err(Failure::Limit(reason)) => {
            eprintln!("posum-verify: {reason}");
            ExitCode::from(2)
        }
    }
}

fn parse_arguments(arguments: &[String]) -> Result<(String, u32, u128, String), String> {
    let [kind, k, n, path] = arguments else {
        return Err("expected four arguments".to_owned());
    };

    if !matches!(kind.as_str(), "plain" | "bases" | "bands") {
        return Err(format!("unknown kind {kind:?}"));
    }

    let k = k
        .parse::<u32>()
        .ok()
        .filter(|&k| k >= 2)
        .ok_or_else(|| format!("invalid k {k:?}: expected an integer >= 2"))?;

    let n = n
        .parse::<u128>()
        .ok()
        .filter(|&n| n >= 2)
        .ok_or_else(|| format!("invalid n {n:?}: expected an integer >= 2"))?;

    Ok((kind.clone(), k, n, path.clone()))
}

/// Why a file is rejected: invalid content, or a value this verifier cannot
/// represent, which is reported as an error rather than a verdict.
enum Failure {
    Invalid(String),
    Limit(String),
}

impl Failure {
    fn invalid(reason: impl fmt::Display) -> Self {
        Self::Invalid(reason.to_string())
    }
}

type Verdict = Result<(), Failure>;

// --- Lines (section 1) ---------------------------------------------------

/// The lines of a file, checked against the common conventions: ASCII, every
/// line ending with a line feed, no carriage return, no empty line.
struct Lines<R> {
    reader: R,
    buffer: Vec<u8>,
    number: u128,
}

impl<R: BufRead> Lines<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
            number: 0,
        }
    }

    /// The next line without its line feed, with its number, or None at the
    /// end of the file.
    fn next_line(&mut self) -> Result<Option<(u128, &str)>, Failure> {
        self.buffer.clear();

        let read = self
            .reader
            .read_until(b'\n', &mut self.buffer)
            .map_err(|error| Failure::Limit(format!("read error: {error}")))?;

        if read == 0 {
            return Ok(None);
        }

        self.number += 1;
        let line = self.number;

        if self.buffer.last() != Some(&b'\n') {
            return Err(Failure::invalid(format!("line {line} has no final line feed")));
        }

        self.buffer.pop();

        if self.buffer.is_empty() {
            return Err(Failure::invalid(format!("line {line} is empty")));
        }

        if let Some(&byte) = self.buffer.iter().find(|&&b| b == b'\r' || b >= 0x80) {
            let what = if byte == b'\r' { "a carriage return" } else { "a non-ASCII byte" };
            return Err(Failure::invalid(format!("line {line} contains {what}")));
        }

        Ok(Some((line, std::str::from_utf8(&self.buffer).expect("ASCII checked above"))))
    }
}

/// A decimal integer under section 1: digits only, no leading zero, `0` only
/// when allowed.
fn parse_integer(text: &str, allow_zero: bool, line: u128) -> Result<u128, Failure> {
    let bytes = text.as_bytes();

    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return Err(Failure::invalid(format!("line {line}: {text:?} is not a decimal integer")));
    }

    if bytes[0] == b'0' && (bytes.len() > 1 || !allow_zero) {
        return Err(Failure::invalid(format!("line {line}: {text:?} has a leading zero or is zero")));
    }

    text.parse::<u128>()
        .map_err(|_| Failure::Limit(format!("line {line}: {text} exceeds 128-bit arithmetic")))
}

// --- Powers --------------------------------------------------------------

/// `floor(x^(1/k))`, exactly.
fn kth_root(x: u128, k: u32) -> u128 {
    let fits = |b: u128| b.checked_pow(k).is_some_and(|p| p <= x);
    let (mut lo, mut hi) = (0u128, 1u128);

    while fits(hi) {
        hi *= 2;
    }

    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    lo
}

fn is_kth_power(x: u128, k: u32) -> bool {
    x > 0 && kth_root(x, k).checked_pow(k) == Some(x)
}

fn add(a: u128, b: u128) -> Result<u128, Failure> {
    a.checked_add(b)
        .ok_or_else(|| Failure::Limit("a sum exceeds 128-bit arithmetic".to_owned()))
}

/// Degree of `v` in `G_k(m)`, for `1 <= v <= m`.
fn degree(v: u128, m: u128, k: u32) -> Result<u128, Failure> {
    let loop_ = u128::from(is_kth_power(add(v, v)?, k));
    Ok(kth_root(add(v, m)?, k) - kth_root(v, k) - loop_)
}

/// One bit per vertex.
struct Seen(Vec<u64>);

impl Seen {
    fn new(n: u128) -> Result<Self, Failure> {
        let words = usize::try_from(n / 64 + 1)
            .map_err(|_| Failure::Limit(format!("n = {n} is too large to track")))?;
        Ok(Self(vec![0; words]))
    }

    /// Marks `v`; false if it was already marked.
    fn insert(&mut self, v: u128) -> bool {
        let (word, bit) = ((v / 64) as usize, v % 64);
        let fresh = self.0[word] & (1 << bit) == 0;
        self.0[word] |= 1 << bit;
        fresh
    }
}

// --- Plain cycle files (section 2) ---------------------------------------

fn verify_plain<R: BufRead>(lines: &mut Lines<R>, k: u32, n: u128) -> Verdict {
    if n < 3 {
        return Err(Failure::invalid("a cycle needs n >= 3"));
    }

    let mut seen = Seen::new(n)?;
    let (mut first, mut second, mut previous) = (0u128, 0u128, 0u128);
    let mut count = 0u128;

    while let Some((line, text)) = lines.next_line()? {
        let v = parse_integer(text, false, line)?;

        if v > n {
            return Err(Failure::invalid(format!("line {line}: {v} is outside 1..{n}")));
        }

        if !seen.insert(v) {
            return Err(Failure::invalid(format!("line {line}: {v} appears twice")));
        }

        if count > 0 && !is_kth_power(add(previous, v)?, k) {
            return Err(Failure::invalid(format!("line {line}: {previous} + {v} is not a {k}-th power")));
        }

        count += 1;
        if count > n {
            return Err(Failure::invalid(format!("more than {n} lines")));
        }

        match count {
            1 => first = v,
            2 => second = v,
            _ => {}
        }
        previous = v;
    }

    if count != n {
        return Err(Failure::invalid(format!("{count} lines, expected {n}")));
    }

    if !is_kth_power(add(previous, first)?, k) {
        return Err(Failure::invalid(format!("closing {previous} + {first} is not a {k}-th power")));
    }

    if first != 1 {
        return Err(Failure::invalid(format!("not canonical: starts with {first}, not 1")));
    }

    if second >= previous {
        return Err(Failure::invalid(format!(
            "not canonical: second vertex {second} is not smaller than the last {previous}"
        )));
    }

    Ok(())
}

// --- Bases files (section 3) ---------------------------------------------

fn verify_bases<R: BufRead>(lines: &mut Lines<R>, k: u32, n: u128) -> Verdict {
    if n < 3 {
        return Err(Failure::invalid("a cycle needs n >= 3"));
    }

    let mut seen = Seen::new(n)?;
    seen.insert(1);
    let (mut v, mut second) = (1u128, 0u128);
    let mut count = 0u128;

    while let Some((line, text)) = lines.next_line()? {
        let b = parse_integer(text, false, line)?;

        if b < 2 {
            return Err(Failure::invalid(format!("line {line}: base {b} is below 2")));
        }

        let power = b
            .checked_pow(k)
            .ok_or_else(|| Failure::Limit(format!("line {line}: {b}^{k} exceeds 128-bit arithmetic")))?;

        if power <= v || power - v > n {
            return Err(Failure::invalid(format!("line {line}: {b}^{k} - {v} is outside 1..{n}")));
        }

        v = power - v;

        if !seen.insert(v) {
            return Err(Failure::invalid(format!("line {line}: decodes to {v}, which appears twice")));
        }

        count += 1;
        if count >= n {
            return Err(Failure::invalid(format!("more than {} lines", n - 1)));
        }

        if count == 1 {
            second = v;
        }
    }

    if count != n - 1 {
        return Err(Failure::invalid(format!("{count} lines, expected {}", n - 1)));
    }

    if !is_kth_power(add(v, 1)?, k) {
        return Err(Failure::invalid(format!("does not close: {v} + 1 is not a {k}-th power")));
    }

    if second >= v {
        return Err(Failure::invalid(format!(
            "not canonical: second vertex {second} is not smaller than the last {v}"
        )));
    }

    Ok(())
}

// --- Bands files (section 4) ---------------------------------------------

struct Band {
    inf: u128,
    sup: u128,
    vertex: u128,
    degree: u128,
}

/// `<inf> - <sup> vertex=<v> degree=<d>` or `<m> vertex=<v> degree=<d>`.
fn parse_band(text: &str, line: u128) -> Result<Band, Failure> {
    let fields: Vec<&str> = text.split(' ').collect();

    let (inf, sup, rest) = match fields.as_slice() {
        [inf, "-", sup, rest @ ..] if rest.len() == 2 => {
            let (inf, sup) = (parse_integer(inf, false, line)?, parse_integer(sup, false, line)?);
            if inf >= sup {
                return Err(Failure::invalid(format!(
                    "line {line}: range {inf} - {sup} is not increasing; a single value is written alone"
                )));
            }
            (inf, sup, rest)
        }
        [m, rest @ ..] if rest.len() == 2 => {
            let m = parse_integer(m, false, line)?;
            (m, m, rest)
        }
        _ => return Err(Failure::invalid(format!("line {line}: malformed band {text:?}"))),
    };

    let vertex = rest[0]
        .strip_prefix("vertex=")
        .ok_or_else(|| Failure::invalid(format!("line {line}: expected vertex=")))?;
    let degree = rest[1]
        .strip_prefix("degree=")
        .ok_or_else(|| Failure::invalid(format!("line {line}: expected degree=")))?;

    Ok(Band {
        inf,
        sup,
        vertex: parse_integer(vertex, false, line)?,
        degree: parse_integer(degree, true, line)?,
    })
}

fn verify_bands<R: BufRead>(lines: &mut Lines<R>, k: u32, n: u128) -> Verdict {
    let mut expected = 1u128;
    let mut previous: Option<(u128, u128)> = None;

    while let Some((line, text)) = lines.next_line()? {
        let band = parse_band(text, line)?;

        if band.inf != expected {
            return Err(Failure::invalid(format!("line {line}: starts at {}, expected {expected}", band.inf)));
        }

        if band.sup >= n {
            return Err(Failure::invalid(format!("line {line}: ends at {}, beyond n - 1 = {}", band.sup, n - 1)));
        }

        if band.vertex > band.inf {
            return Err(Failure::invalid(format!(
                "line {line}: vertex {} is not in G_{k}({})",
                band.vertex, band.inf
            )));
        }

        if band.degree > 1 {
            return Err(Failure::invalid(format!("line {line}: degree {} is not at most 1", band.degree)));
        }

        for m in [band.inf, band.sup] {
            let actual = degree(band.vertex, m, k)?;
            if actual != band.degree {
                return Err(Failure::invalid(format!(
                    "line {line}: vertex {} has degree {actual} in G_{k}({m}), not {}",
                    band.vertex, band.degree
                )));
            }
        }

        if previous == Some((band.vertex, band.degree)) {
            return Err(Failure::invalid(format!(
                "line {line}: not canonical: same vertex and degree as the previous band"
            )));
        }

        previous = Some((band.vertex, band.degree));
        expected = band.sup + 1;
    }

    if expected != n {
        return Err(Failure::invalid(format!("bands end at {}, expected {}", expected - 1, n - 1)));
    }

    Ok(())
}

// --- Unit tests: rules no committed vector can isolate -------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn check(kind: &str, k: u32, n: u128, text: &str) -> Result<(), String> {
        let mut lines = Lines::new(text.as_bytes());
        let verdict = match kind {
            "plain" => verify_plain(&mut lines, k, n),
            "bases" => verify_bases(&mut lines, k, n),
            _ => verify_bands(&mut lines, k, n),
        };
        verdict.map_err(|failure| match failure {
            Failure::Invalid(reason) | Failure::Limit(reason) => reason,
        })
    }

    #[test]
    fn a_single_value_written_as_a_range_is_rejected() {
        // G_2(31): bands 1..30. Write the first band's lone end as "m - m".
        let good = "1 - 2 vertex=1 degree=0\n3 - 7 vertex=1 degree=1\n8 - 13 vertex=2 degree=1\n\
                    14 - 16 vertex=8 degree=1\n17 - 19 vertex=16 degree=1\n20 - 30 vertex=18 degree=1\n";
        assert_eq!(check("bands", 2, 31, good), Ok(()));
        let dash = good.replacen("20 - 30", "20 - 20 vertex=18 degree=1\n21 - 30", 1);
        assert!(check("bands", 2, 31, &dash).is_err());
    }

    #[test]
    fn degree_zero_is_written_0_and_only_there() {
        assert!(parse_integer("0", true, 1).is_ok());
        assert!(parse_integer("0", false, 1).is_err());
        assert!(parse_integer("00", true, 1).is_err());
    }

    #[test]
    fn powers_are_decided_exactly() {
        assert!(is_kth_power(893_871_739, 7));
        assert!(!is_kth_power(893_871_740, 7));
        assert_eq!(kth_root(u128::MAX, 2), u128::from(u64::MAX));
    }

    #[test]
    fn a_trailing_space_is_rejected() {
        assert!(check("plain", 2, 3, "1 \n").is_err());
    }
}
