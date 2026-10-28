//! The WIP panel while a merge, rebase, cherry-pick, revert or patch series is in progress (or
//! files are conflicted): a heading with the operation and its quiet Abort at the right, then
//! three numbered steps (`state::op_steps`). 1 Resolve conflicts holds the Conflicted and
//! Resolved Files lists with View conflicts; 2 Check what goes in holds Staged, then Unstaged
//! Files under a note that other changes stay out; 3, pinned at the bottom (`bottom`), commits
//! the merge or continues the operation (Continue as a split button with Skip and Edit remaining
//! steps in its menu), with the commit form a click away. The step now is marked and open; done steps fold to their one
//! line and reopen with Show (`LayoutMsg::ToggleOpStep`).

use super::UiCache;
use super::commit_box::{self, BoxStyle};
use super::common::{menu_item, menu_item_state, state_actions};
use super::conflict_files::{self, Lists};
use super::details_wip::{OpenFile, Reveal, RowContext, file_list};
use super::theme::{color, type_scale};
use super::widgets::reason::ItemState;
use super::widgets::{Icon, buttons, paint_icon};
use crate::msg::{ConflictMsg, LayoutMsg, Msg, RebaseMsg};
use crate::state::AppState;
use crate::state::op_actions::action_text;
use crate::state::op_confirm::{Risky, banner_msg};
use crate::state::op_steps::{self, OpKind, OpStep, StepState, step_state};
use crate::state::rebase_banner::action_msg;
use crate::ui::widgets::menu::{popup_above_or_below, separator};
use eframe::egui::{self, Align, Layout, Rect, RichText, Sense, Stroke, Ui, vec2};
use git2::RepositoryState;
use glowfly_core::git_version::Feature;
use glowfly_core::i18n::Lang;
use glowfly_core::pick_ops::{CHERRY_PICK_SKIP, REVERT_SKIP};
use glowfly_core::repo::{FileEntry, ResolvedFile, Snapshot};

/// The step marker's circle.
const MARKER: f32 = 28.0;
/// Where a step's title and contents start: the marker and a 12 px gap.
const INDENT: f32 = 40.0;
/// Between steps.
const STEP_GAP: f32 = 12.0;
/// The gap between step 3's buttons.
const BUTTON_GAP: egui::Vec2 = egui::vec2(8.0, 8.0);
/// The least width of Continue's menu.
const MORE_MENU_W: f32 = 200.0;

/// What steps 1 and 2 show: the conflicted and resolved files, and the lists' shared parts.
pub(super) struct TopArgs<'a> {
    pub kind: OpKind,
    pub conflicted: &'a [&'a FileEntry],
    pub resolved: &'a [&'a ResolvedFile],
    pub open: &'a OpenFile<'a>,
    pub reveal: Reveal<'a>,
    pub ctx: RowContext<'a>,
    /// The height the scrolling part of the panel has, for the lists to share.
    pub height: f32,
}

/// The heading and steps 1 and 2.
pub(super) fn top(ui: &mut Ui, st: &AppState, snap: &Snapshot, a: TopArgs, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    heading(ui, st, snap, !a.conflicted.is_empty(), msgs);
    super::path_warnings::banner(ui, t, snap);
    let n = a.conflicted.len();
    let folds = &st.view.op_steps;
    let (resolve_open, check_open) = (folds.is_open(snap, OpStep::Resolve), folds.is_open(snap, OpStep::Check));
    // The lists open share the height left by the heading and the step rows.
    let lists = usize::from(resolve_open) * (usize::from(n > 0) + usize::from(!a.resolved.is_empty())) + usize::from(check_open) * 2;
    let list_h = ((a.height - 260.0) / lists.max(1) as f32).max(96.0);

    let line = op_steps::resolve_line(t, n, a.resolved.len());
    let head = StepHead { n: 1, state: step_state(OpStep::Resolve, n), title: t.tr("details.steps.resolve.title"), line: &line };
    step(ui, head, resolve_open, OpStep::Resolve, t, msgs, |ui, msgs| {
        let lists = Lists { conflicted: a.conflicted, resolved: a.resolved, height: list_h };
        conflict_files::show(ui, st, lists, a.open, a.ctx, msgs);
        if let Some(first) = a.conflicted.first() {
            if ui.add(buttons::primary(t.tr("details.banner.view_conflicts.button"))).clicked() {
                msgs.push(Msg::Conflict(ConflictMsg::OpenConflict(first.path.clone())));
            }
        } else if a.resolved.is_empty() {
            ui.label(RichText::new(t.tr("details.steps.resolve.none")).color(color::faint()));
        }
    });

    let other = snap.unstaged.len() - n;
    let line = op_steps::check_line(t, snap.staged.len(), other);
    let head = StepHead { n: 2, state: step_state(OpStep::Check, n), title: t.tr("details.steps.check.title"), line: &line };
    step(ui, head, check_open, OpStep::Check, t, msgs, |ui, msgs| {
        file_list(ui, snap, true, (a.open, a.reveal), list_h, a.ctx, msgs);
        ui.add_space(8.0);
        let note = op_steps::check_note(t, a.kind);
        ui.add(egui::Label::new(RichText::new(note).size(type_scale::SMALL).color(color::weak())).wrap());
        file_list(ui, snap, false, (a.open, a.reveal), list_h, a.ctx, msgs);
    });
}

