//! One scan: a walk of the roots, then every check that applies.
//!
//! Lives in the library, not the binary, so that the guided flow can run a
//! scan, read its verdict and run another without starting a second process,
//! and so that a test can do the same.

use crate::checks::{self, ImplantScope, OnInfectedConfig, Sink};
use crate::host::Host;
use crate::host_checks;
use crate::host_checks::sh_quote;
use crate::indicators::Indicators;
use crate::verdict::{clean, Entry, Finding, Kind, Level, Verdict};
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

/// How far a scan has come. The same shape for every scan, so that one
/// progress screen serves all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step<'a> {
    /// Stages finished, and stages in all.
    pub done: usize,
    pub total: usize,
    /// What is being worked on now. Empty when the scan is finished.
    pub now: &'a str,
    /// Files listed so far.
    pub files: usize,
    /// Confirmed findings so far.
    pub found: usize,
}

/// Counts the stages of a scan and tells whoever is watching.
struct Watch<'a> {
    done: usize,
    total: usize,
    files: usize,
    found: usize,
    tell: &'a mut dyn FnMut(Step),
}

impl Watch<'_> {
    fn say(&mut self, now: &str) {
        (self.tell)(Step {
            done: self.done,
            total: self.total,
            now,
            files: self.files,
            found: self.found,
        });
    }

    /// The next stage is starting.
    fn stage(&mut self, now: &str, v: &Verdict) {
        self.found = v.hits();
        self.say(now);
        self.done += 1;
    }

    /// Within the implant stage: one large file out of how many.
    fn hashing(&mut self, n: usize, of: usize) {
        // `stage` has already counted this stage as started.
        self.done -= 1;
        self.say(&format!(
            "large files against known hashes, {} of {of}",
            n + 1
        ));
        self.done += 1;
    }
}

/// Walk the roots once and run the checks. Whether anything is moved or
/// stripped is decided by the sink, which is a type: see `quarantine`.
pub fn run(scope: &Scope, sink: &mut Sink) -> Verdict {
    run_watched(scope, sink, &mut |_| {})
}

/// The same scan, telling `tell` how far it has come: while files are being
/// listed, before each check, and once more when it is finished.
pub fn run_watched(scope: &Scope, sink: &mut Sink, tell: &mut dyn FnMut(Step)) -> Verdict {
    let mut watch = Watch {
        done: 0,
        // The walk, then the checks.
        total: if scope.home.is_some() { 11 } else { 8 },
        files: 0,
        found: 0,
        tell,
    };
    let mut v = Verdict::new();
    v.section("Filesystem walk");
    watch.say("listing files");
    let w = walk::walk_watched(
        scope.roots,
        &walk::Options::skipping(sink.root()),
        &mut |files| {
            watch.files = files;
            watch.say("listing files");
        },
    );
    watch.files = w.files.len();
    watch.done = 1;
    v.set_files(w.files.len());
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
        Some(home) => machine(&w, scope, home, &mut v, sink, &mut watch),
        None => directories(&w, scope, &mut v, sink, &mut watch),
    }
    watch.found = v.hits();
    watch.done = watch.total;
    watch.say("");
    v
}

