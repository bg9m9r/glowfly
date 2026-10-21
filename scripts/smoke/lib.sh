#!/usr/bin/env bash
# Helpers for scripts/smoke-linux.sh: waiting on the app's probe file and settings, finding
# widgets by their accessible names, real X11 input through xdotool, and the failure report.
# Defines functions only. Input goes through XTEST: key, type and click never take --window
# (that would send synthetic events with XSendEvent); --window only gives mousemove
# window-relative coordinates.
#
# Uses the globals smoke-linux.sh and pass.sh set: PROBE, WID, APP_PID, PASS, PASS_OUT, CHECK,
# REPO, SMOKE_TIMEOUT, SMOKE_EXPECT_FAIL, XDG_CONFIG_HOME, XDG_STATE_HOME.
# shellcheck disable=SC2154,SC2016  # jq filters name their own $variables

log() {
  printf '[smoke %s %s] %s\n' "$(date +%T)" "${PASS:-}" "$*" >&2
}

# An unexpected failing command fails the check that ran it.
arm_err_trap() {
  trap 'fail "$CHECK" "unexpected error at line $LINENO: $BASH_COMMAND"' ERR
}

# Whether check id $1 is listed in SMOKE_EXPECT_FAIL.
expected() {
  case " ${SMOKE_EXPECT_FAIL:-} " in *" $1 "*) return 0 ;; esac
  return 1
}

# fail <check-id> <message>: saves screenshots, the probe, the logs, the settings and the
# fixture's git state, prints an annotation and exits 1 (the check's subshell, or the script).
# An expected failure reports into expected-<id>/ as a warning. Only the first call per check
# reports; a second (an ERR trap after a failed command substitution) just exits.
fail() {
  local id=$1 msg=$2 level=error dir="$PASS_OUT"
  trap - ERR
  set +e
  if expected "$id"; then
    level=warning
    dir="$PASS_OUT/expected-$id"
  fi
  mkdir -p "$dir"
  if [ ! -e "$dir/.reported-$id" ]; then
    : >"$dir/.reported-$id"
    collect "$dir"
    printf '::%s::smoke %s, check %s: %s\n' "$level" "${PASS:-}" "$id" "$msg" >&9
    log "FAILED $id: $msg"
  fi
  exit 1
}

