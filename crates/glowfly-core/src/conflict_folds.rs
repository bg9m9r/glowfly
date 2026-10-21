//! Folds of the conflict view: the long runs of clean lines between the chunks, drawn as one
//! "N unchanged lines" row until opened. The rule is the whole-file view's: `FOLD_CONTEXT`
//! lines stay next to every chunk (none at the file's start or end), and a run folds only when
//! at least `MIN_FOLD` lines would be hidden.

use crate::repo::Segment;
use crate::whole_file::{FOLD_CONTEXT, MIN_FOLD};

/// Lines `start..end` of clean segment `seg`, folded into one row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CleanFold {
    pub seg: usize,
    pub start: usize,
    pub end: usize,
}

impl CleanFold {
    /// How many lines the fold hides.
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Never true: a fold hides at least `MIN_FOLD` lines.
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }
}

/// The fold of clean segment `seg`, if it is long enough to have one.
pub fn clean_fold(segs: &[Segment], seg: usize) -> Option<CleanFold> {
    let Some(Segment::Clean(lines)) = segs.get(seg) else { return None };
    let chunk_at = |i: Option<usize>| i.and_then(|i| segs.get(i)).is_some_and(|s| matches!(s, Segment::Conflict(_)));
    let lead = if chunk_at(seg.checked_sub(1)) { FOLD_CONTEXT } else { 0 };
    let trail = if chunk_at(Some(seg + 1)) { FOLD_CONTEXT } else { 0 };
    let end = lines.len().saturating_sub(trail);
    (end >= lead + MIN_FOLD).then_some(CleanFold { seg, start: lead, end })
}

/// Every fold of `segs`, in file order.
pub fn clean_folds(segs: &[Segment]) -> Vec<CleanFold> {
    (0..segs.len()).filter_map(|seg| clean_fold(segs, seg)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::Chunk;

    fn clean(n: usize) -> Segment {
        Segment::Clean((0..n).map(|i| format!("{i}\n")).collect())
    }

    fn chunk() -> Segment {
        Segment::Conflict(Chunk { base: Vec::new(), ours: vec!["o\n".into()], theirs: vec!["t\n".into()] })
    }

    #[test]
    fn folds_keep_context_next_to_chunks_only() {
        let segs = [clean(20), chunk(), clean(9), chunk(), clean(10), chunk(), clean(7), clean(3)];
        assert_eq!(clean_fold(&segs, 0), Some(CleanFold { seg: 0, start: 0, end: 17 }), "nothing kept at the file's start");
        assert_eq!(clean_fold(&segs, 2), None, "9 - 6 = 3 hidden lines are too few");
        assert_eq!(clean_fold(&segs, 4), Some(CleanFold { seg: 4, start: 3, end: 7 }));
        assert_eq!(clean_fold(&segs, 4).unwrap().len(), 4);
        assert_eq!(clean_fold(&segs, 6), Some(CleanFold { seg: 6, start: 3, end: 7 }), "nothing kept before a clean run");
        assert_eq!(clean_fold(&segs, 1), None, "a chunk never folds");
        assert_eq!(clean_fold(&segs, 99), None);
        let at_end = [chunk(), clean(7)];
        assert_eq!(clean_fold(&at_end, 1), Some(CleanFold { seg: 1, start: 3, end: 7 }), "nothing kept at the file's end");
        assert_eq!(clean_fold(&[chunk(), clean(6)], 1), None);
        assert_eq!(clean_folds(&segs).iter().map(|f| f.seg).collect::<Vec<_>>(), [0, 4, 6]);
    }
}
