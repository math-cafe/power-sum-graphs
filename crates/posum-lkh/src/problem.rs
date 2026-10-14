//! The files LKH reads and writes: problem, parameters, tours.

use std::fmt;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use posum_core::contraction::Contraction;
use posum_core::propagation::Propagation;

/// The parameters every posum search gives LKH, published once under
/// `witness/engines/` and referenced by every provenance. A search appends
/// only file names, the seed, a time limit and an initial tour.
///
/// - A Hamiltonian cycle is a tour of cost 0: `OPTIMUM = 0` and
///   `STOP_AT_OPTIMUM` stop at the first one.
/// - Every edge costs 0, so the subgradient ascent LKH runs by default has
///   nothing to learn; at `k = 5` it turned a 5 s run into one stopped after
///   ten minutes.
/// - One run per segment: posum starts the next segment itself, with a new
///   seed and the best tour so far.
pub const PARAMETERS: &str = "SUBGRADIENT = NO\n\
                              OPTIMUM = 0\n\
                              STOP_AT_OPTIMUM = YES\n\
                              RUNS = 1\n\
                              TRACE_LEVEL = 1\n";

/// The name of [`PARAMETERS`] under `witness/engines/`.
pub const PARAMETERS_FILE: &str = "lkh-posum-1.par";

/// File names inside a run directory.
pub mod names {
    /// The TSPLIB problem.
    pub const PROBLEM: &str = "problem.hcp";
    /// The fixed parameters, a copy of [`super::PARAMETERS`].
    pub const PARAMETERS: &str = "lkh.par";
    /// The search state.
    pub const STATE: &str = "run.toml";
    /// The tour LKH writes at the end of a run.
    pub const FINAL_TOUR: &str = "final.tour";
    /// The tours LKH writes at each improvement, `$` standing for the cost.
    pub const BEST_TOUR_PATTERN: &str = "best.$.tour";
}

/// Writes the contracted problem in TSPLIB HCP form: the free edges and one
/// link per chain, the links also listed as fixed edges.
///
/// # Errors
///
/// On a write error.
pub fn write_problem(
    path: &Path,
    propagation: &Propagation,
    contraction: &Contraction,
) -> io::Result<()> {
    let graph = propagation.graph();
    let mut file = BufWriter::with_capacity(1 << 20, File::create(path)?);
    writeln!(file, "NAME : G{}_{}", graph.k(), graph.n())?;
    writeln!(
        file,
        "COMMENT : power-sum graph G_{}({}) after degree-two propagation, {} chains contracted to {} nodes; written by posum",
        graph.k(),
        graph.n(),
        contraction.chain_count(),
        contraction.node_count()
    )?;
    writeln!(file, "TYPE : HCP")?;
    writeln!(file, "DIMENSION : {}", contraction.node_count())?;
    writeln!(file, "EDGE_DATA_FORMAT : EDGE_LIST")?;
    writeln!(file, "EDGE_DATA_SECTION")?;
    for (x, y) in contraction
        .free_edges(propagation)
        .chain(contraction.links().iter().copied())
    {
        writeln!(file, "{x} {y}")?;
    }
    writeln!(file, "-1")?;
    writeln!(file, "FIXED_EDGES_SECTION")?;
    for &(x, y) in contraction.links() {
        writeln!(file, "{x} {y}")?;
    }
    writeln!(file, "-1")?;
    writeln!(file, "EOF")?;
    file.flush()
}

/// The parameters of one segment: [`PARAMETERS`] plus the run's own lines.
#[must_use]
pub fn segment_parameters(
    seed: u64,
    time_limit: Option<u64>,
    initial_tour: Option<&str>,
) -> String {
    let mut text = String::from(PARAMETERS);
    text.push_str(&format!("PROBLEM_FILE = {}\n", names::PROBLEM));
    text.push_str(&format!("TOUR_FILE = {}\n", names::FINAL_TOUR));
    text.push_str(&format!(
        "OUTPUT_TOUR_FILE = {}\n",
        names::BEST_TOUR_PATTERN
    ));
    text.push_str(&format!("SEED = {seed}\n"));
    if let Some(seconds) = time_limit {
        text.push_str(&format!("TOTAL_TIME_LIMIT = {seconds}\n"));
    }
    if let Some(tour) = initial_tour {
        text.push_str(&format!("INITIAL_TOUR_FILE = {tour}\n"));
    }
    text
}

/// A tour file that cannot be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TourError {
    /// No `TOUR_SECTION`.
    MissingSection,
    /// A token that is not a node number.
    InvalidNode(String),
    /// No terminating `-1`.
    Unterminated,
}

impl fmt::Display for TourError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSection => write!(f, "no TOUR_SECTION"),
            Self::InvalidNode(token) => write!(f, "invalid node {token:?}"),
            Self::Unterminated => write!(f, "tour not terminated by -1"),
        }
    }
}

impl std::error::Error for TourError {}

/// The node sequence of a TSPLIB tour: the integers after `TOUR_SECTION`, up
/// to the terminating `-1`.
///
/// # Errors
///
/// When the section is missing, unterminated or holds something else.
pub fn parse_tour(text: &str) -> Result<Vec<u32>, TourError> {
    let mut lines = text.lines();
    lines
        .by_ref()
        .find(|line| line.trim() == "TOUR_SECTION")
        .ok_or(TourError::MissingSection)?;
    let mut tour = Vec::new();
    for token in lines.flat_map(str::split_whitespace) {
        if token == "-1" {
            return Ok(tour);
        }
        tour.push(
            token
                .parse()
                .map_err(|_| TourError::InvalidNode(token.to_owned()))?,
        );
    }
    Err(TourError::Unterminated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tours() {
        let text = "NAME : x\nCOMMENT : Length = 0\nTYPE : TOUR\nDIMENSION : 3\nTOUR_SECTION\n1\n3\n2\n-1\nEOF\n";
        assert_eq!(parse_tour(text), Ok(vec![1, 3, 2]));
        assert_eq!(parse_tour("1\n2\n"), Err(TourError::MissingSection));
        assert_eq!(
            parse_tour("TOUR_SECTION\n1\n2\n"),
            Err(TourError::Unterminated)
        );
        assert_eq!(
            parse_tour("TOUR_SECTION\n1\nx\n-1\n"),
            Err(TourError::InvalidNode("x".into()))
        );
    }

    #[test]
    fn parameters() {
        let text = segment_parameters(3, Some(60), Some("best.2.tour"));
        assert!(text.starts_with(PARAMETERS));
        assert!(text.contains("SEED = 3\n") && text.contains("TOTAL_TIME_LIMIT = 60\n"));
        assert!(text.ends_with("INITIAL_TOUR_FILE = best.2.tour\n"));
    }
}
