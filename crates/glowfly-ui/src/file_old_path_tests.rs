//! How a narrow row shares its room between a renamed file's path and its note: pure, no egui.

use super::*;

const PATH: f32 = 200.0;
const NOTE: f32 = 150.0;

#[test]
fn both_show_whole_when_they_fit() {
    assert_eq!(share(PATH + NOTE_GAP + NOTE, PATH, NOTE), (PATH, Some(NOTE)));
    assert_eq!(share(1000.0, PATH, NOTE), (PATH, Some(NOTE)));
}

#[test]
fn the_note_is_cut_first_and_the_path_keeps_its_width() {
    let room = PATH + NOTE_GAP + NOTE - 1.0;
    let (path, note) = share(room, PATH, NOTE);
    assert_eq!(path, PATH);
    assert_eq!(note, Some(NOTE - 1.0));
    let (path, note) = share(PATH + NOTE_GAP + MIN_NOTE, PATH, NOTE);
    assert_eq!((path, note), (PATH, Some(MIN_NOTE)), "a note of exactly the minimum stays");
}

#[test]
fn a_note_under_the_minimum_is_dropped_whole() {
    let (path, note) = share(PATH + NOTE_GAP + MIN_NOTE - 0.5, PATH, NOTE);
    assert_eq!((path, note), (PATH, None));
    assert_eq!(share(PATH + NOTE_GAP, PATH, NOTE), (PATH, None));
}

#[test]
fn the_path_is_cut_only_after_the_note_is_gone() {
    assert_eq!(share(PATH - 30.0, PATH, NOTE), (PATH - 30.0, None));
    assert_eq!(share(0.0, PATH, NOTE), (0.0, None));
    assert_eq!(share(-5.0, PATH, NOTE), (0.0, None), "no room at all");
    // While the note shows, the path is whole.
    let mut room = 1000.0;
    while room > 0.0 {
        let (path, note) = share(room, PATH, NOTE);
        assert!(note.is_none() || path == PATH, "room {room}: a cut path beside a note");
        room -= 1.0;
    }
}

#[test]
fn a_short_note_that_fits_is_never_cut() {
    let short = MIN_NOTE - 20.0;
    assert_eq!(share(PATH + NOTE_GAP + short, PATH, short), (PATH, Some(short)));
    assert_eq!(share(PATH + NOTE_GAP + short - 1.0, PATH, short), (PATH, None), "a short note doesn't get cut below itself");
}

#[test]
fn narrowing_the_room_never_brings_the_note_back() {
    for note_w in [20.0, MIN_NOTE, NOTE, 400.0] {
        let mut gone = false;
        let mut room = 1200.0;
        while room >= 0.0 {
            let (_, note) = share(room, PATH, note_w);
            assert!(!(gone && note.is_some()), "note {note_w}: back at room {room}");
            gone |= note.is_none();
            if let Some(n) = note {
                assert!(n >= note_w.min(MIN_NOTE) - 0.001, "note {note_w} at {room}: room {n} is under the minimum");
            }
            room -= 0.5;
        }
        assert!(gone);
    }
}
