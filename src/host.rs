//! The boundary between the scanner and the machine it is running on.
//!
//! A third of what this tool checks is not on the disk being scanned: the
//! process table, open sockets, the user crontab, the global git
//! configuration, the system's persistence directories. Those answers depend
//! on what happens to be running, so a test cannot assert on them, and a check
//! nobody can test is a check that stops matching without anyone noticing.
//!
//! Everything of that kind is read through [`Host`], and nothing else in the
//! crate is allowed to run a command or open a path outside the roots and the
//! home directory it was given. Two implementations:
//!
//! - [`LiveHost`] asks the running machine.
//! - [`Snapshot`] holds the answers as data. `--host-state DIR` loads one from
//!   files, which is how the conformance corpus drives these checks, and how
//!   state captured on another machine can be examined on this one.
//!
//! The boundary carries [`Probe`], not bare values. "The process table was
//! empty" and "the process table could not be read" are different answers,
//! and a scanner that folds the second into the first reports clean for work
//! it did not do. See ADR-0029.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Which persistence layout and which tools apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    MacOs,
    Windows,
}

impl Platform {
    /// The platform this binary is running on, when the host checks know it.
    pub fn current() -> Option<Self> {
        Self::parse(std::env::consts::OS)
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name.trim() {
            "linux" => Some(Platform::Linux),
            "macos" => Some(Platform::MacOs),
            "windows" => Some(Platform::Windows),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Platform::Linux => "linux",
            Platform::MacOs => "macos",
            Platform::Windows => "windows",
        }
    }
}

/// What came back when the host was asked a question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe<T> {
    /// The question was answered. The answer may be empty.
    Read(T),
    /// The tool that would answer is not installed. Sometimes that is itself
    /// an answer (no `crontab` means no user crontab) and sometimes it is a
    /// check that could not run (no `ps`). The check decides, not the probe.
    NoTool(String),
    /// The question could not be answered. Never the same as an empty answer.
    Failed(String),
}

/// One row of the process table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    pub pid: u32,
    /// The process name as the kernel reports it. On Linux this is at most 15
    /// bytes; on macOS it is the full path of the executable.
    pub name: String,
    /// The full command line.
    pub command: String,
}

/// Something Windows starts by itself that is not a file in a folder: a
/// value under a registry Run key, or a scheduled task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Autostart {
    /// The registry key, or the folder of the task (`\` for the top one).
    pub place: String,
    /// The value's name, or the task's.
    pub name: String,
    /// What it runs.
    pub command: String,
}

/// Everything the scanner asks of the machine rather than of a file it was
/// pointed at.
pub trait Host {
    fn platform(&self) -> Platform;

    /// False when the state was supplied. Advice that only makes sense on the
    /// machine itself, such as a `kill` with a pid, must not be printed for
    /// state captured somewhere else.
    fn is_live(&self) -> bool;

    /// One line for the report header, so a reader can tell which machine the
    /// host checks describe.
    fn describe(&self) -> String;

    fn processes(&self) -> Probe<Vec<Process>>;

    /// Established TCP connections, one per line, as the platform's socket
    /// tool prints them. Unfiltered: deciding which lines matter is the
    /// check's job, so that the decision is tested.
    fn connections(&self) -> Probe<Vec<String>>;

    /// The user's crontab, verbatim.
    fn crontab(&self) -> Probe<String>;

    /// `key=value` lines, as `git config --global --list` prints them.
    fn git_global_config(&self) -> Probe<Vec<String>>;

    /// The directory standing in for `/` when system-wide paths are read.
    fn system_root(&self) -> Probe<PathBuf>;

    /// Windows: every value under the Run and RunOnce keys, for this user and
    /// for the machine. Not asked on any other platform.
    fn run_keys(&self) -> Probe<Vec<Autostart>>;

