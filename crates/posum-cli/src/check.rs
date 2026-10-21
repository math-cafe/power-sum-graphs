//! `posum check`, `doc/formats.md` section 10.2: every bundle of both trees,
//! against the specification and both independent verifiers.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use posum_core::graph::Graph;
use posum_core::islands::Chain;
use posum_core::{bases, format};
use posum_lkh::problem::PARAMETERS;
use posum_lkh::state::{self, Document, Section, Table, Value};

use crate::Failure;
use crate::bundle::{self, FORMAT, Kind, Root, Tree};
use crate::provenance;

/// The two independent verifiers.
pub struct Verifiers {
    /// The Rust verifier binary.
    pub rust: PathBuf,
    /// The Python verifier script.
    pub python: PathBuf,
}

impl Verifiers {
    /// The verifiers of the repository at `repository`.
    pub fn locate(repository: &Path) -> Result<Self, Failure> {
        let rust = repository.join("verify/rust/target/release/posum-verify");
        let python = repository.join("verify/python/posum_verify.py");
        if !rust.is_file() {
            return Err(Failure(format!(
                "{} not found: build it with cargo build --release --manifest-path verify/rust/Cargo.toml",
                rust.display()
            )));
        }
        if !python.is_file() {
            return Err(Failure(format!("{} not found", python.display())));
        }
        Ok(Self { rust, python })
    }

    /// Both verdicts on a file; `Err` names the verifier that refused it.
    fn accept(&self, kind: &str, k: u32, n: u128, file: &Path) -> Result<(), String> {
        let (k, n) = (k.to_string(), n.to_string());
        let rust = Command::new(&self.rust)
            .args([kind, &k, &n])
            .arg(file)
            .output();
        let python = Command::new("python3")
            .arg(&self.python)
            .args([kind, &k, &n])
            .arg(file)
            .output();
        for (name, output) in [("Rust", rust), ("Python", python)] {
            match output {
                Ok(o) if o.status.code() == Some(0) => {}
                Ok(o) => {
                    return Err(format!(
                        "{name} verifier: {}",
                        String::from_utf8_lossy(&o.stdout).trim()
                    ));
                }
                Err(e) => return Err(format!("{name} verifier: cannot run: {e}")),
            }
        }
        Ok(())
    }
}

/// One witness file to check.
struct Job {
    tree: Tree,
    path: PathBuf,
    k: u32,
    n: u128,
    id: String,
}

/// Collected problems.
#[derive(Default)]
struct Report {
    problems: Mutex<Vec<String>>,
}

impl Report {
    fn fail(&self, message: String) {
        self.problems.lock().expect("report lock").push(message);
    }
}

/// Checks keys and types: `schema` lists `(key, type, required)` in order;
/// `table` must hold the required keys, nothing outside the schema, in the
/// schema's order.
fn keys(table: &Table, schema: &[(&str, &str, bool)], what: &str) -> Result<(), String> {
    let mut position = 0;
    for (key, value) in table {
        let Some(offset) = schema[position..].iter().position(|(k, _, _)| k == key) else {
            return Err(format!("{what}: key {key} unexpected or out of order"));
        };
        for (skipped, _, required) in &schema[position..position + offset] {
            if *required {
                if table.iter().any(|(k, _)| k == skipped) {
                    return Err(format!("{what}: key {skipped} out of order"));
                }
                return Err(format!("{what}: missing key {skipped}"));
            }
        }
        let expected = schema[position + offset].1;
        let actual = match value {
            Value::Integer(_) => "integer",
            Value::String(_) => "string",
            Value::Boolean(_) => "boolean",
            Value::Array(_) => "array",
            Value::Inline(_) => "table",
        };
        if actual != expected {
            return Err(format!("{what}: {key} must be of type {expected}"));
        }
        position += offset + 1;
    }
    if let Some((missing, _, _)) = schema[position..].iter().find(|(_, _, required)| *required) {
        return Err(format!("{what}: missing key {missing}"));
    }
    Ok(())
}

fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Checks a provenance file against section 9 and its witness.
fn check_provenance(root: &Root, path: &Path, id: &str, k: u32, n: u128) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = String::from_utf8(bytes).map_err(|_| format!("{}: not UTF-8", path.display()))?;
    let what = path.display().to_string();
    let document = state::parse(&text).map_err(|e| format!("{what}: {e}"))?;
    if state::render(&document) != text {
        return Err(format!("{what}: not in canonical form"));
    }
    keys(
        &document.top,
        &[
            ("id", "string", true),
            ("k", "integer", true),
            ("n", "integer", true),
            ("found", "string", true),
            ("posum", "string", true),
            ("engine", "string", true),
            ("cpu", "string", false),
            ("system", "string", false),
        ],
        &what,
    )?;
    let top = &document.top;
    if state::get_str(top, "id") != Some(id)
        || state::get_int(top, "k") != Some(u64::from(k))
        || state::get_int(top, "n").map(u128::from) != Some(n)
    {
        return Err(format!("{what}: id, k or n differ from the witness"));
    }
    let found = state::get_str(top, "found").unwrap_or_default();
    if found.len() != 20 || !found.ends_with('Z') || found.as_bytes()[10] != b'T' {
        return Err(format!("{what}: found must be YYYY-MM-DDTHH:MM:SSZ"));
    }
    match state::get_str(top, "engine") {
        Some("propagation") if document.tables.is_empty() => Ok(()),
        Some("lkh") => {
            let [lkh, segments @ ..] = document.tables.as_slice() else {
                return Err(format!("{what}: engine lkh needs an [lkh] table"));
            };
            if lkh.name != "lkh" || lkh.array {
                return Err(format!("{what}: engine lkh needs an [lkh] table"));
            }
            keys(
                &lkh.table,
                &[
                    ("version", "string", true),
                    ("binary_sha256", "string", true),
                    ("tree", "string", true),
                    ("contracted", "boolean", true),
                    ("seed", "integer", true),
                    ("segments", "integer", true),
                    ("seconds", "integer", true),
                    ("trials", "integer", true),
                    ("parameters", "string", true),
                    ("parameters_sha256", "string", true),
                    ("guard_overridden", "boolean", false),
                ],
                &format!("{what} [lkh]"),
            )?;
            let t = &lkh.table;
            if !state::get_str(t, "binary_sha256").is_some_and(is_sha256) {
                return Err(format!("{what}: binary_sha256 is not a SHA-256"));
            }
            if state::get_bool(t, "guard_overridden") == Some(false) {
                return Err(format!(
                    "{what}: guard_overridden is written only when true"
                ));
            }
            if state::get_str(t, "parameters") != Some(provenance::parameters_path().as_str())
                || state::get_str(t, "parameters_sha256")
                    != Some(posum_lkh::sha256_hex(PARAMETERS.as_bytes()).as_str())
            {
                return Err(format!(
                    "{what}: parameters differ from {}",
                    provenance::parameters_path()
                ));
            }
            let parameters = fs::read(root.parameters())
                .map_err(|e| format!("{}: {e}", root.parameters().display()))?;
            if parameters != PARAMETERS.as_bytes() {
                return Err(format!(
                    "{}: differs from the LKH parameters of posum",
                    root.parameters().display()
                ));
            }
            let count = state::get_int(t, "segments").unwrap_or(0);
            let listed = if count > 1 {
                usize::try_from(count).unwrap_or(usize::MAX)
            } else {
                0
            };
            if count == 0 || segments.len() != listed {
                return Err(format!(
                    "{what}: segments = {count} but {} [[lkh.segment]] tables",
                    segments.len()
                ));
            }
            let (mut seconds, mut trials) = (0, 0);
            for (i, segment) in segments.iter().enumerate() {
                if segment.name != "lkh.segment" || !segment.array {
                    return Err(format!("{what}: unexpected table {}", segment.name));
                }
                keys(
                    &segment.table,
                    &[
                        ("seed", "integer", true),
                        ("started", "string", true),
                        ("seconds", "integer", true),
                        ("trials", "integer", true),
                        ("initial_tour_sha256", "string", false),
                    ],
                    &format!("{what} segment {}", i + 1),
                )?;
                seconds += state::get_int(&segment.table, "seconds").unwrap_or(0);
                trials += state::get_int(&segment.table, "trials").unwrap_or(0);
            }
            if count > 1
                && (Some(seconds) != state::get_int(t, "seconds")
                    || Some(trials) != state::get_int(t, "trials"))
            {
                return Err(format!(
                    "{what}: seconds or trials differ from the sum of the segments"
                ));
            }
            Ok(())
        }
        _ => Err(format!(
            "{what}: engine must be lkh, or propagation with no table"
        )),
    }
}

