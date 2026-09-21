//! Quarantine, with "this run cannot write" enforced by the type system.
//!
//! This is the reason ADR-0026 chose Rust over Go. The shell implementation
//! guarantees a read-only dry run by convention: `quarantine()` checks a
//! global and returns early. That works, and it worked for a year, but it is
//! a runtime check on a code path whose failure mode is moving somebody's
//! files during what they were told was a read-only scan.
//!
//! Here the guarantee is structural. [`Quarantine<DryRun>`] has no method that
//! can move a file, so "quarantined during a dry run" is not a bug that can be
//! introduced; it fails to compile. A reviewer does not have to notice it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Whether a scan may change the filesystem. Sealed: the only two
/// implementations are [`DryRun`] and [`Apply`], and code outside this module
/// cannot add a third that claims to be read-only while writing.
pub trait Mode: sealed::Sealed {
    /// Present in output so a reader can tell which kind of run produced it.
    const LABEL: &'static str;
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::DryRun {}
    impl Sealed for super::Apply {}
}

/// Inspect and report. The default.
///
/// A `Quarantine<DryRun>` has no method that can move a file. This is the
/// guarantee the module exists for, and it is checked by the compiler on
/// every build rather than by a reviewer noticing:
///
/// ```compile_fail
/// use polinrider::quarantine::{Quarantine, DryRun};
/// use std::path::Path;
///
/// let mut q = Quarantine::<DryRun>::new("/tmp/q");
/// q.take(Path::new("/tmp/evil"), "reason");   // no such method on DryRun
/// ```
///
/// The same call on an [`Apply`] quarantine compiles, because that one is
/// allowed to write.
#[derive(Debug, Clone, Copy)]
pub struct DryRun;

/// Move confirmed artifacts into quarantine. Never deletes.
#[derive(Debug, Clone, Copy)]
pub struct Apply;

impl Mode for DryRun {
    const LABEL: &'static str = "dry run - nothing will be changed";
}
impl Mode for Apply {
    const LABEL: &'static str = "APPLY - confirmed artifacts will be moved to quarantine";
}

/// What happened, or would have happened, to one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// A dry run. The path is where it would have gone.
    Would { from: PathBuf, to: PathBuf },
    /// Moved. The original no longer exists at `from`.
    Moved { from: PathBuf, to: PathBuf },
}

impl Outcome {
    /// The line the scanner prints under the finding.
    pub fn line(&self) -> String {
        match self {
            Outcome::Would { from, .. } => format!("would quarantine: {}", from.display()),
            Outcome::Moved { to, .. } => format!("quarantined -> {}", to.display()),
        }
    }
}

/// A quarantine directory, parameterised on whether this run may write.
#[derive(Debug)]
pub struct Quarantine<M: Mode> {
    root: PathBuf,
    manifest: Vec<(PathBuf, PathBuf, String)>,
    _mode: std::marker::PhantomData<M>,
}

impl<M: Mode> Quarantine<M> {
    /// Where quarantined files and the manifest live.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The destination a given source would map to. Pure: computes a path,
    /// touches nothing. Shared by both modes so a dry run reports exactly the
    /// path an apply would use.
    fn destination(&self, src: &Path) -> PathBuf {
        // Strip the leading separator so an absolute source nests under the
        // quarantine root rather than escaping it.
        let rel = src.strip_prefix("/").unwrap_or(src);
        self.root.join("files").join(rel)
    }
}

impl Quarantine<DryRun> {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            manifest: Vec::new(),
            _mode: std::marker::PhantomData,
        }
    }

    /// Report what would happen. There is deliberately no method on this type
    /// that moves anything: a dry run that writes is not a bug you can write.
    pub fn would_take(&self, src: &Path) -> Outcome {
        Outcome::Would {
            from: src.to_path_buf(),
            to: self.destination(src),
        }
    }
}

