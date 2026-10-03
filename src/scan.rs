//! One scan: a walk of the roots, then every check that applies.
//!
//! Lives in the library, not the binary, so that the guided flow can run a
//! scan, read its verdict and run another without starting a second process,
//! and so that a test can do the same.

use crate::checks::{self, OnInfectedConfig, Sink};
use crate::host::Host;
use crate::host_checks;
use crate::indicators::Indicators;
use crate::verdict::{clean, Entry, Finding, Level, Verdict};
use crate::walk;
use std::path::{Path, PathBuf};

/// What a scan covers.
pub struct Scope<'a> {
    pub roots: &'a [PathBuf],
    pub ioc_dir: &'a Path,
    pub ind: &'a Indicators,
    /// The home directory, when the scan is of a machine or of a disk that
    /// has one. `None` when it is of the given directories and nothing else,
    /// which is what `clean` does.
    pub home: Option<&'a Path>,
    /// The machine, when its live state is to be read. `None` skips the host
    /// checks and says so.
    pub host: Option<&'a dyn Host>,
    pub on_infected_config: OnInfectedConfig,
}

fn extension_dirs(home: &Path) -> Vec<PathBuf> {
    [
        ".vscode/extensions",
        ".vscode-insiders/extensions",
        ".cursor/extensions",
        ".windsurf/extensions",
        ".vscode-oss/extensions",
        ".var/app/com.visualstudio.code/data/vscode/extensions",
    ]
    .iter()
    .map(|p| home.join(p))
    .collect()
}

/// Walk the roots once and run the checks. Whether anything is moved or
/// stripped is decided by the sink, which is a type: see `quarantine`.
pub fn run(scope: &Scope, sink: &mut Sink) -> Verdict {
    let mut v = Verdict::new();
    v.section("Filesystem walk");
    let w = walk::walk_with(scope.roots, &walk::Options::skipping(sink.root()));
    v.push(Finding::info(format!(
        "{} files listed. Not walked: {}",
        w.files.len(),
        walk::PRUNED.join(", ")
    )));
    // Roots are validated before a scan starts, so anything unreadable here is
    // a subdirectory the current user cannot open. Reported, never silent.
    for bad in &w.unreadable {
        v.push(Finding::review(format!(
            "could not read, so it was not scanned: {}",
            bad.display()
        )));
    }

    match scope.home {
        Some(home) => machine(&w, scope, home, &mut v, sink),
        None => directories(&w, scope, &mut v, sink),
    }
    v
}

/// A machine, or a disk with a home directory on it.
fn machine(w: &walk::Walk, scope: &Scope, home: &Path, v: &mut Verdict, sink: &mut Sink) {
    let (ind, host) = (scope.ind, scope.host);
    // One line per host check that did not run, so a section that was skipped
    // can never be mistaken for one that found nothing.
    let skipped = |v: &mut Verdict, name: &str| {
        v.section(format!("{name}: skipped, --fs-only"));
    };

    checks::implants(w, ind, Some(home), scope.ioc_dir, host, v, sink);

    if host.is_some() {
        checks::extensions(&extension_dirs(home), ind, v, sink);
    } else {
        skipped(v, "IDE extensions");
    }

    checks::tasks_json(w, ind, v, sink);
    checks::build_configs(w, ind, scope.on_infected_config, v, sink);
    checks::fonts(w, v, sink);

    if host.is_some() {
        checks::propagation(w, v, sink);
    } else {
        skipped(v, "Propagation artifact");
    }

    checks::packages(w, ind, v);

    match host {
        Some(host) => {
            host_checks::persistence(host, home, ind, v, sink);
            host_checks::shell_startup(home, host.platform(), ind, v);
        }
        None => {
            skipped(v, "Persistence");
            skipped(v, "Shell startup files");
        }
    }

    checks::git_hooks(w, ind, host, v, sink);

    match host {
        Some(host) => {
            host_checks::npm_config(home, ind, v);
            host_checks::interpreters(host, ind, v);
            host_checks::connections(host, ind, v);
        }
        None => {
            skipped(v, "npm configuration");
            skipped(v, "Resident interpreters");
            skipped(v, "Live connections");
        }
    }
}

/// The given directories and nothing else: every check that reads them, and
/// none that read a home directory or a host.
fn directories(w: &walk::Walk, scope: &Scope, v: &mut Verdict, sink: &mut Sink) {
    let ind = scope.ind;
    checks::implants(w, ind, None, scope.ioc_dir, None, v, sink);
    checks::tasks_json(w, ind, v, sink);
    checks::build_configs(w, ind, scope.on_infected_config, v, sink);
    checks::fonts(w, v, sink);
    checks::propagation(w, v, sink);
    checks::packages(w, ind, v);
    checks::git_hooks(w, ind, None, v, sink);
}

/// Where a rendering is going. The console gets evidence cut to a readable
/// width and no inventory; the report file gets all of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Console,
    Report,
}

/// How much of one evidence line the console shows. The report has the rest.
const CONSOLE_WIDTH: usize = 110;

pub fn render(v: &Verdict, target: Target) -> String {
    let mut out = String::new();
    // Everything below can carry bytes chosen by whoever planted what is
    // being reported, so every line goes through `clean` on its way out.
    for entry in v.entries() {
        match entry {
            Entry::Section(title) => out.push_str(&format!("\n== {} ==\n", clean(title))),
            Entry::Finding(f) => {
                out.push_str(&format!("  {} {}\n", f.level.tag(), clean(&f.message)));
                for line in f.remedy.iter().flat_map(|r| r.lines()) {
                    out.push_str(&format!("           {}\n", clean(line)));
                }
            }
            Entry::Detail(line) => {
                let line = clean(line);
                if target == Target::Console && line.chars().count() > CONSOLE_WIDTH {
                    let cut: String = line.chars().take(CONSOLE_WIDTH).collect();
                    out.push_str(&format!("    {cut} ...\n"));
                } else {
                    out.push_str(&format!("    {line}\n"));
                }
            }
            Entry::Note(line) => {
                if target == Target::Report {
                    out.push_str(&format!("    {}\n", clean(line)));
                }
            }
        }
    }
    out
}

/// The counts and the verdict, as they close every report.
pub fn result(v: &Verdict) -> String {
    let hits = v.count(Level::Hit);
    let reviews = v.count(Level::Review);
    let mut result = String::from("\n== RESULT ==\n");
    result.push_str(&format!("  confirmed indicator hits : {hits}\n"));
    result.push_str(&format!("  items needing a human    : {reviews}\n"));
    result.push('\n');

    if hits > 0 {
        let word = if hits == 1 { "indicator" } else { "indicators" };
        let w = 54usize;
        let bar = "#".repeat(w + 7);
        let row = |s: &str| format!("  ##   {s:<w$}##\n");
        result.push_str(&format!("  {bar}\n"));
        result.push_str(&row(""));
        result.push_str(&row("VERDICT: COMPROMISED"));
        result.push_str(&row(""));
        result.push_str(&row(&format!("{hits} confirmed {word} found.")));
        result.push_str(&row("This machine cannot be trusted until it is rebuilt."));
        result.push_str(&row(""));
        result.push_str(&format!("  {bar}\n"));
    } else if reviews > 0 {
        result.push_str("VERDICT: no confirmed indicator.\n");
    } else {
        result.push_str("VERDICT: clean against the current indicator set.\n");
    }
    result
}
