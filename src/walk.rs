//! One pruned walk of the filesystem, reused by every check.
//!
//! ADR-0025. The shell did this eight times, each written as
//! `-not -path '*/node_modules/*'`, which filters what `find` prints without
//! stopping it descending. A backup drive took six hours. Here the walk
//! happens once, directories on the prune list are never entered, and the
//! checks read the resulting list.
//!
//! `node_modules` is not walked at all. Campaign packages are caught by name
//! from manifests and lockfiles, and the payload lives in the project's own
//! config files and fonts rather than inside a dependency. That is a real
//! limitation, recorded in ADR-0025 and pinned by a conformance case.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Directory names never entered. Matched on the final component, so it
/// applies at any depth.
pub const PRUNED: &[&str] = &[
    "node_modules",
    ".git",
    ".Trash",
    ".cache",
    "__MACOSX",
    ".npm",
    ".pnpm-store",
    ".yarn",
    ".venv",
    "venv",
    "Library",
];

/// The result of walking the roots once.
#[derive(Debug, Default)]
pub struct Walk {
    /// Every regular file found, excluding anything under a pruned directory.
    pub files: Vec<PathBuf>,
    /// `.git` directories, listed but never entered, so hooks can still be
    /// read without walking an object store.
    pub git_dirs: Vec<PathBuf>,
    /// Roots that did not exist or could not be read. Reported rather than
    /// skipped silently: scanning nothing must not look like scanning clean.
    pub unreadable: Vec<PathBuf>,
}

/// What a walk leaves out.
#[derive(Debug, Clone)]
pub struct Options<'a> {
    /// Directory names never entered.
    pub prune: &'a [&'a str],
    /// One directory never entered, wherever it is: the quarantine. Without
    /// this, a quarantine placed under a scanned root is walked by the next
    /// scan, which finds every artifact it holds and reports the machine as
    /// still infected by its own evidence.
    pub skip: Option<PathBuf>,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Self {
            prune: PRUNED,
            skip: None,
        }
    }
}

impl<'a> Options<'a> {
    /// The usual prune list, and never enter `quarantine`. Resolved once here
    /// so that a relative path and the absolute one the walk meets compare
    /// equal. A quarantine that does not exist yet has nothing in it to skip.
    pub fn skipping(quarantine: &Path) -> Self {
        Self {
            prune: PRUNED,
            skip: fs::canonicalize(quarantine).ok(),
        }
    }

    fn excludes(&self, dir: &Path) -> bool {
        let Some(name) = dir.file_name() else {
            return false;
        };
        if name.to_str().is_some_and(|n| {
            // Any quarantine, this run's or an earlier one's: 1.x and 2.0
            // both name them this way.
            self.prune.contains(&n) || n.starts_with("polinrider-quarantine")
        }) {
            return true;
        }
        // The name is compared first because it is free; resolving every
        // directory on the disk to compare paths is not.
        self.skip.as_deref().is_some_and(|skip| {
            skip.file_name() == Some(name) && fs::canonicalize(dir).is_ok_and(|d| d == skip)
        })
    }
}

/// Walk every root once. Never follows symlinks: a link into `/` would
/// otherwise turn a project scan into a whole-disk scan, and a link pointing
/// outside the tree is not part of what the caller asked to scan.
pub fn walk(roots: &[PathBuf]) -> Walk {
    walk_with(roots, &Options::default())
}

pub fn walk_with(roots: &[PathBuf], options: &Options) -> Walk {
    let mut out = Walk::default();
    for root in roots {
        match fs::symlink_metadata(root) {
            Ok(m) if m.is_dir() => descend(root, &mut out, options),
            Ok(_) => out.files.push(root.clone()),
            Err(_) => out.unreadable.push(root.clone()),
        }
    }
    out.files.sort();
    out.git_dirs.sort();
    out
}

fn descend(dir: &Path, out: &mut Walk, options: &Options) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        // An unreadable subdirectory is noted, not fatal: a scan of a backup
        // drive will hit directories the current user cannot open.
        Err(_) => {
            out.unreadable.push(dir.to_path_buf());
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let meta = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };

        if meta.is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
                out.git_dirs.push(path);
                continue;
            }
            if options.excludes(&path) {
                continue;
            }
            descend(&path, out, options);
        } else if meta.is_file() {
            out.files.push(path);
        }
    }
}

