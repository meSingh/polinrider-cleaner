//! The indicator set, loaded from `ioc/` at runtime.
//!
//! Plain text files, one entry per line, `#` comments ignored. Deliberately
//! not compiled in: an incident responder has to be able to add an indicator
//! without rebuilding the tool, and the weekly review edits these files.
//!
//! `strong`, `bad_packages` and `weak` are matched as fixed substrings, the
//! same as `grep -F` in the shell. No regex, so nothing in the indicator set
//! can be a malformed pattern that silently matches nothing.

use crate::pattern::{BadPattern, Pattern};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Everything loaded out of `ioc/`.
#[derive(Debug, Default)]
pub struct Indicators {
    /// A match means infection, with no plausible alternative.
    pub strong: Vec<String>,
    /// Strings legitimate projects also contain. Review only, never a hit.
    pub weak: Vec<String>,
    /// Campaign package names, as they appear in a manifest or lockfile.
    pub bad_packages: Vec<String>,
    /// Campaign hosts and addresses.
    pub network: Vec<String>,
    /// Implant binary and process names. Matched against a process name only,
    /// never a command line: matching the command line reports this scanner,
    /// and anyone grepping for the implant, as the implant.
    pub implant_names: Vec<String>,
    /// Paths that are indicators on their own, as patterns. From
    /// `filenames.txt`, which the shell hands to `grep -E`.
    pub filenames: Vec<Pattern>,
}

/// Why the indicator set could not be used. Each of these is an exit code 3
/// condition: the scan did not happen, and must not be reported as clean.
#[derive(Debug)]
pub enum LoadError {
    Missing {
        dir: PathBuf,
    },
    Unreadable {
        file: PathBuf,
        source: io::Error,
    },
    /// A present but empty `strong.txt` would make every scan pass.
    Empty,
    /// A pattern in `filenames.txt` that this build cannot honour. Refused,
    /// because a pattern that silently matches nothing is an indicator that
    /// silently stopped working.
    Pattern(BadPattern),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Missing { dir } => {
                write!(f, "no indicator directory at {}", dir.display())
            }
            LoadError::Unreadable { file, source } => {
                write!(f, "cannot read {}: {source}", file.display())
            }
            LoadError::Pattern(bad) => write!(f, "in filenames.txt, {bad}"),
            LoadError::Empty => f.write_str(
                "the indicator set is empty. Refusing to scan: every result would be clean",
            ),
        }
    }
}

/// Read one indicator file. A missing optional file is an empty list; an
/// unreadable one is an error, because "I could not read it" and "it contained
/// nothing" must not look the same.
fn read_list(dir: &Path, name: &str, required: bool) -> Result<Vec<String>, LoadError> {
    let path = dir.join(name);
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound && !required => return Ok(Vec::new()),
        Err(e) => {
            return Err(LoadError::Unreadable {
                file: path,
                source: e,
            })
        }
    };
    Ok(text
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_owned)
        .collect())
}

impl Indicators {
    pub fn load(dir: &Path) -> Result<Self, LoadError> {
        if !dir.is_dir() {
            return Err(LoadError::Missing {
                dir: dir.to_path_buf(),
            });
        }
        let mut strong = read_list(dir, "strong.txt", true)?;
        let bad_packages = read_list(dir, "bad-packages.txt", true)?;

        // The shell greps strong.txt and bad-packages.txt together when asking
        // "does this file contain an indicator", so a campaign package name
        // found inside a file counts. Matched here for the same reason.
        strong.extend(bad_packages.iter().cloned());

        let indicators = Indicators {
            strong,
            weak: read_list(dir, "weak.txt", false)?,
            bad_packages,
            network: read_list(dir, "network.txt", false)?,
            implant_names: read_list(dir, "implant-names.txt", false)?,
            filenames: read_list(dir, "filenames.txt", false)?
                .iter()
                .map(|p| Pattern::parse(p).map_err(LoadError::Pattern))
                .collect::<Result<_, _>>()?,
        };

        if indicators.strong.is_empty() {
            return Err(LoadError::Empty);
        }
        Ok(indicators)
    }

