//! What a scan concluded, and how that becomes an exit code.
//!
//! The exit codes are a contract other tools depend on, so they live here as
//! one type rather than as integers scattered through the code. See ADR-0002
//! for why `CouldNotRun` is separate from `Confirmed`: they used to share a
//! code, and pointing the scanner at a path that did not exist printed the
//! full compromise playbook.

use std::fmt;

/// How serious a single finding is.
///
/// These are the four the shell implementation prints, and the conformance
/// corpus matches on their exact spelling. Changing the text is a breaking
/// change to anything that greps the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Inventory or hardening advice. Deliberately not counted towards the
    /// verdict: "eight things need your attention" is untrue when six of them
    /// are "you own an SSH key".
    Info,
    /// A check ran and found nothing.
    Ok,
    /// Nothing is confirmed, but a human has to look at this.
    Review,
    /// A confirmed indicator. No plausible innocent explanation.
    Hit,
}

impl Level {
    /// The tag as it appears in the output. The corpus depends on this.
    pub const fn tag(self) -> &'static str {
        match self {
            Level::Ok => "[ok]    ",
            Level::Info => "[info]  ",
            Level::Review => "[review]",
            Level::Hit => "[HIT]   ",
        }
    }
}

/// What a confirmed finding is.
///
/// Every `[HIT]` has one. It is an argument to [`Finding::hit`] and not a
/// field to remember, so a finding that the summary and the "what to do next"
/// block cannot account for does not compile. Everything said about a finding
/// in plain words comes from here, in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A build config carrying a payload. `clean` can cut it out, or the
    /// shape is not one it will touch.
    Config { strippable: bool },
    /// A font file whose bytes are a script.
    FakeFont,
    /// A `tasks.json` that runs on folder open and carries an indicator.
    TasksJson,
    /// A campaign package named in a manifest.
    Package,
    /// A git hook carrying an indicator.
    GitHook,
    /// The second-stage implant, on disk.
    Implant,
    /// The script the payload uses to push itself to other repositories.
    Propagation,
    /// An editor extension carrying an indicator.
    Extension,
    /// Something that starts at login or on a schedule: a systemd unit, an
    /// autostart entry, a launch item, a system cron entry.
    LoginItem,
    /// The user's crontab.
    Crontab,
    /// A shell startup file.
    StartupFile,
    /// The npm registry setting.
    Registry,
    /// Windows: a registry Run entry or a scheduled task. Not a file, so it
    /// cannot be moved; a command removes it.
    Autostart,
    /// A process running now.
    Process,
    /// A connection open now.
    Connection,
}

impl Kind {
    /// Does this prove the payload ran on this machine, as opposed to sitting
    /// in a file that was cloned onto it? The difference between "rebuild"
    /// and "decide whether to rebuild".
    pub const fn ran_here(self) -> bool {
        !matches!(
            self,
            Kind::Config { .. } | Kind::FakeFont | Kind::TasksJson | Kind::Package
        )
    }

    /// Can `--apply` move it into quarantine whole?
    pub const fn movable(self) -> bool {
        matches!(
            self,
            Kind::FakeFont
                | Kind::TasksJson
                | Kind::GitHook
                | Kind::Implant
                | Kind::Propagation
                | Kind::Extension
                | Kind::LoginItem
        )
    }

    /// Does only a person editing a file fix it?
    pub const fn by_hand(self) -> bool {
        matches!(
            self,
            Kind::Crontab | Kind::StartupFile | Kind::Registry | Kind::Autostart
        )
    }

    /// Is it happening now?
    pub const fn running(self) -> bool {
        matches!(self, Kind::Process | Kind::Connection)
    }

