//! The commit details' row of quiet chips, under the author: the short SHA (click copies the
//! full SHA), the parents in git's order (click selects one, right-click copies its SHA) and,
//! when someone else recorded the commit, its committer (click copies "Name <email>").

use super::common::{menu_item, short};
use super::theme::color;
use super::widgets::quiet_chip;
use crate::msg::{Msg, RepoMsg};
use crate::state::AppState;
use crate::ui::theme::type_scale;
use eframe::egui::{Response, RichText, Ui, vec2};
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::CommitDetail;

/// Between two chips, across and down when they wrap.
const GAP: f32 = 6.0;

/// Whether the committer is someone other than the author: a rebase, cherry-pick or `git am`,
/// or a hosting service's web merge. Emails compare without case, as addresses do in practice.
pub(super) fn committer_differs(d: &CommitDetail) -> bool {
    d.committer != d.author || !d.committer_email.eq_ignore_ascii_case(&d.email)
}

/// "Name <email>", or the name alone without an email.
pub(super) fn identity(name: &str, email: &str) -> String {
    if email.is_empty() { name.to_owned() } else { format!("{name} <{email}>") }
}

fn mono(text: impl Into<String>) -> RichText {
    RichText::new(text).font(type_scale::mono(type_scale::SMALL)).color(color::text())
}

/// A tooltip: `first` in monospace, then each of `lines` that is given.
fn tip(r: Response, first: RichText, lines: &[Option<&str>]) -> Response {
    r.on_hover_ui(|ui| {
        ui.label(first);
        for line in lines.iter().flatten() {
            ui.label(*line);
        }
    })
}

/// Draws the chips for commit `d`, wrapping onto more lines in a narrow panel.
pub(super) fn row(ui: &mut Ui, st: &AppState, d: &CommitDetail, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(GAP, GAP);
        let sha = d.oid.to_string();
        let name = t.trf("details.commit.sha", &[("sha", &short(d.oid))]);
        let r = tip(quiet_chip(ui, mono(short(d.oid)), &name), mono(sha.as_str()), &[Some(t.tr("details.commit.sha.tip"))]);
        if r.clicked() {
            msgs.push(Msg::Copy(sha));
        }
        parents(ui, st, t, &d.parents, msgs);
        if committer_differs(d) {
            let who = identity(&d.committer, &d.committer_email);
            let label = t.trf("details.commit.committer.label", &[("committer", &d.committer)]);
            let when = st.settings.format_verbose(d.time, st.view.utc_offset);
            let text = RichText::new(label.as_str()).color(color::text());
            let r =
                tip(quiet_chip(ui, text, &label), RichText::new(who.as_str()), &[Some(&when), Some(t.tr("details.commit.committer.tip"))]);
            if r.clicked() {
                msgs.push(Msg::Copy(who));
            }
        }
    });
}

/// "Parent" or "Parents" and one chip per parent; nothing for a root commit.
fn parents(ui: &mut Ui, st: &AppState, t: Lang, parents: &[Oid], msgs: &mut Vec<Msg>) {
    let label = match parents.len() {
        0 => return,
        1 => t.tr("details.commit.parent.label"),
        _ => t.tr("details.commit.parents"),
    };
    ui.label(RichText::new(label).size(type_scale::SMALL).color(color::weak()));
    for &p in parents {
        let summary = st.snap.as_ref().and_then(|s| s.row_of.get(&p).and_then(|&i| s.commits.get(i))).map(|c| c.summary.as_str());
        let name = t.trf("details.commit.parent.name", &[("sha", &short(p))]);
        let r = tip(quiet_chip(ui, mono(short(p)), &name), mono(p.to_string()), &[summary, Some(t.tr("details.commit.parent.tip"))]);
        if r.clicked() {
            msgs.push(Msg::Repo(RepoMsg::JumpTo(p)));
        }
        r.context_menu(|ui| menu_item(msgs, ui, t.tr("common.copy_sha.button"), Msg::Copy(p.to_string())));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail(committer: &str, committer_email: &str) -> CommitDetail {
        CommitDetail {
            oid: Oid::zero(),
            message: String::new(),
            author: "Ada Lovelace".into(),
            email: "ada@example.com".into(),
            committer: committer.into(),
            committer_email: committer_email.into(),
            time: 0,
            author_time: 0,
            parents: Vec::new(),
            files: Vec::new(),
            absent: Default::default(),
            signature: None,
        }
    }

    #[test]
    fn the_committer_differs_by_name_or_email() {
        assert!(!committer_differs(&detail("Ada Lovelace", "ada@example.com")), "the author committed it");
        assert!(committer_differs(&detail("Grace Hopper", "grace@example.com")));
        assert!(committer_differs(&detail("Ada Lovelace", "ada@work.example.com")), "the same name from another address");
        assert!(!committer_differs(&detail("Ada Lovelace", "Ada@Example.com")), "only the case of the email differs");
    }

    #[test]
    fn identity_leaves_out_an_empty_email() {
        assert_eq!(identity("Ada", "ada@example.com"), "Ada <ada@example.com>");
        assert_eq!(identity("Ada", ""), "Ada");
    }
}
