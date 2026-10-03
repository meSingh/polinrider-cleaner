//! Fixing what the GitHub check found: the screens between the summary and
//! the last step.
//!
//! The same rules as the rest of the flow, and one more. Before anything is
//! pushed, a screen says exactly what would change on GitHub, worked out on
//! the copy. Only `yes` goes ahead, and for the one choice that changes
//! every repository at once, only the owner's name does. Enter is never yes.
//!
//! What a fix did is said from what GitHub shows afterwards, not from what
//! the push printed.

use crate::guide::{
    bad, blank, count, dim, good, line, p, read, strong, warn, word, Console, Session, Stop,
};
use crate::remote::{Findings, OwnerKind};
use crate::remote_fix::{
    self, long_date, short, wrap, Archive, Back, Done, Erase, NoticeFacts, Remove, Repo, Restore,
    ARCHIVED_DESCRIPTION,
};
use crate::ui::{text_of, Span};
use crate::verdict::clean;
use std::path::Path;

/// What the fix screens need to know about the check that came before them.
pub(crate) struct Context<'a> {
    pub(crate) session: &'a Session<'a>,
    pub(crate) owner: &'a str,
    pub(crate) kind: OwnerKind,
    pub(crate) login: &'a str,
    pub(crate) evidence: &'a Path,
    pub(crate) found: &'a Findings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fix {
    Restore,
    Erase,
    Remove,
    Archive,
}

impl Fix {
    const fn word(self) -> &'static str {
        match self {
            Fix::Restore => "restore",
            Fix::Erase => "erase",
            Fix::Remove => "remove",
            Fix::Archive => "archive",
        }
    }
}

/// How one repository came out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum What {
    /// A fix went through. `left` is how many of its branches and tags
    /// still carry the payload afterwards.
    Fixed { fix: Fix, moved: usize, left: usize },
    /// Marked as infected and made read-only. The payload is still in it.
    Archived,
    /// Left as it was, by choice or because the chosen fix was not possible.
    Skipped,
    /// A fix was started and did not finish.
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Outcome {
    pub(crate) repository: String,
    pub(crate) what: What,
}

impl Outcome {
    /// Nothing of the payload is left on a branch or tag.
    pub(crate) fn is_fixed(&self) -> bool {
        matches!(self.what, What::Fixed { left: 0, .. })
    }
}

enum Plan {
    Restore(Restore),
    Erase(Erase),
    Remove(Remove),
    Archive(Archive),
}

impl Plan {
    /// How many branches and tags it would move.
    fn moves(&self) -> usize {
        match self {
            Plan::Restore(plan) => plan.possible(),
            Plan::Erase(plan) => plan.branches.len() + plan.tags.len(),
            Plan::Remove(plan) => plan.lines.len(),
            Plan::Archive(_) => 1,
        }
    }
}

fn repo<'a>(ctx: &'a Context, name: &'a str) -> Repo<'a> {
    Repo::of(
        ctx.session.forge,
        name,
        ctx.evidence,
        ctx.session.ind,
        ctx.found,
        ctx.login,
        ctx.session.stamp,
    )
}

/// Work out what a fix would do. Reads, and may fetch. Never pushes.
fn plan(ctx: &Context, repo: &Repo, fix: Fix) -> Result<Plan, String> {
    match fix {
        Fix::Restore => {
            let plan = remote_fix::plan_restore(repo)?;
            if plan.possible() == 0 {
                return Err(no_restore(&plan).to_string());
            }
            Ok(Plan::Restore(plan))
        }
        Fix::Erase => remote_fix::plan_erase(repo).map(Plan::Erase),
        Fix::Remove => {
            let plan = remote_fix::plan_remove(repo)?;
            if plan.lines.is_empty() {
                return Err("the payload is only on tags, and a tag cannot take a commit".into());
            }
            Ok(Plan::Remove(plan))
        }
        Fix::Archive => remote_fix::plan_archive(
            repo,
            &NoticeFacts {
                owner: ctx.owner,
                is_organization: ctx.kind == OwnerKind::Organization,
                found: ctx.session.stamp,
            },
        )
        .map(Plan::Archive),
    }
}

/// Why no branch can be put back, in the words shown on the screen.
fn no_restore(plan: &Restore) -> &'static str {
    if plan
        .lines
        .iter()
        .any(|l| matches!(l.back, Back::NothingClean))
    {
        "every earlier state GitHub remembers carries the payload or is gone"
    } else {
        "GitHub no longer has the push record"
    }
}

// --- saying things ------------------------------------------------------------

