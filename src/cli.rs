//! Argument parsing, and a preflight that refuses before anything runs.
//!
//! The rule this module exists to enforce: **nothing is scanned, moved or
//! written until every argument has been checked.** A tool that starts work
//! and then discovers a typo has already done something, and this one moves
//! files. Everything below either returns a validated [`Args`] or an error,
//! and the caller exits 3 without touching the filesystem.
//!
//! Three kinds of rejection, deliberately distinguished, because "I do not
//! know that flag" and "I know it and refuse to pretend" are different
//! promises to the person reading the message:
//!
//! - **Unknown**: not a flag this tool has. Lists what it does have.
//! - **Not implemented** or **removed**: a flag the shell version accepts and
//!   this one does not, either not yet or by decision. It refuses rather than
//!   ignoring, because a flag that silently does nothing is how somebody ends
//!   up believing a scan resumed when it started over.
//! - **Invalid**: a real flag with an argument that cannot work. A root that
//!   does not exist is this, not a warning: scanning nothing must never be
//!   reported as scanning clean.

use std::fmt;
use std::path::PathBuf;

/// Every flag this binary accepts. One list, so the help text and the
/// rejection message cannot drift apart.
pub const ACCEPTED: &[(&str, &str)] = &[
    (
        "--fs-only",
        "only the checks that read the filesystem being scanned",
    ),
    (
        "--apply",
        "move confirmed artifacts into quarantine. Never deletes",
    ),
    (
        "--quarantine DIR",
        "where quarantined files go. Default: a new directory in ~",
    ),
    ("--report FILE", "write the full report here"),
    (
        "--ioc DIR",
        "indicator set. Defaults to ioc/ beside the binary",
    ),
    (
        "--home DIR",
        "the home directory to check. Defaults to $HOME",
    ),
    (
        "--host-state DIR",
        "read processes, sockets and crontab from DIR, not this machine",
    ),
    ("-h, --help", "this"),
];

/// Flags the shell implementation accepts that this one does not yet.
/// Refused explicitly rather than ignored.
const NOT_IMPLEMENTED: &[(&str, &str)] = &[
    ("--jobs", "parallel hashing is not implemented yet"),
    ("--background", "detaching is not implemented yet"),
];

/// Flags 1.x had that 2.0 does not have and will not get. ADR-0030. Still
/// recognised, so that somebody arriving from 1.x is told what happened to
/// the flag instead of being told it never existed.
const REMOVED: &[(&str, &str)] = &[
    ("--state", "2.0 does not checkpoint a scan"),
    (
        "--resume",
        "2.0 does not checkpoint a scan, so there is nothing to resume",
    ),
];

