//! A search: from `(k, n)` to a checked canonical cycle, in a run directory.
//!
//! The run directory holds the problem, the fixed parameters, one parameter
//! file and one log per segment, the tours LKH writes, and `run.toml`, the
//! state from which `resume` continues. A segment is one LKH run: it ends
//! with a tour of cost 0, when LKH gives up on its trials, or when the time
//! limit runs out. The next segment starts from the best tour so far with the
//! seed after the previous one, so that LKH does not walk the same path again.

use std::fmt;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, UNIX_EPOCH};

use posum_core::contraction::{Contraction, ContractionError};
use posum_core::cycle::{self, CycleError};
use posum_core::graph::{Graph, GraphError, Vertex};
use posum_core::islands::Chain;
use posum_core::propagation::{Obstruction, Propagation};

use crate::lkh::Lkh;
use crate::memory::{self, Verdict};
use crate::problem::{self, TourError, names};
use crate::state;

/// How to search.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Wall-clock budget for this invocation, `None` for no limit.
    pub time_limit: Option<Duration>,
    /// Seed of the first segment; segment `i` uses `seed + i`.
    pub seed: u64,
    /// Start even when the memory estimate exceeds the available memory.
    pub force: bool,
}

/// One LKH run of a search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    /// Its seed.
    pub seed: u64,
    /// When it started, ISO 8601 UTC.
    pub started: String,
    /// Its wall-clock duration, in whole seconds.
    pub seconds: u64,
    /// Trials LKH completed, as far as its log tells.
    pub trials: u64,
    /// SHA-256 of the tour it started from, if any.
    pub initial_tour_sha256: Option<String>,
}

/// How LKH found a cycle, for the provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LkhRun {
    /// LKH version.
    pub version: String,
    /// LKH tour representation.
    pub tree: String,
    /// SHA-256 of the LKH binary.
    pub binary_sha256: String,
    /// Seed of the first segment.
    pub seed: u64,
    /// Whether forced chains were contracted; always, in posum 1.
    pub contracted: bool,
    /// SHA-256 of [`problem::PARAMETERS`].
    pub parameters_sha256: String,
    /// Whether the memory guard was overridden at any segment.
    pub guard_overridden: bool,
    /// Every segment, in order.
    pub segments: Vec<Segment>,
}

impl LkhRun {
    /// Total wall-clock seconds.
    #[must_use]
    pub fn seconds(&self) -> u64 {
        self.segments.iter().map(|s| s.seconds).sum()
    }

    /// Total trials.
    #[must_use]
    pub fn trials(&self) -> u64 {
        self.segments.iter().map(|s| s.trials).sum()
    }
}

/// What found the cycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Engine {
    /// Propagation alone forced every edge of the cycle.
    Propagation,
    /// LKH.
    Lkh(LkhRun),
}

/// A Hamiltonian cycle found, checked and in canonical order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// The cycle, canonical.
    pub cycle: Vec<Vertex>,
    /// When it was found, ISO 8601 UTC.
    pub found: String,
    /// How.
    pub engine: Engine,
}

/// How a search ended, short of an error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A cycle.
    Found(Found),
    /// The time limit ran out; the run directory can be resumed.
    TimeUp,
}