/// A machine, or a disk with a home directory on it.
fn machine(
    w: &walk::Walk,
    scope: &Scope,
    home: &Path,
    v: &mut Verdict,
    sink: &mut Sink,
    watch: &mut Watch,
) {
    let (ind, host) = (scope.ind, scope.host);
    // One line per host check that did not run, so a section that was skipped
    // can never be mistaken for one that found nothing.
    let skipped = |v: &mut Verdict, name: &str| {
        v.section(format!("{name}: skipped, --fs-only"));
    };

    watch.stage("implant files and processes", v);
    checks::implants(
        w,
        ind,
        &ImplantScope {
            home: Some(home),
            ioc_dir: scope.ioc_dir,
            host,
        },
        v,
        sink,
        &mut |n, of| watch.hashing(n, of),
    );

    watch.stage("editor extensions", v);
    if host.is_some() {
        checks::extensions(&extension_dirs(home), ind, v, sink);
    } else {
        skipped(v, "IDE extensions");
    }

    watch.stage("editor tasks", v);
    checks::tasks_json(w, ind, v, sink);
    watch.stage("build configs", v);
    checks::build_configs(w, ind, scope.on_infected_config, v, sink);
    watch.stage("fonts", v);
    checks::fonts(w, v, sink);

    watch.stage("the scripts the payload spreads with", v);
    if host.is_some() {
        checks::propagation(w, v, sink);
    } else {
        skipped(v, "Propagation artifact");
    }

    watch.stage("packages", v);
    checks::packages(w, ind, v);

    watch.stage("login items and startup files", v);
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

    watch.stage("git hooks", v);
    checks::git_hooks(w, ind, host, v, sink);

    watch.stage("npm settings, running programs and connections", v);
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
fn directories(w: &walk::Walk, scope: &Scope, v: &mut Verdict, sink: &mut Sink, watch: &mut Watch) {
    let ind = scope.ind;
    watch.stage("implant files", v);
    checks::implants(
        w,
        ind,
        &ImplantScope {
            home: None,
            ioc_dir: scope.ioc_dir,
            host: None,
        },
        v,
        sink,
        &mut |n, of| watch.hashing(n, of),
    );
    watch.stage("editor tasks", v);
    checks::tasks_json(w, ind, v, sink);
    watch.stage("build configs", v);
    checks::build_configs(w, ind, scope.on_infected_config, v, sink);
    watch.stage("fonts", v);
    checks::fonts(w, v, sink);
    watch.stage("the scripts the payload spreads with", v);
    checks::propagation(w, v, sink);
    watch.stage("packages", v);
    checks::packages(w, ind, v);
    watch.stage("git hooks", v);
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
        // "Rebuild" is said only when something proves the payload ran here.
        // A payload sitting in a cloned file is a finding, not that proof.
        result.push_str(&row(if v.kinds().any(Kind::ran_here) {
            "This machine cannot be trusted until it is rebuilt."
        } else {
            "The payload is in your project files. See below."
        }));
        result.push_str(&row(""));
        result.push_str(&format!("  {bar}\n"));
    } else if reviews > 0 {
        result.push_str("VERDICT: no confirmed indicator.\n");
    } else {
        result.push_str("VERDICT: clean against the current indicator set.\n");
    }
    result
}

/// The run that produced a verdict, as far as the next command depends on it.
pub struct Run<'a> {
    /// `check` or `clean`.
    pub command: &'a str,
    /// The flags that decide what is looked at, as they were given.
    pub scope_flags: &'a [(String, Option<String>)],
    pub roots: &'a [PathBuf],
    /// Whether this run was allowed to move and strip.
    pub applied: bool,
    pub quarantine: &'a Path,
}

/// One argument as it has to be typed. Left bare when it is plainly safe,
/// because a command full of quotes is harder to read and to trust.
fn arg(text: &str) -> String {
    let plain = !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c));
    if plain {
        text.to_string()
    } else {
        sh_quote(text)
    }
}

impl Run<'_> {
    /// The exact command for `verb`, over the same directories. Never a
    /// placeholder: a line that fails when pasted is worse than no line.
    fn line(&self, verb: &str, apply: bool) -> String {
        let mut parts = vec!["polinrider".to_string(), verb.to_string()];
        if apply {
            parts.push("--apply".into());
        }
        for (flag, value) in self.scope_flags {
            // clean reads the directories it is given and refuses the flags
            // that say otherwise. Only the indicator set carries over.
            if verb == "clean" && flag != "--ioc" {
                continue;
            }
            parts.push(flag.clone());
            if let Some(value) = value {
                parts.push(arg(value));
            }
        }
        parts.extend(self.roots.iter().map(|r| arg(&r.display().to_string())));
        format!("       {}\n", parts.join(" "))
    }
}