/// Checks a value bundle's manifest; returns its witnesses as `(n, id)`.
fn check_value_manifest(
    document: &Document,
    k: u32,
    n: u128,
    bundle: &Path,
) -> Result<Vec<(u128, String)>, String> {
    let what = bundle.join("manifest.toml").display().to_string();
    keys(
        &document.top,
        &[
            ("format", "string", true),
            ("k", "integer", true),
            ("n", "integer", true),
            ("claims", "array", true),
            ("reference", "string", true),
        ],
        &what,
    )?;
    let top = &document.top;
    if state::get_str(top, "format") != Some(FORMAT)
        || state::get_int(top, "k") != Some(u64::from(k))
        || state::get_int(top, "n").map(u128::from) != Some(n)
    {
        return Err(format!(
            "{what}: format, k or n differ from the directory name"
        ));
    }
    let minimal = match state::get(top, "claims") {
        Some(Value::Array(c)) if *c == [Value::String("hamiltonian".into())] => false,
        Some(Value::Array(c))
            if *c
                == [
                    Value::String("hamiltonian".into()),
                    Value::String("minimal".into()),
                ] =>
        {
            true
        }
        _ => {
            return Err(format!(
                "{what}: claims must be [\"hamiltonian\"] or [\"hamiltonian\", \"minimal\"]"
            ));
        }
    };
    let mut tables = document.tables.iter().peekable();
    if minimal {
        let Some(Section {
            name,
            array: false,
            table,
        }) = tables.next()
        else {
            return Err(format!("{what}: a minimal claim needs a [bands] table"));
        };
        if name != "bands" {
            return Err(format!(
                "{what}: a minimal claim needs a [bands] table first"
            ));
        }
        keys(
            table,
            &[
                ("file", "string", true),
                ("sha256", "string", true),
                ("count", "integer", true),
            ],
            &format!("{what} [bands]"),
        )?;
        let bands = Chain::new(k)
            .and_then(|c| c.bands(n))
            .map_err(|e| format!("{what}: minimal claimed, but {e}"))?;
        let mut text = Vec::new();
        format::write_bands(&mut text, &bands).map_err(|e| e.to_string())?;
        let path = bundle.join("bands.txt");
        let actual = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if state::get_str(table, "file") != Some("bands.txt")
            || state::get_str(table, "sha256") != Some(posum_lkh::sha256_hex(&actual).as_str())
            || state::get_int(table, "count")
                != Some(actual.iter().filter(|&&b| b == b'\n').count() as u64)
        {
            return Err(format!("{what}: [bands] does not describe bands.txt"));
        }
        if actual != text {
            return Err(format!(
                "{}: differs from the bands posum computes",
                path.display()
            ));
        }
    } else if bundle.join("bands.txt").exists() {
        return Err(format!("{what}: bands.txt without a minimal claim"));
    }
    let mut witnesses = Vec::new();
    for section in tables {
        if section.name != "witness" || !section.array {
            return Err(format!("{what}: unexpected table {}", section.name));
        }
        keys(
            &section.table,
            &[
                ("id", "string", true),
                ("plain", "table", true),
                ("bases", "table", true),
                ("provenance", "string", true),
            ],
            &format!("{what} [[witness]]"),
        )?;
        let id = state::get_str(&section.table, "id")
            .unwrap_or_default()
            .to_owned();
        if !is_sha256(&id) {
            return Err(format!("{what}: witness id {id} is not a SHA-256"));
        }
        let sha8 = &id[..8];
        for (tree, key) in [(Tree::Plain, "plain"), (Tree::Bases, "bases")] {
            let Some(Value::Inline(file)) = state::get(&section.table, key) else {
                unreachable!("checked by keys")
            };
            keys(
                file,
                &[("file", "string", true), ("url", "string", false)],
                &format!("{what} {key}"),
            )?;
            if state::get_str(file, "file") != Some(tree.cycle_file(sha8).as_str()) {
                return Err(format!(
                    "{what}: {key} file of {sha8} must be {}",
                    tree.cycle_file(sha8)
                ));
            }
        }
        if state::get_str(&section.table, "provenance")
            != Some(format!("{sha8}.provenance.toml").as_str())
        {
            return Err(format!(
                "{what}: provenance of {sha8} must be {sha8}.provenance.toml"
            ));
        }
        if witnesses
            .last()
            .is_some_and(|(_, last): &(u128, String)| *last >= id)
        {
            return Err(format!(
                "{what}: witnesses not in increasing order of identity"
            ));
        }
        witnesses.push((n, id));
    }
    let reference = state::get_str(top, "reference").unwrap_or_default();
    if !witnesses.iter().any(|(_, id)| id == reference) {
        return Err(format!("{what}: the reference is not one of the witnesses"));
    }
    Ok(witnesses)
}

