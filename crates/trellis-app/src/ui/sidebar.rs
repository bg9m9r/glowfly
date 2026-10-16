//! Left panel: local and remote branches, pull requests, issues, Git Flow branches, tags,
//! stashes, worktrees and submodules.

use super::branch_menu::local_branch_menu;
use super::common::{forge_error_rows, hint_row, menu_item, menu_item_if, state_color};
use super::forge_filter_menu::{self, Picker};
use super::ref_visibility;
use super::theme::color;
use super::widgets;
use crate::msg::BranchMsg;
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::StashMsg;
use crate::pull_request::PullRequestMsg;
use crate::state::forge_filters::{self, IssueView, PrView};
use crate::state::palette::StashAction;
use crate::state::{AppState, ForgeState, Selection, Status};
use eframe::egui;
use trellis_core::forge::{self, FilterKind};
use trellis_core::repo::Snapshot;
use trellis_core::settings::GraphFilter;

type Shown<'a> = dyn Fn(&str) -> bool + 'a;

pub fn show(ctx: &egui::Context, st: &mut AppState, msgs: &mut Vec<Msg>) {
    egui::SidePanel::left("refs")
        .resizable(true)
        .default_width(260.0)
        .max_width(440.0)
        .frame(egui::Frame::new().fill(color::panel()))
        .show(ctx, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.add(
                    egui::TextEdit::singleline(&mut st.view.ref_filter)
                        .hint_text("Filter (branches, PRs, issues)")
                        .desired_width(ui.available_width() - 8.0),
                );
            });
            ui.add_space(8.0);
            let filter = st.snap.as_ref().map(|s| st.settings.graph_filter_for(&s.real_workdir)).unwrap_or_default();
            ref_visibility::banner(ui, &filter, msgs);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| sections(ui, st, &filter, msgs));
        });
}