fn banner(io: &mut dyn Console, session: &Session, left: &str, right: &str) {
    let rule = if session.unicode { "─" } else { "-" }.repeat(56);
    blank(io, 2);
    io.say(&[dim(format!("  {rule}"))]);
    io.say(&[strong(format!("  {left}")), p(format!("   {right}"))]);
    io.say(&[dim(format!("  {rule}"))]);
    blank(io, 1);
    io.say(&[dim("  Nothing is changed in this step.")]);
    blank(io, 2);
}

/// An option: the word to type, then what it does on the lines beside it.
fn option(io: &mut dyn Console, name: &str, about: &[Span]) {
    for (n, span) in about.iter().enumerate() {
        let lead = if n == 0 {
            word(format!("      {name:<12}"))
        } else {
            p(" ".repeat(18))
        };
        io.say(&[lead, span.clone()]);
    }
    blank(io, 1);
}

/// "it matches", "both match", "all 4 match".
fn matching(n: usize) -> String {
    match n {
        1 => "it matches".into(),
        2 => "both match".into(),
        n => format!("all {n} match"),
    }
}

fn names(list: &[&str]) -> String {
    list.join(", ")
}

/// The first ten digits of a commit ID, which is how one is said.
fn brief(commit: &str) -> &str {
    commit.get(..10).unwrap_or(commit)
}

/// `11 September` from a push time. The year is on the report.
fn day(at: &str) -> String {
    long_date(at)
        .and_then(|date| date.rsplit_once(' ').map(|(day, _)| day.to_string()))
        .unwrap_or_else(|| clean(at))
}

/// Up to `most` of `paths`, each on a line, then how many were left out.
fn listed(out: &mut Vec<Vec<Span>>, indent: usize, paths: &[String], most: usize) {
    for path in paths.iter().take(most) {
        out.push(vec![dim(format!("{}{}", " ".repeat(indent), clean(path)))]);
    }
    if paths.len() > most {
        out.push(vec![dim(format!(
            "{}and {} more, in the report",
            " ".repeat(indent),
            paths.len() - most
        ))]);
    }
}

// --- the screens --------------------------------------------------------------

/// From the summary to the last step: how to go through the repositories
/// that carry the payload, and then doing it.
pub(crate) fn run(
    ctx: &Context,
    io: &mut dyn Console,
    report: &mut String,
) -> Result<Vec<Outcome>, Stop> {
    let affected: Vec<&str> = ctx.found.affected().into_iter().map(|(r, _)| r).collect();
    let several = affected.len() > 1;
    let mut show = true;
    loop {
        if show {
            blank(io, 2);
            if several {
                option(
                    io,
                    "each",
                    &[
                        p("Go through them one at a time and choose"),
                        p("a fix for each."),
                    ],
                );
                option(
                    io,
                    "all",
                    &[p(format!(
                        "One fix for all {}, after a warning.",
                        affected.len()
                    ))],
                );
            } else {
                option(io, "fix", &[p("Choose how to fix it.")]);
            }
            option(io, "details", &[p("Every branch and file first.")]);
            option(io, "none", &[p("Leave GitHub as it is for now.")]);
            io.say(&[p(if several {
                "  Type each, all, details or none."
            } else {
                "  Type fix, details or none."
            })]);
        }
        show = false;
        match read(io)?.as_str() {
            "each" | "fix" => return each(ctx, &affected, io, report),
            "all" if several => {
                if let Some(outcomes) = all(ctx, &affected, io, report)? {
                    return Ok(outcomes);
                }
                show = true;
            }
            "details" | "d" => {
                crate::guide_github::details(io, ctx.found);
                show = true;
            }
            "none" => {
                blank(io, 1);
                line(io, "  Left as it is. Nothing on GitHub was changed.");
                return Ok(affected
                    .iter()
                    .map(|r| Outcome {
                        repository: (*r).to_string(),
                        what: What::Skipped,
                    })
                    .collect());
            }
            _ => {
                blank(io, 1);
                io.say(&[warn(if several {
                    "  Type each, all, details or none. q quits."
                } else {
                    "  Type fix, details or none. q quits."
                })]);
            }
        }
    }
}

fn each(
    ctx: &Context,
    affected: &[&str],
    io: &mut dyn Console,
    report: &mut String,
) -> Result<Vec<Outcome>, Stop> {
    let mut outcomes: Vec<Outcome> = Vec::new();
    for (n, name) in affected.iter().enumerate() {
        let what = one(ctx, name, n + 1, affected.len(), io, report)?;
        let asked = !matches!(what, What::Skipped);
        outcomes.push(Outcome {
            repository: (*name).to_string(),
            what,
        });
        if asked {
            blank(io, 2);
            io.say(&[
                p("  Press "),
                word("Enter"),
                p(if n + 1 < affected.len() {
                    " for the next repository."
                } else {
                    " to continue."
                }),
            ]);
            read(io)?;
        }
    }
    Ok(outcomes)
}

