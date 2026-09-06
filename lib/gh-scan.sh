#!/usr/bin/env bash
# gh-scan.sh - mirror every repo of an owner, capture push events, scan every ref
#              for PolinRider indicators.
#
# READ-ONLY against GitHub. Never pushes, deletes or rewrites anything.
#
# Usage:
#   gh-scan.sh --owner ACME --owner-type org|user --out "$EV" [options]
#
# Options:
#   --repo OWNER/NAME   scan a single repository instead of the whole account
#   --mirror-only       stop after cloning and capturing events (phase 1)
#   --scan-only         reuse existing mirrors, skip cloning and events
#   --no-forks          skip forked repositories
#   --from-sweep FILE   only touch repositories that appear in a sweep.tsv. On a
#                       large account this is the difference between cloning
#                       everything and cloning the eight repos that were hit
#
# Output in <out>/:
#   <name>.git/         bare mirror, GC frozen (forensic baseline)
#   events/<name>.json  raw PushEvent feed
#   pushes.tsv          push ledger: repo, ref, before, after, actor, time
#   triage.json         per-ref findings
#   triage.txt          human-readable summary

set -uo pipefail
# shellcheck source=lib/common.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

OWNER=""; KIND=""; OUT=""; SINGLE=""; MIRROR_ONLY=0; SCAN_ONLY=0; NOFORK=0; FROM_SWEEP=

while [[ $# -gt 0 ]]; do
  case "$1" in
    --owner)      OWNER="$2"; shift 2 ;;
    --owner-type) KIND="$2"; shift 2 ;;
    --out)        OUT="$2"; shift 2 ;;
    --repo)       SINGLE="$2"; shift 2 ;;
    --from-sweep) FROM_SWEEP="$2"; shift 2 ;;
    --mirror-only) MIRROR_ONLY=1; shift ;;
    --scan-only)   SCAN_ONLY=1; shift ;;
    --no-forks)    NOFORK=1; shift ;;
    -h|--help)     sed -n '2,30p' "$0"; exit 0 ;;
    *) prc_die "unknown argument: $1" ;;
  esac
done

[[ -n "$OWNER" || -n "$SINGLE" ]] || prc_die "need --owner or --repo"
[[ -n "$SINGLE" || -n "$KIND"  ]] || prc_die "need --owner-type org|user"
prc_need git gh jq

OUT="${OUT:-$(prc_default_evidence_dir)}"
OUT="$(prc_prepare_out "$OUT")"
mkdir -p "$OUT/events" || prc_die "cannot create $OUT"
OUT="$(cd "$OUT" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PRC_STRONG="$WORK/strong.txt"
PRC_WEAK="$WORK/weak.txt"
PRC_FILENAME_RE="$WORK/filenames.txt"
prc_load_iocs "$PRC_STRONG"      "$PRC_IOC/strong.txt" "$PRC_IOC/bad-packages.txt"
prc_load_iocs "$PRC_WEAK"        "$PRC_IOC/weak.txt"
prc_load_iocs "$PRC_FILENAME_RE" "$PRC_IOC/filenames.txt"

prc_cache_token
REPOS="$WORK/repos.txt"
if [[ -n "$SINGLE" ]]; then
  echo "$SINGLE" > "$REPOS"
else
  prc_log "listing repositories for $KIND $OWNER"
  prc_list_repos "$OWNER" "$KIND" "$NOFORK" > "$REPOS"
fi
if [[ -n "$FROM_SWEEP" ]]; then
  [[ -f "$FROM_SWEEP" ]] || prc_die "no such file: $FROM_SWEEP"
  awk -F'\t' 'NR>1 && $2 != "" {print $2}' "$FROM_SWEEP" | sort -u > "$WORK/affected.txt"
  grep -Fxf "$WORK/affected.txt" "$REPOS" > "$WORK/filtered.txt" || true
  prc_log "narrowed to $(grep -c . "$WORK/filtered.txt") repositories present in $FROM_SWEEP"
  mv "$WORK/filtered.txt" "$REPOS"
fi
prc_log "$(grep -c . "$REPOS") repositories in scope"

