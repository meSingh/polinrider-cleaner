//! Fixing a repository on GitHub. Four ways, and each is worked out on the
//! copy before anything is pushed.
//!
//! | Fix | What it does on GitHub |
//! |---|---|
//! | restore | moves each branch back to where GitHub's own push record says it was |
//! | erase | rewrites the history without the payload files and pushes all of it |
//! | remove | adds one commit per branch that takes the payload out |
//! | archive | puts a notice on top of the README, then makes the repository read-only |
//!
//! Every fix comes as a pair. `plan_*` reads the copy and says exactly what
//! would change; it may fetch, and it never pushes. The function named after
//! the fix does it, and is only ever called with a plan the operator has seen
//! and answered `yes` to. Nothing is checked out at any point: commits are
//! built with git plumbing against an index file of their own, so the payload
//! never exists as a live file on the disk of the person removing it.
//!
//! A push is never taken at its word. After each one GitHub is asked where
//! the branches now point, and only what it shows is reported as done.
//!
//! What a fix moves away from is kept in the copy, under
//! `refs/polinrider/before-fix/`. See ADR-0037.

use crate::host_checks::sh_quote;
use crate::indicators::Indicators;
use crate::remote::{
    check_ref, indicator_files, is_object_id, mirror_path, Findings, Forge, Mirror, Push,
    RefFinding, Update,
};
use crate::strip;
use crate::verdict::clean;
use std::fs;
use std::path::{Path, PathBuf};

/// One repository that carries the payload, and what is known about it.
pub struct Repo<'a> {
    pub forge: &'a dyn Forge,
    /// `owner/name`.
    pub name: &'a str,
    pub evidence: &'a Path,
    pub ind: &'a Indicators,
    /// Its branches and tags that carry the payload.
    pub findings: Vec<&'a RefFinding>,
    /// The pushes GitHub remembers to those branches.
    pub pushes: Vec<&'a Push>,
    /// Who is signed in: the author of a commit when git has no name set.
    pub login: &'a str,
    /// `YYYYMMDDTHHMMSSZ` for this run, to name what is kept.
    pub stamp: &'a str,
}

impl<'a> Repo<'a> {
    /// One repository out of what a check found.
    pub fn of(
        forge: &'a dyn Forge,
        name: &'a str,
        evidence: &'a Path,
        ind: &'a Indicators,
        found: &'a Findings,
        login: &'a str,
        stamp: &'a str,
    ) -> Self {
        Self {
            forge,
            name,
            evidence,
            ind,
            findings: found
                .confirmed
                .iter()
                .filter(|f| f.repository == name)
                .collect(),
            pushes: found
                .pushes
                .iter()
                .filter(|(repository, _)| repository == name)
                .map(|(_, push)| push)
                .collect(),
            login,
            stamp,
        }
    }

    fn dir(&self) -> PathBuf {
        mirror_path(self.evidence, self.name)
    }

    /// Whether a branch, tag or commit of `mirror` carries the payload.
    fn carries_payload(&self, mirror: &Mirror, treeish: &str) -> Result<bool, String> {
        let (strong, weak) = indicator_files(self.evidence, self.ind)?;
        Ok(check_ref(mirror, self.name, treeish, self.ind, &strong, &weak)?.is_confirmed())
    }

    /// Every payload path at the tip of any affected branch or tag.
    fn tip_paths(&self) -> Vec<String> {
        let mut paths: Vec<String> = self
            .findings
            .iter()
            .flat_map(|f| f.real_paths())
            .map(str::to_owned)
            .collect();
        paths.sort_unstable();
        paths.dedup();
        paths
    }
}

/// What a fix did, as GitHub shows it afterwards.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Done {
    /// Branches and tags that now point where the fix put them.
    pub matched: Vec<String>,
    /// Branches and tags GitHub shows somewhere else.
    pub differ: Vec<String>,
    /// What git said, when it refused part of the push.
    pub refused: Option<String>,
}

/// A branch or tag name as it is said: without `refs/heads/`.
pub fn short(git_ref: &str) -> &str {
    git_ref
        .strip_prefix("refs/heads/")
        .or_else(|| git_ref.strip_prefix("refs/tags/"))
        .unwrap_or(git_ref)
}

/// Push, then ask GitHub what it now shows. `from` is the copy the new
/// commits are in.
fn push_and_check(
    repo: &Repo,
    from: &Path,
    updates: &[Update],
    force: bool,
) -> Result<Done, String> {
    let refused = repo.forge.push(from, updates, force).err();
    let shown = repo.forge.remote_refs(from).map_err(|why| match &refused {
        Some(refused) => refused.clone(),
        None => {
            format!("the push went through, and GitHub could not be asked to confirm it: {why}")
        }
    })?;
    let mut done = Done {
        refused,
        ..Done::default()
    };
    for update in updates {
        let now = shown
            .iter()
            .find(|(name, _)| *name == update.git_ref)
            .map(|(_, id)| id.as_str());
        if now == Some(update.to.as_str()) {
            done.matched.push(update.git_ref.clone());
        } else {
            done.differ.push(update.git_ref.clone());
        }
    }
    if done.matched.is_empty() {
        return Err(done
            .refused
            .unwrap_or_else(|| "GitHub does not show the change".into()));
    }
    Ok(done)
}

