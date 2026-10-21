//! posum: Hamiltonian cycles of power-sum graphs, their certificates and
//! bundles. See `posum --help`.

mod bundle;
mod check;
mod provenance;
mod sweep;

use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use posum_core::graph::{Graph, Vertex};
use posum_core::islands::Chain;
use posum_core::{bases, cycle, format};
use posum_lkh::{Lkh, Options, Outcome, search};
use sha2::{Digest, Sha256};

const HELP: &str = "\
posum: Hamiltonian cycles of power-sum graphs G_k(n)

usage:
  posum islands <k>                     admissible islands of G_k
  posum bands <k> <n>                   lower-bound certificate for n, on stdout
  posum search <k> <n> [options]        find a Hamiltonian cycle with LKH
  posum search --resume <dir> [options] continue a search
  posum sweep <k> <inf> <sup> [options] search every n of a range, in parallel
  posum sweep <k> island [options]      every n of every finite island of G_k
  posum encode <k> <n> <cycle.txt>      plain list to bases, on stdout
  posum decode <k> <n> <cycle.bases>    bases to plain list, on stdout
  posum bundle add [--island | --range <inf> <sup>] [--root <dir>] <run-dir>...
                                        add found witnesses to their bundles
  posum bundle plain [--root <dir>] [--jobs <j>]
                                        rebuild witness/plain/ from witness/bases/
  posum check [--root <dir>] [--jobs <j>]
                                        check every bundle of the bases tree,
                                        and of the plain tree if present, with
                                        both independent verifiers

search options:
  --run-dir <dir>      run directory (default: runs/k<k>-n<n>)
  --time-limit <t>     wall-clock budget: 90, 90s, 15m, 2h or 1d
  --seed <s>           seed of the first segment (default: 1)
  --force              start even above the memory estimate
  --lkh <path>         LKH binary (default: $POSUM_LKH, then lkh/bin/LKH)
  --nice <i>           run LKH under nice (search default: off, sweep: 19)

sweep options, besides --time-limit (per n), --seed, --force, --lkh, --nice:
  --jobs <j>           searches at once (default: cores, lowered to fit memory)
  --run-root <dir>     run directories <dir>/k<k>-n<n> (default: runs)
  --keep               keep problems and tours after success
  Run the same sweep again to resume it; the table is <dir>/sweep.tsv.

bundle and check use --root witness by default; --island puts a witness in
the bundle of the finite admissible island containing n, --range in the range
bundle k<k>-r<inf>-<sup> of consecutive admissible values.

exit codes: 0 done (search: cycle found and checked), 1 error,
            2 search: time limit reached, resumable with --resume";

/// A failure, printed on stderr, exit code 1.
struct Failure(String);