/// Checks an island or range bundle's manifest, sections 8.2 and 8.3;
/// returns its witnesses as `(n, id)`.
fn check_interval_manifest(
    document: &Document,
    kind: &Kind,
    bundle: &Path,
) -> Result<Vec<(u128, String)>, String> {
    let what = bundle.join("manifest.toml").display().to_string();
    let (k, inf, sup, key) = match *kind {
        Kind::Island { k, inf, sup } => (k, inf, sup, "island"),
        Kind::Range { k, inf, sup } => (k, inf, sup, "range"),
        Kind::Value { .. } => unreachable!("an interval bundle"),
    };
    keys(
        &document.top,
        &[
            ("format", "string", true),
            ("k", "integer", true),
            (key, "array", true),
            ("claims", "array", true),
            ("witnesses", "array", true),
        ],
        &what,
    )?;
    let top = &document.top;
    let interval = Value::Array(vec![Value::Integer(inf), Value::Integer(sup)]);
    if state::get_str(top, "format") != Some(FORMAT)
        || state::get_int(top, "k") != Some(u64::from(k))
        || state::get(top, key) != Some(&interval)
    {
        return Err(format!(
            "{what}: format, k or {key} differ from the directory name"
        ));
    }
    if key == "island" {
        let finite = Chain::new(k).map_err(|e| e.to_string())?.islands();
        if !finite.iter().any(|i| i.inf == inf && i.sup == Some(sup)) {
            return Err(format!(
                "{what}: {inf} - {sup} is not a finite admissible island of G_{k}"
            ));
        }
    } else {
        bundle::check_range(k, inf, sup).map_err(|e| format!("{what}: {e}"))?;
    }
    if state::get(top, "claims") != Some(&Value::Array(vec![Value::String("hamiltonian".into())])) {
        return Err(format!("{what}: claims must be [\"hamiltonian\"]"));
    }
    if bundle.join("bands.txt").exists() {
        return Err(format!("{what}: an {key} bundle has no bands.txt"));
    }
    let Some(Value::Array(items)) = state::get(top, "witnesses") else {
        unreachable!("checked by keys")
    };
    let mut witnesses: Vec<(u128, String)> = Vec::new();
    let mut references = BTreeSet::new();
    for item in items {
        let Value::Inline(t) = item else {
            return Err(format!("{what}: witnesses must be inline tables"));
        };
        keys(
            t,
            &[
                ("n", "integer", true),
                ("id", "string", true),
                ("reference", "boolean", false),
            ],
            &format!("{what} witness"),
        )?;
        let n = state::get_int(t, "n").map(u128::from).unwrap_or_default();
        let id = state::get_str(t, "id").unwrap_or_default().to_owned();
        if !is_sha256(&id) || !(inf..=sup).contains(&n) {
            return Err(format!("{what}: witness {id} of n = {n} is invalid"));
        }
        match state::get_bool(t, "reference") {
            Some(true) if !references.insert(n) => {
                return Err(format!("{what}: two references for n = {n}"));
            }
            Some(false) => return Err(format!("{what}: reference is written only when true")),
            _ => {}
        }
        if witnesses
            .last()
            .is_some_and(|last| (last.0, last.1.as_str()) >= (n, id.as_str()))
        {
            return Err(format!(
                "{what}: witnesses not in increasing order of n, then identity"
            ));
        }
        witnesses.push((n, id));
    }
    if let Some(missing) = (inf..=sup).find(|n| !references.contains(n)) {
        return Err(format!("{what}: n = {missing} has no reference witness"));
    }
    if document
        .tables
        .iter()
        .any(|s| s.name != "archive" || s.array)
        || document.tables.len() > 1
    {
        return Err(format!("{what}: only an [archive] table may follow"));
    }
    Ok(witnesses)
}

