//! The indicator set, loaded from `ioc/` at runtime.
//!
//! Plain text files, one entry per line, `#` comments ignored. Deliberately
//! not compiled in: an incident responder has to be able to add an indicator
//! without rebuilding the tool, and the weekly review edits these files.
//!
//! `strong` and `weak` are matched as fixed substrings, the same as `grep -F`
//! in the shell. No regex, so nothing in the indicator set can be a malformed
//! pattern that silently matches nothing. `bad_packages` are names, and are
//! compared as whole names: read out of a manifest or a lockfile where the
//! file is one, and bounded by characters a package name cannot hold where it
//! is not (ADR-0041). Both are also matched against a copy of the text with
//! its escape codes decoded, when it has any (ADR-0040).

use crate::manifest;
use crate::pattern::{BadPattern, Pattern};
use crate::unescape;
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
    /// Paths worth a look but not proof, from `filenames-review.txt`. A match
    /// is review, never confirmed: ADR-0048.
    pub review_filenames: Vec<Pattern>,
    /// SSH keys the campaign installs, from `ssh-keys.txt`: a key's comment or
    /// the key itself, compared whole against that field: ADR-0049.
    pub ssh_keys: Vec<String>,
    /// SHA-256 of files known from the campaign, lowercase, each with its
    /// label. From `hashes.txt`, `<sha256> <label>` per line.
    pub hashes: Vec<(String, String)>,
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
    /// The file it is in, and why.
    Pattern(&'static str, BadPattern),
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
            LoadError::Pattern(file, bad) => write!(f, "in {file}, {bad}"),
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
        let strong = read_list(dir, "strong.txt", true)?;
        // A campaign package name found inside a file counts as a confirmed
        // indicator, as it did when the shell grepped both lists together.
        // It is held apart so that it can be compared as a whole name: see
        // `has_strong` and ADR-0041.
        let bad_packages = read_list(dir, "bad-packages.txt", true)?;

        let indicators = Indicators {
            strong,
            weak: read_list(dir, "weak.txt", false)?,
            bad_packages,
            network: read_list(dir, "network.txt", false)?,
            implant_names: read_list(dir, "implant-names.txt", false)?,
            filenames: read_list(dir, "filenames.txt", false)?
                .iter()
                .map(|p| Pattern::parse(p).map_err(|e| LoadError::Pattern("filenames.txt", e)))
                .collect::<Result<_, _>>()?,
            review_filenames: read_list(dir, "filenames-review.txt", false)?
                .iter()
                .map(|p| {
                    Pattern::parse(p).map_err(|e| LoadError::Pattern("filenames-review.txt", e))
                })
                .collect::<Result<_, _>>()?,
            ssh_keys: read_list(dir, "ssh-keys.txt", false)?
                .into_iter()
                .map(|l| l.trim().to_string())
                .collect(),
            hashes: read_list(dir, "hashes.txt", false)?
                .iter()
                .filter_map(|l| {
                    let (h, label) = l.split_once(char::is_whitespace)?;
                    Some((h.to_ascii_lowercase(), label.trim().to_string()))
                })
                .collect(),
        };

        if indicators.strong.is_empty() && indicators.bad_packages.is_empty() {
            return Err(LoadError::Empty);
        }
        Ok(indicators)
    }

    /// The label of the known campaign file these bytes are, if they are one.
    /// For files a check reads anyway, whatever their size: ADR-0045.
    pub fn known_file(&self, bytes: &[u8]) -> Option<&str> {
        if self.hashes.is_empty() {
            return None;
        }
        let digest = crate::sha256::bytes(bytes);
        self.hashes
            .iter()
            .find(|(h, _)| *h == digest)
            .map(|(_, label)| label.as_str())
    }

    /// Does this text contain a confirmed indicator, as written or once its
    /// escape codes are decoded? The second only for text that has escapes,
    /// so a file without them is read once. See ADR-0040.
    ///
    /// A campaign package name counts when it stands as a whole name, never
    /// as part of a longer one: `sass-format` is not in `sass-formatter`.
    pub fn has_strong(&self, haystack: &str) -> bool {
        self.matches(haystack) || unescape::decode(haystack).is_some_and(|d| self.matches(&d))
    }

    /// `strong.txt` alone, as written or decoded: the question asked of a
    /// manifest, whose package names are read rather than searched for.
    fn has_strong_text(&self, haystack: &str) -> bool {
        matches_or_decoded(&self.strong, haystack)
    }

    fn matches(&self, haystack: &str) -> bool {
        any_in(&self.strong, haystack) || self.names_package(haystack)
    }

    /// Does this text name a campaign package as a whole name? Bounded on both
    /// sides by something that cannot be part of a package name, and not the
    /// second half of a scoped name (`@other/name` is another package).
    pub fn names_package(&self, haystack: &str) -> bool {
        self.bad_packages
            .iter()
            .any(|name| whole_name_at(haystack, name).is_some())
    }

    /// Does this text contain a confirmed indicator only once its escape
    /// codes are decoded? The bytes of such a file do not hold the indicator,
    /// so nothing can be cut out of it by offset.
    pub fn has_strong_only_escaped(&self, haystack: &str) -> bool {
        !self.matches(haystack) && unescape::decode(haystack).is_some_and(|d| self.matches(&d))
    }

    /// The same question asked of bytes that need not be UTF-8. They are
    /// matched as they are first, and read as text only when they have an
    /// escape in them to decode or a package name to bound.
    pub fn bytes_have_strong(&self, bytes: &[u8]) -> bool {
        if self
            .strong
            .iter()
            .any(|i| !i.is_empty() && contains(bytes, i.as_bytes()))
        {
            return true;
        }
        let text = String::from_utf8_lossy(bytes);
        self.names_package(&text)
            || (unescape::has_escape(bytes)
                && unescape::decode(&text).is_some_and(|d| self.matches(&d)))
    }

    /// Where the first confirmed indicator starts, as written. For a cut,
    /// which is made at a byte offset.
    pub fn first_at(&self, bytes: &[u8]) -> Option<usize> {
        let text = String::from_utf8_lossy(bytes);
        let lossless = matches!(text, std::borrow::Cow::Borrowed(_));
        let strong = self.strong.iter().filter_map(|i| find(bytes, i.as_bytes()));
        // An offset into a lossy copy is not an offset into the bytes, so a
        // package name is placed only in a file that is valid UTF-8.
        let packages = self
            .bad_packages
            .iter()
            .filter(|_| lossless)
            .filter_map(|name| whole_name_at(&text, name));
        strong.chain(packages).min()
    }

    /// Does this text name a campaign package anywhere, as a fixed string,
    /// written or decoded? The 1.x question, kept for a manifest that cannot
    /// be read as one: matching its text is better than not looking.
    pub fn text_has_bad_package(&self, haystack: &str) -> bool {
        matches_or_decoded(&self.bad_packages, haystack)
    }

    /// The campaign packages a manifest or lockfile installs, read out of it
    /// by name. See [`ManifestMatch`].
    pub fn manifest_packages(&self, file_name: &str, text: &str) -> ManifestMatch {
        match manifest::package_names(file_name, text) {
            Ok(names) => {
                let mut bad: Vec<String> = names
                    .into_iter()
                    .filter(|n| self.bad_packages.contains(n))
                    .collect();
                bad.sort_unstable();
                bad.dedup();
                if bad.is_empty() {
                    ManifestMatch::Clean
                } else {
                    ManifestMatch::Names(bad)
                }
            }
            Err(why) => ManifestMatch::Unreadable {
                why,
                text_match: self.text_has_bad_package(text),
            },
        }
    }

    /// Does this file, by its path and its bytes, carry a confirmed
    /// indicator? A manifest is read for the names it installs; anything else
    /// is matched as text. For the GitHub check, which meets files by path.
    pub fn file_confirms(&self, path: &str, bytes: &[u8]) -> bool {
        let name = path.rsplit('/').next().unwrap_or(path);
        if !manifest::is_manifest(name) {
            return self.bytes_have_strong(bytes);
        }
        let text = String::from_utf8_lossy(bytes);
        self.has_strong_text(&text)
            || match self.manifest_packages(name, &text) {
                ManifestMatch::Names(_) => true,
                ManifestMatch::Clean => false,
                ManifestMatch::Unreadable { text_match, .. } => text_match,
            }
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

    /// A path worth a look, not proof. Never asked of a path that is already
    /// a confirmed filename.
    pub fn is_review_filename(&self, path: &str) -> bool {
        self.review_filenames.iter().any(|p| p.is_match(path))
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

/// What reading a manifest for campaign packages found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestMatch {
    /// It installs these campaign packages, by name.
    Names(Vec<String>),
    /// It was read and installs none of them.
    Clean,
    /// It could not be read as its format. `text_match` is whether its text
    /// names a campaign package anywhere, which is what 1.x asked.
    Unreadable { why: String, text_match: bool },
}

/// Could this character be part of a package name?
fn in_a_name(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')
}

/// Where `name` first stands in `text` as a whole package name.
fn whole_name_at(text: &str, name: &str) -> Option<usize> {
    if name.is_empty() {
        return None;
    }
    text.match_indices(name)
        .find(|(at, found)| {
            let before = text.get(..*at).unwrap_or_default();
            let after = text.get(at + found.len()..).unwrap_or_default();
            let ends = !after.chars().next().is_some_and(in_a_name);
            let starts = match before.chars().next_back() {
                None => true,
                Some(c) if in_a_name(c) || c == '@' => false,
                // `@other/name` is the scoped package `name` does not mean.
                Some('/') => !before
                    .trim_end_matches('/')
                    .trim_end_matches(in_a_name)
                    .ends_with('@'),
                Some(_) => true,
            };
            starts && ends
        })
        .map(|(at, _)| at)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Whether `haystack` contains any of `list`, as a fixed string.
fn any_in(list: &[String], haystack: &str) -> bool {
    list.iter().any(|i| haystack.contains(i.as_str()))
}

fn matches_or_decoded(list: &[String], haystack: &str) -> bool {
    any_in(list, haystack) || unescape::decode(haystack).is_some_and(|d| any_in(list, &d))
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
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
        assert!(matches!(
            Indicators::load(&bad),
            Err(LoadError::Pattern("filenames.txt", _))
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
        assert!(ind.has_strong("node_modules/evil-pkg/index.js"));
        assert!(ind.has_strong(r#"{"deps":{"evil-pkg":"1.0.0"}}"#));
        assert!(!ind.has_strong("entirely ordinary content"));
        // Part of a longer name, or the same name under somebody's scope, is
        // another package (ADR-0041).
        assert!(!ind.has_strong("npm i evil-pkg-utils"));
        assert!(!ind.has_strong("npm i not-evil-pkg"));
        assert!(!ind.has_strong("import x from '@other/evil-pkg'"));
        assert!(ind.text_has_bad_package("npm i evil-pkg-utils"));
    }

    #[test]
    fn a_manifest_is_read_for_the_names_it_installs() {
        let ind = Indicators {
            strong: vec!["MARKER-ALPHA".into()],
            bad_packages: vec!["evil-pkg".into(), "@bad/scope".into()],
            ..Indicators::default()
        };
        assert_eq!(
            ind.manifest_packages(
                "package.json",
                r#"{"dependencies":{"evil-pkg":"1","@bad/scope":"2","react":"18"}}"#
            ),
            ManifestMatch::Names(vec!["@bad/scope".into(), "evil-pkg".into()])
        );
        // The name inside a longer one, and in a description.
        let near = r#"{"description":"not evil-pkg","dependencies":{"evil-pkg-utils":"1"}}"#;
        assert_eq!(
            ind.manifest_packages("package.json", near),
            ManifestMatch::Clean
        );
        assert!(!ind.file_confirms("app/package.json", near.as_bytes()));
        // The same text in a file that is not a manifest is matched as text.
        assert!(ind.file_confirms("app/notes.txt", near.as_bytes()));
        // A manifest that cannot be read falls back to matching its text.
        let broken = r#"{"dependencies":{"evil-pkg-utils":"1""#;
        assert!(matches!(
            ind.manifest_packages("package.json", broken),
            ManifestMatch::Unreadable {
                text_match: true,
                ..
            }
        ));
        assert!(ind.file_confirms("package.json", broken.as_bytes()));
        // A strong indicator in a manifest is still one, as text.
        assert!(ind.file_confirms(
            "package.json",
            br#"{"scripts":{"postinstall":"MARKER-ALPHA"}}"#
        ));
    }

    #[test]
    fn the_first_indicator_is_found_by_offset_for_a_cut() {
        let ind = Indicators {
            strong: vec!["MARKER-ALPHA".into()],
            bad_packages: vec!["evil-pkg".into()],
            ..Indicators::default()
        };
        assert_eq!(ind.first_at(b"ab evil-pkg MARKER-ALPHA"), Some(3));
        assert_eq!(ind.first_at(b"evil-pkg-utils MARKER-ALPHA"), Some(15));
        assert_eq!(ind.first_at(b"nothing"), None);
    }

    /// Every character of `text` as a `\uXXXX` escape, the way the 0xa322
    /// loader family writes its strings.
    fn escaped(text: &str) -> String {
        text.chars()
            .map(|c| format!("\\u{:04x}", c as u32))
            .collect()
    }

    #[test]
    fn an_indicator_written_in_escape_codes_is_still_an_indicator() {
        // The 5 October finding: 18 of 35 confirmed repositories carried a
        // loader that grep -F against ioc/ finds nothing in.
        let ind = Indicators {
            strong: vec!["MARKER-ALPHA".into()],
            bad_packages: vec!["evil-pkg".into()],
            ..Indicators::default()
        };
        let loader = format!("var a='{}';", escaped("MARKER-ALPHA"));
        assert!(!loader.contains("MARKER-ALPHA"));
        assert!(ind.has_strong(&loader));
        assert!(ind.has_strong_only_escaped(&loader));
        assert!(ind.bytes_have_strong(loader.as_bytes()));
        // Half escaped, half not.
        assert!(ind.has_strong("MARKER-\\x41LPHA"));
        // As written is not "only escaped".
        assert!(!ind.has_strong_only_escaped("MARKER-ALPHA \\u0041"));
        assert!(ind.has_strong(&format!(
            r#"{{"dependencies":{{"{}":"1.0.0"}}}}"#,
            escaped("evil-pkg")
        )));
    }

    #[test]
    fn ordinary_escapes_are_not_an_indicator() {
        let ind = with(&[], &[]);
        let text = "const s = 'caf\\u00e9 na\\u00efve \\x41\\x42 \\u{1F600}';\n".repeat(50);
        assert!(!ind.has_strong(&text));
        assert!(!ind.bytes_have_strong(text.as_bytes()));
        assert!(!ind.has_strong_only_escaped(&text));
    }

    #[test]
    fn bytes_that_are_not_utf8_are_still_matched_both_ways() {
        let ind = with(&[], &[]);
        let mut bytes = vec![0xff, 0xfe];
        bytes.extend_from_slice(escaped("MARKER-ALPHA").as_bytes());
        assert!(ind.bytes_have_strong(&bytes));
        let mut raw = vec![0xff];
        raw.extend_from_slice(b"MARKER-ALPHA");
        assert!(ind.bytes_have_strong(&raw));
        assert!(!ind.bytes_have_strong(&[0xff, 0x00, b'x']));
    }
}