    /// Windows: every scheduled task with what it runs. Unfiltered: which
    /// ones are Windows' own is the check's decision, so that it is tested.
    /// Not asked on any other platform.
    fn scheduled_tasks(&self) -> Probe<Vec<Autostart>>;
}

/// Resolve an absolute system path against a host's root.
pub fn system_path(root: &Path, absolute: &str) -> PathBuf {
    root.join(absolute.trim_start_matches('/'))
}

// ---------------------------------------------------------------------------
// The running machine
// ---------------------------------------------------------------------------

/// The machine this binary is running on.
#[derive(Debug)]
pub struct LiveHost {
    platform: Platform,
    home: PathBuf,
}

impl LiveHost {
    /// Refuses on a platform whose host checks are not built, rather than
    /// running none of them and reporting the result as a scan.
    pub fn new(home: &Path) -> Result<Self, String> {
        match Platform::current() {
            Some(platform) => Ok(Self {
                platform,
                home: home.to_path_buf(),
            }),
            None => Err(format!(
                "the live host checks are not built for {} yet.\n\nRefused rather than skipped. Pass --fs-only to scan the filesystem alone,\nwhich says so in the report.",
                std::env::consts::OS
            )),
        }
    }
}

enum Ran {
    Finished {
        ok: bool,
        stdout: String,
        stderr: String,
    },
    NoTool,
    Failed(String),
}

/// Run one read-only command. `LC_ALL=C` so the messages this module matches
/// on are the ones it expects.
fn run(program: &str, args: &[&str], home: Option<&Path>) -> Ran {
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::null()).env("LC_ALL", "C");
    if let Some(home) = home {
        command.env("HOME", home);
    }
    match command.output() {
        Ok(out) => Ran::Finished {
            ok: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ran::NoTool,
        Err(e) => Ran::Failed(format!("{program}: {e}")),
    }
}

fn first_line(program: &str, stderr: &str) -> String {
    match stderr.lines().map(str::trim).find(|l| !l.is_empty()) {
        Some(line) => format!("{program}: {line}"),
        None => format!("{program} failed without saying why"),
    }
}

fn lines(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

/// `ps -o pid=,<column>=` output: a right-aligned pid, then the column.
fn parse_ps(text: &str) -> Vec<(u32, String)> {
    text.lines()
        .filter_map(|line| {
            let (pid, rest) = line.trim_start().split_once(char::is_whitespace)?;
            Some((pid.parse().ok()?, rest.trim().to_string()))
        })
        .collect()
}

fn ps(column: &str) -> Probe<Vec<(u32, String)>> {
    // ww: never truncate to a terminal width. A command line cut at 80
    // columns is a command line whose interesting half was not read.
    match run("ps", &["axww", "-o", &format!("pid=,{column}=")], None) {
        Ran::Finished {
            ok: true, stdout, ..
        } => Probe::Read(parse_ps(&stdout)),
        Ran::Finished { stderr, .. } => Probe::Failed(first_line("ps", &stderr)),
        Ran::NoTool => Probe::NoTool("ps is not installed".into()),
        Ran::Failed(why) => Probe::Failed(why),
    }
}

/// Run a PowerShell script for its lines. Windows PowerShell 5 is part of
/// every supported Windows, so nothing is asked to be installed.
///
/// The scripts here use no double quote and no variable from outside: each is
/// a constant, and the fields of a line are joined with a tab made inside the
/// script, so nothing has to survive two layers of quoting.
fn powershell(script: &str) -> Probe<Vec<String>> {
    match run(
        "powershell",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ],
        None,
    ) {
        Ran::Finished {
            ok: true, stdout, ..
        } => Probe::Read(lines(&stdout)),
        Ran::Finished { stderr, .. } => Probe::Failed(first_line("powershell", &stderr)),
        Ran::NoTool => Probe::NoTool("powershell is not available".into()),
        Ran::Failed(why) => Probe::Failed(why),
    }
}