# Everything a failure report holds, into folder $1.
collect() {
  local dir=$1
  if [ -n "${WID:-}" ]; then
    import -window "$WID" "$dir/failure.png" 2>/dev/null || import -window root "$dir/failure.png" 2>/dev/null
  else
    import -window root "$dir/failure.png" 2>/dev/null
  fi
  import -window root "$dir/screen.png" 2>/dev/null
  [ -f "${PROBE:-}" ] && cp "$PROBE" "$dir/probe.json"
  [ -f "$PASS_OUT/app.log" ] && [ "$dir" != "$PASS_OUT" ] && cp "$PASS_OUT/app.log" "$dir/app.log"
  [ -n "${XDG_STATE_HOME:-}" ] && cp "$XDG_STATE_HOME"/glowfly/logs/* "$dir/" 2>/dev/null
  [ -n "${XDG_CONFIG_HOME:-}" ] && cp "$XDG_CONFIG_HOME/glowfly/settings.json" "$dir/settings.json" 2>/dev/null
  if [ -d "${REPO:-}/.git" ]; then
    git -C "$REPO" log --all --oneline >"$dir/git-log.txt" 2>&1
    git -C "$REPO" status >"$dir/git-status.txt" 2>&1
  fi
  return 0
}

# shot <name>: a screenshot of the window into the pass's folder.
shot() {
  import -window "$WID" "$PASS_OUT/$1.png" 2>/dev/null || log "could not take screenshot $1"
}

# probe <jq args and filter>: jq -r on the probe file.
probe() {
  jq -r "$@" "$PROBE"
}

# What a timeout message shows of the probe.
probe_state() {
  jq -c '{seq, focused, busy, selection, modal, context_menu, popup, keyboard_focus, draft_summary,
    screen, pixels_per_point, native_pixels_per_point, panels, toasts}' "$PROBE" 2>/dev/null || echo "(no probe file)"
}

# The app must still be running.
app_alive() {
  kill -0 "$APP_PID" 2>/dev/null || fail "$CHECK" "the app exited (see app.log)"
}

# probe_wait <description> <jq filter> [jq args...]: waits until the filter is true.
probe_wait() {
  local desc=$1 filter=$2
  shift 2
  local deadline=$((SECONDS + SMOKE_TIMEOUT))
  until [ -s "$PROBE" ] && jq -e "$@" "$filter" "$PROBE" >/dev/null 2>&1; do
    app_alive
    [ "$SECONDS" -lt "$deadline" ] || fail "$CHECK" "timed out after ${SMOKE_TIMEOUT}s waiting for $desc; probe: $(probe_state)"
    sleep 0.1
  done
}

# settings_wait <description> <jq filter> [jq args...]: the same on settings.json.
settings_wait() {
  local desc=$1 filter=$2 file="$XDG_CONFIG_HOME/glowfly/settings.json"
  shift 2
  local deadline=$((SECONDS + SMOKE_TIMEOUT))
  until [ -s "$file" ] && jq -e "$@" "$filter" "$file" >/dev/null 2>&1; do
    [ "$SECONDS" -lt "$deadline" ] || fail "$CHECK" "timed out after ${SMOKE_TIMEOUT}s waiting for $desc in settings.json"
    sleep 0.1
  done
}

# wait_disk <description> <command...>: waits until the command succeeds.
wait_disk() {
  local desc=$1
  shift
  local deadline=$((SECONDS + SMOKE_TIMEOUT))
  until "$@" >/dev/null 2>&1; do
    [ "$SECONDS" -lt "$deadline" ] || fail "$CHECK" "timed out after ${SMOKE_TIMEOUT}s waiting for $desc"
    sleep 0.1
  done
}

# Before every step: nothing running and the probe unchanged for 300 ms.
settle() {
  local deadline=$((SECONDS + SMOKE_TIMEOUT)) last="" now stable=0
  while [ "$stable" -lt 3 ]; do
    app_alive
    now=$(jq -r 'if .busy then "busy" else (.seq | tostring) end' "$PROBE" 2>/dev/null || true)
    if [ -n "$now" ] && [ "$now" != busy ] && [ "$now" = "$last" ]; then
      stable=$((stable + 1))
    else
      stable=0
    fi
    last=$now
    [ "$SECONDS" -lt "$deadline" ] || fail "$CHECK" "the app did not settle (busy, or the probe kept changing); probe: $(probe_state)"
    sleep 0.1
  done
}

# The jq selection of widgets named $l (exactly, or starting with it when $m is "prefix") in
# region $r ("any" for all) with role $role (empty for any).
WIDGETS_NAMED='[.widgets[]
  | select((.label // .value) as $n | $n != null
      and (if $m == "prefix" then ($n | startswith($l)) else $n == $l end))
  | select($r == "any" or .region == $r)
  | select($role == "" or .role == $role)]'

# find_widget <label|prefix:text> <region|any> [role]: prints the widget's centre in window
# pixels, "x y". Fails unless exactly one widget matches (PICK=first takes the first of many).
find_widget() {
  local label=$1 region=$2 role=${3:-} mode=exact matches n
  case $label in prefix:*) mode=prefix label=${label#prefix:} ;; esac
  matches=$(jq -c --arg l "$label" --arg r "$region" --arg role "$role" --arg m "$mode" "$WIDGETS_NAMED" "$PROBE")
  n=$(jq 'length' <<<"$matches")
  if [ "$n" -eq 0 ] || { [ "$n" -gt 1 ] && [ "${PICK:-only}" != first ]; }; then
    fail "$CHECK" "$n widgets named '$label' (${role:-any role}) in $region; names there: $(jq -c --arg r "$region" \
      '[.widgets[] | select($r == "any" or .region == $r) | (.label // .value)]' "$PROBE")"
  fi
  jq -r --argjson ppp "$(probe .pixels_per_point)" \
    '.[0].rect | "\((.[0] + .[2]) / 2 * $ppp + 0.5 | floor) \((.[1] + .[3]) / 2 * $ppp + 0.5 | floor)"' <<<"$matches"
}

# wait_widget <label|prefix:text> <region|any> [role]: waits until such a widget is listed.
wait_widget() {
  local label=$1 region=$2 role=${3:-} mode=exact
  case $label in prefix:*) mode=prefix label=${label#prefix:} ;; esac
  probe_wait "a widget named '$label' in $region" "$WIDGETS_NAMED | length > 0" \
    --arg l "$label" --arg r "$region" --arg role "$role" --arg m "$mode"
}

# point_at <label> <region> [role]: moves the pointer onto the widget.
point_at() {
  local xy x y
  xy=$(find_widget "$1" "$2" "${3:-}")
  read -r x y <<<"$xy"
  xdotool mousemove --window "$WID" "$x" "$y"
}

# click_widget <label> <region> [button] [role]: moves onto the widget and clicks it.
click_widget() {
  point_at "$1" "$2" "${4:-}"
  sleep 0.1
  xdotool click "${3:-1}"
}

# drag_px <x> <y> <dx> <steps>: presses button 1 at (x, y) in window pixels, moves right by dx
# in `steps` moves 50 ms apart, releases.
drag_px() {
  local x=$1 y=$2 dx=$3 steps=$4 i
  xdotool mousemove --window "$WID" "$x" "$y"
  sleep 0.2
  xdotool mousedown 1
  sleep 0.1
  for ((i = 1; i <= steps; i++)); do
    xdotool mousemove --window "$WID" $((x + dx * i / steps)) "$y"
    sleep 0.05
  done
  sleep 0.1
  xdotool mouseup 1
}

# px <points>: window pixels for a length in points, rounded.
px() {
  awk -v p="$1" -v s="$(probe .pixels_per_point)" 'BEGIN { printf "%d", int(p * s + 0.5) }'
}
