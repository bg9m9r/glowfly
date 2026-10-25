//! Left panel: local and remote branches, pull requests, issues, Git Flow branches, tags,
//! stashes, worktrees and submodules.

use super::backdrop::{self, Surface};
use super::branch_dot::{self, DOT_ROOM};
use super::branch_menu::local_branch_menu;
use super::common::{forge_error_rows, hint_row, menu_item, menu_item_if, state_color};
use super::forge_filter_menu::{self, Picker};
use super::panel_size;
use super::ref_visibility;
use super::stack_rows;
use super::theme::branch_colours::line;
use super::theme::color;
use super::widgets::section_header::{self, Header, HeaderAdd, action_item};
use super::widgets::{self, Part};
use crate::msg::BranchMsg;
use crate::msg::FilterMsg;
use crate::msg::IssueMsg;
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::pull_request::PullRequestMsg;
use crate::state::forge_filters::{self, IssueView, PrView};
use crate::state::panel_layout::Fit;
use crate::state::{AppState, ForgeState, Selection, Status};
use crate::ui::theme::controls;
use crate::ui::widgets::row_menu::RowMenuExt;
use eframe::egui;
use glowfly_core::forge::{self, FilterKind};
use glowfly_core::repo::Snapshot;
use glowfly_core::settings::{GraphFilter, SizedPanel};

type Shown<'a> = dyn Fn(&str) -> bool + 'a;

/// Draws the panel at `fit`'s width (`panel_size::side_fits`).
pub fn show(ctx: &egui::Context, st: &mut AppState, fit: Fit, msgs: &mut Vec<Msg>) {
    let id = egui::Id::new(panel_size::SIDEBAR_ID);
    panel_size::pin(ctx, id, fit.size, st.view.panel_drag == Some(SizedPanel::Sidebar));
    // The filter field's hint, cut with "…" in a narrow panel (whole on hover while empty).
    let filter_hint = st.settings.lang().tr("sidebar.filter.hint");
    let fill = backdrop::fill(st.view.backdrop, Surface::Side, color::panel());
    let resp = panel_size::side(egui::SidePanel::left(id), fit).frame(egui::Frame::new().fill(fill)).show(ctx, |ui| {
        panel_size::fill(ui);
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.add_space(widgets::PAD_X);
        ui.horizontal(|ui| {
            ui.add_space(widgets::PAD_X);
            let width = ui.available_width() - widgets::PAD_X;
            let (hint, cut) = widgets::truncate::hint(ui, filter_hint, width);
            let empty = st.view.ref_filter.is_empty();
            let r = controls::field(ui, egui::TextEdit::singleline(&mut st.view.ref_filter).hint_text(hint).desired_width(width));
            if empty && cut {
                r.on_hover_text(filter_hint);
            }
        });
        ui.add_space(8.0);
        let filter = st.snap.as_ref().map(|s| st.settings.graph_filter_for(&s.real_workdir)).unwrap_or_default();
        ref_visibility::banner(ui, st.settings.lang(), &filter, msgs);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| sections(ui, st, &filter, msgs));
    });
    panel_size::track(ctx, st, SizedPanel::Sidebar, id, resp.response.rect.width(), msgs);
}

