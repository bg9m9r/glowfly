#!/usr/bin/env bash
# One smoke pass for scripts/smoke-linux.sh: a fresh fixture repository, config and HOME, the
# app started on them at one scale factor, then each check in order. Each check runs in a
# subshell under its own id, so a failure names it; a check listed in SMOKE_EXPECT_FAIL only
# warns. The labels looked up here are dry-run on every OS by the journey test
# crates/glowfly-app/src/journey/probe.rs; keep the two in step.
#
# Uses the globals of smoke-linux.sh and the helpers of lib.sh.
# shellcheck disable=SC2154,SC2034,SC2016  # jq filters name their own $variables

# run_pass <scale>
run_pass() {
  SCALE=$1
  PASS="scale-$SCALE"
  PASS_OUT="$SMOKE_OUT/$PASS"
  PASS_DIR="$WORK/$PASS"
  CHECK=startup
  mkdir -p "$PASS_OUT" "$PASS_DIR"
  log "pass at scale $SCALE"
  isolate
  REPO="$PASS_DIR/repo"
  make_fixture "$REPO"
  A_OID=$(git -C "$REPO" rev-list --max-parents=0 HEAD)
  B_OID=$(git -C "$REPO" rev-parse HEAD~1)
  PROBE="$PASS_DIR/probe.json"
  start_app

  run_check 01 scale check_scale
  run_check 02 first-click check_first_click
  run_check 03 commit-box check_commit_box
  run_check 04 context-menu check_context_menu
  run_check 05 shortcut check_shortcut
  run_check 06 drag check_drag
  run_check 07 resize check_resize
  CHECK=stop
  stop_app
  log "pass at scale $SCALE passed"
}

# run_check <NN> <id> <function>: settles and runs the check in a subshell (with errexit
# and the ERR trap inside it only, so an expected failure can go on), takes NN-<id>.png.
run_check() {
  local nn=$1 status
  CHECK=$2
  log "check $CHECK"
  trap - ERR
  set +e
  (
    arm_err_trap
    set -e
    settle
    "$3"
  )
  status=$?
  set -e
  arm_err_trap
  if [ "$status" -ne 0 ]; then
    expected "$CHECK" || exit 1
    log "check $CHECK failed as expected; going on"
  fi
  shot "$nn-$CHECK"
}

# The app's and git's files all inside the pass folder, English labels, and settings that keep
# the welcome, release notes and update check away.
isolate() {
  export HOME="$PASS_DIR/home"
  export XDG_CONFIG_HOME="$PASS_DIR/config" XDG_CACHE_HOME="$PASS_DIR/cache"
  export XDG_STATE_HOME="$PASS_DIR/state" XDG_DATA_HOME="$PASS_DIR/data" XDG_RUNTIME_DIR="$PASS_DIR/run"
  mkdir -p "$HOME" "$XDG_CONFIG_HOME/glowfly" "$XDG_CACHE_HOME" "$XDG_STATE_HOME" "$XDG_DATA_HOME"
  mkdir -m 700 "$XDG_RUNTIME_DIR"
  export GIT_CONFIG_GLOBAL="$PASS_DIR/gitconfig" GIT_CONFIG_NOSYSTEM=1
  git config --file "$GIT_CONFIG_GLOBAL" user.name Smoke
  git config --file "$GIT_CONFIG_GLOBAL" user.email smoke@example.com
  git config --file "$GIT_CONFIG_GLOBAL" init.defaultBranch main
  git config --file "$GIT_CONFIG_GLOBAL" commit.gpgsign false
  git config --file "$GIT_CONFIG_GLOBAL" core.autocrlf false
  export LANG=C.UTF-8 LC_ALL=C.UTF-8
  unset LANGUAGE
  local version
  version=$(tr -d '[:space:]' <"$ROOT/version.txt")
  jq -n --arg v "$version" \
    '{welcome_done: true, reopen_last_repo: false, updates: {check: false, last_checked: null, notes_seen: $v}}' \
    >"$XDG_CONFIG_HOME/glowfly/settings.json"
}

# fixture_commit <second> <file> <text> <message>: one commit at a fixed time.
fixture_commit() {
  printf '%s\n' "$3" >"$REPO/$2"
  git -C "$REPO" add "$2"
  GIT_AUTHOR_DATE="2024-01-01T10:00:0$1" GIT_COMMITTER_DATE="2024-01-01T10:00:0$1" \
    git -C "$REPO" commit -q -m "$4"
}