/// Make the copy agree with GitHub for everything that moved, keeping what
/// it moved away from. `from` is another copy holding the new commits, when
/// they were not made in this one.
fn settle(repo: &Repo, mirror: &Mirror, from: Option<&Path>, updates: &[Update], done: &Done) {
    for update in updates {
        if !done.matched.contains(&update.git_ref) {
            continue;
        }
        let kept = format!(
            "refs/polinrider/before-fix/{}/{}",
            repo.stamp,
            update
                .git_ref
                .strip_prefix("refs/")
                .unwrap_or(&update.git_ref)
        );
        let _ = mirror.git(&["update-ref", &kept, &update.expect]);
        match from {
            Some(from) => {
                let _ = mirror.git(&[
                    "fetch",
                    "--quiet",
                    &from.display().to_string(),
                    &format!("+{0}:{0}", update.git_ref),
                ]);
            }
            None => {
                let _ = mirror.git(&["update-ref", &update.git_ref, &update.to]);
            }
        }
    }
}

// --- restore ------------------------------------------------------------------

/// Where one branch can be put back to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Back {
    /// A commit GitHub recorded the branch at, fetched and checked clean.
    To {
        commit: String,
        /// The earliest push this undoes.
        undoes: Push,
        /// Commits pushed since, which stop being reachable.
        dropped: usize,
    },
    /// A tag. GitHub keeps a push record for branches only.
    Tag,
    /// GitHub remembers no push to this branch.
    NoRecord,
    /// Every earlier state GitHub remembers carries the payload or is gone.
    NothingClean,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreLine {
    pub git_ref: String,
    pub tip: String,
    pub back: Back,
}

/// What a restore would do, branch by branch.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Restore {
    pub lines: Vec<RestoreLine>,
}

impl Restore {
    /// How many branches can be put back.
    pub fn possible(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| matches!(l.back, Back::To { .. }))
            .count()
    }
}

/// Find, for each affected branch, the newest state GitHub recorded that is
/// clean.
///
/// The record is GitHub's own: each push with the commit the branch pointed
/// to before it. It is walked backwards from the newest push, and the first
/// `before` that can still be fetched and checks clean is the target. Commit
/// dates are never consulted, because this campaign forges them. A `before`
/// that carries the payload is an earlier wave of the same attack, and
/// restoring to it would put the payload straight back.
pub fn plan_restore(repo: &Repo) -> Result<Restore, String> {
    let dir = repo.dir();
    let mirror = Mirror { dir: &dir };
    let mut plan = Restore::default();
    for finding in &repo.findings {
        let Some(tip) = mirror.id_of(&finding.git_ref) else {
            continue;
        };
        let mut line = RestoreLine {
            git_ref: finding.git_ref.clone(),
            tip,
            back: Back::NoRecord,
        };
        if !finding.git_ref.starts_with("refs/heads/") {
            line.back = Back::Tag;
            plan.lines.push(line);
            continue;
        }
        let mut pushes: Vec<&Push> = repo
            .pushes
            .iter()
            .filter(|p| p.git_ref == finding.git_ref)
            .copied()
            .collect();
        pushes.sort_by(|a, b| a.at.cmp(&b.at));
        if !pushes.is_empty() {
            line.back = Back::NothingClean;
        }
        for push in pushes.iter().rev() {
            let commit = push.before.as_str();
            if !is_object_id(commit) || commit == line.tip {
                continue;
            }
            if !mirror.has_commit(commit) {
                // A commit the branch was moved off is reachable from
                // nothing, so the copy never had it. GitHub still serves it
                // by ID until it collects its garbage.
                let _ = repo.forge.fetch_commit(&dir, commit);
                if !mirror.has_commit(commit) {
                    continue;
                }
            }
            // Named, so that git here never prunes the thing just fetched.
            let _ = mirror.git(&[
                "update-ref",
                &format!("refs/polinrider/pre-attack/{commit}"),
                commit,
            ]);
            if repo.carries_payload(&mirror, commit)? {
                continue;
            }
            line.back = Back::To {
                commit: commit.to_string(),
                undoes: (*push).clone(),
                dropped: mirror.count(&[&format!("{commit}..{}", line.tip)])?,
            };
            break;
        }
        plan.lines.push(line);
    }
    Ok(plan)
}

/// Move each branch of the plan back. Nothing is edited: only where the
/// branch points changes.
pub fn restore(repo: &Repo, plan: &Restore) -> Result<Done, String> {
    let updates: Vec<Update> = plan
        .lines
        .iter()
        .filter_map(|line| match &line.back {
            Back::To { commit, .. } => Some(Update {
                git_ref: line.git_ref.clone(),
                expect: line.tip.clone(),
                to: commit.clone(),
            }),
            _ => None,
        })
        .collect();
    if updates.is_empty() {
        return Err("no branch of this repository can be put back".into());
    }
    let dir = repo.dir();
    let done = push_and_check(repo, &dir, &updates, true)?;
    settle(repo, &Mirror { dir: &dir }, None, &updates, &done);
    Ok(done)
}

// --- building a commit without a checkout -------------------------------------

