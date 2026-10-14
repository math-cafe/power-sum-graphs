//! LKH-3 on power-sum graphs.
//!
//! posum finds Hamiltonian cycles with Keld Helsgaun's LKH-3, run as a
//! separate process and never redistributed: `scripts/build-lkh.sh` fetches
//! and builds it on the user's machine, next to an `LKH.toml` describing the
//! build. A search
//!
//! 1. refuses an `n` outside the admissible islands, where a vertex of
//!    degree below two rules a cycle out;
//! 2. runs degree-two propagation, which may refute or solve the instance;
//! 3. contracts the forced chains and writes the remaining problem in TSPLIB
//!    HCP form, the chains' links as fixed edges;
//! 4. runs LKH in segments, each with its own seed, until a tour of cost 0 is
//!    found or the time limit runs out, keeping the best tour so that a later
//!    segment can start from it;
//! 5. expands the tour to a cycle of `G_k(n)`, checks it and puts it in
//!    canonical order.
//!
//! A run directory holds the whole state of a search, so that it can be
//! resumed after a time limit, a reboot or an interruption.

pub mod lkh;
pub mod memory;
pub mod problem;
pub mod search;
pub mod state;
pub mod time;

pub use lkh::Lkh;
pub use search::{Engine, Found, LkhRun, Options, Outcome, Segment};

use sha2::{Digest, Sha256};

/// The SHA-256 of `bytes`, in lowercase hexadecimal.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// The SHA-256 of a file, in lowercase hexadecimal, read in blocks.
///
/// # Errors
///
/// When the file cannot be read.
pub fn sha256_file(path: &std::path::Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(hex(&hasher.finalize()));
        }
        hasher.update(&buffer[..read]);
    }
}

/// Bytes in lowercase hexadecimal.
#[must_use]
pub fn hex(digest: &[u8]) -> String {
    use std::fmt::Write;
    digest
        .iter()
        .fold(String::with_capacity(64), |mut text, byte| {
            write!(text, "{byte:02x}").expect("writing to a String");
            text
        })
}

#[cfg(test)]
mod tests {
    #[test]
    fn sha256_of_known_inputs() {
        assert_eq!(
            super::sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            super::sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
