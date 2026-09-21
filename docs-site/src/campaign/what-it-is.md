# What PolinRider is

A supply-chain campaign attributed to DPRK-linked actors, tracked alongside the
Contagious Interview and Famous Chollima cluster. First observed December 2025,
first documented publicly March 2026, still active.

It is **not** repository defacement. The repository changes are how it travels.
The goal is credentials.

## How you get infected

| Entry point | What happens |
|---|---|
| A malicious npm, Go or Composer package | `postinstall` runs, or your build imports the poisoned module |
| A malicious VS Code or Cursor extension | runs the moment the editor loads it |
| A "take-home interview project" repository | you open it in your editor to review it |
| An already-infected repo you cloned | `.vscode/tasks.json` runs a command when the folder opens |

The last one is the important one. **Opening a folder is enough.** You do not
have to run the project, install anything, or click anything.

## What it does once it runs

The visible layer is an obfuscated JavaScript loader. The loader is a blockchain
dead-drop resolver: it reads an encrypted second stage from TRON, Aptos or BNB
Smart Chain, XOR-decrypts it, and `eval()`s it in memory. Because the payload
lives in blockchain transactions, there is no C2 domain to take down, and
blocking one host achieves nothing.

The second stage has been the **DEV#POPPER** remote access trojan and
**OmniStealer**; earlier waves carried **BeaverTail**, followed by
**InvisibleFerret**. What they take:

- browser session cookies and saved passwords
- GitHub personal access tokens, SSH keys, `gh` CLI credentials
- npm, cloud (AWS/GCP), database and CI platform tokens
- every value in every `.env` file it can read
- cryptocurrency wallets and seed phrases, which it targets specifically

## What it leaves behind

| Artifact | Detail |
|---|---|
| Build config files | payload appended after the real `export default` / `module.exports`, behind roughly 280 spaces of padding |
| Fake fonts | `.woff2` files under `public/`, `static/`, `assets/` whose bytes are JavaScript, not a font |
| Editor tasks | `.vscode/tasks.json` with `"runOn": "folderOpen"` running `curl ... \| bash` |
| Propagation script | `temp_auto_push.bat` |
| Second-stage implant | a binary or process named `MicrosoftSystem64`, and the directories `~/.pcl-data` and `~/.pcl-state` |

## Reinfection

Cleaning the remote while an infected machine still holds a valid token puts you
back where you started within minutes. That is the documented behaviour of this
campaign, not a hypothetical, and it is the reason [order matters](../guides/order.md).
