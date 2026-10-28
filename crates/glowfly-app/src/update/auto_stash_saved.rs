//! Saved pending auto-stashes: an auto-stash that has to wait before it goes back (an operation
//! stopped on conflicts, a job runs, a Git Flow finish holds it) is written to the repository's
//! `auto-stashes.json` (`StashEffect::SaveAutoStashes`), and removed from it once its put-back
//! ends. Opening the repository again reads them (`StashEffect::ReadAutoStashes`); those whose
//! stash is still listed are offered one at a time (`Modal::ConfirmChoice`) once the repository
//! is the active tab, ready, and nothing else waits or runs: Put back makes the record a normal
//! pending auto-stash, Leave in stash list forgets it. Cancel asks again at the next open.

use crate::effect::{Effect, StashEffect};
use crate::msg::{AutoStashMsg, Msg};
use crate::state::auto_stash::{
    AutoStashJob, PendingStash, Phase, SavedOffer, blocks, ready, saved_body, saved_gone, saved_leave, saved_left, saved_put_back,
    saved_title,
};
use crate::state::{AppState, Modal};
use git2::Oid;
use glowfly_core::paths::same_spelling;
use glowfly_core::stash_ops::SavedAutoStash;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The active repository was opened: forgets what an earlier read of its records offered and
/// reads them again.
pub(super) fn opened(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    st.auto_stash.saved.retain(|o| !same_spelling(&o.workdir, &workdir));
    fx.push(Effect::Stash(StashEffect::ReadAutoStashes { cwd: workdir }));
}

/// What `workdir`'s file should hold: its pending entry once saved (and its stash found), and
/// the records still waiting for an answer, each stash once.
fn records(st: &AppState, workdir: &Path) -> Vec<SavedAutoStash> {
    let mut out: Vec<SavedAutoStash> = Vec::new();
    let pending = st.auto_stash.pending.iter().filter(|p| p.saved && same_spelling(&p.workdir, workdir));
    let kept = pending.filter_map(|p| Some(SavedAutoStash::new(p.job.op.clone(), p.job.message.clone(), p.oid?)));
    for s in kept.chain(st.auto_stash.saved_of(workdir).map(|o| o.stash.clone())) {
        if !out.iter().any(|o| o.oid == s.oid) {
            out.push(s);
        }
    }
    out
}

/// Rewrites `workdir`'s saved records from what is waiting now.
pub(super) fn save(st: &AppState, workdir: &Path, fx: &mut Vec<Effect>) {
    fx.push(Effect::Stash(StashEffect::SaveAutoStashes { cwd: workdir.to_path_buf(), saved: records(st, workdir) }));
}

/// The pending entry `i` keeps waiting with its stash found: writes it to the saved records the
/// first time.
pub(super) fn keep(st: &mut AppState, i: usize, fx: &mut Vec<Effect>) {
    let p = &mut st.auto_stash.pending[i];
    if p.saved || p.oid.is_none() {
        return;
    }
    p.saved = true;
    let workdir = p.workdir.clone();
    save(st, &workdir, fx);
}

/// `workdir`'s saved records were read (`SavedRead`): keeps the ones not already waiting in
/// memory, and offers one when that repository is the active tab.
pub(super) fn read(st: &mut AppState, workdir: PathBuf, list: Vec<SavedAutoStash>, fx: &mut Vec<Effect>) {
    for stash in list {
        let Some(oid) = stash.oid() else { continue };
        let waiting = st.auto_stash.pending.iter().any(|p| same_spelling(&p.workdir, &workdir) && p.oid == Some(oid));
        if waiting || st.auto_stash.saved_of(&workdir).any(|o| o.oid == oid) {
            continue;
        }
        st.auto_stash.saved.push(SavedOffer { workdir: workdir.clone(), stash, oid, offered: false });
    }
    if active(st, &workdir) {
        offer(st, fx);
    }
}

fn active(st: &AppState, workdir: &Path) -> bool {
    st.snap.as_ref().is_some_and(|s| same_spelling(&s.workdir, workdir))
}

