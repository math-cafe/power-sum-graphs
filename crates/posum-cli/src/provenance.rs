//! Provenance files, `doc/formats.md` section 9, in canonical form.

use posum_lkh::problem::PARAMETERS_FILE;
use posum_lkh::{Engine, Found};

/// Where provenance files point for the LKH parameters.
pub fn parameters_path() -> String {
    format!("witness/engines/{PARAMETERS_FILE}")
}

/// The canonical provenance of a witness found by this posum.
pub fn render(
    id: &str,
    k: u32,
    n: u32,
    found: &Found,
    cpu: Option<&str>,
    system: Option<&str>,
) -> String {
    let engine = match found.engine {
        Engine::Propagation => "propagation",
        Engine::Lkh(_) => "lkh",
    };
    let mut text = format!(
        "id = \"{id}\"\nk = {k}\nn = {n}\nfound = \"{}\"\nposum = \"{}\"\nengine = \"{engine}\"\n",
        found.found,
        env!("CARGO_PKG_VERSION")
    );
    if let Some(cpu) = cpu {
        text.push_str(&format!("cpu = \"{cpu}\"\n"));
    }
    if let Some(system) = system {
        text.push_str(&format!("system = \"{system}\"\n"));
    }
    if let Engine::Lkh(run) = &found.engine {
        text.push_str(&format!(
            "\n[lkh]\nversion = \"{}\"\nbinary_sha256 = \"{}\"\ntree = \"{}\"\ncontracted = {}\nseed = {}\nsegments = {}\nseconds = {}\ntrials = {}\nparameters = \"{}\"\nparameters_sha256 = \"{}\"\n",
            run.version,
            run.binary_sha256,
            run.tree,
            run.contracted,
            run.seed,
            run.segments.len(),
            run.seconds(),
            run.trials(),
            parameters_path(),
            run.parameters_sha256
        ));
        if run.guard_overridden {
            text.push_str("guard_overridden = true\n");
        }
        if run.segments.len() > 1 {
            for segment in &run.segments {
                text.push_str(&format!(
                    "\n[[lkh.segment]]\nseed = {}\nstarted = \"{}\"\nseconds = {}\ntrials = {}\n",
                    segment.seed, segment.started, segment.seconds, segment.trials
                ));
                if let Some(sha) = &segment.initial_tour_sha256 {
                    text.push_str(&format!("initial_tour_sha256 = \"{sha}\"\n"));
                }
            }
        }
    }
    text
}

/// ASCII printable, without `"` or `\`, single spaces: safe in a TOML string
/// without escapes.
fn clean(text: &str) -> Option<String> {
    let kept: String = text
        .chars()
        .map(|c| if c.is_ascii_whitespace() { ' ' } else { c })
        .filter(|c| c.is_ascii_graphic() && !matches!(c, '"' | '\\') || *c == ' ')
        .collect();
    let joined = kept.split_whitespace().collect::<Vec<_>>().join(" ");
    (!joined.is_empty()).then_some(joined)
}

/// The processor model, from `/proc/cpuinfo`.
pub fn cpu() -> Option<String> {
    let text = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    let line = text.lines().find(|l| l.starts_with("model name"))?;
    clean(line.split_once(':')?.1)
}

/// Operating system and kernel, without the host name.
pub fn system() -> Option<String> {
    let read = |file: &str| std::fs::read_to_string(format!("/proc/sys/kernel/{file}")).ok();
    match (read("ostype"), read("osrelease")) {
        (Some(os), Some(release)) => clean(&format!("{} {}", os.trim(), release.trim())),
        _ => clean(std::env::consts::OS),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use posum_lkh::{LkhRun, Segment};

    fn run(segments: usize) -> Found {
        Found {
            cycle: vec![],
            found: "2026-10-14T12:00:00Z".into(),
            engine: Engine::Lkh(LkhRun {
                version: "3.0.14".into(),
                tree: "three-level".into(),
                binary_sha256: "b".into(),
                seed: 1,
                contracted: true,
                parameters_sha256: "p".into(),
                guard_overridden: false,
                segments: (0..segments)
                    .map(|i| Segment {
                        seed: 1 + i as u64,
                        started: "2026-10-14T11:00:00Z".into(),
                        seconds: 10,
                        trials: 2,
                        initial_tour_sha256: (i > 0).then(|| "t".into()),
                    })
                    .collect(),
            }),
        }
    }

    #[test]
    fn one_segment() {
        let text = render("i", 5, 246_877, &run(1), Some("CPU"), None);
        assert!(
            text.starts_with("id = \"i\"\nk = 5\nn = 246877\nfound = \"2026-10-14T12:00:00Z\"\n")
        );
        assert!(text.contains("engine = \"lkh\"\ncpu = \"CPU\"\n\n[lkh]\nversion = \"3.0.14\"\n"));
        assert!(text.contains("segments = 1\nseconds = 10\ntrials = 2\n"));
        assert!(!text.contains("[[lkh.segment]]") && text.ends_with("parameters_sha256 = \"p\"\n"));
    }

    #[test]
    fn several_segments() {
        let text = render("i", 5, 246_877, &run(2), None, None);
        assert_eq!(text.matches("\n\n[[lkh.segment]]\n").count(), 2);
        assert!(text.contains("seconds = 20\ntrials = 4\n"));
        assert!(text.ends_with("trials = 2\ninitial_tour_sha256 = \"t\"\n"));
    }

    #[test]
    fn cleaning() {
        assert_eq!(
            clean("  Intel(R)  Core \"i9\"\t\\x "),
            Some("Intel(R) Core i9 x".into())
        );
        assert_eq!(clean("é"), None);
    }
}
