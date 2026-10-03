//! The filesystem checks.
//!
//! Two of them, the implant and the git configuration, have a half that reads
//! the machine rather than the disk. That half arrives as an optional
//! [`Host`]: `None` under `--fs-only`, and the check says what it did not read.
//!
//! Every message here is matched verbatim by the conformance corpus, which is
//! the specification both implementations answer to. Changing a string is a
//! behaviour change, and the corpus will say so.

use crate::host::Host;
use crate::host_checks::{self, sh_quote, ProcessCheck};
use crate::indicators::Indicators;
use crate::quarantine::{Apply, DryRun, Quarantine};
use crate::strip::{self, Plan};
use crate::verdict::{Finding, Kind, Verdict};
use crate::walk::Walk;
use std::fs;
use std::path::{Path, PathBuf};

/// Where confirmed artifacts go, if anywhere. The two variants are different
/// types, so a dry run has no way to move a file. See `quarantine`.
pub enum Sink<'a> {
    Dry(&'a Quarantine<DryRun>),
    Apply(&'a mut Quarantine<Apply>),
}

impl Sink<'_> {
    /// Where this run's quarantine is, or would be.
    pub fn root(&self) -> &Path {
        match self {
            Sink::Dry(q) => q.root(),
            Sink::Apply(q) => q.root(),
        }
    }

    /// Record a confirmed artifact. Returns the line to print under it.
    pub(crate) fn take(&mut self, path: &Path, reason: &str) -> String {
        match self {
            Sink::Dry(q) => q.would_take(path).line(),
            Sink::Apply(q) => match q.take(path, reason) {
                Ok(o) => o.line(),
                Err(e) => format!("QUARANTINE FAILED ({e}): {}", path.display()),
            },
        }
    }

    /// Cut a payload out of a file, keeping the original. Returns the line to
    /// print under the finding. A dry run has no way to do it: `strip` does
    /// not exist on `Quarantine<DryRun>`.
    fn strip(&mut self, path: &Path, plan: &Plan) -> String {
        let outcome = match self {
            Sink::Dry(q) => q.would_strip(path).line(),
            Sink::Apply(q) => match q.strip(path, &plan.keep, "stripped-config") {
                Ok(o) => o.line(),
                Err(e) => {
                    return format!(
                        "STRIP FAILED ({e}), the file is unchanged: {}",
                        path.display()
                    )
                }
            },
        };
        let verb = match self {
            Sink::Dry(_) => "would strip",
            Sink::Apply(_) => "stripped",
        };
        format!(
            "{verb} {} bytes appended after line {}\n{outcome}",
            plan.removed, plan.last_kept_line
        )
    }
}

/// The config filenames the campaign appends to.
pub(crate) fn is_build_config(name: &str) -> bool {
    const STEMS: &[&str] = &[
        "postcss.config",
        "tailwind.config",
        "eslint.config",
        "vite.config",
        "next.config",
        "rollup.config",
        "webpack.config",
        "babel.config",
        "gridsome.config",
        "vue.config",
    ];
    name == "truffle.js"
        || STEMS
            .iter()
            .any(|s| name.starts_with(s) && name[s.len()..].starts_with('.'))
}

fn is_manifest(name: &str) -> bool {
    matches!(
        name,
        "package.json" | "package-lock.json" | "pnpm-lock.yaml" | "yarn.lock"
    )
}

/// Obfuscation tells, used to qualify a "content after the module end"
/// finding. Without this a flat config that legitimately runs long is flagged.
fn looks_like_payload(tail: &str) -> bool {
    const TELLS: &[&str] = &[
        "eval(",
        "new Function(",
        "Buffer.from(",
        "child_process",
        "atob(",
        "fromCharCode",
    ];
    TELLS.iter().any(|t| tail.contains(t)) || tail.lines().any(|l| l.len() > 500)
}

/// A config whose module ends and is then followed by a long remainder that
/// looks like a payload. Returns the line count and the line the module ends
/// on. Shared by the machine check and the remote one, so the two cannot come
/// to disagree about what a suspicious config looks like.
pub(crate) fn payload_shaped_tail(body: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = body.lines().collect();
    let total = lines.len();
    let end = lines
        .iter()
        .rposition(|l| l.starts_with("export default") || l.starts_with("module.exports"))?;
    if total <= end + 16 {
        return None;
    }
    let tail = lines.get(end + 1..).unwrap_or_default().join("\n");
    looks_like_payload(&tail).then_some((total, end + 1))
}

