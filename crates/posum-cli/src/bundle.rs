//! Witness bundles, `doc/formats.md` sections 7 and 8: adding a witness from
//! a run directory, and the manifests, READMEs and bands files that follow
//! from a bundle's contents.
//!
//! Everything but the witnesses is derived: after each addition the
//! manifests, READMEs and bands files of the bundle are rewritten from the
//! files present, in both trees, so they cannot drift from them. The one
//! choice kept from the previous manifest is the reference witness.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use posum_core::graph::Graph;
use posum_core::islands::{Chain, Island};
use posum_core::{bases, cycle, format};
use posum_lkh::problem::{PARAMETERS, PARAMETERS_FILE};
use posum_lkh::state::{self, Document, Section, Value};

use crate::Failure;

/// The format line of every manifest.
pub const FORMAT: &str = "posum-bundle 1";

/// What a bundle covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// One `(k, n)`.
    Value {
        /// The exponent.
        k: u32,
        /// The order.
        n: u128,
    },
    /// Every `n` of one finite admissible island.
    Island {
        /// The exponent.
        k: u32,
        /// First `n`.
        inf: u128,
        /// Last `n`.
        sup: u128,
    },
}

impl Kind {
    /// The bundle's directory name, section 7.
    pub fn name(&self) -> String {
        match *self {
            Self::Value { k, n } => format!("k{k}-n{n}"),
            Self::Island { k, inf, sup } => format!("k{k}-n{inf}-{sup}"),
        }
    }

    /// The bundle a directory name designates, if it is one.
    pub fn parse(name: &str) -> Option<Self> {
        let number = |t: &str| {
            (!t.is_empty() && !t.starts_with('0') && t.bytes().all(|b| b.is_ascii_digit()))
                .then(|| t.parse().ok())
                .flatten()
        };
        let (k, rest) = name.strip_prefix('k')?.split_once("-n")?;
        let k = u32::try_from(number(k)?).ok()?;
        match rest.split_once('-') {
            None => Some(Self::Value {
                k,
                n: number(rest)?,
            }),
            Some((inf, sup)) => Some(Self::Island {
                k,
                inf: number(inf)?,
                sup: number(sup)?,
            }),
        }
    }

    /// The exponent.
    pub fn k(&self) -> u32 {
        match *self {
            Self::Value { k, .. } | Self::Island { k, .. } => k,
        }
    }
}

/// The two trees and the engines directory under a witness root.
#[derive(Clone, Debug)]
pub struct Root {
    /// The witness root, `witness/` in the repository.
    pub path: PathBuf,
}

impl Root {
    /// `<root>/plain` or `<root>/bases`.
    pub fn tree(&self, tree: Tree) -> PathBuf {
        self.path.join(tree.name())
    }

    /// The directory of `kind` in `tree`.
    pub fn bundle(&self, tree: Tree, kind: &Kind) -> PathBuf {
        self.tree(tree).join(kind.name())
    }

    /// The directory holding the witnesses of `n` in `kind`, in `tree`.
    pub fn witnesses(&self, tree: Tree, kind: &Kind, n: u128) -> PathBuf {
        match kind {
            Kind::Value { .. } => self.bundle(tree, kind),
            Kind::Island { .. } => self.bundle(tree, kind).join(format!("n{n}")),
        }
    }

    /// The LKH parameters every provenance references.
    pub fn parameters(&self) -> PathBuf {
        self.path.join("engines").join(PARAMETERS_FILE)
    }
}

/// One of the two parallel trees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tree {
    /// Plain cycle lists.
    Plain,
    /// The bases encoding.
    Bases,
}

impl Tree {
    /// Both trees.
    pub const BOTH: [Self; 2] = [Self::Plain, Self::Bases];

    /// Its directory name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Bases => "bases",
        }
    }

    /// The cycle file of witness `sha8` in this tree.
    pub fn cycle_file(self, sha8: &str) -> String {
        match self {
            Self::Plain => format!("{sha8}.txt.xz"),
            Self::Bases => format!("{sha8}.bases.xz"),
        }
    }
}

