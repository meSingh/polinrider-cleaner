//! The guided flow: one session from "something is wrong" to "here is what
//! was found, what was done about it and what is left for you".
//!
//! Running `polinrider` with no arguments lands here. It asks what to check,
//! scans, shows exactly what it would move or strip, does it only on an
//! explicit yes, checks again, and says what it cannot do for you. The same
//! session, no second command.
//!
//! Three rules govern every prompt, each of which exists because the shell
//! version got it wrong once (ADR-0021, ADR-0022, ADR-0024):
//!
//! - **Only `yes` changes anything.** Not Enter, not a default, not anything
//!   that merely is not `no`.
//! - **Only `q` leaves.** A blank line asks again. It never quits and never
//!   picks an option that writes.
//! - **Input that ends, stops.** If stdin closes, the session ends where it
//!   is with nothing further changed, and says so.
//!
//! All reading and printing goes through [`Console`], so a test drives a
//! whole session from a list of answers. See ADR-0032.

use crate::checks::{OnInfectedConfig, Sink};
use crate::host::Host;
use crate::indicators::Indicators;
use crate::quarantine::{Apply, DryRun, Quarantine};
use crate::scan::{self, Scope, Target};
use crate::verdict::{clean, Entry, ExitCode, Level, Verdict};
use std::path::{Path, PathBuf};

/// Where the session talks and listens.
pub trait Console {
    fn say(&mut self, text: &str);
    /// Show a prompt and read one line, trimmed. `None` when input has ended.
    fn ask(&mut self, prompt: &str) -> Option<String>;
}

/// Everything a session needs that was decided before it started.
pub struct Session<'a> {
    pub ind: &'a Indicators,
    pub ioc_dir: &'a Path,
    pub home: &'a Path,
    /// This machine's live state, when it can be read. Without it the session
    /// can still check directories.
    pub host: Option<&'a dyn Host>,
    pub quarantine: &'a Path,
}

/// What a finished session leaves behind.
pub struct Outcome {
    pub exit: ExitCode,
    /// Every scan of the session in full, for the report file.
    pub report: String,
}

