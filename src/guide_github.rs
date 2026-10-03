//! The GitHub half of the guided flow: an organization's repositories, or an
//! account's.
//!
//! The same four steps and the same rules as the rest of the flow (see
//! `guide`), with three things of its own:
//!
//! - **Sign-in is settled first.** `gh`, GitHub's own CLI, has to be installed
//!   and signed in. If it is not, a screen says exactly what to run and checks
//!   again, because a check that fails half way through a list of
//!   repositories is worse than one that never started.
//! - **Organizations are listed**, so nobody types a name from memory.
//! - **A progress screen while it works.** Copying every repository of an
//!   organization takes as long as it takes, and a screen that says nothing
//!   for ten minutes reads as a hang.
//!
//! Everything here reads and reports. What changes GitHub is in `guide_fix`,
//! which this hands over to once the summary is on the screen.

use crate::guide::{
    bad, blank, count, dim, good, header, line, meter, numbered, p, read, strong, thousands, tilde,
    todo, warn, word, Console, Meter, Session, Stop, Todo,
};
use crate::guide_fix::{Context, Fix, Outcome, What};
use crate::host::Probe;
use crate::remote::{self, Check, Findings, OwnerKind, Progress, RefFinding};
use crate::ui::{text_of, Span};
use crate::verdict::{clean, ExitCode};
use std::time::Instant;

/// Steps 2 to 4 for an organization or an account.
pub(crate) fn run(
    session: &Session,
    kind: OwnerKind,
    io: &mut dyn Console,
    report: &mut String,
    worst: &mut Option<ExitCode>,
) -> Result<(), Stop> {
    let login = ready(session, io)?;
    let owner = choose_owner(session, kind, &login, io)?;

    let evidence = match remote::prepare_evidence(session.evidence) {
        Ok(dir) => dir,
        Err(e) => {
            blank(io, 2);
            for text in e.to_string().lines() {
                io.say(&[warn(format!("  {text}"))]);
            }
            return Ok(());
        }
    };

    header(io, session, 3, &format!("Checking {owner}"), false);
    io.say(&[good("  This only reads. Nothing on GitHub is changed.")]);
    blank(io, 1);
    io.say(&[dim(
        "  You can leave this running. A large organization can take",
    )]);
    io.say(&[dim(
        "  a long time: it copies every repository to check it.",
    )]);
    io.say(&[dim(
        "  Ctrl-C stops it. Copies already made are kept, so starting",
    )]);
    io.say(&[dim("  again picks up where it left off.")]);
    blank(io, 2);

    let started = Instant::now();
    let unicode = session.unicode;
    let checked = remote::check(
        session.forge,
        &Check {
            owner: &owner,
            kind,
            evidence: &evidence,
            ind: session.ind,
        },
        &mut |progress| {
            let last = progress.done == progress.total;
            io.progress(
                &progress_lines(&progress, started.elapsed().as_secs(), unicode),
                last,
            );
        },
    );
    let found = match checked {
        Ok(found) => found,
        Err(why) => {
            blank(io, 1);
            io.say(&[warn(format!(
                "  I could not list the repositories of {owner}: {}",
                clean(&why)
            ))]);
            return Ok(());
        }
    };

    *worst = Some(exit_code(&found));
    report.push_str(&written(&owner, session, &found));

    header(io, session, 3, "What I found", false);
    summary(io, session, &owner, &evidence, &found);

    let mut outcomes: Vec<Outcome> = Vec::new();
    if !found.confirmed.is_empty() {
        // The fixes. Each shows what it would change and waits for a yes.
        outcomes = crate::guide_fix::run(
            &Context {
                session,
                owner: &owner,
                kind,
                login: &login,
                evidence: &evidence,
                found: &found,
            },
            io,
            report,
        )?;
    } else if !found.review.is_empty() {
        blank(io, 2);
        io.say(&[p("  Press "), word("Enter"), p(" to continue.")]);
        io.say(&[
            p("  Type "),
            word("details"),
            p(" to see every branch and file first."),
        ]);
        loop {
            match read(io)?.as_str() {
                "" => break,
                "details" | "d" => {
                    details(io, &found);
                    blank(io, 2);
                    io.say(&[p("  Press "), word("Enter"), p(" to continue.")]);
                }
                _ => {
                    blank(io, 1);
                    io.say(&[warn("  Press Enter, or type details. q quits.")]);
                }
            }
        }
    }

    header(io, session, 4, "What to do now", false);
    for spans in what_now(&found, &outcomes) {
        report.push_str(&text_of(&spans));
        report.push('\n');
        io.say(&spans);
    }
    blank(io, 2);
    line(io, "  The full report, with every branch and file:");
    io.say(&[p(format!("  {}", tilde(session.report, session.home)))]);
    blank(io, 1);
    io.say(&[p("  To check again:   "), word("polinrider")]);
    Ok(())
}

/// What the whole check comes to. A repository that could not be copied was
/// not checked, and an incomplete check is not a clean one.
fn exit_code(found: &Findings) -> ExitCode {
    if !found.confirmed.is_empty() {
        ExitCode::Confirmed
    } else if !found.not_checked.is_empty() {
        ExitCode::CouldNotRun
    } else if !found.review.is_empty() {
        ExitCode::Review
    } else {
        ExitCode::Clean
    }
}

// --- step 2: sign-in, and whose repositories ----------------------------------