pub fn tasks_json(walk: &Walk, ind: &Indicators, v: &mut Verdict, sink: &mut Sink) {
    v.section("Workspace tasks that run on folder open");
    let mut seen = 0usize;
    let mut flagged = 0usize;

    for path in walk.files.iter().filter(|p| {
        p.file_name().and_then(|n| n.to_str()) == Some("tasks.json")
            && p.parent()
                .and_then(|d| d.file_name())
                .and_then(|n| n.to_str())
                == Some(".vscode")
    }) {
        seen += 1;
        let Ok(body) = fs::read_to_string(path) else {
            continue;
        };
        if !body.contains("folderOpen") {
            continue;
        }
        flagged += 1;
        if ind.has_strong(&body) {
            let line = sink.take(path, "malicious-tasks-json");
            v.push(
                Finding::hit(
                    Kind::TasksJson,
                    format!(
                        "tasks.json runs on folder open AND contains an indicator: {}",
                        path.display()
                    ),
                )
                .at(path)
                .with_remedy(line),
            );
        } else {
            v.push(Finding::review(format!(
                "tasks.json runs on folder open, verify the command by hand: {}",
                path.display()
            )));
        }
    }

    if flagged == 0 {
        v.push(Finding::ok(if seen == 0 {
            "no .vscode/tasks.json found under the scanned paths".to_string()
        } else {
            format!("{seen} .vscode/tasks.json checked, none run on folder open")
        }));
    }
}

/// What to do with a build config that carries an indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnInfectedConfig {
    /// `check`: report it and leave it. `--apply` moves files, and moving this
    /// one would take the project's own config with it.
    Report,
    /// `clean`: cut an appended payload out of the file, keeping the original.
    Strip,
}

pub fn build_configs(
    walk: &Walk,
    ind: &Indicators,
    on_infected: OnInfectedConfig,
    v: &mut Verdict,
    sink: &mut Sink,
) {
    v.section("Build configs with code after the module end");
    let mut seen = 0usize;
    let mut flagged = 0usize;
    let mut stripped = 0usize;

    for path in walk.by_name(is_build_config) {
        seen += 1;
        // Bytes, not a string: a payload does not promise to be valid UTF-8,
        // and a file that failed to decode used to be skipped without a word.
        let Ok(bytes) = fs::read(path) else {
            v.push(Finding::review(format!(
                "could not read, so it was not checked: {}",
                path.display()
            )));
            continue;
        };
        let body = String::from_utf8_lossy(&bytes);

        if let Some(planned) = strip::plan(&bytes, ind) {
            flagged += 1;
            let hit = Finding::hit(
                Kind::Config {
                    strippable: planned.is_ok(),
                },
                format!("config file contains an indicator: {}", path.display()),
            )
            .at(path);
            match (on_infected, planned) {
                (OnInfectedConfig::Report, _) => v.push(hit.with_remedy(
                    "do not edit this file by hand. `polinrider clean` strips an appended payload and keeps the original; otherwise delete the clone and re-clone after the remote is clean.",
                )),
                (OnInfectedConfig::Strip, Ok(plan)) => {
                    stripped += 1;
                    show_cut(&plan, v);
                    v.push(hit.with_remedy(sink.strip(path, &plan)));
                }
                (OnInfectedConfig::Strip, Err(refusal)) => v.push(hit.with_remedy(format!(
                    "not stripped: {}. Delete the clone and re-clone after the remote is clean.",
                    refusal.reason()
                ))),
            }
            continue;
        }

        let lines: Vec<&str> = body.lines().collect();
        if let Some((total, end)) = payload_shaped_tail(&body) {
            flagged += 1;
            v.push(Finding::review(format!(
                "content after module end that looks like a payload ({total} lines, module ends at {end}): {}",
                path.display()
            )));
        }

        if lines.iter().any(|l| l.len() > 4000) {
            flagged += 1;
            v.push(Finding::review(format!(
                "line longer than 4000 characters, an obfuscation tell: {}",
                path.display()
            )));
        }
    }

    if flagged == 0 {
        v.push(Finding::ok(format!(
            "{seen} build config files checked, nothing appended after the module end"
        )));
    }
    if stripped > 0 {
        // Said every time, because it is the part people assume the opposite
        // of: the working tree is clean and the repository is not.
        v.push(Finding::info(
            "Stripping cleans the files in the working tree. It does not touch git: the commit that carried the payload may still be in this repository's history and on its remote.",
        ));
        v.push(Finding::info(
            "Review each change with git diff before committing it. Nothing was staged, committed, reset or stashed.",
        ));
    }
}

