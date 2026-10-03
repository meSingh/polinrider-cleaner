//! Checking repositories on GitHub: an organization's, or an account's.
//!
//! Read-only against GitHub. Every repository is mirror-cloned into an
//! evidence directory and every branch and tag of the mirror is checked.
//! Nothing is ever checked out, so the payload never exists as a live file on
//! the disk of the person looking for it, and no editor or task runner can
//! reach it.
//!
//! Like the host checks, this asks things of the outside world, and like them
//! it does so through one boundary: [`Forge`]. [`GitHub`] is the real thing,
//! through `gh` and `git`. [`Supplied`] reads the same answers from a
//! directory of bare repositories and text files, which is how the tests and
//! the conformance corpus drive it without a network or a login.
//!
//! This module and `host` are the only two that run a command.

use crate::checks::{is_build_config, payload_shaped_tail};
use crate::host::Probe;
use crate::indicators::Indicators;
use crate::pattern::Pattern;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Whose repositories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerKind {
    Organization,
    Account,
}

/// One push to a branch, as GitHub recorded it. The record is made by GitHub
/// and tied to the identity that pushed, not to whatever author a backdated
/// commit claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Push {
    pub git_ref: String,
    pub before: String,
    pub head: String,
    pub actor: String,
    pub at: String,
    /// Commits the push carried. Zero means the branch was moved without
    /// adding history: a force-push.
    pub size: u64,
}

/// Everything asked of GitHub.
pub trait Forge {
    /// One line for the report: where the answers came from.
    fn describe(&self) -> String;
    /// The account whose access is being used.
    fn signed_in_as(&self) -> Probe<String>;
    /// `owner/name` for every repository of the owner.
    fn repositories(&self, owner: &str, kind: OwnerKind) -> Probe<Vec<String>>;
    /// Mirror-clone `owner/name` into `dest`, which does not exist yet.
    fn mirror(&self, repository: &str, dest: &Path) -> Result<(), String>;
    /// The pushes GitHub still remembers for a repository. `Failed` is not
    /// "none": an empty feed and a failed call look the same downstream, and
    /// that difference decides whether a repository is believed untouched.
    fn pushes(&self, repository: &str) -> Probe<Vec<Push>>;
}

// --- running git and gh -------------------------------------------------------

enum Ran {
    Finished {
        ok: bool,
        stdout: Vec<u8>,
        stderr: String,
    },
    NoTool,
    Failed(String),
}