# make_fixture <dir>: three commits, a staged new file and an unstaged edit.
make_fixture() {
  git init -q -b main "$1"
  fixture_commit 1 a.txt a "Add a.txt"
  fixture_commit 2 b.txt b "Add b.txt"
  fixture_commit 3 notes.txt "$(seq 1 20 | sed 's/^/line /')" "Add notes"
  printf 'buy milk\n' >"$REPO/todo.txt"
  git -C "$REPO" add todo.txt
  printf 'line 21\n' >>"$REPO/notes.txt"
}

start_app() {
  WINIT_X11_SCALE_FACTOR="$SCALE" GLOWFLY_SMOKE_PROBE="$PROBE" RUST_LOG=glowfly=debug \
    "$BIN" "$REPO" >"$PASS_OUT/app.log" 2>&1 &
  APP_PID=$!
  WID=$(timeout 60 xdotool search --sync --onlyvisible --pid "$APP_PID" 2>/dev/null | head -n1 || true)
  if [ -z "$WID" ]; then
    WID=$(xdotool search --onlyvisible --class glowfly 2>/dev/null | head -n1 || true)
  fi
  [ -n "$WID" ] || fail startup "no window of the app appeared within 60 s"
  xdotool windowmove "$WID" 0 0
  probe_wait "the repository to load with the row 'Add notes'" \
    '.repo != null and .busy == false and any(.widgets[]; (.label // .value) == "Add notes" and .region == "central")'
  [ "$(probe .modal)" = false ] || fail startup "a startup dialog is in the way"
  # A second client to take the focus away: placed off Glowfly's window.
  xmessage -geometry +2350+1400 "focus thief" &
  THIEF_PID=$!
  THIEF=$(timeout 20 xdotool search --sync --onlyvisible --class xmessage 2>/dev/null | head -n1 || true)
  [ -n "$THIEF" ] || fail startup "the focus thief window (xmessage) did not appear"
  xdotool windowactivate --sync "$WID"
  probe_wait "the window to have the focus" '.focused == true'
}

stop_app() {
  kill -TERM "$APP_PID" 2>/dev/null || true
  local i
  for ((i = 0; i < 100; i++)); do
    kill -0 "$APP_PID" 2>/dev/null || break
    sleep 0.1
  done
  kill -KILL "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  kill "$THIEF_PID" 2>/dev/null || true
  wait "$THIEF_PID" 2>/dev/null || true
  APP_PID="" THIEF_PID="" WID=""
}

# winit took the scale factor from WINIT_X11_SCALE_FACTOR (it falls back to 1 without a
# RandR output), and the zoom is 100%.
check_scale() {
  probe_wait "native_pixels_per_point $SCALE and pixels_per_point $SCALE x zoom" \
    'def dist(a; b): if a > b then a - b else b - a end;
     .zoom_percent == 100 and dist(.native_pixels_per_point; $s) <= 0.01
       and dist(.pixels_per_point; $s * .zoom_percent / 100) <= 0.01' --argjson s "$SCALE"
}

# The click that gives the window the focus must also reach it, (a) when the click itself
# focuses the window and (b) right after the window manager focused it.
check_first_click() {
  xdotool windowactivate --sync "$THIEF"
  probe_wait "(a) the window to lose the focus to xmessage" '.focused == false'
  point_at "Add b.txt" central
  sleep 0.2
  xdotool click 1
  probe_wait "(a) click to focus: the click to give the window the focus" '.focused == true'
  probe_wait "(a) click to focus: that same click to select 'Add b.txt' (the first click was dropped)" \
    '.selection == $sel' --arg sel "commit:$B_OID"

  xdotool windowactivate --sync "$THIEF"
  probe_wait "(b) the window to lose the focus to xmessage" '.focused == false'
  xdotool windowactivate --sync "$WID"
  probe_wait "(b) focus from the window manager" '.focused == true'
  settle
  click_widget "Add a.txt" central
  probe_wait "(b) focus from the window manager, then click: the click to select 'Add a.txt' (the first click was dropped)" \
    '.selection == $sel' --arg sel "commit:$A_OID"
}

summary_is() { [ "$(git -C "$REPO" log -1 --format=%s)" = "$1" ]; }
only_todo_committed() { [ "$(git -C "$REPO" show --name-only --format= HEAD)" = todo.txt ]; }
notes_still_modified() { [ "$(git -C "$REPO" diff --name-only)" = notes.txt ]; }