const PS_PROCESSES: &str = "Get-CimInstance Win32_Process | ForEach-Object { '{0}{3}{1}{3}{2}' -f $_.ProcessId, $_.Name, ([string]$_.CommandLine -replace '\\s+',' '), [char]9 }";

const PS_CONNECTIONS: &str = "Get-NetTCPConnection -State Established -ErrorAction Stop | ForEach-Object { '{0}{3}{1}:{2}' -f (Get-Process -Id $_.OwningProcess -ErrorAction SilentlyContinue).ProcessName, $_.RemoteAddress, $_.RemotePort, [char]9 }";

const PS_RUN_KEYS: &str = "foreach ($k in 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run','HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce','HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run','HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce') { if (Test-Path $k) { foreach ($q in (Get-ItemProperty -Path $k).PSObject.Properties) { if (@('PSPath','PSParentPath','PSChildName','PSDrive','PSProvider') -notcontains $q.Name) { '{0}{3}{1}{3}{2}' -f $k, $q.Name, ([string]$q.Value -replace '\\s+',' '), [char]9 } } } }";

const PS_TASKS: &str = "Get-ScheduledTask -ErrorAction Stop | ForEach-Object { '{0}{3}{1}{3}{2}' -f $_.TaskPath, $_.TaskName, ((($_.Actions | ForEach-Object { '{0} {1}' -f $_.Execute, $_.Arguments }) -join ' ') -replace '\\s+',' '), [char]9 }";

/// `place<TAB>name<TAB>command` per line. A line with fewer fields is one
/// PowerShell wrapped or a value with nothing in it, and is kept with what
/// it has: dropping it would drop something that starts by itself.
fn parse_autostarts(lines: &[String]) -> Vec<Autostart> {
    lines
        .iter()
        .filter_map(|line| {
            let mut fields = line.splitn(3, '\t');
            let place = fields.next()?.trim().to_string();
            let name = fields.next()?.trim().to_string();
            (!place.is_empty()).then(|| Autostart {
                place,
                name,
                command: fields.next().unwrap_or_default().trim().to_string(),
            })
        })
        .collect()
}

impl Host for LiveHost {
    fn platform(&self) -> Platform {
        self.platform
    }

    fn is_live(&self) -> bool {
        true
    }

    fn describe(&self) -> String {
        format!("this machine ({})", self.platform.name())
    }

    fn processes(&self) -> Probe<Vec<Process>> {
        if self.platform == Platform::Windows {
            let me = std::process::id();
            return map(powershell(PS_PROCESSES), |lines| {
                lines
                    .iter()
                    .filter_map(|line| {
                        let mut fields = line.splitn(3, '\t');
                        let pid: u32 = fields.next()?.trim().parse().ok()?;
                        let name = fields.next()?.trim().to_string();
                        Some(Process {
                            pid,
                            name,
                            command: fields.next().unwrap_or_default().trim().to_string(),
                        })
                    })
                    .filter(|p| p.pid != me)
                    .collect()
            });
        }
        // Two calls, because a name can contain spaces and so can a command
        // line: one listing with both columns cannot be split reliably.
        let names = match ps("comm") {
            Probe::Read(n) => n,
            Probe::NoTool(why) => return Probe::NoTool(why),
            Probe::Failed(why) => return Probe::Failed(why),
        };
        let commands = match ps("command") {
            Probe::Read(c) => c,
            Probe::NoTool(why) => return Probe::NoTool(why),
            Probe::Failed(why) => return Probe::Failed(why),
        };
        let me = std::process::id();
        Probe::Read(
            names
                .into_iter()
                .filter(|(pid, _)| *pid != me)
                .map(|(pid, name)| {
                    // A process that exited between the two calls has a name
                    // and no command line. It is kept: the name is the field
                    // the implant check reads.
                    let command = commands
                        .iter()
                        .find(|(p, _)| *p == pid)
                        .map(|(_, c)| c.clone())
                        .unwrap_or_default();
                    Process { pid, name, command }
                })
                .collect(),
        )
    }