fn run(program: &str, args: &[&str], dir: Option<&Path>) -> Ran {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .env("LC_ALL", "C")
        // Never sit waiting on a credential prompt nobody can see.
        .env("GIT_TERMINAL_PROMPT", "0");
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    match command.output() {
        Ok(out) => Ran::Finished {
            ok: out.status.success(),
            stdout: out.stdout,
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ran::NoTool,
        Err(e) => Ran::Failed(format!("{program}: {e}")),
    }
}

fn first_line(program: &str, stderr: &str) -> String {
    match stderr.lines().map(str::trim).find(|l| !l.is_empty()) {
        Some(line) => format!("{program}: {line}"),
        None => format!("{program} failed without saying why"),
    }
}

/// Run a command for its text. Missing tool and failure stay different.
fn text(program: &str, args: &[&str], dir: Option<&Path>) -> Probe<String> {
    match run(program, args, dir) {
        Ran::Finished {
            ok: true, stdout, ..
        } => Probe::Read(String::from_utf8_lossy(&stdout).into_owned()),
        Ran::Finished { stderr, .. } => Probe::Failed(first_line(program, &stderr)),
        Ran::NoTool => Probe::NoTool(format!("{program} is not installed")),
        Ran::Failed(why) => Probe::Failed(why),
    }
}

fn lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `ref<TAB>before<TAB>head<TAB>actor<TAB>at<TAB>size` per line.
fn parse_pushes(text: &str) -> Vec<Push> {
    text.lines()
        .filter_map(|line| {
            let mut f = line.split('\t');
            Some(Push {
                git_ref: f.next()?.to_string(),
                before: f.next()?.to_string(),
                head: f.next()?.to_string(),
                actor: f.next()?.to_string(),
                at: f.next()?.to_string(),
                size: f.next()?.trim().parse().ok()?,
            })
        })
        .filter(|p| !p.git_ref.is_empty())
        .collect()
}

// --- GitHub itself ------------------------------------------------------------

/// GitHub, through the `gh` command the operator has signed in to.
#[derive(Debug, Default)]
pub struct GitHub;

impl Forge for GitHub {
    fn describe(&self) -> String {
        "GitHub, through the gh command".into()
    }

    fn signed_in_as(&self) -> Probe<String> {
        match text("gh", &["api", "user", "--jq", ".login"], None) {
            Probe::Read(login) if login.trim().is_empty() => {
                Probe::Failed("gh is not signed in. Run: gh auth login".into())
            }
            Probe::Read(login) => Probe::Read(login.trim().to_string()),
            Probe::Failed(_) => Probe::Failed("gh is not signed in. Run: gh auth login".into()),
            other => other,
        }
    }

    fn repositories(&self, owner: &str, _kind: OwnerKind) -> Probe<Vec<String>> {
        // gh does the JSON. This crate has no parser for it and does not need
        // one: --jq turns the answer into lines.
        match text(
            "gh",
            &[
                "repo",
                "list",
                owner,
                "--limit",
                "1000",
                "--json",
                "nameWithOwner",
                "--jq",
                ".[].nameWithOwner",
            ],
            None,
        ) {
            Probe::Read(out) => Probe::Read(lines(&out)),
            Probe::NoTool(why) => Probe::NoTool(why),
            Probe::Failed(why) => Probe::Failed(why),
        }
    }

    fn mirror(&self, repository: &str, dest: &Path) -> Result<(), String> {
        // The token never appears in an argument or in the mirror's config:
        // git asks gh for it. 1.x put it in the clone URL, where it stayed in
        // the mirror's config on disk and showed in the process list.
        let url = format!("https://github.com/{repository}.git");
        let dest = dest.display().to_string();
        match run(
            "git",
            &[
                "-c",
                "credential.helper=",
                "-c",
                "credential.helper=!gh auth git-credential",
                "clone",
                "--quiet",
                "--mirror",
                &url,
                &dest,
            ],
            None,
        ) {
            Ran::Finished { ok: true, .. } => Ok(()),
            Ran::Finished { stderr, .. } => Err(first_line("git", &stderr)),
            Ran::NoTool => Err("git is not installed".into()),
            Ran::Failed(why) => Err(why),
        }
    }

    fn pushes(&self, repository: &str) -> Probe<Vec<Push>> {
        let endpoint = format!("/repos/{repository}/events?per_page=100");
        match text(
            "gh",
            &[
                "api",
                &endpoint,
                "--paginate",
                "--jq",
                r#".[] | select(.type=="PushEvent") | [(.payload.ref // ""), (.payload.before // ""), (.payload.head // ""), (.actor.login // ""), (.created_at // ""), ((.payload.size // 0)|tostring)] | @tsv"#,
            ],
            None,
        ) {
            Probe::Read(out) => Probe::Read(parse_pushes(&out)),
            Probe::NoTool(why) => Probe::NoTool(why),
            Probe::Failed(why) => Probe::Failed(why),
        }
    }
}

// --- supplied -----------------------------------------------------------------

/// GitHub's answers held as files, for tests and the conformance corpus.
///
/// | In the directory | Holds |
/// |---|---|
/// | `whoami` | the login that is signed in |
/// | `repos/<owner>` | `owner/name`, one per line |
/// | `git/<owner>/<name>.git` | a bare repository to mirror from |
/// | `pushes/<owner>/<name>.tsv` | ref, before, head, actor, time, size, tab-separated |
///
/// As with supplied host state, a file that is absent is a question nobody
/// answered and is reported as that, never read as empty.
#[derive(Debug, Clone)]
pub struct Supplied {
    dir: PathBuf,
}

impl Supplied {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn read(&self, file: &Path, what: &str) -> Probe<String> {
        match fs::read(file) {
            Ok(bytes) => Probe::Read(String::from_utf8_lossy(&bytes).into_owned()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                Probe::Failed(format!("{what} was not supplied in {}", self.dir.display()))
            }
            Err(e) => Probe::Failed(format!("cannot read {}: {e}", file.display())),
        }
    }
}

impl Forge for Supplied {
    fn describe(&self) -> String {
        format!("supplied from {}, NOT read from GitHub", self.dir.display())
    }

    fn signed_in_as(&self) -> Probe<String> {
        match self.read(&self.dir.join("whoami"), "whoami") {
            Probe::Read(login) => Probe::Read(login.trim().to_string()),
            other => other,
        }
    }

    fn repositories(&self, owner: &str, _kind: OwnerKind) -> Probe<Vec<String>> {
        match self.read(&self.dir.join("repos").join(owner), "the repository list") {
            Probe::Read(out) => Probe::Read(lines(&out)),
            Probe::NoTool(why) => Probe::NoTool(why),
            Probe::Failed(why) => Probe::Failed(why),
        }
    }

    fn mirror(&self, repository: &str, dest: &Path) -> Result<(), String> {
        let source = self.dir.join("git").join(format!("{repository}.git"));
        if !source.is_dir() {
            return Err(format!("no repository at {}", source.display()));
        }
        let (source, dest) = (source.display().to_string(), dest.display().to_string());
        match run(
            "git",
            &["clone", "--quiet", "--mirror", &source, &dest],
            None,
        ) {
            Ran::Finished { ok: true, .. } => Ok(()),
            Ran::Finished { stderr, .. } => Err(first_line("git", &stderr)),
            Ran::NoTool => Err("git is not installed".into()),
            Ran::Failed(why) => Err(why),
        }
    }

    fn pushes(&self, repository: &str) -> Probe<Vec<Push>> {
        let file = self.dir.join("pushes").join(format!("{repository}.tsv"));
        match self.read(&file, "the push record") {
            Probe::Read(out) => Probe::Read(parse_pushes(&out)),
            Probe::NoTool(why) => Probe::NoTool(why),
            Probe::Failed(why) => Probe::Failed(why),
        }
    }
}

// --- the evidence directory ---------------------------------------------------

/// Why an evidence directory cannot be used.
#[derive(Debug)]
pub enum EvidenceError {
    /// Inside a git working tree. Mirrors hold live malware: inside a checkout
    /// an editor indexes them, and one `git add -A` republishes the payload
    /// from the operator's own account.
    InsideCheckout {
        dir: PathBuf,
        checkout: PathBuf,
    },
    Create {
        dir: PathBuf,
        source: io::Error,
    },
}

impl std::fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvidenceError::InsideCheckout { dir, checkout } => write!(
                f,
                "refusing to keep repository copies in {}: it is inside the git checkout at {}.\n\nThe copies hold live malware. Inside a checkout an editor indexes them, and\none 'git add -A' publishes the payload from your own account.",
                dir.display(),
                checkout.display()
            ),
            EvidenceError::Create { dir, source } => {
                write!(f, "cannot create {}: {source}", dir.display())
            }
        }
    }
}