fn read(path: &Path) -> Result<Vec<u8>, Failure> {
    fs::read(path).map_err(|e| Failure(format!("{}: {e}", path.display())))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), Failure> {
    fs::write(path, bytes).map_err(|e| Failure(format!("{}: {e}", path.display())))
}

/// Compresses `source` to `target` with `xz -9e -T1`, section 6.
fn compress(source: &Path, target: &Path) -> Result<(), Failure> {
    let temporary = target.with_extension("xz.new");
    let output = fs::File::create(&temporary)
        .map_err(|e| Failure(format!("{}: {e}", temporary.display())))?;
    let status = Command::new("xz")
        .args(["-9e", "-T1", "-c"])
        .arg(source)
        .stdout(output)
        .status()
        .map_err(|e| Failure(format!("cannot run xz: {e}")))?;
    if !status.success() {
        return Err(Failure(format!("xz failed on {}", source.display())));
    }
    fs::rename(&temporary, target).map_err(|e| Failure(format!("{}: {e}", target.display())))
}

/// The identity, `k` and `n` of a provenance file.
pub fn provenance_header(path: &Path) -> Result<(String, u32, u128), Failure> {
    let text = String::from_utf8(read(path)?)
        .map_err(|_| Failure(format!("{}: not UTF-8", path.display())))?;
    let document = state::parse(&text).map_err(|e| Failure(format!("{}: {e}", path.display())))?;
    let id = state::get_str(&document.top, "id")
        .ok_or_else(|| Failure(format!("{}: no id", path.display())))?;
    let k = state::get_int(&document.top, "k").and_then(|k| u32::try_from(k).ok());
    let n = state::get_int(&document.top, "n");
    match (k, n) {
        (Some(k), Some(n)) => Ok((id.to_owned(), k, u128::from(n))),
        _ => Err(Failure(format!("{}: no k or n", path.display()))),
    }
}

/// The finite admissible island containing `n`.
fn island_of(k: u32, n: u128) -> Result<Island, Failure> {
    Chain::new(k)?
        .islands()
        .into_iter()
        .find(|island| island.sup.is_some() && island.contains(n))
        .ok_or_else(|| {
            Failure(format!(
                "n = {n} lies in no finite admissible island of G_{k}"
            ))
        })
}

