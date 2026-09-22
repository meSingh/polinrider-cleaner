//! The `polinrider` binary.
//!
//! Argument handling lives in `cli`, which refuses anything it does not
//! recognise before a single file is read. This binary moves things; a typo
//! must not get as far as doing work.

use polinrider::checks::{self, Sink};
use polinrider::cli::{self, Rejection};
use polinrider::indicators::Indicators;
use polinrider::quarantine::{Apply, DryRun, Quarantine};
use polinrider::verdict::{Entry, ExitCode, Level, Verdict};
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
            run_checks(&w, &ind, &args, &mut v, &mut sink);
        }
        if let Err(e) = q.write_manifest() {
            eprintln!("polinrider: could not write the quarantine manifest: {e}");
        }
        finish(&v, &mut out, &args.report)
    } else {
        let q = Quarantine::<DryRun>::new(&args.quarantine);
        let mut sink = Sink::Dry(&q);
        run_checks(&w, &ind, &args, &mut v, &mut sink);
        finish(&v, &mut out, &args.report)
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
    v: &mut Verdict,
    sink: &mut Sink,
) {
    checks::implants(w, ind, &args.home, &args.ioc, v, sink);

    if args.fs_only {
        v.section("IDE extensions: skipped, --fs-only");
    } else {
        checks::extensions(&extension_dirs(&args.home), ind, v, sink);
    }

    checks::tasks_json(w, ind, v, sink);
    checks::build_configs(w, ind, v);
    checks::fonts(w, v, sink);

    if args.fs_only {
        v.section("Propagation artifact: skipped, --fs-only");
    } else {
        checks::propagation(w, v, sink);
    }

    checks::packages(w, ind, v);
    checks::git_hooks(w, ind, v, sink);

    if args.fs_only {
        for name in [
            "Persistence",
            "Shell startup files",
            "npm configuration",
            "Resident interpreters",
            "Live connections",
        ] {
            v.section(format!("{name}: skipped, --fs-only"));
        }
    }
}

fn render(v: &Verdict, out: &mut String) {
    for entry in v.entries() {
        match entry {
            Entry::Section(title) => out.push_str(&format!("\n== {title} ==\n")),
            Entry::Finding(f) => {
                out.push_str(&format!("  {} {}\n", f.level.tag(), f.message));
                if let Some(r) = &f.remedy {
                    out.push_str(&format!("           {r}\n"));
                }
            }
        }
    }
}

fn finish(v: &Verdict, out: &mut String, report: &Option<PathBuf>) -> ExitCode {
    render(v, out);

    let hits = v.count(Level::Hit);
    let reviews = v.count(Level::Review);
    out.push_str("\n== RESULT ==\n");
    out.push_str(&format!("  confirmed indicator hits : {hits}\n"));
    out.push_str(&format!("  items needing a human    : {reviews}\n"));
    out.push('\n');

    let code = v.exit_code();
    if hits > 0 {
        let word = if hits == 1 { "indicator" } else { "indicators" };
        let w = 54usize;
        let bar = "#".repeat(w + 7);
        let row = |s: &str| format!("  ##   {s:<w$}##\n");
        out.push_str(&format!("  {bar}\n"));
        out.push_str(&row(""));
        out.push_str(&row("VERDICT: COMPROMISED"));
        out.push_str(&row(""));
        out.push_str(&row(&format!("{hits} confirmed {word} found.")));
        out.push_str(&row("This machine cannot be trusted until it is rebuilt."));
        out.push_str(&row(""));
        out.push_str(&format!("  {bar}\n"));
    } else if reviews > 0 {
        out.push_str("VERDICT: no confirmed indicator.\n");
    } else {
        out.push_str("VERDICT: clean against the current indicator set.\n");
    }

    print!("{out}");
    if let Some(path) = report {
        if let Err(e) = write_report(path, out) {
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
