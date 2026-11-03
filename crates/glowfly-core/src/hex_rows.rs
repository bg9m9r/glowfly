//! The rows of a binary diff's hex view: both sides' bytes compared position by position (byte
//! `i` of the old side against byte `i` of the new), cut into rows of a fixed number of bytes.
//! A row is the same on both sides, changed, or on one side only (past the shorter side's end,
//! or every row of an added or deleted file). With context, runs of unchanged rows away from
//! every change fold into one "N unchanged rows" row.

/// What a row of bytes is on the two sides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HexKind {
    /// Every byte of the row is the same on both sides.
    Same,
    /// The row is on both sides and at least one byte differs (or is on one side only).
    Changed,
    /// The row is on the old side only.
    OldOnly,
    /// The row is on the new side only.
    NewOnly,
}

/// One row of the hex view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HexRow {
    /// The bytes from `offset`, `per_row` of them on each side that has them.
    Bytes { offset: usize, kind: HexKind },
    /// A run of `count` unchanged rows folded away.
    Fold { count: usize },
}

/// The rows of `old` against `new` (`None` for a side without the file), `per_row` bytes a row;
/// with `context`, unchanged rows further than that from a changed row fold away.
pub fn hex_rows(old: Option<&[u8]>, new: Option<&[u8]>, per_row: usize, context: Option<usize>) -> Vec<HexRow> {
    let per_row = per_row.max(1);
    let (a, b) = (old.unwrap_or_default(), new.unwrap_or_default());
    let n = a.len().max(b.len()).div_ceil(per_row);
    let kinds: Vec<HexKind> = (0..n).map(|r| kind(old, new, r * per_row, per_row)).collect();
    let Some(context) = context else {
        return kinds.iter().enumerate().map(|(r, &kind)| HexRow::Bytes { offset: r * per_row, kind }).collect();
    };
    let changed: Vec<usize> = (0..n).filter(|&r| kinds[r] != HexKind::Same).collect();
    let near = |r: usize| {
        let i = changed.partition_point(|&c| c + context < r);
        changed.get(i).is_some_and(|&c| c <= r + context)
    };
    let mut out = Vec::new();
    for (r, &kind) in kinds.iter().enumerate() {
        if near(r) {
            out.push(HexRow::Bytes { offset: r * per_row, kind });
        } else if let Some(HexRow::Fold { count }) = out.last_mut() {
            *count += 1;
        } else {
            out.push(HexRow::Fold { count: 1 });
        }
    }
    out
}

fn kind(old: Option<&[u8]>, new: Option<&[u8]>, start: usize, per_row: usize) -> HexKind {
    let has = |s: Option<&[u8]>| s.is_some_and(|s| s.len() > start);
    match (has(old), has(new)) {
        (true, true) => {
            let differs = (start..start + per_row).any(|i| byte(old, i) != byte(new, i));
            if differs { HexKind::Changed } else { HexKind::Same }
        }
        (true, false) => HexKind::OldOnly,
        // A row on neither side doesn't happen: rows stop at the longer side's end.
        (false, _) => HexKind::NewOnly,
    }
}

/// Byte `i` of a side, `None` past its end or without the file.
pub fn byte(side: Option<&[u8]>, i: usize) -> Option<u8> {
    side.and_then(|s| s.get(i).copied())
}

/// How many positions differ between the two sides, the longer side's tail included.
pub fn differing(old: Option<&[u8]>, new: Option<&[u8]>) -> usize {
    let len = old.map_or(0, <[u8]>::len).max(new.map_or(0, <[u8]>::len));
    (0..len).filter(|&i| byte(old, i) != byte(new, i)).count()
}

/// The character the ASCII column shows for `b`: itself when printable, `None` for a dot
/// placeholder.
pub fn ascii(b: u8) -> Option<char> {
    (0x20..=0x7e).contains(&b).then_some(b as char)
}

#[cfg(test)]
#[path = "hex_rows_tests.rs"]
mod tests;