impl<E: std::fmt::Display> From<E> for Failure {
    fn from(e: E) -> Self {
        Self(e.to_string())
    }
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match run(&arguments) {
        Ok(code) => code,
        Err(Failure(message)) => {
            eprintln!("posum: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &[String]) -> Result<ExitCode, Failure> {
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match words.as_slice() {
        [] | ["-h" | "--help" | "help"] => {
            println!("{HELP}");
            Ok(ExitCode::SUCCESS)
        }
        ["-V" | "--version"] => {
            println!("posum {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        ["islands", k] => islands(exponent(k)?),
        ["bands", k, n] => bands(exponent(k)?, integer(n, "n")?),
        ["encode", k, n, path] => encode(&graph(k, n)?, Path::new(path)),
        ["decode", k, n, path] => decode(&graph(k, n)?, Path::new(path)),
        ["search", rest @ ..] => search_command(rest),
        ["bundle", "add", rest @ ..] => bundle_add(rest),
        ["bundle", "plain", rest @ ..] => bundle_plain(rest),
        ["check", rest @ ..] => check_command(rest),
        ["sweep", rest @ ..] => sweep::command(rest),
        _ => Err(Failure(format!(
            "invalid command; see posum --help\n\n{HELP}"
        ))),
    }
}

fn integer(text: &str, name: &str) -> Result<u128, Failure> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Failure(format!(
            "{name} must be a decimal integer, not {text:?}"
        )));
    }
    text.parse()
        .map_err(|_| Failure(format!("{name} = {text} is too large")))
}

fn exponent(text: &str) -> Result<u32, Failure> {
    u32::try_from(integer(text, "k")?).map_err(|_| Failure(format!("k = {text} is too large")))
}

fn graph(k: &str, n: &str) -> Result<Graph, Failure> {
    Ok(Graph::new(exponent(k)?, integer(n, "n")?)?)
}

fn stdout() -> BufWriter<io::StdoutLock<'static>> {
    BufWriter::with_capacity(1 << 20, io::stdout().lock())
}

fn islands(k: u32) -> Result<ExitCode, Failure> {
    format::write_islands(stdout(), &Chain::new(k)?.islands())?;
    Ok(ExitCode::SUCCESS)
}

fn bands(k: u32, n: u128) -> Result<ExitCode, Failure> {
    format::write_bands(stdout(), &Chain::new(k)?.bands(n)?)?;
    Ok(ExitCode::SUCCESS)
}

fn open(path: &Path) -> Result<BufReader<File>, Failure> {
    let file = File::open(path).map_err(|e| Failure(format!("{}: {e}", path.display())))?;
    Ok(BufReader::with_capacity(1 << 20, file))
}

fn encode(graph: &Graph, path: &Path) -> Result<ExitCode, Failure> {
    let cycle = format::read_plain(open(path)?, graph.n())
        .map_err(|e| Failure(format!("{}: {e}", path.display())))?;
    let bases =
        bases::encode(graph, &cycle).map_err(|e| Failure(format!("{}: {e}", path.display())))?;
    format::write_bases(stdout(), &bases)?;
    Ok(ExitCode::SUCCESS)
}

fn decode(graph: &Graph, path: &Path) -> Result<ExitCode, Failure> {
    let bases =
        format::read_bases(open(path)?).map_err(|e| Failure(format!("{}: {e}", path.display())))?;
    let cycle =
        bases::decode(graph, &bases).map_err(|e| Failure(format!("{}: {e}", path.display())))?;
    format::write_plain(stdout(), &cycle)?;
    Ok(ExitCode::SUCCESS)
}

/// The outcome of adding one run directory, filled in by a worker.
type Added = Option<Result<(bundle::Kind, String), Failure>>;

fn bundle_add(words: &[&str]) -> Result<ExitCode, Failure> {
    let mut root = PathBuf::from("witness");
    let mut target = bundle::Target::Value;
    let mut runs = Vec::new();
    let mut rest = words.iter();
    while let Some(&word) = rest.next() {
        match word {
            "--island" => target = bundle::Target::Island,
            "--range" => {
                let mut bound = |name| {
                    rest.next()
                        .ok_or_else(|| Failure("--range needs <inf> <sup>".into()))
                        .and_then(|t| integer(t, name))
                };
                let inf = bound("inf")?;
                let sup = bound("sup")?;
                target = bundle::Target::Range { inf, sup };
            }
            "--root" => {
                root = PathBuf::from(
                    rest.next()
                        .ok_or_else(|| Failure("--root needs a value".into()))?,
                );
            }
            _ if word.starts_with("--") => return Err(Failure(format!("unknown option {word}"))),
            _ => runs.push(PathBuf::from(word)),
        }
    }
    if runs.is_empty() {
        return Err(Failure(
            "usage: posum bundle add [--island | --range <inf> <sup>] [--root <dir>] <run-dir>..."
                .into(),
        ));
    }
    let root = bundle::Root { path: root };
    // Each witness is compressed twice with xz -9e: run them in parallel,
    // then report in the order given.
    let jobs = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<std::sync::Mutex<Added>> =
        runs.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(runs.len()) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(run) = runs.get(index) else { break };
                    let result = bundle::add(&root, run, target);
                    *results[index].lock().expect("result lock") = Some(result);
                }
            });
        }
    });
    let mut kinds = Vec::new();
    for result in results {
        let (kind, message) = result
            .into_inner()
            .expect("result lock")
            .expect("every run done")?;
        println!("{message}");
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }
    for kind in &kinds {
        bundle::refresh(&root, kind)?;
    }
    Ok(ExitCode::SUCCESS)
}

