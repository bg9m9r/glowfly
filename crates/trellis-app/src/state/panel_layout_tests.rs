use super::*;

fn sides(room: f32, sidebar: Option<f32>, details: Option<f32>) -> (Fit, Fit) {
    let f = fit(room, MAIN_MIN_WIDTH, &[(SIDEBAR, sidebar), (DETAILS, details)]);
    (f[0], f[1])
}

#[test]
fn defaults_read_well_and_grow_with_the_window() {
    let (s, d) = sides(1400.0, None, None);
    assert_eq!((s.size, d.size), (260.0, 400.0));
    let (s, d) = sides(2000.0, None, None);
    assert_eq!((s.size, d.size), (300.0, 500.0));
    let (s, d) = sides(3840.0, None, None);
    assert_eq!((s.size, d.size), (320.0, 520.0), "the defaults stop growing");
}

#[test]
fn a_too_small_saved_width_is_repaired_to_the_minimum() {
    // The reported drawer: dragged to ~120 px in a 2000 px window.
    let (s, d) = sides(2000.0, Some(40.0), Some(120.0));
    assert_eq!((s.size, d.size), (SIDEBAR.min, DETAILS.min));
    assert_eq!((s.lo, d.lo), (SIDEBAR.min, DETAILS.min), "a drag can't go below the minimum either");
}

#[test]
fn a_non_number_saved_width_falls_back_to_the_default() {
    let (s, d) = sides(1400.0, Some(f32::NAN), Some(f32::INFINITY));
    assert_eq!((s.size, d.size), (260.0, 400.0));
}

#[test]
fn a_huge_saved_width_is_held_to_its_share_of_the_window() {
    let (s, d) = sides(1400.0, Some(5000.0), Some(5000.0));
    assert!((s.size + d.size - (1400.0 - MAIN_MIN_WIDTH)).abs() < 0.01, "the main view keeps its minimum");
    assert!(d.size <= 700.0 && s.size <= 560.0);
    let (_, d) = sides(4000.0, None, Some(5000.0));
    assert_eq!(d.size, DETAILS.max_abs, "a huge window still caps the drawer");
}

#[test]
fn a_shrinking_window_squeezes_the_drawers_but_keeps_the_saved_size() {
    let saved = (Some(300.0), Some(500.0));
    let (s, d) = sides(1000.0, saved.0, saved.1);
    assert!((s.size + d.size - (1000.0 - MAIN_MIN_WIDTH)).abs() < 0.01);
    assert!(s.size >= SIDEBAR.min && d.size >= DETAILS.min);
    // The window grows again: the saved sizes come back (they were never overwritten).
    let (s, d) = sides(2000.0, saved.0, saved.1);
    assert_eq!((s.size, d.size), (300.0, 500.0));
}

#[test]
fn a_window_too_small_for_every_minimum_degrades_without_overlap() {
    for room in [0.0, 100.0, 360.0, 500.0, 799.0] {
        let (s, d) = sides(room, Some(300.0), Some(500.0));
        assert!(s.size >= 0.0 && d.size >= 0.0, "{room}");
        assert!(s.size + d.size <= (room - MAIN_MIN_WIDTH).max(0.0) + 0.01, "{room}: main keeps priority");
        assert!(s.lo <= s.size && s.size <= s.hi && d.lo <= d.size && d.size <= d.hi, "{room}");
    }
    let (s, d) = sides(f32::NAN, None, None);
    assert_eq!((s.size, d.size), (0.0, 0.0));
}

#[test]
fn a_drag_may_not_push_into_the_other_drawer_or_the_main_view() {
    let (s, d) = sides(1200.0, Some(260.0), Some(400.0));
    assert_eq!(s.hi, 1200.0 - MAIN_MIN_WIDTH - 400.0);
    assert_eq!(d.hi, 1200.0 - MAIN_MIN_WIDTH - 260.0);
    let (s, d) = sides(2400.0, Some(260.0), Some(400.0));
    assert_eq!((s.hi, d.hi), (SIDEBAR.max_abs, DETAILS.max_abs), "in a wide window their own caps are the tighter ones");
}

#[test]
fn one_drawer_alone_takes_up_to_its_cap() {
    let f = fit_one(800.0, MAIN_MIN_HEIGHT, TERMINAL, Some(10_000.0));
    assert_eq!(f.size, 800.0 * 0.7);
    let f = fit_one(700.0, MAIN_MIN_HEIGHT, TERMINAL, None);
    assert_eq!(f.size, 220.0);
    let f = fit_one(200.0, MAIN_MIN_HEIGHT, ACTIVITY, None);
    assert_eq!(f.size, 40.0, "a tiny window: the logs give way to the main view");
}

#[test]
fn sanitize_rounds_and_clamps_a_dragged_size() {
    assert_eq!(DETAILS.sanitize(123.4), Some(DETAILS.min));
    assert_eq!(DETAILS.sanitize(431.6), Some(432.0));
    assert_eq!(SIDEBAR.sanitize(9e9), Some(SIDEBAR.max_abs));
    assert_eq!(SIDEBAR.sanitize(f32::NAN), None);
}

#[test]
fn secondary_ranges_stay_ordered_in_a_small_room() {
    assert_eq!(PR_SIDE.range_for(2000.0), (280.0, 600.0));
    assert_eq!(PR_SIDE.range_for(700.0), (280.0, 350.0));
    let (lo, hi) = HISTORY_LIST.range_for(100.0);
    assert!(lo <= hi);
}