fn sections(ui: &mut egui::Ui, st: &mut AppState, graph: &GraphFilter, msgs: &mut Vec<Msg>) {
    let actions = crate::state::section_actions::all(st);
    let t = st.settings.lang();
    let open_tabs = crate::state::tabs::open_places(&st.tabs);
    let open_issue = st.issue.as_ref().map(|v| v.key.clone());
    let trusted = crate::state::custom_commands::repo_trusted(st);
    let Some(snap) = &st.snap else { return };
    let commands = crate::state::custom_commands::MenuCommands::of(&st.custom_commands, snap, trusted, t);
    let repo_filters = st.settings.forge_filters_for(&snap.real_workdir);
    let filter = st.view.ref_filter.to_lowercase();
    let shown = |name: &str| glowfly_core::matching::contains_lower(name, &filter);
    let current = snap.head_branch.clone().unwrap_or_default();
    let sel = &st.selection;
    let fs = st.forge.as_ref();
    let idle = st.job.is_none();
    let sub_cx = crate::state::submodules::section_cx(st);
    let sections = &mut st.view.sections;
    let hover_next = &mut st.view.hover_tip_next;
    let tips_seen = &mut st.view.tips_seen;
    let slot = super::build_status::tip_slot(fs);

    let local_menu = |ui: &mut egui::Ui, msgs: &mut Vec<Msg>| action_item(ui, msgs, &actions.local);
    let local = Header {
        key: "local",
        title: t.tr("sidebar.local.header"),
        count: Some(snap.locals.len()),
        add: Some(HeaderAdd::send(&actions.local)),
        menu: Some(&local_menu),
        t,
    };
    section_header::show(ui, local, &mut sections.local, msgs);
    if sections.local {
        let mut elbows = stack_rows::Elbows::default();
        for row in crate::state::stack_rows::local_rows(snap, &shown, !filter.is_empty()) {
            let (b, dent) = (row.branch, stack_rows::indent(row.depth));
            // Noted before the hidden check, so rows scrolled out of view still anchor elbows.
            elbows.note(&b.name, ui.cursor().top() + widgets::LIST_ROW_H / 2.0);
            if widgets::list_row_hidden(ui) {
                continue;
            }
            let pr = fs.and_then(|f| Some(f.pr_label(crate::state::push_pr::open_pr(snap, f, &b.name)?.id)));
            let worktree = crate::state::worktree_branches::elsewhere(snap, &b.name);
            let mut parts = Vec::new();
            if b.ahead > 0 {
                parts.push(Part::Ahead(b.ahead));
            }
            if b.behind > 0 {
                parts.push(Part::Behind(b.behind));
            }
            if let Some(l) = &pr {
                parts.push(Part::Text(l));
            }
            if row.link.is_some_and(|l| l.behind) {
                parts.push(Part::Text(t.tr("sidebar.local.restack.part")));
            }
            if worktree.is_some() {
                parts.push(Part::Text(t.tr("sidebar.local.worktree.part")));
            }
            let label = if b.is_head { format!("✔  {}", b.name) } else { b.name.clone() };
            let selected = b.oid.is_some_and(|o| sel.includes(o));
            let right = (!parts.is_empty()).then_some((&parts[..], color::weak()));
            let (r, marker, cut) =
                widgets::list_row_cut(ui, widgets::ROW_INDENT + DOT_ROOM + dent, &label, right, slot, selected, b.is_head);
            branch_dot::paint(ui.painter(), r.rect, widgets::ROW_INDENT + dent, line(snap.branch_swatch(&b.name)));
            elbows.paint(ui.painter(), r.rect, row.depth, row.link.map(|l| l.on.as_str()), color::weak());
            if r.hovered() {
                *hover_next = b.oid;
            }
            let tip = super::build_status::tip_row(ui, &r, marker, fs, b.oid, tips_seen);
            let tracking = match (&b.upstream, &b.tracking) {
                (Some(u), _) => Some(format!("tracking {u}")),
                (None, Some(t)) => Some(format!("tracking {} (not on the remote)", t.display())),
                (None, None) => None,
            };
            // A cut name comes first, whole.
            let whole = cut.then(|| label.clone());
            let stacked = crate::state::stack_rows::tip_line(t, row.link);
            let lines: Vec<String> = [whole, tracking, stacked, tip, worktree.map(crate::state::worktree_branches::local_tip_line)]
                .into_iter()
                .flatten()
                .collect();
            let r = if lines.is_empty() {
                r
            } else {
                r.on_hover_text(lines.join(
                    "
",
                ))
            };
            if r.double_clicked() {
                msgs.push(Msg::Branch(BranchMsg::Checkout(b.name.clone())));
            } else if r.clicked()
                && let Some(o) = b.oid
            {
                msgs.push(Msg::Repo(RepoMsg::JumpTo(o)));
            }
            ref_visibility::row_marker(ui, &r, widgets::GUTTER_X, &format!("refs/heads/{}", b.name), &b.name, graph, t, msgs);
            r.row_menu(|ui| local_branch_menu(ui, t, b, snap, fs, graph, sel, &commands, msgs));
        }
    }

    let connections = &st.settings.connections;
    let filtering = !filter.is_empty();
    let add_remote = &actions.remote;
    let ctx = super::remotes::Ctx { shown: &shown, sel, current: &current, fs, graph, idle, filtering, connections, add_remote, t };
    super::remotes::section(ui, snap, &mut sections.remote, ctx, hover_next, tips_seen, msgs);

    if let Some(f) = fs.filter(|f| f.host.is_some()) {
        let title = f.prs_header(t);
        let chip_tip =
            if f.pr_noun() == "merge request" { t.tr("sidebar.mrs.filter_chip.tip") } else { t.tr("sidebar.prs.filter_chip.tip") };
        let view = forge_filters::pr_view(f, &repo_filters);
        let picker = Picker { kind: FilterKind::Prs, label: &view.label, filters: &repo_filters, provider: f.host_provider() };
        let menu = |ui: &mut egui::Ui, msgs: &mut Vec<Msg>| {
            super::pr_menu_items::pr_menu(ui, t, snap, f, idle, msgs);
            ui.separator();
            menu_item(msgs, ui, t.tr("sidebar.new_filter"), Msg::ForgeFilter(FilterMsg::NewDialog(FilterKind::Prs)));
        };
        let add = actions.pr.as_ref().map(HeaderAdd::send);
        let section = ("prs", title, chip_tip, view.rows.len());
        forge_filter_menu::header(ui, t, section, &mut sections.prs, picker, (add, &menu), msgs);
        if sections.prs {
            pull_requests(ui, snap, f, &view, sel, &shown, msgs);
        }
    }
    if let Some(f) = fs.filter(|f| f.tracker.is_some()) {
        let view = forge_filters::issue_view(f, &repo_filters);
        let provider = f.tracker.as_ref().map(|t| t.provider);
        let picker = Picker { kind: FilterKind::Issues, label: &view.label, filters: &repo_filters, provider };
        let menu = |ui: &mut egui::Ui, msgs: &mut Vec<Msg>| {
            if let Some(a) = &actions.issue {
                action_item(ui, msgs, a);
            }
            ui.separator();
            menu_item(msgs, ui, t.tr("sidebar.new_filter"), Msg::ForgeFilter(FilterMsg::NewDialog(FilterKind::Issues)));
        };
        let add = actions.issue.as_ref().map(HeaderAdd::send);
        let section = ("issues", t.tr("sidebar.issues.header"), t.tr("sidebar.issues.filter_chip.tip"), view.rows.len());
        forge_filter_menu::header(ui, t, section, &mut sections.issues, picker, (add, &menu), msgs);
        if sections.issues {
            let in_app = provider.is_some_and(forge::issues_in_app);
            issues(ui, snap, f, &view, &shown, (open_issue.as_deref(), in_app), msgs);
        }
    }
    if let Some(s) = snap.gitflow.settings() {
        let rows = super::gitflow_section::Rows { shown: &shown, sel, fs, graph, filtering, commands: &commands, t };
        super::gitflow_section::section(ui, snap, &s, &mut sections.gitflow, rows, hover_next, msgs);
    }

    let rows = super::tags::Rows { shown: &shown, sel, idle, graph, new_tag: &actions.tag, annotated: &actions.annotated_tag, t };
    super::tags::section(ui, snap, &mut sections.tags, rows, msgs);

    let stash = (&actions.stash, &actions.stash_staged);
    super::stashes::section(ui, t, snap, &mut sections.stashes, (sel, &shown), stash, msgs);
    let rows = super::worktrees::Rows { shown: &shown, sel, open_tabs: &open_tabs, add: &actions.worktree, filtering, t };
    super::worktrees::section(ui, snap, &mut sections.worktrees, rows, hover_next, msgs);
    super::submodules::section(ui, t, snap, &sub_cx, &mut sections.submodules, (&shown, filtering, &actions.submodule), msgs);
    ui.add_space(12.0);
}