/// Drops the active repository's records whose stash is no longer listed, saying so in the log
/// and rewriting the file.
fn validate(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let workdir = snap.workdir.clone();
    let listed = |oid: Oid| snap.stashes.iter().any(|s| s.oid == oid);
    let (gone, keep): (Vec<SavedOffer>, Vec<SavedOffer>) =
        std::mem::take(&mut st.auto_stash.saved).into_iter().partition(|o| same_spelling(&o.workdir, &workdir) && !listed(o.oid));
    st.auto_stash.saved = keep;
    if gone.is_empty() {
        return;
    }
    let t = st.settings.lang();
    for o in &gone {
        st.log(true, saved_gone(t, &o.stash.message));
    }
    save(st, &workdir, fx);
}

/// Offers the active repository's next saved record, when it is ready (nothing in progress or
/// conflicted), no job runs, no dialog is open and no auto-stash of its own waits.
pub(super) fn offer(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    if !st.auto_stash.saved_of(&workdir).any(|o| !o.offered) {
        return;
    }
    validate(st, fx);
    let Some(snap) = st.snap.as_ref() else { return };
    let busy = st.modal.is_some() || st.job.is_some() || st.trust.held.is_some() || st.auto_stash.index_of(&workdir).is_some();
    if busy || !ready(snap) {
        return;
    }
    let dirty = blocks(snap);
    let Some(o) = st.auto_stash.saved.iter_mut().find(|o| !o.offered && same_spelling(&o.workdir, &workdir)) else { return };
    o.offered = true;
    let (t, oid, name) = (st.settings.lang(), o.oid, o.stash.message.clone());
    st.modal = Some(Modal::ConfirmChoice {
        title: saved_title(t).to_owned(),
        body: saved_body(t, &name, dirty),
        button: saved_put_back(t).to_owned(),
        then: Box::new(Msg::AutoStash(AutoStashMsg::PutBackSaved { workdir: workdir.clone(), oid })),
        alt: saved_leave(t).to_owned(),
        alt_then: Box::new(Msg::AutoStash(AutoStashMsg::LeaveSaved { workdir, oid })),
    });
}

/// The offer for `workdir`'s stash `oid`, when it is still waiting and `workdir` is the active
/// tab. An answer for another tab is ignored, and the offer comes again when that tab is.
fn answered(st: &mut AppState, workdir: &Path, oid: Oid) -> Option<SavedOffer> {
    let i = st.auto_stash.saved.iter().position(|o| o.oid == oid && same_spelling(&o.workdir, workdir))?;
    if !active(st, workdir) {
        st.auto_stash.saved[i].offered = false;
        return None;
    }
    Some(st.auto_stash.saved.remove(i))
}

/// Put back: the record becomes a pending auto-stash and goes back as the quiet put-back job.
pub(super) fn put_back(st: &mut AppState, workdir: &Path, oid: Oid, now: Instant, fx: &mut Vec<Effect>) {
    if st.auto_stash.index_of(workdir).is_some() {
        // Another auto-stash of this tab waits: offer this one again after it.
        if let Some(o) = st.auto_stash.saved.iter_mut().find(|o| o.oid == oid && same_spelling(&o.workdir, workdir)) {
            o.offered = false;
        }
        return;
    }
    let Some(o) = answered(st, workdir, oid) else { return };
    if !st.snap.as_ref().is_some_and(|s| s.stashes.iter().any(|x| x.oid == oid)) {
        let t = st.settings.lang();
        st.log(true, saved_gone(t, &o.stash.message));
        save(st, workdir, fx);
        return;
    }
    let job = AutoStashJob { op: o.stash.op, message: o.stash.message, before: Vec::new() };
    let phase = Phase::Ended { from: st.load_gen };
    st.auto_stash.pending.push(PendingStash { workdir: o.workdir, job, oid: Some(oid), phase, told: true, saved: true });
    super::auto_stash::seen(st, false, now, fx);
}

/// Leave in stash list: forgets the record; the stash stays in STASHES.
pub(super) fn leave(st: &mut AppState, workdir: &Path, oid: Oid, fx: &mut Vec<Effect>) {
    let Some(o) = answered(st, workdir, oid) else { return };
    save(st, workdir, fx);
    let t = st.settings.lang();
    st.log(true, saved_left(t, &o.stash.message));
    offer(st, fx);
}