    /// Does this text contain a confirmed indicator?
    pub fn has_strong(&self, haystack: &str) -> bool {
        self.strong.iter().any(|i| haystack.contains(i.as_str()))
    }

    /// Does this text name a campaign package?
    pub fn has_bad_package(&self, haystack: &str) -> bool {
        self.bad_packages
            .iter()
            .any(|i| haystack.contains(i.as_str()))
    }

    /// The campaign host or address this line names, if it names one.
    ///
    /// Not a plain substring test. `grep -F` for `23.0.0.1` also matches
    /// `123.0.0.1` and `23.0.0.19`, which are other people's addresses, and a
    /// connection to one of those would be reported as a live connection to
    /// the campaign. The match has to end where the address or name ends.
    pub fn infrastructure_in(&self, line: &str) -> Option<&str> {
        self.network
            .iter()
            .map(String::as_str)
            .find(|indicator| names_endpoint(line, indicator))
    }

    /// Every campaign host or address `text` names, in the order the
    /// indicator file lists them. A minified bundle is one line, so asking
    /// line by line for the first match would report one name and miss the
    /// rest.
    pub fn infrastructure_named(&self, text: &str) -> Vec<&str> {
        self.network
            .iter()
            .map(String::as_str)
            .filter(|indicator| text.lines().any(|line| names_endpoint(line, indicator)))
            .collect()
    }

    /// Windows: is this the image name of the implant, or does this command
    /// run it? Compared without regard to case and with or without `.exe`,
    /// as Windows itself compares them.
    pub fn is_implant_image(&self, name: &str) -> bool {
        let bare = |s: &str| {
            let lower = s.to_ascii_lowercase();
            lower
                .strip_suffix(".exe")
                .map_or(lower.clone(), str::to_owned)
        };
        let base = bare(name.rsplit(['/', '\\']).next().unwrap_or(name));
        !base.is_empty() && self.implant_names.iter().any(|i| bare(i) == base)
    }

    /// Windows: the implant named anywhere in a command line or a registry
    /// value. Whole names only: a longer name that merely begins with the
    /// implant's is somebody else's program.
    pub fn names_implant(&self, command: &str) -> bool {
        let lower = command.to_ascii_lowercase();
        self.implant_names.iter().any(|implant| {
            let implant = implant.to_ascii_lowercase();
            let implant = implant.strip_suffix(".exe").unwrap_or(&implant);
            !implant.is_empty()
                && lower.match_indices(implant).any(|(at, found)| {
                    let before = lower.get(..at).and_then(|b| b.chars().next_back());
                    let after = lower
                        .get(at + found.len()..)
                        .unwrap_or_default()
                        .trim_start_matches(".exe")
                        .chars()
                        .next();
                    let word = |c: Option<char>| {
                        c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                    };
                    !word(before) && !word(after)
                })
        })
    }

    /// Is this the name of the implant process?
    ///
    /// The name only, compared whole, never a command line: anything that
    /// merely mentions the implant, this scanner included, would otherwise be
    /// reported as the implant running.
    ///
    /// `kernel_truncates` is true on Linux, where the kernel keeps 15 bytes of
    /// a process name. An implant whose name is longer shows up cut short, and
    /// comparing whole names never matches it. A name of exactly 15 bytes that
    /// begins a longer implant name is that implant.
    pub fn is_implant_process(&self, name: &str, kernel_truncates: bool) -> bool {
        const KERNEL_NAME_LEN: usize = 15;
        let base = name.rsplit('/').next().unwrap_or(name);
        if base.is_empty() {
            return false;
        }
        self.implant_names.iter().any(|implant| {
            implant == base
                || (kernel_truncates
                    && base.len() == KERNEL_NAME_LEN
                    && implant.len() > KERNEL_NAME_LEN
                    && implant.starts_with(base))
        })
    }

    /// Is this path an indicator by its name alone?
    pub fn is_bad_filename(&self, path: &str) -> bool {
        self.filenames.iter().any(|p| p.is_match(path))
    }

