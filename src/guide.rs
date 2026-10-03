//! The guided flow: one calm session from "something is wrong" to "here is
//! what was found, what was done about it and what is left for you".
//!
//! Running `polinrider` with no arguments lands here. The person using it has
//! just learned they may have malware on every machine they own, so the flow
//! is built for somebody who is not reading carefully (ADR-0035):
//!
//! - **One question per screen**, with room around it and the answer on a
//!   line of its own.
//! - **Answers are words**: `computer`, `folder`, `yes`, `no`, `details`.
//!   Never a number to match against a list.
//! - **Every step says where you are** and whether it changes anything.
//! - **A summary first**, in plain words. The full list is shown only when
//!   asked for, and is always in the report file.
//!
//! Three rules govern every prompt, each of which exists because the shell
//! version got it wrong once (ADR-0021, ADR-0022, ADR-0024, ADR-0032):
//!
//! - **Only `yes` changes anything.** Enter only ever picks a choice that
//!   reads and never one that writes.
//! - **Only `q` leaves.** A blank line never quits.
//! - **Input that ends, stops.** If stdin closes, the session ends where it
//!   is with nothing further changed, and says so.
//!
//! All reading and printing goes through [`Console`], so a test drives a
//! whole session from a list of answers. Lines are built from [`Span`]s and
//! never from marked-up strings, so a path found on disk is only ever text.

use crate::checks::{OnInfectedConfig, Sink};
use crate::host::Host;
use crate::indicators::Indicators;
use crate::quarantine::{Apply, DryRun, Quarantine};
use crate::remote::{Forge, OwnerKind};
use crate::scan::{self, Scope, Target};
use crate::ui::{text_of, Span, Tone};
use crate::verdict::{clean, ExitCode, Finding, Kind, Level, Verdict};
use std::path::{Path, PathBuf};

/// Where the session talks and listens.
pub trait Console {
    /// Print one line.
    fn say(&mut self, line: &[Span]);
    /// Show the prompt mark on a line of its own and read one line, trimmed.
    /// `None` when input has ended.
    fn ask(&mut self) -> Option<String>;
    /// Show how far a long job has come. Called many times with the same
    /// number of lines; a terminal redraws them in place. `last` is true for
    /// the final call, which is the only one a log or a pipe needs.
    fn progress(&mut self, lines: &[Vec<Span>], last: bool);
}

/// Everything a session needs that was decided before it started.
pub struct Session<'a> {
    pub ind: &'a Indicators,
    pub ioc_dir: &'a Path,
    pub home: &'a Path,
    /// This machine's live state, when it can be read. Without it the session
    /// can still check folders.
    pub host: Option<&'a dyn Host>,
    pub quarantine: &'a Path,
    /// Where the full report will be saved. Named on the last screen, because
    /// the screens themselves deliberately do not show everything.
    pub report: &'a Path,
    /// What to call the system this is running on: "Linux", "macOS".
    pub system: &'a str,
    /// Whether the terminal can draw a rule with box characters.
    pub unicode: bool,
    /// GitHub, or something standing in for it.
    pub forge: &'a dyn Forge,
    /// Where copies of repositories are kept while they are checked.
    pub evidence: &'a Path,
}

/// What a finished session leaves behind.
pub struct Outcome {
    pub exit: ExitCode,
    /// Every scan of the session in full, for the report file.
    pub report: String,
}

/// Why a step did not produce a value.
pub(crate) enum Stop {
    /// The operator typed `q`.
    Quit,
    /// Input ended.
    Ended,
    /// The operator typed `back`.
    Back,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum What {
    Computer,
    Folders,
    Organization,
    Account,
}

// --- saying things -----------------------------------------------------------

pub(crate) fn p(text: impl Into<String>) -> Span {
    Span::new(Tone::Plain, text)
}
pub(crate) fn word(text: impl Into<String>) -> Span {
    Span::new(Tone::Accent, text)
}
pub(crate) fn good(text: impl Into<String>) -> Span {
    Span::new(Tone::Good, text)
}
pub(crate) fn warn(text: impl Into<String>) -> Span {
    Span::new(Tone::Warn, text)
}
pub(crate) fn bad(text: impl Into<String>) -> Span {
    Span::new(Tone::Bad, text)
}
pub(crate) fn dim(text: impl Into<String>) -> Span {
    Span::new(Tone::Dim, text)
}
pub(crate) fn strong(text: impl Into<String>) -> Span {
    Span::new(Tone::Strong, text)
}

pub(crate) fn blank(io: &mut dyn Console, lines: usize) {
    for _ in 0..lines {
        io.say(&[]);
    }
}

pub(crate) fn line(io: &mut dyn Console, text: &str) {
    io.say(&[p(text)]);
}

pub(crate) fn read(io: &mut dyn Console) -> Result<String, Stop> {
    blank(io, 1);
    match io.ask() {
        None => Err(Stop::Ended),
        Some(answer) => {
            let answer = answer.to_ascii_lowercase();
            if answer == "q" || answer == "quit" {
                Err(Stop::Quit)
            } else {
                Ok(answer)
            }
        }
    }
}

/// The top of a step: where you are, what it is, and whether it changes
/// anything.
pub(crate) fn header(
    io: &mut dyn Console,
    session: &Session,
    step: usize,
    title: &str,
    reads_only: bool,
) {
    let rule = if session.unicode { "─" } else { "-" }.repeat(56);
    blank(io, 2);
    io.say(&[dim(format!("  {rule}"))]);
    io.say(&[
        strong(format!("  STEP {step} OF 4")),
        p(format!("   {title}")),
    ]);
    io.say(&[dim(format!("  {rule}"))]);
    if reads_only {
        blank(io, 1);
        io.say(&[dim("  Nothing is changed in this step.")]);
    }
    blank(io, 2);
}

/// A path as somebody would say it: under the home directory it starts `~`.
/// Cleaned, because it came off a disk.
pub(crate) fn tilde(path: &Path, home: &Path) -> String {
    let shown = match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    };
    clean(&shown)
}

