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
    /// Nothing is confirmed, but a human has to look at this.
    Review,
    /// A confirmed indicator. No plausible innocent explanation.
    Hit,
}

impl Level {
    /// The tag as it appears in the output. The corpus depends on this.
    pub const fn tag(self) -> &'static str {
        match self {
            Level::Info => "[info]  ",
            Level::Review => "[review]",
            Level::Hit => "[HIT]   ",
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
}

impl Finding {
    pub fn new(level: Level, message: impl Into<String>) -> Self {
        Self { level, message: message.into(), remedy: None }
    }

    pub fn hit(message: impl Into<String>) -> Self {
        Self::new(Level::Hit, message)
    }

    pub fn review(message: impl Into<String>) -> Self {
        Self::new(Level::Review, message)
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(Level::Info, message)
    }

    #[must_use]
    pub fn with_remedy(mut self, remedy: impl Into<String>) -> Self {
        self.remedy = Some(remedy.into());
        self
    }
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

/// Everything a scan concluded.
#[derive(Debug, Default)]
pub struct Verdict {
    findings: Vec<Finding>,
}

impl Verdict {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, finding: Finding) {
        self.findings.push(finding);
    }

    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    pub fn count(&self, level: Level) -> usize {
        self.findings.iter().filter(|f| f.level == level).count()
    }

    pub fn hits(&self) -> usize {
        self.count(Level::Hit)
    }

    pub fn reviews(&self) -> usize {
        self.count(Level::Review)
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
        v.push(Finding::hit("config file contains an indicator"));
        assert_eq!(v.exit_code(), ExitCode::Confirmed);
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
