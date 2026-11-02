//! The chip catalog: one row per chip and badge kind, each drawn by the painter the app uses
//! (`widgets::chip` underneath): quiet chips, the signature badge, the SSH agent chip and key
//! marks, issue labels and picker chips (one with its × hovered), status words, a journal row,
//! file-status badges, a tab group chip and the status bar's update chip. Its pixel golden is
//! the standing check that chips come in two heights only, 18 and 22 px.

use super::snapshot_tests::remove_fixture;
use super::snapshot_tests_signature::{checked, ssh, with_signed_head};
use super::snapshot_tests_updates::{status_bar, the_offer};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::ssh_marks::AgentPill;
use crate::state::updates::UpdateStatus;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::theme::{DARK, apply_style, color, egui_theme};
use crate::ui::widgets::chip::{Chip, ChipSize, Look};
use crate::ui::{file_status, issue, journal, prefs, pull_request, signature_badge, tab_paint, update_notice, widgets};
use eframe::egui::{self, Event, Rect, RichText, Ui, pos2, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::CommitDetail;
use glowfly_core::settings::ThemeChoice;
use glowfly_core::signature_check::SignatureCheck;
use glowfly_core::signing::SigningFormat;

const KEY: &str = "SHA256:nu3mXdiXjT/P69lIp2s2NqrOSBxSCmevSqfmgZp4JuA";

struct Catalog {
    signed: AppState,
    detail: CommitDetail,
    checks: Vec<Result<SignatureCheck, String>>,
    updates: AppState,
    msgs: Vec<Msg>,
    /// The second picker chip, for the pointer to hover its ×.
    picker: Option<Rect>,
}

/// A row: its name in weak text, then `f`'s chips.
fn row(ui: &mut Ui, name: &str, f: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(30.0);
        ui.add_sized(vec2(110.0, 22.0), egui::Label::new(RichText::new(name).color(color::weak())));
        ui.spacing_mut().item_spacing.x = 8.0;
        f(ui);
    });
}

