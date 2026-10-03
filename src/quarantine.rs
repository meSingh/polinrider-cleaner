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
use std::path::{Component, Path, PathBuf};

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
    /// A dry run of a strip. Nothing was copied and nothing was written.
    WouldStrip { from: PathBuf, to: PathBuf },
    /// The file at `from` was rewritten without its payload. The original, as
    /// it was, is at `to`.
    Stripped { from: PathBuf, to: PathBuf },
}

impl Outcome {
    /// The line the scanner prints under the finding.
    pub fn line(&self) -> String {
        match self {
            Outcome::Would { from, .. } => format!("would quarantine: {}", from.display()),
            Outcome::Moved { to, .. } => format!("quarantined -> {}", to.display()),
            Outcome::WouldStrip { to, .. } => {
                format!("the original would be kept at: {}", to.display())
            }
            Outcome::Stripped { to, .. } => format!("original kept -> {}", to.display()),
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

    /// The destination a given source would map to. Reads the filesystem to
    /// resolve the source and writes nothing. Shared by both modes so a dry
    /// run reports exactly the path an apply would use.
    fn destination(&self, src: &Path) -> PathBuf {
        // Resolved first, so `../code/x` and a symlinked root map to where the
        // file really is. A source that cannot be resolved is nested as given.
        let resolved = fs::canonicalize(src).unwrap_or_else(|_| src.to_path_buf());
        self.root.join("files").join(nested(&resolved))
    }
}

/// A path with everything removed that could carry it out of the directory
/// it is joined to.
///
/// Joining an absolute path replaces what it is joined to. Stripping a leading
/// `/` dealt with that on Unix and with nothing else: on Windows `C:\code\x`
/// is absolute without one, so the destination came out as the source itself.
/// A move onto itself does nothing and was reported as quarantined, and a
/// strip wrote the cleaned file over the only copy of the original. A `..` did
/// the same on any platform. So the path is rebuilt from its parts: a drive
/// becomes a directory named after its letter, a root is dropped, and a `..`
/// that survived resolving becomes a directory called `_parent_`.
fn nested(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::Prefix(prefix) => {
                // `C:` and the verbatim `\\?\C:` both become `C`.
                let drive: String = prefix
                    .as_os_str()
                    .to_string_lossy()
                    .chars()
                    .filter(char::is_ascii_alphanumeric)
                    .collect();
                if !drive.is_empty() {
                    out.push(drive);
                }
            }
            Component::RootDir | Component::CurDir => {}
            Component::ParentDir => out.push("_parent_"),
            Component::Normal(name) => out.push(name),
        }
    }
    out
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