#[derive(Debug)]
pub enum Rejection {
    Unknown(String),
    NotImplemented {
        flag: String,
        why: String,
    },
    Removed {
        flag: String,
        why: String,
    },
    NotForCommand {
        flag: String,
        command: &'static str,
    },
    MissingValue(String),
    NoRoots,
    BadRoot {
        path: PathBuf,
        why: String,
    },
    BadIoc {
        path: PathBuf,
    },
    QuarantineUnusable {
        path: PathBuf,
        why: String,
    },
    BadHostState {
        path: PathBuf,
        why: String,
    },
    Conflict {
        first: String,
        second: String,
        why: String,
    },
    HelpRequested,
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rejection::Unknown(flag) => {
                writeln!(f, "unknown option: {flag}")?;
                writeln!(f, "\nNothing was scanned. This tool moves files, so it refuses")?;
                writeln!(f, "anything it does not recognise rather than guessing.\n")?;
                writeln!(f, "What it accepts:")?;
                for (flag, help) in ACCEPTED {
                    writeln!(f, "  {flag:<18} {help}")?;
                }
                Ok(())
            }
            Rejection::NotImplemented { flag, why } => {
                writeln!(f, "{flag} is not available in this build: {why}.")?;
                writeln!(f, "\nRefused rather than ignored. A flag that silently does")?;
                writeln!(f, "nothing is how somebody comes to believe a scan resumed")?;
                writeln!(f, "when it actually started over.\n")?;
                write!(f, "The shell implementation supports it: ./polinrider.sh")
            }
            Rejection::Removed { flag, why } => {
                writeln!(f, "{flag} was removed in 2.0: {why}.")?;
                writeln!(f, "\nNothing was scanned. Every run starts from the beginning and")?;
                writeln!(f, "walks the filesystem once. Refused rather than ignored: a flag")?;
                write!(f, "that silently does nothing would let a scan look resumed.")
            }
            Rejection::NotForCommand { flag, command } => write!(
                f,
                "{flag} does not apply to {command}.\n\nNothing was scanned. Refused rather than ignored: a flag that is accepted\nand then does nothing leaves you believing it did something."
            ),
            Rejection::MissingValue(flag) => write!(f, "{flag} needs a value"),
            Rejection::NoRoots => write!(
                f,
                "no directory given to scan.\n\nA scan of nothing is not a clean scan, so this is an error\nrather than an empty pass."
            ),
            Rejection::BadRoot { path, why } => write!(
                f,
                "cannot scan {}: {why}.\n\nRefused before starting. Scanning the other roots and\nreporting clean would hide the one that was mistyped.",
                path.display()
            ),
            Rejection::BadIoc { path } => write!(
                f,
                "no indicator set at {}.\n\nWithout indicators every scan is clean, which is worse than\nno scan at all.",
                path.display()
            ),
            Rejection::QuarantineUnusable { path, why } => write!(
                f,
                "--apply cannot use {}: {why}.\n\nChecked before scanning, so a run does not get halfway through\nfinding things it then cannot quarantine.",
                path.display()
            ),
            Rejection::BadHostState { path, why } => write!(
                f,
                "--host-state cannot use {}: {why}.\n\nRefused before starting. Falling back to this machine's own state would\nanswer a question nobody asked.",
                path.display()
            ),
            Rejection::Conflict { first, second, why } => write!(
                f,
                "{first} and {second} cannot be given together: {why}.\n\nNothing was scanned."
            ),
            Rejection::HelpRequested => Ok(()),
        }
    }
}

/// What the binary was asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Scan and report. `--apply` moves confirmed artifacts into quarantine
    /// and never changes the contents of a file.
    Check,
    /// Scan the given working trees and, with `--apply`, also cut an appended
    /// payload out of a build config in place. Reads nothing outside the
    /// roots and never touches git. ADR-0031.
    Clean,
    /// The guided flow: asks what to check, scans, and changes something only
    /// on an explicit yes. What running with no arguments does. ADR-0032.
    Guide,
}

impl Command {
    pub const fn name(self) -> &'static str {
        match self {
            Command::Check => "check",
            Command::Clean => "clean",
            Command::Guide => "guide",
        }
    }
}

#[derive(Debug)]
pub struct Args {
    pub command: Command,
    pub roots: Vec<PathBuf>,
    pub fs_only: bool,
    pub apply: bool,
    pub quarantine: PathBuf,
    pub report: Option<PathBuf>,
    pub ioc: PathBuf,
    pub home: PathBuf,
    /// Host state supplied as files, instead of read from this machine.
    pub host_state: Option<PathBuf>,
}

