//! `posum sweep`, `doc/design.md` section 5.3: `posum search` over a range
//! of `n`, in parallel, resumable by running the same command again.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use posum_core::islands::Chain;
use posum_lkh::problem::names;
use posum_lkh::{Engine, Lkh, Options, Outcome, memory, search, state};

use crate::{Failure, duration, exponent, integer, record};

/// The state of one `n`.
#[derive(Clone, Debug)]
enum Row {
    /// Not reached yet.
    Pending,
    /// Running.
    Running,
    /// A witness: identity, seconds, trials.
    Done(String, u64, u64),
    /// Time ran out; resumable.
    TimeUp,
    /// An error.
    Failed(String),
}

impl Row {
    fn line(&self, n: u128) -> String {
        match self {
            Self::Pending => format!("{n}\tpending\t\t\t"),
            Self::Running => format!("{n}\trunning\t\t\t"),
            Self::Done(id, seconds, trials) => format!("{n}\tdone\t{id}\t{seconds}\t{trials}"),
            Self::TimeUp => format!("{n}\tresumable\t\t\t"),
            Self::Failed(reason) => {
                format!("{n}\tfailed\t{}\t\t", reason.replace(['\t', '\n'], " "))
            }
        }
    }
}

/// The table behind `sweep.tsv`, rewritten at each change. Lines of other
/// values, from earlier sweeps into the same run root, are kept as they were.
struct Table {
    path: PathBuf,
    rows: BTreeMap<u128, Row>,
    others: BTreeMap<u128, String>,
}

impl Table {
    fn new(path: PathBuf, values: &[u128]) -> Self {
        let mut others = BTreeMap::new();
        for line in fs::read_to_string(&path)
            .unwrap_or_default()
            .lines()
            .skip(1)
        {
            if let Some(n) = line.split('\t').next().and_then(|n| n.parse().ok()) {
                others.insert(n, line.to_owned());
            }
        }
        let rows: BTreeMap<u128, Row> = values.iter().map(|&n| (n, Row::Pending)).collect();
        others.retain(|n, _| !rows.contains_key(n));
        Self { path, rows, others }
    }

    fn set(&mut self, n: u128, row: Row) {
        self.rows.insert(n, row);
        let mut text = String::from("n\tstate\tid\tseconds\ttrials\n");
        let mut lines: BTreeMap<u128, String> = self.others.clone();
        lines.extend(self.rows.iter().map(|(n, row)| (*n, row.line(*n))));
        for line in lines.values() {
            text.push_str(line);
            text.push('\n');
        }
        let temporary = self.path.with_extension("tsv.new");
        if fs::write(&temporary, text).is_ok() {
            let _ = fs::rename(&temporary, &self.path);
        }
    }
}

/// The witness already in a run directory, from its provenance: identity,
/// seconds and trials.
fn existing(dir: &Path) -> Option<(String, u64, u64)> {
    let entry = fs::read_dir(dir).ok()?.filter_map(Result::ok).find(|e| {
        e.file_name()
            .to_string_lossy()
            .ends_with(".provenance.toml")
    })?;
    let document = state::parse(&fs::read_to_string(entry.path()).ok()?).ok()?;
    let id = state::get_str(&document.top, "id")?.to_owned();
    let lkh = document.sections("lkh").next();
    let field = |key| lkh.and_then(|s| state::get_int(&s.table, key)).unwrap_or(0);
    Some((id, field("seconds"), field("trials")))
}

/// Deletes what a finished run can regenerate: the problem and the tours.
fn tidy(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == names::PROBLEM || Path::new(&name).extension().is_some_and(|e| e == "tour") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// One `n`: start, resume, or nothing to do.
fn one(lkh: &Lkh, dir: &Path, k: u32, n: u128, options: &Options, keep: bool) -> Row {
    if let Some((id, seconds, trials)) = existing(dir) {
        return Row::Done(id, seconds, trials);
    }
    // A search reports to its run directory, which a new search creates.
    let mut log = Vec::new();
    let outcome = if dir.join(names::STATE).exists() {
        search::resume(lkh, dir, options, &mut log)
    } else {
        search::start(lkh, dir, k, n, options, &mut log)
    };
    if dir.is_dir()
        && let Ok(mut file) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("search.log"))
    {
        let _ = file.write_all(&log);
    }
    match outcome {
        Ok(Outcome::TimeUp) => Row::TimeUp,
        Ok(Outcome::Found(found)) => match record(dir, k, &found) {
            Ok(id) => {
                if !keep {
                    tidy(dir);
                }
                let (seconds, trials) = match &found.engine {
                    Engine::Lkh(run) => (run.seconds(), run.trials()),
                    Engine::Propagation => (0, 0),
                };
                Row::Done(id, seconds, trials)
            }
            Err(Failure(reason)) => Row::Failed(reason),
        },
        Err(error) => Row::Failed(error.to_string()),
    }
}

