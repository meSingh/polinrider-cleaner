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
use polinrider::remote::{self, Forge, GitHub, Supplied};
use polinrider::scan::{self, Run, Scope, Target};
use polinrider::ui::{Span, Ui};
use polinrider::verdict::ExitCode;

use std::io::{BufRead, Write};
use std::path::Path;
use std::process::ExitCode as ProcExit;

/// Print to standard output, and carry on if nobody is reading.
///
/// `println!` panics when the reader has gone away, and `polinrider | head`
/// is an ordinary thing to type. A scan that has finished has an exit code to
/// return whether or not anybody read its report to the end, and a crash with
/// a Rust backtrace hint is not what a security tool should say on its way
/// out. `print_stdout` is denied in Cargo.toml so that this stays the only
/// way anything is printed.
fn emit(text: &str) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(text.as_bytes());
    let _ = out.flush();
}

fn main() -> ProcExit {
    let ui = Ui::for_stdout();
    let args = match cli::parse(std::env::args().skip(1), cli::default_ioc()) {
        Ok(a) => a,
        Err(Rejection::HelpRequested) => {
            emit(&cli::usage());
            return ProcExit::from(0);
        }
        Err(Rejection::VersionRequested) => {
            emit(&about(&ui));
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

    // The wordmark, on every run. On the console only: a report file is read
    // by tools and attached to tickets, and has no use for it.
    emit(&format!("{}\n", ui.banner(&cli::version())));

    if args.command == Command::Guide {
        return guided(&args, &ind, host, ui);
    }

    let clean = args.command == Command::Clean;
    let out = opening(&args, host);

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

    let closing = format!(
        "{}{}",
        scan::result(&v),
        scan::next_steps(
            &v,
            &Run {
                command: args.command.name(),
                scope_flags: &args.scope_flags,
                roots: &args.roots,
                applied: args.apply,
                quarantine: &args.quarantine,
            }
        )
    );
    emit(&ui.paint(&format!(
        "{out}{}{closing}",
        scan::render(&v, Target::Console)
    )));
    if let Some(path) = &args.report {
        let body = format!("{out}{}{closing}", scan::render(&v, Target::Report));
        if let Err(e) = write_report(path, &body) {
            eprintln!("polinrider: could not write {}: {e}", path.display());
        }
    }
    ProcExit::from(v.exit_code().code() as u8)
}

/// The name people call the system this binary was built for.
fn system_name() -> &'static str {
    match std::env::consts::OS {
        "linux" => "Linux",
        "macos" => "macOS",
        "windows" => "Windows",
        other => other,
    }
}

/// The opening of a report, in sentences: what was detected, what kind of run
/// this is, and whether anything will be changed. Said before the first
/// finding, because "did that just move my files" is the wrong thing to be
/// wondering while reading a result. The same text opens the report file.
fn opening(args: &cli::Args, host: Option<&dyn Host>) -> String {
    let clean = args.command == Command::Clean;
    let mut out = format!("PolinRider local {}\n\n", args.command.name());

    out.push_str(&format!("  Detected a {} system.\n", system_name()));
    out.push_str(match (clean, host) {
        (true, _) => "  A clean of the directories below and nothing else. This machine is not examined.\n",
        (false, Some(h)) if h.is_live() => "  A local check: this machine, and the directories below.\n",
        (false, Some(_)) => "  A check of supplied host state, and the directories below. NOT this machine.\n",
        (false, None) => "  A check of files only (--fs-only). Processes, sockets and login items are not read.\n",
    });
    out.push_str(&match (args.apply, clean) {
        (false, _) => {
            "  DRY RUN, read-only. No changes will be made at this stage.\n".to_string()
        }
        (true, false) => format!(
            "  APPLY. Confirmed artifacts will be moved to quarantine. Nothing is deleted.\n  Originals go to {}\n",
            args.quarantine.display()
        ),
        (true, true) => format!(
            "  APPLY. Appended payloads will be stripped in place and other confirmed artifacts moved to quarantine. Nothing is deleted.\n  Originals go to {}\n",
            args.quarantine.display()
        ),
    });

    out.push_str(&format!(
        "\n  directories  {}\n",
        args.roots
            .iter()
            .map(|r| r.display().to_string())
            .collect::<Vec<_>>()
            .join("\n               ")
    ));
    out.push_str(&format!(
        "  host state   {}\n",
        match host {
            Some(h) => h.describe(),
            None if clean => "not read".to_string(),
            None => "not read, --fs-only".to_string(),
        }
    ));
    out
}

fn write_report(path: &Path, body: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, body)
}