enum Change<'a> {
    Delete(&'a str),
    Write(&'a str, Vec<u8>),
}

/// The author to give a commit when git has none configured: the account
/// signed in, at the address GitHub provides for exactly this.
fn identity(mirror: &Mirror, login: &str) -> Vec<(&'static str, String)> {
    let set = |key: &str| {
        mirror
            .word(&["config", "--get", key])
            .is_ok_and(|v| !v.is_empty())
    };
    if set("user.name") && set("user.email") {
        return Vec::new();
    }
    let email = format!("{login}@users.noreply.github.com");
    vec![
        ("GIT_AUTHOR_NAME", login.to_string()),
        ("GIT_AUTHOR_EMAIL", email.clone()),
        ("GIT_COMMITTER_NAME", login.to_string()),
        ("GIT_COMMITTER_EMAIL", email),
    ]
}

/// One new commit on top of `parent` with `changes` applied. Built in an
/// index file of its own; no file is ever written out.
fn commit(
    repo: &Repo,
    mirror: &Mirror,
    parent: &str,
    changes: &[Change],
    message: &[&str],
) -> Result<String, String> {
    let index = mirror.dir.join("polinrider-index");
    let _ = fs::remove_file(&index);
    let index_path = index.display().to_string();
    let env = [("GIT_INDEX_FILE", index_path.as_str())];
    let built = (|| {
        mirror.git_with(&["read-tree", parent], &env, None)?;
        // "<mode> <id>\t<path>" per entry, and mode 0 takes the path out.
        // Handed over on input, which works in a copy with no checkout and
        // carries any file name whole.
        let mut entries: Vec<u8> = Vec::new();
        for change in changes {
            let (mode, id, path) = match change {
                Change::Delete(path) => ("0".to_string(), "0".repeat(parent.len()), *path),
                Change::Write(path, bytes) => {
                    let id =
                        mirror.git_with(&["hash-object", "-w", "--stdin"], &[], Some(bytes))?;
                    // Keep the mode the file had. A new file is an ordinary one.
                    let listed = mirror.word(&["ls-tree", parent, "--", path])?;
                    let mode = listed
                        .split_whitespace()
                        .next()
                        .filter(|m| m.starts_with("100"))
                        .unwrap_or("100644")
                        .to_string();
                    (mode, String::from_utf8_lossy(&id).trim().to_string(), *path)
                }
            };
            entries.extend(format!("{mode} {id}\t{path}").into_bytes());
            entries.push(0);
        }
        mirror.git_with(
            &["update-index", "-z", "--index-info"],
            &env,
            Some(&entries),
        )?;
        let tree = mirror.git_with(&["write-tree"], &env, None)?;
        Ok::<String, String>(String::from_utf8_lossy(&tree).trim().to_string())
    })();
    let _ = fs::remove_file(&index);
    let tree = built?;

    let who = identity(mirror, repo.login);
    let who: Vec<(&str, &str)> = who.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let mut args = vec!["commit-tree", tree.as_str(), "-p", parent];
    for paragraph in message {
        args.extend(["-m", paragraph]);
    }
    let id = mirror.git_with(&args, &who, None)?;
    let id = String::from_utf8_lossy(&id).trim().to_string();
    if is_object_id(&id) {
        Ok(id)
    } else {
        Err("git commit-tree did not answer with a commit".into())
    }
}

// --- remove -------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveLine {
    pub git_ref: String,
    pub tip: String,
    /// Build configs the payload is cut out of. The file stays.
    pub strip: Vec<String>,
    /// Files that are the payload, deleted whole.
    pub delete: Vec<String>,
}

/// What a remove would do, branch by branch.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Remove {
    pub lines: Vec<RemoveLine>,
    /// Tags that carry the payload. A commit cannot be added to a tag.
    pub tags: Vec<String>,
}

/// The cleaned bytes of a build config, when the payload is the one shape
/// that is cut out and not deleted.
fn stripped(mirror: &Mirror, treeish: &str, path: &str, ind: &Indicators) -> Option<Vec<u8>> {
    let bytes = mirror.blob(treeish, path)?;
    match strip::plan(&bytes, ind) {
        Some(Ok(plan)) => Some(plan.keep),
        _ => None,
    }
}

pub fn plan_remove(repo: &Repo) -> Result<Remove, String> {
    let dir = repo.dir();
    let mirror = Mirror { dir: &dir };
    let mut plan = Remove::default();
    for finding in &repo.findings {
        if !finding.git_ref.starts_with("refs/heads/") {
            plan.tags.push(finding.git_ref.clone());
            continue;
        }
        let Some(tip) = mirror.id_of(&finding.git_ref) else {
            continue;
        };
        let (strip, delete) = finding
            .real_paths()
            .into_iter()
            .map(str::to_owned)
            .partition(|path| stripped(&mirror, &tip, path, repo.ind).is_some());
        plan.lines.push(RemoveLine {
            git_ref: finding.git_ref.clone(),
            tip,
            strip,
            delete,
        });
    }
    Ok(plan)
}

/// Add one commit to each branch of the plan and push it. An ordinary push:
/// nothing is rewritten, and the commit can be reverted.
pub fn remove(repo: &Repo, plan: &Remove) -> Result<Done, String> {
    let dir = repo.dir();
    let mirror = Mirror { dir: &dir };
    let mut updates: Vec<Update> = Vec::new();
    for line in &plan.lines {
        let mut changes: Vec<Change> = Vec::new();
        for path in &line.strip {
            match stripped(&mirror, &line.tip, path, repo.ind) {
                Some(keep) => changes.push(Change::Write(path, keep)),
                None => changes.push(Change::Delete(path)),
            }
        }
        changes.extend(line.delete.iter().map(|path| Change::Delete(path)));
        let new = commit(
            repo,
            &mirror,
            &line.tip,
            &changes,
            &[
                "Remove PolinRider payload",
                "Takes out the files and the appended code that carry indicators of the PolinRider supply-chain compromise. Found with polinrider-cleaner. No history is rewritten: older commits still hold the payload. Change every credential that was on a computer with a clone of this repository.",
            ],
        )?;
        // Checked before it goes anywhere: a commit that leaves the payload
        // in place is not a fix, and is not pushed as one.
        if repo.carries_payload(&mirror, &new)? {
            return Err(format!(
                "the payload would still be in {} afterwards. Nothing was pushed.",
                short(&line.git_ref)
            ));
        }
        updates.push(Update {
            git_ref: line.git_ref.clone(),
            expect: line.tip.clone(),
            to: new,
        });
    }
    if updates.is_empty() {
        return Err("no branch of this repository can take a commit".into());
    }
    let done = push_and_check(repo, &dir, &updates, false)?;
    settle(repo, &mirror, None, &updates, &done);
    Ok(done)
}