/// Why a search could not go on.
#[derive(Debug)]
pub enum SearchError {
    /// Invalid parameters.
    Graph(GraphError),
    /// `n` is outside the admissible islands: this vertex has degree below
    /// two, so `G_k(n)` has no Hamiltonian cycle.
    NotAdmissible {
        /// The smallest such vertex.
        vertex: u128,
        /// Its degree.
        degree: u8,
    },
    /// Propagation proved that `G_k(n)` has no Hamiltonian cycle.
    Obstruction(Obstruction),
    /// The memory estimate exceeds the available memory, and the search was
    /// not forced.
    Memory {
        /// Estimated bytes.
        estimate: u64,
        /// Available bytes.
        available: u64,
    },
    /// The run directory already holds something, for a new search.
    Exists(PathBuf),
    /// The run directory's state is missing, unreadable or inconsistent.
    State(String),
    /// LKH failed, or returned an unusable tour.
    Lkh(String),
    /// A file could not be read or written.
    Io(PathBuf, io::Error),
    /// This many consecutive segments ran all their trials without improving
    /// the best tour.
    Stalled(u32),
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Graph(e) => write!(f, "{e}"),
            Self::NotAdmissible { vertex, degree } => write!(
                f,
                "n is outside the admissible islands: vertex {vertex} has degree {degree}, so there is no Hamiltonian cycle"
            ),
            Self::Obstruction(o) => write!(f, "no Hamiltonian cycle, by propagation: {o}"),
            Self::Memory {
                estimate,
                available,
            } => write!(
                f,
                "estimated memory {} exceeds the {} available; --force starts anyway",
                memory::gb(*estimate),
                memory::gb(*available)
            ),
            Self::Exists(dir) => write!(
                f,
                "{} already exists; choose another --run-dir, or continue it with --resume",
                dir.display()
            ),
            Self::State(reason) => write!(f, "run directory: {reason}"),
            Self::Lkh(reason) => write!(f, "LKH: {reason}"),
            Self::Io(path, e) => write!(f, "{}: {e}", path.display()),
            Self::Stalled(count) => write!(
                f,
                "{count} segments in a row ran all their trials without improving the best tour; \
                 G_k(n) may have no Hamiltonian cycle (G_2(31) has none), and LKH cannot prove it"
            ),
        }
    }
}

impl std::error::Error for SearchError {}

impl From<GraphError> for SearchError {
    fn from(e: GraphError) -> Self {
        Self::Graph(e)
    }
}

impl From<Obstruction> for SearchError {
    fn from(e: Obstruction) -> Self {
        Self::Obstruction(e)
    }
}

fn io_at(path: &Path) -> impl FnOnce(io::Error) -> SearchError + '_ {
    move |e| SearchError::Io(path.to_path_buf(), e)
}

/// The persistent state of a search, `run.toml`.
#[derive(Clone, Debug)]
struct RunState {
    k: u32,
    n: Vertex,
    seed: u64,
    nodes: u64,
    binary_sha256: String,
    guard_overridden: bool,
    segments: Vec<Segment>,
}

impl RunState {
    fn text(&self) -> String {
        let mut text = format!(
            "posum = \"{}\"\nk = {}\nn = {}\nseed = {}\nnodes = {}\nlkh_binary_sha256 = \"{}\"\nguard_overridden = {}\n",
            env!("CARGO_PKG_VERSION"),
            self.k,
            self.n,
            self.seed,
            self.nodes,
            self.binary_sha256,
            self.guard_overridden
        );
        for s in &self.segments {
            text.push_str(&format!(
                "\n[[segment]]\nseed = {}\nstarted = \"{}\"\nseconds = {}\ntrials = {}\n",
                s.seed, s.started, s.seconds, s.trials
            ));
            if let Some(sha) = &s.initial_tour_sha256 {
                text.push_str(&format!("initial_tour_sha256 = \"{sha}\"\n"));
            }
        }
        text
    }

    fn save(&self, dir: &Path) -> Result<(), SearchError> {
        let path = dir.join(names::STATE);
        let temporary = dir.join(format!("{}.new", names::STATE));
        fs::write(&temporary, self.text()).map_err(io_at(&temporary))?;
        fs::rename(&temporary, &path).map_err(io_at(&path))
    }