/// Where mirrors go unless told otherwise: under the system's temporary
/// directory, which is outside every checkout and cleared on restart, so that
/// forgetting about infected mirrors is the safe outcome and not the
/// dangerous one.
pub fn default_evidence_dir() -> PathBuf {
    std::env::temp_dir().join("polinrider-evidence")
}

/// Check and create the evidence directory. Never inside a git checkout:
/// found by looking for `.git` up the tree, without running git.
pub fn prepare_evidence(dir: &Path) -> Result<PathBuf, EvidenceError> {
    let absolute = if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(dir)
    };
    if let Some(checkout) = absolute.ancestors().find(|a| a.join(".git").exists()) {
        return Err(EvidenceError::InsideCheckout {
            dir: absolute.clone(),
            checkout: checkout.to_path_buf(),
        });
    }
    fs::create_dir_all(&absolute).map_err(|source| EvidenceError::Create {
        dir: absolute.clone(),
        source,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&absolute, fs::Permissions::from_mode(0o700));
    }
    Ok(absolute)
}

// --- reading a mirror ---------------------------------------------------------

/// A bare mirror on disk, read through git plumbing. Nothing is checked out.
struct Mirror<'a> {
    dir: &'a Path,
}

impl Mirror<'_> {
    fn git(&self, args: &[&str]) -> Result<Vec<u8>, String> {
        match run("git", args, Some(self.dir)) {
            Ran::Finished {
                ok: true, stdout, ..
            } => Ok(stdout),
            Ran::Finished { stderr, .. } => Err(first_line("git", &stderr)),
            Ran::NoTool => Err("git is not installed".into()),
            Ran::Failed(why) => Err(why),
        }
    }

    /// Orphaned objects are the restore targets. They must survive.
    fn freeze(&self) {
        let _ = self.git(&["config", "gc.auto", "0"]);
        let _ = self.git(&["config", "gc.pruneExpire", "never"]);
    }

    fn refs(&self) -> Result<Vec<String>, String> {
        let out = self.git(&[
            "for-each-ref",
            "--format=%(refname)",
            "refs/heads/",
            "refs/tags/",
        ])?;
        Ok(lines(&String::from_utf8_lossy(&out)))
    }

    fn paths(&self, git_ref: &str) -> Result<Vec<String>, String> {
        let out = self.git(&["ls-tree", "-r", "--name-only", "-z", git_ref])?;
        Ok(nul_separated(&out))
    }

    fn blob(&self, git_ref: &str, path: &str) -> Option<Vec<u8>> {
        self.git(&["cat-file", "blob", &format!("{git_ref}:{path}")])
            .ok()
    }

    /// Paths in `git_ref` whose text contains any line of `patterns`, as a
    /// fixed string. Binary files are skipped, as the shell's `-I` does.
    fn containing(&self, git_ref: &str, patterns: &Path) -> Result<Vec<String>, String> {
        let file = patterns.display().to_string();
        match run(
            "git",
            &["grep", "-I", "-F", "-l", "-z", "-f", &file, git_ref],
            Some(self.dir),
        ) {
            Ran::Finished {
                ok: true, stdout, ..
            } => {
                let prefix = format!("{git_ref}:");
                Ok(nul_separated(&stdout)
                    .into_iter()
                    .map(|entry| {
                        entry
                            .strip_prefix(&prefix)
                            .map_or(entry.clone(), str::to_owned)
                    })
                    .collect())
            }
            // git grep exits 1 when nothing matched, which is an answer.
            Ran::Finished { stderr, .. } if stderr.trim().is_empty() => Ok(Vec::new()),
            Ran::Finished { stderr, .. } => Err(first_line("git", &stderr)),
            Ran::NoTool => Err("git is not installed".into()),
            Ran::Failed(why) => Err(why),
        }
    }
}

