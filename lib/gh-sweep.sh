#!/usr/bin/env bash
# gh-sweep.sh - list every ref-touching event across an account since T0.
#               Fastest way to establish scope when the infection window is known.
#
# READ-ONLY. Changes nothing.
#
# Usage:
#   gh-sweep.sh --owner ACME --owner-type org|user --since 2026-07-27T03:00:00Z --out ./evidence
#
# Options:
#   --actor LOGIN   restrict to one GitHub login
#   --no-forks      skip forked repositories
#
# Set --since about two hours BEFORE the first push you believe was malicious.
# Start the window earlier than you think it needs to be.
#
# Output: <out>/sweep.tsv   time, repo, event, actor, ref, before, after, size

set -uo pipefail
# shellcheck source=lib/common.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

OWNER=""; KIND=""; SINCE=""; OUT="./evidence"; ACTOR=""; NOFORK=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --owner)      OWNER="$2"; shift 2 ;;
    --owner-type) KIND="$2"; shift 2 ;;
    --since)      SINCE="$2"; shift 2 ;;
    --out)        OUT="$2"; shift 2 ;;
    --actor)      ACTOR="$2"; shift 2 ;;
    --no-forks)   NOFORK=1; shift ;;
    -h|--help)    sed -n '2,20p' "$0"; exit 0 ;;
    *) prc_die "unknown argument: $1" ;;
  esac
done

[[ -n "$OWNER" && -n "$KIND" && -n "$SINCE" ]] || prc_die "need --owner, --owner-type and --since"
prc_need gh jq

mkdir -p "$OUT/events" || prc_die "cannot create $OUT"
SWEEP="$OUT/sweep.tsv"
printf 'time\trepo\tevent\tactor\tref\tbefore\tafter\tsize\n' > "$SWEEP"

REPOS="$(prc_list_repos "$OWNER" "$KIND" "$NOFORK")"
printf 'repos in scope: %s\n' "$(printf '%s\n' "$REPOS" | grep -c .)" >&2

for NWO in $REPOS; do
  NAME="${NWO##*/}"
  printf '  sweeping %s\n' "$NWO" >&2
  gh api "/repos/${NWO}/events?per_page=100" --paginate \
    > "$OUT/events/${NAME}.json" 2>/dev/null || echo '[]' > "$OUT/events/${NAME}.json"

  jq -r --arg repo "$NWO" --arg since "$SINCE" --arg actor "$ACTOR" '
    (if type=="array" then . else [] end)[]
    | select(.created_at >= $since)
    | select($actor == "" or .actor.login == $actor)
    | select(.type=="PushEvent" or .type=="CreateEvent" or .type=="DeleteEvent")
    | [ .created_at, $repo, .type, (.actor.login // ""),
        (.payload.ref // ""),
        (.payload.before // ""), (.payload.head // ""),
        ((.payload.size // 0)|tostring) ] | @tsv
  ' "$OUT/events/${NAME}.json" >> "$SWEEP" 2>/dev/null
done

echo
echo "=== Ref-touching activity since $SINCE ==="
{ head -1 "$SWEEP"; tail -n +2 "$SWEEP" | sort; } | column -t -s"$(printf '\t')" 2>/dev/null \
  || { head -1 "$SWEEP"; tail -n +2 "$SWEEP" | sort; }

CREATED="$(awk -F'\t' 'NR>1 && $3=="CreateEvent" && $5 != "" {print $2"\t"$5}' "$SWEEP" | sort -u)"
if [[ -n "$CREATED" ]]; then
  echo
  echo "=== Branches CREATED inside the window ==="
  echo "These are attacker artifacts unless someone claims them. A restore does"
  echo "not remove them, because there is no prior commit to restore to. Confirm"
  echo "each one with its supposed author, then delete what nobody claims:"
  echo
  printf '%s\n' "$CREATED" | while IFS=$'\t' read -r r ref; do
    printf '  gh api -X DELETE "/repos/%s/git/refs/heads/%s"\n' "$r" "${ref#refs/heads/}"
  done
fi

echo
echo "$(tail -n +2 "$SWEEP" | grep -c .) events. Full ledger: $SWEEP"
cat <<'EOF'

How to read this
----------------
Every PushEvent needs a named owner who will say "yes, that was me, at that time,
on that branch". Anything unclaimed is the attacker.

CreateEvent with a branch ref inside the window means the malware made a new
branch. Those are attacker artifacts. Delete them rather than restoring them.

For each unclaimed PushEvent, `before` is the restore target. The refs API rejects
short SHAs, so expand before use:

  gh api /repos/OWNER/REPO/commits/<short> --jq .sha

If every branch in a repo moved to the same `after` SHA with size 0, the malware
made one commit and pointed every ref at it. You then only need to read one diff
per repo to confirm the payload, not one per branch.
EOF
