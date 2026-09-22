//! The indicator set, loaded from `ioc/` at runtime.
//!
//! Plain text files, one entry per line, `#` comments ignored. Deliberately
//! not compiled in: an incident responder has to be able to add an indicator
//! without rebuilding the tool, and the weekly review edits these files.
//!
//! `strong`, `bad_packages` and `weak` are matched as fixed substrings, the
//! same as `grep -F` in the shell. No regex, so nothing in the indicator set
//! can be a malformed pattern that silently matches nothing.

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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

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
                    "# a comment\n\nrmcej%otb%\n\n# another\nCot%3t=shtP\n",
                ),
                ("bad-packages.txt", "# packages\nevil-pkg\n"),
            ],
        );
        let ind = Indicators::load(&dir).expect("loads");
        assert!(ind.strong.contains(&"rmcej%otb%".to_string()));
        assert!(ind.strong.contains(&"Cot%3t=shtP".to_string()));
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