/// One repository: the fixes it can take, then the one chosen.
fn one(
    ctx: &Context,
    name: &str,
    n: usize,
    of: usize,
    io: &mut dyn Console,
    report: &mut String,
) -> Result<What, Stop> {
    banner(
        io,
        ctx.session,
        &format!("REPOSITORY {n} OF {of}"),
        &clean(name),
    );
    let repo = repo(ctx, name);
    // Looked up before the choices are shown, so that restore is offered
    // only where it is possible.
    let restore = remote_fix::plan_restore(&repo);
    let can_restore = restore.as_ref().is_ok_and(|plan| plan.possible() > 0);

    if let (true, Ok(plan)) = (can_restore, &restore) {
        let total = plan.lines.len();
        let record = match (plan.possible(), total) {
            (1, 1) => {
                let commit = plan.lines.iter().find_map(|l| match &l.back {
                    Back::To { commit, .. } => Some(brief(commit).to_string()),
                    _ => None,
                });
                format!(
                    "Record found. Commit {} checked clean.",
                    commit.unwrap_or_default()
                )
            }
            (2, 2) => "Record found for both branches. Checked clean.".to_string(),
            (can, total) if can == total => {
                format!("Record found for all {total} branches. Checked clean.")
            }
            (can, total) => {
                format!("Record found for {can} of {total}. The others stay as they are.")
            }
        };
        option(
            io,
            "restore",
            &[
                p("Put each branch back where GitHub's own push"),
                p("record says it was before the attack."),
                p("The cleanest fix: nothing is edited."),
                good(record),
            ],
        );
    }
    option(
        io,
        "erase",
        &[
            p("Take the payload out of every commit in the"),
            p("history and push it all back. Every commit"),
            p("ID changes, and old clones need to be"),
            p("updated. I show how afterwards."),
        ],
    );
    option(
        io,
        "remove",
        &[
            p("Add one commit per branch that takes the"),
            p("payload out. Easy to undo. The payload"),
            p("stays in older commits."),
        ],
    );
    option(
        io,
        "archive",
        &[
            p("Stop using this repository. It is marked"),
            p("as infected and made read-only."),
        ],
    );
    option(io, "skip", &[p("Leave it as it is for now.")]);
    let words = if can_restore {
        "restore, erase, remove, archive or skip"
    } else {
        let why = match &restore {
            Ok(plan) => no_restore(plan).to_string(),
            Err(why) => clean(why),
        };
        io.say(&[dim("  restore is not possible here:")]);
        for text in wrap(&format!("{why}."), 56) {
            io.say(&[dim(format!("  {text}"))]);
        }
        blank(io, 1);
        "erase, remove, archive or skip"
    };
    io.say(&[p(format!("  Type {words}."))]);

    loop {
        let fix = match read(io)?.as_str() {
            "restore" if can_restore => Fix::Restore,
            "restore" => {
                blank(io, 1);
                io.say(&[warn("  restore is not possible for this repository.")]);
                continue;
            }
            "erase" => Fix::Erase,
            "remove" => Fix::Remove,
            "archive" => Fix::Archive,
            "skip" => return Ok(What::Skipped),
            _ => {
                blank(io, 1);
                io.say(&[warn(format!("  Type {words}. q quits."))]);
                continue;
            }
        };
        if let Some(what) = attempt(ctx, &repo, fix, io, report)? {
            return Ok(what);
        }
        blank(io, 1);
        io.say(&[p(format!("  Type {words}."))]);
    }
}

