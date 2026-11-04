//! The hex view's rows: positional comparison, tails, added and deleted files, and folding.

use super::*;
use HexKind::*;

fn bytes(offset: usize, kind: HexKind) -> HexRow {
    HexRow::Bytes { offset, kind }
}

#[test]
fn rows_compare_byte_by_byte_and_mark_the_tail() {
    let old = [0u8; 20];
    let mut new = [0u8; 36];
    new[3] = 1;
    let rows = hex_rows(Some(&old), Some(&new), 16, None);
    assert_eq!(rows, [bytes(0, Changed), bytes(16, Changed), bytes(32, NewOnly)], "row 16 holds old's last 4 bytes and new's 16");
    assert_eq!(differing(Some(&old), Some(&new)), 1 + 16);
    let rows = hex_rows(Some(&new), Some(&old), 16, None);
    assert_eq!(rows[2], bytes(32, OldOnly));
}

#[test]
fn added_and_deleted_files_are_one_sided() {
    let b = [7u8; 20];
    assert_eq!(hex_rows(None, Some(&b), 8, None), [bytes(0, NewOnly), bytes(8, NewOnly), bytes(16, NewOnly)]);
    assert_eq!(hex_rows(Some(&b), None, 16, None), [bytes(0, OldOnly), bytes(16, OldOnly)]);
    assert_eq!(differing(None, Some(&b)), 20);
}

#[test]
fn unchanged_runs_away_from_changes_fold() {
    let old = [0u8; 16 * 10];
    let mut new = old;
    new[16 * 5] = 9;
    let rows = hex_rows(Some(&old), Some(&new), 16, Some(2));
    let want = [
        HexRow::Fold { count: 3 },
        bytes(48, Same),
        bytes(64, Same),
        bytes(80, Changed),
        bytes(96, Same),
        bytes(112, Same),
        HexRow::Fold { count: 2 },
    ];
    assert_eq!(rows, want);
    assert_eq!(hex_rows(Some(&old), Some(&old), 16, Some(2)), [HexRow::Fold { count: 10 }], "identical bytes fold whole");
    assert_eq!(hex_rows(Some(&old), Some(&new), 16, None).len(), 10, "without context every row shows");
}

#[test]
fn ascii_shows_printable_bytes_only() {
    assert_eq!((ascii(b'A'), ascii(b' '), ascii(b'~'), ascii(b'.')), (Some('A'), Some(' '), Some('~'), Some('.')));
    assert_eq!((ascii(0), ascii(0x7f), ascii(0xe9)), (None, None, None));
}