fn nul_separated(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|b| *b == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect()
}

// --- what a check finds -------------------------------------------------------

/// What one branch or tag of one repository holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefFinding {
    pub repository: String,
    pub git_ref: String,
    /// Files whose text contains a confirmed indicator.
    pub indicator_files: Vec<String>,
    /// Paths that are indicators by name.
    pub named_files: Vec<String>,
    /// Font files that are not fonts, with their first bytes.
    pub fake_fonts: Vec<(String, String)>,
    /// `tasks.json` files that run on folder open and carry no indicator.
    pub folder_open_tasks: Vec<String>,
    /// Files containing a weak signal.
    pub weak_files: Vec<String>,
    /// Build configs followed by a payload-shaped remainder.
    pub suspicious_configs: Vec<String>,
}

impl RefFinding {
    /// Every path that makes this a confirmed finding.
    pub fn confirmed_paths(&self) -> Vec<&str> {
        let mut paths: Vec<&str> = self
            .indicator_files
            .iter()
            .chain(&self.named_files)
            .map(String::as_str)
            .chain(self.fake_fonts.iter().map(|(path, _)| path.as_str()))
            .collect();
        paths.sort_unstable();
        paths.dedup();
        paths
    }

    /// The confirmed paths that are not the operator's own detection tooling.
    pub fn real_paths(&self) -> Vec<&str> {
        self.confirmed_paths()
            .into_iter()
            .filter(|p| !is_detection_tooling(p))
            .collect()
    }

    pub fn is_confirmed(&self) -> bool {
        !self.real_paths().is_empty()
    }

    pub fn needs_review(&self) -> bool {
        !self.is_confirmed()
            && (!self.weak_files.is_empty()
                || !self.suspicious_configs.is_empty()
                || !self.folder_open_tasks.is_empty())
    }

    /// The branch or tag name without `refs/heads/`.
    pub fn short_ref(&self) -> &str {
        self.git_ref
            .strip_prefix("refs/heads/")
            .or_else(|| self.git_ref.strip_prefix("refs/tags/"))
            .unwrap_or(&self.git_ref)
    }
}

/// Paths that contain indicator strings because they are detection tooling
/// and not payload: a scanner cannot tell "this file is the malware" from
/// "this file detects the malware", since both hold the same strings.
///
/// The list is 1.x's, less one entry. On the `v2` branch the shell's list had
/// grown `(lib|ci|src|conformance)/`, added to quiet this repository's own
/// self-scan. Applied to somebody else's repository it calls every infected
/// file under `src/` their own tooling. It is not carried here.
fn is_detection_tooling(path: &str) -> bool {
    const BENIGN: &[&str] = &[
        r"(^|/)\.github/workflows/[^/]*polinrider[^/]*\.(yml|yaml)$",
        r"(^|/)\.github/polinrider/",
        r"(^|/)(polinrider|scan-workspace|gh-scan|gh-sweep|gh-restore|triage-filter|check-macos|check-linux|check-windows|preflight|selftest|install-workflow|local-common|common)[^/]*\.(sh|ps1)$",
        r"(^|/)ioc/[^/]*\.txt$",
        r"\.md$",
        r"(^|/)docs/",
        "readme",
    ];
    let path = path.to_ascii_lowercase();
    BENIGN
        .iter()
        .filter_map(|p| Pattern::parse(p).ok())
        .any(|p| p.is_match(&path))
}