/// `posum sweep <k> <inf> <sup> | <k> island [options]`.
pub fn command(words: &[&str]) -> Result<ExitCode, Failure> {
    let mut positional = Vec::new();
    let mut root = PathBuf::from("runs");
    let mut jobs = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let mut options = Options {
        time_limit: None,
        seed: 1,
        force: false,
        nice: Some(19),
    };
    let (mut keep, mut lkh_path) = (false, None);
    let mut rest = words.iter();
    while let Some(&word) = rest.next() {
        let mut value = |name: &str| {
            rest.next()
                .copied()
                .ok_or_else(|| Failure(format!("{name} needs a value")))
        };
        match word {
            "--jobs" => {
                jobs = usize::try_from(integer(value("--jobs")?, "jobs")?)
                    .map_err(|_| Failure("too many jobs".into()))?
                    .max(1);
            }
            "--run-root" => root = PathBuf::from(value("--run-root")?),
            "--time-limit" => options.time_limit = Some(duration(value("--time-limit")?)?),
            "--seed" => {
                options.seed = u64::try_from(integer(value("--seed")?, "seed")?)
                    .map_err(|_| Failure("seed too large".into()))?;
            }
            "--nice" => {
                options.nice = Some(
                    u8::try_from(integer(value("--nice")?, "nice")?)
                        .map_err(|_| Failure("nice must be 0 to 19".into()))?
                        .min(19),
                );
            }
            "--lkh" => lkh_path = Some(PathBuf::from(value("--lkh")?)),
            "--keep" => keep = true,
            "--force" => options.force = true,
            _ if word.starts_with("--") => return Err(Failure(format!("unknown option {word}"))),
            _ => positional.push(word),
        }
    }

    let (k, values): (u32, Vec<u128>) =
        match positional.as_slice() {
            [k, "island"] => {
                let k = exponent(k)?;
                let values = Chain::new(k)?
                    .islands()
                    .iter()
                    .filter_map(|i| i.sup.map(|sup| i.inf..=sup))
                    .flatten()
                    .collect();
                (k, values)
            }
            [k, inf, sup] => {
                let (inf, sup) = (integer(inf, "inf")?, integer(sup, "sup")?);
                if inf > sup {
                    return Err(Failure(format!("empty range {inf} - {sup}")));
                }
                (exponent(k)?, (inf..=sup).collect())
            }
            _ => return Err(Failure(
                "usage: posum sweep <k> <inf> <sup> [options] | posum sweep <k> island [options]"
                    .into(),
            )),
        };
    if values.is_empty() {
        return Err(Failure(format!("G_{k} has no finite admissible island")));
    }

    // LKH takes one core; memory is the other bound. Judge J searches at
    // once on the largest n, and lower J rather than refuse.
    let largest = *values.iter().max().expect("values is not empty");
    let one_search = u64::try_from(largest)
        .unwrap_or(u64::MAX)
        .saturating_mul(memory::CONTRACTION_BYTES_PER_VERTEX + memory::LKH_BYTES_PER_NODE);
    jobs = jobs.min(values.len());
    if let Some(available) = memory::available()
        && !options.force
    {
        let fit = usize::try_from(available / one_search.max(1))
            .unwrap_or(usize::MAX)
            .max(1);
        if fit < jobs {
            eprintln!(
                "posum: warning: memory fits about {fit} searches at once, not {jobs}; running {fit} (--force keeps {jobs})"
            );
            jobs = fit;
        }
    }

    let lkh = Lkh::locate(lkh_path.as_deref())?;
    fs::create_dir_all(&root).map_err(|e| Failure(format!("{}: {e}", root.display())))?;
    let table = Mutex::new(Table::new(root.join("sweep.tsv"), &values));
    eprintln!(
        "posum: sweep of G_{k}(n), {} values from {} to {largest}, {jobs} at once, LKH {}",
        values.len(),
        values[0],
        &lkh.binary_sha256[..12]
    );

    let started = Instant::now();
    let next = AtomicUsize::new(0);
    let finished = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..jobs {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&n) = values.get(index) else { break };
                    table.lock().expect("table lock").set(n, Row::Running);
                    let row = one(&lkh, &root.join(format!("k{k}-n{n}")), k, n, &options, keep);
                    let done = finished.fetch_add(1, Ordering::Relaxed) + 1;
                    let what = match &row {
                        Row::Done(id, seconds, trials) => {
                            format!("{} in {seconds} s, {trials} trials", &id[..8])
                        }
                        Row::TimeUp => "time limit, resumable".to_owned(),
                        Row::Failed(reason) => format!("FAILED: {reason}"),
                        Row::Pending | Row::Running => unreachable!("a finished search"),
                    };
                    eprintln!("[{done}/{}] G_{k}({n}): {what}", values.len());
                    table.lock().expect("table lock").set(n, row);
                }
            });
        }
    });

    let rows = table.into_inner().expect("table lock").rows;
    let count = |f: fn(&Row) -> bool| rows.values().filter(|r| f(r)).count();
    let (done, resumable, failed) = (
        count(|r| matches!(r, Row::Done(..))),
        count(|r| matches!(r, Row::TimeUp)),
        count(|r| matches!(r, Row::Failed(_))),
    );
    eprintln!(
        "posum: {done} found, {resumable} resumable, {failed} failed, in {:.0} s; table in {}",
        started.elapsed().as_secs_f64(),
        root.join("sweep.tsv").display()
    );
    Ok(if failed > 0 {
        ExitCode::FAILURE
    } else if resumable > 0 {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    })
}