    fn load(dir: &Path) -> Result<Self, SearchError> {
        let path = dir.join(names::STATE);
        let text = fs::read_to_string(&path).map_err(io_at(&path))?;
        let document = state::parse(&text).map_err(|e| SearchError::State(e.to_string()))?;
        let missing =
            |key: &str| SearchError::State(format!("{} has no valid {key}", names::STATE));
        let top = &document.top;
        let int = |key: &str| state::get_int(top, key).ok_or_else(|| missing(key));
        let mut segments = Vec::new();
        for section in &document.tables {
            let (name, table) = (&section.name, &section.table);
            if name != "segment" || !section.array {
                return Err(SearchError::State(format!("unexpected table {name}")));
            }
            let int = |key: &str| state::get_int(table, key).ok_or_else(|| missing(key));
            segments.push(Segment {
                seed: int("seed")?,
                started: state::get_str(table, "started")
                    .ok_or_else(|| missing("started"))?
                    .to_owned(),
                seconds: int("seconds")?,
                trials: int("trials")?,
                initial_tour_sha256: state::get_str(table, "initial_tour_sha256")
                    .map(str::to_owned),
            });
        }
        Ok(Self {
            k: u32::try_from(int("k")?).map_err(|_| missing("k"))?,
            n: Vertex::try_from(int("n")?).map_err(|_| missing("n"))?,
            seed: int("seed")?,
            nodes: int("nodes")?,
            binary_sha256: state::get_str(top, "lkh_binary_sha256")
                .ok_or_else(|| missing("lkh_binary_sha256"))?
                .to_owned(),
            guard_overridden: state::get_bool(top, "guard_overridden")
                .ok_or_else(|| missing("guard_overridden"))?,
            segments,
        })
    }
}

/// Applies the memory guard to `estimate`; returns whether it was overridden.
fn guard(estimate: u64, force: bool, what: &str, log: &mut dyn Write) -> Result<bool, SearchError> {
    let available = memory::available();
    let verdict = memory::judge(estimate, available);
    let total = available.map_or_else(|| "unknown".to_owned(), memory::gb);
    match verdict {
        Verdict::Fine => {
            let _ = writeln!(
                log,
                "posum: {what}: about {} (available {total})",
                memory::gb(estimate)
            );
            Ok(false)
        }
        Verdict::Warn => {
            let _ = writeln!(
                log,
                "posum: warning: {what} needs about {}, more than half of the {total} available",
                memory::gb(estimate)
            );
            Ok(false)
        }
        Verdict::Refuse if force => {
            let _ = writeln!(
                log,
                "posum: warning: {what} needs about {}, more than the {total} available; forced",
                memory::gb(estimate)
            );
            Ok(true)
        }
        Verdict::Refuse => Err(SearchError::Memory {
            estimate,
            available: available.expect("Refuse needs a known amount"),
        }),
    }
}

/// Propagation and contraction of `G_k(n)`, with the memory guard, after
/// refusing `n` outside the islands. `Err(Found)` when propagation alone
/// solves the instance.
fn prepare(
    graph: &Graph,
    force: bool,
    log: &mut dyn Write,
) -> Result<Result<(Propagation, Contraction, bool), Found>, SearchError> {
    if let Ok(chain) = Chain::new(graph.k())
        && let Some((vertex, degree)) = chain.blocking(u128::from(graph.n()))
    {
        return Err(SearchError::NotAdmissible { vertex, degree });
    }
    let n = u64::from(graph.n());
    let mut overridden = guard(
        n * memory::PROPAGATION_BYTES_PER_VERTEX,
        force,
        "propagation",
        log,
    )?;
    let started = Instant::now();
    let propagation = Propagation::run(graph)?;
    let _ = writeln!(
        log,
        "posum: G_{}({}): {} edges forced, {} forbidden, {} free, in {:.1} s",
        graph.k(),
        graph.n(),
        propagation.forced_edges(),
        propagation.forbidden_edges(),
        propagation.free_edges(),
        started.elapsed().as_secs_f64()
    );
    let contraction = match Contraction::new(&propagation) {
        Ok(contraction) => contraction,
        Err(ContractionError::Solved) => {
            let cycle = propagation
                .forced_cycle()
                .expect("a solved propagation has its cycle");
            return Ok(Err(Found {
                cycle,
                found: crate::time::now(),
                engine: Engine::Propagation,
            }));
        }
        Err(e) => return Err(SearchError::State(e.to_string())),
    };
    let nodes = contraction.node_count() as u64;
    let _ = writeln!(
        log,
        "posum: {} chains contracted, {nodes} nodes left for LKH",
        contraction.chain_count()
    );
    let estimate = n * memory::CONTRACTION_BYTES_PER_VERTEX + nodes * memory::LKH_BYTES_PER_NODE;
    overridden |= guard(estimate, force, "LKH", log)?;
    Ok(Ok((propagation, contraction, overridden)))
}