fn draw(ui: &mut Ui, c: &mut Catalog) {
    let t = Lang::En;
    row(ui, "Quiet chips", |ui| {
        let _ = widgets::quiet_chip(ui, RichText::new("a1b2c3d").font(crate::ui::theme::type_scale::mono(12.0)), "SHA");
        let _ = widgets::quiet_chip(ui, "Grace Hopper", "Committer");
    });
    row(ui, "Signature", |ui| {
        let oid = c.detail.oid;
        for r in &c.checks {
            checked(&mut c.signed, oid, r.clone());
            signature_badge::show(ui, &c.signed, &c.detail, &mut c.msgs);
        }
    });
    row(ui, "SSH", |ui| {
        prefs::ssh_agent::pill_widget(ui, t, AgentPill::Running(2));
        prefs::ssh_agent::pill_widget(ui, t, AgentPill::Unreachable);
        prefs::ssh_card::chip(ui, "agent only", "Only in the agent", color::weak());
    });
    row(ui, "Status words", |ui| {
        pull_request::threads::chip(ui, "Outdated", color::warn());
        pull_request::threads::chip(ui, "Resolved", color::added());
        pull_request::threads::chip(ui, "Pending", color::weak());
        // As the pull request header, its checks, the release notes and Profiles draw them.
        Chip::new(ChipSize::Badge, "Draft").look(Look::new(color::raised(), color::weak())).show(ui);
        Chip::new(ChipSize::Badge, "Required").look(Look::new(color::raised(), color::weak())).show(ui);
        Chip::new(ChipSize::Badge, "New").look(Look::new(color::wash(color::added(), 0.25), color::added())).show(ui);
        Chip::new(ChipSize::Badge, "Current").look(Look::new(color::wash(color::primary(), 0.25), color::text())).show(ui);
    });
    row(ui, "Journal", |ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(320.0, 22.0), egui::Sense::hover());
        let p = ui.painter();
        let (mut x, y) = (rect.left(), rect.center().y);
        x = journal::chip(p, ChipSize::Chip, x, y, "main", 200.0, color::text()).right() + 6.0;
        x = journal::chip(p, ChipSize::Badge, x, y, "unreachable", 200.0, color::warn()).right() + 6.0;
        journal::chip(p, ChipSize::Badge, x, y, "now", 200.0, color::text());
    });
    row(ui, "File status", |ui| {
        for s in ['M', 'A', 'D', 'R', '!'] {
            file_status::status_chip(ui, s);
        }
    });
    row(ui, "Tab group", |ui| {
        let font = crate::ui::theme::type_scale::font(tab_paint::CHIP_TEXT);
        let w = ui.painter().layout_no_wrap("Work".into(), font, color::text()).size().x;
        let room = tab_paint::chip_room(w) - tab_paint::CHIP_GAP;
        let (slot, _) = ui.allocate_exact_size(vec2(room, 22.0), egui::Sense::hover());
        let r = widgets::chip::rect_at(ChipSize::Badge, slot.left(), slot.center().y, room);
        tab_paint::paint_group_chip(ui.painter(), r, "Work", color::accent());
    });
    row(ui, "Update", |ui| {
        ui.allocate_ui_with_layout(vec2(260.0, 26.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            update_notice::status_item(ui, &c.updates, &mut c.msgs);
        });
    });
    // Last, so the hovered ×'s tooltip covers no other row.
    row(ui, "Issue labels", |ui| {
        issue::info::chip(ui, ChipSize::Badge, "open", color::added());
        issue::info::chip(ui, ChipSize::Chip, "bug", color::text());
        issue::info::chip(ui, ChipSize::Chip, "good first issue", color::text());
        let _ = issue::picker::chip(ui, "enhancement");
        let at = ui.cursor().left();
        let _ = issue::picker::chip(ui, "ui");
        c.picker = Some(Rect::from_min_max(pos2(at, ui.min_rect().top()), ui.min_rect().max));
    });
}

#[test]
fn chips_catalog_dark() {
    let (signed, dir, oid) = with_signed_head(ThemeChoice::Dark, "chips-catalog", Some(SigningFormat::Ssh));
    // The signed head's detail, as the fixture marks it.
    let mut detail = glowfly_core::repo::commit_detail(&dir, oid).unwrap();
    detail.signature = Some(SigningFormat::Ssh);
    let verified = format!("G\0ada@example.com\0{KEY}\0{KEY}\0fully\0Good \"git\" signature for ada@example.com\n");
    let no_signers = "error: gpg.ssh.allowedSignersFile needs to be configured and exist for ssh signature verification\n";
    let checks =
        vec![ssh(&verified, ""), ssh("N\0\0\0\0undefined\0\n", no_signers), ssh("B\0\0\0\0never\0Could not verify signature.\n", "")];
    let updates = status_bar(UpdateStatus::Available(Box::new(the_offer())));
    let catalog = Catalog { signed, detail, checks, updates, msgs: Vec::new(), picker: None };
    let mut h = crate::ui::snapshot_check::builder().with_size(vec2(640.0, 380.0)).build_ui_state(
        |ui, c: &mut Catalog| {
            color::set_palette(&DARK);
            apply_style(ui.ctx(), &DARK);
            ui.ctx().set_theme(egui_theme(&DARK));
            ui.painter().rect_filled(ui.max_rect(), 0.0, DARK.panel);
            draw(ui, c);
        },
        catalog,
    );
    h.run();
    // The pointer on the second picker chip's ×: its hover square.
    let chip = h.state().picker.expect("the picker chip");
    h.input_mut().events.push(Event::PointerMoved(pos2(chip.right() - 11.0, chip.center().y)));
    h.run();
    h.run();
    h.check_snapshot("chips_catalog_dark");
    drop(h);
    remove_fixture(&dir);
}
