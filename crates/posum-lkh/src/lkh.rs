//! The LKH binary and the description of its build.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::state;

/// An LKH binary built by `scripts/build-lkh.sh`, with the description the
/// script writes next to it, `LKH.toml`. posum records these fields in every
/// provenance; it reads them rather than assume them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lkh {
    /// The binary.
    pub path: PathBuf,
    /// LKH version, e.g. `3.0.14`.
    pub version: String,
    /// Tour representation, `three-level` or `two-level`.
    pub tree: String,
    /// SHA-256 of the binary, checked against the file.
    pub binary_sha256: String,
}

/// Why no usable LKH was found.
#[derive(Debug)]
pub enum LkhError {
    /// No binary at any of the places looked at.
    NotFound,
    /// A binary without its `LKH.toml`, or with an unreadable one.
    Description(PathBuf, String),
    /// A binary whose SHA-256 is not the one its `LKH.toml` records.
    Mismatch {
        /// The binary.
        path: PathBuf,
        /// SHA-256 in `LKH.toml`.
        recorded: String,
        /// SHA-256 of the file.
        actual: String,
    },
}

impl fmt::Display for LkhError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(
                f,
                "no LKH binary found: build one with scripts/build-lkh.sh, then pass --lkh or set POSUM_LKH"
            ),
            Self::Description(path, reason) => write!(
                f,
                "{}: {reason}; posum needs the LKH.toml written by scripts/build-lkh.sh",
                path.display()
            ),
            Self::Mismatch {
                path,
                recorded,
                actual,
            } => write!(
                f,
                "{}: SHA-256 {actual}, but LKH.toml records {recorded}; rebuild with scripts/build-lkh.sh",
                path.display()
            ),
        }
    }
}

impl std::error::Error for LkhError {}

impl Lkh {
    /// The LKH binary to use: `explicit` if given, else `$POSUM_LKH`, else
    /// `lkh/bin/LKH` under the current directory, where
    /// `scripts/build-lkh.sh` installs it by default.
    ///
    /// # Errors
    ///
    /// When no binary is found, its description is missing or unreadable, or
    /// the binary is not the one described.
    pub fn locate(explicit: Option<&Path>) -> Result<Self, LkhError> {
        let path = explicit
            .map(Path::to_path_buf)
            .or_else(|| std::env::var_os("POSUM_LKH").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("lkh/bin/LKH"));
        if !path.is_file() {
            return Err(LkhError::NotFound);
        }
        let description = PathBuf::from(format!("{}.toml", path.display()));
        let text = std::fs::read_to_string(&description)
            .map_err(|e| LkhError::Description(description.clone(), e.to_string()))?;
        let document = state::parse(&text)
            .map_err(|e| LkhError::Description(description.clone(), e.to_string()))?;
        let field = |key| {
            state::get_str(&document.top, key)
                .map(str::to_owned)
                .ok_or_else(|| LkhError::Description(description.clone(), format!("no {key}")))
        };
        let (version, tree, recorded) =
            (field("version")?, field("tree")?, field("binary_sha256")?);
        let actual = crate::sha256_file(&path)
            .map_err(|e| LkhError::Description(path.clone(), e.to_string()))?;
        if actual != recorded {
            return Err(LkhError::Mismatch {
                path,
                recorded,
                actual,
            });
        }
        let path = path.canonicalize().unwrap_or(path);
        Ok(Self {
            path,
            version,
            tree,
            binary_sha256: actual,
        })
    }
}