impl Quarantine<Apply> {
    /// Create the quarantine directory and its restore instructions.
    pub fn create(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("files"))?;
        fs::write(root.join("RESTORE.txt"), RESTORE_TXT)?;
        Ok(Self {
            root,
            manifest: Vec::new(),
            _mode: std::marker::PhantomData,
        })
    }

    /// Move a file into quarantine. Moves, never deletes: if this fails the
    /// original is still where it was, which is the behaviour that matters
    /// when the caller is wrong about what it found.
    pub fn take(&mut self, src: &Path, reason: &str) -> io::Result<Outcome> {
        let dest = self.destination(src);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        // rename first: atomic, and cheap on the same filesystem. Fall back to
        // copy-then-remove across devices, which a mounted backup drive or a
        // container bind mount will hit.
        match fs::rename(src, &dest) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
                fs::copy(src, &dest)?;
                fs::remove_file(src)?;
            }
            Err(e) => return Err(e),
        }
        self.manifest
            .push((src.to_path_buf(), dest.clone(), reason.to_string()));
        Ok(Outcome::Moved {
            from: src.to_path_buf(),
            to: dest,
        })
    }

    /// Write the receipt. Every quarantined file, where it came from, and why.
    pub fn write_manifest(&self) -> io::Result<PathBuf> {
        let path = self.root.join("manifest.tsv");
        let mut out = String::from("original_path\tquarantined_path\treason\n");
        for (from, to, reason) in &self.manifest {
            out.push_str(&format!(
                "{}\t{}\t{}\n",
                from.display(),
                to.display(),
                reason
            ));
        }
        fs::write(&path, out)?;
        Ok(path)
    }

    pub fn taken(&self) -> usize {
        self.manifest.len()
    }
}

const RESTORE_TXT: &str = "\
Nothing here was deleted. To put a file back:

  while IFS=$'\\t' read -r orig dest reason; do
    [ \"$orig\" = \"original_path\" ] && continue
    mkdir -p \"$(dirname \"$orig\")\" && mv \"$dest\" \"$orig\"
  done < manifest.tsv

Keep this directory until the incident is closed. It is evidence.
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dry_run_reports_the_same_path_an_apply_would_use() {
        let dry = Quarantine::<DryRun>::new("/tmp/q");
        let Outcome::Would { to, .. } = dry.would_take(Path::new("/home/x/bad.woff2")) else {
            unreachable!("would_take always reports Would")
        };
        assert_eq!(to, Path::new("/tmp/q/files/home/x/bad.woff2"));
    }

    #[test]
    fn apply_moves_and_does_not_delete() -> io::Result<()> {
        let tmp = std::env::temp_dir().join(format!("prc-q-{}", std::process::id()));
        let src_dir = tmp.join("tree");
        fs::create_dir_all(&src_dir)?;
        let src = src_dir.join("fake.woff2");
        fs::write(&src, b"var x=1")?;

        let mut q = Quarantine::<Apply>::create(tmp.join("q"))?;
        let outcome = q.take(&src, "font-masquerade")?;

        assert!(!src.exists(), "the original must not still be in the tree");
        let Outcome::Moved { to, .. } = &outcome else {
            unreachable!()
        };
        assert!(
            to.exists(),
            "the file must exist in quarantine, not be deleted"
        );
        assert_eq!(fs::read(to)?, b"var x=1", "contents must survive the move");

        let manifest = q.write_manifest()?;
        let text = fs::read_to_string(&manifest)?;
        assert!(
            text.contains("font-masquerade"),
            "the reason belongs in the receipt"
        );
        assert!(text.contains("fake.woff2"));
        assert_eq!(q.taken(), 1);

        fs::remove_dir_all(&tmp)?;
        Ok(())
    }

    #[test]
    fn an_absolute_source_nests_under_the_root_rather_than_escaping_it() {
        let q = Quarantine::<DryRun>::new("/tmp/q");
        let Outcome::Would { to, .. } = q.would_take(Path::new("/etc/passwd")) else {
            unreachable!()
        };
        assert!(to.starts_with("/tmp/q/files"), "got {}", to.display());
    }
}