/// Step 3, pinned at the bottom of the panel: the commit form for a merge (or conflicts alone);
/// for the others Continue, Skip and Edit remaining steps, and the commit form on request.
pub(super) fn bottom(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, kind: OpKind, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    let t = st.settings.lang();
    let n = op_steps::conflicted(snap);
    let state = step_state(OpStep::Finish, n);
    let (open, form_open) = (st.view.op_steps.is_open(snap, OpStep::Finish), st.view.op_steps.is_open(snap, OpStep::CommitForm));
    let title = op_steps::finish_title(t, snap, kind);
    let line = match (state, kind) {
        (StepState::Later, OpKind::Merge | OpKind::Conflicts) => t.tr("details.steps.finish.waiting"),
        (StepState::Later, _) => t.tr("details.steps.finish.waiting_continue"),
        _ => "",
    };
    let repo_state = snap.state;
    let skip_blocked = st.git.blocked(Feature::SequencerSkip);
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);
    let head = StepHead { n: 3, state, title: &title, line };
    step_head(ui, &head, open, OpStep::Finish, t, msgs);
    if !open {
        return;
    }
    indented(ui, |ui| match kind {
        OpKind::Merge | OpKind::Conflicts => commit_box::show(ui, st, cache, BoxStyle { titled: false, primary: true }, msgs),
        _ => {
            let toggle = if form_open { t.tr("details.steps.commit_form.hide") } else { t.tr("details.steps.commit_form.show") };
            if ui.add(buttons::quiet(toggle).small()).clicked() {
                msgs.push(Msg::Layout(LayoutMsg::ToggleOpStep(OpStep::CommitForm)));
            }
            if form_open {
                commit_box::show(ui, st, cache, BoxStyle { titled: false, primary: false }, msgs);
                ui.add_space(4.0);
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = BUTTON_GAP;
                finish_buttons(ui, st, repo_state, n, skip_blocked.as_deref(), msgs);
            });
        }
    });
}

/// Continue as one primary split button (disabled while files are conflicted: git refuses it
/// before), whose caret opens a menu above it with Skip and, for an interactive rebase, Edit
/// remaining steps…. Without a Continue the other actions are plain buttons.
fn finish_buttons(ui: &mut Ui, st: &AppState, state: RepositoryState, conflicted: usize, skip_blocked: Option<&str>, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let Some(steps) = state_actions(state) else { return };
    let mut cont = None;
    let mut others = Vec::new();
    for &(label, a) in steps.iter().filter(|(_, a)| !destructive(a)) {
        if label == "Continue" {
            cont = Some((label, a));
        } else {
            let old_git = (a == CHERRY_PICK_SKIP || a == REVERT_SKIP).then_some(skip_blocked).flatten();
            others.push((label, a, old_git));
        }
    }
    let edit_steps = state == RepositoryState::RebaseInteractive;
    let Some((label, a)) = cont else {
        for (label, a, old_git) in others {
            if ui.add(buttons::secondary(action_text(t, label)).enabled(old_git.is_none()).why(old_git)).clicked() {
                msgs.push(banner_msg(st, label, a));
            }
        }
        return;
    };
    let waiting = (conflicted > 0).then(|| t.tr("details.steps.finish.waiting_continue"));
    let button = buttons::primary(action_text(t, label)).enabled(waiting.is_none()).why(waiting);
    if others.is_empty() && !edit_steps {
        if ui.add(button).clicked() {
            msgs.push(action_msg(state, label, a));
        }
        return;
    }
    let popup = ui.make_persistent_id("op-finish-more");
    let open = ui.memory(|m| m.is_popup_open(popup));
    let more = t.tr("details.steps.finish.more.button");
    let (main, caret) = buttons::split(button, more).open(open).show(ui);
    if main.clicked() {
        msgs.push(action_msg(state, label, a));
    }
    let caret = if open { caret } else { caret.on_hover_text(more) };
    if caret.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    let close = egui::PopupCloseBehavior::CloseOnClickOutside;
    popup_above_or_below(ui, popup, &caret, egui::AboveOrBelow::Above, close, |ui| {
        ui.set_min_width(MORE_MENU_W);
        let before = msgs.len();
        // A skip asks first and drops the commit's changes: after Edit remaining steps…, set
        // apart, in the danger style and with an ellipsis.
        let (risky, plain): (Vec<_>, Vec<_>) = others.into_iter().partition(|(_, a, _)| Risky::of(a).is_some());
        for (label, a, old_git) in plain {
            menu_item_state(msgs, ui, action_text(t, label), ItemState::blocked_by(old_git), banner_msg(st, label, a));
        }
        if edit_steps {
            menu_item(msgs, ui, t.tr("details.banner.edit_steps.button"), Msg::Rebase(RebaseMsg::OpenRemaining));
        }
        if !risky.is_empty() {
            separator(ui);
        }
        for (label, a, old_git) in risky {
            let text = format!("{}…", action_text(t, label));
            menu_item_state(msgs, ui, text, ItemState::danger_unless(old_git), banner_msg(st, label, a));
        }
        if msgs.len() > before {
            ui.memory_mut(|m| m.close_popup());
        }
    });
}