/// Wrap prose to a readable width. `first` opens the first line, a step
/// number for instance, and `rest` indents the lines under it.
fn prose(first: &str, rest: &str, text: &str) -> String {
    const WIDTH: usize = 78;
    let mut out = String::new();
    let mut line = first.to_string();
    let mut started = false;
    for word in text.split_whitespace() {
        if started && line.chars().count() + 1 + word.chars().count() > WIDTH {
            out.push_str(&line);
            out.push('\n');
            line = rest.to_string();
            started = false;
        }
        if started {
            line.push(' ');
        }
        line.push_str(word);
        started = true;
    }
    out.push_str(&line);
    out.push('\n');
    out
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// What to do about a verdict, worked out from what was found.
///
/// The verdict says what is true. This says what to do, in order, with the
/// exact command wherever there is one. Short on purpose: somebody reading
/// it has just been told their machine is compromised.
pub fn next_steps(v: &Verdict, run: &Run) -> String {
    let mut out = String::new();
    if v.hits() == 0 {
        if v.reviews() == 0 {
            out.push_str(&prose(
                "  ",
                "  ",
                "Clean against today's indicators is not proof that nothing was ever here. If you had a reason to check, rotate your credentials anyway: they may have been taken from another machine or a shared secret store.",
            ));
            return out;
        }
        out.push_str("\n== WHAT TO DO NEXT ==\n");
        out.push_str(&prose(
            "  ",
            "  ",
            &format!(
                "Nothing is confirmed. {} marked [review] above {} a person, because this tool cannot judge {} for you.",
                plural(v.reviews(), "line", "lines"),
                if v.reviews() == 1 { "needs" } else { "need" },
                if v.reviews() == 1 { "it" } else { "them" },
            ),
        ));
        for (n, step) in [
            "Read each [review] line. It names a file, a process or a setting. Ask whether you put it there.",
            "A [review] line that says a check could not run, or was skipped, means that part was not looked at.",
            "If one of them is not yours, treat it as confirmed: disconnect from the network, and rotate your credentials from a different machine.",
        ]
        .iter()
        .enumerate()
        {
            out.push_str(&prose(&format!("  {}. ", n + 1), "     ", step));
        }
        out.push_str(&prose(
            "  ",
            "  ",
            "The [info] lines are inventory and hardening advice, not findings.",
        ));
        return out;
    }

    let kinds: Vec<Kind> = v.kinds().collect();
    let count = |want: fn(&Kind) -> bool| kinds.iter().filter(|k| want(k)).count();
    let ran_here = kinds.iter().any(|k| k.ran_here());
    let running = count(|k| k.running());
    let movable = count(|k| k.movable());
    let strippable = count(|k| matches!(k, Kind::Config { strippable: true }));
    let not_strippable = count(|k| matches!(k, Kind::Config { strippable: false }));
    let packages = count(|k| matches!(k, Kind::Package));
    let by_hand = count(|k| matches!(k, Kind::StartupFile));
    let cleaning = run.command == "clean";

    out.push_str("\n== WHAT TO DO NEXT ==\n");
    out.push_str(&prose(
        "  ",
        "  ",
        if ran_here {
            "The payload ran on this machine: something was found outside your project files."
        } else {
            "The payload is in your project files. Nothing was found outside them."
        },
    ));
    out.push('\n');

    let mut n = 0usize;
    let mut step = |out: &mut String, text: &str, command: Option<String>| {
        n += 1;
        out.push_str(&prose(&format!("  {n}. "), "     ", text));
        if let Some(command) = command {
            out.push_str(&command);
        }
    };

    if running > 0 {
        step(
            &mut out,
            "Disconnect this machine from the network now. Something is running or connected: the [HIT] lines above say what to stop.",
            None,
        );
    }

    if run.applied {
        if movable > 0 || (cleaning && strippable > 0) {
            step(
                &mut out,
                &format!(
                    "Done in this run: the originals are in {}. RESTORE.txt there says how to put one back.",
                    run.quarantine.display()
                ),
                None,
            );
        }
    } else if cleaning && movable + strippable > 0 {
        step(
            &mut out,
            &format!(
                "Strip {} and move {} into quarantine. Nothing is deleted and every original is kept:",
                plural(strippable, "config file", "config files"),
                plural(movable, "confirmed artifact", "confirmed artifacts")
            ),
            Some(run.line("clean", true)),
        );
    } else if movable > 0 {
        step(
            &mut out,
            &format!(
                "Move {} into quarantine. Nothing is deleted:",
                plural(movable, "confirmed artifact", "confirmed artifacts")
            ),
            Some(run.line(run.command, true)),
        );
    }
    if !cleaning && strippable > 0 {
        step(
            &mut out,
            &format!(
                "Cut the payload out of {} in place. The originals are kept:",
                plural(strippable, "config file", "config files")
            ),
            Some(run.line("clean", true)),
        );
    }
    if not_strippable > 0 {
        step(
            &mut out,
            &format!(
                "{} could not be stripped safely. Delete those clones and clone again once the remote is clean.",
                plural(not_strippable, "config file", "config files")
            ),
            None,
        );
    }
    if packages > 0 {
        step(
            &mut out,
            &format!(
                "Remove the campaign package from {} named above, delete node_modules and reinstall.",
                if packages == 1 {
                    "the manifest"
                } else {
                    "the manifests"
                }
            ),
            None,
        );
    }
    if by_hand > 0 {
        step(
            &mut out,
            &format!(
                "{} to be edited by hand: a startup file, a crontab or an npm setting. The line under each [HIT] says what to remove.",
                if by_hand == 1 {
                    "1 finding has".to_string()
                } else {
                    format!("{by_hand} findings have")
                }
            ),
            None,
        );
    }
    step(
        &mut out,
        "Check again. It should come back with no [HIT]:",
        Some(run.line(run.command, false)),
    );
    step(
        &mut out,
        "Rotate every credential this account could reach, from a DIFFERENT machine: GitHub tokens and SSH keys, npm tokens, cloud keys and anything in a .env file.",
        None,
    );
    if ran_here {
        step(
            &mut out,
            "Rebuild this machine from a clean install. Quarantine does not make it trustworthy again. Do not restore a backup taken after the infection.",
            None,
        );
    } else {
        step(
            &mut out,
            "Decide whether this machine needs rebuilding. The payload runs when an infected project is built or opened in an editor. If that happened after the infection arrived, or you are not sure, treat the machine as compromised and rebuild it.",
            None,
        );
    }
    step(
        &mut out,
        "Clean the remote. The infected commit may still be in each repository's history and on GitHub. This beta does not do that yet; the released tool on the main branch does.",
        None,
    );
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn steps(findings: Vec<Finding>, command: &str, applied: bool, roots: &[&str]) -> String {
        let mut v = Verdict::new();
        for f in findings {
            v.push(f);
        }
        let roots: Vec<PathBuf> = roots.iter().map(PathBuf::from).collect();
        let flags = if command == "check" {
            vec![("--fs-only".to_string(), None)]
        } else {
            Vec::new()
        };
        next_steps(
            &v,
            &Run {
                command,
                scope_flags: &flags,
                roots: &roots,
                applied,
                quarantine: Path::new("/home/x/polinrider-quarantine-20261003T000000Z"),
            },
        )
    }

    /// The same words on one line, for asserting on a sentence without caring
    /// where it happened to wrap.
    fn flat(text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn config() -> Finding {
        Finding::hit(
            Kind::Config { strippable: true },
            "config file contains an indicator",
        )
    }

    #[test]
    fn something_outside_the_projects_means_rebuild() {
        let text = steps(
            vec![
                config(),
                Finding::hit(Kind::LoginItem, "systemd unit contains an indicator"),
            ],
            "check",
            false,
            &["/home/x/code"],
        );
        assert!(text.contains("The payload ran on this machine"), "{text}");
        assert!(
            text.contains("Rebuild this machine from a clean install"),
            "{text}"
        );
        assert!(
            text.contains("       polinrider check --apply --fs-only /home/x/code\n"),
            "{text}"
        );
        // clean refuses --fs-only, so the line for it must not carry it.
        assert!(
            text.contains("       polinrider clean --apply /home/x/code\n"),
            "{text}"
        );
        assert!(
            text.contains("       polinrider check --fs-only /home/x/code\n"),
            "{text}"
        );
    }

    #[test]
    fn a_payload_only_in_project_files_does_not_claim_the_machine_is_lost() {
        // Nothing proves it ran. Saying "rebuild" for a cloned file is the
        // same false alarm the rest of this tool works to avoid, and saying
        // "you are fine" would be worse. It says how to decide.
        let text = steps(
            vec![
                config(),
                Finding::hit(Kind::FakeFont, "font file is not a font"),
            ],
            "check",
            false,
            &["/home/x/code"],
        );
        assert!(
            text.contains("The payload is in your project files"),
            "{text}"
        );
        assert!(
            text.contains("Decide whether this machine needs rebuilding"),
            "{text}"
        );
        assert!(!text.contains("Rebuild this machine from"), "{text}");
        assert!(!text.contains("Disconnect"), "{text}");

        let mut v = Verdict::new();
        v.push(config());
        assert!(result(&v).contains("The payload is in your project files. See below."));
        v.push(Finding::hit(
            Kind::Process,
            "an implant process is running now",
        ));
        assert!(result(&v).contains("This machine cannot be trusted until it is rebuilt."));
    }

    #[test]
    fn a_running_implant_puts_the_network_first() {
        let text = steps(
            vec![Finding::hit(
                Kind::Process,
                "an implant process is running now",
            )],
            "check",
            false,
            &["/home/x/code"],
        );
        assert!(
            text.contains("  1. Disconnect this machine from the network now."),
            "{text}"
        );
        assert!(
            !text.contains("--apply"),
            "nothing here can be moved: {text}"
        );
    }

    #[test]
    fn a_path_with_a_space_is_printed_so_that_it_pastes() {
        let text = steps(
            vec![config()],
            "clean",
            false,
            &["/home/x/my code", "/srv/it's"],
        );
        assert!(
            text.contains("       polinrider clean --apply '/home/x/my code' '/srv/it'\\''s'\n"),
            "{text}"
        );
        // No command line ever carries a blank to fill in.
        for line in text.lines().filter(|l| l.starts_with("       polinrider ")) {
            assert!(!line.contains('<') && !line.contains("..."), "{line}");
        }
    }

    #[test]
    fn after_an_apply_it_does_not_say_to_apply_again() {
        let text = steps(
            vec![
                config(),
                Finding::hit(Kind::FakeFont, "font file is not a font"),
            ],
            "clean",
            true,
            &["/home/x/code"],
        );
        assert!(!text.contains("--apply"), "{text}");
        assert!(
            flat(&text).contains("the originals are in /home/x/polinrider-quarantine-"),
            "{text}"
        );
        assert!(
            text.contains("       polinrider clean /home/x/code\n"),
            "{text}"
        );
    }

    #[test]
    fn each_thing_only_a_person_can_do_gets_a_step() {
        let text = steps(
            vec![
                Finding::hit(
                    Kind::Config { strippable: false },
                    "config file contains an indicator",
                ),
                Finding::hit(Kind::Package, "known-bad package referenced"),
                Finding::hit(
                    Kind::StartupFile,
                    "shell startup file contains an indicator",
                ),
            ],
            "check",
            false,
            &["/home/x/code"],
        );
        assert!(
            text.contains("1 config file could not be stripped safely"),
            "{text}"
        );
        assert!(
            text.contains("Remove the campaign package from the manifest"),
            "{text}"
        );
        assert!(
            text.contains("1 finding has to be edited by hand"),
            "{text}"
        );
        // Steps are numbered without a gap.
        let numbers: Vec<&str> = text
            .lines()
            .filter_map(|l| {
                l.strip_prefix("  ")
                    .and_then(|l| l.split_once(". "))
                    .map(|(n, _)| n)
            })
            .filter(|n| n.chars().all(|c| c.is_ascii_digit()))
            .collect();
        let expected: Vec<String> = (1..=numbers.len()).map(|n| n.to_string()).collect();
        assert_eq!(numbers, expected, "{text}");
    }

    #[test]
    fn review_only_says_what_a_person_should_look_at_and_never_rebuild() {
        let text = steps(
            vec![Finding::review("user crontab is not empty")],
            "check",
            false,
            &["/x"],
        );
        assert!(
            flat(&text)
                .contains("Nothing is confirmed. 1 line marked [review] above needs a person"),
            "{text}"
        );
        assert!(
            flat(&text).contains("Ask whether you put it there"),
            "{text}"
        );
        assert!(!text.contains("Rebuild"), "{text}");
        assert!(
            !text.contains("polinrider "),
            "no command to run for a review: {text}"
        );
    }

    #[test]
    fn a_clean_result_is_one_sentence_and_no_list() {
        let text = steps(vec![Finding::ok("all clear")], "check", false, &["/x"]);
        assert!(!text.contains("WHAT TO DO NEXT"));
        assert!(text.contains("rotate your credentials anyway"), "{text}");
    }

    #[test]
    fn prose_wraps_under_its_number_and_never_past_the_width() {
        let wrapped = prose("  1. ", "     ", &"word ".repeat(40));
        assert!(wrapped.lines().count() > 1);
        assert!(wrapped.starts_with("  1. word word"));
        for (i, line) in wrapped.lines().enumerate() {
            assert!(line.chars().count() <= 78, "{line}");
            if i > 0 {
                assert!(line.starts_with("     word"), "{line}");
            }
        }
    }
}
