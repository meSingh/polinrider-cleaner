//! The `polinrider` binary.
//!
//! Argument handling lives in `cli`, which refuses anything it does not
//! recognise before a single file is read. This binary moves things; a typo
//! must not get as far as doing work.

use polinrider::checks::{self, Sink};
use polinrider::cli::{self, Rejection};
use polinrider::host::{Host, LiveHost, Snapshot};
use polinrider::host_checks;
use polinrider::indicators::Indicators;
use polinrider::quarantine::{Apply, DryRun, Quarantine};
use polinrider::verdict::{clean, Entry, ExitCode, Level, Verdict};
use polinrider::walk;

use std::path::{Path, PathBuf};
use std::process::ExitCode as ProcExit;

fn main() -> ProcExit {
    let args = match cli::parse(std::env::args().skip(1), cli::default_ioc()) {
        Ok(a) => a,
        Err(Rejection::HelpRequested) => {
            print!("{}", cli::usage());
            return ProcExit::from(0);
        }
        Err(e) => {
            eprintln!("polinrider: {e}");
            return ProcExit::from(ExitCode::CouldNotRun.code() as u8);
        }
    };

    let ind = match Indicators::load(&args.ioc) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("polinrider: {e}");
            return ProcExit::from(ExitCode::CouldNotRun.code() as u8);
        }
    };

    // The host is settled before anything is printed or walked. Supplied state
    // that does not load, or a platform whose live checks are not built, is a
    // scan that cannot run, not one that runs with a third of it missing.
    let host: Option<Box<dyn Host>> = if args.fs_only {
        None
    } else if let Some(dir) = &args.host_state {
        match Snapshot::load(dir) {
            Ok(s) => Some(Box::new(s)),
            Err(e) => {
                eprintln!("polinrider: {e}");
                return ProcExit::from(ExitCode::CouldNotRun.code() as u8);
            }
        }
    } else {
        match LiveHost::new(&args.home) {
            Ok(h) => Some(Box::new(h)),
            Err(e) => {
                eprintln!("polinrider: {e}");
                return ProcExit::from(ExitCode::CouldNotRun.code() as u8);
            }
        }
    };
    let host = host.as_deref();

    let mut out = String::new();
    out.push_str(&format!(
        "PolinRider local check - {} - scan\n",
        std::env::consts::OS
    ));
    out.push_str(&format!(
        "roots: {}\n",
        args.roots
            .iter()
            .map(|r| r.display().to_string())
            .collect::<Vec<_>>()
            .join(" ")
    ));
    out.push_str(&format!(
        "mode: {}\n",
        if args.apply {
            "APPLY - confirmed artifacts will be moved to quarantine"
        } else {
            "dry run - nothing will be changed"
        }
    ));
    out.push_str(&format!(
        "host state: {}\n",
        match host {
            Some(h) => h.describe(),
            None => "not read, --fs-only".to_string(),
        }
    ));

    let mut v = Verdict::new();
    v.section("Filesystem walk");
    let w = walk::walk(&args.roots);
    v.push(polinrider::Finding::info(format!(
        "{} files listed. Not walked: {}",
        w.files.len(),
        walk::PRUNED.join(", ")
    )));
    // Roots were validated before this point, so anything unreadable here is a
    // subdirectory the current user cannot open. Reported, never silent.
    for bad in &w.unreadable {
        v.push(polinrider::Finding::review(format!(
            "could not read, so it was not scanned: {}",
            bad.display()
        )));
    }

    let code = if args.apply {
        let mut q = match Quarantine::<Apply>::create(&args.quarantine) {
            Ok(q) => q,
            Err(e) => {
                eprintln!(
                    "polinrider: cannot create {}: {e}",
                    args.quarantine.display()
                );
                return ProcExit::from(ExitCode::CouldNotRun.code() as u8);
            }
        };
        {
            let mut sink = Sink::Apply(&mut q);
            run_checks(&w, &ind, &args, host, &mut v, &mut sink);
        }
        if let Err(e) = q.write_manifest() {
            eprintln!("polinrider: could not write the quarantine manifest: {e}");
        }
        finish(&v, &out, &args.report)
    } else {
        let q = Quarantine::<DryRun>::new(&args.quarantine);
        let mut sink = Sink::Dry(&q);
        run_checks(&w, &ind, &args, host, &mut v, &mut sink);
        finish(&v, &out, &args.report)
    };

    ProcExit::from(code.code() as u8)
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

fn run_checks(
    w: &walk::Walk,
    ind: &Indicators,
    args: &cli::Args,
    host: Option<&dyn Host>,
    v: &mut Verdict,
    sink: &mut Sink,
) {
    // One line per host check that did not run, so a section that was skipped
    // can never be mistaken for one that found nothing.
    let skipped = |v: &mut Verdict, name: &str| {
        v.section(format!("{name}: skipped, --fs-only"));
    };

    checks::implants(w, ind, &args.home, &args.ioc, host, v, sink);

    if host.is_some() {
        checks::extensions(&extension_dirs(&args.home), ind, v, sink);
    } else {
        skipped(v, "IDE extensions");
    }

    checks::tasks_json(w, ind, v, sink);
    checks::build_configs(w, ind, v);
    checks::fonts(w, v, sink);

    if host.is_some() {
        checks::propagation(w, v, sink);
    } else {
        skipped(v, "Propagation artifact");
    }

    checks::packages(w, ind, v);

    match host {
        Some(host) => {
            host_checks::persistence(host, &args.home, ind, v, sink);
            host_checks::shell_startup(&args.home, host.platform(), ind, v);
        }
        None => {
            skipped(v, "Persistence");
            skipped(v, "Shell startup files");
        }
    }

    checks::git_hooks(w, ind, host, v, sink);

    match host {
        Some(host) => {
            host_checks::npm_config(&args.home, ind, v);
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

/// Where a rendering is going. The console gets evidence cut to a readable
/// width and no inventory; the report file gets all of it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Console,
    Report,
}

/// How much of one evidence line the console shows. The report has the rest.
const CONSOLE_WIDTH: usize = 110;

fn render(v: &Verdict, target: Target) -> String {
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

fn finish(v: &Verdict, header: &str, report: &Option<PathBuf>) -> ExitCode {
    let hits = v.count(Level::Hit);
    let reviews = v.count(Level::Review);
    let mut result = String::from("\n== RESULT ==\n");
    result.push_str(&format!("  confirmed indicator hits : {hits}\n"));
    result.push_str(&format!("  items needing a human    : {reviews}\n"));
    result.push('\n');

    let code = v.exit_code();
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

    print!("{header}{}{result}", render(v, Target::Console));
    if let Some(path) = report {
        let body = format!("{header}{}{result}", render(v, Target::Report));
        if let Err(e) = write_report(path, &body) {
            eprintln!("polinrider: could not write {}: {e}", path.display());
        }
    }
    code
}

fn write_report(path: &Path, body: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, body)
}
