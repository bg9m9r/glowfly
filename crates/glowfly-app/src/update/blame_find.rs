//! Moving within one blame: find in the file, go to line, and the next or previous run.

use crate::state::AppState;
use crate::state::blame_find::typed_line;

/// Searches again for the find field's text and goes to the first match at or after the top
/// line (else the first).
pub(super) fn find_changed(st: &mut AppState) {
    let Some(view) = st.blame.as_mut() else { return };
    let line = match &view.data {
        Some(d) => view.find.refresh(&d.blame.lines, view.top_line),
        None => view.find.refresh(&[], 0),
    };
    if let Some(line) = line {
        view.land_on(line);
    }
}

/// To the next (`d` > 0) or previous match, wrapping round.
pub(super) fn find_step(st: &mut AppState, d: isize) {
    let Some(view) = st.blame.as_mut() else { return };
    if let Some(line) = view.find.step(d) {
        view.land_on(line);
    }
}

pub(super) fn focus_find(st: &mut AppState) {
    if let Some(view) = st.blame.as_mut() {
        view.focus_find = true;
    }
}

/// Goes to the 1-based line typed in the Line field, clamped into the file; other text is
/// ignored.
pub(super) fn go_to_line(st: &mut AppState) {
    let Some(view) = st.blame.as_mut() else { return };
    if let Some(line) = typed_line(&view.goto, view.line_count()) {
        view.land_on(line);
    }
}

/// To the first line of the next (`d` > 0) or previous run, from the cursor (else the top
/// line); stops at the ends.
pub(super) fn step_run(st: &mut AppState, d: isize) {
    let Some(view) = st.blame.as_mut() else { return };
    let Some(data) = &view.data else { return };
    let runs = &data.blame.runs;
    let base = view.cursor.unwrap_or(view.top_line);
    let Some(k) = data.blame.run_of(base) else { return };
    let target = if d > 0 {
        runs.get(k + 1).map(|r| r.start)
    } else if base > runs[k].start {
        Some(runs[k].start)
    } else {
        k.checked_sub(1).map(|p| runs[p].start)
    };
    if let Some(line) = target {
        view.land_on(line);
    }
}
