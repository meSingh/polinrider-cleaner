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
use crate::host_checks::{self, ProcessCheck};
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
fn is_build_config(name: &str) -> bool {
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
                    Kind::InProject,
                    format!(
                        "tasks.json runs on folder open AND contains an indicator: {}",
                        path.display()
                    ),
                )
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
            );
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
        let total = lines.len();
        let end = lines
            .iter()
            .rposition(|l| l.starts_with("export default") || l.starts_with("module.exports"));

        if let Some(end) = end {
            if total > end + 16 {
                let tail = lines.get(end + 1..).unwrap_or_default().join("\n");
                if looks_like_payload(&tail) {
                    flagged += 1;
                    v.push(Finding::review(format!(
                        "content after module end that looks like a payload ({total} lines, module ends at {}): {}",
                        end + 1,
                        path.display()
                    )));
                }
            }
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
                Kind::InProject,
                format!(
                    "font file is not a font (first bytes: {hex}): {}",
                    path.display()
                ),
            )
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
                    Kind::OnMachine,
                    format!("git hook contains an indicator: {}", hook.display()),
                )
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
pub fn implants(
    walk: &Walk,
    ind: &Indicators,
    home: Option<&Path>,
    ioc_dir: &Path,
    host: Option<&dyn Host>,
    v: &mut Verdict,
    sink: &mut Sink,
) {
    v.section("Second-stage implant");
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
            v.push(
                Finding::hit(
                    Kind::OnMachine,
                    format!("implant artifact present: {}", path.display()),
                )
                .with_remedy(line_out),
            );
        }
    }

    // 2. a renamed binary still hashes the same. Bounded by size so the sweep
    //    does not read every file on the disk.
    let known = load_hashes(ioc_dir);
    if !known.is_empty() {
        for path in &walk.files {
            let Ok(meta) = fs::metadata(path) else {
                continue;
            };
            let len = meta.len();
            if !(10 * 1024 * 1024..=300 * 1024 * 1024).contains(&len) {
                continue;
            }
            let Ok(digest) = crate::sha256::file(path) else {
                continue;
            };
            if let Some(label) = known.iter().find(|(h, _)| *h == digest).map(|(_, l)| l) {
                found = true;
                let line_out = sink.take(path, "second-stage-implant");
                v.push(
                    Finding::hit(
                        Kind::OnMachine,
                        format!(
                            "file matches a known implant hash ({label}): {}",
                            path.display()
                        ),
                    )
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
                Kind::OnMachine,
                format!("propagation script present: {}", path.display()),
            )
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

        for file in w.by_name(|n| {
            [".js", ".mjs", ".cjs", ".ts", ".json", ".map"]
                .iter()
                .any(|e| n.ends_with(e))
        }) {
            if !ind.file_has_strong(file) {
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

        for root in flagged {
            found = true;
            let line = sink.take(&root, "ide-extension");
            v.push(
                Finding::hit(
                    Kind::OnMachine,
                    format!("extension contains an indicator: {}", root.display()),
                )
                .with_remedy(line),
            );
        }
    }

    if !any_dir {
        v.push(Finding::ok("no IDE extension directories found"));
    } else if !found {
        v.push(Finding::ok("no IDE extension contains an indicator"));
    }
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
