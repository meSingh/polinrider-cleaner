**What this changes**
<!-- One or two sentences. -->

**Why**
<!-- Link the issue, or the source for a new indicator. -->

**Type**
- [ ] New or updated indicator in `ioc/`
- [ ] False-positive fix
- [ ] Missed-detection fix
- [ ] Bug fix
- [ ] Documentation
- [ ] Other:

**Checks**
- [ ] `./ci/selftest.sh` passes
- [ ] `bash -n` is clean on every script I touched
- [ ] `shellcheck --severity=warning` is clean on every script I touched
- [ ] Windows only: `check-windows.ps1` parses, and PSScriptAnalyzer reports no error or warning
- [ ] No script I touched deletes anything. Destructive steps remain behind `--apply` and remain reversible
- [ ] Docs updated if behaviour or output changed

**For a new indicator**
- [ ] Source linked, and the indicator is public
- [ ] Placed in the right file: `strong.txt` only if a match means infection with
      no plausible alternative, otherwise `weak.txt`
- [ ] I checked it does not match a common legitimate file

**Tested on**
<!-- OS, shell version, and what you ran it against. -->
