//! The text files of `doc/formats.md`: plain cycles, bases, bands, islands.
//!
//! Readers enforce the common conventions of section 1 — ASCII, one decimal
//! integer per line without leading zero, every line ended by a line feed, no
//! carriage return, no empty line — and leave the meaning to [`crate::cycle`]
//! and [`crate::bases`]. Writers produce exactly the canonical bytes.

use std::fmt;
use std::io::{self, BufRead, Write};

use crate::graph::Vertex;
use crate::islands::{Band, Island};

/// A file that breaks the conventions, or cannot be read.
#[derive(Debug)]
pub enum FormatError {
    /// A syntax error at a 1-based line.
    Syntax {
        /// The line.
        line: u64,
        /// What is wrong with it.
        reason: &'static str,
    },
    /// An integer out of the range the file allows, at a 1-based line.
    Range {
        /// The line.
        line: u64,
        /// The largest value allowed.
        max: u128,
    },
    /// The file could not be read or written.
    Io(io::Error),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { line, reason } => write!(f, "line {line}: {reason}"),
            Self::Range { line, max } => write!(f, "line {line}: value outside 1..={max}"),
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for FormatError {}

impl From<io::Error> for FormatError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Reads one positive integer per line, each at most `max`.
fn read_integers<R: BufRead>(mut reader: R, max: u128) -> Result<Vec<u128>, FormatError> {
    let mut values = Vec::new();
    let mut buffer = Vec::new();
    let mut line = 0u64;
    loop {
        buffer.clear();
        if reader.read_until(b'\n', &mut buffer)? == 0 {
            return Ok(values);
        }
        line += 1;
        let syntax = |reason| FormatError::Syntax { line, reason };
        let Some(text) = buffer.strip_suffix(b"\n") else {
            return Err(syntax("no final line feed"));
        };
        if text.is_empty() {
            return Err(syntax("empty line"));
        }
        if text.contains(&b'\r') {
            return Err(syntax("carriage return"));
        }
        if !text.iter().all(u8::is_ascii_digit) {
            return Err(syntax("not a decimal integer"));
        }
        if text[0] == b'0' {
            return Err(syntax("leading zero or zero"));
        }
        let value = text
            .iter()
            .try_fold(0u128, |acc, &d| {
                acc.checked_mul(10)?.checked_add(u128::from(d - b'0'))
            })
            .filter(|&value| value <= max)
            .ok_or(FormatError::Range { line, max })?;
        values.push(value);
    }
}

/// Reads a plain cycle file of `G_k(n)`: one vertex per line, each in `1..=n`.
///
/// # Errors
///
/// On a syntax error, a value outside `1..=n`, or a read error.
pub fn read_plain<R: BufRead>(reader: R, n: Vertex) -> Result<Vec<Vertex>, FormatError> {
    let values = read_integers(reader, u128::from(n))?;
    Ok(values
        .into_iter()
        .map(|v| Vertex::try_from(v).expect("checked against n"))
        .collect())
}

/// Reads a bases file: one base per line, each at most `2^64 - 1`.
///
/// # Errors
///
/// On a syntax error, a value too large, or a read error.
pub fn read_bases<R: BufRead>(reader: R) -> Result<Vec<u64>, FormatError> {
    let values = read_integers(reader, u128::from(u64::MAX))?;
    Ok(values
        .into_iter()
        .map(|b| u64::try_from(b).expect("checked against u64::MAX"))
        .collect())
}

/// Writes one value per line.
///
/// # Errors
///
/// On a write error.
pub fn write_lines<W: Write, T: fmt::Display>(
    mut writer: W,
    values: impl IntoIterator<Item = T>,
) -> io::Result<()> {
    for value in values {
        writeln!(writer, "{value}")?;
    }
    writer.flush()
}

/// Writes a plain cycle file.
///
/// # Errors
///
/// On a write error.
pub fn write_plain<W: Write>(writer: W, cycle: &[Vertex]) -> io::Result<()> {
    write_lines(writer, cycle)
}

/// Writes a bases file.
///
/// # Errors
///
/// On a write error.
pub fn write_bases<W: Write>(writer: W, bases: &[u64]) -> io::Result<()> {
    write_lines(writer, bases)
}

/// Writes a bands file.
///
/// # Errors
///
/// On a write error.
pub fn write_bands<W: Write>(writer: W, bands: &[Band]) -> io::Result<()> {
    write_lines(writer, bands)
}

/// Writes an islands listing.
///
/// # Errors
///
/// On a write error.
pub fn write_islands<W: Write>(writer: W, islands: &[Island]) -> io::Result<()> {
    write_lines(writer, islands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cycle::tests::G2_32;

    fn vector(name: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/../../tests/vectors/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    fn syntax(result: Result<Vec<Vertex>, FormatError>) -> &'static str {
        match result {
            Err(FormatError::Syntax { reason, .. }) => reason,
            other => panic!("expected a syntax error, got {other:?}"),
        }
    }

    #[test]
    fn plain_round_trip() {
        let bytes = vector("g2-32.txt");
        assert_eq!(read_plain(&bytes[..], 32).unwrap(), G2_32);
        let mut written = Vec::new();
        write_plain(&mut written, &G2_32).unwrap();
        assert_eq!(written, bytes);
    }

    #[test]
    fn syntax_vectors() {
        assert_eq!(
            syntax(read_plain(&vector("crlf.txt")[..], 32)),
            "carriage return"
        );
        assert_eq!(
            syntax(read_plain(&vector("nofinal.txt")[..], 32)),
            "no final line feed"
        );
        assert_eq!(
            syntax(read_plain(&vector("leading0.txt")[..], 32)),
            "leading zero or zero"
        );
        assert_eq!(syntax(read_plain(&b"1\n\n2\n"[..], 32)), "empty line");
        assert_eq!(
            syntax(read_plain(&b"1 \n"[..], 32)),
            "not a decimal integer"
        );
        assert_eq!(syntax(read_plain(&b"0\n"[..], 32)), "leading zero or zero");
        assert!(matches!(
            read_plain(&b"33\n"[..], 32),
            Err(FormatError::Range { line: 1, .. })
        ));
        let huge = format!("{}0\n", u128::MAX);
        assert!(matches!(
            read_bases(huge.as_bytes()),
            Err(FormatError::Range { .. })
        ));
        assert_eq!(read_plain(&b""[..], 32).unwrap(), Vec::<Vertex>::new());
    }

    #[test]
    fn bases_read_back() {
        let bytes = vector("g2-32.bases");
        let bases = read_bases(&bytes[..]).unwrap();
        let mut written = Vec::new();
        write_bases(&mut written, &bases).unwrap();
        assert_eq!(written, bytes);
    }
}