fn pull_requests(ui: &mut egui::Ui, snap: &Snapshot, f: &ForgeState, view: &PrView, sel: &Selection, shown: &Shown, msgs: &mut Vec<Msg>) {
    if let Some(h) = &view.hint {
        forge_filter_menu::hint(ui, h, msgs);
    }
    if let Status::Error(e) = &f.host_status {
        forge_error_rows(ui, e, msgs);
    } else if let Some(text) = &view.empty {
        hint_row(ui, text.clone());
    }
    let slot = if view.rows.iter().any(|p| p.checks.is_some()) { super::build_status::MARKER_SLOT } else { 0.0 };
    for pr in view.rows.iter().copied().filter(|p| shown(&p.title) || shown(&p.source) || shown(&p.id.to_string())) {
        if widgets::list_row_hidden(ui) {
            continue;
        }
        let title = if pr.draft { format!("[Draft] {}", pr.title) } else { pr.title.clone() };
        let votes = pr.approvals();
        let right = format!("{}{}", f.pr_label(pr.id), if votes > 0 { format!(" ✔{votes}") } else { String::new() });
        let tip = f.pr_heads.get(&pr.id).copied();
        let selected = tip.is_some_and(|o| *sel == Selection::Commit(o));
        // The tooltip has the whole title, so a cut one needs no hover of its own.
        let (r, marker, _) = widgets::list_row_cut(
            ui,
            widgets::ROW_INDENT,
            &title,
            Some((&[Part::Text(&right)][..], color::progress())),
            slot,
            selected,
            false,
        );
        if let Some(c) = &pr.checks {
            super::build_status::paint_marker(&ui.painter_at(r.rect), marker.center(), c.state);
        }
        let r = r.on_hover_text(super::pr_menu_items::pr_tooltip(pr));
        if r.double_clicked() {
            msgs.push(Msg::PullRequest(PullRequestMsg::Open(pr.id)));
        } else if r.clicked()
            && let Some(o) = tip
        {
            msgs.push(Msg::Repo(RepoMsg::JumpTo(o)));
        }
        r.row_menu(|ui| {
            menu_item(msgs, ui, format!("Show {}", f.pr_noun()), Msg::PullRequest(PullRequestMsg::Open(pr.id)));
            menu_item(msgs, ui, "Open in browser", Msg::OpenUrl(pr.url.clone()));
            super::build_status::pr_menu_item(ui, pr, msgs);
            // A local source branch is checked out like any other (refused when another worktree
            // has it); otherwise a new branch tracking the remote one is made.
            let local = snap.locals.iter().any(|b| b.name == pr.source);
            let (blocked, m) = match crate::state::branches::pr_checkout(snap, &f.remote, &pr.source) {
                Ok(_) if local => (
                    crate::state::worktree_branches::checkout_blocked(snap, &pr.source),
                    Msg::Branch(BranchMsg::Checkout(pr.source.clone())),
                ),
                Ok(cmd) => (None, Msg::Job(JobMsg::Git { label: "Checkout".into(), cmds: vec![cmd] })),
                Err(why) => (Some(why), Msg::Job(JobMsg::Git { label: "Checkout".into(), cmds: Vec::new() })),
            };
            menu_item_if(msgs, ui, format!("Checkout {}", pr.source), blocked.as_deref(), m);
            let base = crate::state::worktree_branches::pr_base(snap, &f.remote, &pr.source).err();
            let create = Msg::Worktree(crate::worktree::WorktreeMsg::CreateForPr { id: pr.id, source: pr.source.clone() });
            menu_item_if(msgs, ui, "Create worktree…", base.as_deref(), create);
            menu_item(msgs, ui, "Copy link", Msg::Copy(pr.url.clone()));
        });
    }
}