/// The evidence for a cut, printed above the finding: the last line that
/// stays and the start of what goes. Shown in a dry run too, which is the
/// point of it. Nobody should have to take the cut on trust.
fn show_cut(plan: &Plan, v: &mut Verdict) {
    // The last line as it will be, not as it is: in the file as found, that
    // line still has the padding and the payload on the end of it.
    let kept = String::from_utf8_lossy(&plan.keep);
    v.detail(format!(
        "keeps through line {}: {}",
        plan.last_kept_line,
        kept.lines().last().unwrap_or_default()
    ));
    v.detail(format!(
        "cuts {} bytes, starting: {}",
        plan.removed, plan.preview
    ));
}

pub fn fonts(walk: &Walk, v: &mut Verdict, sink: &mut Sink) {
    v.section("Font files that are not fonts");
    let mut seen = 0usize;
    let mut flagged = 0usize;

    for path in
        walk.by_name(|n| (n.ends_with(".woff") || n.ends_with(".woff2")) && !n.starts_with("._"))
    {
        if path.components().any(|c| c.as_os_str() == "__MACOSX") {
            continue;
        }
        seen += 1;
        let Ok(bytes) = fs::read(path) else { continue };

        // An empty file has no magic bytes to check, and a Git LFS pointer is
        // a text stub standing in for the font. Both are known false
        // positives, both pinned by conformance cases.
        if bytes.is_empty() || bytes.starts_with(b"version https://git-lfs") {
            continue;
        }
        let Some(magic) = bytes.get(..4) else {
            // Shorter than four bytes and not empty: not a font, but not worth
            // a confirmed hit either. The shell skips these too.
            continue;
        };
        if magic == b"wOFF" || magic == b"wOF2" || magic == b"vers" {
            continue;
        }

        flagged += 1;
        let hex = magic
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        let line = sink.take(path, "font-masquerade");
        v.push(
            Finding::hit(
                Kind::FakeFont,
                format!(
                    "font file is not a font (first bytes: {hex}): {}",
                    path.display()
                ),
            )
            .at(path)
            .with_remedy(line),
        );
    }

    if flagged == 0 {
        v.push(Finding::ok(format!(
            "{seen} font files checked, all are real fonts"
        )));
    }
}

pub fn packages(walk: &Walk, ind: &Indicators, v: &mut Verdict) {
    v.section("Known-bad packages");
    let mut found = false;

    for path in walk.by_name(is_manifest) {
        let Ok(body) = fs::read_to_string(path) else {
            continue;
        };
        if ind.has_bad_package(&body) {
            found = true;
            v.push(
                Finding::hit(
                    Kind::Package,
                    format!("known-bad package referenced: {}", path.display()),
                )
                .at(path)
                .with_remedy(
                    "remove the dependency, delete node_modules and the lockfile entry, reinstall.",
                ),
            );
        }
    }

    if !found {
        v.push(Finding::ok(
            "no known-bad package names in manifests or lockfiles",
        ));
    }
}

pub fn git_hooks(
    walk: &Walk,
    ind: &Indicators,
    host: Option<&dyn Host>,
    v: &mut Verdict,
    sink: &mut Sink,
) {
    v.section("Git configuration and hooks");
    match host {
        Some(host) => host_checks::git_global_config(host, v),
        // The global hooksPath lives in the user's git config, which is host
        // state; under --fs-only it is not read. Reported as ok so the section
        // always says something rather than printing nothing.
        None => v.push(Finding::ok("no global core.hooksPath")),
    }

    for hook in walk.git_hooks() {
        if ind.file_has_strong(&hook) {
            let line = sink.take(&hook, "git-hook");
            v.push(
                Finding::hit(
                    Kind::GitHook,
                    format!("git hook contains an indicator: {}", hook.display()),
                )
                .at(&hook)
                .with_remedy(line),
            );
        } else {
            v.push(Finding::review(format!(
                "active git hook, verify by hand: {}",
                hook.display()
            )));
        }
    }
}