/// The files of a directory tree, relative, with each tree's cycle suffix
/// replaced by a common one, for comparing the two trees.
fn listing(dir: &Path, tree: Tree) -> Result<BTreeSet<String>, String> {
    let mut out = BTreeSet::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current).map_err(|e| format!("{}: {e}", current.display()))? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let relative = path
                .strip_prefix(dir)
                .expect("under dir")
                .to_string_lossy()
                .into_owned();
            let suffix = match tree {
                Tree::Plain => ".txt.xz",
                Tree::Bases => ".bases.xz",
            };
            out.insert(
                relative
                    .strip_suffix(suffix)
                    .map_or(relative.clone(), |stem| format!("{stem}.<cycle>")),
            );
        }
    }
    Ok(out)
}

/// Checks one bundle in both trees, structure first; returns the witness
/// files left to verify.
fn check_bundle(root: &Root, kind: &Kind, trees: &[Tree]) -> Result<Vec<Job>, String> {
    let plain = root.bundle(Tree::Plain, kind);
    let bases = root.bundle(Tree::Bases, kind);
    if !bases.is_dir() {
        return Err(format!(
            "{} has no counterpart {}",
            plain.display(),
            bases.display()
        ));
    }
    let manifest = fs::read(bases.join("manifest.toml"))
        .map_err(|e| format!("{}: {e}", bases.join("manifest.toml").display()))?;
    let text = String::from_utf8(manifest)
        .map_err(|_| format!("{}: manifest not UTF-8", bases.display()))?;
    let document =
        state::parse(&text).map_err(|e| format!("{}/manifest.toml: {e}", bases.display()))?;
    if state::render(&document) != text {
        return Err(format!(
            "{}/manifest.toml: not in canonical form",
            bases.display()
        ));
    }
    let witnesses = match *kind {
        Kind::Value { k, n } => check_value_manifest(&document, k, n, &bases)?,
        Kind::Island { .. } | Kind::Range { .. } => {
            check_interval_manifest(&document, kind, &bases)?
        }
    };

    let left = listing(&bases, Tree::Bases)?;
    if trees.contains(&Tree::Plain) {
        if !plain.is_dir() {
            return Err(format!("{} is missing from the plain tree", kind.name()));
        }
        let right = listing(&plain, Tree::Plain)?;
        if left != right {
            let only: Vec<_> = left.symmetric_difference(&right).take(3).cloned().collect();
            return Err(format!(
                "{}: the two trees hold different files, e.g. {only:?}",
                kind.name()
            ));
        }
        for relative in &left {
            if !relative.ends_with(".<cycle>")
                && fs::read(plain.join(relative)).ok() != fs::read(bases.join(relative)).ok()
            {
                return Err(format!(
                    "{}: {relative} differs between the trees",
                    kind.name()
                ));
            }
        }
    }

    let mut expected = BTreeSet::from(["manifest.toml".to_owned(), "README.md".to_owned()]);
    if bases.join("bands.txt").exists() {
        expected.insert("bands.txt".into());
    }
    let mut jobs = Vec::new();
    for (n, id) in &witnesses {
        let sha8 = &id[..8];
        let prefix = match kind {
            Kind::Value { .. } => String::new(),
            Kind::Island { .. } | Kind::Range { .. } => format!("n{n}/"),
        };
        expected.insert(format!("{prefix}{sha8}.<cycle>"));
        expected.insert(format!("{prefix}{sha8}.provenance.toml"));
        let dir = root.witnesses(Tree::Bases, kind, *n);
        check_provenance(
            root,
            &dir.join(format!("{sha8}.provenance.toml")),
            id,
            kind.k(),
            *n,
        )?;
        for &tree in trees {
            let path = root.witnesses(tree, kind, *n).join(tree.cycle_file(sha8));
            if !path.is_file() {
                return Err(format!("{} is missing", path.display()));
            }
            jobs.push(Job {
                tree,
                path,
                k: kind.k(),
                n: *n,
                id: id.clone(),
            });
        }
    }
    if left != expected {
        let extra: Vec<_> = left
            .symmetric_difference(&expected)
            .take(3)
            .cloned()
            .collect();
        return Err(format!(
            "{}: files and manifest disagree, e.g. {extra:?}",
            kind.name()
        ));
    }
    if let Kind::Value { k, n } = *kind
        && bases.join("bands.txt").exists()
    {
        Verifiers::locate(&repository(root))
            .map_err(|f| f.0)?
            .accept("bands", k, n, &bases.join("bands.txt"))
            .map_err(|e| format!("{}/bands.txt: {e}", bases.display()))?;
    }
    Ok(jobs)
}