/// Show what a fix would change, ask, and do it on a yes. `None` when
/// nothing was changed and another fix can be chosen.
fn attempt(
    ctx: &Context,
    repo: &Repo,
    fix: Fix,
    io: &mut dyn Console,
    report: &mut String,
) -> Result<Option<What>, Stop> {
    blank(io, 2);
    io.say(&[
        strong(format!("  {}", clean(repo.name))),
        p(format!("   {}", fix.word())),
    ]);
    blank(io, 1);
    if fix == Fix::Erase {
        io.say(&[dim("  Reading the whole history. This only reads.")]);
        blank(io, 1);
    }
    let plan = match plan(ctx, repo, fix) {
        Ok(plan) => plan,
        Err(why) => {
            io.say(&[warn(format!("  {} is not possible here:", fix.word()))]);
            for text in wrap(&clean(&why), 56) {
                io.say(&[warn(format!("  {text}"))]);
            }
            return Ok(None);
        }
    };
    line(io, "  This would change, on GitHub:");
    blank(io, 1);
    for spans in would(&plan) {
        io.say(&spans);
    }
    io.say(&[good("  Nothing has been pushed yet.")]);
    blank(io, 2);
    io.say(&[
        p("  Type "),
        word("yes"),
        p(if fix == Fix::Archive {
            format!(" to archive {}.", clean(repo.name))
        } else {
            " to push this to GitHub.".to_string()
        }),
    ]);
    io.say(&[
        p("  Type "),
        word("no"),
        p(format!(" to leave {} as it is.", clean(repo.name))),
    ]);
    loop {
        match read(io)?.as_str() {
            "yes" => break,
            "no" | "n" => {
                blank(io, 1);
                line(io, "  Nothing was changed.");
                return Ok(None);
            }
            _ => {
                blank(io, 1);
                io.say(&[warn("  Type yes or no. Enter is not yes. q quits.")]);
            }
        }
    }

    blank(io, 1);
    io.say(&[dim(if fix == Fix::Erase {
        "  Rewriting the history, then pushing. This can take a while."
    } else {
        "  Pushing to GitHub."
    })]);
    let result = perform(repo, &plan);
    record(report, repo.name, fix, &plan, &result);
    blank(io, 1);
    match result {
        Ok(done) => {
            let what = outcome(fix, &plan, &done);
            for spans in did(repo, &plan, &done) {
                io.say(&spans);
            }
            if fix == Fix::Erase {
                blank(io, 1);
                let lines = clone_update(repo.name, &branch_names(&plan));
                report.push('\n');
                for spans in &lines {
                    report.push_str(&text_of(spans));
                    report.push('\n');
                    io.say(spans);
                }
            }
            Ok(Some(what))
        }
        Err(why) => {
            io.say(&[warn("  That did not work.")]);
            for text in wrap(&clean(&why), 56) {
                io.say(&[warn(format!("  {text}"))]);
            }
            if fix == Fix::Archive {
                // An archive is three changes, and some may have gone through.
                return Ok(Some(What::Failed(why)));
            }
            blank(io, 1);
            for text in [
                "  Nothing on GitHub was changed. A protected branch or",
                "  a ruleset can refuse a push, and so can a push that",
                "  landed since the check.",
            ] {
                io.say(&[dim(text)]);
            }
            Ok(None)
        }
    }
}

/// Do what a plan says. The only place a fix is called from.
fn perform(repo: &Repo, plan: &Plan) -> Result<Done, String> {
    match plan {
        Plan::Restore(plan) => remote_fix::restore(repo, plan),
        Plan::Erase(plan) => remote_fix::erase(repo, plan),
        Plan::Remove(plan) => remote_fix::remove(repo, plan),
        Plan::Archive(plan) => remote_fix::archive(repo, plan).map(|()| Done {
            matched: vec![plan.branch.clone()],
            ..Done::default()
        }),
    }
}

fn outcome(fix: Fix, plan: &Plan, done: &Done) -> What {
    let carrying = match plan {
        Plan::Restore(plan) => plan.lines.len(),
        Plan::Remove(plan) => plan.lines.len() + plan.tags.len(),
        Plan::Erase(_) => plan.moves(),
        Plan::Archive(_) => return What::Archived,
    };
    What::Fixed {
        fix,
        moved: done.matched.len(),
        left: carrying.saturating_sub(done.matched.len()),
    }
}

fn branch_names(plan: &Plan) -> Vec<String> {
    match plan {
        Plan::Erase(plan) => plan.branches.iter().map(|b| short(b).to_string()).collect(),
        _ => Vec::new(),
    }
}