/// Everything a check of one owner found.
#[derive(Debug, Default)]
pub struct Findings {
    pub repositories: usize,
    pub refs: usize,
    /// Repositories that could not be mirrored. They were NOT checked, and a
    /// result that leaves them out covers less than it appears to.
    pub not_checked: Vec<(String, String)>,
    /// Repositories whose push record could not be read. Missing evidence,
    /// not an absence of pushes.
    pub no_push_record: Vec<String>,
    pub confirmed: Vec<RefFinding>,
    pub review: Vec<RefFinding>,
    /// Branches flagged only because of the operator's own detection files.
    pub own_tooling: usize,
    /// Pushes that landed on a confirmed branch, whoever made them.
    pub pushes: Vec<(String, Push)>,
}

impl Findings {
    /// Repositories with a confirmed finding, with how many of their branches
    /// carry it, in name order.
    pub fn affected(&self) -> Vec<(&str, usize)> {
        let mut out: Vec<(&str, usize)> = Vec::new();
        for finding in &self.confirmed {
            match out.iter_mut().find(|(r, _)| *r == finding.repository) {
                Some((_, n)) => *n += 1,
                None => out.push((&finding.repository, 1)),
            }
        }
        out.sort_unstable();
        out
    }

    /// Who pushed to a confirmed branch. Never filtered by whether the name
    /// is familiar: this campaign pushes as whoever is logged in, so a
    /// colleague's name is the expected case and not an exculpatory one.
    pub fn pushers(&self) -> Vec<&str> {
        let mut actors: Vec<&str> = self
            .pushes
            .iter()
            .map(|(_, push)| push.actor.as_str())
            .filter(|a| !a.is_empty())
            .collect();
        actors.sort_unstable();
        actors.dedup();
        actors
    }

    /// Affected repositories GitHub still has a push record for. Only these
    /// can be moved back to where they were before the attack.
    pub fn restorable(&self) -> Vec<&str> {
        let mut repos: Vec<&str> = self.pushes.iter().map(|(r, _)| r.as_str()).collect();
        repos.sort_unstable();
        repos.dedup();
        repos
    }
}

/// What to check, and with what.
pub struct Check<'a> {
    pub owner: &'a str,
    pub kind: OwnerKind,
    /// A prepared evidence directory: see [`prepare_evidence`].
    pub evidence: &'a Path,
    pub ind: &'a Indicators,
}

/// Mirror every repository of the owner and check every branch and tag.
/// `progress` is told the name of each repository as it is started.
pub fn check(
    forge: &dyn Forge,
    request: &Check,
    progress: &mut dyn FnMut(&str),
) -> Result<Findings, String> {
    let repositories = match forge.repositories(request.owner, request.kind) {
        Probe::Read(list) => list,
        Probe::NoTool(why) | Probe::Failed(why) => return Err(why),
    };

    // git grep reads its fixed strings from files.
    let strong = request.evidence.join("indicators-strong.txt");
    let weak = request.evidence.join("indicators-weak.txt");
    fs::write(&strong, request.ind.strong.join("\n") + "\n")
        .and_then(|()| fs::write(&weak, request.ind.weak.join("\n") + "\n"))
        .map_err(|e| format!("cannot write to {}: {e}", request.evidence.display()))?;

    let mut findings = Findings {
        repositories: repositories.len(),
        ..Findings::default()
    };

    for repository in &repositories {
        progress(repository);
        let dest = request
            .evidence
            .join(format!("{}.git", repository.replace('/', "__")));
        if !dest.is_dir() {
            if let Err(why) = forge.mirror(repository, &dest) {
                findings.not_checked.push((repository.clone(), why));
                continue;
            }
        }
        let mirror = Mirror { dir: &dest };
        mirror.freeze();

        let refs = match mirror.refs() {
            Ok(refs) => refs,
            Err(why) => {
                findings.not_checked.push((repository.clone(), why));
                continue;
            }
        };
        let mut confirmed_refs: Vec<String> = Vec::new();
        for git_ref in &refs {
            findings.refs += 1;
            let finding = check_ref(&mirror, repository, git_ref, request.ind, &strong, &weak)?;
            if finding.is_confirmed() {
                confirmed_refs.push(git_ref.clone());
                findings.confirmed.push(finding);
            } else if !finding.confirmed_paths().is_empty() {
                findings.own_tooling += 1;
            } else if finding.needs_review() {
                findings.review.push(finding);
            }
        }

        if !confirmed_refs.is_empty() {
            match forge.pushes(repository) {
                Probe::Read(pushes) => findings.pushes.extend(
                    pushes
                        .into_iter()
                        .filter(|p| confirmed_refs.contains(&p.git_ref))
                        .map(|p| (repository.clone(), p)),
                ),
                Probe::NoTool(_) | Probe::Failed(_) => {
                    findings.no_push_record.push(repository.clone());
                }
            }
        }
    }
    Ok(findings)
}