# ---------------------------------------------------------------------------
# 1. Mirror clone + push event capture
# ---------------------------------------------------------------------------
if [[ $SCAN_ONLY -eq 0 ]]; then
  printf 'repo\tref\tbefore\tafter\tactor\tcreated_at\tsize\tforced_hint\n' > "$OUT/pushes.tsv"
  while read -r NWO; do
    [[ -z "$NWO" ]] && continue
    prc_log "$NWO"
    prc_mirror "$NWO" "$OUT/${NWO##*/}.git" || continue
    prc_log "  capturing push events"
    prc_capture_events "$NWO" "$OUT/events" "$OUT/pushes.tsv"
  done < "$REPOS"
  if [[ -n "${PRC_CLONE_FAILURES:-}" ]]; then
    printf '%s\n' "$PRC_CLONE_FAILURES" | grep -v '^$' > "$OUT/clone-failed.txt"
    prc_log "WARNING: $(grep -c . "$OUT/clone-failed.txt") repository/repositories failed to clone."
    prc_log "         They were NOT scanned. See $OUT/clone-failed.txt"
  fi
  if [[ -n "${PRC_EVENT_FAILURES:-}" ]]; then
    printf '%s\n' "$PRC_EVENT_FAILURES" | grep -v '^$' > "$OUT/events-failed.txt"
    prc_log "WARNING: the events API failed for $(grep -c . "$OUT/events-failed.txt") repository/repositories."
    prc_log "         They are listed in $OUT/events-failed.txt and have NO push history"
    prc_log "         in the ledger. That is missing evidence, not an absence of attacks."
    prc_log "         Re-run for those repositories before you conclude anything."
  fi
  prc_log "push ledger written: $OUT/pushes.tsv"
  prc_log "NOTE: the events API keeps ~300 events per repo. Capture is time-critical."
  if [[ "$KIND" == "org" && -n "$OWNER" ]]; then
    prc_log "      On GitHub Enterprise Cloud, also export the org audit log:"
    prc_log "      gh api --paginate \"/orgs/${OWNER}/audit-log?phrase=action:git.push&include=all\" > $OUT/audit-log-git-push.json"
  fi
fi

[[ $MIRROR_ONLY -eq 1 ]] && { prc_log "mirror-only mode, stopping before scan."; exit 0; }

# ---------------------------------------------------------------------------
# 2. Scan every ref
# ---------------------------------------------------------------------------
PRC_REPORT="$OUT/triage.json"
PRC_SUMMARY="$OUT/triage.txt"
# shellcheck disable=SC2034  # PRC_FIRST is read and updated by prc_scan_ref
PRC_FIRST=1
: > "$PRC_SUMMARY"
echo '[' > "$PRC_REPORT"

while read -r NWO; do
  [[ -z "$NWO" ]] && continue
  DEST="$OUT/${NWO##*/}.git"
  [[ -d "$DEST" ]] || { prc_log "no mirror for $NWO, skipping"; continue; }
  prc_log "scanning $NWO"
  while read -r REF; do
    [[ -z "$REF" ]] && continue
    prc_scan_ref "$NWO" "$DEST" "$REF"
  done < <(git -C "$DEST" for-each-ref --format='%(refname)' refs/heads/ refs/tags/ 2>/dev/null)
done < "$REPOS"

echo ']' >> "$PRC_REPORT"

INFECTED=$(jq '[.[] | select(.verdict=="INFECTED")] | length' "$PRC_REPORT" 2>/dev/null || echo "?")
REVIEW=$(jq   '[.[] | select(.verdict=="review")]   | length' "$PRC_REPORT" 2>/dev/null || echo "?")
TOTAL=$(jq    'length' "$PRC_REPORT" 2>/dev/null || echo "?")

cat <<EOF

=========================================================
 PolinRider triage complete
=========================================================
 refs scanned : $TOTAL
 INFECTED     : $INFECTED
 review       : $REVIEW
 report       : $PRC_REPORT
 summary      : $PRC_SUMMARY
 push ledger  : $OUT/pushes.tsv
 mirrors      : $OUT/*.git
$(if [[ -f "$OUT/clone-failed.txt" ]]; then
    printf ' NOT SCANNED  : %s repo(s) failed to clone, see %s\n' \
      "$(grep -c . "$OUT/clone-failed.txt")" "$OUT/clone-failed.txt"
  fi
  if [[ -f "$OUT/events-failed.txt" ]]; then
    printf ' INCOMPLETE   : events API failed for %s repo(s), see %s\n' \
      "$(grep -c . "$OUT/events-failed.txt")" "$OUT/events-failed.txt"
  fi)

$(if [[ "${PRC_EMBEDDED:-0}" != "1" ]]; then cat <<INNER

 Next:
   1. Filter out matches inside your own detection tooling:
        triage-filter.sh $PRC_REPORT
   2. Read what actually matched, do not trust the count:
        cat $PRC_SUMMARY
   3. A "clean" verdict means the current indicator set is absent from that ref.
      It is not proof the ref was never touched. Reconcile pushes.tsv against
      pushes a named person will vouch for.
INNER
fi)
=========================================================
EOF
