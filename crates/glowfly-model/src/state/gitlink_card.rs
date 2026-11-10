//! The card a diff shows for a submodule's gitlink instead of its two "Subproject commit" rows
//! (ui/diff/gitlink_card.rs): its title, the old and new ids, the commits the move passes over
//! (oldest first, the ones nearest the end of the move when there are many), why they can't be
//! listed, what a submodule is, and whether to offer Open submodule and Record this checkout.
//! The move is `session::DiffSlot::gitlink`. Pure.

use super::AppState;
use super::submodules::short;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{CommitRun, DiffTarget, GITLINK_COUNT_MAX, GitlinkMove, GitlinkUnknown, MoveKind};

/// The open diff's gitlink move, when it is for the diff on screen.
pub fn shown(st: &AppState) -> Option<&GitlinkMove> {
    let (target, _) = st.session.diff.view.as_ref()?;
    st.session.diff.gitlink.as_ref().filter(|(t, _)| t == target).map(|(_, m)| m.as_ref())
}

/// The direction mark painted beside the title.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Forward,
    Backward,
    /// Another line of history.
    Fork,
}

/// One list of commits: a heading, the line for the earlier ones not listed, and the rows as
/// (short id, subject, full id), oldest first.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub heading: String,
    pub more: Option<String>,
    pub rows: Vec<(String, String, String)>,
}

/// What the card says.
#[derive(Clone, Debug, PartialEq)]
pub struct CardText {
    pub mark: Option<Mark>,
    pub title: String,
    /// Short old id, short new id, full old id, full new id: when the move has two commits.
    pub ids: Option<(String, String, String, String)>,
    pub groups: Vec<Group>,
    /// Where the two sides of a sideways move part.
    pub base: Option<String>,
    pub explain: String,
    /// The working tree's side has changes inside the submodule.
    pub warning: Option<String>,
    /// Why the commits can't be listed.
    pub reason: Option<String>,
}

pub fn text(t: Lang, m: &GitlinkMove) -> CardText {
    let id = |o: Option<git2::Oid>| o.map(short).unwrap_or_default();
    let (old, new) = (id(m.old), id(m.new));
    let ids = match (m.old, m.new) {
        (Some(o), Some(n)) if o != n => Some((old.clone(), new.clone(), o.to_string(), n.to_string())),
        _ => None,
    };
    let mut c = CardText {
        mark: None,
        title: String::new(),
        ids,
        groups: Vec::new(),
        base: None,
        explain: t.tr("diff.gitlink.explain").to_owned(),
        warning: m.dirty.then(|| t.tr("diff.gitlink.dirty").to_owned()),
        reason: None,
    };
    match &m.kind {
        MoveKind::Added => c.title = t.trf("diff.gitlink.title.added", &[("commit", &new)]),
        MoveKind::Removed => c.title = t.trf("diff.gitlink.title.removed", &[("commit", &old)]),
        MoveKind::Same => c.title = t.trf("diff.gitlink.title.same", &[("commit", &new)]),
        MoveKind::Forward => {
            c.mark = Some(Mark::Forward);
            c.title = if m.ahead.capped {
                t.trf("diff.gitlink.title.forward_many", &[("count", &GITLINK_COUNT_MAX)])
            } else {
                t.trn("diff.gitlink.title.forward", m.ahead.total, &[])
            };
            c.groups.push(group(t, t.tr("diff.gitlink.group.forward"), &m.ahead));
        }
        MoveKind::Backward => {
            c.mark = Some(Mark::Backward);
            c.title = if m.behind.capped {
                t.trf("diff.gitlink.title.backward_many", &[("count", &GITLINK_COUNT_MAX)])
            } else {
                t.trn("diff.gitlink.title.backward", m.behind.total, &[])
            };
            c.groups.push(group(t, t.tr("diff.gitlink.group.backward"), &m.behind));
        }
        MoveKind::Sideways { base } => {
            c.mark = Some(Mark::Fork);
            c.title = match base {
                Some(_) => {
                    let (ahead, behind) = (&m.ahead, &m.behind);
                    let many = [("count", &GITLINK_COUNT_MAX as &dyn std::fmt::Display)];
                    let added =
                        if ahead.capped { t.trf("diff.gitlink.added_many", &many) } else { t.trn("diff.gitlink.added", ahead.total, &[]) };
                    let left = if behind.capped {
                        t.trf("diff.gitlink.left_out_many", &many)
                    } else {
                        t.trn("diff.gitlink.left_out", behind.total, &[])
                    };
                    t.trf("diff.gitlink.title.sideways", &[("added", &added), ("left", &left)])
                }
                None => t.tr("diff.gitlink.title.unrelated").to_owned(),
            };
            c.groups.push(group(t, t.tr("diff.gitlink.group.new_side"), &m.ahead));
            c.groups.push(group(t, t.tr("diff.gitlink.group.old_side"), &m.behind));
            c.base = base.as_ref().map(|(id, subject)| t.trf("diff.gitlink.base", &[("commit", &short(*id)), ("subject", subject)]));
        }
        MoveKind::Unknown(why) => {
            c.title = t.trf("diff.gitlink.title.unknown", &[("old", &old), ("new", &new)]);
            c.reason = Some(reason(t, *why, &old, &new));
        }
    }
    c
}