/// The dry run: every line of what a plan would change.
fn would(plan: &Plan) -> Vec<Vec<Span>> {
    let mut out: Vec<Vec<Span>> = Vec::new();
    match plan {
        Plan::Restore(plan) => {
            let width = plan
                .lines
                .iter()
                .map(|l| short(&l.git_ref).len())
                .max()
                .unwrap_or(0);
            let mut dropped = 0;
            for l in &plan.lines {
                let name = format!("      {:<width$}   ", clean(short(&l.git_ref)));
                match &l.back {
                    Back::To {
                        commit,
                        undoes,
                        dropped: n,
                    } => {
                        dropped += n;
                        out.push(vec![p(name), p(format!("back to {}", brief(commit)))]);
                        out.push(vec![dim(format!(
                            "      {}   undoes the push of {} by {}",
                            " ".repeat(width),
                            day(&undoes.at),
                            clean(&undoes.actor)
                        ))]);
                    }
                    Back::Tag => out.push(vec![p(name), warn("stays as it is: a tag")]),
                    Back::NoRecord => {
                        out.push(vec![p(name), warn("stays as it is: no push record")]);
                    }
                    Back::NothingClean => out.push(vec![
                        p(name),
                        warn("stays as it is: no clean state on record"),
                    ]),
                }
            }
            out.push(vec![]);
            out.push(vec![p(match plan.possible() {
                1 => "  The commit was fetched and checked clean.".to_string(),
                2 => "  Both commits were fetched and checked clean.".to_string(),
                n => format!("  All {n} commits were fetched and checked clean."),
            })]);
            out.push(vec![p(format!(
                "  {} pushed since then {} being reachable.",
                count(dropped, "commit", "commits"),
                if dropped == 1 { "stops" } else { "stop" }
            ))]);
            out.push(vec![dim(
                "  If any of that was real work, it is kept in the copy",
            )]);
            out.push(vec![dim("  and can be brought back.")]);
            if plan.possible() < plan.lines.len() {
                out.push(vec![warn(
                    "  What stays as it is still carries the payload.",
                )]);
            }
        }
        Plan::Erase(plan) => {
            out.push(vec![p(format!(
                "      {} taken out of every commit:",
                if plan.paths.len() == 1 {
                    "1 file is".to_string()
                } else {
                    format!("{} files are", plan.paths.len())
                }
            ))]);
            listed(&mut out, 10, &plan.paths, 8);
            out.push(vec![]);
            out.push(vec![p(format!(
                "      Every commit gets a new ID: {} in all.",
                plan.commits
            ))]);
            out.push(vec![p(format!(
                "      {} of them {} these files.",
                plan.touched,
                if plan.touched == 1 {
                    "changes"
                } else {
                    "change"
                }
            ))]);
            let pushed = match plan.tags.len() {
                0 => count(plan.branches.len(), "branch", "branches"),
                n => format!(
                    "{} and {}",
                    count(plan.branches.len(), "branch", "branches"),
                    count(n, "tag", "tags")
                ),
            };
            out.push(vec![p(format!(
                "      {pushed} {} pushed again, over what is there.",
                if plan.branches.len() + plan.tags.len() == 1 {
                    "is"
                } else {
                    "are"
                }
            ))]);
            if !plan.put_back.is_empty() {
                out.push(vec![]);
                out.push(vec![p(
                    "      The clean part of each build config is put back,",
                )]);
                out.push(vec![p("      as one new commit on:")]);
                let on: Vec<String> = plan
                    .put_back
                    .iter()
                    .map(|(b, _)| short(b).to_string())
                    .collect();
                listed(&mut out, 10, &on, 8);
            }
            out.push(vec![]);
            out.push(vec![p(
                "  Every existing clone then needs updating. I show how",
            )]);
            out.push(vec![p("  afterwards.")]);
            out.push(vec![dim(
                "  GitHub keeps the old commits for a while, by ID and in",
            )]);
            out.push(vec![dim(
                "  pull requests. The last step says how to clear them.",
            )]);
        }
        Plan::Remove(plan) => {
            let width = plan
                .lines
                .iter()
                .map(|l| short(&l.git_ref).len())
                .max()
                .unwrap_or(0);
            for l in &plan.lines {
                out.push(vec![p(format!(
                    "      {:<width$}   one new commit",
                    clean(short(&l.git_ref))
                ))]);
                let mut what: Vec<String> = l
                    .strip
                    .iter()
                    .map(|path| format!("cuts the payload out of {path}"))
                    .collect();
                what.extend(l.delete.iter().map(|path| format!("deletes {path}")));
                listed(&mut out, 9 + width, &what, 4);
            }
            out.push(vec![]);
            out.push(vec![p(
                "  An ordinary push. Nothing is rewritten, and the commit",
            )]);
            out.push(vec![p("  can be reverted.")]);
            out.push(vec![warn(
                "  The payload stays in older commits: anyone who checks",
            )]);
            out.push(vec![warn("  one out still gets it.")]);
            if !plan.tags.is_empty() {
                let tags: Vec<&str> = plan.tags.iter().map(|t| short(t)).collect();
                out.push(vec![warn(format!(
                    "  {} the payload and cannot take a commit: {}",
                    if tags.len() == 1 {
                        "1 tag carries".to_string()
                    } else {
                        format!("{} tags carry", tags.len())
                    },
                    clean(&names(&tags))
                ))]);
            }
        }
        Plan::Archive(plan) => {
            if plan.already_noticed {
                out.push(vec![p("      The notice is already on top of the README.")]);
            } else {
                out.push(vec![p(format!(
                    "      A notice is added to the top of {}, as one",
                    clean(&plan.readme)
                ))]);
                out.push(vec![p(format!(
                    "      new commit on {}. {}",
                    clean(short(&plan.branch)),
                    if plan.readme_exists {
                        "Nothing in the file is removed."
                    } else {
                        "The file is new."
                    }
                ))]);
            }
            out.push(vec![]);
            out.push(vec![p("      The description becomes")]);
            out.push(vec![p(format!("      \"{ARCHIVED_DESCRIPTION}\""))]);
            out.push(vec![]);
            out.push(vec![p(
                "      The repository is archived: read-only, no pushes,",
            )]);
            out.push(vec![p("      and GitHub shows an archived banner on it.")]);
            out.push(vec![]);
            out.push(vec![warn(
                "  The payload stays inside it. Anyone who clones it still",
            )]);
            out.push(vec![warn(
                "  gets the malware. This stops it being used, not being there.",
            )]);
            out.push(vec![dim("  An owner can unarchive it later.")]);
            if !plan.already_noticed {
                out.push(vec![]);
                out.push(vec![p("  The notice, as it is written into the README:")]);
                out.push(vec![]);
                for text in plan.notice.lines() {
                    out.push(vec![dim(format!("      {}", clean(text)))]);
                }
            }
        }
    }
    out.push(vec![]);
    out
}