/// 48210 as 48,210.
pub(crate) fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub(crate) fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

// --- the session -------------------------------------------------------------

/// Run a session. Never panics on bad input and never writes without a yes.
pub fn run(session: &Session, io: &mut dyn Console) -> Outcome {
    let mut report = String::from("PolinRider guided check\n");
    io.say(&[word(format!("  {} detected.", session.system))]);
    io.say(&[good("  Nothing will be changed unless you type yes.")]);

    let mut worst: Option<ExitCode> = None;
    match steps(session, io, &mut report, &mut worst) {
        Err(Stop::Quit) => {
            blank(io, 2);
            line(io, "  Stopped. Nothing further was changed.");
        }
        Err(Stop::Ended) => {
            blank(io, 2);
            line(
                io,
                "  Input ended. Stopped here, and nothing further was changed.",
            );
        }
        Err(Stop::Back) | Ok(()) => {}
    }

    let exit = match worst {
        Some(code) => code,
        None => {
            // An incomplete run is not a clean one.
            line(
                io,
                "  Nothing was checked, so this says nothing about whether you are infected.",
            );
            ExitCode::CouldNotRun
        }
    };
    blank(io, 1);
    Outcome { exit, report }
}

fn steps(
    session: &Session,
    io: &mut dyn Console,
    report: &mut String,
    worst: &mut Option<ExitCode>,
) -> Result<(), Stop> {
    // --- 1: what. A step that is backed out of returns here. --------------
    loop {
        let what = choose_what(session, io)?;
        let outcome = match what {
            What::Computer | What::Folders => local(session, what, io, report, worst),
            What::Organization => {
                crate::guide_github::run(session, OwnerKind::Organization, io, report, worst)
            }
            What::Account => {
                crate::guide_github::run(session, OwnerKind::Account, io, report, worst)
            }
        };
        match outcome {
            Err(Stop::Back) => continue,
            other => return other,
        }
    }
}

/// This computer, or folders on it: steps 2 to 4.
fn local(
    session: &Session,
    what: What,
    io: &mut dyn Console,
    report: &mut String,
    worst: &mut Option<ExitCode>,
) -> Result<(), Stop> {
    let roots = choose_roots(session, what, io)?;

    let scope = Scope {
        roots: &roots,
        ioc_dir: session.ioc_dir,
        ind: session.ind,
        home: (what == What::Computer).then_some(session.home),
        host: if what == What::Computer {
            session.host
        } else {
            None
        },
        // The dry run is asked what a strip would do, so that the number
        // shown before the question is the number done after it.
        on_infected_config: OnInfectedConfig::Strip,
    };

    // --- 3: check, and say what was found -----------------------------------
    blank(io, 2);
    io.say(&[dim(if what == What::Computer {
        "  Checking now. This only reads. A whole home folder can take a few minutes."
    } else {
        "  Checking now. This only reads."
    })]);
    let dry = Quarantine::<DryRun>::new(session.quarantine);
    let found = scan::run(&scope, &mut Sink::Dry(&dry));
    report.push_str(&format!(
        "\nfirst check, read-only\n{}{}",
        scan::render(&found, Target::Report),
        scan::result(&found)
    ));
    *worst = Some(found.exit_code());

    header(io, session, 3, "What I found", false);
    io.say(&[p(format!(
        "  Checked {} {} in {}{}.",
        thousands(found.files()),
        if found.files() == 1 { "file" } else { "files" },
        count(roots.len(), "folder", "folders"),
        if what == What::Computer {
            ", and this computer"
        } else {
            ""
        }
    ))]);
    blank(io, 2);
    tally(io, &found);

    let mut current: Vec<Finding> = found.findings().cloned().collect();
    let mut declined = 0usize;
    if found.hits() == 0 {
        if found.reviews() > 0 {
            blank(io, 2);
            listing(io, session, &found, Level::Review);
        }
    } else {
        blank(io, 2);
        summary(io, &found, what);
        let fixable = fixable(&found);
        blank(io, 2);
        if fixable.total() == 0 {
            line(io, "  None of these can be fixed for you.");
            line(io, "  The next step says what to do.");
            blank(io, 2);
            io.say(&[p("  Press "), word("Enter"), p(" to continue.")]);
            read(io)?;
        } else if offer(io, session, &found, &fixable)? {
            let after = contain(session, &scope, io, report, &fixable);
            current = after.findings().cloned().collect();
            blank(io, 2);
            io.say(&[p("  Press "), word("Enter"), p(" to continue.")]);
            read(io)?;
        } else {
            declined = fixable.total();
            blank(io, 2);
            line(io, "  Left as it is. Nothing was moved or stripped.");
            blank(io, 2);
            io.say(&[p("  Press "), word("Enter"), p(" to continue.")]);
            read(io)?;
        }
    }

    // --- 4: what to do now --------------------------------------------------
    header(io, session, 4, "What to do now", false);
    let last = what_now(session, &found, &current, declined, what);
    for spans in &last {
        io.say(spans);
        report.push_str(&text_of(spans));
        report.push('\n');
    }
    blank(io, 2);
    line(io, "  The full report, with every finding:");
    io.say(&[p(format!("  {}", tilde(session.report, session.home)))]);
    blank(io, 1);
    io.say(&[p("  To check again:   "), word("polinrider")]);
    Ok(())
}