fn sections(ui: &mut egui::Ui, st: &mut AppState, graph: &GraphFilter, msgs: &mut Vec<Msg>) {
    let open_tabs = crate::state::tabs::open_places(&st.tabs);
    let Some(snap) = &st.snap else { return };
    let repo_filters = st.settings.forge_filters_for(&snap.real_workdir);
    let filter = st.view.ref_filter.to_lowercase();
    let shown = |name: &str| trellis_core::matching::contains_lower(name, &filter);
    let current = snap.head_branch.clone().unwrap_or_default();
    let sel = st.selection.clone();
    let fs = st.forge.as_ref();
    let idle = st.job.is_none();
    let sections = &mut st.view.sections;
    let hover_next = &mut st.view.hover_tip_next;
    let tips_seen = &mut st.view.tips_seen;
    let slot = super::build_status::tip_slot(fs);

    widgets::section_header(ui, "LOCAL", Some(snap.locals.len()), &mut sections.local);
    if sections.local {
        for b in snap.locals.iter().filter(|b| shown(&b.name)) {
            if widgets::list_row_hidden(ui) {
                continue;
            }
            let mut right = String::new();
            if b.ahead > 0 {
                right.push_str(&format!("{}↑ ", b.ahead));
            }
            if b.behind > 0 {
                right.push_str(&format!("{}↓", b.behind));
            }
            if let Some((f, pr)) = fs.and_then(|f| Some((f, crate::state::push_pr::open_pr(snap, f, &b.name)?))) {
                right.push_str(&format!(" {}", f.pr_label(pr.id)));
            }
            let label = if b.is_head { format!("✔  {}", b.name) } else { b.name.clone() };
            let selected = b.oid.is_some_and(|o| sel == Selection::Commit(o));
            let right = (!right.is_empty()).then_some((right.trim(), color::weak()));
            let (r, marker) = widgets::list_row_slot(ui, 26.0, &label, right, slot, selected, b.is_head);
            if r.hovered() {
                *hover_next = b.oid;
            }
            let tip = super::build_status::tip_row(ui, &r, marker, fs, b.oid, tips_seen);
            let tracking = match (&b.upstream, &b.tracking) {
                (Some(u), _) => Some(format!("tracking {u}")),
                (None, Some(t)) => Some(format!("tracking {} (not on the remote)", t.display())),
                (None, None) => None,
            };
            let r = match (tracking, tip) {
                (Some(t), Some(c)) => r.on_hover_text(format!(
                    "{t}
{c}"
                )),
                (Some(t), None) | (None, Some(t)) => r.on_hover_text(t),
                (None, None) => r,
            };
            if r.double_clicked() {
                msgs.push(Msg::Job(JobMsg::Git { label: "Checkout".into(), cmds: vec![crate::state::branches::checkout_local(&b.name)] }));
            } else if r.clicked()
                && let Some(o) = b.oid
            {
                msgs.push(Msg::Repo(RepoMsg::JumpTo(o)));
            }
            ref_visibility::row_marker(ui, &r, 13.0, &format!("refs/heads/{}", b.name), &b.name, graph, msgs);
            r.context_menu(|ui| local_branch_menu(ui, b, snap, fs, graph, &sel, msgs));
        }
    }

    let connections = &st.settings.connections;
    let ctx =
        super::remotes::Ctx { shown: &shown, sel: &sel, current: &current, fs, graph, idle, filtering: !filter.is_empty(), connections };
    super::remotes::section(ui, snap, &mut sections.remote, ctx, hover_next, tips_seen, msgs);

    if let Some(f) = fs.filter(|f| f.host.is_some()) {
        let title = if f.pr_noun() == "merge request" { "MERGE REQUESTS" } else { "PULL REQUESTS" };
        let view = forge_filters::pr_view(f, &repo_filters);
        let picker = Picker { kind: FilterKind::Prs, label: &view.label, filters: &repo_filters, provider: f.host_provider() };
        forge_filter_menu::header(ui, title, view.rows.len(), &mut sections.prs, picker, msgs);
        if sections.prs {
            pull_requests(ui, snap, f, &view, &sel, &shown, msgs);
        }
    }
    if let Some(f) = fs.filter(|f| f.tracker.is_some()) {
        let view = forge_filters::issue_view(f, &repo_filters);
        let provider = f.tracker.as_ref().map(|t| t.provider);
        let picker = Picker { kind: FilterKind::Issues, label: &view.label, filters: &repo_filters, provider };
        forge_filter_menu::header(ui, "ISSUES", view.rows.len(), &mut sections.issues, picker, msgs);
        if sections.issues {
            issues(ui, snap, f, &view, &shown, msgs);
        }
    }
    if let Some(s) = snap.gitflow.settings() {
        let rows = super::gitflow_section::Rows { shown: &shown, sel: &sel, fs, graph, filtering: !filter.is_empty() };
        super::gitflow_section::section(ui, snap, &s, &mut sections.gitflow, rows, hover_next, msgs);
    }

    let rows = super::tags::Rows { shown: &shown, sel: &sel, idle, graph };
    super::tags::section(ui, snap, &mut sections.tags, rows, msgs);

    widgets::section_header(ui, "STASHES", Some(snap.stashes.len()), &mut sections.stashes);
    if sections.stashes {
        for s in snap.stashes.iter().filter(|s| shown(&s.message)) {
            if widgets::list_row_hidden(ui) {
                continue;
            }
            let r = widgets::list_row(ui, 26.0, &s.message, None, sel == Selection::Commit(s.oid), false);
            let stash = |action| Msg::Job(JobMsg::Stash { action, index: s.index, oid: s.oid });
            if r.clicked() {
                msgs.push(Msg::Repo(RepoMsg::SelectCommit(s.oid)));
            }
            r.context_menu(|ui| {
                menu_item(msgs, ui, "Apply", stash(StashAction::Apply));
                menu_item(msgs, ui, "Pop", stash(StashAction::Pop));
                menu_item(msgs, ui, "Rename…", Msg::Stash(StashMsg::RenameDialog { oid: s.oid }));
                menu_item(
                    msgs,
                    ui,
                    "Delete",
                    Msg::confirm(
                        "Delete stash",
                        format!("Delete \"{}\"? This cannot be undone.", s.message),
                        "Delete",
                        stash(StashAction::Drop),
                    ),
                );
            });
        }
    }
    if crate::state::worktrees::section_shown(snap) {
        let rows = super::worktrees::Rows { shown: &shown, sel: &sel, open_tabs: &open_tabs };
        super::worktrees::section(ui, snap, &mut sections.worktrees, rows, hover_next, msgs);
    }
    if !snap.submodules.is_empty() {
        super::submodules::section(ui, snap, &mut sections.submodules, &shown, msgs);
    }
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
        let (r, marker) = widgets::list_row_slot(ui, 26.0, &title, Some((&right, color::purple())), slot, selected, false);
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
        r.context_menu(|ui| {
            menu_item(msgs, ui, format!("Show {}", f.pr_noun()), Msg::PullRequest(PullRequestMsg::Open(pr.id)));
            menu_item(msgs, ui, "Open in browser", Msg::OpenUrl(pr.url.clone()));
            super::build_status::pr_menu_item(ui, pr, msgs);
            let checkout = crate::state::branches::pr_checkout(snap, &f.remote, &pr.source);
            let (blocked, cmds) = match checkout {
                Ok(cmd) => (None, vec![cmd]),
                Err(why) => (Some(why), Vec::new()),
            };
            let m = Msg::Job(JobMsg::Git { label: "Checkout".into(), cmds });
            menu_item_if(msgs, ui, format!("Checkout {}", pr.source), blocked.as_deref(), m);
            menu_item(msgs, ui, "Copy link", Msg::Copy(pr.url.clone()));
        });
    }
}

fn issues(ui: &mut egui::Ui, snap: &Snapshot, f: &ForgeState, view: &IssueView, shown: &Shown, msgs: &mut Vec<Msg>) {
    forge_filter_menu::hint(ui, &view.hint, msgs);
    if let Status::Error(e) = &f.tracker_status {
        forge_error_rows(ui, e, msgs);
    }
    for issue in view.rows.iter().filter(|i| shown(&i.title) || shown(&i.key)) {
        if widgets::list_row_hidden(ui) {
            continue;
        }
        let text = format!("{}  {}", f.mention(&issue.key), issue.title);
        let r = widgets::list_row(ui, 26.0, &text, Some((&issue.state, state_color(&issue.state))), false, false);
        let assignee = issue.assignee.as_deref().unwrap_or("unassigned");
        let mut tip = format!("{} {}: {}\n{} · {assignee}", issue.kind, issue.key, issue.title, issue.state);
        if let Some(line) = forge_filters::facet_line(&issue.facets) {
            tip.push_str(&format!("\n{line}"));
        }
        let r = r.on_hover_text(tip);
        if r.double_clicked() {
            msgs.push(Msg::OpenUrl(issue.url.clone()));
        }
        r.context_menu(|ui| {
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
