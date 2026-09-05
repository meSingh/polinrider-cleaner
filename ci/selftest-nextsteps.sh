#!/usr/bin/env bash
# selftest-nextsteps.sh - offline tests for the evidence location and the
# generated next steps. No network, no GitHub, no side effects outside $TMPDIR.
#
# The bug these exist to prevent: printing a command that does not run. An
# earlier version emitted "--since <T0>" and "<two hours before the first bad
# push>" as literal text, which fails the moment anyone pastes it.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=lib/common.sh
. "$ROOT/lib/common.sh"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/prc-nextsteps.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT
PASS=0; FAIL=0
ok()   { PASS=$((PASS+1)); printf '  ok   %s\n' "$*"; }
no()   { FAIL=$((FAIL+1)); printf '  FAIL %s\n' "$*"; }
check(){ if eval "$2" >/dev/null 2>&1; then ok "$1"; else no "$1"; fi; }

echo "== timestamps =="
T=$(prc_shift_back_2h 2026-08-10T21:14:20Z || true)
[[ "$T" == "2026-08-10T19:14:20Z" ]] && ok "two hours back is exact" || no "two hours back: got '$T'"
# BSD date silently returns ctime format if -v follows -f. That string used to
# reach a printed --since. The helper must reject anything non-ISO.
prc_shift_back_2h "Mon Aug 10 21:14:20 UTC 2026" >/dev/null 2>&1 \
  && no "ctime input rejected" || ok "ctime input rejected"
prc_shift_back_2h "" >/dev/null 2>&1 && no "empty input rejected" || ok "empty input rejected"
prc_shift_back_2h "2026-08-10" >/dev/null 2>&1 && no "date-only rejected" || ok "date-only rejected"
prc_shift_back_2h "; rm -rf /" >/dev/null 2>&1 && no "shell metacharacters rejected" || ok "shell metacharacters rejected"

echo "== evidence location =="
REPO="$TMP/a-checkout"; mkdir -p "$REPO"; git -C "$REPO" init -q 2>/dev/null
( POLINRIDER_ALLOW_UNSAFE_OUT=0; prc_assert_safe_out "$REPO/evidence" ) >/dev/null 2>&1 \
  && no "refuses evidence inside a git checkout" || ok "refuses evidence inside a git checkout"
# shellcheck disable=SC2034  # read by prc_assert_safe_out inside the subshell
( POLINRIDER_ALLOW_UNSAFE_OUT=1; prc_assert_safe_out "$REPO/evidence" ) >/dev/null 2>&1 \
  && ok "override lets it through" || no "override lets it through"
( prc_assert_safe_out "$TMP/loose/evidence" ) >/dev/null 2>&1 \
  && ok "allows a path outside any checkout" || no "allows a path outside any checkout"