fn choose_what(session: &Session, io: &mut dyn Console) -> Result<What, Stop> {
    header(io, session, 1, "What should I check?", true);
    if session.host.is_some() {
        io.say(&[
            word("      computer"),
            p("       This whole computer: your home folder,"),
        ]);
        line(
            io,
            "                     login items and what is running right now.",
        );
    } else {
        io.say(&[
            dim("      computer"),
            dim("       Not available in this build on this system."),
        ]);
    }
    blank(io, 1);
    io.say(&[
        word("      folder"),
        p("         One folder, repository or drive."),
    ]);
    line(io, "                     Files only.");
    blank(io, 1);
    io.say(&[
        word("      organization"),
        p("   Every repository and branch of a"),
    ]);
    line(io, "                     GitHub organization.");
    blank(io, 1);
    io.say(&[
        word("      account"),
        p("        Every repository you own on GitHub."),
    ]);
    blank(io, 2);
    line(io, "  Type one of the words above, then press Enter.");
    io.say(&[dim("  q quits. Nothing has been changed.")]);

    loop {
        match read(io)?.as_str() {
            "computer" | "c" if session.host.is_some() => return Ok(What::Computer),
            "computer" | "c" => {
                blank(io, 1);
                io.say(&[warn(
                    "  That is not available here. Type one of the others.",
                )]);
            }
            "folder" | "f" => return Ok(What::Folders),
            "organization" | "organisation" | "org" | "o" => return Ok(What::Organization),
            "account" | "a" => return Ok(What::Account),
            _ => {
                blank(io, 1);
                io.say(&[warn(
                    "  Type computer, folder, organization or account. q quits.",
                )]);
            }
        }
    }
}

/// The folders people usually keep code in, that exist under this home.
fn usual_roots(home: &Path) -> Vec<PathBuf> {
    [
        "Sites",
        "Projects",
        "code",
        "dev",
        "src",
        "projects",
        "work",
        "git",
        "Documents",
    ]
    .iter()
    .map(|d| home.join(d))
    .filter(|d| d.is_dir())
    .collect()
}

fn expand(home: &Path, typed: &str) -> PathBuf {
    match typed.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None if typed == "~" => home.to_path_buf(),
        None => PathBuf::from(typed),
    }
}

fn choose_roots(session: &Session, what: What, io: &mut dyn Console) -> Result<Vec<PathBuf>, Stop> {
    header(
        io,
        session,
        2,
        if what == What::Computer {
            "Where should I look?"
        } else {
            "Which folder?"
        },
        true,
    );
    // What Enter accepts. For the computer it is the whole home folder:
    // somebody who wanted one folder would have said folder, and checking a
    // computer by looking in one code directory is the same check twice
    // under two names. For a folder it is the places code usually lives.
    let suggested = if what == What::Computer {
        vec![session.home.to_path_buf()]
    } else {
        usual_roots(session.home)
    };
    if what == What::Computer {
        line(io, "  Your home folder, and everything in it:");
        blank(io, 1);
        io.say(&[p(format!(
            "      {}",
            clean(&session.home.display().to_string())
        ))]);
        blank(io, 2);
        io.say(&[p("  Press "), word("Enter"), p(" to check it.")]);
        line(io, "  Or type a different folder, then press Enter.");
    } else if suggested.is_empty() {
        line(io, "  Type the folder to check, then press Enter.");
    } else {
        line(io, "  I found these code folders in your home folder:");
        blank(io, 1);
        for dir in &suggested {
            io.say(&[p(format!("      {}", tilde(dir, session.home)))]);
        }
        blank(io, 2);
        io.say(&[p("  Press "), word("Enter"), p(" to check these.")]);
        line(io, "  Or type a different folder, then press Enter.");
    }

    let mut roots: Vec<PathBuf> = Vec::new();
    loop {
        // Not lowercased like other answers: this one is a path.
        blank(io, 1);
        let typed = match io.ask() {
            None => return Err(Stop::Ended),
            Some(typed) => typed,
        };
        match typed.to_ascii_lowercase().as_str() {
            "q" | "quit" => return Err(Stop::Quit),
            "back" | "b" => return Err(Stop::Back),
            "" => {
                if !roots.is_empty() {
                    return Ok(roots);
                }
                if !suggested.is_empty() {
                    return Ok(suggested);
                }
                // A blank line with nothing chosen asks again. It does not
                // check nothing and call it clean.
                blank(io, 1);
                io.say(&[warn("  I need a folder to check. Type one, or q to quit.")]);
            }
            _ => {
                let dir = expand(session.home, &typed);
                blank(io, 1);
                if dir.is_dir() {
                    if !roots.contains(&dir) {
                        roots.push(dir.clone());
                    }
                    io.say(&[good("      added  "), p(tilde(&dir, session.home))]);
                    blank(io, 1);
                    io.say(&[
                        p("  Press "),
                        word("Enter"),
                        p(" to start, or type another folder."),
                    ]);
                } else {
                    io.say(&[
                        warn("  That folder does not exist: "),
                        p(tilde(&dir, session.home)),
                    ]);
                    io.say(&[dim("  Type it again. back returns to the first question.")]);
                }
            }
        }
    }
}

// --- step 3: what was found ---------------------------------------------------

/// The two numbers that matter, each with what it means.
fn tally(io: &mut dyn Console, v: &Verdict) {
    if v.hits() == 0 && v.reviews() == 0 {
        io.say(&[
            good("      NOTHING FOUND"),
            p("     clean against today's indicators"),
        ]);
        return;
    }
    if v.hits() > 0 {
        io.say(&[
            bad(format!("      CONFIRMED   {}", v.hits())),
            p("     the PolinRider payload is here"),
        ]);
    } else {
        io.say(&[good("      CONFIRMED   0"), p("     nothing confirmed")]);
    }
    if v.reviews() > 0 {
        io.say(&[
            warn(format!("      TO REVIEW   {}", v.reviews())),
            p("     needs your eyes, may be nothing"),
        ]);
    }
}