/// Starts a new search of `G_k(n)` in `dir`, which must not exist or be
/// empty.
///
/// # Errors
///
/// See [`SearchError`].
pub fn start(
    lkh: &Lkh,
    dir: &Path,
    k: u32,
    n: u128,
    options: &Options,
    log: &mut dyn Write,
) -> Result<Outcome, SearchError> {
    if dir.exists() && fs::read_dir(dir).map_err(io_at(dir))?.next().is_some() {
        return Err(SearchError::Exists(dir.to_path_buf()));
    }
    let graph = Graph::new(k, n)?;
    let (propagation, contraction, overridden) = match prepare(&graph, options.force, log)? {
        Ok(prepared) => prepared,
        Err(found) => return Ok(Outcome::Found(found)),
    };
    fs::create_dir_all(dir).map_err(io_at(dir))?;
    let path = dir.join(names::PROBLEM);
    problem::write_problem(&path, &propagation, &contraction).map_err(io_at(&path))?;
    drop(propagation);
    let path = dir.join(names::PARAMETERS);
    fs::write(&path, problem::PARAMETERS).map_err(io_at(&path))?;
    let mut run = RunState {
        k,
        n: graph.n(),
        seed: options.seed,
        nodes: contraction.node_count() as u64,
        binary_sha256: lkh.binary_sha256.clone(),
        guard_overridden: overridden,
        segments: Vec::new(),
    };
    run.save(dir)?;
    segments(lkh, dir, &graph, &contraction, &mut run, options, log)
}

/// Continues the search in `dir`, from its best tour.
///
/// # Errors
///
/// See [`SearchError`]; in particular when `lkh` is not the binary the search
/// started with, since one provenance names one binary.
pub fn resume(
    lkh: &Lkh,
    dir: &Path,
    options: &Options,
    log: &mut dyn Write,
) -> Result<Outcome, SearchError> {
    let mut run = RunState::load(dir)?;
    if run.binary_sha256 != lkh.binary_sha256 {
        return Err(SearchError::State(format!(
            "started with LKH {}, not {}; resume with the same binary",
            run.binary_sha256, lkh.binary_sha256
        )));
    }
    let graph = Graph::new(run.k, u128::from(run.n))?;
    let Ok((propagation, contraction, overridden)) = prepare(&graph, options.force, log)? else {
        return Err(SearchError::State(
            "propagation solves this instance alone".into(),
        ));
    };
    drop(propagation);
    if contraction.node_count() as u64 != run.nodes {
        return Err(SearchError::State(format!(
            "{} nodes now, {} when the search started: another posum version?",
            contraction.node_count(),
            run.nodes
        )));
    }
    run.guard_overridden |= overridden;
    run.save(dir)?;
    segments(lkh, dir, &graph, &contraction, &mut run, options, log)
}

/// The lowest-cost `best.<cost>.tour` in `dir`, as `(cost, file name)`.
fn best_tour(dir: &Path) -> Result<Option<(u64, String)>, SearchError> {
    let (prefix, suffix) = names::BEST_TOUR_PATTERN
        .split_once('$')
        .expect("the pattern holds $");
    let mut best = None;
    for entry in fs::read_dir(dir).map_err(io_at(dir))? {
        let name = entry
            .map_err(io_at(dir))?
            .file_name()
            .to_string_lossy()
            .into_owned();
        let cost = name
            .strip_prefix(prefix)
            .and_then(|r| r.strip_suffix(suffix))
            .and_then(|c| c.parse::<u64>().ok());
        if let Some(cost) = cost
            && best.as_ref().is_none_or(|&(c, _)| cost < c)
        {
            best = Some((cost, name));
        }
    }
    Ok(best)
}