impl Walk {
    /// Files whose name matches a predicate. The checks use this instead of
    /// walking again.
    pub fn by_name<'a>(
        &'a self,
        pred: impl Fn(&str) -> bool + 'a,
    ) -> impl Iterator<Item = &'a PathBuf> {
        self.files
            .iter()
            .filter(move |p| p.file_name().and_then(|n| n.to_str()).is_some_and(&pred))
    }

    /// Executable, non-sample hook files in every `.git/hooks` found.
    pub fn git_hooks(&self) -> Vec<PathBuf> {
        let mut hooks = Vec::new();
        for g in &self.git_dirs {
            let Ok(entries) = fs::read_dir(g.join("hooks")) else {
                continue;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("sample") {
                    continue;
                }
                if p.is_file() && is_executable(&p) {
                    hooks.push(p);
                }
            }
        }
        hooks.sort();
        hooks
    }
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(p)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_p: &Path) -> bool {
    true
}

/// Create a directory and everything above it, for callers that need one.
pub fn ensure_dir(p: &Path) -> io::Result<()> {
    fs::create_dir_all(p)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn tree(name: &str, files: &[&str]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("prc-walk-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for f in files {
            let p = root.join(f);
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).expect("mkdir");
            }
            fs::write(&p, b"x").expect("write");
        }
        root
    }

    #[test]
    fn pruned_directories_are_never_entered() {
        // The whole point of ADR-0025. A file inside node_modules must not
        // appear, no matter how deep.
        let root = tree(
            "prune",
            &[
                "proj/postcss.config.mjs",
                "proj/node_modules/evil/deep/deeper/payload.js",
                "proj/.cache/thing.js",
                "proj/src/index.js",
            ],
        );
        let w = walk(std::slice::from_ref(&root));
        let names: Vec<String> = w.files.iter().map(|p| p.display().to_string()).collect();
        assert!(names.iter().any(|n| n.ends_with("postcss.config.mjs")));
        assert!(names.iter().any(|n| n.ends_with("src/index.js")));
        assert!(
            !names.iter().any(|n| n.contains("node_modules")),
            "{names:?}"
        );
        assert!(!names.iter().any(|n| n.contains(".cache")), "{names:?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_quarantine_under_a_scanned_root_is_never_walked() {
        // The second scan of a guided session used to find the artifacts the
        // first one had just quarantined, and report them as still present.
        let root = tree(
            "quarantine",
            &[
                "proj/a.js",
                "proj/evidence-here/files/proj/fake.woff2",
                "polinrider-quarantine-20260101T000000Z/files/proj/fake.woff2",
            ],
        );
        let w = walk_with(
            std::slice::from_ref(&root),
            &Options::skipping(&root.join("proj/evidence-here")),
        );
        let names: Vec<String> = w.files.iter().map(|p| p.display().to_string()).collect();
        assert_eq!(names.len(), 1, "{names:?}");
        assert!(names[0].ends_with("proj/a.js"));

        // Without being told, the oddly named one is walked like anything else.
        let w = walk(std::slice::from_ref(&root));
        assert_eq!(w.files.len(), 2);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn git_is_listed_but_not_walked() {
        // Hooks still have to be readable without walking an object store.
        let root = tree(
            "git",
            &[
                "proj/.git/hooks/pre-commit",
                "proj/.git/objects/aa/bb",
                "proj/a.js",
            ],
        );
        let w = walk(std::slice::from_ref(&root));
        assert_eq!(w.git_dirs.len(), 1, "the .git directory should be listed");
        assert!(
            !w.files
                .iter()
                .any(|p| p.display().to_string().contains("/.git/")),
            "nothing inside .git should be in the file list"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_root_is_reported_not_silently_skipped() {
        // Scanning nothing must never look like scanning clean.
        let w = walk(&[PathBuf::from("/definitely/not/here/at/all")]);
        assert_eq!(w.unreadable.len(), 1);
        assert!(w.files.is_empty());
    }

    #[test]
    fn symlinks_are_not_followed() {
        // A link to / would turn a project scan into a whole-disk scan.
        let root = tree("symlink", &["proj/real.js"]);
        #[cfg(unix)]
        {
            let link = root.join("proj/escape");
            let _ = std::os::unix::fs::symlink("/etc", &link);
            let w = walk(std::slice::from_ref(&root));
            assert!(
                !w.files
                    .iter()
                    .any(|p| p.display().to_string().contains("passwd")),
                "must not have followed the link out of the tree"
            );
        }
        let _ = fs::remove_dir_all(&root);
    }
}