/// The same kind however it will be dealt with: two configs are two configs.
fn family(kind: Kind) -> Kind {
    match kind {
        Kind::Config { .. } => Kind::Config { strippable: true },
        other => other,
    }
}

/// Confirmed findings as counts of plain things, in the order first seen.
fn grouped(v: &Verdict, ran_here: bool) -> Vec<(Kind, usize)> {
    let mut groups: Vec<(Kind, usize)> = Vec::new();
    for kind in v.kinds().filter(|k| k.ran_here() == ran_here).map(family) {
        match groups.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, n)) => *n += 1,
            None => groups.push((kind, 1)),
        }
    }
    groups
}

/// What was confirmed, in plain words and without a single path, and the one
/// sentence that says what it means.
fn summary(io: &mut dyn Console, v: &Verdict, what: What) {
    let projects = grouped(v, false);
    let beyond = grouped(v, true);
    if !projects.is_empty() {
        io.say(&[strong("  In your projects")]);
        for (kind, n) in &projects {
            io.say(&[p(format!("      {}", kind.plain(*n)))]);
        }
    }
    if !beyond.is_empty() {
        if !projects.is_empty() {
            blank(io, 1);
        }
        io.say(&[strong(if what == What::Computer {
            "  On this computer"
        } else {
            "  Signs that it has run"
        })]);
        for (kind, n) in &beyond {
            io.say(&[p(format!("      {}", kind.plain(*n)))]);
        }
    }
    blank(io, 1);
    match beyond.first() {
        Some((kind, _)) => io.say(&[bad(format!(
            "  {} means the payload has run {}.",
            kind.evidence(),
            if what == What::Computer {
                "on this computer"
            } else {
                "where these files came from"
            }
        ))]),
        None => {
            line(io, "  The payload is in project files.");
            line(io, "  Nothing here shows that it has run.");
        }
    }
}

/// What a yes would do.
struct Fixable {
    strip: usize,
    moves: usize,
}

impl Fixable {
    fn total(&self) -> usize {
        self.strip + self.moves
    }
}

fn fixable(v: &Verdict) -> Fixable {
    Fixable {
        strip: v
            .kinds()
            .filter(|k| matches!(k, Kind::Config { strippable: true }))
            .count(),
        moves: v.kinds().filter(|k| k.movable()).count(),
    }
}

/// The one question that can change anything. Returns whether the answer was
/// yes. Anything that is not yes, no or details asks again.
fn offer(
    io: &mut dyn Console,
    session: &Session,
    found: &Verdict,
    fixable: &Fixable,
) -> Result<bool, Stop> {
    let of = format!("{} of the {}", fixable.total(), found.hits());
    io.say(&[p(format!("  I can deal with {of} now:"))]);
    blank(io, 1);
    if fixable.strip > 0 {
        io.say(&[p(format!(
            "      strip the payload out of {}",
            count(fixable.strip, "config file", "config files")
        ))]);
    }
    if fixable.moves > 0 {
        io.say(&[p(format!(
            "      move {} into quarantine",
            count(fixable.moves, "file", "files")
        ))]);
    }
    blank(io, 1);
    io.say(&[
        good("  Nothing is deleted."),
        p(" Every original is kept in"),
    ]);
    io.say(&[p(format!("  {}", tilde(session.quarantine, session.home)))]);
    blank(io, 2);
    io.say(&[p("  Type "), word("yes"), p(" to do it.")]);
    io.say(&[
        p("  Type "),
        word("no"),
        p(" to leave everything as it is."),
    ]);
    io.say(&[
        p("  Type "),
        word("details"),
        p(" to see every finding first."),
    ]);

    loop {
        match read(io)?.as_str() {
            "yes" | "y" => return Ok(true),
            "no" | "n" => return Ok(false),
            "details" | "d" => {
                blank(io, 2);
                listing(io, session, found, Level::Hit);
                if found.reviews() > 0 {
                    blank(io, 2);
                    listing(io, session, found, Level::Review);
                }
                blank(io, 2);
                io.say(&[
                    p("  Type "),
                    word("yes"),
                    p(format!(" to deal with {of}, or ")),
                    word("no"),
                    p(" to leave them."),
                ]);
            }
            _ => {
                blank(io, 1);
                io.say(&[warn(
                    "  Type yes, no or details. Nothing happens until you do.",
                )]);
            }
        }
    }
}

/// Every finding of one level: what it is, then where, on two lines.
fn listing(io: &mut dyn Console, session: &Session, v: &Verdict, level: Level) {
    io.say(&[if level == Level::Hit {
        bad("  CONFIRMED")
    } else {
        warn("  TO REVIEW")
    }]);
    for finding in v.findings().filter(|f| f.level == level) {
        blank(io, 1);
        io.say(&[p(format!("      {}", clean(finding.what())))]);
        if let Some(path) = &finding.path {
            io.say(&[dim(format!("      {}", tilde(path, session.home)))]);
        }
    }
}