/// Why a step did not produce a value.
enum Stop {
    /// The operator typed `q`.
    Quit,
    /// Input ended.
    Ended,
    /// The operator typed `b`.
    Back,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum What {
    Computer,
    Directories,
}

fn read(io: &mut dyn Console, prompt: &str) -> Result<String, Stop> {
    match io.ask(prompt) {
        None => Err(Stop::Ended),
        Some(line) if line.eq_ignore_ascii_case("q") => Err(Stop::Quit),
        Some(line) => Ok(line),
    }
}

/// Run a session. Never panics on bad input and never writes without a yes.
pub fn run(session: &Session, io: &mut dyn Console) -> Outcome {
    let mut report = String::new();
    io.say("This is a beta build of 2.0.");
    io.say("Type q at any prompt to stop. Nothing is changed unless you type yes.");

    let mut worst: Option<ExitCode> = None;
    let stopped = steps(session, io, &mut report, &mut worst);

    match stopped {
        Err(Stop::Quit) => io.say("\nStopped. Nothing further was changed."),
        Err(Stop::Ended) => io.say("\nInput ended. Stopped here, and nothing further was changed."),
        Err(Stop::Back) | Ok(()) => {}
    }

    let exit = match worst {
        Some(code) => code,
        None => {
            // An incomplete run is not a clean one.
            io.say("Nothing was scanned, so this says nothing about whether you are infected.");
            ExitCode::CouldNotRun
        }
    };
    if exit == ExitCode::Confirmed {
        io.say("\nExit code 2: a confirmed indicator was found during this session, whatever was done about it afterwards.");
    }
    Outcome { exit, report }
}

fn steps(
    session: &Session,
    io: &mut dyn Console,
    report: &mut String,
    worst: &mut Option<ExitCode>,
) -> Result<(), Stop> {
    // --- 1 and 2: what, and where -------------------------------------------
    let (what, roots) = loop {
        let what = choose_what(session, io)?;
        match choose_roots(session, what, io) {
            Ok(roots) => break (what, roots),
            Err(Stop::Back) => continue,
            Err(stop) => return Err(stop),
        }
    };

    let scope = |on_infected_config| Scope {
        roots: &roots,
        ioc_dir: session.ioc_dir,
        ind: session.ind,
        home: (what == What::Computer).then_some(session.home),
        host: if what == What::Computer {
            session.host
        } else {
            None
        },
        on_infected_config,
    };

    // --- 3: scan. A dry run, and it shows what an apply would do. -----------
    io.say("\nStep 3 of 6. Scanning. This reads only.");
    let dry = Quarantine::<DryRun>::new(session.quarantine);
    let found = scan::run(&scope(OnInfectedConfig::Strip), &mut Sink::Dry(&dry));
    io.say(&format!(
        "{}{}",
        scan::render(&found, Target::Console),
        scan::result(&found)
    ));
    report.push_str(&format!(
        "first scan, dry run\n{}{}",
        scan::render(&found, Target::Report),
        scan::result(&found)
    ));
    *worst = Some(found.exit_code());

    if found.hits() == 0 {
        if found.reviews() > 0 {
            io.say("\nNothing confirmed. The [review] lines above need your eyes: they are things this tool cannot judge for you.");
        }
        prevent(io);
        return Ok(());
    }

    // --- 4: contain ---------------------------------------------------------
    io.say("\nStep 4 of 6. Containing what was found.");
    let (movable, strippable) = actionable(&found);
    let mut applied = false;
    if movable + strippable == 0 {
        io.say("Nothing found here can be moved or stripped for you. Each [HIT] above says what to do by hand.");
    } else {
        io.say(&format!(
            "{movable} artifact(s) can be moved into quarantine and {strippable} file(s) can have an appended payload stripped in place."
        ));
        io.say(&format!(
            "Nothing is deleted. Every original is kept under {}",
            session.quarantine.display()
        ));
        if confirm(io, "Type yes to do it, or no to leave everything as it is:")? {
            applied = contain(session, &scope(OnInfectedConfig::Strip), io, report);
        } else {
            io.say("Left as it is. Nothing was moved or stripped.");
        }
    }

    // --- 5: credentials, and what this build cannot reach -------------------
    io.say("\nStep 5 of 6. Credentials and remotes. This part is yours.");
    io.say("  The payload is a remote access trojan and an infostealer. Assume every");
    io.say("  credential this user account could reach has been taken.");
    io.say("  1. Rotate them from a DIFFERENT machine: GitHub tokens and SSH keys, npm");
    io.say("     tokens, cloud keys, and anything in a .env file under the scanned paths.");
    io.say("  2. Clean the remote only after that. This beta does not scan or clean");
    io.say("     GitHub yet. The released tool on the main branch does: ./polinrider.sh");
    io.say("  3. An infected commit may still be in each repository's history and on");
    io.say("     its remote. Stripping a file does not change that.");
    read(
        io,
        "Press Enter to go on to the final check, or q to stop here:",
    )?;

    // --- verify, only if something was changed ------------------------------
    if applied {
        io.say("\nChecking again, now that the artifacts are out of the way.");
        let after = scan::run(&scope(OnInfectedConfig::Strip), &mut Sink::Dry(&dry));
        let remaining = scan::render(&only_findings(&after), Target::Console);
        if !remaining.trim().is_empty() {
            io.say(&remaining);
        }
        io.say(&scan::result(&after));
        report.push_str(&format!(
            "\nsecond scan, after containing\n{}{}",
            scan::render(&after, Target::Report),
            scan::result(&after)
        ));
        if after.hits() == 0 {
            io.say("The files are clean against the current indicator set. That is the files, not the machine: see step 5.");
        } else {
            io.say("Some findings remain. They are the ones that cannot be moved for you.");
        }
    }

    prevent(io);
    Ok(())
}

fn choose_what(session: &Session, io: &mut dyn Console) -> Result<What, Stop> {
    io.say("\nStep 1 of 6. What do you want to check?");
    if session.host.is_some() {
        io.say("  1  This computer                      files, persistence, running processes, live connections");
    } else {
        io.say(
            "  1  This computer                      not available in this build on this platform",
        );
    }
    io.say("  2  A folder, a repository or a drive  files only");
    loop {
        match read(io, "Type 1 or 2:")?.as_str() {
            "1" if session.host.is_some() => return Ok(What::Computer),
            "1" => io.say("That is not available here. Type 2 to check directories."),
            "2" => return Ok(What::Directories),
            _ => io.say("Type 1 or 2, or q to stop."),
        }
    }
}

/// The directories people usually keep code in, that exist under this home.
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
    io.say(
        "\nStep 2 of 6. Where to look. The scan is only as good as the directories it is given.",
    );
    let usual = if what == What::Computer {
        usual_roots(session.home)
    } else {
        Vec::new()
    };
    if usual.is_empty() {
        io.say("Type one directory per line, then an empty line to start. b goes back.");
    } else {
        io.say("Found these under your home directory:");
        for dir in &usual {
            io.say(&format!("    {}", dir.display()));
        }
        io.say("Press Enter to scan them, or type your own, one directory per line, then an empty line. b goes back.");
    }