/// What `--version` prints: the banner, then which build this is and what it
/// can match. Counts, not paths: where the indicator files live is of no use
/// to somebody checking what they have, and how many there are is.
fn about(ui: &Ui) -> String {
    let mut out = ui.banner(&cli::version());
    out.push('\n');
    out.push_str(&ui.fact("version", &ui.accent(cli::VERSION)));
    out.push_str(&ui.fact(
        "build",
        &match cli::commit() {
            Some(commit) => ui.bold(commit),
            None => ui.dim("not recorded"),
        },
    ));
    out.push_str(&ui.fact("platform", std::env::consts::OS));
    match Indicators::load(&cli::default_ioc()) {
        Ok(i) => {
            let sep = if ui.unicode { " · " } else { ", " };
            let count = |n: usize, what: &str| format!("{} {what}", ui.accent(&n.to_string()));
            // `strong` is held with the package names merged in, because a
            // package name found in a file counts. Shown apart here.
            let counts = [
                count(
                    i.strong.len().saturating_sub(i.bad_packages.len()),
                    "strong",
                ),
                count(i.bad_packages.len(), "packages"),
                count(i.network.len(), "network"),
                count(i.implant_names.len(), "implant names"),
                count(i.weak.len(), "weak"),
            ];
            out.push_str(&ui.fact("indicators", &counts.join(sep)));
        }
        // The one case where the location matters: it is what has to be fixed.
        Err(e) => {
            out.push_str(&ui.fact(
                "indicators",
                &ui.alarm("NOT USABLE. Every scan will be refused."),
            ));
            out.push_str(&ui.fact("", &e.to_string()));
        }
    }
    out
}

/// The terminal, as the guided flow sees it.
struct Terminal {
    ui: Ui,
    /// Lines of the progress block now on screen, to be drawn over.
    drawn: usize,
}

impl Console for Terminal {
    fn say(&mut self, line: &[Span]) {
        emit(&format!("{}\n", self.ui.line(line)));
    }

    fn progress(&mut self, lines: &[Vec<Span>], last: bool) {
        if !self.ui.live {
            // A pipe or a log gets the finished state once, not every frame.
            if last {
                for line in lines {
                    self.say(line);
                }
            }
            return;
        }
        let mut frame = String::new();
        if self.drawn > 0 {
            // Back up over the previous frame.
            frame.push_str(&format!("\x1b[{}A", self.drawn));
        }
        for line in lines {
            // Clear the line, then draw it.
            frame.push_str(&format!("\x1b[2K{}\n", self.ui.line(line)));
        }
        emit(&frame);
        self.drawn = if last { 0 } else { lines.len() };
    }

    fn ask(&mut self) -> Option<String> {
        emit(&format!("  {} ", self.ui.accent(">")));
        let mut line = String::new();
        match std::io::stdin().lock().read_line(&mut line) {
            // Zero bytes is the end of input, which is not the same as an
            // empty line and must not be read as one.
            Ok(0) | Err(_) => {
                emit("\n");
                None
            }
            Ok(_) => Some(line.trim().to_string()),
        }
    }
}

fn guided(args: &cli::Args, ind: &Indicators, host: Option<&dyn Host>, ui: Ui) -> ProcExit {
    // The screens show a summary, so the whole of it has to be somewhere.
    // A guided run always saves a report, in the home directory unless told
    // where.
    let report = args.report.clone().unwrap_or_else(|| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        args.home.join(format!(
            "polinrider-report-{}.txt",
            polinrider::quarantine::stamp(now)
        ))
    });
    let forge: Box<dyn Forge> = match &args.forge_state {
        Some(dir) => Box::new(Supplied::new(dir)),
        None => Box::new(GitHub),
    };
    let evidence = args
        .evidence
        .clone()
        .unwrap_or_else(remote::default_evidence_dir);
    let session = Session {
        ind,
        ioc_dir: &args.ioc,
        home: &args.home,
        host,
        quarantine: &args.quarantine,
        report: &report,
        system: system_name(),
        unicode: ui.unicode,
        forge: forge.as_ref(),
        evidence: &evidence,
    };
    let outcome = guide::run(&session, &mut Terminal { ui, drawn: 0 });
    if let Err(e) = write_report(&report, &outcome.report) {
        eprintln!(
            "polinrider: the report could not be saved to {}: {e}",
            report.display()
        );
    }
    ProcExit::from(outcome.exit.code() as u8)
}