D="$(prc_prepare_out "$TMP/made")"
[[ -d "$TMP/made" && "$D" == /* && "$D" -ef "$TMP/made" ]] \
  && ok "creates and echoes an absolute path" || no "creates and echoes an absolute path: got '$D'"
if stat --version >/dev/null 2>&1; then MODE=$(stat -c '%a' "$TMP/made"); else MODE=$(stat -f '%Lp' "$TMP/made"); fi
[[ "$MODE" == "700" ]] \
  && ok "evidence directory is mode 700" || no "evidence directory is mode 700"

# --- fixtures ---------------------------------------------------------------
mk_triage() {  # mk_triage <file> <repo>
  cat > "$1" <<JSON
[
 {"repo":"$2","ref":"refs/heads/main","verdict":"INFECTED",
  "ioc_strings":["refs/heads/main:public/fonts/fa-solid-400.woff2:1:rmcej%otb%"],
  "ioc_filenames":[],"font_masquerade":[],"weak_signals":[],"config_tail":[]},
 {"repo":"$2","ref":"refs/heads/dev","verdict":"INFECTED",
  "ioc_strings":["refs/heads/dev:.vscode/tasks.json:3:folderOpen"],
  "ioc_filenames":[],"font_masquerade":[],"weak_signals":[],"config_tail":[]},
 {"repo":"acme/scanner","ref":"refs/heads/main","verdict":"INFECTED",
  "ioc_strings":["refs/heads/main:ioc/strong.txt:5:rmcej%otb%"],
  "ioc_filenames":[],"font_masquerade":[],"weak_signals":[],"config_tail":[]},
 {"repo":"acme/fine","ref":"refs/heads/main","verdict":"clean",
  "ioc_strings":[],"ioc_filenames":[],"font_masquerade":[],"weak_signals":[],"config_tail":[]}
]
JSON
}

echo "== no push events: the committed-payload case =="
E1="$TMP/e1"; mkdir -p "$E1"; mk_triage "$E1/triage.json" acme/app
printf 'repo\tref\tbefore\tafter\tactor\tcreated_at\tsize\tforced_hint\n' > "$E1/pushes.tsv"
# shellcheck disable=SC2034  # read by check() through eval
OUT1="$("$ROOT/lib/next-steps.sh" --triage "$E1/triage.json" --out "$E1" --owner acme --owner-type org 2>&1)"
D1="$E1/NEXT-STEPS.md"
check "writes NEXT-STEPS.md"                  "[[ -s '$D1' ]]"
check "counts one repo, two branches"         "grep -q '1 repositories, 2 branches' <<< \"\$OUT1\""
check "own detection tooling is excluded"     "! grep -q 'acme/scanner' '$E1/affected-repos.txt'"
check "clean verdicts are excluded"           "! grep -q 'acme/fine' '$E1/affected-repos.txt'"
check "says there is nothing to restore to"   "grep -q 'no earlier state to restore to' '$D1'"
check "warns off sweep.sh by name"            "grep -q 'Do not run sweep.sh' <<< \"\$OUT1\""
check "offers no runnable sweep command"      "! grep -qE 'sweep\\.sh (--|.*--since)' <<< \"\$OUT1\""
check "document offers no sweep either"       "! grep -q 'sweep.sh --' '$D1'"
check "does not offer restore.sh"             "! grep -q 'restore.sh --sweep' '$D1'"
check "offers the org wrapper, not the user one" "grep -q 'github-org-recovery/clean-repo.sh' '$D1'"
check "no printf option errors"               "! grep -qi 'invalid option' <<< \"\$OUT1\""

echo "== push events present: the force-push case =="
E2="$TMP/e2"; mkdir -p "$E2"; mk_triage "$E2/triage.json" acme/app
{ printf 'repo\tref\tbefore\tafter\tactor\tcreated_at\tsize\tforced_hint\n'
  printf 'acme/app\trefs/heads/main\taaa\tbbb\tmallory\t2026-08-10T21:14:20Z\t0\t\n'
  printf 'acme/app\trefs/heads/main\tccc\tddd\tmallory\t2026-09-01T10:00:00Z\t0\t\n'
} > "$E2/pushes.tsv"
# shellcheck disable=SC2034  # read by check() through eval
OUT2="$("$ROOT/lib/next-steps.sh" --triage "$E2/triage.json" --out "$E2" --owner acme --owner-type user 2>&1)"
D2="$E2/NEXT-STEPS.md"
check "T0 is the earliest event minus two hours" "grep -q -- '--since 2026-08-10T19:14:20Z' '$D2'"
check "names the actor"                          "grep -q 'mallory' '$D2'"
check "counts the zero-commit pushes"            "grep -q 'carried \*\*zero commits\*\*' '$D2'"
check "lists the restore candidate"              "grep -q '^acme/app$' '$E2/restorable-repos.txt'"
check "T0 also appears on screen"                "grep -q -- '--since 2026-08-10T19:14:20Z' <<< \"\$OUT2\""
check "offers the sweep"                         "grep -q 'sweep.sh --user acme' '$D2'"
check "offers the user wrapper"                  "grep -q 'github-account-recovery/clean-repo.sh' '$D2'"

echo "== a push to an unflagged branch is not evidence =="
E3="$TMP/e3"; mkdir -p "$E3"; mk_triage "$E3/triage.json" acme/app
{ printf 'repo\tref\tbefore\tafter\tactor\tcreated_at\tsize\tforced_hint\n'
  printf 'acme/app\trefs/heads/unrelated\taaa\tbbb\tdana\t2026-08-10T21:14:20Z\t3\t\n'
} > "$E3/pushes.tsv"
# shellcheck disable=SC2034  # read by check() through eval
OUT3="$("$ROOT/lib/next-steps.sh" --triage "$E3/triage.json" --out "$E3" --owner acme --owner-type user 2>&1)"
check "unflagged-branch push is not a restore candidate" "[[ ! -s '$E3/restorable-repos.txt' ]]"
check "falls back to the removal path"                   "grep -q 'no earlier state to restore to' '$E3/NEXT-STEPS.md'"
check "offers no runnable sweep"                         "! grep -q 'sweep.sh --' '$E3/NEXT-STEPS.md'"

echo "== every printed command is runnable =="
# The whole point. No angle-bracket placeholders anywhere in either document,
# and every fenced bash line must survive the shell parser.
for d in "$D1" "$D2" "$E3/NEXT-STEPS.md"; do
  grep -qE '<[A-Za-z0-9_ .-]+>' "$d" \
    && no "no placeholders left in $(basename "$(dirname "$d")")/NEXT-STEPS.md" \
    || ok "no placeholders left in $(basename "$(dirname "$d")")/NEXT-STEPS.md"
  bad=0
  while IFS= read -r line; do
    [[ -z "$line" || "$line" == \#* ]] && continue
    bash -n <<< "$line" 2>/dev/null || { bad=1; printf '       unparseable: %s\n' "$line"; }
  done < <(awk '/^```bash$/{f=1;next} /^```$/{f=0} f' "$d")
  [[ $bad -eq 0 ]] && ok "every bash block parses in $(basename "$(dirname "$d")")" \
                   || no "every bash block parses in $(basename "$(dirname "$d")")"
done

echo "== the cleaner refuses to work inside a checkout =="
"$ROOT/lib/gh-clean.sh" acme/app --out "$REPO/evidence" >/dev/null 2>&1 \
  && no "gh-clean refuses an unsafe --out" || ok "gh-clean refuses an unsafe --out"
check "gh-clean never force-pushes" "! grep -qE 'push .*(--force|-f )' '$ROOT/lib/gh-clean.sh'"

printf '\n  passed %s, failed %s\n' "$PASS" "$FAIL"
[[ "$FAIL" -eq 0 ]] || exit 1