# Typing real keys into the commit summary, then committing the staged file only.
check_commit_box() {
  local text="Smoke: typed with real keys (scale $SCALE)"
  click_widget "Uncommitted changes" central
  probe_wait "the working tree's row to be selected" '.selection == "wip"'
  settle
  click_widget "Commit summary" details 1 TextInput
  probe_wait "the commit summary to have the keyboard focus" '.keyboard_focus.label == "Commit summary"'
  xdotool type --delay 40 "$text"
  probe_wait "the typed summary to read exactly '$text' (dropped, doubled or wrongly shifted keys)" \
    '.draft_summary == $t' --arg t "$text"
  settle
  click_widget "prefix:Commit 1 file" details
  probe_wait "the toast 'Commit succeeded'" 'any(.toasts[]; .ok and (.text | startswith("Commit succeeded")))'
  wait_disk "the commit's summary on disk to be '$text'" summary_is "$text"
  only_todo_committed || fail "$CHECK" "the commit holds $(git -C "$REPO" show --name-only --format= HEAD | tr '\n' ' '), not only todo.txt"
  notes_still_modified || fail "$CHECK" "notes.txt is no longer modified in the working tree"
}

branch_at() { [ "$(git -C "$REPO" rev-parse --verify -q refs/heads/smoke-branch)" = "$1" ]; }

# A right-click opens the commit's menu; its item opens a dialog whose field takes typing and
# Enter.
check_context_menu() {
  local head summary
  head=$(git -C "$REPO" rev-parse HEAD)
  summary=$(git -C "$REPO" log -1 --format=%s)
  click_widget "$summary" central 3
  wait_widget "Create branch here…" any
  settle
  click_widget "Create branch here…" any
  probe_wait "the new-branch dialog with its field focused" '.modal == true and .keyboard_focus.role == "TextInput"'
  xdotool type --delay 40 smoke-branch
  xdotool key Return
  wait_disk "refs/heads/smoke-branch at $head" branch_at "$head"
  probe_wait "the dialog to close" '.modal == false'
}

# Ctrl+Shift+L hides and shows the left panel, saved in the settings.
check_shortcut() {
  xdotool key ctrl+shift+l
  settings_wait "show_left_panel false" '.show_left_panel == false'
  probe_wait "the left panel to be hidden" '.panels.refs == null'
  settle
  xdotool key ctrl+shift+l
  settings_wait "show_left_panel true" '.show_left_panel == true'
  probe_wait "the left panel to be back" '.panels.refs != null'
}

# Dragging the sidebar's edge 100 points to the right widens it, and the width is saved.
check_drag() {
  local x1 y0 y1 old x y step width
  read -r x1 y0 y1 < <(probe '.panels.refs | "\(.[2]) \(.[1]) \(.[3])"')
  old=$(probe '.panels.refs | .[2] - .[0]')
  x=$(px "$x1")
  y=$(px "$(awk -v a="$y0" -v b="$y1" 'BEGIN { print (a + b) / 2 }')")
  step=$(px 20)
  drag_px "$x" "$y" $((step * 5)) 5
  probe_wait "the sidebar to be 100 points wider than $old (within 4)" \
    '(.panels.refs | .[2] - .[0]) as $w | ($w - $old - 100) | (. <= 4 and . >= -4)' --argjson old "$old"
  settle
  width=$(probe '.panels.refs | .[2] - .[0]')
  settings_wait "panel_sizes.sidebar within 4 of $width" \
    '.panel_sizes.sidebar != null and ((.panel_sizes.sidebar - $w) | (. <= 4 and . >= -4))' --argjson w "$width"
}

# A resized window lays out to its new size and still maps the pointer right.
check_resize() {
  local w h
  w=$(awk -v s="$SCALE" 'BEGIN { printf "%d", 1200 * s + 0.5 }')
  h=$(awk -v s="$SCALE" 'BEGIN { printf "%d", 720 * s + 0.5 }')
  xdotool windowsize "$WID" "$w" "$h"
  probe_wait "the window to be 1200 x 720 points (within 2)" \
    '(.screen[0] - 1200 | . <= 2 and . >= -2) and (.screen[1] - 720 | . <= 2 and . >= -2)'
  settle
  probe_wait "the layout to fit the new size" \
    '.panels.refs != null and .panels.detail != null and .panels.refs[2] <= .panels.detail[0]
      and .panels.detail[2] <= .screen[0] + 1'
  click_widget "Add b.txt" central
  probe_wait "a click after the resize to select 'Add b.txt'" '.selection == $sel' --arg sel "commit:$B_OID"
}