/// Move and strip, say what was done, and check again. Returns the second
/// check, which is what the last screen is written from.
fn contain(
    session: &Session,
    scope: &Scope,
    io: &mut dyn Console,
    report: &mut String,
    fixable: &Fixable,
) -> Verdict {
    blank(io, 2);
    let mut quarantine = match Quarantine::<Apply>::create(session.quarantine) {
        Ok(q) => q,
        Err(e) => {
            io.say(&[warn(format!(
                "  Could not create {}: {e}",
                tilde(session.quarantine, session.home)
            ))]);
            line(io, "  Nothing was changed.");
            let dry = Quarantine::<DryRun>::new(session.quarantine);
            return scan::run(scope, &mut Sink::Dry(&dry));
        }
    };
    let done = scan::run(scope, &mut Sink::Apply(&mut quarantine));
    if let Err(e) = quarantine.write_manifest() {
        io.say(&[warn(format!(
            "  Could not write the quarantine manifest: {e}"
        ))]);
    }
    report.push_str(&format!(
        "\ncontaining what was found\n{}",
        scan::render(&done, Target::Report)
    ));

    io.say(&[good("  Done.")]);
    blank(io, 1);
    for (from, reason) in quarantine.receipts() {
        io.say(&[
            p(if reason == "stripped-config" {
                "      stripped   "
            } else {
                "      moved      "
            }),
            p(tilde(from, session.home)),
        ]);
    }
    if quarantine.taken() < fixable.total() {
        blank(io, 1);
        io.say(&[warn(format!(
            "  {} could not be moved or stripped. The report says why.",
            fixable.total() - quarantine.taken()
        ))]);
    }

    let dry = Quarantine::<DryRun>::new(session.quarantine);
    let after = scan::run(scope, &mut Sink::Dry(&dry));
    report.push_str(&format!(
        "\nsecond check, after containing\n{}{}",
        scan::render(&after, Target::Report),
        scan::result(&after)
    ));
    blank(io, 2);
    if after.hits() == 0 {
        io.say(&[
            p("  Checked again. "),
            good("No confirmed finding is left in what was checked."),
        ]);
    } else {
        io.say(&[
            p("  Checked again. "),
            bad(format!(
                "{} left.",
                if after.hits() == 1 {
                    "1 confirmed finding is".to_string()
                } else {
                    format!("{} confirmed findings are", after.hits())
                }
            )),
        ]);
        line(
            io,
            if after.hits() == 1 {
                "  It needs you, and the next step says how."
            } else {
                "  They need you, and the next step says how."
            },
        );
    }
    after
}

// --- step 4: what to do now ---------------------------------------------------

/// One thing to do: a title, the files it is about, and a line or two of how.
pub(crate) struct Todo {
    pub(crate) title: String,
    pub(crate) paths: Vec<String>,
    pub(crate) how: Vec<String>,
}

pub(crate) fn todo(title: impl Into<String>, how: &[&str]) -> Todo {
    Todo {
        title: title.into(),
        paths: Vec::new(),
        how: how.iter().map(|h| (*h).to_string()).collect(),
    }
}

/// The paths of the findings of some kinds, as they would be said.
fn paths_of(findings: &[Finding], home: &Path, want: fn(Kind) -> bool) -> Vec<String> {
    findings
        .iter()
        .filter(|f| f.kind.is_some_and(want))
        .filter_map(|f| f.path.as_deref())
        .map(|path| tilde(path, home))
        .collect()
}

/// The last screen, as lines. Written from what was found at the start and
/// what is still there now.
fn what_now(
    session: &Session,
    found: &Verdict,
    current: &[Finding],
    declined: usize,
    what: What,
) -> Vec<Vec<Span>> {
    let mut out: Vec<Vec<Span>> = Vec::new();

    if found.hits() == 0 {
        if found.reviews() == 0 {
            out.push(vec![good("  Nothing found.")]);
            out.push(vec![]);
            for text in [
                "  Clean against today's indicators is not proof that nothing",
                "  was ever here. If you had a reason to check, change your",
                "  passwords anyway.",
            ] {
                out.push(vec![dim(text)]);
            }
            return out;
        }
        out.push(vec![
            warn("  Nothing is confirmed."),
            p(format!(
                " {} your eyes.",
                if found.reviews() == 1 {
                    "1 thing needs".to_string()
                } else {
                    format!("{} things need", found.reviews())
                }
            )),
        ]);
        let todos = [
            todo(
                "Look at each item listed above",
                &["Ask whether you put it there."],
            ),
            todo(
                "If one of them is not yours, treat it as confirmed",
                &[
                    "Disconnect from the network, and change your passwords",
                    "from a different computer.",
                ],
            ),
        ];
        numbered(&mut out, &todos);
        return out;
    }

    let ran_here = found.kinds().any(Kind::ran_here);
    let here = if what == What::Computer {
        "This computer is"
    } else {
        "These folders are"
    };
    out.push(if ran_here {
        vec![
            bad(format!("  {here} not safe yet.")),
            p(" Do these in order."),
        ]
    } else {
        vec![
            warn("  The payload was in your project files."),
            p(" Do these in order."),
        ]
    });

    let kinds_now: Vec<Kind> = current.iter().filter_map(|f| f.kind).collect();
    let mut todos: Vec<Todo> = Vec::new();

    if kinds_now.iter().any(|k| k.running()) {
        todos.push(todo(
            "Disconnect from the network, now",
            &["Something from the payload is running or connected."],
        ));
    }
    if declined > 0 {
        todos.push(todo(
            format!(
                "Deal with the {} you left in place",
                count(declined, "finding", "findings")
            ),
            &["Run polinrider again and type yes."],
        ));
    }
    let unstrippable = |k: Kind| matches!(k, Kind::Config { strippable: false });
    if kinds_now.iter().any(|k| unstrippable(*k)) {
        let mut t = todo(
            "Replace the config files that could not be cleaned safely",
            &["Delete that clone, and clone it again once GitHub is clean."],
        );
        t.paths = paths_of(current, session.home, unstrippable);
        todos.push(t);
    }
    if kinds_now.contains(&Kind::Package) {
        let mut t = todo(
            "Remove the campaign package",
            &["Delete the dependency, delete node_modules, reinstall."],
        );
        t.paths = paths_of(current, session.home, |k| k == Kind::Package);
        todos.push(t);
    }
    if kinds_now.contains(&Kind::StartupFile) {
        let mut t = todo(
            "Edit your shell startup file",
            &["Remove the line that runs the payload."],
        );
        t.paths = paths_of(current, session.home, |k| k == Kind::StartupFile);
        todos.push(t);
    }
    if kinds_now.contains(&Kind::Crontab) {
        todos.push(todo(
            "Edit your scheduled jobs",
            &["Run crontab -e and remove the line that calls the campaign."],
        ));
    }
    if kinds_now.contains(&Kind::Registry) {
        let mut t = todo(
            "Fix your npm registry setting",
            &["Point it back at a registry you trust."],
        );
        t.paths = paths_of(current, session.home, |k| k == Kind::Registry);
        todos.push(t);
    }
    todos.push(todo(
        "Change your passwords and keys, from a DIFFERENT computer",
        &[
            "GitHub tokens and SSH keys, npm tokens, cloud keys,",
            "anything in a .env file.",
        ],
    ));
    todos.push(if ran_here && what == What::Computer {
        todo(
            "Rebuild this computer from a clean install",
            &["The payload ran here. Quarantine does not make it safe."],
        )
    } else if ran_here {
        todo(
            "Rebuild the computer these files came from",
            &["The payload ran there. Quarantine does not make it safe."],
        )
    } else {
        todo(
            "Decide whether to rebuild this computer",
            &[
                "The payload runs when an infected project is built or",
                "opened in an editor. If that happened, or you are not",
                "sure, rebuild.",
            ],
        )
    });
    todos.push(todo(
        "Clean your repositories on GitHub",
        &["This beta cannot do that yet. The released tool can."],
    ));
    numbered(&mut out, &todos);
    out
}