/// The repository holding a witness root: its parent.
fn repository(root: &Root) -> PathBuf {
    root.path
        .canonicalize()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Verifies one witness file: decompressed, accepted by both verifiers, and
/// of the identity the manifest gives.
fn verify(job: &Job, verifiers: &Verifiers, scratch: &Path, index: usize) -> Result<(), String> {
    let bytes = bundle::decompress(&job.path).map_err(|f| f.0)?;
    let file = bundle::scratch(scratch, &format!("w{index}"), &bytes).map_err(|f| f.0)?;
    let kind = match job.tree {
        Tree::Plain => "plain",
        Tree::Bases => "bases",
    };
    let result = verifiers.accept(kind, job.k, job.n, &file);
    let _ = fs::remove_file(&file);
    result.map_err(|e| format!("{}: {e}", job.path.display()))?;
    let plain = match job.tree {
        Tree::Plain => bytes,
        Tree::Bases => {
            let graph = Graph::new(job.k, job.n).map_err(|e| e.to_string())?;
            let decoded = format::read_bases(&bytes[..]).map_err(|e| e.to_string())?;
            let cycle = bases::decode(&graph, &decoded)
                .map_err(|e| format!("{}: {e}", job.path.display()))?;
            let mut text = Vec::new();
            format::write_plain(&mut text, &cycle).map_err(|e| e.to_string())?;
            text
        }
    };
    if posum_lkh::sha256_hex(&plain) != job.id {
        return Err(format!(
            "{}: identity differs from the manifest",
            job.path.display()
        ));
    }
    Ok(())
}

/// Section 7.5: the island and range bundles of one `k` are pairwise
/// disjoint.
fn disjoint(kinds: &[Kind]) -> Result<(), String> {
    let mut intervals: Vec<(u32, u128, u128, String)> = kinds
        .iter()
        .filter_map(|kind| match *kind {
            Kind::Island { k, inf, sup } | Kind::Range { k, inf, sup } => {
                Some((k, inf, sup, kind.name()))
            }
            Kind::Value { .. } => None,
        })
        .collect();
    intervals.sort();
    for pair in intervals.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if a.0 == b.0 && b.1 <= a.2 {
            return Err(format!("{} and {} overlap", a.3, b.3));
        }
    }
    Ok(())
}