// --- erase --------------------------------------------------------------------

/// What an erase would do.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Erase {
    /// Every path the payload was ever committed under, in any commit.
    pub paths: Vec<String>,
    /// Commits in the history. Every one gets a new ID.
    pub commits: usize,
    /// Commits that touch those paths.
    pub touched: usize,
    pub branches: Vec<String>,
    pub tags: Vec<String>,
    /// Build configs whose clean part is put back on a branch afterwards,
    /// as one new commit: (branch, paths).
    pub put_back: Vec<(String, Vec<String>)>,
}

pub fn plan_erase(repo: &Repo) -> Result<Erase, String> {
    let dir = repo.dir();
    let mirror = Mirror { dir: &dir };
    let mut paths = mirror.payload_paths_in_history(repo.ind)?;
    paths.extend(repo.tip_paths());
    paths.sort_unstable();
    paths.dedup();
    if paths.is_empty() {
        return Err("found no payload file in the history of this repository".into());
    }

    let mut touched = vec!["--branches", "--tags", "--"];
    touched.extend(paths.iter().map(String::as_str));
    let mut plan = Erase {
        commits: mirror.count(&["--branches", "--tags"])?,
        touched: mirror.count(&touched)?,
        ..Erase::default()
    };
    for (name, _) in mirror.tips()? {
        if name.starts_with("refs/heads/") {
            plan.branches.push(name);
        } else {
            plan.tags.push(name);
        }
    }
    for finding in &repo.findings {
        if !finding.git_ref.starts_with("refs/heads/") {
            continue;
        }
        let keep: Vec<String> = finding
            .real_paths()
            .into_iter()
            .filter(|path| stripped(&mirror, &finding.git_ref, path, repo.ind).is_some())
            .map(str::to_owned)
            .collect();
        if !keep.is_empty() {
            plan.put_back.push((finding.git_ref.clone(), keep));
        }
    }
    plan.paths = paths;
    Ok(plan)
}

/// Rewrite the history without the payload files and push every branch and
/// tag. Done in a second copy, so that a rewrite that fails or does not
/// check out clean leaves nothing behind and pushes nothing.
pub fn erase(repo: &Repo, plan: &Erase) -> Result<Done, String> {
    let dir = repo.dir();
    let mirror = Mirror { dir: &dir };
    let scratch = repo
        .evidence
        .join("rewrite")
        .join(format!("{}.git", repo.name.replace('/', "__")));
    let _ = fs::remove_dir_all(&scratch);
    if let Some(parent) = scratch.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    let result = erase_in(repo, plan, &mirror, &scratch);
    let _ = fs::remove_dir_all(&scratch);
    result
}

fn erase_in(repo: &Repo, plan: &Erase, mirror: &Mirror, scratch: &Path) -> Result<Done, String> {
    let before = mirror.tips()?;
    let origin = mirror.word(&["config", "--get", "remote.origin.url"])?;
    mirror.git(&[
        "clone",
        "--quiet",
        "--mirror",
        ".",
        &scratch.display().to_string(),
    ])?;
    let work = Mirror { dir: scratch };
    work.git(&["config", "remote.origin.url", &origin])?;
    work.git(&["config", "gc.auto", "0"])?;

    // git filter-branch ships with git, so nobody is asked to install
    // anything. The filter is a shell command, hence the quoting. Commits
    // left empty are kept: dropping them can delete a branch outright.
    let filter = format!(
        "git rm --cached --ignore-unmatch -r -q -- {}",
        plan.paths
            .iter()
            .map(|p| sh_quote(p))
            .collect::<Vec<_>>()
            .join(" ")
    );
    work.git_with(
        &[
            "filter-branch",
            "-f",
            "--index-filter",
            &filter,
            "--tag-name-filter",
            "cat",
            "--",
            "--branches",
            "--tags",
        ],
        &[("FILTER_BRANCH_SQUELCH_WARNING", "1")],
        None,
    )
    .map_err(|why| format!("the rewrite failed, and nothing was pushed: {why}"))?;

    // The clean part of each build config goes back, as one commit.
    for (git_ref, paths) in &plan.put_back {
        let Some(tip) = work.id_of(git_ref) else {
            continue;
        };
        let changes: Vec<Change> = paths
            .iter()
            .filter_map(|path| {
                stripped(mirror, git_ref, path, repo.ind).map(|keep| Change::Write(path, keep))
            })
            .collect();
        if changes.is_empty() {
            continue;
        }
        let new = commit(
            repo,
            &work,
            &tip,
            &changes,
            &[
                "Put back build configuration without the PolinRider payload",
                "The history of this repository was rewritten to take the PolinRider payload out of every commit, which removed these files. This commit puts back what they held before the payload was appended.",
            ],
        )?;
        work.git(&["update-ref", git_ref, &new])?;
    }

    // Not trusted until looked at: every file version in the new history.
    let left = work.payload_paths_in_history(repo.ind)?;
    if let Some(path) = left.first() {
        return Err(format!(
            "the payload is still in the rewritten history, in {}. Nothing was pushed.",
            clean(path)
        ));
    }

    let mut updates: Vec<Update> = Vec::new();
    for (git_ref, old) in &before {
        let Some(new) = work.id_of(git_ref) else {
            return Err(format!(
                "the rewrite lost {}. Nothing was pushed.",
                short(git_ref)
            ));
        };
        if new != *old {
            updates.push(Update {
                git_ref: git_ref.clone(),
                expect: old.clone(),
                to: new,
            });
        }
    }
    if updates.is_empty() {
        return Err("the rewrite changed nothing. Nothing was pushed.".into());
    }
    let done = push_and_check(repo, scratch, &updates, true)?;
    settle(repo, mirror, Some(scratch), &updates, &done);
    Ok(done)
}

