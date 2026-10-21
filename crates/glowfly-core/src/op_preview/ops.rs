//! One preview per operation: merge, reset and cherry-pick here, rebase and drop in
//! `ops_rebase.rs`. Each builds an [`OpPreview`] from the graph, a replay or merge-tree.

use super::replay::{Applied, Replay};
use super::shape::{after, arriving, gone, line};
use super::{AfterRow, FF_ONLY, Limits, OpPreview, Outcome, PICK_MERGE, Stop, StopWhy, UNRELATED, ref_moved, too_many};
use crate::conflict_check::{Prediction, predict};
use crate::error::{Error, Result};
use git2::{Oid, Repository};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[path = "ops_rebase.rs"]
mod rebase_ops;
pub(super) use rebase_ops::{drop, rebase};

/// What every operation's preview reads.
pub(super) struct Cx<'a> {
    pub repo: &'a Repository,
    pub workdir: &'a Path,
    pub head: Oid,
    pub subject: Option<String>,
    pub lim: &'a Limits,
}

impl Cx<'_> {
    /// A preview with nothing worked out yet: `outcome`, no rows.
    fn base(&self, outcome: Outcome) -> OpPreview {
        OpPreview {
            op: super::PreviewOp::Reset { to: self.head },
            head: self.head,
            subject: self.subject.clone(),
            outcome,
            after: Vec::new(),
            after_more: false,
            moved: Vec::new(),
            gone: Vec::new(),
            gone_count: super::Count { n: 0, more: false },
            arriving: None,
            patch_check_skipped: false,
        }
    }

    /// Fills `p.after` from `tip` after the `first` rows.
    fn rows(
        &self,
        p: &mut OpPreview,
        first: Vec<AfterRow>,
        tip: Option<Oid>,
        made: &HashMap<Oid, Oid>,
        copy: bool,
        arrives: &HashSet<Oid>,
    ) -> Result<()> {
        let (rows, more) = after(self.repo, first, tip, made, copy, arrives, self.lim.rows)?;
        p.after = rows;
        p.after_more = more;
        Ok(())
    }

    /// The history as it is: nothing changes.
    fn nothing(&self) -> Result<OpPreview> {
        let mut p = self.base(Outcome::Nothing);
        self.rows(&mut p, Vec::new(), Some(self.head), &HashMap::new(), false, &HashSet::new())?;
        Ok(p)
    }

    /// The branch moves to `to`, which contains it (a fast-forward) or not (a reset): the
    /// commits `to` brings are marked as arriving.
    fn moves_to(&self, outcome: Outcome, to: Oid) -> Result<OpPreview> {
        let mut p = self.base(outcome);
        let (arrives, count) = arriving(self.repo, to, self.head, self.lim.count)?;
        p.arriving = Some(count);
        self.rows(&mut p, Vec::new(), Some(to), &HashMap::new(), false, &arrives)?;
        Ok(p)
    }

    /// Whether `a` contains `b` (or is it).
    fn contains(&self, a: Oid, b: Oid) -> Result<bool> {
        Ok(a == b || self.repo.graph_descendant_of(a, b)?)
    }

    /// Refuses when `name` no longer names `oid`.
    fn check_ref(&self, name: &str, oid: Oid) -> Result<()> {
        let now = self.repo.revparse_single(name).and_then(|o| o.peel_to_commit()).map(|c| c.id()).ok();
        if now != Some(oid) {
            return Err(Error::Unsupported(ref_moved(name)));
        }
        Ok(())
    }
}

/// How the branch's merges may fast-forward (`merge.ff`, then `branch.<b>.mergeOptions`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FfMode {
    Allow,
    Never,
    Only,
}

/// `merge.ff` (false: never, "only": only), then the last of `--ff`, `--no-ff` and `--ff-only`
/// in the branch's `mergeOptions`.
pub(super) fn ff_mode(repo: &Repository, branch: Option<&str>) -> FfMode {
    let Ok(cfg) = repo.config().and_then(|mut c| c.snapshot()) else { return FfMode::Allow };
    let mut mode = match cfg.get_string("merge.ff") {
        Ok(v) if v.trim().eq_ignore_ascii_case("only") => FfMode::Only,
        Ok(v) => match git2::Config::parse_bool(v.trim()) {
            Ok(false) => FfMode::Never,
            _ => FfMode::Allow,
        },
        Err(_) => FfMode::Allow,
    };
    if let Some(b) = branch
        && let Ok(opts) = cfg.get_string(&format!("branch.{b}.mergeoptions"))
    {
        for token in opts.split_whitespace() {
            match token {
                "--ff" => mode = FfMode::Allow,
                "--no-ff" => mode = FfMode::Never,
                "--ff-only" => mode = FfMode::Only,
                _ => {}
            }
        }
    }
    mode
}