fn check_ref(
    mirror: &Mirror,
    repository: &str,
    git_ref: &str,
    ind: &Indicators,
    strong: &Path,
    weak: &Path,
) -> Result<RefFinding, String> {
    let mut finding = RefFinding {
        repository: repository.to_string(),
        git_ref: git_ref.to_string(),
        indicator_files: mirror.containing(git_ref, strong)?,
        weak_files: if ind.weak.is_empty() {
            Vec::new()
        } else {
            mirror.containing(git_ref, weak)?
        },
        ..RefFinding::default()
    };

    for path in mirror.paths(git_ref)? {
        let name = path.rsplit('/').next().unwrap_or(&path);
        if ind.is_bad_filename(&path) {
            finding.named_files.push(path.clone());
        }

        // tasks.json is a finding only when it runs on folder open, and it
        // is a confirmed one only when it also carries an indicator, in which
        // case the content match above already has it. The shell's remote
        // scan called any folder-open task INFECTED while its machine check
        // called the same file review; the machine check's answer is the one
        // the corpus argues for, and this follows it.
        if (path == ".vscode/tasks.json" || path.ends_with("/.vscode/tasks.json"))
            && !finding.indicator_files.contains(&path)
            && mirror
                .blob(git_ref, &path)
                .is_some_and(|b| String::from_utf8_lossy(&b).contains("folderOpen"))
        {
            finding.folder_open_tasks.push(path.clone());
        }

        let lower = name.to_ascii_lowercase();
        if (lower.ends_with(".woff") || lower.ends_with(".woff2"))
            && !name.starts_with("._")
            && !path.split('/').any(|part| part == "__MACOSX")
        {
            if let Some(bytes) = mirror.blob(git_ref, &path) {
                // An empty blob has nothing to inspect, and a Git LFS pointer
                // is a text stub standing in for the font.
                let magic = bytes.get(..4);
                let real = bytes.is_empty()
                    || bytes.starts_with(b"version https://git-lfs")
                    || matches!(magic, Some(b"wOFF" | b"wOF2") | None);
                if !real {
                    let hex = magic
                        .unwrap_or_default()
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    finding.fake_fonts.push((path.clone(), hex));
                }
            }
        }

        if is_build_config(name) && !finding.indicator_files.contains(&path) {
            if let Some(bytes) = mirror.blob(git_ref, &path) {
                if payload_shaped_tail(&String::from_utf8_lossy(&bytes)).is_some() {
                    finding.suspicious_configs.push(path.clone());
                }
            }
        }
    }
    Ok(finding)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    const STRONG: &str = "MARKER-ALPHA";

    fn ind() -> Indicators {
        Indicators {
            strong: vec![STRONG.into()],
            weak: vec!["weak-signal-string".into()],
            filenames: vec![Pattern::parse(r"(^|/)temp_helper\.bat$").expect("parses")],
            ..Indicators::default()
        }
    }

    fn git(dir: &Path, args: &[&str]) {
        let out = Command::new("git")
            .args(["-c", "user.name=test", "-c", "user.email=test@localhost"])
            .args([
                "-c",
                "init.defaultBranch=main",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// The files of one branch: (path, contents).
    type Files<'a> = &'a [(&'a str, &'a [u8])];

    /// A supplied forge with one owner, built from (repository, branch, files).
    struct World {
        dir: PathBuf,
    }

    impl World {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("prc-remote-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(dir.join("forge/repos")).expect("mkdir");
            fs::write(dir.join("forge/whoami"), "tester\n").expect("write");
            fs::write(dir.join("forge/repos/acme"), "").expect("write");
            Self { dir }
        }

        fn repo(&self, name: &str, branches: &[(&str, Files)]) {
            let work = self.dir.join("work").join(name);
            fs::create_dir_all(&work).expect("mkdir");
            git(&work, &["init", "-q", "-b", "main"]);
            fs::write(work.join("README.txt"), "base\n").expect("write");
            git(&work, &["add", "-A"]);
            git(&work, &["commit", "-q", "-m", "base"]);
            for (branch, files) in branches {
                if *branch != "main" {
                    git(&work, &["checkout", "-q", "-b", branch, "main"]);
                } else {
                    git(&work, &["checkout", "-q", "main"]);
                }
                for (path, body) in *files {
                    let file = work.join(path);
                    fs::create_dir_all(file.parent().expect("parent")).expect("mkdir");
                    fs::write(file, body).expect("write");
                }
                git(&work, &["add", "-A"]);
                git(&work, &["commit", "-q", "-m", "change"]);
            }
            let bare = self.dir.join("forge/git/acme");
            fs::create_dir_all(&bare).expect("mkdir");
            git(
                &self.dir,
                &[
                    "clone",
                    "-q",
                    "--bare",
                    &work.display().to_string(),
                    &bare.join(format!("{name}.git")).display().to_string(),
                ],
            );
            let list = self.dir.join("forge/repos/acme");
            let mut repos = fs::read_to_string(&list).expect("read");
            repos.push_str(&format!("acme/{name}\n"));
            fs::write(list, repos).expect("write");
        }

        fn pushes(&self, name: &str, tsv: &str) {
            let dir = self.dir.join("forge/pushes/acme");
            fs::create_dir_all(&dir).expect("mkdir");
            fs::write(dir.join(format!("{name}.tsv")), tsv).expect("write");
        }

        fn check(&self) -> Findings {
            let forge = Supplied::new(self.dir.join("forge"));
            let evidence = prepare_evidence(&self.dir.join("evidence")).expect("evidence");
            let ind = ind();
            check(
                &forge,
                &Check {
                    owner: "acme",
                    kind: OwnerKind::Organization,
                    evidence: &evidence,
                    ind: &ind,
                },
                &mut |_| {},
            )
            .expect("check runs")
        }
    }

    impl Drop for World {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn infected_config() -> Vec<u8> {
        format!(
            "export default {{}}\n{}var x='{STRONG}';\n",
            " ".repeat(280)
        )
        .into_bytes()
    }

    #[test]
    fn every_branch_is_checked_not_only_the_default_one() {
        // The payload may sit on a branch nobody looks at.
        let w = World::new("branches");
        w.repo(
            "shop",
            &[
                ("main", &[("src/index.js", b"export const a = 1\n")]),
                ("old-feature", &[("postcss.config.mjs", &infected_config())]),
            ],
        );
        w.repo("blog", &[("main", &[("index.md", b"hello\n")])]);
        let found = w.check();
        assert_eq!(found.repositories, 2);
        assert_eq!(found.refs, 3);
        assert_eq!(found.affected(), vec![("acme/shop", 1)]);
        assert_eq!(found.confirmed[0].short_ref(), "old-feature");
        assert_eq!(found.confirmed[0].real_paths(), vec!["postcss.config.mjs"]);
        assert!(found.review.is_empty());
        assert!(found.not_checked.is_empty());
    }

    #[test]
    fn a_fake_font_and_a_file_named_like_the_campaign_are_confirmed() {
        let w = World::new("shapes");
        w.repo(
            "site",
            &[(
                "main",
                &[
                    ("public/fonts/inter.woff2", b"var _0x=1;\n" as &[u8]),
                    ("public/fonts/real.woff2", b"wOF2 rest of a font"),
                    (
                        "public/fonts/lfs.woff2",
                        b"version https://git-lfs.github.com/spec/v1\n",
                    ),
                    ("scripts/temp_helper.bat", b"@echo off\n"),
                ],
            )],
        );
        let found = w.check();
        let finding = &found.confirmed[0];
        assert_eq!(
            finding.real_paths(),
            vec!["public/fonts/inter.woff2", "scripts/temp_helper.bat"]
        );
        assert_eq!(finding.fake_fonts[0].1, "76 61 72 20");
    }

    #[test]
    fn a_folder_open_task_without_an_indicator_is_review_not_confirmed() {
        let w = World::new("tasks");
        w.repo(
            "app",
            &[(
                "main",
                &[(
                    ".vscode/tasks.json",
                    b"{\"tasks\":[{\"runOptions\":{\"runOn\":\"folderOpen\"}}]}\n" as &[u8],
                )],
            )],
        );
        let found = w.check();
        assert!(found.confirmed.is_empty());
        assert_eq!(found.review.len(), 1);
        assert_eq!(
            found.review[0].folder_open_tasks,
            vec![".vscode/tasks.json"]
        );
    }

    #[test]
    fn the_operators_own_detection_files_are_not_the_payload() {
        // A scanner and its documentation hold the same strings the malware
        // does. A branch flagged only for those is counted and set aside.
        let w = World::new("tooling");
        w.repo(
            "infra",
            &[(
                "main",
                &[
                    (
                        ".github/workflows/polinrider-scan.yml",
                        format!("run: grep {STRONG}\n").as_bytes(),
                    ),
                    (
                        "docs/incident.md",
                        format!("we found {STRONG}\n").as_bytes(),
                    ),
                ],
            )],
        );
        // The same string in ordinary source is not tooling, whatever the
        // directory is called.
        w.repo(
            "api",
            &[(
                "main",
                &[("src/vendor.js", format!("var a='{STRONG}'\n").as_bytes())],
            )],
        );
        let found = w.check();
        assert_eq!(found.own_tooling, 1);
        assert_eq!(found.affected(), vec![("acme/api", 1)]);
        assert_eq!(found.confirmed[0].real_paths(), vec!["src/vendor.js"]);
    }

    #[test]
    fn who_pushed_is_read_for_confirmed_branches_only_and_never_filtered() {
        let w = World::new("pushes");
        w.repo(
            "shop",
            &[
                ("main", &[("a.js", b"ok\n" as &[u8])]),
                ("release", &[("vite.config.js", &infected_config())]),
            ],
        );
        w.pushes(
            "shop",
            "refs/heads/release\taaa\tbbb\talice\t2026-09-12T10:00:00Z\t0\n\
             refs/heads/release\tbbb\tccc\tbob\t2026-09-13T10:00:00Z\t1\n\
             refs/heads/main\tddd\teee\tcarol\t2026-09-14T10:00:00Z\t2\n",
        );
        let found = w.check();
        assert_eq!(found.pushers(), vec!["alice", "bob"]);
        assert_eq!(found.restorable(), vec!["acme/shop"]);
        assert_eq!(
            found.pushes[0].1.size, 0,
            "a push with no commits is a force-push"
        );
        assert!(found.no_push_record.is_empty());
    }

    #[test]
    fn a_missing_push_record_and_a_failed_clone_are_said_not_assumed() {
        let w = World::new("missing");
        w.repo(
            "shop",
            &[("main", &[("next.config.js", &infected_config())])],
        );
        // Listed, and not there to clone.
        let list = w.dir.join("forge/repos/acme");
        let mut repos = fs::read_to_string(&list).expect("read");
        repos.push_str("acme/ghost\n");
        fs::write(list, repos).expect("write");

        let found = w.check();
        assert_eq!(found.repositories, 2);
        assert_eq!(found.not_checked.len(), 1);
        assert_eq!(found.not_checked[0].0, "acme/ghost");
        assert_eq!(found.no_push_record, vec!["acme/shop"]);
        assert!(found.pushers().is_empty());
    }

    #[test]
    fn evidence_is_refused_inside_a_git_checkout() {
        let dir = std::env::temp_dir().join(format!("prc-evidence-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("checkout/.git")).expect("mkdir");
        assert!(matches!(
            prepare_evidence(&dir.join("checkout/tmp/evidence")),
            Err(EvidenceError::InsideCheckout { .. })
        ));
        assert!(prepare_evidence(&dir.join("elsewhere/evidence")).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn detection_tooling_is_recognised_by_path_and_src_is_not_tooling() {
        for tooling in [
            ".github/workflows/polinrider-scan.yml",
            ".github/polinrider/ioc/strong.txt",
            "ci/scan-workspace.sh",
            "ioc/strong.txt",
            "docs/adr/0001.md",
            "README",
            "notes/incident.MD",
        ] {
            assert!(is_detection_tooling(tooling), "{tooling}");
        }
        for payload in [
            "postcss.config.mjs",
            "src/vendor.js",
            "lib/util.js",
            "public/fonts/inter.woff2",
            ".github/workflows/deploy.yml",
        ] {
            assert!(!is_detection_tooling(payload), "{payload}");
        }
    }
}