/// Adds the witness of a run directory to its bundle, in both trees, and
/// returns the bundle with a message; the caller then rewrites the bundle's
/// derived files with [`refresh`], once per bundle. With `island`, the
/// bundle is the one of the finite island containing `n`; otherwise the
/// value bundle of `n`.
pub fn add(root: &Root, run_dir: &Path, island: bool) -> Result<(Kind, String), Failure> {
    let provenances: Vec<PathBuf> = fs::read_dir(run_dir)
        .map_err(|e| Failure(format!("{}: {e}", run_dir.display())))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".provenance.toml"))
        .collect();
    let [provenance] = provenances.as_slice() else {
        return Err(Failure(format!(
            "{}: expected one witness, found {}",
            run_dir.display(),
            provenances.len()
        )));
    };
    let (id, k, n) = provenance_header(provenance)?;
    let sha8 = &id[..8];
    let plain = run_dir.join(format!("{sha8}.txt"));
    let bytes = read(&plain)?;
    if posum_lkh::sha256_hex(&bytes) != id {
        return Err(Failure(format!(
            "{}: SHA-256 differs from the provenance id",
            plain.display()
        )));
    }
    let graph = Graph::new(k, n)?;
    let cycle = format::read_plain(&bytes[..], graph.n())
        .map_err(|e| Failure(format!("{}: {e}", plain.display())))?;
    cycle::check_canonical(&graph, &cycle)
        .map_err(|e| Failure(format!("{}: {e}", plain.display())))?;

    let kind = if island {
        let found = island_of(k, n)?;
        Kind::Island {
            k,
            inf: found.inf,
            sup: found.sup.expect("a finite island"),
        }
    } else {
        Kind::Value { k, n }
    };

    let mut added = false;
    for tree in Tree::BOTH {
        let dir = root.witnesses(tree, &kind, n);
        fs::create_dir_all(&dir).map_err(|e| Failure(format!("{}: {e}", dir.display())))?;
        let target_provenance = dir.join(format!("{sha8}.provenance.toml"));
        if target_provenance.exists() {
            let (existing, _, _) = provenance_header(&target_provenance)?;
            if existing != id {
                return Err(Failure(format!(
                    "{}: another witness has the prefix {sha8}; refusing to overwrite",
                    dir.display()
                )));
            }
            continue;
        }
        let cycle_file = dir.join(tree.cycle_file(sha8));
        match tree {
            Tree::Plain => compress(&plain, &cycle_file)?,
            Tree::Bases => {
                let encoded = bases::encode(&graph, &cycle)?;
                let temporary = dir.join(format!("{sha8}.bases"));
                let file = fs::File::create(&temporary)
                    .map_err(|e| Failure(format!("{}: {e}", temporary.display())))?;
                format::write_bases(std::io::BufWriter::new(file), &encoded)?;
                compress(&temporary, &cycle_file)?;
                fs::remove_file(&temporary)
                    .map_err(|e| Failure(format!("{}: {e}", temporary.display())))?;
            }
        }
        fs::copy(provenance, &target_provenance)
            .map_err(|e| Failure(format!("{}: {e}", target_provenance.display())))?;
        added = true;
    }

    let parameters = root.parameters();
    if parameters.exists() {
        if read(&parameters)? != PARAMETERS.as_bytes() {
            return Err(Failure(format!(
                "{} differs from this posum's LKH parameters",
                parameters.display()
            )));
        }
    } else {
        fs::create_dir_all(parameters.parent().expect("engines directory"))
            .map_err(|e| Failure(e.to_string()))?;
        write(&parameters, PARAMETERS.as_bytes())?;
    }

    let message = format!(
        "{} {sha8} {} {}",
        if added { "added" } else { "already present:" },
        if added { "to" } else { "in" },
        kind.name()
    );
    Ok((kind, message))
}

/// The identities of the witnesses in a directory, from their provenance
/// files, in increasing order.
pub fn identities(dir: &Path) -> Result<Vec<String>, Failure> {
    let mut ids = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| Failure(format!("{}: {e}", dir.display())))? {
        let path = entry.map_err(|e| Failure(e.to_string()))?.path();
        if path.to_string_lossy().ends_with(".provenance.toml") {
            ids.push(provenance_header(&path)?.0);
        }
    }
    ids.sort();
    Ok(ids)
}

/// The previous manifest of a bundle, if any.
fn previous(root: &Root, kind: &Kind) -> Option<Document> {
    let text = fs::read_to_string(root.bundle(Tree::Plain, kind).join("manifest.toml")).ok()?;
    state::parse(&text).ok()
}

fn string(s: &str) -> Value {
    Value::String(s.to_owned())
}

fn strings(items: &[&str]) -> Value {
    Value::Array(items.iter().map(|s| string(s)).collect())
}