/// What a fix did, from what GitHub shows afterwards.
fn did(repo: &Repo, plan: &Plan, done: &Done) -> Vec<Vec<Span>> {
    let name = clean(repo.name);
    let n = done.matched.len();
    let mut out: Vec<Vec<Span>> = Vec::new();
    match plan {
        Plan::Restore(_) => out.push(vec![good(format!(
            "  Done. {} of {name} put back.",
            count(n, "branch", "branches")
        ))]),
        Plan::Erase(_) => out.push(vec![good(format!(
            "  Done. The payload is out of the history of {name}."
        ))]),
        Plan::Remove(_) => out.push(vec![good(format!(
            "  Done. The payload is out of {} of {name}.",
            count(n, "branch", "branches")
        ))]),
        Plan::Archive(_) => {
            out.push(vec![good(format!("  Done. {name} is archived,"))]);
            out.push(vec![good("  with the notice on top of its README.")]);
            return out;
        }
    }
    out.push(vec![p(format!(
        "  Checked on GitHub afterwards: {}.",
        matching(n)
    ))]);
    if !done.differ.is_empty() {
        let differ: Vec<&str> = done.differ.iter().map(|r| short(r)).collect();
        out.push(vec![]);
        out.push(vec![warn(format!(
            "  GitHub did not take {}: {}",
            done.differ.len(),
            clean(&names(&differ))
        ))]);
        if let Some(why) = &done.refused {
            for text in wrap(&clean(why), 56) {
                out.push(vec![dim(format!("  {text}"))]);
            }
        }
    }
    let stays: Vec<&str> = match plan {
        Plan::Restore(plan) => plan
            .lines
            .iter()
            .filter(|l| !matches!(l.back, Back::To { .. }))
            .map(|l| short(&l.git_ref))
            .collect(),
        Plan::Remove(plan) => plan.tags.iter().map(|t| short(t)).collect(),
        _ => Vec::new(),
    };
    if !stays.is_empty() {
        out.push(vec![]);
        out.push(vec![bad(format!(
            "  Still carrying the payload: {}",
            clean(&names(&stays))
        ))]);
        out.push(vec![dim(
            "  Run polinrider again and choose erase or remove for it.",
        )]);
    }
    out
}

/// How to bring an existing clone into line after an erase. Shown on the
/// screen and written to the report, so that it can be sent to a team.
fn clone_update(name: &str, branches: &[String]) -> Vec<Vec<Span>> {
    let name = clean(name);
    let first = branches
        .iter()
        .find(|b| *b == "main" || *b == "master")
        .or(branches.first())
        .map_or("main".to_string(), |b| clean(b));
    let others: Vec<String> = branches
        .iter()
        .map(|b| clean(b))
        .filter(|b| *b != first)
        .collect();
    let mut out = vec![
        vec![p(format!(
            "  Every existing clone of {name} now needs updating."
        ))],
        vec![p("  Send this to everyone who has one.")],
        vec![],
        vec![strong("  1  "), strong("Save any work that is not pushed")],
        vec![dim(
            "     Copy the files you changed somewhere outside the clone.",
        )],
        vec![dim("     The next step discards local changes.")],
        vec![],
        vec![strong("  2  "), strong("Reset the clone to match GitHub")],
        vec![],
        vec![word("         git fetch origin")],
        vec![word(format!("         git checkout {first}"))],
        vec![word(format!("         git reset --hard origin/{first}"))],
    ];
    if !others.is_empty() {
        out.push(vec![]);
        out.push(vec![dim(
            "     Repeat the last two lines for each branch you use:",
        )]);
        for text in wrap(&others.join(", "), 54) {
            out.push(vec![dim(format!("     {text}"))]);
        }
    }
    out.extend([
        vec![],
        vec![
            strong("  3  "),
            strong("Do not git pull, merge or push from the old history"),
        ],
        vec![dim(
            "     That joins the old commits to the new ones and sends the",
        )],
        vec![dim("     payload back to GitHub.")],
        vec![],
        vec![p(
            "  Simpler, and impossible to get wrong: delete the clone and",
        )],
        vec![p("  clone it again.")],
    ]);
    out
}