    /// Report what a strip would do. Like everything on this type, it only
    /// computes a path.
    pub fn would_strip(&self, src: &Path) -> Outcome {
        Outcome::WouldStrip {
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

    /// Rewrite a file without its payload, keeping the original.
    ///
    /// The one operation in this tool that changes the contents of a file in
    /// place. The order is the point: the original is copied into quarantine
    /// and read back before anything touches the working tree, the cleaned
    /// file is written beside the original, and only then does a rename put
    /// it in place. If any step fails the original is still where it was.
    ///
    /// ```compile_fail
    /// use polinrider::quarantine::{Quarantine, DryRun};
    /// use std::path::Path;
    ///
    /// let mut q = Quarantine::<DryRun>::new("/tmp/q");
    /// q.strip(Path::new("/tmp/postcss.config.mjs"), b"", "reason");   // not on DryRun
    /// ```
    pub fn strip(&mut self, src: &Path, cleaned: &[u8], reason: &str) -> io::Result<Outcome> {
        let dest = self.destination(src);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let original = fs::read(src)?;
        fs::write(&dest, &original)?;
        if fs::read(&dest)? != original {
            return Err(io::Error::other(
                "the copy in quarantine does not match the original",
            ));
        }

        let mut temp = src.as_os_str().to_owned();
        temp.push(".polinrider-tmp");
        let temp = PathBuf::from(temp);
        let written = fs::write(&temp, cleaned)
            .and_then(|()| fs::set_permissions(&temp, fs::metadata(src)?.permissions()))
            .and_then(|()| fs::rename(&temp, src));
        if let Err(e) = written {
            let _ = fs::remove_file(&temp);
            return Err(e);
        }

        self.manifest
            .push((src.to_path_buf(), dest.clone(), reason.to_string()));
        Ok(Outcome::Stripped {
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

/// `YYYYMMDDTHHMMSSZ` for a number of seconds since the Unix epoch, in UTC.
///
/// Every run gets a quarantine directory of its own, named with this. A
/// shared directory would let a second run overwrite the first one's manifest
/// and, worse, write a second file over a first with the same path. Written
/// out because the crate has no dependencies; the date arithmetic is the
/// standard days-to-civil conversion.
pub fn stamp(seconds: u64) -> String {
    let days = seconds / 86_400;
    let rest = seconds % 86_400;
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

const RESTORE_TXT: &str = "\
Nothing here was deleted. To put a file back:

  while IFS=$'\\t' read -r orig dest reason; do
    [ \"$orig\" = \"original_path\" ] && continue
    mkdir -p \"$(dirname \"$orig\")\" && mv \"$dest\" \"$orig\"
  done < manifest.tsv

A row whose reason is stripped-config is different: that file was cleaned in
place and is still in your project. The copy here is the infected original, and
putting it back undoes the cleaning.

Keep this directory until the incident is closed. It is evidence.
";

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
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
    fn a_strip_keeps_the_original_and_rewrites_the_file_in_place() -> io::Result<()> {
        let tmp = std::env::temp_dir().join(format!("prc-q-strip-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("tree"))?;
        let src = tmp.join("tree/postcss.config.mjs");
        fs::write(&src, b"export default {}\nPAYLOAD")?;

        let mut q = Quarantine::<Apply>::create(tmp.join("q"))?;
        let outcome = q.strip(&src, b"export default {}\n", "stripped-config")?;

        assert_eq!(fs::read(&src)?, b"export default {}\n", "cleaned in place");
        let Outcome::Stripped { to, .. } = &outcome else {
            unreachable!()
        };
        assert_eq!(
            fs::read(to)?,
            b"export default {}\nPAYLOAD",
            "the original survives, byte for byte"
        );
        assert!(
            !tmp.join("tree/postcss.config.mjs.polinrider-tmp").exists(),
            "no temporary file is left in the working tree"
        );
        let manifest = fs::read_to_string(q.write_manifest()?)?;
        assert!(manifest.contains("stripped-config"));
        assert_eq!(q.taken(), 1);

        fs::remove_dir_all(&tmp)?;
        Ok(())
    }

    #[test]
    fn a_strip_that_cannot_keep_the_original_changes_nothing() -> io::Result<()> {
        // The quarantine destination's parent is a file, so the copy fails.
        // The working tree must be exactly as it was.
        let tmp = std::env::temp_dir().join(format!("prc-q-stripfail-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("tree"))?;
        let src = tmp.join("tree/next.config.js");
        fs::write(&src, b"module.exports = {}\nPAYLOAD")?;

        let mut q = Quarantine::<Apply>::create(tmp.join("q"))?;
        let blocker = q.destination(&src);
        if let Some(dir) = blocker.parent().and_then(Path::parent) {
            fs::create_dir_all(dir)?;
            fs::write(blocker.parent().unwrap_or(dir), b"in the way")?;
        }

        assert!(q
            .strip(&src, b"module.exports = {}\n", "stripped-config")
            .is_err());
        assert_eq!(fs::read(&src)?, b"module.exports = {}\nPAYLOAD");
        assert_eq!(q.taken(), 0);

        fs::remove_dir_all(&tmp)?;
        Ok(())
    }

    #[test]
    fn no_source_path_can_carry_a_file_out_of_the_quarantine() {
        // Each of these used to land outside files/, and two of them landed
        // on the source itself.
        let inside = |p: &str| {
            let n = nested(Path::new(p));
            assert!(n.is_relative(), "{p} -> {}", n.display());
            assert!(
                n.components().all(|c| matches!(c, Component::Normal(_))),
                "{p} -> {}",
                n.display()
            );
            n
        };
        assert_eq!(inside("/home/x/bad.woff2"), Path::new("home/x/bad.woff2"));
        assert_eq!(
            inside("../../etc/passwd"),
            Path::new("_parent_/_parent_/etc/passwd")
        );
        assert_eq!(inside("./code/x.js"), Path::new("code/x.js"));
        assert_eq!(inside("code/../x.js"), Path::new("code/_parent_/x.js"));
    }

    #[cfg(windows)]
    #[test]
    fn a_windows_drive_becomes_a_directory_not_a_new_root() {
        assert_eq!(
            nested(Path::new(r"C:\code\x.js")),
            Path::new(r"C\code\x.js")
        );
        assert_eq!(
            nested(Path::new(r"\\?\C:\code\x.js")),
            Path::new(r"C\code\x.js")
        );
    }

    #[test]
    fn a_source_given_with_dot_dot_is_quarantined_under_the_root() -> io::Result<()> {
        // `polinrider check ../code` hands the walk paths that begin with `..`.
        let tmp = std::env::temp_dir().join(format!("prc-q-dotdot-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("tree/sub"))?;
        let src = tmp.join("tree/sub/../fake.woff2");
        fs::write(&src, b"var x=1")?;

        let mut q = Quarantine::<Apply>::create(tmp.join("q"))?;
        let Outcome::Moved { to, .. } = q.take(&src, "font-masquerade")? else {
            unreachable!()
        };
        assert!(
            !tmp.join("tree/fake.woff2").exists(),
            "moved out of the tree"
        );
        assert!(to.exists());
        let files = fs::canonicalize(tmp.join("q/files"))?;
        assert!(
            fs::canonicalize(&to)?.starts_with(&files),
            "landed at {}, outside {}",
            to.display(),
            files.display()
        );
        fs::remove_dir_all(&tmp)?;
        Ok(())
    }

    #[test]
    fn a_timestamp_is_the_right_day_including_a_leap_day() {
        assert_eq!(stamp(0), "19700101T000000Z");
        assert_eq!(stamp(951_782_400), "20000229T000000Z");
        assert_eq!(stamp(1_000_000_000), "20010909T014640Z");
        assert_eq!(stamp(1_709_251_199), "20240229T235959Z");
        assert_eq!(stamp(1_709_251_200), "20240301T000000Z");
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