/// Parse and validate. Returns only arguments it is safe to act on.
pub fn parse<I: Iterator<Item = String>>(argv: I, default_ioc: PathBuf) -> Result<Args, Rejection> {
    let mut args = argv.peekable();
    let mut roots: Vec<PathBuf> = Vec::new();
    let (mut fs_only, mut apply) = (false, false);
    let (mut quarantine, mut report, mut ioc, mut home) = (None, None, None, None);
    let mut host_state = None;

    // The subcommand, when present. `check` is the default.
    let command = match args.peek().map(String::as_str) {
        Some("check") => {
            args.next();
            Command::Check
        }
        Some("clean") => {
            args.next();
            Command::Clean
        }
        Some("guide") => {
            args.next();
            Command::Guide
        }
        // Nothing at all: the guided flow, which asks. Anything else without
        // a command is a check, as it always was.
        None => Command::Guide,
        _ => Command::Check,
    };
    // A flag that means nothing to the command it was given with is refused,
    // not ignored. `clean` reads the directories it is given and nothing
    // else; `guide` asks for its directories and for a yes before it writes.
    let not_for = |flag: &str, commands: &[Command]| -> Result<(), Rejection> {
        if commands.contains(&command) {
            return Err(Rejection::NotForCommand {
                flag: flag.to_string(),
                command: command.name(),
            });
        }
        Ok(())
    };
    let not_for_clean = |flag: &str| not_for(flag, &[Command::Clean]);

    while let Some(arg) = args.next() {
        let mut value = |flag: &str| {
            args.next()
                .ok_or_else(|| Rejection::MissingValue(flag.into()))
        };
        match arg.as_str() {
            "--fs-only" => {
                not_for("--fs-only", &[Command::Clean, Command::Guide])?;
                fs_only = true;
            }
            "--apply" => {
                not_for("--apply", &[Command::Guide])?;
                apply = true;
            }
            "--quarantine" => quarantine = Some(PathBuf::from(value("--quarantine")?)),
            "--report" => report = Some(PathBuf::from(value("--report")?)),
            "--ioc" => ioc = Some(PathBuf::from(value("--ioc")?)),
            "--home" => {
                not_for_clean("--home")?;
                home = Some(PathBuf::from(value("--home")?));
            }
            "--host-state" => {
                not_for_clean("--host-state")?;
                host_state = Some(PathBuf::from(value("--host-state")?));
            }
            "-h" | "--help" => return Err(Rejection::HelpRequested),
            other if other.starts_with('-') => {
                // A known-but-unbuilt flag gets its own message. Its value, if
                // it takes one, is consumed so the error names the flag rather
                // than its argument.
                if let Some((flag, why)) = NOT_IMPLEMENTED.iter().find(|(f, _)| *f == other) {
                    return Err(Rejection::NotImplemented {
                        flag: (*flag).to_string(),
                        why: (*why).to_string(),
                    });
                }
                if let Some((flag, why)) = REMOVED.iter().find(|(f, _)| *f == other) {
                    return Err(Rejection::Removed {
                        flag: (*flag).to_string(),
                        why: (*why).to_string(),
                    });
                }
                return Err(Rejection::Unknown(other.to_string()));
            }
            root => {
                not_for("a directory on the command line", &[Command::Guide])?;
                roots.push(PathBuf::from(root));
            }
        }
    }

    // --- preflight. Nothing above this point touched the filesystem. -------
    if roots.is_empty() && command != Command::Guide {
        return Err(Rejection::NoRoots);
    }
    for root in &roots {
        match std::fs::metadata(root) {
            Ok(m) if m.is_dir() => {}
            Ok(_) => {
                return Err(Rejection::BadRoot {
                    path: root.clone(),
                    why: "not a directory".into(),
                })
            }
            Err(e) => {
                return Err(Rejection::BadRoot {
                    path: root.clone(),
                    why: e.to_string(),
                })
            }
        }
    }

    let ioc = ioc.unwrap_or(default_ioc);
    if !ioc.join("strong.txt").is_file() {
        return Err(Rejection::BadIoc { path: ioc });
    }

    let home = home
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/"));

    // A directory of its own for every run, in the home directory. Not the
    // working directory: run from inside a project, that put live malware in
    // a git checkout, one `git add -A` away from being published, and under a
    // root the next scan would walk.
    let quarantine = quarantine.unwrap_or_else(|| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        home.join(format!(
            "polinrider-quarantine-{}",
            crate::quarantine::stamp(now)
        ))
    });
    if apply {
        // Check now, not after a scan has already found something.
        let probe = quarantine.parent().filter(|p| !p.as_os_str().is_empty());
        if let Some(parent) = probe {
            if !parent.exists() {
                return Err(Rejection::QuarantineUnusable {
                    path: quarantine.clone(),
                    why: format!("{} does not exist", parent.display()),
                });
            }
        }
        if quarantine.exists() && !quarantine.is_dir() {
            return Err(Rejection::QuarantineUnusable {
                path: quarantine,
                why: "exists and is not a directory".into(),
            });
        }
    }

    if let Some(dir) = &host_state {
        if fs_only {
            return Err(Rejection::Conflict {
                first: "--fs-only".into(),
                second: "--host-state".into(),
                why: "one says not to read host state and the other supplies it".into(),
            });
        }
        if !dir.is_dir() {
            return Err(Rejection::BadHostState {
                path: dir.clone(),
                why: "not a directory".into(),
            });
        }
    }

    Ok(Args {
        command,
        roots,
        fs_only,
        apply,
        quarantine,
        report,
        ioc,
        home,
        host_state,
    })
}