/// One fix, for the report file: every branch and tag by its full name.
fn record(report: &mut String, name: &str, fix: Fix, plan: &Plan, result: &Result<Done, String>) {
    report.push_str(&format!("\nFIX  {}  {}\n", clean(name), fix.word()));
    match plan {
        Plan::Restore(plan) => {
            for l in &plan.lines {
                report.push_str(&match &l.back {
                    Back::To { commit, undoes, .. } => format!(
                        "  {}  {} -> {}  (undoes the push of {} by {})\n",
                        clean(&l.git_ref),
                        l.tip,
                        commit,
                        clean(&undoes.at),
                        clean(&undoes.actor)
                    ),
                    _ => format!(
                        "  {}  not restorable, left at {}\n",
                        clean(&l.git_ref),
                        l.tip
                    ),
                });
            }
        }
        Plan::Erase(plan) => {
            for path in &plan.paths {
                report.push_str(&format!("  removed from every commit: {}\n", clean(path)));
            }
            for (branch, paths) in &plan.put_back {
                report.push_str(&format!(
                    "  clean part put back on {}: {}\n",
                    clean(branch),
                    clean(&paths.join(", "))
                ));
            }
        }
        Plan::Remove(plan) => {
            for l in &plan.lines {
                for path in &l.strip {
                    report.push_str(&format!(
                        "  {}  payload cut out of {}\n",
                        clean(&l.git_ref),
                        clean(path)
                    ));
                }
                for path in &l.delete {
                    report.push_str(&format!(
                        "  {}  deleted {}\n",
                        clean(&l.git_ref),
                        clean(path)
                    ));
                }
            }
            for tag in &plan.tags {
                report.push_str(&format!(
                    "  {}  a tag, still carries the payload\n",
                    clean(tag)
                ));
            }
        }
        Plan::Archive(plan) => {
            report.push_str(&format!(
                "  notice on top of {} on {}; description replaced; archived\n",
                clean(&plan.readme),
                clean(&plan.branch)
            ));
        }
    }
    match result {
        Ok(done) => {
            for git_ref in &done.matched {
                report.push_str(&format!("  done, checked on GitHub: {}\n", clean(git_ref)));
            }
            for git_ref in &done.differ {
                report.push_str(&format!("  NOT CHANGED on GitHub: {}\n", clean(git_ref)));
            }
            if let Some(why) = &done.refused {
                report.push_str(&format!("  git said: {}\n", clean(why)));
            }
            report.push_str(
                "  what it replaced is kept in the copy under refs/polinrider/before-fix/\n",
            );
        }
        Err(why) => report.push_str(&format!("  FAILED: {}\n", clean(why))),
    }
}

// --- all at once --------------------------------------------------------------