/// Wait until `gh` is installed and signed in, saying how at each try.
/// Returns the login.
fn ready(session: &Session, io: &mut dyn Console) -> Result<String, Stop> {
    loop {
        let missing = match session.forge.signed_in_as() {
            Probe::Read(login) => return Ok(login),
            Probe::NoTool(_) => true,
            Probe::Failed(_) => false,
        };
        header(io, session, 2, "Getting GitHub ready", true);
        io.say(&[
            p("  To check GitHub I use "),
            strong("gh"),
            p(", GitHub's own CLI."),
        ]);
        io.say(&[warn(if missing {
            "  It is not installed on this computer."
        } else {
            "  It is installed, and not signed in to GitHub."
        })]);
        blank(io, 2);
        let mut step = 1;
        if missing {
            io.say(&[
                strong(format!("  {step}  ")),
                p("Install it, in another terminal window"),
            ]);
            blank(io, 1);
            for command in install_commands(session.system) {
                io.say(&[word(format!("         {command}"))]);
            }
            blank(io, 2);
            step += 1;
        }
        io.say(&[
            strong(format!("  {step}  ")),
            p(if missing {
                "Sign in"
            } else {
                "Sign in, in another terminal window"
            }),
        ]);
        blank(io, 1);
        io.say(&[word("         gh auth login")]);
        blank(io, 2);
        io.say(&[
            p("  Press "),
            word("Enter"),
            p(" when that is done and I will check again."),
        ]);
        io.say(&[dim(
            "  back returns to the first question. q quits. Nothing has been changed.",
        )]);
        match read(io)?.as_str() {
            "back" | "b" => return Err(Stop::Back),
            _ => {}
        }
    }
}

/// How to install `gh`, for the system this is running on.
fn install_commands(system: &str) -> &'static [&'static str] {
    match system {
        "macOS" => &["brew install gh"],
        "Windows" => &["winget install --id GitHub.cli"],
        _ => &[
            "sudo apt install gh        (Debian, Ubuntu)",
            "sudo dnf install gh        (Fedora)",
        ],
    }
}