/// The second-stage implant: known install and persistence paths, plus a hash
/// sweep so a renamed binary is still caught.
///
/// `~` in `implant-paths.txt` expands against the home directory given here,
/// not the process environment, so a scan of a mounted backup can point at the
/// backup's home rather than the running user's.
/// What to run before quarantining a file that is how the implant starts
/// itself. Read from where the file is: a launch agent, a systemd user unit
/// or an autostart entry.
fn stop_first(path: &Path) -> Vec<String> {
    let text = path.display().to_string();
    let quoted = sh_quote(&text);
    if text.contains("/LaunchAgents/") {
        vec![format!(
            "stop it first: launchctl bootout gui/$(id -u) {quoted} 2>/dev/null || launchctl unload {quoted}"
        )]
    } else if text.contains("/systemd/user/") {
        let unit = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        vec![
            format!(
                "stop it first: systemctl --user disable --now {}",
                sh_quote(&unit)
            ),
            "and: loginctl disable-linger \"$(whoami)\"".to_string(),
        ]
    } else if text.contains("/autostart/") {
        vec!["it will not start again once this file is quarantined".to_string()]
    } else {
        Vec::new()
    }
}

/// Where the implant check looks beyond the files of the walk.
pub struct ImplantScope<'a> {
    /// The home directory its install paths are under, when there is one.
    pub home: Option<&'a Path>,
    pub ioc_dir: &'a Path,
    /// The machine, for its process table.
    pub host: Option<&'a dyn Host>,
}

pub fn implants(
    walk: &Walk,
    ind: &Indicators,
    scope: &ImplantScope,
    v: &mut Verdict,
    sink: &mut Sink,
    // Told (done, of) before each large file is hashed: this is the one
    // check that can take minutes, and it should not take them in silence.
    hashing: &mut dyn FnMut(usize, usize),
) {
    v.section("Second-stage implant");
    let (home, ioc_dir, host) = (scope.home, scope.ioc_dir, scope.host);
    let mut found = false;

    // 1. the paths the implant installs itself to. They are under the home
    //    directory, so `clean`, which is pointed at a repository and nothing
    //    else, passes no home and this half does not run.
    let paths = home.and_then(|_| fs::read_to_string(ioc_dir.join("implant-paths.txt")).ok());
    if let (Some(home), Some(text)) = (home, paths) {
        for line in text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
        {
            // A Windows path; check-windows.ps1 owns those.
            if line.starts_with('%') {
                continue;
            }
            let path = match line.strip_prefix("~/") {
                Some(rest) => home.join(rest),
                None => Path::new(line).to_path_buf(),
            };
            if !path.exists() {
                continue;
            }
            found = true;
            let line_out = sink.take(&path, "second-stage-implant");
            let mut finding = Finding::hit(
                Kind::Implant,
                format!("implant artifact present: {}", path.display()),
            )
            .at(&path);
            // Moving the file does not stop what it already started. Said
            // first, because it has to be done first.
            for advice in stop_first(&path) {
                finding = finding.with_remedy(advice);
            }
            v.push(finding.with_remedy(line_out));
        }
    }

    // 2. a renamed binary still hashes the same. Bounded by size so the sweep
    //    does not read every file on the disk.
    let known = load_hashes(ioc_dir);
    if !known.is_empty() {
        let large: Vec<&PathBuf> = walk
            .files
            .iter()
            .filter(|path| {
                fs::metadata(path)
                    .is_ok_and(|m| (10 * 1024 * 1024..=300 * 1024 * 1024).contains(&m.len()))
            })
            .collect();
        for (n, path) in large.iter().copied().enumerate() {
            hashing(n, large.len());
            let Ok(digest) = crate::sha256::file(path) else {
                continue;
            };
            if let Some(label) = known.iter().find(|(h, _)| *h == digest).map(|(_, l)| l) {
                found = true;
                let line_out = sink.take(path, "second-stage-implant");
                v.push(
                    Finding::hit(
                        Kind::Implant,
                        format!(
                            "file matches a known implant hash ({label}): {}",
                            path.display()
                        ),
                    )
                    .at(path)
                    .with_remedy(line_out),
                );
            }
        }
    }

    // 3. the process table. The implant sets its own process title, so a
    //    match there is a finding even with nothing on disk.
    let Some(host) = host else {
        if !found {
            v.push(Finding::ok(if home.is_some() {
                "no second-stage implant found on disk (--fs-only: process table not read)"
            } else {
                "no file under the scanned paths matches a known implant hash"
            }));
        }
        return;
    };
    match host_checks::implant_processes(host, ind, v) {
        ProcessCheck::Running => {}
        ProcessCheck::NoneRunning if !found => {
            v.push(Finding::ok("no second-stage implant found"));
        }
        // Not read: the review line above already says so, and "none found"
        // may only be claimed for the half that was looked at.
        ProcessCheck::NotRead if !found => {
            v.push(Finding::ok("no second-stage implant found on disk"));
        }
        ProcessCheck::NoneRunning | ProcessCheck::NotRead => {}
    }
}