    /// What it is, in words somebody who has never heard of an indicator can
    /// read, for `n` of them. No path, no jargon.
    pub fn plain(self, n: usize) -> String {
        let (one, many) = match self {
            Kind::Config { .. } => (
                "config file with the payload hidden in it",
                "config files with the payload hidden in them",
            ),
            Kind::FakeFont => (
                "font file that is really a script",
                "font files that are really scripts",
            ),
            Kind::TasksJson => (
                "editor task that runs the payload when a folder is opened",
                "editor tasks that run the payload when a folder is opened",
            ),
            Kind::Package => (
                "project that depends on a campaign package",
                "projects that depend on a campaign package",
            ),
            Kind::GitHook => (
                "git hook carrying the payload",
                "git hooks carrying the payload",
            ),
            Kind::Implant => (
                "copy of the implant program",
                "copies of the implant program",
            ),
            Kind::Propagation => (
                "script the payload uses to spread",
                "scripts the payload uses to spread",
            ),
            Kind::Extension => (
                "editor extension carrying the payload",
                "editor extensions carrying the payload",
            ),
            Kind::LoginItem => (
                "login item that starts the payload",
                "login items that start the payload",
            ),
            Kind::Crontab => (
                "scheduled job that calls the campaign",
                "scheduled jobs that call the campaign",
            ),
            Kind::StartupFile => (
                "shell startup file that runs the payload",
                "shell startup files that run the payload",
            ),
            Kind::Registry => (
                "npm setting pointing at the campaign",
                "npm settings pointing at the campaign",
            ),
            Kind::Autostart => (
                "entry that starts the payload when Windows starts",
                "entries that start the payload when Windows starts",
            ),
            Kind::Process => (
                "program from the payload running right now",
                "programs from the payload running right now",
            ),
            Kind::Connection => (
                "live connection to the campaign's servers",
                "live connections to the campaign's servers",
            ),
        };
        format!("{n} {}", if n == 1 { one } else { many })
    }

    /// The same thing as the subject of a sentence: "A login item means...".
    /// Only for kinds that prove the payload ran.
    pub const fn evidence(self) -> &'static str {
        match self {
            Kind::GitHook => "A git hook",
            Kind::Implant => "A copy of the implant",
            Kind::Propagation => "The spreading script",
            Kind::Extension => "An infected editor extension",
            Kind::LoginItem => "A login item",
            Kind::Crontab => "A scheduled job",
            Kind::StartupFile => "A shell startup file",
            Kind::Registry => "An npm setting",
            Kind::Autostart => "An entry that starts with Windows",
            Kind::Process => "A running program",
            Kind::Connection => "A live connection",
            Kind::Config { .. } | Kind::FakeFont | Kind::TasksJson | Kind::Package => {
                "A file in a project"
            }
        }
    }
}

/// One thing the scan concluded, about one place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub level: Level,
    /// What was found, in a sentence. Never a bare path.
    pub message: String,
    /// What to do about it, printed indented under the message. `None` when
    /// the message is self-explanatory.
    pub remedy: Option<String>,
    /// What kind of confirmed finding this is. `None` for everything that is
    /// not a `[HIT]`.
    pub kind: Option<Kind>,
    /// The file or directory this is about, when it is about one. Held apart
    /// from the message so that a display can put it on its own line without
    /// guessing where the sentence ends and the path begins.
    pub path: Option<std::path::PathBuf>,
}

impl Finding {
    pub fn new(level: Level, message: impl Into<String>) -> Self {
        Self {
            level,
            message: message.into(),
            remedy: None,
            kind: None,
            path: None,
        }
    }

    pub fn hit(kind: Kind, message: impl Into<String>) -> Self {
        Self {
            kind: Some(kind),
            ..Self::new(Level::Hit, message)
        }
    }

    pub fn review(message: impl Into<String>) -> Self {
        Self::new(Level::Review, message)
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(Level::Info, message)
    }

    /// A check that ran and found nothing. Not counted in the verdict, but it
    /// must be printed: a silent section is indistinguishable from one that
    /// never ran, and that ambiguity is how a skipped check gets missed.
    pub fn ok(message: impl Into<String>) -> Self {
        Self::new(Level::Ok, message)
    }