/// A GitHub login: letters, digits and hyphens. Anything else is not a name
/// GitHub would accept, and is not passed on to a command.
fn is_login(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 39
        && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn choose_owner(
    session: &Session,
    kind: OwnerKind,
    login: &str,
    io: &mut dyn Console,
) -> Result<String, Stop> {
    let organization = kind == OwnerKind::Organization;
    header(
        io,
        session,
        2,
        if organization {
            "Which organization?"
        } else {
            "Which account?"
        },
        true,
    );
    io.say(&[p("  Signed in to GitHub as "), strong(clean(login)), p(".")]);
    blank(io, 1);

    let mut listed: Vec<String> = Vec::new();
    if organization {
        match session.forge.organizations() {
            Probe::Read(orgs) if !orgs.is_empty() => {
                line(io, "  Your organizations:");
                blank(io, 1);
                let width = orgs.iter().map(|o| o.login.len()).max().unwrap_or(0);
                for org in &orgs {
                    let mut spans = vec![word(format!("      {:<width$}", clean(&org.login)))];
                    if let Some(n) = org.repositories {
                        spans.push(dim(format!(
                            "   {:>5} {}",
                            thousands(n),
                            if n == 1 { "repository" } else { "repositories" }
                        )));
                    }
                    io.say(&spans);
                    listed.push(org.login.clone());
                }
                blank(io, 2);
                line(io, "  Type the name of one, then press Enter.");
                io.say(&[dim("  Not listed? Type its name anyway.")]);
            }
            Probe::Read(_) => {
                line(io, "  You are not a member of any organization.");
                blank(io, 1);
                line(io, "  Type an organization's name, then press Enter.");
            }
            Probe::NoTool(why) | Probe::Failed(why) => {
                io.say(&[warn(format!(
                    "  I could not list your organizations: {}",
                    clean(&why)
                ))]);
                blank(io, 1);
                line(io, "  Type an organization's name, then press Enter.");
            }
        }
    } else {
        io.say(&[
            p("  Press "),
            word("Enter"),
            p(format!(" to check the repositories of {}.", clean(login))),
        ]);
        line(io, "  Or type a different account name, then press Enter.");
    }

    loop {
        // Not lowercased by `read`: a login keeps the case it is listed in.
        blank(io, 1);
        let typed = match io.ask() {
            None => return Err(Stop::Ended),
            Some(typed) => typed,
        };
        match typed.to_ascii_lowercase().as_str() {
            "q" | "quit" => return Err(Stop::Quit),
            "back" | "b" => return Err(Stop::Back),
            "" if !organization => return Ok(login.to_string()),
            "" => {
                blank(io, 1);
                io.say(&[warn("  Type the organization's name. q quits.")]);
            }
            lower => {
                if let Some(exact) = listed.iter().find(|l| l.to_ascii_lowercase() == lower) {
                    return Ok(exact.clone());
                }
                if is_login(&typed) {
                    return Ok(typed);
                }
                blank(io, 1);
                io.say(&[warn(
                    "  That is not a GitHub name: letters, digits and hyphens only.",
                )]);
            }
        }
    }
}

// --- step 3: progress, then what was found ------------------------------------

/// The standard progress screen: a bar, what it is on, what it has found so
/// far and how long it has run. Always the same number of lines, so a
/// terminal can redraw it in place.
fn progress_lines(progress: &Progress, seconds: u64, unicode: bool) -> Vec<Vec<Span>> {
    meter(
        &Meter {
            done: progress.done,
            total: progress.total,
            now: progress.now,
            so_far: format!("{} checked", count(progress.refs, "branch", "branches")),
            found: (progress.affected > 0).then(|| {
                format!(
                    "{} with the payload",
                    count(progress.affected, "repository", "repositories")
                )
            }),
        },
        seconds,
        unicode,
    )
}

fn summary(
    io: &mut dyn Console,
    session: &Session,
    owner: &str,
    evidence: &std::path::Path,
    found: &Findings,
) {
    io.say(&[p(format!(
        "  Checked {} and {} of {}.",
        count(found.repositories, "repository", "repositories"),
        count(found.refs, "branch", "branches"),
        clean(owner)
    ))]);
    io.say(&[dim(if evidence.starts_with(std::env::temp_dir()) {
        "  Copies are in a temporary folder, cleared on restart.".to_string()
    } else {
        format!("  Copies are in {}", tilde(evidence, session.home))
    })]);
    if !found.not_checked.is_empty() {
        blank(io, 1);
        io.say(&[warn(format!(
            "  {} could not be copied and {} NOT checked.",
            count(found.not_checked.len(), "repository", "repositories"),
            if found.not_checked.len() == 1 {
                "was"
            } else {
                "were"
            }
        ))]);
    }
    blank(io, 2);

    let affected = found.affected();
    if affected.is_empty() && found.review.is_empty() {
        io.say(&[
            good("      NOTHING FOUND"),
            p("     clean against today's indicators"),
        ]);
    } else {
        if affected.is_empty() {
            io.say(&[good("      CONFIRMED   0"), p("     nothing confirmed")]);
        } else {
            io.say(&[
                bad(format!("      CONFIRMED   {}", affected.len())),
                p(format!(
                    "     {} the PolinRider payload",
                    if affected.len() == 1 {
                        "repository carries"
                    } else {
                        "repositories carry"
                    }
                )),
            ]);
        }
        if !found.review.is_empty() {
            io.say(&[
                warn(format!("      TO REVIEW   {}", found.review.len())),
                p("     needs your eyes, may be nothing"),
            ]);
        }
    }
    if found.own_tooling > 0 {
        blank(io, 1);
        io.say(&[dim(format!(
            "  {} matched only your own detection files and {} set aside.",
            count(found.own_tooling, "branch", "branches"),
            if found.own_tooling == 1 {
                "was"
            } else {
                "were"
            }
        ))]);
    }
    if affected.is_empty() {
        return;
    }

    blank(io, 2);
    io.say(&[strong("  The payload is in")]);
    let width = affected.iter().map(|(r, _)| r.len()).max().unwrap_or(0);
    for (repository, branches) in &affected {
        io.say(&[p(format!(
            "      {:<width$}   {}",
            clean(repository),
            count(*branches, "branch", "branches")
        ))]);
    }
    blank(io, 1);
    let pushers = found.pushers();
    if pushers.is_empty() {
        io.say(&[strong("  Pushed by")]);
        io.say(&[dim("      GitHub no longer has the push record for these.")]);
    } else {
        io.say(&[strong("  Pushed by")]);
        io.say(&[p(format!("      {}", clean(&pushers.join(", "))))]);
        blank(io, 1);
        io.say(&[
            bad("  Their computers need checking too."),
            p(" A familiar name is"),
        ]);
        line(
            io,
            "  expected: the payload pushes as whoever is logged in.",
        );
    }
    blank(io, 2);
    io.say(&[warn("  Do not open these repositories in an editor,")]);
    io.say(&[warn("  and do not git pull an existing clone.")]);
}

/// Every confirmed and review finding: the repository and branch, then each
/// path on its own line.
pub(crate) fn details(io: &mut dyn Console, found: &Findings) {
    let mut show = |title: Span, findings: &[RefFinding], confirmed: bool| {
        if findings.is_empty() {
            return;
        }
        blank(io, 2);
        io.say(&[title]);
        for finding in findings {
            blank(io, 1);
            io.say(&[
                p(format!("      {}   ", clean(&finding.repository))),
                strong(clean(finding.short_ref())),
            ]);
            for path in paths_shown(finding, confirmed) {
                io.say(&[dim(format!("      {}", clean(&path)))]);
            }
        }
    };
    show(bad("  CONFIRMED"), &found.confirmed, true);
    show(warn("  TO REVIEW"), &found.review, false);
}

/// The paths that explain a finding, each with why when it is not obvious.
fn paths_shown(finding: &RefFinding, confirmed: bool) -> Vec<String> {
    if confirmed {
        return finding
            .real_paths()
            .into_iter()
            .map(str::to_owned)
            .collect();
    }
    let mut out: Vec<String> = Vec::new();
    for path in &finding.folder_open_tasks {
        out.push(format!("{path}   runs on folder open"));
    }
    for path in &finding.suspicious_configs {
        out.push(format!("{path}   code after the module end"));
    }
    for path in &finding.weak_files {
        out.push(format!(
            "{path}   a weak signal, also found in clean projects"
        ));
    }
    out
}

// --- step 4, and the report ---------------------------------------------------

fn names(list: &[&str]) -> String {
    match list {
        [] => String::new(),
        [one] => (*one).to_string(),
        [most @ .., last] => format!("{} and {last}", most.join(", ")),
    }
}

/// The last screen, written from what was found and what was done about it.
fn what_now(found: &Findings, outcomes: &[Outcome]) -> Vec<Vec<Span>> {
    let mut out: Vec<Vec<Span>> = Vec::new();
    let affected = found.affected();
    let unchecked = || -> Todo {
        let mut t = todo(
            "Check again for what could not be copied",
            &["These were NOT checked. Run polinrider again once they can be read."],
        );
        t.paths = found
            .not_checked
            .iter()
            .map(|(repository, _)| clean(repository))
            .collect();
        t
    };

    if affected.is_empty() {
        let mut todos: Vec<Todo> = Vec::new();
        if found.review.is_empty() {
            out.push(vec![if found.not_checked.is_empty() {
                good("  Nothing found.")
            } else {
                warn("  Nothing found in what could be checked.")
            }]);
            if found.not_checked.is_empty() {
                out.push(vec![]);
                for text in [
                    "  Clean against today's indicators is not proof that nothing",
                    "  was ever here. If you had a reason to check, change your",
                    "  passwords anyway.",
                ] {
                    out.push(vec![dim(text)]);
                }
            }
        } else {
            out.push(vec![
                warn("  Nothing is confirmed."),
                p(format!(
                    " {} your eyes.",
                    if found.review.len() == 1 {
                        "1 branch needs".to_string()
                    } else {
                        format!("{} branches need", found.review.len())
                    }
                )),
            ]);
            todos.push(todo(
                "Look at each file listed under details",
                &["Ask whether you put it there."],
            ));
            todos.push(todo(
                "If one of them is not yours, treat it as confirmed",
                &["Change your passwords from a clean computer first."],
            ));
        }
        if !found.not_checked.is_empty() {
            todos.push(unchecked());
        }
        numbered(&mut out, &todos);
        return out;
    }

    let fixed = outcomes.iter().filter(|o| o.is_fixed()).count();
    let archived: Vec<&str> = outcomes
        .iter()
        .filter(|o| o.what == What::Archived)
        .map(|o| o.repository.as_str())
        .collect();
    // Everything that is neither fixed nor archived still carries the
    // payload and is still in use.
    let open: Vec<&str> = affected
        .iter()
        .map(|(repository, _)| *repository)
        .filter(|repository| {
            !outcomes
                .iter()
                .any(|o| o.repository == *repository && (o.is_fixed() || o.what == What::Archived))
        })
        .collect();
    let repositories = |n: usize| count(n, "repository", "repositories");

    out.push(if fixed == 0 && archived.is_empty() {
        vec![
            bad(format!(
                "  {} on GitHub {} the payload.",
                repositories(affected.len()),
                if affected.len() == 1 {
                    "carries"
                } else {
                    "carry"
                }
            )),
            p(" Do these in order."),
        ]
    } else if open.is_empty() && archived.is_empty() {
        vec![
            good(match affected.len() {
                1 => "  The repository is fixed.".to_string(),
                2 => "  Both repositories are fixed.".to_string(),
                n => format!("  All {n} repositories are fixed."),
            }),
            p(" Do these next, in order."),
        ]
    } else {
        let mut said = if fixed == 0 {
            "  No repository is fixed yet.".to_string()
        } else {
            format!(
                "  {fixed} of {} {} fixed.",
                repositories(affected.len()),
                if fixed == 1 { "is" } else { "are" }
            )
        };
        if !archived.is_empty() {
            said.push_str(&format!(" {} archived.", archived.len()));
        }
        vec![
            if open.is_empty() {
                good(said)
            } else {
                warn(said)
            },
            p(" Do these in order."),
        ]
    });

    let mut todos: Vec<Todo> = Vec::new();
    if !open.is_empty() {
        let mut t = todo(
            "Do not open or pull what is not fixed",
            &[
                "Opening one in an editor can run the payload. A pull or",
                "a push from an old clone spreads it.",
            ],
        );
        t.paths = open.iter().map(|r| clean(r)).collect();
        todos.push(t);
    }
    let pushers = found.pushers();
    let how = [
        "Run polinrider on each and choose computer. A clean",
        "GitHub and an infected laptop is clean for as long as",
        "the next push takes.",
    ];
    todos.push(if pushers.is_empty() {
        todo(
            "Check every computer that pushes to these repositories",
            &how,
        )
    } else {
        todo(
            format!("Check the computers of {}", clean(&names(&pushers))),
            &how,
        )
    });
    todos.push(todo(
        "Change passwords and keys, from a clean computer",
        &["GitHub tokens and SSH keys, npm tokens, cloud keys."],
    ));
    if !open.is_empty() {
        let mut t = todo(
            match open.as_slice() {
                [one] => format!("Fix {}, which still carries the payload", clean(one)),
                many => format!(
                    "Fix the {} that still carry the payload",
                    repositories(many.len())
                ),
            },
            &["Run polinrider again and choose a fix for each."],
        );
        if open.len() > 1 {
            t.paths = open.iter().map(|r| clean(r)).collect();
        }
        for o in outcomes {
            if let What::Failed(why) = &o.what {
                t.how
                    .push(format!("{}: {}", clean(&o.repository), clean(why)));
            }
        }
        todos.push(t);
    }

    let used = |fix: Fix| {
        outcomes
            .iter()
            .any(|o| matches!(o.what, What::Fixed { fix: f, moved, .. } if f == fix && moved > 0))
    };
    if used(Fix::Restore) || used(Fix::Erase) {
        todos.push(todo(
            "Delete old clones and clone again",
            &[
                "A push from an old clone puts the payload back. To",
                "update a clone and keep it, the steps are in the report.",
            ],
        ));
        todos.push(todo(
            "Ask GitHub Support to clear the old commits",
            &[
                "GitHub keeps the commits a fix moved away from. For a",
                "while they can still be fetched by ID, and from a pull",
                "request. Ask Support for a garbage collection of each",
                "repository that was fixed.",
            ],
        ));
    } else if used(Fix::Remove) {
        todos.push(todo(
            "Pull the fix into every clone, once its computer is checked",
            &[
                "The payload is still in older commits. Do not check",
                "one out.",
            ],
        ));
    }
    if !archived.is_empty() {
        let mut t = todo(
            "Tell people the archived repositories are not to be used",
            &["The payload is still inside. The README says so."],
        );
        t.paths = archived.iter().map(|r| clean(r)).collect();
        todos.push(t);
    }
    if !found.not_checked.is_empty() {
        todos.push(unchecked());
    }
    numbered(&mut out, &todos);
    out
}

/// The whole of it, for the report file: every branch and every path.
fn written(owner: &str, session: &Session, found: &Findings) -> String {
    let mut out = format!(
        "\nGitHub check of {}\n  answers from: {}\n  repositories: {}\n  branches and tags: {}\n",
        clean(owner),
        session.forge.describe(),
        found.repositories,
        found.refs
    );
    for (repository, why) in &found.not_checked {
        out.push_str(&format!(
            "  NOT CHECKED  {}  ({})\n",
            clean(repository),
            clean(why)
        ));
    }
    for repository in &found.no_push_record {
        out.push_str(&format!(
            "  NO PUSH RECORD  {}  (missing evidence, not an absence of pushes)\n",
            clean(repository)
        ));
    }
    for (title, findings, confirmed) in [
        ("CONFIRMED", &found.confirmed, true),
        ("TO REVIEW", &found.review, false),
    ] {
        if findings.is_empty() {
            continue;
        }
        out.push_str(&format!("\n{title}\n"));
        for finding in findings {
            out.push_str(&format!(
                "  {}  {}\n",
                clean(&finding.repository),
                clean(&finding.git_ref)
            ));
            for path in paths_shown(finding, confirmed) {
                out.push_str(&format!("      {}\n", clean(&path)));
            }
        }
    }
    if !found.pushes.is_empty() {
        out.push_str("\nPUSHES TO CONFIRMED BRANCHES\n");
        for (repository, push) in &found.pushes {
            out.push_str(&format!(
                "  {}  {}  {}  {}..{}  by {}{}\n",
                clean(&push.at),
                clean(repository),
                clean(&push.git_ref),
                clean(&push.before),
                clean(&push.head),
                clean(&push.actor),
                if push.size == 0 {
                    "  (no commits: a force-push)"
                } else {
                    ""
                }
            ));
        }
    }
    out.push('\n');
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::guide::{run as run_session, Outcome};
    use crate::indicators::Indicators;
    use crate::remote::fixture::World;
    use std::collections::VecDeque;
    use std::fs;

    const STRONG: &str = "MARKER-ALPHA";

    struct Script {
        answers: VecDeque<String>,
        said: String,
        /// How many progress updates were shown, and whether the last one
        /// was marked as the last.
        progress: (usize, bool),
    }

    impl Console for Script {
        fn say(&mut self, line: &[Span]) {
            self.said.push_str(&text_of(line));
            self.said.push('\n');
        }
        fn ask(&mut self) -> Option<String> {
            self.said.push_str("  > \n");
            self.answers.pop_front()
        }
        fn progress(&mut self, lines: &[Vec<Span>], last: bool) {
            self.progress = (self.progress.0 + 1, last);
            if last {
                for line in lines {
                    self.say(line);
                }
            }
        }
    }

    fn session_run(w: &World, answers: &[&str]) -> (Outcome, Script) {
        session_on(w, None, answers)
    }

    fn session_on(
        w: &World,
        host: Option<&dyn crate::host::Host>,
        answers: &[&str],
    ) -> (Outcome, Script) {
        let ind = Indicators {
            strong: vec![STRONG.into()],
            ..Indicators::default()
        };
        let forge = w.forge();
        let home = w.dir.join("home");
        fs::create_dir_all(&home).expect("mkdir");
        let session = Session {
            ind: &ind,
            ioc_dir: &w.dir.join("ioc"),
            home: &home,
            host,
            quarantine: &w.dir.join("q"),
            report: &home.join("polinrider-report.txt"),
            system: "Linux",
            unicode: false,
            forge: &forge,
            evidence: &w.dir.join("evidence"),
            stamp: "20261003T120000Z",
        };
        let mut io = Script {
            answers: answers.iter().map(|a| (*a).to_string()).collect(),
            said: String::new(),
            progress: (0, false),
        };
        let outcome = run_session(&session, &mut io);
        (outcome, io)
    }

    fn infected() -> Vec<u8> {
        format!(
            "export default {{}}\n{}var x='{STRONG}';\n",
            " ".repeat(280)
        )
        .into_bytes()
    }

    fn acme() -> World {
        let w = World::new(&format!(
            "guide-{}",
            std::thread::current()
                .name()
                .unwrap_or("t")
                .replace("::", "-")
        ));
        w.repo("blog", &[("main", &[("index.md", b"hello\n" as &[u8])])]);
        w.repo(
            "shop",
            &[
                ("main", &[("a.js", b"ok\n" as &[u8])]),
                ("release", &[("vite.config.js", &infected())]),
            ],
        );
        w.pushes(
            "shop",
            "refs/heads/release\taaa\tbbb\talice\t2026-09-12T10:00:00Z\t0\n\
             refs/heads/release\tbbb\tccc\tbob\t2026-09-13T10:00:00Z\t1\n",
        );
        fs::write(w.dir.join("forge/orgs"), "acme\t2\nacme-labs\t7\n").expect("write");
        w
    }

    #[test]
    fn an_organization_is_chosen_from_a_list_checked_and_summarised() {
        let w = acme();
        let (outcome, io) = session_run(&w, &["organization", "ACME", "none"]);
        assert_eq!(outcome.exit, ExitCode::Confirmed, "{}", io.said);
        // The list, and a name typed in the wrong case still finds it.
        assert!(io.said.contains("Signed in to GitHub as tester."));
        assert!(io.said.contains("      acme"));
        assert!(io.said.contains("7 repositories"));
        // Progress: once per repository and once at the end.
        assert_eq!(io.progress, (3, true));
        assert!(io.said.contains("2 of 2"), "{}", io.said);
        // The summary, in plain words.
        assert!(io
            .said
            .contains("Checked 2 repositories and 3 branches of acme."));
        assert!(io
            .said
            .contains("CONFIRMED   1     repository carries the PolinRider payload"));
        assert!(io.said.contains("      acme/shop   1 branch"));
        assert!(io.said.contains("      alice, bob"));
        assert!(io.said.contains("Their computers need checking too."));
        assert!(!io.said.contains("vite.config.js"), "no path until details");
        // The last screen.
        assert!(io.said.contains("Check the computers of alice and bob"));
        assert!(io
            .said
            .contains("Left as it is. Nothing on GitHub was changed."));
        assert!(io
            .said
            .contains("Fix acme/shop, which still carries the payload"));
        // The report has what the screens left out.
        assert!(outcome.report.contains("acme/shop  refs/heads/release"));
        assert!(outcome.report.contains("vite.config.js"));
        assert!(outcome.report.contains("a force-push"));
    }

    #[test]
    fn details_lists_every_branch_and_file_and_changes_nothing() {
        let w = acme();
        let (_, io) = session_run(&w, &["org", "acme", "details", "none"]);
        assert!(io.said.contains("  CONFIRMED\n"));
        assert!(io.said.contains("      acme/shop   release\n"));
        assert!(io.said.contains("      vite.config.js\n"));
    }

    #[test]
    fn an_account_defaults_to_whoever_is_signed_in() {
        let w = acme();
        fs::write(w.dir.join("forge/repos/tester"), "").expect("write");
        let (outcome, io) = session_run(&w, &["account", ""]);
        assert!(io
            .said
            .contains("Press Enter to check the repositories of tester."));
        assert!(io
            .said
            .contains("Checked 0 repositories and 0 branches of tester."));
        assert!(io.said.contains("NOTHING FOUND"));
        assert_eq!(outcome.exit, ExitCode::Clean);
    }

    #[test]
    fn a_missing_gh_gets_install_and_sign_in_steps_and_is_checked_again() {
        let w = acme();
        fs::write(w.dir.join("forge/whoami.absent"), "").expect("write");
        // Enter twice with nothing fixed, then back out and leave.
        let (outcome, io) = session_run(&w, &["organization", "", "", "back", "q"]);
        assert_eq!(io.said.matches("Getting GitHub ready").count(), 3);
        assert!(io
            .said
            .contains("To check GitHub I use gh, GitHub's own CLI."));
        assert!(io.said.contains("It is not installed on this computer."));
        assert!(io.said.contains("sudo apt install gh"));
        assert!(io.said.contains("         gh auth login"));
        assert_eq!(
            io.said.matches("STEP 1 OF 4").count(),
            2,
            "back returns to step 1"
        );
        assert_eq!(outcome.exit, ExitCode::CouldNotRun);
    }

    #[test]
    fn a_signed_out_gh_is_told_to_sign_in_and_not_to_install() {
        let w = acme();
        fs::write(w.dir.join("forge/whoami"), "\n").expect("write");
        let (_, io) = session_run(&w, &["organization", "q"]);
        assert!(io
            .said
            .contains("It is installed, and not signed in to GitHub."));
        assert!(!io.said.contains("Install it"));
        assert!(io.said.contains("gh auth login"));
    }

    #[test]
    fn a_name_that_is_not_a_github_name_is_never_passed_on() {
        let w = acme();
        let (_, io) = session_run(&w, &["organization", "acme; rm -rf ~", "", "q"]);
        assert!(io.said.contains("That is not a GitHub name"));
        assert!(io.said.contains("Type the organization's name."));
        assert!(!io.said.contains("Checking "));
    }

    #[test]
    fn a_repository_that_could_not_be_copied_is_not_a_clean_result() {
        let w = World::new("guide-not-checked");
        w.repo("blog", &[("main", &[("index.md", b"hello\n" as &[u8])])]);
        let list = w.dir.join("forge/repos/acme");
        let mut repos = fs::read_to_string(&list).expect("read");
        repos.push_str("acme/ghost\n");
        fs::write(list, repos).expect("write");
        fs::write(w.dir.join("forge/orgs"), "acme\t2\n").expect("write");

        let (outcome, io) = session_run(&w, &["organization", "acme"]);
        assert!(io
            .said
            .contains("1 repository could not be copied and was NOT checked."));
        assert!(io.said.contains("Nothing found in what could be checked."));
        assert!(io.said.contains("     acme/ghost\n"), "{}", io.said);
        assert_eq!(outcome.exit, ExitCode::CouldNotRun);
    }

    /// An organization with real history behind it. shop was clean, was
    /// force-pushed with the payload by alice, and was pushed to again by
    /// bob. website has carried the payload for longer than GitHub remembers.
    fn attacked(name: &str) -> World {
        let w = World::new(name);
        w.repo(
            "shop",
            &[(
                "main",
                &[("postcss.config.mjs", b"export default {}\n" as &[u8])],
            )],
        );
        w.push(
            "shop",
            "main",
            &[("postcss.config.mjs", &infected())],
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
        w.repo(
            "website",
            &[(
                "main",
                &[
                    ("vite.config.js", &infected() as &[u8]),
                    ("README.md", b"# Website\n"),
                ],
            )],
        );
        fs::write(w.dir.join("forge/orgs"), "acme\t2\n").expect("write");
        w
    }

    fn carries(w: &World, name: &str) -> bool {
        let log = crate::remote::fixture::git_out(
            &w.bare(name),
            &["log", "--branches", "--tags", "-p", "--format=%H"],
        );
        log.contains(STRONG) || log.contains("inter.woff2")
    }

    #[test]
    fn each_repository_is_fixed_only_after_its_own_dry_run_and_a_yes() {
        let w = attacked("guide-each");
        let (outcome, io) = session_run(
            &w,
            &[
                "organization",
                "acme",
                "each",
                // shop: restore is offered, shown, and done on a yes.
                "restore",
                "yes",
                "",
                // website: no record, so restore is not on the screen.
                "restore",
                "erase",
                "yes",
                "",
            ],
        );
        assert!(
            io.said.contains("  REPOSITORY 1 OF 2   acme/shop"),
            "{}",
            io.said
        );
        assert!(io.said.contains("Record found. Commit "));
        assert!(io.said.contains("checked clean."));
        assert!(io.said.contains("  acme/shop   restore"));
        assert!(io.said.contains("undoes the push of 11 September by alice"));
        assert!(io
            .said
            .contains("2 commits pushed since then stop being reachable."));
        assert!(io.said.contains("  Done. 1 branch of acme/shop put back."));
        assert!(io
            .said
            .contains("  Checked on GitHub afterwards: it matches."));
        assert_eq!(
            w.file("shop", "main", "postcss.config.mjs").as_deref(),
            Some("export default {}\n")
        );

        assert!(io.said.contains("  REPOSITORY 2 OF 2   acme/website"));
        assert!(io.said.contains("  restore is not possible here:"));
        assert!(io.said.contains("  GitHub no longer has the push record."));
        assert!(io
            .said
            .contains("  restore is not possible for this repository."));
        assert!(io.said.contains("1 file is taken out of every commit:"));
        assert!(io.said.contains("Every commit gets a new ID: 2 in all."));
        assert!(io.said.contains("1 of them changes these files."));
        assert!(io
            .said
            .contains("  Done. The payload is out of the history of acme/website."));
        assert!(io
            .said
            .contains("  Every existing clone of acme/website now needs updating."));
        assert!(io.said.contains("         git reset --hard origin/main"));
        assert!(!carries(&w, "website"));
        // The config came back clean, as one commit.
        assert_eq!(
            w.file("website", "main", "vite.config.js").as_deref(),
            Some("export default {}\n")
        );

        assert!(io.said.contains("  Both repositories are fixed."));
        assert!(io.said.contains("Delete old clones and clone again"));
        assert!(io
            .said
            .contains("Ask GitHub Support to clear the old commits"));
        assert!(!io.said.contains("Do not open or pull what is not fixed"));
        // The check found the payload, and the exit says so whatever was
        // done about it afterwards.
        assert_eq!(outcome.exit, ExitCode::Confirmed);
        assert!(outcome.report.contains("FIX  acme/shop  restore"));
        assert!(outcome
            .report
            .contains("done, checked on GitHub: refs/heads/main"));
        assert!(outcome.report.contains("FIX  acme/website  erase"));
        assert!(outcome
            .report
            .contains("removed from every commit: vite.config.js"));
        assert!(outcome.report.contains("git reset --hard origin/main"));
    }

    #[test]
    fn nothing_but_yes_pushes_and_no_or_skip_leave_github_alone() {
        let w = attacked("guide-no");
        let before = (
            w.tip("shop", "refs/heads/main"),
            w.tip("website", "refs/heads/main"),
        );
        let (_, io) = session_run(
            &w,
            &[
                "organization",
                "acme",
                "fix",
                // Enter, a near miss, then no: each asks again or backs out.
                "restore",
                "",
                "sure",
                "no",
                "skip",
                "remove",
                "y",
                "no",
                "skip",
            ],
        );
        assert_eq!(
            io.said
                .matches("  Type yes or no. Enter is not yes. q quits.")
                .count(),
            3
        );
        assert_eq!(io.said.matches("  Nothing was changed.").count(), 2);
        assert!(!io.said.contains("Pushing to GitHub"));
        assert!(!io.said.contains("  Done."));
        assert_eq!(
            before,
            (
                w.tip("shop", "refs/heads/main"),
                w.tip("website", "refs/heads/main"),
            )
        );
        assert!(io
            .said
            .contains("2 repositories on GitHub carry the payload."));
        assert!(io
            .said
            .contains("Fix the 2 repositories that still carry the payload"));
    }

    #[test]
    fn quitting_at_the_yes_question_pushes_nothing() {
        let w = attacked("guide-quit");
        let before = w.tip("shop", "refs/heads/main");
        let (_, io) = session_run(&w, &["organization", "acme", "each", "erase", "q"]);
        assert!(io.said.contains("Nothing has been pushed yet."));
        assert_eq!(before, w.tip("shop", "refs/heads/main"));
        // Input that simply ends is not a yes either.
        let (_, _) = session_run(&w, &["organization", "acme", "each", "erase"]);
        assert_eq!(before, w.tip("shop", "refs/heads/main"));
    }

    #[test]
    fn all_at_once_needs_the_owners_name_and_yes_is_not_enough() {
        let w = attacked("guide-all");
        let (_, io) = session_run(
            &w,
            &[
                "organization",
                "acme",
                "all",
                "remove",
                "yes",
                "",
                "acme",
                "",
            ],
        );
        assert!(io.said.contains("  ALL 2 REPOSITORIES   acme"));
        assert!(io
            .said
            .contains("  This is a big step. Read this before you answer."));
        assert!(io
            .said
            .contains("It changes 2 repositories and 2 branches and tags on GitHub,"));
        assert!(io
            .said
            .contains("  Type the organization's name to go ahead:  acme"));
        assert_eq!(
            io.said
                .matches("  Type acme to go ahead, or no. yes is not enough here.")
                .count(),
            2
        );
        assert!(io
            .said
            .contains("      acme/shop      done, 1 branch or tag checked on GitHub"));
        assert_eq!(
            w.file("shop", "main", "postcss.config.mjs").as_deref(),
            Some("export default {}\n")
        );
        assert_eq!(w.file("shop", "main", "public/fonts/inter.woff2"), None);
        assert_eq!(
            w.file("website", "main", "vite.config.js").as_deref(),
            Some("export default {}\n")
        );
        assert!(io.said.contains("  Both repositories are fixed."));
        assert!(io
            .said
            .contains("Pull the fix into every clone, once its computer is checked"));
    }

    #[test]
    fn all_at_once_leaves_alone_what_cannot_take_the_fix_and_says_which() {
        let w = attacked("guide-all-restore");
        let website = w.tip("website", "refs/heads/main");
        let (_, io) = session_run(&w, &["organization", "acme", "all", "restore", "ACME", ""]);
        assert!(io.said.contains("      1 can take restore:"));
        assert!(io.said.contains("      1 cannot, and will be left alone:"));
        assert!(io.said.contains("          acme/website"));
        assert!(io
            .said
            .contains("          GitHub no longer has the push record"));
        assert!(io.said.contains("      acme/website   left alone"));
        assert_eq!(website, w.tip("website", "refs/heads/main"));
        assert!(!carries(&w, "shop"));
        assert!(io.said.contains("  1 of 2 repositories is fixed."));
        assert!(io
            .said
            .contains("Fix acme/website, which still carries the payload"));
        assert!(io.said.contains("Do not open or pull what is not fixed"));

        // Backing out at the name changes nothing and returns to the choice.
        let w = attacked("guide-all-back");
        let shop = w.tip("shop", "refs/heads/main");
        let (_, io) = session_run(&w, &["organization", "acme", "all", "erase", "no", "none"]);
        assert_eq!(
            io.said
                .matches("  Type each, all, details or none.")
                .count(),
            2
        );
        assert_eq!(shop, w.tip("shop", "refs/heads/main"));
    }

    #[test]
    fn archive_shows_the_notice_before_the_question_and_says_the_payload_stays() {
        let w = attacked("guide-archive");
        let (_, io) = session_run(
            &w,
            &["organization", "acme", "each", "skip", "archive", "yes", ""],
        );
        assert!(io.said.contains("  acme/website   archive"));
        assert!(io
            .said
            .contains("A notice is added to the top of README.md, as one"));
        assert!(io
            .said
            .contains("new commit on main. Nothing in the file is removed."));
        assert!(io
            .said
            .contains("\"INFECTED with PolinRider malware. Do not clone or use.\""));
        assert!(io
            .said
            .contains("  The payload stays inside it. Anyone who clones it still"));
        assert!(io.said.contains(
            "      # INFECTED WITH MALWARE. DO NOT CLONE, OPEN OR BUILD THIS REPOSITORY."
        ));
        assert!(io.said.contains("      > [!CAUTION]"));
        assert!(io.said.contains("  Type yes to archive acme/website."));
        assert!(io.said.contains("  Done. acme/website is archived,"));

        let readme = w.file("website", "main", "README.md").expect("readme");
        assert!(readme.starts_with("# INFECTED WITH MALWARE."));
        assert!(readme.ends_with("\n---\n\n# Website\n"));
        assert!(w.dir.join("forge/changed/acme/website.archived").exists());

        assert!(io
            .said
            .contains("  No repository is fixed yet. 1 archived."));
        assert!(io
            .said
            .contains("Tell people the archived repositories are not to be used"));
        assert!(io
            .said
            .contains("Fix acme/shop, which still carries the payload"));
    }

    #[test]
    fn everything_checks_this_computer_and_then_github_and_keeps_the_worse_result() {
        use crate::host::{Platform, Snapshot};
        let w = attacked("guide-everything");
        fs::write(w.dir.join("forge/repos/tester"), "").expect("write");
        let host = Snapshot::quiet(Platform::Linux, w.dir.join("root"));
        let (outcome, io) = session_on(
            &w,
            Some(&host),
            &[
                "everything",
                "",
                // The computer is clean. Then GitHub: a wrong word, the
                // organization, then the account, then done.
                "github",
                "organization",
                "acme",
                "none",
                "account",
                "",
                "done",
            ],
        );
        assert!(io
            .said
            .contains("      everything     This computer first, then GitHub."));
        assert!(
            io.said.contains("STEP 3 OF 4   Checking this computer"),
            "{}",
            io.said
        );
        assert!(io.said.contains("11 of 11"));
        assert!(io.said.contains("  NEXT   GitHub"));
        assert!(io.said.contains("  This computer is done. GitHub is next."));
        assert!(io
            .said
            .contains("  Type organization, account or done. q quits."));
        assert!(io
            .said
            .contains("Checked 2 repositories and 2 branches of acme."));
        assert!(io.said.contains("  NEXT   More on GitHub?"));
        assert!(io
            .said
            .contains("Checked 0 repositories and 0 branches of tester."));
        // Clean computer, clean account, infected organization: the session
        // found a payload, and a later clean check does not take that back.
        assert_eq!(outcome.exit, ExitCode::Confirmed);
        assert!(outcome.report.contains("first check, read-only"));
        assert!(outcome.report.contains("GitHub check of acme"));
        assert!(outcome.report.contains("GitHub check of tester"));
    }

    #[test]
    fn everything_can_stop_after_the_computer_and_is_not_offered_without_one() {
        use crate::host::{Platform, Snapshot};
        let w = attacked("guide-everything-done");
        let host = Snapshot::quiet(Platform::Linux, w.dir.join("root"));
        let (outcome, io) = session_on(&w, Some(&host), &["everything", "", "done"]);
        assert!(io
            .said
            .contains("      done           Stop here. GitHub is not checked."));
        assert!(!io.said.contains("Signed in to GitHub"));
        assert_eq!(outcome.exit, ExitCode::Clean);

        // A build that cannot read this computer does not offer to.
        let (outcome, io) = session_run(&w, &["everything", "q"]);
        assert!(!io.said.contains("      everything"));
        assert!(io
            .said
            .contains("That is not available here. Type one of the others."));
        assert_eq!(outcome.exit, ExitCode::CouldNotRun);
    }

    #[test]
    fn the_progress_block_is_always_the_same_height() {
        let at = |done, affected| {
            progress_lines(
                &Progress {
                    done,
                    total: 42,
                    now: "acme/website",
                    refs: 96,
                    affected,
                },
                130,
                true,
            )
        };
        let (start, middle, end) = (at(0, 0), at(14, 1), at(42, 1));
        assert_eq!(start.len(), middle.len());
        assert_eq!(middle.len(), end.len());
        let text: Vec<String> = middle.iter().map(|l| text_of(l)).collect();
        assert!(text[0].ends_with("14 of 42"), "{}", text[0]);
        assert_eq!(text[0].matches('█').count(), 12);
        assert_eq!(text[0].matches('░').count(), 24);
        assert!(text.contains(&"      now        acme/website".to_string()));
        assert!(text.contains(&"                 1 repository with the payload".to_string()));
        assert!(text.contains(&"      running    2 minutes".to_string()));
        assert!(text_of(&end[2]).ends_with("finished"));
    }
}