/// Rewrites the manifests, READMEs and bands files of a bundle from its
/// witnesses, identically in both trees.
pub fn refresh(root: &Root, kind: &Kind) -> Result<(), Failure> {
    let old = previous(root, kind);
    let mut document = Document::default();
    let top = &mut document.top;
    top.push(("format".into(), string(FORMAT)));
    top.push(("k".into(), Value::Integer(u128::from(kind.k()))));
    let mut bands_text = None;
    let readme;

    match *kind {
        Kind::Value { k, n } => {
            let ids = identities(&root.witnesses(Tree::Plain, kind, n))?;
            let reference = old
                .as_ref()
                .and_then(|d| state::get_str(&d.top, "reference").map(str::to_owned))
                .filter(|r| ids.contains(r))
                .or_else(|| ids.first().cloned())
                .ok_or_else(|| Failure(format!("{}: no witness", kind.name())))?;
            let minimal = Chain::new(k)?.bands(n).ok();
            top.push(("n".into(), Value::Integer(n)));
            if minimal.is_some() {
                top.push(("claims".into(), strings(&["hamiltonian", "minimal"])));
            } else {
                top.push(("claims".into(), strings(&["hamiltonian"])));
            }
            top.push(("reference".into(), string(&reference)));
            if let Some(bands) = &minimal {
                let mut text = Vec::new();
                format::write_bands(&mut text, bands)?;
                document.tables.push(Section {
                    name: "bands".into(),
                    array: false,
                    table: vec![
                        ("file".into(), string("bands.txt")),
                        ("sha256".into(), string(&posum_lkh::sha256_hex(&text))),
                        ("count".into(), Value::Integer(bands.len() as u128)),
                    ],
                });
                bands_text = Some(text);
            }
            for id in &ids {
                let sha8 = &id[..8];
                let file = |name: String| Value::Inline(vec![("file".into(), Value::String(name))]);
                document.tables.push(Section {
                    name: "witness".into(),
                    array: true,
                    table: vec![
                        ("id".into(), string(id)),
                        ("plain".into(), file(Tree::Plain.cycle_file(sha8))),
                        ("bases".into(), file(Tree::Bases.cycle_file(sha8))),
                        (
                            "provenance".into(),
                            Value::String(format!("{sha8}.provenance.toml")),
                        ),
                    ],
                });
            }
            readme = readme_value(k, n, minimal.is_some(), &reference, ids.len());
        }
        Kind::Island { k, inf, sup } => {
            let old_references: Vec<(u128, String)> = old
                .as_ref()
                .and_then(|d| match state::get(&d.top, "witnesses") {
                    Some(Value::Array(items)) => Some(items.clone()),
                    _ => None,
                })
                .unwrap_or_default()
                .iter()
                .filter_map(|item| match item {
                    Value::Inline(t) if state::get_bool(t, "reference") == Some(true) => Some((
                        u128::from(state::get_int(t, "n")?),
                        state::get_str(t, "id")?.to_owned(),
                    )),
                    _ => None,
                })
                .collect();
            let mut witnesses = Vec::new();
            let mut values = 0usize;
            let mut count = 0usize;
            let bundle = root.bundle(Tree::Plain, kind);
            for n in inf..=sup {
                let dir = bundle.join(format!("n{n}"));
                if !dir.is_dir() {
                    continue;
                }
                let ids = identities(&dir)?;
                if ids.is_empty() {
                    continue;
                }
                values += 1;
                let reference = old_references
                    .iter()
                    .find(|(m, id)| *m == n && ids.contains(id))
                    .map_or_else(|| ids[0].clone(), |(_, id)| id.clone());
                for id in ids {
                    count += 1;
                    let mut entry =
                        vec![("n".into(), Value::Integer(n)), ("id".into(), string(&id))];
                    if id == reference {
                        entry.push(("reference".into(), Value::Boolean(true)));
                    }
                    witnesses.push(Value::Inline(entry));
                }
            }
            top.push((
                "island".into(),
                Value::Array(vec![Value::Integer(inf), Value::Integer(sup)]),
            ));
            top.push(("claims".into(), strings(&["hamiltonian"])));
            top.push(("witnesses".into(), Value::Array(witnesses)));
            readme = readme_island(k, inf, sup, values, count);
        }
    }

    let manifest = state::render(&document);
    for tree in Tree::BOTH {
        let dir = root.bundle(tree, kind);
        write(&dir.join("manifest.toml"), manifest.as_bytes())?;
        write(&dir.join("README.md"), readme.as_bytes())?;
        let bands = dir.join("bands.txt");
        match &bands_text {
            Some(text) => write(&bands, text)?,
            None if bands.exists() => {
                fs::remove_file(&bands).map_err(|e| Failure(e.to_string()))?;
            }
            None => {}
        }
    }
    Ok(())
}

