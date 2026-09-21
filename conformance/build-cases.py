#!/usr/bin/env python3
"""Generate conformance/cases/*.json.

The cases are generated rather than hand-written so the set stays internally
consistent: one place defines what a fixture looks like, and a reviewer reads
intent rather than JSON punctuation.

No case contains a payload. Fixtures write {{STRONG}}, {{WEAK}}, {{BADPKG}}
and {{PAD}}, and conformance/run.sh substitutes real values from ioc/ when it
builds the tree. Working malware strings stay out of the repository, and the
corpus cannot drift away from the indicator set it is testing.
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

for name, body in CASES.items():
    with open(os.path.join(D, name + ".json"), "w") as f:
        json.dump(body, f, indent=2)
        f.write("\n")

print(f"{len(CASES)} cases written to {D}")