/// `hashes.txt` is `<sha256><space><label>` per line.
fn load_hashes(ioc_dir: &Path) -> Vec<(String, String)> {
    let Ok(text) = fs::read_to_string(ioc_dir.join("hashes.txt")) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let (h, label) = l.split_once(char::is_whitespace)?;
            Some((h.to_ascii_lowercase(), label.trim().to_string()))
        })
        .collect()
}

/// The propagation script the campaign drops to spread to other remotes.
pub fn propagation(walk: &Walk, v: &mut Verdict, sink: &mut Sink) {
    v.section("Propagation artifact temp_auto_push.bat");
    let mut found = false;
    for path in walk.by_name(|n| n == "temp_auto_push.bat" || n == "config.bat") {
        found = true;
        let line = sink.take(path, "propagation-script");
        v.push(
            Finding::hit(
                Kind::Propagation,
                format!("propagation script present: {}", path.display()),
            )
            .at(path)
            .with_remedy(line),
        );
    }
    if !found {
        v.push(Finding::ok("temp_auto_push.bat not found"));
    }
}

/// Editor extensions, which is how the campaign most often arrives.
///
/// Scoped to the extension directories given, and matched on content. The
/// weak list is deliberately not used here: a bundled extension legitimately
/// contains "folderOpen", which is a codicon name, and matching it produces
/// pages of noise.
pub fn extensions(dirs: &[PathBuf], ind: &Indicators, v: &mut Verdict, sink: &mut Sink) {
    v.section("IDE extensions");
    let mut any_dir = false;
    let mut found = false;

    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        any_dir = true;
        // Nothing is pruned here. An extension ships its dependencies inside
        // its own node_modules, and that is as good a place for a payload as
        // any. The project walk prunes node_modules because a project's
        // dependencies are caught by name from its lockfile; an installed
        // extension has no lockfile to catch them from.
        let everything = crate::walk::Options {
            prune: &[],
            skip: None,
        };
        let w = crate::walk::walk_with(std::slice::from_ref(dir), &everything);
        let mut flagged: Vec<PathBuf> = Vec::new();
        // Files that name a campaign host or address and carry no confirmed
        // indicator, with what they name.
        let mut naming: Vec<(&Path, Vec<&str>)> = Vec::new();

        for file in w.by_name(|n| {
            [".js", ".mjs", ".cjs", ".ts", ".json", ".map"]
                .iter()
                .any(|e| n.ends_with(e))
        }) {
            let Ok(bytes) = fs::read(file) else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes);
            if !ind.has_strong(&text) {
                // The weak list is useless in an extension bundle: one
                // legitimately contains "folderOpen" and "windowsHide". Only
                // the campaign's own infrastructure is worth a human's time.
                let named = ind.infrastructure_named(&text);
                if !named.is_empty() {
                    naming.push((file, named));
                }
                continue;
            }
            // Report the extension, not every file inside it.
            let ext_root = file
                .strip_prefix(dir)
                .ok()
                .and_then(|r| r.components().next())
                .map(|c| dir.join(c.as_os_str()));
            if let Some(root) = ext_root {
                if !flagged.contains(&root) {
                    flagged.push(root);
                }
            }
        }

        // An extension already confirmed is not also listed for review.
        naming.retain(|(file, _)| !flagged.iter().any(|root| file.starts_with(root)));
        for (file, named) in naming.iter().take(20) {
            v.push(
                Finding::review(format!(
                    "extension references campaign infrastructure ({}): {}",
                    named.join(" "),
                    file.display()
                ))
                .at(file),
            );
        }

        for root in flagged {
            found = true;
            let line = sink.take(&root, "ide-extension");
            v.push(
                Finding::hit(
                    Kind::Extension,
                    format!("extension contains an indicator: {}", root.display()),
                )
                .at(&root)
                .with_remedy(line),
            );
        }
    }

    if !any_dir {
        v.push(Finding::ok("no IDE extension directories found"));
        return;
    }
    if !found {
        v.push(Finding::ok("no IDE extension contains an indicator"));
    }

    // What changed lately, by name, in the report. Inventory and not a
    // finding: an extension updates itself, and the one to worry about is the
    // one nobody remembers installing.
    const RECENT_DAYS: u64 = 60;
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(RECENT_DAYS * 86_400));
    let mut recent = 0usize;
    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter(|e| {
                let changed = e.metadata().and_then(|m| m.modified()).ok();
                matches!((changed, cutoff), (Some(changed), Some(cutoff)) if changed >= cutoff)
            })
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort_unstable();
        recent += names.len();
        for name in names {
            v.note(format!("    {name}"));
        }
    }
    if recent > 0 {
        v.push(Finding::info(format!(
            "{recent} extension{} installed or updated in the last {RECENT_DAYS} days. Names are in the report; check any you did not install yourself.",
            if recent == 1 { "" } else { "s" }
        )));
    }
}