    /// Name the file or directory this finding is about.
    #[must_use]
    pub fn at(mut self, path: impl AsRef<std::path::Path>) -> Self {
        self.path = Some(path.as_ref().to_path_buf());
        self
    }

    /// The message without its path on the end, for a display that shows the
    /// path on a line of its own.
    pub fn what(&self) -> &str {
        match &self.path {
            Some(path) => self
                .message
                .strip_suffix(&path.display().to_string())
                .map_or(self.message.as_str(), |m| {
                    m.trim_end().trim_end_matches(':')
                }),
            None => &self.message,
        }
    }

    /// Add a line of advice under the message. Called twice, it adds a second
    /// line rather than replacing the first: "stop it first" and "would
    /// quarantine" both belong under a persistence finding.
    #[must_use]
    pub fn with_remedy(mut self, remedy: impl Into<String>) -> Self {
        let remedy = remedy.into();
        self.remedy = Some(match self.remedy {
            Some(existing) => format!("{existing}\n{remedy}"),
            None => remedy,
        });
        self
    }
}

/// Strip everything that could drive a terminal out of one line of output.
///
/// Paths, command lines, crontab entries and file contents all reach the
/// report, and all of them can be chosen by whoever planted the thing being
/// reported. Control characters go, including the C1 range some terminals
/// read as an escape introducer. A newline goes too: a filename containing
/// one could otherwise forge a second line that looks like a finding.
pub fn clean(line: &str) -> String {
    line.chars()
        .map(|c| if c == '\t' { ' ' } else { c })
        .filter(|c| !c.is_control())
        .collect()
}

/// The exit code contract. Documented in docs-site reference/exit-codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExitCode {
    /// Clean against the current indicator set. Not proof of anything more.
    Clean = 0,
    /// Review items only; nothing confirmed.
    Review = 1,
    /// At least one confirmed indicator.
    Confirmed = 2,
    /// The scan could not run. Separate from `Confirmed` on purpose: a tool
    /// that reports a compromise it did not find is worse than one that
    /// reports nothing. ADR-0002.
    CouldNotRun = 3,
}

impl ExitCode {
    pub const fn code(self) -> i32 {
        self as i32
    }
}

impl fmt::Display for ExitCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ExitCode::Clean => "clean against the current indicator set",
            ExitCode::Review => "no confirmed indicator; review items only",
            ExitCode::Confirmed => "COMPROMISED",
            ExitCode::CouldNotRun => "the scan could not run",
        };
        f.write_str(s)
    }
}

/// One line of a scan: a section heading, a finding inside it, or the
/// evidence a finding refers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Section(String),
    Finding(Finding),
    /// Evidence printed under a finding: a process, a crontab line. Shown on
    /// the console cut to a readable width and written to the report whole.
    /// Carries no level and counts towards nothing.
    Detail(String),
    /// Inventory that goes to the report file only. On the console a wall of
    /// paths nobody reads is worse than the count that summarises it.
    Note(String),
}

/// Everything a scan concluded, in the order it concluded it.
#[derive(Debug, Default)]
pub struct Verdict {
    entries: Vec<Entry>,
    /// How many files the walk listed. Said back to the operator, because
    /// "it was quick" and "it looked at eight files" are the same fact.
    files: usize,
}