/// Where `ioc/` sits relative to the running binary.
pub fn default_ioc() -> PathBuf {
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

pub fn usage() -> String {
    let mut s = String::from(
        "polinrider - detect and clean up after the PolinRider supply-chain campaign.\n\n  polinrider                           the guided flow. Asks, scans, and changes\n                                       something only when you type yes\n  polinrider check [options] ROOT...   scan and report\n  polinrider clean [options] REPO...   scan working trees, and with --apply\n                                       strip an appended payload in place\n\nOptions:\n",
    );
    for (flag, help) in ACCEPTED {
        s.push_str(&format!("  {flag:<18} {help}\n"));
    }
    s.push_str("\nclean takes --apply, --quarantine, --report and --ioc. It never touches git:\nnothing is staged, committed, reset or stashed, and the original of every\nfile it changes is kept in quarantine.\n");
    s.push_str("\nExit codes: 0 clean - 1 review items only - 2 a confirmed indicator\n            3 the scan could not run.\n");
    s
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn ioc_fixture(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("prc-cli-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&d).expect("mkdir");
        std::fs::write(d.join("strong.txt"), "marker\n").expect("write");
        d
    }

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn an_unknown_flag_is_refused_and_the_message_lists_what_is_accepted() {
        let ioc = ioc_fixture("unknown");
        let e = parse(args(&["--delete-everything", "/tmp"]).into_iter(), ioc)
            .expect_err("must refuse");
        assert!(matches!(e, Rejection::Unknown(ref f) if f == "--delete-everything"));
        let msg = e.to_string();
        assert!(msg.contains("Nothing was scanned"), "{msg}");
        assert!(
            msg.contains("--fs-only"),
            "the message must list real flags: {msg}"
        );
    }

    #[test]
    fn a_known_but_unbuilt_flag_says_so_rather_than_being_ignored() {
        let ioc = ioc_fixture("notimpl");
        let e = parse(args(&["--jobs", "4", "/tmp"]).into_iter(), ioc).expect_err("must refuse");
        assert!(matches!(e, Rejection::NotImplemented { ref flag, .. } if flag == "--jobs"));
        assert!(e.to_string().contains("Refused rather than ignored"));
    }

    #[test]
    fn a_flag_removed_in_2_0_says_it_was_removed() {
        // The whole point: --state used to be accepted and silently dropped,
        // and a scan that started over looked like one that had resumed.
        for flag in ["--state", "--resume"] {
            let ioc = ioc_fixture("removed");
            let e =
                parse(args(&[flag, "/tmp/s", "/tmp"]).into_iter(), ioc).expect_err("must refuse");
            assert!(matches!(e, Rejection::Removed { flag: ref f, .. } if f == flag));
            let msg = e.to_string();
            assert!(msg.contains("was removed in 2.0"), "{msg}");
            assert!(msg.contains("Nothing was scanned"), "{msg}");
        }
    }

    #[test]
    fn a_root_that_does_not_exist_stops_the_run() {
        // Not a warning. Scanning the remaining roots and reporting clean
        // would hide the mistyped one.
        let ioc = ioc_fixture("badroot");
        let e = parse(args(&["/tmp", "/definitely/not/here"]).into_iter(), ioc)
            .expect_err("must refuse");
        assert!(matches!(e, Rejection::BadRoot { .. }));
    }

    #[test]
    fn no_roots_is_an_error_not_an_empty_clean_pass() {
        let ioc = ioc_fixture("noroots");
        assert!(matches!(
            parse(args(&["--fs-only"]).into_iter(), ioc),
            Err(Rejection::NoRoots)
        ));
    }

    #[test]
    fn a_missing_indicator_set_stops_the_run() {
        let absent = std::env::temp_dir().join("prc-cli-no-ioc-at-all");
        let _ = std::fs::remove_dir_all(&absent);
        assert!(matches!(
            parse(args(&["/tmp"]).into_iter(), absent),
            Err(Rejection::BadIoc { .. })
        ));
    }

    #[test]
    fn a_flag_missing_its_value_is_refused() {
        let ioc = ioc_fixture("missingval");
        assert!(matches!(
            parse(args(&["/tmp", "--report"]).into_iter(), ioc),
            Err(Rejection::MissingValue(_))
        ));
    }

    #[test]
    fn supplied_host_state_is_refused_alongside_fs_only() {
        // One flag says "do not read host state", the other supplies some.
        // Picking a winner silently would run a different scan from the one
        // that was asked for.
        let ioc = ioc_fixture("conflict");
        let e = parse(
            args(&["--fs-only", "--host-state", "/tmp", "/tmp"]).into_iter(),
            ioc,
        )
        .expect_err("must refuse");
        assert!(matches!(e, Rejection::Conflict { .. }));
        assert!(e.to_string().contains("Nothing was scanned"));
    }

    #[test]
    fn a_host_state_directory_that_is_not_there_stops_the_run() {
        // Falling back to the live machine would report this laptop's state
        // as the state of whatever the directory was meant to describe.
        let ioc = ioc_fixture("nohoststate");
        let e = parse(
            args(&["--host-state", "/definitely/not/here", "/tmp"]).into_iter(),
            ioc,
        )
        .expect_err("must refuse");
        assert!(matches!(e, Rejection::BadHostState { .. }));
    }

    #[test]
    fn clean_is_a_command_and_refuses_flags_that_mean_nothing_to_it() {
        let ioc = ioc_fixture("clean");
        let a = parse(
            args(&["clean", "--apply", "--ioc", ioc.to_str().unwrap(), "/tmp"]).into_iter(),
            PathBuf::from("unused"),
        )
        .expect("valid");
        assert_eq!(a.command, Command::Clean);
        assert!(a.apply);

        for flag in ["--fs-only", "--home", "--host-state"] {
            let e = parse(
                args(&["clean", flag, "/tmp", "/tmp"]).into_iter(),
                ioc_fixture("clean-flags"),
            )
            .expect_err("must refuse");
            assert!(
                matches!(e, Rejection::NotForCommand { flag: ref f, .. } if f == flag),
                "{flag}: {e}"
            );
        }
    }

    #[test]
    fn no_arguments_at_all_is_the_guided_flow() {
        let ioc = ioc_fixture("guide");
        let a = parse(args(&[]).into_iter(), ioc).expect("valid");
        assert_eq!(a.command, Command::Guide);
        assert!(a.roots.is_empty());
    }

    #[test]
    fn the_guided_flow_refuses_to_be_told_to_apply() {
        // It asks before it writes. A flag that answers for the operator
        // would make "nothing is changed unless you type yes" untrue.
        for extra in ["--apply", "--fs-only", "/tmp"] {
            let e = parse(
                args(&["guide", extra]).into_iter(),
                ioc_fixture("guide-flags"),
            )
            .expect_err("must refuse");
            assert!(matches!(e, Rejection::NotForCommand { .. }), "{extra}: {e}");
        }
    }

    #[test]
    fn the_default_quarantine_is_a_new_directory_in_the_home_directory() {
        // Never the working directory, which may be the project being cleaned.
        let ioc = ioc_fixture("defaultq");
        let a = parse(
            args(&["check", "--fs-only", "--home", "/somewhere/home", "/tmp"]).into_iter(),
            ioc,
        )
        .expect("valid");
        let name = a.quarantine.file_name().unwrap().to_str().unwrap();
        assert_eq!(
            a.quarantine.parent().unwrap(),
            std::path::Path::new("/somewhere/home")
        );
        assert!(name.starts_with("polinrider-quarantine-20"), "{name}");
        assert!(name.ends_with('Z'), "{name}");
    }

    #[test]
    fn valid_arguments_parse() {
        let ioc = ioc_fixture("valid");
        let a = parse(
            args(&["check", "--fs-only", "--ioc", ioc.to_str().unwrap(), "/tmp"]).into_iter(),
            PathBuf::from("unused"),
        )
        .expect("valid");
        assert_eq!(a.command, Command::Check);
        assert!(a.fs_only);
        assert!(!a.apply);
        assert_eq!(a.roots, vec![PathBuf::from("/tmp")]);
    }
}