/// Checks every bundle under `root`; returns whether all passed.
pub fn check(root: &Root, jobs: usize) -> Result<bool, Failure> {
    let verifiers = Verifiers::locate(&repository(root))?;
    let mut kinds = Vec::new();
    for tree in Tree::BOTH {
        let dir = root.tree(tree);
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            let name = entry
                .map_err(|e| Failure(e.to_string()))?
                .file_name()
                .to_string_lossy()
                .into_owned();
            match Kind::parse(&name) {
                Some(kind) if !kinds.contains(&kind) => kinds.push(kind),
                Some(_) => {}
                None => {
                    return Err(Failure(format!(
                        "{}: {name} is not a bundle name",
                        dir.display()
                    )));
                }
            }
        }
    }
    kinds.sort_by_key(|kind| (kind.k(), kind.name()));
    if kinds.is_empty() {
        return Err(Failure(format!("no bundle under {}", root.path.display())));
    }
    disjoint(&kinds).map_err(Failure)?;
    // The bases tree is the repository's; the plain tree is checked when
    // present, from a release or rebuilt by posum bundle plain.
    let trees: Vec<Tree> = if root.tree(Tree::Plain).is_dir() {
        Tree::BOTH.to_vec()
    } else {
        vec![Tree::Bases]
    };
    let scope_name = if trees.len() == 2 {
        "both trees"
    } else {
        "bases tree"
    };

    let report = Report::default();
    let mut work = Vec::new();
    for kind in &kinds {
        match check_bundle(root, kind, &trees) {
            Ok(jobs) => work.push((kind, jobs)),
            Err(problem) => report.fail(problem),
        }
    }

    let scratch = std::env::temp_dir().join(format!("posum-check-{}", std::process::id()));
    fs::create_dir_all(&scratch).map_err(|e| Failure(format!("{}: {e}", scratch.display())))?;
    let all: Vec<&Job> = work.iter().flat_map(|(_, jobs)| jobs).collect();
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..jobs.max(1) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = all.get(index) else { break };
                    if let Err(problem) = verify(job, &verifiers, &scratch, index) {
                        report.fail(problem);
                    }
                }
            });
        }
    });
    let _ = fs::remove_dir_all(&scratch);

    let problems = report.problems.into_inner().expect("report lock");
    for (kind, jobs) in &work {
        let name = kind.name();
        let failed = problems
            .iter()
            .any(|p| p.contains(&format!("/{name}/")) || p.contains(&format!("{name}:")));
        if !failed {
            println!("ok    {name}: {} witness files, {scope_name}", jobs.len());
        }
    }
    for problem in &problems {
        println!("FAIL  {problem}");
    }
    println!(
        "posum check: {} bundles, {} problems",
        kinds.len(),
        problems.len()
    );
    Ok(problems.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALUE: &[(&str, &str, bool)] = &[
        ("format", "string", true),
        ("k", "integer", true),
        ("n", "integer", true),
        ("claims", "array", true),
        ("reference", "string", true),
    ];

    #[test]
    fn manifest_vectors() {
        let canon = state::parse(include_str!("../../../tests/vectors/canon.toml")).unwrap();
        assert_eq!(keys(&canon.top, VALUE, "canon"), Ok(()));
        let noncanon = state::parse(include_str!("../../../tests/vectors/noncanon.toml")).unwrap();
        assert!(
            keys(&noncanon.top, VALUE, "noncanon")
                .unwrap_err()
                .contains("out of order")
        );
    }

    #[test]
    fn schema() {
        let table = |text: &str| state::parse(text).unwrap().top;
        assert!(
            keys(&table("format = \"x\"\nk = 4\n"), VALUE, "t")
                .unwrap_err()
                .contains("missing key n")
        );
        assert!(
            keys(&table("format = \"x\"\nk = \"4\"\n"), VALUE, "t")
                .unwrap_err()
                .contains("must be of type integer")
        );
        assert!(
            keys(&table("format = \"x\"\nextra = 1\n"), VALUE, "t")
                .unwrap_err()
                .contains("unexpected")
        );
    }
}