    let mut roots: Vec<PathBuf> = Vec::new();
    loop {
        let line = read(io, "Directory:")?;
        if line.eq_ignore_ascii_case("b") {
            return Err(Stop::Back);
        }
        if line.is_empty() {
            if !roots.is_empty() {
                return Ok(roots);
            }
            if !usual.is_empty() {
                return Ok(usual);
            }
            // A blank line with nothing chosen asks again. It does not scan
            // nothing and call it clean.
            io.say("No directory yet. Type one, or q to stop.");
            continue;
        }
        let dir = expand(session.home, &line);
        if dir.is_dir() {
            io.say(&format!("    added {}", dir.display()));
            if !roots.contains(&dir) {
                roots.push(dir);
            }
        } else {
            io.say(&format!(
                "    no such directory: {}",
                clean(&dir.display().to_string())
            ));
        }
    }
}

/// How many findings an apply would move, and how many it would strip. Read
/// from the dry run's own lines, so the number shown is the number done.
fn actionable(v: &Verdict) -> (usize, usize) {
    let lines: Vec<&str> = v
        .findings()
        .filter(|f| f.level == Level::Hit)
        .filter_map(|f| f.remedy.as_deref())
        .flat_map(str::lines)
        .collect();
    (
        lines
            .iter()
            .filter(|l| l.starts_with("would quarantine:"))
            .count(),
        lines
            .iter()
            .filter(|l| l.starts_with("would strip "))
            .count(),
    )
}

/// Yes or no. Anything else, a blank line included, asks again.
fn confirm(io: &mut dyn Console, prompt: &str) -> Result<bool, Stop> {
    loop {
        let answer = read(io, prompt)?.to_ascii_lowercase();
        match answer.as_str() {
            "yes" | "y" => return Ok(true),
            "no" | "n" => return Ok(false),
            _ => io.say("Type yes or no. Nothing happens until you do."),
        }
    }
}

/// Move and strip. Returns whether anything was actually changed.
fn contain(session: &Session, scope: &Scope, io: &mut dyn Console, report: &mut String) -> bool {
    let mut quarantine = match Quarantine::<Apply>::create(session.quarantine) {
        Ok(q) => q,
        Err(e) => {
            io.say(&format!(
                "Could not create {}: {e}. Nothing was changed.",
                session.quarantine.display()
            ));
            return false;
        }
    };
    let done = scan::run(scope, &mut Sink::Apply(&mut quarantine));
    if let Err(e) = quarantine.write_manifest() {
        io.say(&format!("Could not write the quarantine manifest: {e}"));
    }
    let lines = scan::render(&only_findings(&done), Target::Console);
    io.say(&lines);
    io.say(&format!(
        "{} original(s) are in {}. RESTORE.txt there says how to put one back.",
        quarantine.taken(),
        session.quarantine.display()
    ));
    report.push_str(&format!(
        "\ncontaining, with --apply semantics\n{}",
        scan::render(&done, Target::Report)
    ));
    quarantine.taken() > 0
}

