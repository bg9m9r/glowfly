//! The details panel's signature badge: Verified / Signed / Expired / Revoked / Bad signature,
//! a tooltip with signer, key and reason, and a menu (Check again, Copy key, settings).

use super::common::menu_item;
use super::theme::color;
use crate::msg::{Msg, PrefsMsg, RepoMsg};
use crate::state::signatures::{BadgeState, badge};
use crate::state::{AppState, PrefPage};
use crate::ui::theme::type_scale;
use eframe::egui::{self, Color32, CornerRadius, Painter, Pos2, Sense, Stroke, StrokeKind, pos2, vec2};
use glowfly_core::repo::CommitDetail;
use glowfly_core::signature_check::Verdict;

/// The pill's height.
const HEIGHT: f32 = 20.0;

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
    let state = match st.signatures.get(d.oid) {
        _ if held => BadgeState::Held,
        Some(r) => BadgeState::Done(r),
        None => {
            // Nothing known: ask, unless the check is already on its way.
            if !st.signatures.pending(d.oid) {
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

    let galley = ui.painter().layout_no_wrap(b.label.into(), type_scale::font(type_scale::SMALL), color::text());
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 30.0, HEIGHT), Sense::click());
    let p = ui.painter();
    let radius = CornerRadius::same((HEIGHT / 2.0) as u8);
    p.rect_filled(rect, radius, color::wash(tint, if resp.hovered() { 0.28 } else { 0.16 }));
    p.rect_stroke(rect, radius, Stroke::new(1.0_f32, tint), StrokeKind::Inside);
    paint_glyph(p, pos2(rect.left() + 12.0, rect.center().y), glyph, tint);
    p.galley(pos2(rect.left() + 21.0, rect.center().y - galley.size().y / 2.0), galley, color::text());

    let label = b.label;
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let resp = resp.on_hover_text(b.tooltip);
    let key = b.checked.map(|c| c.key).unwrap_or_default();
    let oid = d.oid;
    resp.context_menu(|ui| {
        if !held {
            menu_item(msgs, ui, t.tr("signature.menu.check_again"), Msg::Repo(RepoMsg::CheckSignatureAgain(oid)));
        }
        if !key.is_empty() {
            menu_item(msgs, ui, t.tr("signature.menu.copy_key"), Msg::Copy(key.clone()));
        }
        menu_item(msgs, ui, t.tr("signature.menu.settings"), Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Signing)));
    });
}

/// Paints `glyph` about 7px wide around `c`, as shapes (the bundled fonts lack check marks and
/// crosses).
fn paint_glyph(p: &Painter, c: Pos2, glyph: Glyph, tint: Color32) {
    let stroke = Stroke::new(1.6_f32, tint);
    let at = |x: f32, y: f32| c + vec2(x, y);
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
            p.circle_stroke(c, 3.5, Stroke::new(1.4_f32, tint));
        }
    }
}
