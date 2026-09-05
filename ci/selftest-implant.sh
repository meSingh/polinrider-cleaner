#!/usr/bin/env bash
# selftest-implant.sh - offline test of second-stage implant detection.
#
# Plants the implant's install, persistence and working-directory artifacts in a
# throwaway HOME, then asserts the local check finds all of them, matches a
# renamed binary by hash, quarantines rather than deletes, and does not report
# itself. No network, no credentials, nothing outside a temp directory.
#
# Usage: ./selftest-implant.sh

set -uo pipefail
SDIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SDIR/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
FAILED=0

pass() { printf '  ok    %s\n' "$*"; }
fail() { printf '  FAIL  %s\n' "$*"; FAILED=1; }

case "$(uname -s)" in
  Darwin) CHECK="$ROOT/machine-cleanup/check-macos.sh" ;;
  *)      CHECK="$ROOT/machine-cleanup/check-linux.sh" ;;
esac

sha256() {
  if   command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}'
  elif command -v shasum    >/dev/null 2>&1; then shasum -a 256 "$1" | awk '{print $1}'
  fi
}

# --- fixture ----------------------------------------------------------------
H="$TMP/home"
mkdir -p "$H/.local/share" "$H/.config/systemd/user" "$H/.config/autostart" \
         "$H/Library/LaunchAgents" "$H/code"

echo 'implant' > "$H/.local/share/MicrosoftSystem64"
mkdir -p "$H/.pcl-data" "$H/.pcl-state"
echo '[Service]'     > "$H/.config/systemd/user/MicrosoftSystem64.service"
echo '[Desktop]'     > "$H/.config/autostart/MicrosoftSystem64.desktop"
echo '<plist/>'      > "$H/Library/LaunchAgents/com.launchkeeper.MicrosoftSystem64.plist"

# A renamed implant binary, over the size floor of the hash sweep.
RENAMED="$H/code/totally-legit-helper"
dd if=/dev/zero of="$RENAMED" bs=1048576 count=12 2>/dev/null
printf 'unique-marker-for-selftest' >> "$RENAMED"

# Fixture indicator set: the real one, with this file's hash added.
IOCDIR="$TMP/ioc"; cp -R "$ROOT/ioc" "$IOCDIR"
printf '%s  selftest fixture binary\n' "$(sha256 "$RENAMED")" >> "$IOCDIR/hashes.txt"

# --- 1. dry run --------------------------------------------------------------
OUT="$(HOME="$H" PRC_IOC_DIR="$IOCDIR" "$CHECK" "$H/code" 2>&1)"
RC=$?
echo "assertions, dry run:"
if [[ $RC -eq 2 ]]; then pass "exits 2 with an implant present"; else fail "expected exit 2, got $RC"; fi

for want in ".local/share/MicrosoftSystem64" ".pcl-data" ".pcl-state" \
            "MicrosoftSystem64.service" "MicrosoftSystem64.desktop" \
            "com.launchkeeper.MicrosoftSystem64.plist"; do
  if printf '%s\n' "$OUT" | grep -q "$want"; then pass "found: $want"; else fail "missed: $want"; fi
done

if printf '%s\n' "$OUT" | grep -q "matches a known implant hash"; then
  pass "renamed binary caught by hash"
else fail "renamed binary not caught by hash"; fi

for cmd in "systemctl --user disable --now" "launchctl bootout"; do
  if printf '%s\n' "$OUT" | grep -q "$cmd"; then pass "prints the disable command: $cmd"
  else fail "no disable command for: $cmd"; fi
done

if printf '%s\n' "$OUT" | grep -q "would quarantine"; then
  pass "dry run says what it would move"; else fail "dry run gave no quarantine preview"; fi
if [[ -e "$H/.local/share/MicrosoftSystem64" ]]; then
  pass "dry run left the artifacts in place"; else fail "dry run moved something"; fi

# The scanner must not report itself. This shell's command line mentions the
# implant name many times over; only an actual process named for it counts.
if printf '%s\n' "$OUT" | grep -q "an implant process is running now"; then
  fail "reported a running implant when none exists (self-detection)"
else pass "no self-detection from command lines mentioning the implant"; fi

# --- 2. apply ----------------------------------------------------------------
echo
echo "assertions, --apply:"
Q="$TMP/quarantine"
OUT="$(HOME="$H" PRC_IOC_DIR="$IOCDIR" "$CHECK" --apply --quarantine "$Q" "$H/code" 2>&1)"

if [[ ! -e "$H/.local/share/MicrosoftSystem64" ]]; then
  pass "implant binary moved out of place"; else fail "implant binary still in place"; fi
if [[ -f "$Q/manifest.tsv" ]] && [[ "$(grep -c . "$Q/manifest.tsv")" -gt 1 ]]; then
  pass "quarantine manifest written"; else fail "no quarantine manifest"; fi
if find "$Q/files" -name 'MicrosoftSystem64' 2>/dev/null | grep -q .; then
  pass "artifact preserved in quarantine, not deleted"; else fail "artifact was not preserved"; fi
if [[ -f "$Q/RESTORE.txt" ]]; then pass "restore instructions written"; else fail "no RESTORE.txt"; fi

# --- 3. clean control --------------------------------------------------------
echo
echo "assertions, clean control:"
CH="$TMP/clean-home"; mkdir -p "$CH/code"
OUT="$(HOME="$CH" PRC_IOC_DIR="$ROOT/ioc" "$CHECK" "$CH/code" 2>&1)"
if printf '%s\n' "$OUT" | grep -q "no second-stage implant found"; then
  pass "clean home reports no implant"; else fail "clean home reported an implant"; fi

echo
if [[ $FAILED -eq 0 ]]; then echo "IMPLANT SELFTEST PASSED"; exit 0; fi
echo "IMPLANT SELFTEST FAILED"; exit 1
