//! The `polinrider` binary.
//!
//! Argument handling lives in `cli`, which refuses anything it does not
//! recognise before a single file is read. This binary moves things; a typo
//! must not get as far as doing work.

use polinrider::checks::{OnInfectedConfig, Sink};
use polinrider::cli::{self, Command, Rejection};
use polinrider::guide::{self, Console, Session};
use polinrider::host::{Host, LiveHost, Snapshot};
use polinrider::indicators::Indicators;
use polinrider::quarantine::{Apply, DryRun, Quarantine};
use polinrider::scan::{self, Scope, Target};
use polinrider::verdict::ExitCode;

use std::io::{BufRead, Write};
use std::path::Path;
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
    let host: Option<Box<dyn Host>> = if args.fs_only || args.command == Command::Clean {
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
            // The guided flow can still check directories, and says that
            // "this computer" is not on offer.
            Err(_) if args.command == Command::Guide => None,
            Err(e) => {
                eprintln!("polinrider: {e}");
                return ProcExit::from(ExitCode::CouldNotRun.code() as u8);
            }
        }
    };
    let host = host.as_deref();

    if args.command == Command::Guide {
        return guided(&args, &ind, host);
    }

    let clean = args.command == Command::Clean;
    let mut out = String::new();
    out.push_str(&format!(
        "PolinRider local {} - {} - scan\n",
        args.command.name(),
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
        match (args.apply, clean) {
            (false, _) => "dry run - nothing will be changed",
            (true, false) => "APPLY - confirmed artifacts will be moved to quarantine",
            (true, true) =>
                "APPLY - appended payloads are stripped in place, other confirmed artifacts are moved to quarantine. Every original is kept",
        }
    ));
    out.push_str(&format!(
        "host state: {}\n",
        match host {
            Some(h) => h.describe(),
            None if clean =>
                "not read, clean looks only at the directories it is given".to_string(),
            None => "not read, --fs-only".to_string(),
        }
    ));

    let scope = Scope {
        roots: &args.roots,
        ioc_dir: &args.ioc,
        ind: &ind,
        home: if clean { None } else { Some(&args.home) },
        host,
        on_infected_config: if clean {
            OnInfectedConfig::Strip
        } else {
            OnInfectedConfig::Report
        },
    };

    let v = if args.apply {
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
        let v = scan::run(&scope, &mut Sink::Apply(&mut q));
        if let Err(e) = q.write_manifest() {
            eprintln!("polinrider: could not write the quarantine manifest: {e}");
        }
        v
    } else {
        let q = Quarantine::<DryRun>::new(&args.quarantine);
        scan::run(&scope, &mut Sink::Dry(&q))
    };

    let closing = scan::result(&v);
    print!("{out}{}{closing}", scan::render(&v, Target::Console));
    if let Some(path) = &args.report {
        let body = format!("{out}{}{closing}", scan::render(&v, Target::Report));
        if let Err(e) = write_report(path, &body) {
            eprintln!("polinrider: could not write {}: {e}", path.display());
        }
    }
    ProcExit::from(v.exit_code().code() as u8)
}

fn write_report(path: &Path, body: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, body)
}

/// The terminal, as the guided flow sees it.
struct Terminal;

impl Console for Terminal {
    fn say(&mut self, text: &str) {
        println!("{text}");
    }

    fn ask(&mut self, prompt: &str) -> Option<String> {
        print!("{prompt} ");
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        match std::io::stdin().lock().read_line(&mut line) {
            // Zero bytes is the end of input, which is not the same as an
            // empty line and must not be read as one.
            Ok(0) | Err(_) => {
                println!();
                None
            }
            Ok(_) => Some(line.trim().to_string()),
        }
    }
}

fn guided(args: &cli::Args, ind: &Indicators, host: Option<&dyn Host>) -> ProcExit {
    let session = Session {
        ind,
        ioc_dir: &args.ioc,
        home: &args.home,
        host,
        quarantine: &args.quarantine,
    };
    let outcome = guide::run(&session, &mut Terminal);
    if let Some(path) = &args.report {
        if !outcome.report.is_empty() {
            match write_report(path, &outcome.report) {
                Ok(()) => println!("The full report is in {}", path.display()),
                Err(e) => eprintln!("polinrider: could not write {}: {e}", path.display()),
            }
        }
    }
    ProcExit::from(outcome.exit.code() as u8)
}