/// The ISSUES rows. Where the tracker has issue details in Glowfly (`in_app`), a click opens
/// the issue panel (and its row shows selected while `open`); elsewhere a double-click opens
/// the issue's page.
fn issues(
    ui: &mut egui::Ui,
    snap: &Snapshot,
    f: &ForgeState,
    view: &IssueView,
    shown: &Shown,
    (open, in_app): (Option<&str>, bool),
    msgs: &mut Vec<Msg>,
) {
    forge_filter_menu::hint(ui, &view.hint, msgs);
    if let Status::Error(e) = &f.tracker_status {
        forge_error_rows(ui, e, msgs);
    }
    for issue in view.rows.iter().filter(|i| shown(&i.title) || shown(&i.key)) {
        if widgets::list_row_hidden(ui) {
            continue;
        }
        let text = format!("{}  {}", f.mention(&issue.key), issue.title);
        let selected = open == Some(issue.key.as_str());
        // The tooltip has the whole title, so a cut one needs no hover of its own.
        let right = [Part::Text(&issue.state)];
        let (r, _, _) =
            widgets::list_row_cut(ui, widgets::ROW_INDENT, &text, Some((&right[..], state_color(&issue.state))), 0.0, selected, false);
        let assignee = issue.assignee.as_deref().unwrap_or("unassigned");
        let mut tip = format!("{} {}: {}\n{} · {assignee}", issue.kind, issue.key, issue.title, issue.state);
        if let Some(line) = forge_filters::facet_line(&issue.facets) {
            tip.push_str(&format!("\n{line}"));
        }
        let r = r.on_hover_text(tip);
        if in_app && r.clicked() {
            msgs.push(Msg::Issue(IssueMsg::Open(issue.key.clone())));
        } else if !in_app && r.double_clicked() {
            msgs.push(Msg::OpenUrl(issue.url.clone()));
        }
        r.row_menu(|ui| {
            if in_app {
                menu_item(msgs, ui, format!("View {}", f.issue_noun()), Msg::Issue(IssueMsg::Open(issue.key.clone())));
            }
            menu_item(
                msgs,
                ui,
                format!("Create branch from {}…", f.issue_noun()),
                Msg::Branch(BranchMsg::NewBranch { start: None, name: forge::branch_name_for(issue) }),
            );
            super::gitflow_section::issue_items(ui, snap, f, issue, msgs);
            menu_item(msgs, ui, f.open_issue_label(), Msg::OpenUrl(issue.url.clone()));
            let (label, text) = f.copy_issue_item(issue);
            menu_item(msgs, ui, label, Msg::Copy(text));
        });
    }
}

#[cfg(test)]
#[path = "snapshot_tests/snapshot_tests_section_headers.rs"]
mod snapshot_tests_section_headers;