// --- archive ------------------------------------------------------------------

/// What an archived repository's description becomes.
pub const ARCHIVED_DESCRIPTION: &str = "INFECTED with PolinRider malware. Do not clone or use.";

/// The first line of the notice. Also how a notice already there is known.
const NOTICE_HEADING: &str =
    "# INFECTED WITH MALWARE. DO NOT CLONE, OPEN OR BUILD THIS REPOSITORY.";

/// The facts the notice states.
pub struct NoticeFacts<'a> {
    /// The organization or account that owns the repository.
    pub owner: &'a str,
    pub is_organization: bool,
    /// Today, as `YYYYMMDD...`.
    pub found: &'a str,
}

/// What an archive would do.
#[derive(Debug, PartialEq, Eq)]
pub struct Archive {
    /// The branch the repository opens on, and where it points.
    pub branch: String,
    pub tip: String,
    /// The file the notice goes on top of.
    pub readme: String,
    /// False when the repository has no README yet and gets one.
    pub readme_exists: bool,
    /// True when a notice from an earlier run is already there.
    pub already_noticed: bool,
    pub notice: String,
}

/// `2026-09-11T09:14:00Z` or `20260911T...` as `11 September 2026`.
pub fn long_date(stamp: &str) -> Option<String> {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let digits: String = stamp
        .chars()
        .take_while(|c| *c != 'T')
        .filter(char::is_ascii_digit)
        .collect();
    if digits.len() != 8 {
        return None;
    }
    let year = digits.get(..4)?;
    let month: usize = digits.get(4..6)?.parse().ok()?;
    let day: usize = digits.get(6..8)?.parse().ok()?;
    let month = MONTHS.get(month.checked_sub(1)?)?;
    (1..=31)
        .contains(&day)
        .then(|| format!("{day} {month} {year}"))
}