pub(crate) fn numbered(out: &mut Vec<Vec<Span>>, todos: &[Todo]) {
    for (n, todo) in todos.iter().enumerate() {
        out.push(vec![]);
        out.push(vec![]);
        out.push(vec![
            strong(format!("  {}  ", n + 1)),
            strong(todo.title.clone()),
        ]);
        for path in &todo.paths {
            out.push(vec![p(format!("     {path}"))]);
        }
        for how in &todo.how {
            out.push(vec![dim(format!("     {how}"))]);
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::host::{Platform, Probe, Process, Snapshot};
    use std::collections::VecDeque;
    use std::fs;

    const STRONG: &str = "MARKER-ALPHA";
    const IMPLANT: &str = "implant-process-name-x64";

    /// A console that answers from a list and remembers what was said.
    struct Script {
        answers: VecDeque<String>,
        said: String,
    }

    impl Script {
        fn new(answers: &[&str]) -> Self {
            Self {
                answers: answers.iter().map(|a| (*a).to_string()).collect(),
                said: String::new(),
            }
        }
    }

    impl Console for Script {
        fn say(&mut self, line: &[Span]) {
            self.said.push_str(&text_of(line));
            self.said.push('\n');
        }
        fn ask(&mut self) -> Option<String> {
            self.said.push_str("  > \n");
            self.answers.pop_front()
        }
        fn progress(&mut self, _lines: &[Vec<Span>], _last: bool) {}
    }

    fn ind() -> Indicators {
        Indicators {
            strong: vec![STRONG.into()],
            bad_packages: vec!["evil-campaign-pkg".into()],
            implant_names: vec![IMPLANT.into()],
            ..Indicators::default()
        }
    }

    struct World {
        dir: PathBuf,
        quarantine: PathBuf,
    }

    impl World {
        fn new(name: &str, files: &[(&str, &str)]) -> Self {
            let dir = std::env::temp_dir().join(format!("prc-guide-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            for sub in ["home", "root", "ioc", "repo"] {
                fs::create_dir_all(dir.join(sub)).expect("mkdir");
            }
            for (file, body) in files {
                let path = dir.join(file);
                fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
                fs::write(path, body).expect("write");
            }
            Self {
                quarantine: dir.join("q"),
                dir,
            }
        }

        fn repo(&self) -> String {
            self.dir.join("repo").display().to_string()
        }

        fn run_with(&self, host: Option<&dyn Host>, answers: &[&str]) -> (Outcome, Script) {
            let ind = ind();
            let session = Session {
                ind: &ind,
                ioc_dir: &self.dir.join("ioc"),
                home: &self.dir.join("home"),
                host,
                quarantine: &self.quarantine,
                report: &self.dir.join("home/polinrider-report.txt"),
                system: "Linux",
                unicode: false,
                forge: &crate::remote::Supplied::new(self.dir.join("forge")),
                evidence: &self.dir.join("evidence"),
            };
            let mut io = Script::new(answers);
            let outcome = run(&session, &mut io);
            (outcome, io)
        }

        fn run(&self, answers: &[&str]) -> (Outcome, Script) {
            self.run_with(None, answers)
        }

        fn config(&self) -> String {
            fs::read_to_string(self.dir.join("repo/postcss.config.mjs")).expect("read")
        }
    }

    impl Drop for World {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn infected() -> String {
        format!(
            "export default {{}}\n{}var x='{STRONG}';\n",
            " ".repeat(280)
        )
    }

    #[test]
    fn quitting_before_a_check_is_not_a_clean_result() {
        let w = World::new("quit", &[]);
        let (outcome, io) = w.run(&["q"]);
        assert_eq!(outcome.exit, ExitCode::CouldNotRun);
        assert!(io.said.contains("Nothing was checked"));
    }

    #[test]
    fn a_clean_folder_ends_clean_without_asking_to_change_anything() {
        let w = World::new(
            "clean",
            &[("repo/postcss.config.mjs", "export default {}\n")],
        );
        let (outcome, io) = w.run(&["folder", &w.repo(), ""]);
        assert_eq!(outcome.exit, ExitCode::Clean);
        assert!(io.said.contains("NOTHING FOUND"), "{}", io.said);
        assert!(!io.said.contains("Type yes"), "{}", io.said);
        assert!(io.said.contains("STEP 4 OF 4"));
        assert!(io.said.contains("To check again:   polinrider"));
    }

    #[test]
    fn the_summary_is_plain_words_and_names_no_path() {
        // The person reading this is stressed. Counts and what they mean
        // first; the paths are one word away, and in the report.
        let w = World::new(
            "summary",
            &[
                ("repo/postcss.config.mjs", &infected()),
                ("repo/public/fake.woff2", "var a = 1\n"),
                (
                    "repo/package.json",
                    "{\"dependencies\":{\"evil-campaign-pkg\":\"1\"}}\n",
                ),
            ],
        );
        let (_, io) = w.run(&["folder", &w.repo(), ""]);
        let step3 = io.said.split("STEP 3 OF 4").nth(1).expect("step 3");
        assert!(step3.contains("Checked 3 files in 1 folder."), "{step3}");
        assert!(step3.contains("CONFIRMED   3"), "{step3}");
        assert!(step3.contains("1 config file with the payload hidden in it"));
        assert!(step3.contains("1 font file that is really a script"));
        assert!(step3.contains("1 project that depends on a campaign package"));
        assert!(step3.contains("I can deal with 2 of the 3 now:"), "{step3}");
        assert!(!step3.contains("postcss.config.mjs"), "{step3}");
        assert!(!step3.contains("[HIT]"), "{step3}");
    }

    #[test]
    fn details_shows_every_finding_and_then_asks_again() {
        let w = World::new("details", &[("repo/postcss.config.mjs", &infected())]);
        let (_, io) = w.run(&["folder", &w.repo(), "", "details", "no", ""]);
        assert!(io.said.contains("  CONFIRMED\n"), "{}", io.said);
        assert!(io
            .said
            .contains("      config file contains an indicator\n"));
        assert!(io.said.contains("postcss.config.mjs"));
        assert!(io
            .said
            .contains("Type yes to deal with 1 of the 1, or no to leave them."));
        assert_eq!(w.config(), infected(), "details changes nothing");
    }

    #[test]
    fn a_payload_is_stripped_only_after_an_explicit_yes() {
        let w = World::new("yes", &[("repo/postcss.config.mjs", &infected())]);
        let (outcome, io) = w.run(&["folder", &w.repo(), "", "yes", ""]);
        assert_eq!(w.config(), "export default {}\n");
        assert!(w.dir.join("q/manifest.tsv").is_file());
        assert!(io.said.contains("  Done."));
        assert!(io.said.contains("      stripped   "));
        assert!(io
            .said
            .contains("Checked again. No confirmed finding is left"));
        // What was found is still what the session reports.
        assert_eq!(outcome.exit, ExitCode::Confirmed);
        assert!(outcome.report.contains("second check"));
        assert!(outcome.report.contains("Change your passwords and keys"));
    }

    #[test]
    fn a_quarantine_inside_the_checked_folder_is_not_found_again() {
        let mut w = World::new(
            "inside",
            &[
                ("repo/postcss.config.mjs", &infected()),
                ("repo/public/fake.woff2", "var a = 1\n"),
            ],
        );
        w.quarantine = w.dir.join("repo/set-aside");
        let (_, io) = w.run(&["folder", &w.repo(), "", "yes", ""]);
        assert!(w.quarantine.join("manifest.tsv").is_file());
        assert!(io.said.contains("      moved      "));
        assert!(
            io.said.contains("No confirmed finding is left"),
            "{}",
            io.said
        );
    }

    #[test]
    fn no_leaves_everything_exactly_as_it_was_and_says_what_is_left() {
        let w = World::new("no", &[("repo/postcss.config.mjs", &infected())]);
        let (outcome, io) = w.run(&["folder", &w.repo(), "", "no", ""]);
        assert_eq!(w.config(), infected());
        assert!(!w.dir.join("q").exists(), "no quarantine is even created");
        assert!(io.said.contains("Left as it is"));
        assert!(!io.said.contains("  Done."));
        assert!(io
            .said
            .contains("Deal with the 1 finding you left in place"));
        assert_eq!(outcome.exit, ExitCode::Confirmed);
    }

    #[test]
    fn enter_or_anything_that_is_not_yes_never_changes_a_file() {
        // Enter, a typo, "sure", "ok": each asks again. Only yes writes.
        let w = World::new("blank", &[("repo/postcss.config.mjs", &infected())]);
        let (_, io) = w.run(&["folder", &w.repo(), "", "", "sure", "ok", "y e s"]);
        assert_eq!(
            w.config(),
            infected(),
            "input ran out while it was still asking: nothing is changed"
        );
        assert!(io.said.contains("Type yes, no or details"));
        assert!(io.said.contains("Input ended"));
    }

    #[test]
    fn q_at_the_question_stops_without_changing_anything() {
        let w = World::new("q-apply", &[("repo/postcss.config.mjs", &infected())]);
        let (outcome, io) = w.run(&["folder", &w.repo(), "", "q"]);
        assert_eq!(w.config(), infected());
        assert!(io.said.contains("Stopped. Nothing further was changed."));
        assert_eq!(outcome.exit, ExitCode::Confirmed);
    }

    #[test]
    fn answers_are_words_and_a_wrong_one_asks_again() {
        let w = World::new("retry", &[("repo/index.js", "export const a = 1\n")]);
        let (outcome, io) = w.run(&["1", "", "FOLDER", "/definitely/not/here", "", &w.repo(), ""]);
        assert_eq!(outcome.exit, ExitCode::Clean);
        assert!(io
            .said
            .contains("Type computer, folder, organization or account."));
        assert!(io.said.contains("That folder does not exist"));
        assert!(io.said.contains("I need a folder to check"));
        assert!(!io.said.contains("Type 1 or 2"));
    }

    #[test]
    fn back_returns_to_the_first_question() {
        let w = World::new("back", &[("repo/index.js", "export const a = 1\n")]);
        let (outcome, io) = w.run(&["folder", "back", "folder", &w.repo(), ""]);
        assert_eq!(outcome.exit, ExitCode::Clean);
        assert_eq!(io.said.matches("STEP 1 OF 4").count(), 2);
    }

    #[test]
    fn a_running_implant_is_named_and_no_question_is_put_that_has_no_yes() {
        let w = World::new(
            "computer",
            &[("home/code/index.js", "export const a = 1\n")],
        );
        let mut host = Snapshot::quiet(Platform::Linux, w.dir.join("root"));
        host.processes = Probe::Read(vec![Process {
            pid: 4242,
            name: "implant-process".into(),
            command: "x".into(),
        }]);
        // Enter accepts the whole home folder.
        let (outcome, io) = w.run_with(Some(&host), &["computer", "", ""]);
        assert_eq!(outcome.exit, ExitCode::Confirmed);
        assert!(
            io.said.contains("Your home folder, and everything in it:"),
            "{}",
            io.said
        );
        assert!(io
            .said
            .contains("Checked 1 file in 1 folder, and this computer."));
        assert!(io.said.contains(", and this computer."));
        assert!(io
            .said
            .contains("1 program from the payload running right now"));
        assert!(io
            .said
            .contains("A running program means the payload has run on this computer."));
        assert!(io.said.contains("None of these can be fixed for you."));
        assert!(!io.said.contains("Type yes"));
        assert!(io.said.contains("  1  Disconnect from the network, now"));
        assert!(io
            .said
            .contains("Rebuild this computer from a clean install"));
    }

    #[test]
    fn a_payload_only_in_project_files_does_not_say_the_computer_is_lost() {
        let w = World::new("project-only", &[("repo/postcss.config.mjs", &infected())]);
        let (_, io) = w.run(&["folder", &w.repo(), "", "yes", ""]);
        assert!(io.said.contains("Nothing here shows that it has run."));
        assert!(io.said.contains("The payload was in your project files."));
        assert!(io.said.contains("Decide whether to rebuild this computer"));
        assert!(!io.said.contains("not safe yet"));
    }

    #[test]
    fn review_only_lists_what_to_look_at() {
        let w = World::new(
            "review",
            &[(
                "repo/.vscode/tasks.json",
                "{\"tasks\":[{\"runOptions\":{\"runOn\":\"folderOpen\"}}]}\n",
            )],
        );
        let (outcome, io) = w.run(&["folder", &w.repo(), ""]);
        assert_eq!(outcome.exit, ExitCode::Review);
        assert!(io.said.contains("CONFIRMED   0"));
        assert!(io.said.contains("TO REVIEW   1"));
        assert!(io.said.contains("  TO REVIEW\n"));
        assert!(io
            .said
            .contains("Nothing is confirmed. 1 thing needs your eyes."));
        assert!(!io.said.contains("Rebuild"));
    }

    #[test]
    fn computer_checks_the_whole_home_folder_and_folder_suggests_where_code_lives() {
        // The two choices used to be the same check: computer looked in the
        // code folders and so did folder. A payload in ~/Downloads was in
        // neither.
        let w = World::new(
            "home",
            &[
                ("home/code/index.js", "export const a = 1\n"),
                ("home/Downloads/unzipped/postcss.config.mjs", &infected()),
            ],
        );
        let host = Snapshot::quiet(Platform::Linux, w.dir.join("root"));
        let (outcome, io) = w.run_with(Some(&host), &["computer", ""]);
        assert_eq!(outcome.exit, ExitCode::Confirmed, "{}", io.said);
        assert!(io
            .said
            .contains("1 config file with the payload hidden in it"));

        // folder offers the code folder it found, and Enter takes it. The
        // payload in Downloads is outside what was asked for.
        let (outcome, io) = w.run(&["folder", ""]);
        assert!(io
            .said
            .contains("I found these code folders in your home folder:"));
        assert!(io.said.contains("      ~/code\n"), "{}", io.said);
        assert_eq!(outcome.exit, ExitCode::Clean, "{}", io.said);
    }

    #[test]
    fn this_computer_is_not_offered_when_the_host_cannot_be_read() {
        let w = World::new("nohost", &[("repo/index.js", "export const a = 1\n")]);
        let (outcome, io) = w.run(&["computer", "folder", &w.repo(), ""]);
        assert_eq!(outcome.exit, ExitCode::Clean);
        assert!(io
            .said
            .contains("Not available in this build on this system."));
        assert!(io
            .said
            .contains("That is not available here. Type one of the others."));
    }

    #[test]
    fn numbers_and_paths_read_the_way_people_say_them() {
        assert_eq!(thousands(7), "7");
        assert_eq!(thousands(48210), "48,210");
        assert_eq!(thousands(1_000_000), "1,000,000");
        let home = Path::new("/home/x");
        assert_eq!(tilde(Path::new("/home/x/Sites/shop"), home), "~/Sites/shop");
        assert_eq!(tilde(Path::new("/home/x"), home), "~");
        assert_eq!(tilde(Path::new("/srv/code"), home), "/srv/code");
        // A path is cleaned on its way to the screen.
        assert_eq!(tilde(Path::new("/srv/a\x1b[2Jb"), home), "/srv/a[2Jb");
    }
}