/// What would have to be changed if anything else here is a confirmed
/// finding: private keys and credential files in the home directory, and
/// `.env` files under the scanned paths.
///
/// Inventory, never a finding. Owning an SSH key is not suspicious and a
/// `.pub` file is not a credential. Only paths are listed, and only in the
/// report: nothing is read, so nothing can be printed.
pub fn credentials(walk: &Walk, home: Option<&Path>, v: &mut Verdict) {
    v.section(if home.is_some() {
        "Credential surface on this machine"
    } else {
        "Credential surface under the scanned paths"
    });
    let mut files: Vec<PathBuf> = Vec::new();
    if let Some(home) = home {
        if let Ok(entries) = fs::read_dir(home.join(".ssh")) {
            let mut keys: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("id_") && !n.ends_with(".pub"))
                })
                .collect();
            keys.sort();
            files.extend(keys);
        }
        files.extend(
            [
                ".aws/credentials",
                ".config/gcloud/credentials.db",
                ".docker/config.json",
                ".kube/config",
                ".netrc",
            ]
            .iter()
            .map(|p| home.join(p))
            .filter(|p| p.exists()),
        );
    }
    let env: Vec<&PathBuf> = walk.by_name(|n| n.starts_with(".env")).collect();
    for file in files.iter().chain(env.iter().copied()) {
        v.note(format!("  credential material: {}", file.display()));
    }
    if files.is_empty() && env.is_empty() {
        v.push(Finding::ok(
            "no credential files found under the scanned paths",
        ));
        return;
    }
    let count =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    v.push(Finding::info(format!(
        "{} and {} under the scanned paths.",
        count(
            files.len(),
            "private key or credential file",
            "private key or credential files"
        ),
        count(env.len(), ".env file", ".env files")
    )));
    v.push(Finding::info(
        "None of this is a finding. It is the list to change if anything else was a HIT.",
    ));
    v.push(Finding::info("Full paths are in the report file."));
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn a_payload_in_an_extension_s_own_node_modules_is_found() {
        // The shell greps the whole extension directory. The first Rust
        // version reused the project walk, which never enters node_modules,
        // and so could not see a payload in an extension's bundled dependency.
        let dir = std::env::temp_dir().join(format!("prc-ext-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let exts = dir.join("extensions");
        let dep = exts.join("publisher.helper-1.0.0/node_modules/dep");
        fs::create_dir_all(&dep).expect("mkdir");
        fs::write(dep.join("index.js"), "module.exports = 'MARKER-ALPHA'\n").expect("write");
        fs::create_dir_all(exts.join("publisher.fine-2.0.0")).expect("mkdir");
        fs::write(
            exts.join("publisher.fine-2.0.0/extension.js"),
            "exports.activate = () => {}\n",
        )
        .expect("write");

        let ind = Indicators {
            strong: vec!["MARKER-ALPHA".into()],
            ..Indicators::default()
        };
        let q = Quarantine::<DryRun>::new(dir.join("q"));
        let mut v = Verdict::new();
        extensions(&[exts], &ind, &mut v, &mut Sink::Dry(&q));

        let hits: Vec<&str> = v
            .findings()
            .filter(|f| f.level == crate::verdict::Level::Hit)
            .map(|f| f.message.as_str())
            .collect();
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert!(
            hits[0].ends_with("publisher.helper-1.0.0"),
            "the extension, not the file: {hits:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prc-checks-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    fn put(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, body).expect("write");
    }

    #[test]
    fn an_extension_naming_campaign_infrastructure_is_review_and_names_what_matched() {
        let dir = scratch("ext-net");
        let exts = dir.join("extensions");
        put(
            &exts.join("pub.caller-1.0.0/out/main.js"),
            "fetch('http://198.51.100.7/a'); fetch('https://c2.example.test/b')\n",
        );
        // An address that only contains the campaign's is somebody else's.
        put(
            &exts.join("pub.neighbour-1.0.0/out/main.js"),
            "fetch('http://198.51.100.70/a')\n",
        );
        // Confirmed for another reason: reported once, as the hit.
        put(
            &exts.join("pub.both-1.0.0/out/main.js"),
            "var a='MARKER-ALPHA'; fetch('http://198.51.100.7/a')\n",
        );
        let ind = Indicators {
            strong: vec!["MARKER-ALPHA".into()],
            network: vec!["198.51.100.7".into(), "c2.example.test".into()],
            ..Indicators::default()
        };
        let q = Quarantine::<DryRun>::new(dir.join("q"));
        let mut v = Verdict::new();
        extensions(&[exts], &ind, &mut v, &mut Sink::Dry(&q));

        let reviews: Vec<&str> = v
            .findings()
            .filter(|f| f.level == crate::verdict::Level::Review)
            .map(|f| f.message.as_str())
            .collect();
        assert_eq!(reviews.len(), 1, "{reviews:?}");
        assert!(reviews[0].starts_with(
            "extension references campaign infrastructure (198.51.100.7 c2.example.test): "
        ));
        assert!(reviews[0].ends_with("pub.caller-1.0.0/out/main.js"));
        assert_eq!(v.hits(), 1);
        // Fresh on disk, so all three are recent: counted, and named only in
        // the report.
        assert!(v
            .findings()
            .any(|f| f.message.starts_with("3 extensions installed or updated")));
        assert!(v.entries().iter().any(
            |e| matches!(e, crate::verdict::Entry::Note(n) if n.trim() == "pub.caller-1.0.0")
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_implant_that_starts_itself_comes_with_how_to_stop_it_first() {
        let dir = scratch("stop-first");
        let home = dir.join("home");
        let ioc = dir.join("ioc");
        put(
            &ioc.join("implant-paths.txt"),
            "# comment\n~/.config/systemd/user/helper's.service\n~/.config/autostart/helper.desktop\n~/.local/share/helper\n",
        );
        put(
            &home.join(".config/systemd/user/helper's.service"),
            "[Unit]\n",
        );
        put(
            &home.join(".config/autostart/helper.desktop"),
            "[Desktop Entry]\n",
        );
        put(&home.join(".local/share/helper"), "binary\n");
        let q = Quarantine::<DryRun>::new(dir.join("q"));
        let mut v = Verdict::new();
        implants(
            &Walk::default(),
            &Indicators::default(),
            &ImplantScope {
                home: Some(&home),
                ioc_dir: &ioc,
                host: None,
            },
            &mut v,
            &mut Sink::Dry(&q),
            &mut |_, _| {},
        );
        let remedies: Vec<String> = v.findings().filter_map(|f| f.remedy.clone()).collect();
        assert_eq!(v.hits(), 3);
        // The unit: stopped by name, quoted, and before the move.
        let unit = remedies
            .iter()
            .find(|r| r.contains("systemctl"))
            .expect("unit advice");
        let lines: Vec<&str> = unit.lines().collect();
        assert_eq!(
            lines[0],
            "stop it first: systemctl --user disable --now 'helper'\\''s.service'"
        );
        assert_eq!(lines[1], "and: loginctl disable-linger \"$(whoami)\"");
        assert_eq!(lines.len(), 3, "the quarantine line comes last: {unit}");
        assert!(remedies
            .iter()
            .any(|r| r.starts_with("it will not start again once this file is quarantined")));
        // A plain file needs no stopping.
        assert!(remedies.iter().any(|r| r.lines().count() == 1));

        let agent = stop_first(Path::new("/Users/x/Library/LaunchAgents/com.helper.plist"));
        assert_eq!(
            agent,
            vec!["stop it first: launchctl bootout gui/$(id -u) '/Users/x/Library/LaunchAgents/com.helper.plist' 2>/dev/null || launchctl unload '/Users/x/Library/LaunchAgents/com.helper.plist'"]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn credentials_are_counted_and_named_in_the_report_and_never_read() {
        let dir = scratch("credentials");
        let home = dir.join("home");
        put(&home.join(".ssh/id_ed25519"), "PRIVATE\n");
        put(&home.join(".ssh/id_ed25519.pub"), "public\n");
        put(&home.join(".ssh/known_hosts"), "host\n");
        put(&home.join(".aws/credentials"), "[default]\n");
        put(&home.join("code/app/.env"), "TOKEN=SECRET\n");
        put(&home.join("code/app/.env.local"), "TOKEN=SECRET\n");
        put(&home.join("code/app/env.js"), "export {}\n");
        let w = crate::walk::walk(&[home.join("code")]);

        let mut v = Verdict::new();
        credentials(&w, Some(&home), &mut v);
        let said: Vec<&str> = v.findings().map(|f| f.message.as_str()).collect();
        assert_eq!(
            said[0],
            "2 private key or credential files and 2 .env files under the scanned paths."
        );
        assert!(v.findings().all(|f| f.level == crate::verdict::Level::Info));
        let notes: Vec<&str> = v
            .entries()
            .iter()
            .filter_map(|e| match e {
                crate::verdict::Entry::Note(n) => Some(n.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(notes.len(), 4);
        assert!(notes[0].ends_with(".ssh/id_ed25519"));
        assert!(!notes
            .iter()
            .any(|n| n.ends_with(".pub") || n.contains("known_hosts")));
        assert!(!format!("{:?}", v.entries()).contains("SECRET"));

        // Without a machine to read, only what is under the scanned paths.
        let mut v = Verdict::new();
        credentials(&w, None, &mut v);
        assert_eq!(
            v.findings().next().map(|f| f.message.as_str()),
            Some("0 private key or credential files and 2 .env files under the scanned paths.")
        );
        let mut v = Verdict::new();
        credentials(&Walk::default(), None, &mut v);
        assert_eq!(
            v.findings().next().map(|f| f.level),
            Some(crate::verdict::Level::Ok)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_config_names_match_the_shell_glob() {
        assert!(is_build_config("postcss.config.mjs"));
        assert!(is_build_config("tailwind.config.js"));
        assert!(is_build_config("next.config.ts"));
        assert!(is_build_config("truffle.js"));
        // Not a config: a source file that merely starts with the same word.
        assert!(!is_build_config("postcssdoesnotcount.js"));
        assert!(!is_build_config("index.js"));
    }

    #[test]
    fn a_long_flat_config_is_not_a_payload_without_a_tell() {
        // Flat configs legitimately open with `export default [` and run long.
        // Length alone must not flag, or every large config is a finding.
        let ordinary = (0..200)
            .map(|i| format!("  rule{i},"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!looks_like_payload(&ordinary));
        assert!(looks_like_payload("const x = eval(atob('...'))"));
        assert!(looks_like_payload(&format!(
            "const p = '{}'",
            "A".repeat(600)
        )));
    }
}