/// The checked canonical cycle of a tour of cost 0, if LKH wrote one.
fn found(
    dir: &Path,
    graph: &Graph,
    contraction: &Contraction,
    run: &RunState,
    lkh: &Lkh,
) -> Result<Option<Found>, SearchError> {
    let Some((0, name)) = best_tour(dir)? else {
        return Ok(None);
    };
    let path = dir.join(&name);
    let text = fs::read_to_string(&path).map_err(io_at(&path))?;
    let tour = problem::parse_tour(&text)
        .map_err(|e: TourError| SearchError::Lkh(format!("{name}: {e}")))?;
    let mut cycle = contraction
        .expand(&tour)
        .map_err(|e| SearchError::Lkh(format!("{name}: {e}")))?;
    cycle::check(graph, &cycle).map_err(|e: CycleError| {
        SearchError::Lkh(format!("{name}: not a Hamiltonian cycle: {e}"))
    })?;
    cycle::canonicalize(&mut cycle);
    let modified = fs::metadata(&path)
        .and_then(|m| m.modified())
        .map_err(io_at(&path))?;
    let when = modified
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    Ok(Some(Found {
        cycle,
        found: crate::time::format_unix(when),
        engine: Engine::Lkh(LkhRun {
            version: lkh.version.clone(),
            tree: lkh.tree.clone(),
            binary_sha256: lkh.binary_sha256.clone(),
            seed: run.seed,
            contracted: true,
            parameters_sha256: crate::sha256_hex(problem::PARAMETERS.as_bytes()),
            guard_overridden: run.guard_overridden,
            segments: run.segments.clone(),
        }),
    }))
}

/// Trials completed, from an LKH log: `Trials.min` when the run ended, else
/// the last trial number of the `* <trial>: Cost = ...` improvement lines.
fn trials(lines: &[String]) -> u64 {
    if let Some(value) = lines.iter().rev().find_map(|l| {
        l.strip_prefix("Trials.min = ")
            .and_then(|r| r.split(',').next())
            .and_then(|v| v.trim().parse().ok())
    }) {
        return value;
    }
    lines
        .iter()
        .filter_map(|l| {
            l.strip_prefix('*')?
                .trim_start()
                .split_once(':')?
                .0
                .parse()
                .ok()
        })
        .max()
        .unwrap_or(0)
}

/// Consecutive complete segments without improvement after which a search
/// gives up.
pub const MAX_STALLED_SEGMENTS: u32 = 100;

/// Whether an LKH log line is worth showing: improvements and summaries.
/// The full log stays in the run directory.
fn is_progress(line: &str) -> bool {
    [
        "*",
        "Run ",
        "Successes/Runs",
        "Trials.min",
        "Time.total",
        "Preprocessing time",
    ]
    .iter()
    .any(|prefix| line.starts_with(prefix))
}

