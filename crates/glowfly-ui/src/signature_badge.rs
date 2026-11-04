//! The details panel's signature badge: Verified / Signed / Expired / Revoked / Bad signature,
//! a tooltip with signer, key and reason, and a menu (Check again, Copy key, settings) that a
//! click opens below it and a right-click at the pointer.

use super::common::menu_item;
use super::theme::color;
use crate::widgets::chip::{Chip, ChipSize, Lead, Look};
use crate::widgets::menu;
use eframe::egui::{self, Color32, Painter, Pos2, Stroke, vec2};
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::CommitDetail;
use glowfly_core::signature_check::Verdict;
use glowfly_model::msg::{Msg, PrefsMsg, RepoMsg};
use glowfly_model::state::signatures::{BadgeState, badge};
use glowfly_model::state::{AppState, PrefPage};

/// The glyph's size inside the chip's glyph slot.
const GLYPH: f32 = 8.0;

/// The mark drawn before the label.
#[derive(Clone, Copy)]
enum Glyph {
    Check,
    Ring,
    Cross,
}

/// Draws the badge for the commit `d` (in a right-to-left layout); nothing for an unsigned one.
pub(super) fn show(ui: &mut egui::Ui, st: &AppState, d: &CommitDetail, msgs: &mut Vec<Msg>) {
    let Some(format) = d.signature else { return };
    let held = !st.runs_git_unasked();
    let state = match st.session.reads.signatures.get(d.oid) {
        _ if held => BadgeState::Held,
        Some(r) => BadgeState::Done(r),
        None => {
            // Nothing known: ask, unless the check is already on its way.
            if !st.session.reads.signatures.pending(d.oid) {
                msgs.push(Msg::Repo(RepoMsg::WantSignature(d.oid)));
            }
            BadgeState::Checking
        }
    };
    let done = matches!(state, BadgeState::Done(_));
    let t = st.settings.lang();
    let b = badge(t, format, state);
    let (tint, glyph) = match b.verdict {
        _ if !done => (color::weak(), Glyph::Ring),
        Verdict::Verified => (color::added(), Glyph::Check),
        Verdict::Signed | Verdict::Expired => (color::warn(), Glyph::Ring),
        Verdict::Revoked | Verdict::Bad => (color::removed(), Glyph::Cross),
    };

    let mark = move |p: &Painter, c: Pos2, _slot: f32| paint_glyph(p, c, glyph, tint);
    let look = Look::new(color::wash(tint, 0.16), color::text()).edged(tint);
    // The chip's id, known before it is drawn, so it stays latched while its menu is open.
    let open = menu::below_open(ui.ctx(), ui.next_auto_id());
    let chip = Chip::new(ChipSize::Chip, b.label).look(look).lead(Lead::Paint(&mark)).latched(open);
    let resp = chip.button(ui).on_hover_text(b.tooltip);
    let key = b.checked.map(|c| c.key).unwrap_or_default();
    let items = |ui: &mut egui::Ui| menu_items(ui, t, (held, d.oid, &key), msgs);
    // A click opens the menu below the chip; a right-click opens it at the pointer.
    menu::below(ui, &resp, items);
    menu::context_menu(&resp, |ui| menu_items(ui, t, (held, d.oid, &key), msgs));
}

/// The badge's menu: Check again (unless git is held), Copy key (when one is known) and the
/// signing settings.
fn menu_items(ui: &mut egui::Ui, t: Lang, (held, oid, key): (bool, Oid, &str), msgs: &mut Vec<Msg>) {
    if !held {
        menu_item(msgs, ui, t.tr("signature.menu.check_again"), Msg::Repo(RepoMsg::CheckSignatureAgain(oid)));
    }
    if !key.is_empty() {
        menu_item(msgs, ui, t.tr("signature.menu.copy_key"), Msg::Copy(key.to_owned()));
    }
    menu_item(msgs, ui, t.tr("signature.menu.settings"), Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Signing)));
}

/// Paints `glyph` `GLYPH` px wide around `c`, as shapes (the bundled fonts lack check marks and
/// crosses).
fn paint_glyph(p: &Painter, c: Pos2, glyph: Glyph, tint: Color32) {
    let stroke = Stroke::new(1.6_f32, tint);
    // Drawn on a 7 px grid, scaled to the glyph's size.
    let k = GLYPH / 7.0;
    let at = |x: f32, y: f32| c + vec2(x * k, y * k);
    match glyph {
        Glyph::Check => {
            p.line_segment([at(-3.5, 0.0), at(-1.0, 2.5)], stroke);
            p.line_segment([at(-1.0, 2.5), at(3.5, -3.0)], stroke);
        }
        Glyph::Cross => {
            p.line_segment([at(-3.0, -3.0), at(3.0, 3.0)], stroke);
            p.line_segment([at(-3.0, 3.0), at(3.0, -3.0)], stroke);
        }
        Glyph::Ring => {
            p.circle_stroke(c, 3.5 * k, Stroke::new(1.4_f32, tint));
        }
    }
}