    fn connections(&self) -> Probe<Vec<String>> {
        match self.platform {
            Platform::MacOs => match run("lsof", &["-nP", "-iTCP", "-sTCP:ESTABLISHED"], None) {
                // lsof exits 1 when nothing matched, which is an answer. Only
                // a failure that printed no connections and did say why is
                // treated as a failure.
                Ran::Finished { ok, stdout, stderr } => {
                    if ok || !stdout.trim().is_empty() || stderr.trim().is_empty() {
                        Probe::Read(lines(&stdout))
                    } else {
                        Probe::Failed(first_line("lsof", &stderr))
                    }
                }
                Ran::NoTool => Probe::NoTool("lsof not available".into()),
                Ran::Failed(why) => Probe::Failed(why),
            },
            Platform::Linux => {
                for tool in ["ss", "netstat"] {
                    match run(tool, &["-tnp"], None) {
                        Ran::Finished {
                            ok: true, stdout, ..
                        } => return Probe::Read(lines(&stdout)),
                        Ran::Finished { stderr, .. } => {
                            return Probe::Failed(first_line(tool, &stderr))
                        }
                        Ran::NoTool => {}
                        Ran::Failed(why) => return Probe::Failed(why),
                    }
                }
                Probe::NoTool("neither ss nor netstat available".into())
            }
            // The owning process by name, then where it is connected to.
            Platform::Windows => powershell(PS_CONNECTIONS),
        }
    }

    fn crontab(&self) -> Probe<String> {
        if self.platform == Platform::Windows {
            return Probe::NoTool("Windows has no crontab".into());
        }
        match run("crontab", &["-l"], None) {
            Ran::Finished {
                ok: true, stdout, ..
            } => Probe::Read(stdout),
            // "no crontab for <user>" is exit 1 and is the empty answer.
            Ran::Finished { stderr, .. } if stderr.to_ascii_lowercase().contains("no crontab") => {
                Probe::Read(String::new())
            }
            Ran::Finished { stderr, .. } => Probe::Failed(first_line("crontab", &stderr)),
            Ran::NoTool => Probe::NoTool("crontab is not installed".into()),
            Ran::Failed(why) => Probe::Failed(why),
        }
    }

    fn git_global_config(&self) -> Probe<Vec<String>> {
        // HOME is set to the home being checked, so --home and the global
        // configuration describe the same account.
        match run("git", &["config", "--global", "--list"], Some(&self.home)) {
            Ran::Finished {
                ok: true, stdout, ..
            } => Probe::Read(lines(&stdout)),
            // No global configuration file at all is the empty answer.
            Ran::Finished { stderr, .. }
                if stderr.trim().is_empty() || stderr.contains("unable to read config file") =>
            {
                Probe::Read(Vec::new())
            }
            Ran::Finished { stderr, .. } => Probe::Failed(first_line("git", &stderr)),
            Ran::NoTool => Probe::NoTool("git is not installed".into()),
            Ran::Failed(why) => Probe::Failed(why),
        }
    }

    fn system_root(&self) -> Probe<PathBuf> {
        if self.platform == Platform::Windows {
            let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
            return Probe::Read(PathBuf::from(format!("{drive}\\")));
        }
        Probe::Read(PathBuf::from("/"))
    }

    fn run_keys(&self) -> Probe<Vec<Autostart>> {
        map(powershell(PS_RUN_KEYS), |lines| parse_autostarts(&lines))
    }

    fn scheduled_tasks(&self) -> Probe<Vec<Autostart>> {
        map(powershell(PS_TASKS), |lines| parse_autostarts(&lines))
    }
}

// ---------------------------------------------------------------------------
// Supplied state
// ---------------------------------------------------------------------------