/// `git merge <theirs>` (at `oid`): up to date and fast-forwards from the graph, a real merge
/// from merge-tree. The branch's history afterwards starts with the merge it makes.
pub(super) fn merge(cx: &Cx, theirs: &str, oid: Oid) -> Result<OpPreview> {
    cx.check_ref(theirs, oid)?;
    let head = cx.head;
    if cx.contains(head, oid)? {
        return cx.nothing();
    }
    let mode = ff_mode(cx.repo, cx.subject.as_deref());
    let can_ff = cx.repo.graph_descendant_of(oid, head)?;
    if can_ff && mode != FfMode::Never {
        return cx.moves_to(Outcome::FastForward, oid);
    }
    if !can_ff && mode == FfMode::Only {
        return Err(Error::Unsupported(FF_ONLY.into()));
    }
    let merge_row = AfterRow::NewMerge { theirs: theirs.to_owned() };
    let (outcome, first) = if can_ff {
        (Outcome::Done, vec![merge_row])
    } else {
        if let Err(e) = cx.repo.merge_base(head, oid) {
            if e.code() == git2::ErrorCode::NotFound {
                return Err(Error::Unsupported(UNRELATED.into()));
            }
            return Err(e.into());
        }
        match predict(cx.workdir, head, oid)? {
            Prediction::Conflicts(paths) => {
                let stop = Stop { why: StopWhy::Conflicts(paths), applied: 0, waiting: Vec::new() };
                (Outcome::Stopped(stop), vec![AfterRow::Stopped { at: None }])
            }
            Prediction::UpToDate => return cx.nothing(),
            Prediction::FastForward | Prediction::Clean => (Outcome::Done, vec![merge_row]),
        }
    };
    let mut p = cx.base(outcome);
    p.arriving = Some(arriving(cx.repo, oid, head, cx.lim.count)?.1);
    cx.rows(&mut p, first, Some(head), &HashMap::new(), false, &HashSet::new())?;
    Ok(p)
}

/// `git reset <to>`: the branch moves to `to`; the commits only the old tip reached are gone.
pub(super) fn reset(cx: &Cx, to: Oid) -> Result<OpPreview> {
    if cx.repo.find_commit(to).is_err() {
        return Err(Error::NotFound(format!("commit {to}")));
    }
    if to == cx.head {
        return cx.nothing();
    }
    let mut p = cx.moves_to(Outcome::Done, to)?;
    let (g, count) = gone(cx.repo, cx.head, Some(to), cx.subject.as_deref(), &[], &HashMap::new(), cx.lim)?;
    p.gone = g;
    p.gone_count = count;
    Ok(p)
}

/// `git cherry-pick <oids>` (oldest first) on HEAD: each pick makes a copy; an emptied commit
/// or conflicts stop it, with the picks before already on the branch.
pub(super) fn cherry_pick(cx: &Cx, oids: &[Oid]) -> Result<OpPreview> {
    let mut commits = Vec::with_capacity(oids.len());
    for o in oids {
        let c = cx.repo.find_commit(*o)?;
        if c.parent_count() > 1 {
            return Err(Error::Unsupported(PICK_MERGE.into()));
        }
        commits.push(c);
    }
    if commits.len() > cx.lim.replay {
        return Err(Error::Unsupported(too_many(commits.len())));
    }
    if commits.is_empty() {
        return cx.nothing();
    }
    let mut r = Replay::new(cx.repo, Some(cx.head))?;
    for (i, c) in commits.iter().enumerate() {
        let why = match r.pick(c, false, false)? {
            Applied::New(_) | Applied::Kept => continue,
            Applied::Emptied => StopWhy::Empty,
            Applied::Conflicts(paths) => StopWhy::Conflicts(paths),
        };
        let waiting = commits[i + 1..].iter().map(line).collect();
        let mut p = cx.base(Outcome::Stopped(Stop { why, applied: i, waiting }));
        let first = vec![AfterRow::Stopped { at: Some(line(c)) }];
        cx.rows(&mut p, first, r.tip, &r.made, true, &HashSet::new())?;
        return Ok(p);
    }
    let mut p = cx.base(Outcome::Done);
    cx.rows(&mut p, Vec::new(), r.tip, &r.made, true, &HashSet::new())?;
    Ok(p)
}
