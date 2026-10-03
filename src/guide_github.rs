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
//! This stage reads and reports. It changes nothing on GitHub. The fixes
//! arrive after it, each put on the screen only when it works.

use crate::guide::{
    bad, blank, count, dim, good, header, line, numbered, p, read, strong, thousands, tilde, todo,
    warn, word, Console, Session, Stop, Todo,
};
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

    if !found.confirmed.is_empty() || !found.review.is_empty() {
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
    for spans in what_now(&owner, kind, &found) {
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
    const WIDTH: usize = 36;
    // Nothing to do is a full bar, not a division by zero.
    let filled = (progress.done * WIDTH)
        .checked_div(progress.total)
        .map_or(WIDTH, |cells| cells.min(WIDTH));
    let (full, empty) = if unicode { ("█", "░") } else { ("#", ".") };
    let running = match seconds / 60 {
        0 => "less than a minute".to_string(),
        m => count(m as usize, "minute", "minutes"),
    };
    vec![
        vec![
            p("      "),
            word(full.repeat(filled)),
            dim(empty.repeat(WIDTH - filled)),
            p(format!("   {} of {}", progress.done, progress.total)),
        ],
        vec![],
        vec![p(format!(
            "      now        {}",
            if progress.done == progress.total {
                "finished".to_string()
            } else {
                clean(progress.now)
            }
        ))],
        vec![p(format!(
            "      so far     {} checked",
            count(progress.refs, "branch", "branches")
        ))],
        vec![
            p("                 "),
            if progress.affected == 0 {
                dim("nothing found yet")
            } else {
                bad(format!(
                    "{} with the payload",
                    count(progress.affected, "repository", "repositories")
                ))
            },
        ],
        vec![p(format!("      running    {running}"))],
    ]
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
fn details(io: &mut dyn Console, found: &Findings) {
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

fn what_now(owner: &str, kind: OwnerKind, found: &Findings) -> Vec<Vec<Span>> {
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

    out.push(vec![
        bad(format!(
            "  {} on GitHub {} the payload.",
            count(affected.len(), "repository", "repositories"),
            if affected.len() == 1 {
                "carries"
            } else {
                "carry"
            }
        )),
        p(" Do these in order."),
    ]);

    let pushers = found.pushers();
    let mut todos = vec![
        todo(
            "Do not open or pull these repositories",
            &[
                "Opening one in an editor can run the payload. A pull or",
                "a push from an old clone spreads it.",
            ],
        ),
        if pushers.is_empty() {
            todo(
                "Check every computer that pushes to these repositories",
                &[
                    "Run polinrider on each and choose computer. A clean",
                    "GitHub and an infected laptop is clean for as long as",
                    "the next push takes.",
                ],
            )
        } else {
            todo(
                format!("Check the computers of {}", clean(&names(&pushers))),
                &[
                    "Run polinrider on each and choose computer. A clean",
                    "GitHub and an infected laptop is clean for as long as",
                    "the next push takes.",
                ],
            )
        },
        todo(
            "Change passwords and keys, from a clean computer",
            &["GitHub tokens and SSH keys, npm tokens, cloud keys."],
        ),
    ];
    // Until the fixes are built, say so plainly and name what does work.
    let released = format!(
        "Until then the released tool does it: ./polinrider.sh {} {}",
        if kind == OwnerKind::Organization {
            "--org"
        } else {
            "--user"
        },
        clean(owner)
    );
    todos.push(todo(
        format!(
            "Fix the {}",
            if affected.len() == 1 {
                "repository".to_string()
            } else {
                format!("{} repositories", affected.len())
            }
        ),
        &[
            "Fixing them from this screen is the next part of this beta.",
            released.as_str(),
        ],
    ));
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
            host: None,
            quarantine: &w.dir.join("q"),
            report: &home.join("polinrider-report.txt"),
            system: "Linux",
            unicode: false,
            forge: &forge,
            evidence: &w.dir.join("evidence"),
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
        let (outcome, io) = session_run(&w, &["organization", "ACME", "", ""]);
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
        assert!(io.said.contains("./polinrider.sh --org acme"));
        // The report has what the screens left out.
        assert!(outcome.report.contains("acme/shop  refs/heads/release"));
        assert!(outcome.report.contains("vite.config.js"));
        assert!(outcome.report.contains("a force-push"));
    }

    #[test]
    fn details_lists_every_branch_and_file_and_changes_nothing() {
        let w = acme();
        let (_, io) = session_run(&w, &["org", "acme", "details", ""]);
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