/// Host state held as data rather than read from the machine.
///
/// Loaded from a directory by `--host-state`, or built directly by a test.
/// The directory holds one file per question:
///
/// | File | Holds |
/// |---|---|
/// | `platform` | `linux`, `macos` or `windows`. Required |
/// | `processes` | one process per line: pid, name, command line, tab-separated |
/// | `connections` | socket tool output, one connection per line |
/// | `crontab` | the user crontab, verbatim |
/// | `git-config` | `key=value` lines, as `git config --global --list` prints |
/// | `root/` | stands in for `/`, or for the system drive, when system directories are read |
/// | `run-keys` | Windows: key, value name, what it runs, tab-separated |
/// | `scheduled-tasks` | Windows: task folder, task name, what it runs, tab-separated |
///
/// A file that is absent means the question was **not answered**, and the
/// check reports that rather than treating it as empty. An empty file is the
/// empty answer. `<name>.absent` records that the tool which would answer was
/// not installed on the machine the state came from.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Where the state came from, for the report header.
    pub source: String,
    pub platform: Platform,
    pub processes: Probe<Vec<Process>>,
    pub connections: Probe<Vec<String>>,
    pub crontab: Probe<String>,
    pub git_global_config: Probe<Vec<String>>,
    pub system_root: Probe<PathBuf>,
    pub run_keys: Probe<Vec<Autostart>>,
    pub scheduled_tasks: Probe<Vec<Autostart>>,
}

/// Why a host-state directory could not be used. Each is an exit code 3
/// condition: half-loaded state must not become a scan.
#[derive(Debug)]
pub enum SnapshotError {
    NotADirectory(PathBuf),
    NoPlatform(PathBuf),
    BadPlatform { file: PathBuf, found: String },
    BadProcess { file: PathBuf, line: usize },
    Unreadable { file: PathBuf, source: io::Error },
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnapshotError::NotADirectory(dir) => {
                write!(f, "--host-state: {} is not a directory", dir.display())
            }
            SnapshotError::NoPlatform(file) => write!(
                f,
                "--host-state: {} is missing. It must hold 'linux', 'macos' or 'windows',\nbecause that decides which persistence locations are read.",
                file.display()
            ),
            SnapshotError::BadPlatform { file, found } => write!(
                f,
                "--host-state: {} holds '{found}'. It must hold 'linux', 'macos' or 'windows'.",
                file.display()
            ),
            SnapshotError::BadProcess { file, line } => write!(
                f,
                "--host-state: {} line {line} is not 'pid<TAB>name<TAB>command line'.\n\nRefused rather than skipped: a process that was dropped while loading is a\nprocess that was never checked.",
                file.display()
            ),
            SnapshotError::Unreadable { file, source } => {
                write!(f, "--host-state: cannot read {}: {source}", file.display())
            }
        }
    }
}

impl Snapshot {
    /// A machine with nothing running, nothing scheduled and nothing
    /// configured, whose system directories are under `root`. The starting
    /// point for tests, which then set the one field they are about.
    pub fn quiet(platform: Platform, root: impl Into<PathBuf>) -> Self {
        Self {
            source: "a test".into(),
            platform,
            processes: Probe::Read(Vec::new()),
            connections: Probe::Read(Vec::new()),
            crontab: Probe::Read(String::new()),
            git_global_config: Probe::Read(Vec::new()),
            system_root: Probe::Read(root.into()),
            run_keys: Probe::Read(Vec::new()),
            scheduled_tasks: Probe::Read(Vec::new()),
        }
    }

    pub fn load(dir: &Path) -> Result<Self, SnapshotError> {
        if !dir.is_dir() {
            return Err(SnapshotError::NotADirectory(dir.to_path_buf()));
        }

        let platform_file = dir.join("platform");
        let platform = match fs::read_to_string(&platform_file) {
            Ok(text) => Platform::parse(&text).ok_or_else(|| SnapshotError::BadPlatform {
                file: platform_file.clone(),
                found: text.trim().to_string(),
            })?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(SnapshotError::NoPlatform(platform_file))
            }
            Err(e) => {
                return Err(SnapshotError::Unreadable {
                    file: platform_file,
                    source: e,
                })
            }
        };