fn readme_value(k: u32, n: u128, minimal: bool, reference: &str, count: usize) -> String {
    let mut text = format!("# G_{k}({n})\n\n");
    if minimal {
        text.push_str(&format!(
            "G_{k}({n}) has a Hamiltonian cycle, and no G_{k}(m) with m < {n} has one:\n\
             {n} is the least n for which G_{k}(n) is Hamiltonian.\n\n\
             - `bands.txt` certifies every m < {n}: each line names a vertex of degree\n  \
             at most one in G_{k}(m) for every m of its range.\n"
        ));
    } else {
        text.push_str(&format!("G_{k}({n}) has a Hamiltonian cycle.\n\n"));
        if (k, n) == (2, 32) {
            // No bands file can certify it: G_2(31) has every degree at
            // least two. doc/design.md section 6.3c.
            text.push_str(
                "32 is also the least n for which G_2(n) is Hamiltonian, a known result\n\
                 (B. Dobbelaere, OEIS A071984) cited here, not certified: G_2(31) has every\n\
                 degree at least two, so no bands file can rule it out.\n\n",
            );
        }
    }
    if count == 1 {
        text.push_str(&format!(
            "- 1 witness found by posum, with its provenance, cited in papers:\n  `{reference}`.\n\n"
        ));
    } else {
        text.push_str(&format!(
            "- {count} witnesses found by posum, each with its provenance; the reference,\n  cited in papers, is `{reference}`.\n\n"
        ));
    }
    text.push_str(&format!(
        "Check a witness with either independent verifier of the repository, after\n\
         decompressing it:\n\n\
         ```console\n\
         xz -dk {sha8}.txt.xz\n\
         verify/rust/target/release/posum-verify plain {k} {n} {sha8}.txt\n\
         python3 verify/python/posum_verify.py plain {k} {n} {sha8}.txt\n\
         ```\n\n\
         The format of every file is specified in `doc/formats.md`.\n",
        sha8 = &reference[..8]
    ));
    text
}

fn readme_island(k: u32, inf: u128, sup: u128, values: usize, count: usize) -> String {
    let size = sup - inf + 1;
    format!(
        "# G_{k}(n) for every n of the admissible island {inf} - {sup}\n\n\
         Every G_{k}(n) with {inf} <= n <= {sup}, the {size} values of a finite admissible\n\
         island of G_{k}, has a Hamiltonian cycle. This bundle holds witnesses for\n\
         {values} of them, {count} in all, one directory `n<n>/` per value, each witness with\n\
         its provenance; `manifest.toml` marks one reference per value.\n\n\
         Check a witness with either independent verifier of the repository, after\n\
         decompressing it:\n\n\
         ```console\n\
         xz -dk n<n>/<sha8>.txt.xz\n\
         verify/rust/target/release/posum-verify plain {k} <n> n<n>/<sha8>.txt\n\
         python3 verify/python/posum_verify.py plain {k} <n> n<n>/<sha8>.txt\n\
         ```\n\n\
         The format of every file is specified in `doc/formats.md`.\n"
    )
}

/// Writes `bytes` to a fresh file under `dir`, for the verifiers.
pub fn scratch(dir: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf, Failure> {
    let path = dir.join(name);
    let mut file =
        fs::File::create(&path).map_err(|e| Failure(format!("{}: {e}", path.display())))?;
    file.write_all(bytes)
        .map_err(|e| Failure(format!("{}: {e}", path.display())))?;
    Ok(path)
}

/// Decompresses an xz file to memory.
pub fn decompress(path: &Path) -> Result<Vec<u8>, Failure> {
    let output = Command::new("xz")
        .args(["-dc"])
        .arg(path)
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| Failure(format!("cannot run xz: {e}")))?;
    if !output.status.success() {
        return Err(Failure(format!(
            "{}: xz: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::Kind;

    #[test]
    fn names() {
        for kind in [
            Kind::Value {
                k: 6,
                n: 15_748_961,
            },
            Kind::Island {
                k: 5,
                inf: 246_877,
                sup: 248_831,
            },
        ] {
            assert_eq!(Kind::parse(&kind.name()), Some(kind));
        }
        assert_eq!(Kind::parse("k6-n015748961"), None);
        assert_eq!(Kind::parse("k6n15748961"), None);
        assert_eq!(Kind::parse("engines"), None);
    }
}