/// The hits and review items of a scan with their evidence, without the
/// sections that found nothing. For the second look, where the full report
/// would bury the two lines that matter.
fn only_findings(v: &Verdict) -> Verdict {
    let mut out = Verdict::new();
    // Evidence is printed before some findings and after others. Lines seen
    // before a finding wait here until it is known whether the finding stays.
    let mut waiting: Vec<&String> = Vec::new();
    let mut after_kept = false;
    for entry in v.entries() {
        match entry {
            Entry::Section(_) => {
                waiting.clear();
                after_kept = false;
            }
            Entry::Detail(line) if after_kept => out.detail(line.clone()),
            Entry::Detail(line) => waiting.push(line),
            Entry::Note(_) => {}
            Entry::Finding(f) if f.level >= Level::Review => {
                for line in waiting.drain(..) {
                    out.detail(line.clone());
                }
                out.push(f.clone());
                after_kept = true;
            }
            Entry::Finding(_) => {
                waiting.clear();
                after_kept = false;
            }
        }
    }
    out
}

fn prevent(io: &mut dyn Console) {
    io.say("\nStep 6 of 6. So that it does not come back.");
    io.say("  - npm config set ignore-scripts true   stops install scripts running by default");
    io.say(
        "  - Scan every push in CI, so a reinfection is caught by a check and not by a stranger.",
    );
    io.say("  - Run this again after the weekly indicator update. Clean today is clean against today's list.");
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
        asked: usize,
    }

    impl Script {
        fn new(answers: &[&str]) -> Self {
            Self {
                answers: answers.iter().map(|a| (*a).to_string()).collect(),
                said: String::new(),
                asked: 0,
            }
        }
    }

    impl Console for Script {
        fn say(&mut self, text: &str) {
            self.said.push_str(text);
            self.said.push('\n');
        }
        fn ask(&mut self, prompt: &str) -> Option<String> {
            self.asked += 1;
            self.said.push_str(prompt);
            self.said.push('\n');
            self.answers.pop_front()
        }
    }

    fn ind() -> Indicators {
        Indicators {
            strong: vec![STRONG.into()],
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
            };
            let mut io = Script::new(answers);
            let outcome = run(&session, &mut io);
            (outcome, io)
        }

        fn run(&self, answers: &[&str]) -> (Outcome, Script) {
            self.run_with(None, answers)
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
    fn quitting_before_a_scan_is_not_a_clean_result() {
        let w = World::new("quit", &[]);
        let (outcome, io) = w.run(&["q"]);
        assert_eq!(outcome.exit, ExitCode::CouldNotRun);
        assert!(io.said.contains("Nothing was scanned"));
    }

    #[test]
    fn a_clean_folder_ends_clean_without_asking_to_change_anything() {
        let w = World::new(
            "clean",
            &[("repo/postcss.config.mjs", "export default {}\n")],
        );
        let (outcome, io) = w.run(&["2", &w.repo(), ""]);
        assert_eq!(outcome.exit, ExitCode::Clean);
        assert!(!io.said.contains("Type yes"), "{}", io.said);
        assert!(io.said.contains("Step 6 of 6"));
    }

    #[test]
    fn a_payload_is_stripped_only_after_an_explicit_yes() {
        let w = World::new("yes", &[("repo/postcss.config.mjs", &infected())]);
        let (outcome, io) = w.run(&["2", &w.repo(), "", "yes", ""]);
        assert_eq!(
            fs::read_to_string(w.dir.join("repo/postcss.config.mjs")).expect("read"),
            "export default {}\n"
        );
        assert!(w.dir.join("q/manifest.tsv").is_file());
        assert!(io.said.contains("Checking again"));
        assert!(io.said.contains("clean against the current indicator set"));
        // What was found is still what the session reports.
        assert_eq!(outcome.exit, ExitCode::Confirmed);
        assert!(outcome.report.contains("second scan"));
    }

    #[test]
    fn a_quarantine_inside_the_scanned_folder_is_not_found_again() {
        // The session's second scan must not walk into the evidence the
        // first one just set aside and call the folder still infected.
        let mut w = World::new(
            "inside",
            &[
                ("repo/postcss.config.mjs", &infected()),
                ("repo/public/fake.woff2", "var a = 1\n"),
            ],
        );
        w.quarantine = w.dir.join("repo/set-aside");
        let (_, io) = w.run(&["2", &w.repo(), "", "yes", ""]);
        assert!(w.quarantine.join("manifest.tsv").is_file());
        assert!(io.said.contains("Checking again"));
        assert!(
            io.said
                .contains("The files are clean against the current indicator set"),
            "{}",
            io.said
        );
        assert!(!io.said.contains("Some findings remain"));
    }

    #[test]
    fn no_leaves_everything_exactly_as_it_was() {
        let w = World::new("no", &[("repo/postcss.config.mjs", &infected())]);
        let (outcome, io) = w.run(&["2", &w.repo(), "", "no", ""]);
        assert_eq!(
            fs::read_to_string(w.dir.join("repo/postcss.config.mjs")).expect("read"),
            infected()
        );
        assert!(!w.dir.join("q").exists(), "no quarantine is even created");
        assert!(io.said.contains("Left as it is"));
        assert!(!io.said.contains("Checking again"));
        assert_eq!(outcome.exit, ExitCode::Confirmed);
    }

    #[test]
    fn a_blank_line_or_anything_that_is_not_yes_never_applies() {
        // Enter, a typo, "sure", "ok": each asks again. Only yes writes.
        let w = World::new("blank", &[("repo/postcss.config.mjs", &infected())]);
        let (_, io) = w.run(&["2", &w.repo(), "", "", "sure", "ok", "Y E S"]);
        assert_eq!(
            fs::read_to_string(w.dir.join("repo/postcss.config.mjs")).expect("read"),
            infected(),
            "input ran out while it was still asking: nothing is changed"
        );
        assert!(io.said.contains("Type yes or no"));
        assert!(io.said.contains("Input ended"));
    }

    #[test]
    fn q_at_the_apply_prompt_stops_without_changing_anything() {
        let w = World::new("q-apply", &[("repo/postcss.config.mjs", &infected())]);
        let (outcome, io) = w.run(&["2", &w.repo(), "", "q"]);
        assert_eq!(
            fs::read_to_string(w.dir.join("repo/postcss.config.mjs")).expect("read"),
            infected()
        );
        assert!(io.said.contains("Stopped. Nothing further was changed."));
        assert_eq!(outcome.exit, ExitCode::Confirmed);
    }

    #[test]
    fn a_wrong_menu_choice_or_a_missing_directory_asks_again() {
        let w = World::new("retry", &[("repo/index.js", "export const a = 1\n")]);
        let (outcome, io) = w.run(&["7", "", "2", "/definitely/not/here", "", &w.repo(), ""]);
        assert_eq!(outcome.exit, ExitCode::Clean);
        assert!(io.said.contains("Type 1 or 2, or q to stop."));
        assert!(io.said.contains("no such directory"));
        assert!(io.said.contains("No directory yet"));
    }

    #[test]
    fn b_goes_back_to_the_first_question() {
        let w = World::new("back", &[("repo/index.js", "export const a = 1\n")]);
        let (outcome, io) = w.run(&["2", "b", "2", &w.repo(), ""]);
        assert_eq!(outcome.exit, ExitCode::Clean);
        assert_eq!(io.said.matches("Step 1 of 6").count(), 2);
    }

    #[test]
    fn a_running_implant_is_reported_and_nothing_is_offered_that_cannot_be_done() {
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
        // Enter accepts the code directory found under the home directory.
        let (outcome, io) = w.run_with(Some(&host), &["1", "", ""]);
        assert_eq!(outcome.exit, ExitCode::Confirmed);
        assert!(io.said.contains("an implant process is running now"));
        assert!(io
            .said
            .contains("Nothing found here can be moved or stripped for you"));
        assert!(!io.said.contains("Type yes"));
        assert!(io.said.contains("Rotate them from a DIFFERENT machine"));
    }

    #[test]
    fn this_computer_is_not_offered_when_the_host_cannot_be_read() {
        let w = World::new("nohost", &[("repo/index.js", "export const a = 1\n")]);
        let (outcome, io) = w.run(&["1", "2", &w.repo(), ""]);
        assert_eq!(outcome.exit, ExitCode::Clean);
        assert!(io
            .said
            .contains("not available in this build on this platform"));
        assert!(io.said.contains("That is not available here"));
    }
}