/// Runs segments until a cycle is found or the time limit runs out.
fn segments(
    lkh: &Lkh,
    dir: &Path,
    graph: &Graph,
    contraction: &Contraction,
    run: &mut RunState,
    options: &Options,
    log: &mut dyn Write,
) -> Result<Outcome, SearchError> {
    let deadline = options.time_limit.map(|limit| Instant::now() + limit);
    let mut stalled = 0;
    loop {
        if let Some(found) = found(dir, graph, contraction, run, lkh)? {
            return Ok(Outcome::Found(found));
        }
        let remaining = deadline.map(|d| d.saturating_duration_since(Instant::now()));
        if remaining.is_some_and(|r| r.as_secs() == 0) {
            return Ok(Outcome::TimeUp);
        }

        let number = run.segments.len() + 1;
        let seed = run.seed + run.segments.len() as u64;
        let before = best_tour(dir)?;
        let initial = before.as_ref().map(|(_, name)| name.clone());
        let initial_sha = match &initial {
            Some(name) => {
                Some(crate::sha256_file(&dir.join(name)).map_err(io_at(&dir.join(name)))?)
            }
            None => None,
        };
        let parameters = format!("segment-{number}.par");
        let text =
            problem::segment_parameters(seed, remaining.map(|r| r.as_secs()), initial.as_deref());
        fs::write(dir.join(&parameters), text).map_err(io_at(&dir.join(&parameters)))?;
        let log_path = dir.join(format!("segment-{number}.log"));
        let mut segment_log = fs::File::create(&log_path).map_err(io_at(&log_path))?;
        let _ = writeln!(
            log,
            "posum: segment {number}, seed {seed}{}",
            initial
                .as_ref()
                .map_or(String::new(), |t| format!(", from {t}"))
        );

        let started = crate::time::now();
        let clock = Instant::now();
        let mut child = Command::new(&lkh.path)
            .arg(&parameters)
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| SearchError::Lkh(format!("cannot start {}: {e}", lkh.path.display())))?;

        let (sender, receiver) = mpsc::channel();
        for stream in [
            child
                .stdout
                .take()
                .map(|s| Box::new(s) as Box<dyn io::Read + Send>),
            child
                .stderr
                .take()
                .map(|s| Box::new(s) as Box<dyn io::Read + Send>),
        ]
        .into_iter()
        .flatten()
        {
            let sender = sender.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
            });
        }
        drop(sender);

        // LKH checks its own TOTAL_TIME_LIMIT only between trials, and a trial
        // takes hours at k = 7: past the deadline, posum stops it.
        let mut lines = Vec::new();
        let mut killed = false;
        let status = loop {
            match receiver.recv_timeout(Duration::from_millis(500)) {
                Ok(line) => {
                    let _ = writeln!(segment_log, "{line}");
                    if is_progress(&line) {
                        let _ = writeln!(log, "{line}");
                    }
                    lines.push(line);
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => {}
            }
            if let Some(status) = child
                .try_wait()
                .map_err(|e| SearchError::Lkh(e.to_string()))?
            {
                break status;
            }
            if !killed && deadline.is_some_and(|d| Instant::now() >= d + Duration::from_secs(5)) {
                let _ = writeln!(log, "posum: time limit reached, stopping LKH");
                let _ = child.kill();
                killed = true;
            }
        };
        for line in receiver.try_iter() {
            let _ = writeln!(segment_log, "{line}");
            lines.push(line);
        }

        run.segments.push(Segment {
            seed,
            started,
            seconds: clock.elapsed().as_secs(),
            trials: trials(&lines),
            initial_tour_sha256: initial_sha,
        });
        run.save(dir)?;

        if !killed && !status.success() {
            return Err(SearchError::Lkh(format!(
                "exited with {status}; see {}",
                log_path.display()
            )));
        }

        // A segment that ran all its trials without improving the best tour
        // says little; a long run of them, as on G_2(31), which has no
        // Hamiltonian cycle, means LKH is going round in circles.
        let after = best_tour(dir)?;
        if killed || after.as_ref().map(|b| b.0) != before.as_ref().map(|b| b.0) {
            stalled = 0;
        } else {
            stalled += 1;
            if stalled == MAX_STALLED_SEGMENTS {
                return Err(SearchError::Stalled(stalled));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trials_from_logs() {
        let lines = |text: &str| text.lines().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            trials(&lines(
                "* 1: Cost = 9, Time = 0.1 sec.\n* 4: Cost = 0, Time = 0.2 sec.\n"
            )),
            4
        );
        assert_eq!(
            trials(&lines(
                "* 4: Cost = 0\nTrials.min = 7, Trials.avg = 7.0, Trials.max = 7\n"
            )),
            7
        );
        assert_eq!(trials(&[]), 0);
    }

    #[test]
    fn state_round_trip() {
        let run = RunState {
            k: 5,
            n: 246_877,
            seed: 1,
            nodes: 85_485,
            binary_sha256: "ab".into(),
            guard_overridden: false,
            segments: vec![
                Segment {
                    seed: 1,
                    started: "2026-10-14T12:00:00Z".into(),
                    seconds: 3,
                    trials: 2,
                    initial_tour_sha256: None,
                },
                Segment {
                    seed: 2,
                    started: "2026-10-14T12:00:03Z".into(),
                    seconds: 4,
                    trials: 1,
                    initial_tour_sha256: Some("cd".into()),
                },
            ],
        };
        let dir = std::env::temp_dir().join(format!("posum-state-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        run.save(&dir).unwrap();
        let back = RunState::load(&dir).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(back.text(), run.text());
        assert_eq!(back.segments, run.segments);
    }
}