fn bundle_plain(words: &[&str]) -> Result<ExitCode, Failure> {
    let mut root = PathBuf::from("witness");
    let mut jobs = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let mut rest = words.iter();
    while let Some(&word) = rest.next() {
        let mut value = |name: &str| {
            rest.next()
                .copied()
                .ok_or_else(|| Failure(format!("{name} needs a value")))
        };
        match word {
            "--root" => root = PathBuf::from(value("--root")?),
            "--jobs" => {
                jobs = usize::try_from(integer(value("--jobs")?, "jobs")?)
                    .map_err(|_| Failure("too many jobs".into()))?;
            }
            _ => return Err(Failure(format!("unknown argument {word}"))),
        }
    }
    println!("{}", bundle::plain(&bundle::Root { path: root }, jobs)?);
    Ok(ExitCode::SUCCESS)
}

fn check_command(words: &[&str]) -> Result<ExitCode, Failure> {
    let mut root = PathBuf::from("witness");
    let mut jobs = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let mut rest = words.iter();
    while let Some(&word) = rest.next() {
        let mut value = |name: &str| {
            rest.next()
                .copied()
                .ok_or_else(|| Failure(format!("{name} needs a value")))
        };
        match word {
            "--root" => root = PathBuf::from(value("--root")?),
            "--jobs" => {
                jobs = usize::try_from(integer(value("--jobs")?, "jobs")?)
                    .map_err(|_| Failure("too many jobs".into()))?;
            }
            _ => return Err(Failure(format!("unknown argument {word}"))),
        }
    }
    let passed = check::check(&bundle::Root { path: root }, jobs)?;
    Ok(if passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// `90`, `90s`, `15m`, `2h`, `1d`.
fn duration(text: &str) -> Result<Duration, Failure> {
    let (digits, unit) = match text.char_indices().find(|(_, c)| !c.is_ascii_digit()) {
        Some((i, _)) => text.split_at(i),
        None => (text, "s"),
    };
    let scale = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86_400,
        _ => {
            return Err(Failure(format!(
                "invalid time limit {text:?}: use 90, 90s, 15m, 2h or 1d"
            )));
        }
    };
    let value: u64 = digits
        .parse()
        .map_err(|_| Failure(format!("invalid time limit {text:?}")))?;
    Ok(Duration::from_secs(
        value
            .checked_mul(scale)
            .ok_or_else(|| Failure("time limit too large".into()))?,
    ))
}

fn search_command(words: &[&str]) -> Result<ExitCode, Failure> {
    let mut positional = Vec::new();
    let (mut run_dir, mut resume, mut lkh_path) = (None, None, None);
    let mut options = Options {
        time_limit: None,
        seed: 1,
        force: false,
        nice: None,
    };
    let mut rest = words.iter();
    while let Some(&word) = rest.next() {
        let mut value = |name: &str| {
            rest.next()
                .copied()
                .ok_or_else(|| Failure(format!("{name} needs a value")))
        };
        match word {
            "--run-dir" => run_dir = Some(PathBuf::from(value("--run-dir")?)),
            "--resume" => resume = Some(PathBuf::from(value("--resume")?)),
            "--lkh" => lkh_path = Some(PathBuf::from(value("--lkh")?)),
            "--time-limit" => options.time_limit = Some(duration(value("--time-limit")?)?),
            "--seed" => {
                options.seed = u64::try_from(integer(value("--seed")?, "seed")?)
                    .map_err(|_| Failure("seed too large".into()))?;
            }
            "--force" => options.force = true,
            "--nice" => {
                options.nice = Some(
                    u8::try_from(integer(value("--nice")?, "nice")?)
                        .map_err(|_| Failure("nice must be 0 to 19".into()))?
                        .min(19),
                );
            }
            _ if word.starts_with("--") => return Err(Failure(format!("unknown option {word}"))),
            _ => positional.push(word),
        }
    }

    let lkh = Lkh::locate(lkh_path.as_deref())?;
    let mut log = io::stderr();
    let (outcome, dir, k) = match (resume, positional.as_slice()) {
        (Some(dir), []) => {
            if run_dir.is_some() {
                return Err(Failure(
                    "--resume names the run directory; drop --run-dir".into(),
                ));
            }
            let outcome = search::resume(&lkh, &dir, &options, &mut log)?;
            (outcome, dir, None)
        }
        (None, [k, n]) => {
            let (k, n) = (exponent(k)?, integer(n, "n")?);
            let dir = run_dir.unwrap_or_else(|| PathBuf::from(format!("runs/k{k}-n{n}")));
            let outcome = search::start(&lkh, &dir, k, n, &options, &mut log)?;
            (outcome, dir, Some(k))
        }
        _ => {
            return Err(Failure(
                "usage: posum search <k> <n> [options] | posum search --resume <dir> [options]"
                    .into(),
            ));
        }
    };

    match outcome {
        Outcome::TimeUp => {
            eprintln!(
                "posum: time limit reached; continue with: posum search --resume {}",
                dir.display()
            );
            Ok(ExitCode::from(2))
        }
        Outcome::Found(found) => {
            let graph_k = match k {
                Some(k) => k,
                None => read_k(&dir)?,
            };
            let n = found.cycle.len();
            let id = record(&dir, graph_k, &found)?;
            let sha8 = &id[..8];
            eprintln!("posum: Hamiltonian cycle of G_{graph_k}({n}) found and checked");
            eprintln!("posum: {}", dir.join(format!("{sha8}.txt")).display());
            println!("{id}");
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// Checks a found cycle, writes it as `<sha8>.txt` with its provenance in
/// `dir`, and returns its identity.
fn record(dir: &Path, k: u32, found: &posum_lkh::Found) -> Result<String, Failure> {
    let n = Vertex::try_from(found.cycle.len()).map_err(|_| Failure("cycle too long".into()))?;
    let graph = Graph::new(k, u128::from(n))?;
    cycle::check_canonical(&graph, &found.cycle)?;
    fs::create_dir_all(dir).map_err(|e| Failure(format!("{}: {e}", dir.display())))?;
    let id = write_witness(dir, &found.cycle)?;
    let text = provenance::render(
        &id,
        k,
        n,
        found,
        provenance::cpu().as_deref(),
        provenance::system().as_deref(),
    );
    let path = dir.join(format!("{}.provenance.toml", &id[..8]));
    fs::write(&path, text).map_err(|e| Failure(format!("{}: {e}", path.display())))?;
    Ok(id)
}

/// `k` from a run directory's state.
fn read_k(dir: &Path) -> Result<u32, Failure> {
    let path = dir.join(posum_lkh::problem::names::STATE);
    let text =
        fs::read_to_string(&path).map_err(|e| Failure(format!("{}: {e}", path.display())))?;
    let document = posum_lkh::state::parse(&text)?;
    posum_lkh::state::get_int(&document.top, "k")
        .and_then(|k| u32::try_from(k).ok())
        .ok_or_else(|| Failure(format!("{}: no k", path.display())))
}

/// Writes the canonical plain list as `<sha8>.txt` in `dir`, hashing it on
/// the way; returns its identity.
fn write_witness(dir: &Path, cycle: &[Vertex]) -> Result<String, Failure> {
    struct Tee<W: Write> {
        inner: W,
        hasher: Sha256,
    }
    impl<W: Write> Write for Tee<W> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let written = self.inner.write(bytes)?;
            self.hasher.update(&bytes[..written]);
            Ok(written)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.inner.flush()
        }
    }
    let temporary = dir.join("cycle.txt.new");
    let file =
        File::create(&temporary).map_err(|e| Failure(format!("{}: {e}", temporary.display())))?;
    let mut tee = Tee {
        inner: BufWriter::with_capacity(1 << 20, file),
        hasher: Sha256::new(),
    };
    format::write_plain(&mut tee, cycle)?;
    let id = posum_lkh::hex(&tee.hasher.finalize());
    let path = dir.join(format!("{}.txt", &id[..8]));
    fs::rename(&temporary, &path).map_err(|e| Failure(format!("{}: {e}", path.display())))?;
    Ok(id)
}