/// Abort and Reset bisect, which the heading carries.
fn destructive(args: &[&str]) -> bool {
    matches!(Risky::of(args), Some(Risky::Abort | Risky::ResetBisect))
}

/// The heading: the operation (with its step), Abort at the right, then what the operation
/// says of where it stopped (a rebase's stopped commit, a patch's subject) and its notes.
fn heading(ui: &mut Ui, st: &AppState, snap: &Snapshot, conflicts_left: bool, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let title = op_steps::heading(t, snap);
    title_row(ui, st, snap, &title, msgs);
    ui.add_space(2.0);
    if let Some(p) = &snap.rebase {
        super::rebase::banner::details(ui, t, p, conflicts_left, msgs);
    }
    super::am_banner::lines(ui, t, snap, conflicts_left);
    if let Some(note) = crate::state::auto_stash::banner_note(st) {
        ui.add(egui::Label::new(RichText::new(note).size(type_scale::SMALL).color(color::weak())).wrap());
    }
    if let Some(r) = st.session.gitflow_resume.as_ref().filter(|r| r.resume.merging(snap)) {
        let text = crate::state::gitflow_resume::waiting_text(t, &r.resume);
        ui.add(egui::Label::new(RichText::new(text).size(type_scale::SMALL).color(color::weak())).wrap());
    }
    ui.add_space(12.0);
}

/// The operation's title in the subhead style with its quiet Abort (or Reset bisect) at the
/// right; when the two don't fit on one line (a narrow panel, German), the title takes the whole
/// width and the buttons go on their own line under it. Shared with bisect's card.
pub(super) fn title_row(ui: &mut Ui, st: &AppState, snap: &Snapshot, title: &str, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let actions: Vec<_> = state_actions(snap.state).into_iter().flatten().filter(|(_, a)| destructive(a)).collect();
    let quiet = |label: &'static str| buttons::quiet(action_text(t, label)).small();
    let gap = ui.spacing().item_spacing.x;
    let buttons_w: f32 = actions.iter().map(|(label, _)| quiet(label).desired_width(ui) + gap).sum();
    let font = type_scale::strong(ui, type_scale::SUBHEAD);
    let title_w = ui.fonts(|f| f.layout_no_wrap(title.to_owned(), font.clone(), color::text()).size().x);
    let text = RichText::new(title).font(font).color(color::text());
    let mut add_buttons = |ui: &mut Ui| {
        for (label, a) in &actions {
            if ui.add(quiet(label)).clicked() {
                msgs.push(banner_msg(st, label, a));
            }
        }
    };
    if title_w + buttons_w <= ui.available_width() {
        beside(ui, add_buttons, |ui| {
            ui.add_space(2.0);
            ui.add(egui::Label::new(text).wrap());
        });
    } else {
        ui.add(egui::Label::new(text).wrap());
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = BUTTON_GAP;
            add_buttons(ui);
        });
    }
}

/// A step's row: its number (or a check once done), title and one line.
struct StepHead<'a> {
    n: usize,
    state: StepState,
    title: &'a str,
    /// What the step comes to, at the right: "2 left", "Staged: 1 · Left out: 2".
    line: &'a str,
}