/// One fix for every affected repository. `None` when the operator backed
/// out before anything was changed.
fn all(
    ctx: &Context,
    affected: &[&str],
    io: &mut dyn Console,
    report: &mut String,
) -> Result<Option<Vec<Outcome>>, Stop> {
    let owner = clean(ctx.owner);
    banner(
        io,
        ctx.session,
        &format!("ALL {} REPOSITORIES", affected.len()),
        &owner,
    );
    line(io, "  Which fix, for all of them?");
    blank(io, 1);
    option(io, "restore", &[p("where GitHub's record allows it")]);
    option(
        io,
        "erase",
        &[
            p("rewrite the history of every one; every"),
            p("clone then needs updating"),
        ],
    );
    option(io, "remove", &[p("one new commit per branch")]);
    line(io, "  Type restore, erase or remove.");
    io.say(&[dim("  no goes back to choosing one at a time.")]);
    let fix = loop {
        match read(io)?.as_str() {
            "restore" => break Fix::Restore,
            "erase" => break Fix::Erase,
            "remove" => break Fix::Remove,
            "no" | "n" | "back" | "b" => return Ok(None),
            _ => {
                blank(io, 1);
                io.say(&[warn("  Type restore, erase, remove or no. q quits.")]);
            }
        }
    };

    blank(io, 1);
    io.say(&[dim(
        "  Working out what that would change. This only reads.",
    )]);
    let repos: Vec<Repo> = affected.iter().map(|name| repo(ctx, name)).collect();
    let plans: Vec<Result<Plan, String>> = repos.iter().map(|r| plan(ctx, r, fix)).collect();
    let can: Vec<&str> = repos
        .iter()
        .zip(&plans)
        .filter(|(_, plan)| plan.is_ok())
        .map(|(r, _)| r.name)
        .collect();
    blank(io, 2);
    if can.is_empty() {
        io.say(&[warn(format!("  None of them can take {}:", fix.word()))]);
        for (r, plan) in repos.iter().zip(&plans) {
            if let Err(why) = plan {
                io.say(&[p(format!("      {}", clean(r.name)))]);
                for text in wrap(&clean(why), 50) {
                    io.say(&[dim(format!("      {text}"))]);
                }
            }
        }
        return Ok(None);
    }

    let moves: usize = plans.iter().flatten().map(Plan::moves).sum();
    io.say(&[bad("  This is a big step. Read this before you answer.")]);
    blank(io, 1);
    io.say(&[p(format!(
        "      It changes {} and {} on GitHub,",
        count(can.len(), "repository", "repositories"),
        count(moves, "branch or tag", "branches and tags")
    ))]);
    line(io, "      one after another, without asking again.");
    blank(io, 1);
    io.say(&[p(format!("      {} can take {}:", can.len(), fix.word()))]);
    for text in wrap(&clean(&names(&can)), 50) {
        io.say(&[p(format!("          {text}"))]);
    }
    let cannot: Vec<(&str, &String)> = repos
        .iter()
        .zip(&plans)
        .filter_map(|(r, plan)| plan.as_ref().err().map(|why| (r.name, why)))
        .collect();
    if !cannot.is_empty() {
        blank(io, 1);
        io.say(&[p(format!(
            "      {} cannot, and will be left alone:",
            cannot.len()
        ))]);
        for (name, why) in &cannot {
            io.say(&[p(format!("          {}", clean(name)))]);
            for text in wrap(&clean(why), 46) {
                io.say(&[dim(format!("          {text}"))]);
            }
        }
    }
    blank(io, 1);
    line(
        io,
        "      It can take a long time, and it should not be stopped",
    );
    line(
        io,
        "      half way. Everyone with a clone will need a fresh one.",
    );
    blank(io, 2);
    let whose = if ctx.kind == OwnerKind::Organization {
        "organization's"
    } else {
        "account's"
    };
    io.say(&[
        p(format!("  Type the {whose} name to go ahead:  ")),
        word(owner.clone()),
    ]);
    io.say(&[
        p("  Type "),
        word("no"),
        p(" to go back and choose one at a time."),
    ]);
    loop {
        let typed = read(io)?;
        if typed == ctx.owner.to_ascii_lowercase() {
            break;
        }
        if matches!(typed.as_str(), "no" | "n" | "back" | "b") {
            return Ok(None);
        }
        blank(io, 1);
        io.say(&[warn(format!(
            "  Type {owner} to go ahead, or no. yes is not enough here."
        ))]);
    }

    blank(io, 2);
    let width = affected.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut outcomes: Vec<Outcome> = Vec::new();
    let mut updates: Vec<Vec<Span>> = Vec::new();
    for (r, plan) in repos.iter().zip(&plans) {
        let name = format!("      {:<width$}   ", clean(r.name));
        let what = match plan {
            Err(_) => {
                io.say(&[p(name), dim("left alone")]);
                What::Skipped
            }
            Ok(plan) => {
                let result = perform(r, plan);
                record(report, r.name, fix, plan, &result);
                match result {
                    Ok(done) => {
                        let what = outcome(fix, plan, &done);
                        let left = matches!(what, What::Fixed { left, .. } if left > 0);
                        io.say(&[
                            p(name),
                            good(format!(
                                "done, {} checked on GitHub",
                                count(done.matched.len(), "branch or tag", "branches and tags")
                            )),
                            if left {
                                warn("   part still carries it")
                            } else {
                                p("")
                            },
                        ]);
                        if fix == Fix::Erase {
                            updates.push(vec![]);
                            updates.extend(clone_update(r.name, &branch_names(plan)));
                        }
                        what
                    }
                    Err(why) => {
                        io.say(&[p(name), warn("did not work, nothing changed")]);
                        for text in wrap(&clean(&why), 50) {
                            io.say(&[dim(format!("          {text}"))]);
                        }
                        What::Failed(why)
                    }
                }
            }
        };
        outcomes.push(Outcome {
            repository: r.name.to_string(),
            what,
        });
    }
    if !updates.is_empty() {
        for spans in &updates {
            report.push_str(&text_of(spans));
            report.push('\n');
        }
        blank(io, 2);
        line(io, "  Every existing clone of these now needs updating.");
        line(
            io,
            "  The report has the steps for each, to send to everyone",
        );
        line(io, "  who has one. The short version: delete the clone and");
        line(io, "  clone it again.");
    }
    blank(io, 2);
    io.say(&[p("  Press "), word("Enter"), p(" to continue.")]);
    read(io)?;
    Ok(Some(outcomes))
}