fn group(t: Lang, heading: &str, run: &CommitRun) -> Group {
    let hidden = run.total.saturating_sub(run.commits.len());
    let more = if run.capped {
        Some(t.trf("diff.gitlink.earlier_many", &[("count", &hidden)]))
    } else {
        (hidden > 0).then(|| t.trn("diff.gitlink.earlier", hidden, &[]))
    };
    let rows = run.commits.iter().map(|(id, subject)| (short(*id), subject.clone(), id.to_string())).collect();
    Group { heading: heading.to_owned(), more, rows }
}

fn reason(t: Lang, why: GitlinkUnknown, old: &str, new: &str) -> String {
    match why {
        GitlinkUnknown::NotCheckedOut => t.tr("diff.gitlink.reason.not_checked_out").to_owned(),
        GitlinkUnknown::Refused => t.tr("diff.gitlink.reason.refused").to_owned(),
        GitlinkUnknown::Missing { old: true, new: true } => t.trf("diff.gitlink.reason.missing_both", &[("old", &old), ("new", &new)]),
        GitlinkUnknown::Missing { old: true, .. } => t.trf("diff.gitlink.reason.missing", &[("commit", &old)]),
        GitlinkUnknown::Missing { .. } => t.trf("diff.gitlink.reason.missing", &[("commit", &new)]),
        GitlinkUnknown::Shallow => t.tr("diff.gitlink.reason.shallow").to_owned(),
    }
}

/// The submodule's path when the card offers Record this checkout: an unstaged diff of a
/// submodule whose checkout can be recorded (`inside::record_blocked`).
pub fn record_offered(st: &AppState) -> Option<String> {
    let m = shown(st)?;
    let (DiffTarget::Unstaged(p), _) = st.session.diff.view.as_ref()? else { return None };
    let snap = st.snap.as_ref()?;
    let sm = snap.submodules.iter().find(|s| s.path == *p && s.path == m.path)?;
    super::submodules::inside::record_blocked(st.settings.lang(), sm, snap).is_none().then(|| sm.path.clone())
}

/// The submodule's path when the card offers Open submodule: one checked out in the active
/// repository.
pub fn open_offered(st: &AppState) -> Option<String> {
    let m = shown(st)?;
    let sm = st.snap.as_ref()?.submodules.iter().find(|s| s.path == m.path)?;
    super::submodules::open_blocked(sm).is_none().then(|| sm.path.clone())
}

#[cfg(test)]
#[path = "gitlink_card_tests.rs"]
mod tests;