/// A step: its row, then (`open`) `body` indented under the title, and the line joining it to
/// the next step down the markers' column.
fn step(ui: &mut Ui, head: StepHead, open: bool, which: OpStep, t: Lang, msgs: &mut Vec<Msg>, body: impl FnOnce(&mut Ui, &mut Vec<Msg>)) {
    let marker = step_head(ui, &head, open, which, t, msgs);
    if open {
        indented(ui, |ui| body(ui, msgs));
    }
    ui.add_space(STEP_GAP);
    let x = marker.center().x;
    let line = [egui::pos2(x, marker.bottom() + 4.0), egui::pos2(x, ui.cursor().min.y - 4.0)];
    let done = head.state == StepState::Done;
    ui.painter().line_segment(line, Stroke::new(2.0_f32, if done { color::added().gamma_multiply(0.5) } else { color::border() }));
}

/// `right` at the top right (laid out right to left), and `left` top-down in the room left, so a
/// long title wraps instead of running under the buttons.
fn beside(ui: &mut Ui, right: impl FnOnce(&mut Ui), left: impl FnOnce(&mut Ui)) {
    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        right(ui);
        ui.with_layout(Layout::top_down(Align::Min), left);
    });
}

/// `add` with the steps' indent, as wide as the room left.
fn indented<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let margin = egui::Margin { left: INDENT as i8, right: 0, top: 4, bottom: 0 };
    egui::Frame::new().inner_margin(margin).show(ui, add).inner
}

/// A step's row; returns the marker's rect.
fn step_head(ui: &mut Ui, head: &StepHead, open: bool, which: OpStep, t: Lang, msgs: &mut Vec<Msg>) -> Rect {
    ui.horizontal_top(|ui| {
        let (marker, _) = ui.allocate_exact_size(vec2(MARKER, MARKER), Sense::hover());
        paint_marker(ui, marker, head.n, head.state, which == OpStep::Resolve);
        ui.add_space(INDENT - MARKER - ui.spacing().item_spacing.x);
        // Show or Hide at the top right; the title wraps in the room left, its line under it.
        beside(
            ui,
            |ui| {
                let (label, tip) = if open {
                    (t.tr("details.steps.hide"), t.tr("details.steps.hide.tip"))
                } else {
                    (t.tr("details.steps.show"), t.tr("details.steps.show.tip"))
                };
                // A chevron, so a long title in a narrow panel keeps the room: down while open.
                let icon = if open { Icon::ChevronDown } else { Icon::ExpandRight };
                let r = ui.add(buttons::icon_only(icon, format!("{label}: {}", head.title)).tip(tip));
                if r.clicked() {
                    msgs.push(Msg::Layout(LayoutMsg::ToggleOpStep(which)));
                }
            },
            |ui| {
                ui.add_space(4.0);
                let text = match head.state {
                    StepState::Now => RichText::new(head.title).font(type_scale::strong(ui, type_scale::BODY)).color(color::text()),
                    StepState::Done => RichText::new(head.title).color(color::weak()),
                    StepState::Later => RichText::new(head.title).color(color::faint()),
                };
                ui.add(egui::Label::new(text).wrap());
                if !head.line.is_empty() {
                    let tone = if head.state == StepState::Done { color::added() } else { color::faint() };
                    ui.add(egui::Label::new(RichText::new(head.line).size(type_scale::SMALL).color(tone)).wrap());
                }
            },
        );
        marker
    })
    .inner
}

/// The step's circle: ringed in the accent (coral while conflicts wait) for the step now, filled
/// green with a check once done, a quiet ring for a step later.
fn paint_marker(ui: &Ui, rect: Rect, n: usize, state: StepState, resolve: bool) {
    let p = ui.painter();
    let (c, r) = (rect.center(), MARKER / 2.0 - 1.0);
    let number = |col| {
        let font = type_scale::strong(ui, type_scale::SMALL);
        p.text(c, egui::Align2::CENTER_CENTER, n.to_string(), font, col);
    };
    match state {
        StepState::Done => {
            p.circle_filled(c, r, color::added());
            paint_icon(p, c, Icon::Check, color::panel());
        }
        StepState::Now => {
            let col = if resolve { super::needs_you::fill() } else { color::primary() };
            p.circle_stroke(c, r - 1.0, Stroke::new(2.0_f32, col));
            number(col);
        }
        StepState::Later => {
            p.circle_stroke(c, r - 1.0, Stroke::new(2.0_f32, color::border()));
            number(color::faint());
        }
    }
}
