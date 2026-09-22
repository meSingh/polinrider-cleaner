//! The `polinrider` binary.
//!
//! Argument parsing is hand-written rather than pulled from a crate. This is
//! the cleanup tool for a package supply-chain campaign; every dependency it
//! does not have is one fewer thing a user has to trust to run it during an
//! incident.

use polinrider::checks::{self, Sink};
use polinrider::indicators::Indicators;
use polinrider::quarantine::{Apply, DryRun, Quarantine};
use polinrider::verdict::{Entry, ExitCode, Level, Verdict};
use polinrider::walk;

use std::path::{Path, PathBuf};
use std::process::ExitCode as ProcExit;

const USAGE: &str = "\
polinrider - detect and clean up after the PolinRider supply-chain campaign.

  polinrider check [options] ROOT...

Options:
  --fs-only            only the checks that read the filesystem being scanned.
                       Skips live processes, sockets, npm config and $HOME
                       persistence, which describe the machine you are running
                       on rather than the disk you pointed at.
  --apply              move confirmed artifacts into quarantine. Never deletes.
  --quarantine DIR     where they go. Default: a directory beside the report.
  --report FILE        write the full report here.
  --state DIR          walk manifest and checkpoints.
  --ioc DIR            indicator set. Default: ioc/ beside the binary's source.
  -h, --help           this.

Exit codes: 0 clean - 1 review items only - 2 a confirmed indicator
            3 the scan could not run.
";

struct Args {
    roots: Vec<PathBuf>,
    fs_only: bool,
    apply: bool,
    quarantine: Option<PathBuf>,
    report: Option<PathBuf>,
    ioc: Option<PathBuf>,
}

fn parse(mut argv: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut a = Args {
        roots: Vec::new(),
        fs_only: false,
        apply: false,
        quarantine: None,
        report: None,
        ioc: None,
    };
    let next = |flag: &str, it: &mut dyn Iterator<Item = String>| {
        it.next().ok_or_else(|| format!("{flag} needs a value"))
    };
    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--fs-only" => a.fs_only = true,
            "--apply" => a.apply = true,
            "--quarantine" => a.quarantine = Some(next("--quarantine", &mut argv)?.into()),
            "--report" => a.report = Some(next("--report", &mut argv)?.into()),
            "--ioc" => a.ioc = Some(next("--ioc", &mut argv)?.into()),
            // Accepted and ignored: resume is not implemented yet, and a flag
            // that is silently dropped is better than one that errors out of a
            // script that used to work. It is reported below.
            "--state" | "--jobs" => {
                let _ = next(&arg, &mut argv)?;
            }
            "-h" | "--help" => return Err(String::new()),
            s if s.starts_with('-') => return Err(format!("unknown argument: {s}")),
            s => a.roots.push(PathBuf::from(s)),
        }
    }
    Ok(a)
}

/// Where `ioc/` lives relative to the running binary, so the tool works from a
/// checkout without being told.
fn default_ioc() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        for dir in exe.ancestors() {
            let candidate = dir.join("ioc");
            if candidate.join("strong.txt").is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from("ioc")
}

fn render(v: &Verdict, out: &mut String) {
    for entry in v.entries() {
        match entry {
            Entry::Section(title) => {
                out.push_str(&format!("\n== {title} ==\n"));
            }
            Entry::Finding(f) => {
                out.push_str(&format!("  {} {}\n", f.level.tag(), f.message));
                if let Some(r) = &f.remedy {
                    out.push_str(&format!("           {r}\n"));
                }
            }
        }
    }
}

fn main() -> ProcExit {
    let args = match parse(std::env::args().skip(1).skip_while(|a| a == "check")) {
        Ok(a) => a,
        Err(msg) => {
            if msg.is_empty() {
                print!("{USAGE}");
                return ProcExit::from(0);
            }
            eprintln!("{msg}\nTry --help");
            return ProcExit::from(ExitCode::CouldNotRun.code() as u8);
        }
    };

    if args.roots.is_empty() {
        eprintln!("polinrider: no roots given. A scan of nothing is not a clean scan.");
        return ProcExit::from(ExitCode::CouldNotRun.code() as u8);
    }

    let ioc_dir = args.ioc.clone().unwrap_or_else(default_ioc);
    let ind = match Indicators::load(&ioc_dir) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("polinrider: {e}");
            return ProcExit::from(ExitCode::CouldNotRun.code() as u8);
        }
    };

    let mut out = String::new();
    out.push_str(&format!(
        "PolinRider local check - {} - {}\n",
        std::env::consts::OS,
        "scan"
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
    for bad in &w.unreadable {
        v.push(polinrider::Finding::review(format!(
            "could not read, so it was not scanned: {}",
            bad.display()
        )));
    }

    // The two quarantine types are different, so the whole scan is run under
    // whichever one this invocation is allowed to use.
    let qroot = args
        .quarantine
        .clone()
        .unwrap_or_else(|| PathBuf::from("polinrider-quarantine"));

    let code = if args.apply {
        let mut q = match Quarantine::<Apply>::create(&qroot) {
            Ok(q) => q,
            Err(e) => {
                eprintln!("polinrider: cannot create {}: {e}", qroot.display());
                return ProcExit::from(ExitCode::CouldNotRun.code() as u8);
            }
        };
        {
            let mut sink = Sink::Apply(&mut q);
            run_checks(&w, &ind, &mut v, &mut sink, args.fs_only);
        }
        if let Err(e) = q.write_manifest() {
            eprintln!("polinrider: could not write the quarantine manifest: {e}");
        }
        finish(&v, &mut out, &args.report)
    } else {
        let q = Quarantine::<DryRun>::new(&qroot);
        let mut sink = Sink::Dry(&q);
        run_checks(&w, &ind, &mut v, &mut sink, args.fs_only);
        finish(&v, &mut out, &args.report)
    };

    ProcExit::from(code.code() as u8)
}

fn run_checks(w: &walk::Walk, ind: &Indicators, v: &mut Verdict, sink: &mut Sink, fs_only: bool) {
    checks::tasks_json(w, ind, v, sink);
    checks::build_configs(w, ind, v);
    checks::fonts(w, v, sink);
    checks::packages(w, ind, v);
    checks::git_hooks(w, ind, v, sink);

    if fs_only {
        for name in [
            "IDE extensions",
            "Propagation artifact",
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