impl Verdict {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, finding: Finding) {
        self.entries.push(Entry::Finding(finding));
    }

    /// Start a section. Every check opens one, so a section that prints no
    /// findings still shows it ran.
    pub fn section(&mut self, title: impl Into<String>) {
        self.entries.push(Entry::Section(title.into()));
    }

    pub fn detail(&mut self, line: impl Into<String>) {
        self.entries.push(Entry::Detail(line.into()));
    }

    pub fn note(&mut self, line: impl Into<String>) {
        self.entries.push(Entry::Note(line.into()));
    }

    pub fn set_files(&mut self, files: usize) {
        self.files = files;
    }

    pub fn files(&self) -> usize {
        self.files
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn findings(&self) -> impl Iterator<Item = &Finding> {
        self.entries.iter().filter_map(|e| match e {
            Entry::Finding(f) => Some(f),
            Entry::Section(_) | Entry::Detail(_) | Entry::Note(_) => None,
        })
    }

    pub fn count(&self, level: Level) -> usize {
        self.findings().filter(|f| f.level == level).count()
    }

    pub fn hits(&self) -> usize {
        self.count(Level::Hit)
    }

    pub fn reviews(&self) -> usize {
        self.count(Level::Review)
    }

    /// The kind of every confirmed finding.
    pub fn kinds(&self) -> impl Iterator<Item = Kind> + '_ {
        self.findings().filter_map(|f| f.kind)
    }

    /// The exit code this verdict implies.
    ///
    /// `CouldNotRun` is never produced here: it means the scan did not happen,
    /// which is a condition the caller detects before there is a verdict to
    /// read at all.
    pub fn exit_code(&self) -> ExitCode {
        if self.hits() > 0 {
            ExitCode::Confirmed
        } else if self.reviews() > 0 {
            ExitCode::Review
        } else {
            ExitCode::Clean
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn a_clean_scan_exits_zero() {
        assert_eq!(Verdict::new().exit_code(), ExitCode::Clean);
    }

    #[test]
    fn info_alone_is_still_clean() {
        // Owning an SSH key is not a finding. This is the whole reason Info
        // exists as a separate level.
        let mut v = Verdict::new();
        v.push(Finding::info("3 private key files under the scanned paths"));
        v.push(Finding::info("npm ignore-scripts is 'false'"));
        assert_eq!(v.exit_code(), ExitCode::Clean);
        assert_eq!(v.reviews(), 0);
    }

    #[test]
    fn review_alone_exits_one() {
        let mut v = Verdict::new();
        v.push(Finding::review("tasks.json runs on folder open"));
        assert_eq!(v.exit_code(), ExitCode::Review);
    }

    #[test]
    fn one_hit_outranks_any_number_of_reviews() {
        let mut v = Verdict::new();
        for _ in 0..10 {
            v.push(Finding::review("something to look at"));
        }
        v.push(Finding::hit(
            Kind::FakeFont,
            "config file contains an indicator",
        ));
        assert_eq!(v.exit_code(), ExitCode::Confirmed);
    }

    #[test]
    fn evidence_lines_do_not_count_towards_the_verdict() {
        let mut v = Verdict::new();
        v.detail("  412 node -e ...");
        v.note("/etc/systemd/system/some.service");
        assert_eq!(v.exit_code(), ExitCode::Clean);
        assert_eq!(v.findings().count(), 0);
    }

    #[test]
    fn a_second_remedy_is_added_not_swapped_in() {
        let f = Finding::hit(Kind::LoginItem, "launch item contains an indicator")
            .with_remedy("unload it first")
            .with_remedy("would quarantine: /x");
        assert_eq!(
            f.remedy.as_deref(),
            Some("unload it first\nwould quarantine: /x")
        );
    }

    #[test]
    fn output_cannot_carry_an_escape_sequence_or_forge_a_line() {
        // A file named to clear the screen, and one named to print a fake
        // "[ok]" line under the real finding.
        assert_eq!(clean("evil\x1b[2Jname"), "evil[2Jname");
        assert_eq!(clean("a\n  [ok]     all fine"), "a  [ok]     all fine");
        assert_eq!(clean("a\u{9b}31mb"), "a31mb");
        assert_eq!(clean("tab\tseparated"), "tab separated");
        assert_eq!(
            clean("plain path/with spaces.js"),
            "plain path/with spaces.js"
        );
    }

    #[test]
    fn exit_codes_match_the_documented_contract() {
        // These numbers are depended on by CI, by scripts, and by the
        // conformance corpus. They are not free to change.
        assert_eq!(ExitCode::Clean.code(), 0);
        assert_eq!(ExitCode::Review.code(), 1);
        assert_eq!(ExitCode::Confirmed.code(), 2);
        assert_eq!(ExitCode::CouldNotRun.code(), 3);
    }
}