/// The notice, as it is written into the README: a heading first, so that it
/// is the largest thing on the page, then the warning in bold.
fn notice(repo: &Repo, facts: &NoticeFacts) -> String {
    let paths = repo.tip_paths();
    // A file name goes inside backticks, so it cannot hold one.
    let tidy = |path: &str| clean(path).replace('`', "'");
    let files = match paths.as_slice() {
        [] => "its files".to_string(),
        [one] => format!("`{}`", tidy(one)),
        [first, rest @ ..] => format!(
            "`{}` and {} other file{}",
            tidy(first),
            rest.len(),
            if rest.len() == 1 { "" } else { "s" }
        ),
    };
    let branches = repo.findings.len();
    let found = long_date(facts.found).unwrap_or_else(|| "the day this notice was added".into());
    let since = repo
        .pushes
        .iter()
        .map(|p| p.at.as_str())
        .min()
        .and_then(long_date);
    let owner = clean(facts.owner);

    let mut body = vec![
        "**This repository is infected with malware and has been archived. Do not clone it, open it in an editor, or build it.**".to_string(),
        String::new(),
        format!(
            "It carries the PolinRider supply-chain payload. It was found on {found} in {files}, on {branches} branch{}. Opening the folder in an editor or running a build can run the malware.",
            if branches == 1 { "" } else { "es" }
        ),
        String::new(),
    ];
    // Left out when GitHub has no record of when: a guessed date here would
    // tell people who are affected that they are not.
    let who = match &since {
        Some(since) => {
            format!("**If you cloned, opened or built this repository on or after {since}**")
        }
        None => "**If you ever cloned, opened or built this repository**".to_string(),
    };
    body.push(format!(
        "{who}, treat that computer as compromised: check it with [polinrider-cleaner](https://github.com/meSingh/polinrider-cleaner), and change your GitHub tokens, SSH keys, npm tokens and cloud keys from a different computer."
    ));
    body.push(String::new());
    body.push(format!(
        "The payload has not been removed. This repository is kept read-only as a record and must not be used. If you need this code, ask {} for a clean copy.",
        if facts.is_organization {
            format!("the owners of the {owner} organization")
        } else {
            format!("its owner, {owner},")
        }
    ));

    let mut out = format!("{NOTICE_HEADING}\n\n> [!CAUTION]\n");
    for paragraph in body {
        if paragraph.is_empty() {
            out.push_str(">\n");
        }
        for line in wrap(&paragraph, 70) {
            out.push_str("> ");
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

/// `text` broken at spaces into lines of at most `width` characters. A word
/// longer than that, such as a link, gets a line to itself.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

pub fn plan_archive(repo: &Repo, facts: &NoticeFacts) -> Result<Archive, String> {
    let dir = repo.dir();
    let mirror = Mirror { dir: &dir };
    let branch = mirror
        .default_branch()
        .ok_or("this repository has no default branch to put the notice on")?;
    let tip = mirror
        .id_of(&branch)
        .ok_or("the default branch points at nothing")?;
    let top = mirror.git(&["ls-tree", "--name-only", "-z", &tip])?;
    let readme = top
        .split(|b| *b == 0)
        .map(|name| String::from_utf8_lossy(name).into_owned())
        .find(|name| name.eq_ignore_ascii_case("README.md"));
    let already_noticed = readme
        .as_deref()
        .and_then(|path| mirror.blob(&tip, path))
        .is_some_and(|bytes| bytes.starts_with(NOTICE_HEADING.as_bytes()));
    Ok(Archive {
        branch,
        tip,
        readme_exists: readme.is_some(),
        readme: readme.unwrap_or_else(|| "README.md".into()),
        already_noticed,
        notice: notice(repo, facts),
    })
}

/// Put the notice on top of the README, replace the description, and make
/// the repository read-only, in that order: an archived repository takes no
/// push and no edit.
pub fn archive(repo: &Repo, plan: &Archive) -> Result<(), String> {
    let dir = repo.dir();
    let mirror = Mirror { dir: &dir };
    if !plan.already_noticed {
        // On top of what is there. Nothing that was in the file is removed.
        let mut text = plan.notice.clone().into_bytes();
        if plan.readme_exists {
            text.extend_from_slice(b"\n---\n\n");
            text.extend(mirror.blob(&plan.tip, &plan.readme).unwrap_or_default());
        }
        let new = commit(
            repo,
            &mirror,
            &plan.tip,
            &[Change::Write(&plan.readme, text)],
            &[
                "Mark this repository as infected with PolinRider",
                "Adds a notice on top of the README. The payload has NOT been removed: this repository is being archived as a record, and must not be cloned, opened or built.",
            ],
        )?;
        let updates = [Update {
            git_ref: plan.branch.clone(),
            expect: plan.tip.clone(),
            to: new,
        }];
        let done = push_and_check(repo, &dir, &updates, false).map_err(|why| {
            format!("the notice could not be pushed, and nothing else was changed: {why}")
        })?;
        settle(repo, &mirror, None, &updates, &done);
    }
    repo.forge
        .set_description(repo.name, ARCHIVED_DESCRIPTION)
        .map_err(|why| {
            format!("the notice is on the README. The description could not be changed, and the repository is NOT archived: {why}")
        })?;
    repo.forge.archive(repo.name).map_err(|why| {
        format!("the notice and the description are in place. The repository could NOT be archived: {why}")
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::pattern::Pattern;
    use crate::remote::fixture::{git, git_out, World};
    use crate::remote::{check, prepare_evidence, Check, OwnerKind, Supplied};

    const STRONG: &str = "MARKER-ALPHA";
    const STAMP: &str = "20261003T120000Z";
    const CLEAN_CONFIG: &str = "export default { plugins: {} }\n";

    fn ind() -> Indicators {
        Indicators {
            strong: vec![STRONG.into()],
            filenames: vec![Pattern::parse(r"(^|/)temp_helper\.bat$").expect("parses")],
            ..Indicators::default()
        }
    }

    /// The notice with its line breaks and quote marks folded away.
    fn flat(notice: &str) -> String {
        notice
            .lines()
            .map(|l| l.trim_start_matches('>').trim())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn infected_config() -> Vec<u8> {
        format!("{CLEAN_CONFIG}{}var x='{STRONG}';\n", " ".repeat(280)).into_bytes()
    }

    /// A pretend GitHub, checked, with what the fixes need to hand.
    struct Checked {
        w: World,
        forge: Supplied,
        evidence: PathBuf,
        ind: Indicators,
        found: Findings,
    }

    impl Checked {
        fn new(w: World) -> Self {
            let forge = w.forge();
            let evidence = prepare_evidence(&w.dir.join("evidence")).expect("evidence");
            let ind = ind();
            let found = Self::check(&forge, &evidence, &ind);
            Self {
                w,
                forge,
                evidence,
                ind,
                found,
            }
        }

        fn check(forge: &Supplied, evidence: &Path, ind: &Indicators) -> Findings {
            check(
                forge,
                &Check {
                    owner: "acme",
                    kind: OwnerKind::Organization,
                    evidence,
                    ind,
                },
                &mut |_| {},
            )
            .expect("check runs")
        }

        fn again(&self) -> Findings {
            Self::check(&self.forge, &self.evidence, &self.ind)
        }

        fn repo<'a>(&'a self, name: &'a str) -> Repo<'a> {
            Repo::of(
                &self.forge,
                name,
                &self.evidence,
                &self.ind,
                &self.found,
                "tester",
                STAMP,
            )
        }
    }

    /// shop: main was clean, then force-pushed with the payload by alice,
    /// then pushed to again by bob from an infected clone.
    fn attacked() -> (Checked, String) {
        let w = World::new(&format!(
            "fix-{}",
            std::thread::current()
                .name()
                .unwrap_or("t")
                .replace("::", "-")
        ));
        w.repo(
            "shop",
            &[
                ("legacy", &[("vite.config.js", &infected_config())]),
                ("main", &[("postcss.config.mjs", CLEAN_CONFIG.as_bytes())]),
            ],
        );
        let (clean, _) = w.push(
            "shop",
            "main",
            &[("postcss.config.mjs", &infected_config())],
            true,
            Some(("alice", "2026-09-11T09:14:00Z")),
        );
        w.push(
            "shop",
            "main",
            &[("public/fonts/inter.woff2", b"var _0x=1;\n")],
            false,
            Some(("bob", "2026-09-12T16:40:00Z")),
        );
        (Checked::new(w), clean)
    }

    #[test]
    fn restore_goes_back_to_the_newest_clean_state_github_recorded() {
        let (c, clean) = attacked();
        assert_eq!(c.found.affected(), vec![("acme/shop", 2)]);
        let repo = c.repo("acme/shop");
        let plan = plan_restore(&repo).expect("plans");

        // legacy has no push on record. main has two: the one before bob's
        // carries the payload, so the target is the one before alice's,
        // which no branch reached any more and had to be fetched by ID.
        assert_eq!(plan.lines.len(), 2);
        assert_eq!(plan.lines[0].git_ref, "refs/heads/legacy");
        assert_eq!(plan.lines[0].back, Back::NoRecord);
        let Back::To {
            commit,
            undoes,
            dropped,
        } = &plan.lines[1].back
        else {
            unreachable!("main can be put back: {:?}", plan.lines[1].back)
        };
        assert_eq!(*commit, clean);
        assert_eq!(undoes.actor, "alice");
        assert_eq!(*dropped, 2);
        assert_eq!(plan.possible(), 1);
        // Planning pushed nothing.
        assert_ne!(c.w.tip("shop", "refs/heads/main"), clean);

        let done = restore(&repo, &plan).expect("restores");
        assert_eq!(done.matched, vec!["refs/heads/main"]);
        assert!(done.differ.is_empty());
        assert_eq!(c.w.tip("shop", "refs/heads/main"), clean);
        assert_eq!(
            c.w.file("shop", "main", "postcss.config.mjs").as_deref(),
            Some(CLEAN_CONFIG)
        );
        // What it moved away from is kept in the copy.
        let dir = mirror_path(&c.evidence, "acme/shop");
        assert_eq!(
            git_out(
                &dir,
                &[
                    "rev-parse",
                    &format!("refs/polinrider/before-fix/{STAMP}/heads/main")
                ]
            ),
            plan.lines[1].tip
        );
        // And a second check finds only the branch that could not be put back.
        let after = c.again();
        assert_eq!(after.affected(), vec![("acme/shop", 1)]);
        assert_eq!(after.confirmed[0].short_ref(), "legacy");
    }

    #[test]
    fn restore_never_targets_an_earlier_wave() {
        // Every state GitHub remembers carries the payload.
        let w = World::new("fix-waves");
        w.repo(
            "site",
            &[("main", &[("vite.config.js", &infected_config())])],
        );
        w.push(
            "site",
            "main",
            &[("next.config.js", &infected_config())],
            false,
            Some(("alice", "2026-09-12T10:00:00Z")),
        );
        let c = Checked::new(w);
        let repo = c.repo("acme/site");
        let plan = plan_restore(&repo).expect("plans");
        assert_eq!(plan.lines[0].back, Back::NothingClean);
        assert_eq!(plan.possible(), 0);
        let before = c.w.tip("site", "refs/heads/main");
        assert!(restore(&repo, &plan).is_err());
        assert_eq!(c.w.tip("site", "refs/heads/main"), before);
    }

    #[test]
    fn a_push_that_landed_since_the_check_is_never_overwritten() {
        let (c, _) = attacked();
        let repo = c.repo("acme/shop");
        let plan = plan_restore(&repo).expect("plans");
        // Somebody pushes after the plan was shown.
        let (_, newer) =
            c.w.push("shop", "main", &[("late.js", b"work\n")], false, None);
        let refused = restore(&repo, &plan).expect_err("refused");
        assert!(refused.contains("git push"), "{refused}");
        assert_eq!(c.w.tip("shop", "refs/heads/main"), newer);
    }

    #[test]
    fn remove_adds_one_commit_that_cuts_the_payload_and_rewrites_nothing() {
        let (c, _) = attacked();
        let repo = c.repo("acme/shop");
        let plan = plan_remove(&repo).expect("plans");
        let main = plan
            .lines
            .iter()
            .find(|l| l.git_ref == "refs/heads/main")
            .expect("main");
        // The config is cut, not deleted. The fake font is the payload.
        assert_eq!(main.strip, vec!["postcss.config.mjs"]);
        assert_eq!(main.delete, vec!["public/fonts/inter.woff2"]);
        let old = c.w.tip("shop", "refs/heads/main");

        let done = remove(&repo, &plan).expect("removes");
        assert_eq!(done.matched.len(), 2);
        assert_eq!(
            c.w.file("shop", "main", "postcss.config.mjs").as_deref(),
            Some(CLEAN_CONFIG)
        );
        assert_eq!(c.w.file("shop", "main", "public/fonts/inter.woff2"), None);
        // One commit on top of what was there.
        assert_eq!(c.w.tip("shop", "refs/heads/main^"), old);
        assert!(c.again().confirmed.is_empty());
    }

    #[test]
    fn erase_takes_the_payload_out_of_every_commit_and_puts_clean_configs_back() {
        let w = World::new("fix-erase");
        w.repo(
            "shop",
            &[("main", &[("postcss.config.mjs", CLEAN_CONFIG.as_bytes())])],
        );
        // The payload once sat in a file that has since been overwritten:
        // it is in the history and not at the tip.
        w.push(
            "shop",
            "main",
            &[("old/vendor.js", format!("var a='{STRONG}'\n").as_bytes())],
            false,
            None,
        );
        w.push(
            "shop",
            "main",
            &[("old/vendor.js", b"var a=1\n")],
            false,
            None,
        );
        w.push(
            "shop",
            "main",
            &[
                ("postcss.config.mjs", &infected_config()),
                ("scripts/temp_helper.bat", b"@echo off\n"),
            ],
            false,
            None,
        );
        w.tag("shop", "v1", "main");
        let c = Checked::new(w);
        let repo = c.repo("acme/shop");

        let plan = plan_erase(&repo).expect("plans");
        assert_eq!(
            plan.paths,
            vec![
                "old/vendor.js",
                "postcss.config.mjs",
                "scripts/temp_helper.bat"
            ]
        );
        assert_eq!(plan.commits, 5);
        assert_eq!(plan.branches, vec!["refs/heads/main"]);
        assert_eq!(plan.tags, vec!["refs/tags/v1"]);
        assert_eq!(
            plan.put_back,
            vec![(
                "refs/heads/main".to_string(),
                vec!["postcss.config.mjs".to_string()]
            )]
        );
        let old = c.w.tip("shop", "refs/heads/main");

        let done = erase(&repo, &plan).expect("erases");
        assert_eq!(done.matched, vec!["refs/heads/main", "refs/tags/v1"]);
        assert!(done.differ.is_empty());
        assert_ne!(c.w.tip("shop", "refs/heads/main"), old);
        // Not in any commit that a branch or tag reaches.
        let everything = git_out(
            &c.w.bare("shop"),
            &["log", "--branches", "--tags", "-p", "--format=%H"],
        );
        assert!(!everything.contains(STRONG));
        assert!(!everything.contains("temp_helper.bat"));
        // The config is back, clean, and the rest of the project is intact.
        assert_eq!(
            c.w.file("shop", "main", "postcss.config.mjs").as_deref(),
            Some(CLEAN_CONFIG)
        );
        assert_eq!(
            c.w.file("shop", "main", "README.txt").as_deref(),
            Some("base\n")
        );
        assert!(c.again().confirmed.is_empty());
        // The second copy is gone, and the old history is kept in the first.
        assert!(!c.evidence.join("rewrite/acme__shop.git").exists());
        let dir = mirror_path(&c.evidence, "acme/shop");
        assert_eq!(
            git_out(
                &dir,
                &[
                    "rev-parse",
                    &format!("refs/polinrider/before-fix/{STAMP}/heads/main")
                ]
            ),
            old
        );
    }

    #[test]
    fn archive_puts_the_notice_on_top_and_removes_nothing() {
        let (c, _) = attacked();
        // Give it a README with something in it.
        c.w.push(
            "shop",
            "main",
            &[("README.md", b"# Shop\n\nHow to run it.\n")],
            false,
            None,
        );
        let found = c.again();
        let repo = Repo::of(
            &c.forge,
            "acme/shop",
            &c.evidence,
            &c.ind,
            &found,
            "tester",
            STAMP,
        );
        let facts = NoticeFacts {
            owner: "acme",
            is_organization: true,
            found: STAMP,
        };
        let plan = plan_archive(&repo, &facts).expect("plans");
        assert_eq!(plan.branch, "refs/heads/main");
        assert!(plan.readme_exists);
        assert!(!plan.already_noticed);
        assert!(plan.notice.starts_with(NOTICE_HEADING));
        assert!(plan.notice.contains("\n> [!CAUTION]\n"));
        assert!(flat(&plan.notice).contains(
            "found on 3 October 2026 in `postcss.config.mjs` and 2 other files, on 2 branches"
        ));
        assert!(flat(&plan.notice).contains("on or after 11 September 2026**"));
        assert!(flat(&plan.notice).contains("the owners of the acme organization"));

        archive(&repo, &plan).expect("archives");
        let readme = c.w.file("shop", "main", "README.md").expect("readme");
        assert!(readme.starts_with(NOTICE_HEADING), "{readme}");
        assert!(
            readme.ends_with("\n---\n\n# Shop\n\nHow to run it.\n"),
            "{readme}"
        );
        let changed = c.w.dir.join("forge/changed/acme");
        assert_eq!(
            fs::read_to_string(changed.join("shop.description")).expect("description"),
            format!("{ARCHIVED_DESCRIPTION}\n")
        );
        assert!(changed.join("shop.archived").exists());

        // A second run does not stack a second notice.
        let found = c.again();
        let repo = Repo::of(
            &c.forge,
            "acme/shop",
            &c.evidence,
            &c.ind,
            &found,
            "tester",
            STAMP,
        );
        assert!(plan_archive(&repo, &facts).expect("plans").already_noticed);
    }

    #[test]
    fn a_repository_without_a_readme_gets_one_and_no_date_is_guessed() {
        let w = World::new("fix-noreadme");
        w.repo(
            "old-site",
            &[("main", &[("vite.config.js", &infected_config())])],
        );
        let c = Checked::new(w);
        let repo = c.repo("acme/old-site");
        let plan = plan_archive(
            &repo,
            &NoticeFacts {
                owner: "tester",
                is_organization: false,
                found: STAMP,
            },
        )
        .expect("plans");
        assert!(!plan.readme_exists);
        assert_eq!(plan.readme, "README.md");
        assert!(flat(&plan.notice).contains("in `vite.config.js`, on 1 branch."));
        assert!(
            flat(&plan.notice).contains("**If you ever cloned, opened or built this repository**")
        );
        assert!(flat(&plan.notice).contains("ask its owner, tester, for a clean copy"));
        archive(&repo, &plan).expect("archives");
        assert!(c
            .w
            .file("old-site", "main", "README.md")
            .expect("readme")
            .starts_with(NOTICE_HEADING));
    }

    #[test]
    fn a_commit_is_authored_by_the_signed_in_account_when_git_has_no_name() {
        let (c, _) = attacked();
        let dir = mirror_path(&c.evidence, "acme/shop");
        let mirror = Mirror { dir: &dir };
        // Whatever this machine has configured, the answer is one or the other.
        let who = identity(&mirror, "octocat");
        if !who.is_empty() {
            assert_eq!(
                who[1],
                (
                    "GIT_AUTHOR_EMAIL",
                    "octocat@users.noreply.github.com".to_string()
                )
            );
        }
        git(&dir, &["config", "user.name", "Somebody"]);
        git(&dir, &["config", "user.email", "somebody@example.com"]);
        assert!(identity(&mirror, "octocat").is_empty());
    }

    #[test]
    fn dates_are_said_the_way_people_say_them() {
        assert_eq!(
            long_date("2026-09-11T09:14:00Z").as_deref(),
            Some("11 September 2026")
        );
        assert_eq!(
            long_date("20261003T120000Z").as_deref(),
            Some("3 October 2026")
        );
        assert_eq!(long_date("2026-13-01T00:00:00Z"), None);
        assert_eq!(long_date("soon"), None);
    }
}