    /// Read a file and test it. Binary files are read lossily rather than
    /// skipped: the payload is appended to text files, but a file with one
    /// invalid byte is still worth matching.
    pub fn file_has_strong(&self, path: &Path) -> bool {
        match fs::read(path) {
            Ok(bytes) => self.has_strong(&String::from_utf8_lossy(&bytes)),
            Err(_) => false,
        }
    }
}

fn is_ipv4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Does `line` contain `indicator` as a whole address or a whole host name?
fn names_endpoint(line: &str, indicator: &str) -> bool {
    if indicator.is_empty() {
        return false;
    }
    let address = is_ipv4(indicator);
    line.match_indices(indicator).any(|(at, found)| {
        let before = line.get(..at).and_then(|s| s.chars().next_back());
        let after = line.get(at + found.len()..).and_then(|s| s.chars().next());
        if address {
            // A dot may follow: BSD netstat writes the port as a fifth group.
            // A digit either side, or a dot before, is a different address.
            !before.is_some_and(|c| c.is_ascii_digit() || c == '.')
                && !after.is_some_and(|c| c.is_ascii_digit())
        } else {
            // A dot before is a subdomain of the campaign host, which counts.
            let part_of_a_name = |c: char| c.is_ascii_alphanumeric() || c == '-';
            !before.is_some_and(part_of_a_name) && !after.is_some_and(part_of_a_name)
        }
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn with(network: &[&str], implants: &[&str]) -> Indicators {
        Indicators {
            strong: vec!["MARKER-ALPHA".into()],
            network: network.iter().map(|s| (*s).to_string()).collect(),
            implant_names: implants.iter().map(|s| (*s).to_string()).collect(),
            ..Indicators::default()
        }
    }

    #[test]
    fn an_address_matches_whole_or_not_at_all() {
        // 203.0.113.0/24 is reserved for documentation. Never a live address.
        let ind = with(&["203.0.113.7"], &[]);
        assert!(ind
            .infrastructure_in("tcp ESTAB 10.0.0.2:51514 203.0.113.7:443")
            .is_some());
        // BSD netstat: the port is a fifth dotted group.
        assert!(ind
            .infrastructure_in("tcp4 10.0.0.2.51514 203.0.113.7.443")
            .is_some());
        // Somebody else's address that merely contains it.
        assert!(ind
            .infrastructure_in("10.0.0.2:51514 203.0.113.71:443")
            .is_none());
        assert!(ind
            .infrastructure_in("10.0.0.2:51514 1203.0.113.7:443")
            .is_none());
        assert!(ind
            .infrastructure_in("10.0.0.2:51514 9.203.0.113.7:443")
            .is_none());
    }

    #[test]
    fn a_host_name_matches_itself_and_its_subdomains_only() {
        let ind = with(&["c2.example"], &[]);
        assert!(ind
            .infrastructure_in("node 77 TCP h:1->c2.example:443")
            .is_some());
        assert!(ind
            .infrastructure_in("TCP h:1->api.c2.example:443")
            .is_some());
        assert!(ind
            .infrastructure_in("TCP h:1->notc2.example:443")
            .is_none());
        assert!(ind.infrastructure_in("TCP h:1->c2.examples:443").is_none());
    }

    #[test]
    fn an_implant_is_matched_by_process_name_not_by_mention() {
        let ind = with(&[], &["implant-process-name-x64"]);
        assert!(ind.is_implant_process("implant-process-name-x64", false));
        // macOS reports the executable's full path.
        assert!(ind.is_implant_process("/Users/x/Library/implant-process-name-x64", false));
        // A different program whose name merely contains it.
        assert!(!ind.is_implant_process("not-implant-process-name-x64", false));
        assert!(!ind.is_implant_process("grep", false));
        assert!(!ind.is_implant_process("", false));
    }

    #[test]
    fn linux_cuts_a_process_name_to_15_bytes_and_the_cut_name_still_matches() {
        // Verified in the sandbox: a binary called MicrosoftSystem64 shows in
        // ps as MicrosoftSystem. A whole-name comparison never matches it, so
        // the check was silently dead on Linux.
        let ind = with(&[], &["implant-process-name-x64"]);
        assert!(ind.is_implant_process("implant-process", true));
        // Not on macOS, where names are not cut and 15 bytes is just a name.
        assert!(!ind.is_implant_process("implant-process", false));
        // A shorter prefix is somebody else's program.
        assert!(!ind.is_implant_process("implant", true));
    }

    /// A named directory per test. The first version derived the name from the
    /// fixture contents, which collided between tests, and cargo runs them in
    /// parallel: one test overwrote another's indicator files and the failure
    /// looked like a parsing bug.
    fn fixture(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prc-ioc-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        for (name, body) in files {
            fs::write(dir.join(name), body).expect("write fixture");
        }
        dir
    }

    #[test]
    fn comments_and_blank_lines_are_not_indicators() {
        // A scanner that treats "#" as an indicator matches every shell script
        // on the machine.
        let dir = fixture(
            "comments",
            &[
                (
                    "strong.txt",
                    // Synthetic markers, never live ones. A repository that
                    // commits real indicator strings trips every scanner that
                    // clones it, including its own. This test is about comment
                    // and blank-line parsing, not about any one indicator.
                    "# a comment\n\nMARKER-ALPHA\n\n# another\nMARKER-BETA\n",
                ),
                ("bad-packages.txt", "# packages\nevil-pkg\n"),
            ],
        );
        let ind = Indicators::load(&dir).expect("loads");
        assert!(ind.strong.contains(&"MARKER-ALPHA".to_string()));
        assert!(ind.strong.contains(&"MARKER-BETA".to_string()));
        assert!(!ind.strong.iter().any(|s| s.starts_with('#')));
        assert!(!ind.strong.iter().any(|s| s.is_empty()));
    }

    #[test]
    fn an_empty_indicator_set_refuses_rather_than_passing_everything() {
        // The failure this prevents: strong.txt truncated, every scan clean.
        let dir = fixture(
            "empty",
            &[
                ("strong.txt", "# nothing but comments\n"),
                ("bad-packages.txt", ""),
            ],
        );
        assert!(matches!(Indicators::load(&dir), Err(LoadError::Empty)));
    }

    #[test]
    fn a_missing_directory_is_an_error_not_an_empty_set() {
        let dir = std::env::temp_dir().join("prc-ioc-definitely-absent");
        let _ = fs::remove_dir_all(&dir);
        assert!(matches!(
            Indicators::load(&dir),
            Err(LoadError::Missing { .. })
        ));
    }

    #[test]
    fn filename_patterns_load_and_a_bad_one_stops_the_scan() {
        let dir = fixture(
            "filenames",
            &[
                ("strong.txt", "marker-one\n"),
                ("bad-packages.txt", ""),
                ("filenames.txt", "# by name\n(^|/)temp_helper\\.bat$\n"),
            ],
        );
        let ind = Indicators::load(&dir).expect("loads");
        assert!(ind.is_bad_filename("win/temp_helper.bat"));
        assert!(!ind.is_bad_filename("win/temp_helper.bat.bak"));

        let bad = fixture(
            "badpattern",
            &[
                ("strong.txt", "marker-one\n"),
                ("bad-packages.txt", ""),
                ("filenames.txt", "(^|/unclosed\n"),
            ],
        );
        assert!(matches!(Indicators::load(&bad), Err(LoadError::Pattern(_))));
    }

    #[test]
    fn package_names_also_count_as_content_indicators() {
        let dir = fixture(
            "packages",
            &[
                ("strong.txt", "marker-one\n"),
                ("bad-packages.txt", "evil-pkg\n"),
            ],
        );
        let ind = Indicators::load(&dir).expect("loads");
        assert!(ind.has_strong("something evil-pkg something"));
        assert!(ind.has_bad_package(r#"{"deps":{"evil-pkg":"1.0.0"}}"#));
        assert!(!ind.has_strong("entirely ordinary content"));
    }
}