        let processes = match supplied(dir, "processes")? {
            Probe::Read(text) => Probe::Read(parse_processes(&dir.join("processes"), &text)?),
            Probe::NoTool(why) => Probe::NoTool(why),
            Probe::Failed(why) => Probe::Failed(why),
        };

        let root = dir.join("root");
        let system_root = if root.is_dir() {
            Probe::Read(root)
        } else {
            Probe::Failed(not_supplied(dir, "root/"))
        };

        Ok(Self {
            source: dir.display().to_string(),
            platform,
            processes,
            connections: map(supplied(dir, "connections")?, |t| lines(&t)),
            crontab: supplied(dir, "crontab")?,
            git_global_config: map(supplied(dir, "git-config")?, |t| lines(&t)),
            system_root,
            run_keys: map(supplied(dir, "run-keys")?, |t| parse_autostarts(&lines(&t))),
            scheduled_tasks: map(supplied(dir, "scheduled-tasks")?, |t| {
                parse_autostarts(&lines(&t))
            }),
        })
    }
}

fn not_supplied(dir: &Path, name: &str) -> String {
    format!("{name} was not supplied in {}", dir.display())
}

fn map<T, U>(probe: Probe<T>, f: impl FnOnce(T) -> U) -> Probe<U> {
    match probe {
        Probe::Read(t) => Probe::Read(f(t)),
        Probe::NoTool(why) => Probe::NoTool(why),
        Probe::Failed(why) => Probe::Failed(why),
    }
}

/// One question's file. Present is an answer, `.absent` is "the tool was not
/// installed there", and neither is "nobody asked".
fn supplied(dir: &Path, name: &str) -> Result<Probe<String>, SnapshotError> {
    let file = dir.join(name);
    match fs::read(&file) {
        Ok(bytes) => Ok(Probe::Read(String::from_utf8_lossy(&bytes).into_owned())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if dir.join(format!("{name}.absent")).exists() {
                Ok(Probe::NoTool(format!(
                    "{name} is recorded as not available"
                )))
            } else {
                Ok(Probe::Failed(not_supplied(dir, name)))
            }
        }
        Err(e) => Err(SnapshotError::Unreadable { file, source: e }),
    }
}

fn parse_processes(file: &Path, text: &str) -> Result<Vec<Process>, SnapshotError> {
    let mut out = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let bad = || SnapshotError::BadProcess {
            file: file.to_path_buf(),
            line: index + 1,
        };
        let mut fields = line.splitn(3, '\t');
        let pid = fields
            .next()
            .and_then(|p| p.trim().parse().ok())
            .ok_or_else(bad)?;
        let name = fields.next().ok_or_else(bad)?.to_string();
        let command = fields.next().ok_or_else(bad)?.to_string();
        out.push(Process { pid, name, command });
    }
    Ok(out)
}

impl Host for Snapshot {
    fn platform(&self) -> Platform {
        self.platform
    }

    fn is_live(&self) -> bool {
        false
    }

    fn describe(&self) -> String {
        format!(
            "supplied from {} ({}), NOT read from this machine",
            self.source,
            self.platform.name()
        )
    }

    fn processes(&self) -> Probe<Vec<Process>> {
        self.processes.clone()
    }

    fn connections(&self) -> Probe<Vec<String>> {
        self.connections.clone()
    }

    fn crontab(&self) -> Probe<String> {
        self.crontab.clone()
    }

    fn git_global_config(&self) -> Probe<Vec<String>> {
        self.git_global_config.clone()
    }

    fn system_root(&self) -> Probe<PathBuf> {
        self.system_root.clone()
    }

    fn run_keys(&self) -> Probe<Vec<Autostart>> {
        self.run_keys.clone()
    }

    fn scheduled_tasks(&self) -> Probe<Vec<Autostart>> {
        self.scheduled_tasks.clone()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn state(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prc-host-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        for (file, body) in files {
            let path = dir.join(file);
            fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            fs::write(path, body).expect("write");
        }
        dir
    }

    #[test]
    fn ps_output_parses_with_spaces_in_the_name() {
        // macOS reports the full path of the executable, and paths under
        // "Application Support" contain a space.
        let rows =
            parse_ps("    1 /sbin/launchd\n  412 /Users/x/Library/Application Support/App/bin\n");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].0, 412);
        assert_eq!(rows[1].1, "/Users/x/Library/Application Support/App/bin");
    }

    #[test]
    fn a_question_nobody_answered_is_not_an_empty_answer() {
        // The failure this prevents: a state directory with no processes file
        // loading as "nothing is running".
        let dir = state("unanswered", &[("platform", "linux\n")]);
        let snap = Snapshot::load(&dir).expect("loads");
        assert!(matches!(snap.processes, Probe::Failed(_)));
        assert!(matches!(snap.crontab, Probe::Failed(_)));
        assert!(matches!(snap.connections, Probe::Failed(_)));
        assert!(matches!(snap.system_root, Probe::Failed(_)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_empty_file_is_the_empty_answer() {
        let dir = state(
            "empty",
            &[
                ("platform", "macos"),
                ("processes", ""),
                ("crontab", ""),
                ("connections", ""),
                ("git-config", ""),
                ("root/.keep", ""),
            ],
        );
        let snap = Snapshot::load(&dir).expect("loads");
        assert_eq!(snap.platform, Platform::MacOs);
        assert_eq!(snap.processes, Probe::Read(Vec::new()));
        assert_eq!(snap.crontab, Probe::Read(String::new()));
        assert!(matches!(snap.system_root, Probe::Read(_)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_tool_recorded_as_missing_is_its_own_answer() {
        let dir = state("absent", &[("platform", "linux"), ("crontab.absent", "")]);
        let snap = Snapshot::load(&dir).expect("loads");
        assert!(matches!(snap.crontab, Probe::NoTool(_)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn processes_load_with_tabs_and_keep_the_whole_command_line() {
        let dir = state(
            "procs",
            &[
                ("platform", "linux"),
                (
                    "processes",
                    "1\tinit\t/sbin/init\n77\tnode\tnode -e \"a\tb\"\n",
                ),
            ],
        );
        let snap = Snapshot::load(&dir).expect("loads");
        let Probe::Read(procs) = snap.processes else {
            unreachable!("processes should have loaded")
        };
        assert_eq!(procs.len(), 2);
        assert_eq!(procs[1].pid, 77);
        assert_eq!(procs[1].name, "node");
        // A tab inside the command line belongs to the command line.
        assert_eq!(procs[1].command, "node -e \"a\tb\"");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_malformed_process_line_refuses_the_whole_state() {
        // Dropping the line would be dropping a process from the scan.
        let dir = state(
            "badproc",
            &[
                ("platform", "linux"),
                ("processes", "1\tinit\t/sbin/init\nnot a process\n"),
            ],
        );
        assert!(matches!(
            Snapshot::load(&dir),
            Err(SnapshotError::BadProcess { line: 2, .. })
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn state_without_a_platform_is_refused() {
        let dir = state("noplatform", &[("processes", "")]);
        assert!(matches!(
            Snapshot::load(&dir),
            Err(SnapshotError::NoPlatform(_))
        ));
        let other = state("badplatform", &[("platform", "plan9")]);
        assert!(matches!(
            Snapshot::load(&other),
            Err(SnapshotError::BadPlatform { .. })
        ));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&other);
    }

    #[test]
    fn system_paths_resolve_under_the_supplied_root() {
        assert_eq!(
            system_path(Path::new("/state/root"), "/etc/cron.d"),
            PathBuf::from("/state/root/etc/cron.d")
        );
        assert_eq!(
            system_path(Path::new("/"), "/etc/cron.d"),
            PathBuf::from("/etc/cron.d")
        );
    }
}
