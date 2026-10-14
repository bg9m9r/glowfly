//! Left panel: local and remote branches, pull requests, issues, tags and stashes.

use super::branch_menu::local_branch_menu;
use super::common::{forge_error_rows, hint_row, menu_item, state_color};
use super::menus::remote_branch_menu;
use super::ref_visibility;
use super::theme::color;
use super::widgets;
use crate::msg::Msg;
use crate::state::{AppState, ForgeState, Selection, Status};
use eframe::egui::{self, RichText};
use trellis_core::forge;
use trellis_core::settings::GraphFilter;

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
            let filter = st.snap.as_ref().map(|s| st.settings.graph_filter_for(&s.workdir)).unwrap_or_default();
            ref_visibility::banner(ui, &filter, msgs);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| sections(ui, st, &filter, msgs));
        });
}

fn sections(ui: &mut egui::Ui, st: &mut AppState, graph: &GraphFilter, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    let filter = st.view.ref_filter.to_lowercase();
    let shown = |name: &str| filter.is_empty() || name.to_lowercase().contains(&filter);
    let current = snap.head_branch.clone().unwrap_or_default();
    let sel = st.selection.clone();
    let fs = st.forge.as_ref();
    let sections = &mut st.view.sections;
    let hover_next = &mut st.view.hover_tip_next;

    widgets::section_header(ui, "LOCAL", Some(snap.locals.len()), &mut sections.local);
    if sections.local {
        for b in snap.locals.iter().filter(|b| shown(&b.name)) {
            let mut right = String::new();
            if b.ahead > 0 {
                right.push_str(&format!("{}↑ ", b.ahead));
            }
            if b.behind > 0 {
                right.push_str(&format!("{}↓", b.behind));
            }
            if let Some((f, pr)) = fs.and_then(|f| Some((f, f.pr_for_branch(&b.name)?))) {
                right.push_str(&format!(" {}", f.pr_label(pr.id)));
            }
            let label = if b.is_head { format!("✔  {}", b.name) } else { b.name.clone() };
            let selected = b.oid.is_some_and(|o| sel == Selection::Commit(o));
            let r = widgets::list_row(ui, 26.0, &label, (!right.is_empty()).then_some((right.trim(), color::weak())), selected, b.is_head);
            if r.hovered() {
                *hover_next = b.oid;
            }
            let r = match (&b.upstream, &b.tracking) {
                (Some(u), _) => r.on_hover_text(format!("tracking {u}")),
                (None, Some(t)) => r.on_hover_text(format!("tracking {} (not on the remote)", t.display())),
                (None, None) => r,
            };
            if r.double_clicked() {
                msgs.push(Msg::git("Checkout", &["checkout", &b.name]));
            } else if r.clicked()
                && let Some(o) = b.oid
            {
                msgs.push(Msg::JumpTo(o));
            }
            ref_visibility::row_marker(ui, &r, 13.0, &format!("refs/heads/{}", b.name), &b.name, graph, msgs);
            r.context_menu(|ui| local_branch_menu(ui, b, snap, fs, graph, msgs));
        }
    }

    let remote_count = snap.remotes.iter().map(|(_, b)| b.len()).sum();
    widgets::section_header(ui, "REMOTE", Some(remote_count), &mut sections.remote);
    if sections.remote {
        for (remote, branches) in &snap.remotes {
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                ui.add(egui::Label::new(RichText::new(remote).color(color::weak()).size(12.0)).selectable(false));
            });
            for (name, oid) in branches.iter().filter(|(n, _)| shown(n)) {
                let r = widgets::list_row(ui, 34.0, name, None, sel == Selection::Commit(*oid), false);
                if r.hovered() {
                    *hover_next = Some(*oid);
                }
                let full = format!("{remote}/{name}");
                if r.double_clicked() {
                    msgs.push(Msg::git("Checkout", &["checkout", "--track", &full]));
                } else if r.clicked() {
                    msgs.push(Msg::JumpTo(*oid));
                }
                ref_visibility::row_marker(ui, &r, 20.0, &format!("refs/remotes/{full}"), &full, graph, msgs);
                r.context_menu(|ui| remote_branch_menu(ui, remote, name, &current, fs, graph, msgs));
            }
        }
    }

    if let Some(f) = fs.filter(|f| f.host.is_some()) {
        let title = if f.pr_noun() == "merge request" { "MERGE REQUESTS" } else { "PULL REQUESTS" };
        widgets::section_header(ui, title, Some(f.prs.len()), &mut sections.prs);
        if sections.prs {
            pull_requests(ui, f, &sel, &shown, msgs);
        }
    }
    if let Some(f) = fs.filter(|f| f.tracker.is_some()) {
        widgets::section_header(ui, "ISSUES", Some(f.my_issues.len()), &mut sections.issues);
        if sections.issues {
            issues(ui, f, &shown, msgs);
        }
    }

    widgets::section_header(ui, "TAGS", Some(snap.tags.len()), &mut sections.tags);
    if sections.tags {
        for (name, oid) in snap.tags.iter().filter(|(n, _)| shown(n)) {
            let r = widgets::list_row(ui, 26.0, name, None, sel == Selection::Commit(*oid), false);
            if r.clicked() {
                msgs.push(Msg::JumpTo(*oid));
            }
            r.context_menu(|ui| {
                menu_item(msgs, ui, "Checkout (detached)", Msg::git("Checkout", &["checkout", &format!("refs/tags/{name}")]));
                for (remote, _) in &snap.remotes {
                    menu_item(msgs, ui, format!("Push to {remote}"), Msg::git("Push tag", &["push", remote, &format!("refs/tags/{name}")]));
                }
                menu_item(
                    msgs,
                    ui,
                    "Delete",
                    Msg::confirm("Delete tag", format!("Delete local tag {name}?"), "Delete", Msg::git("Delete tag", &["tag", "-d", name])),
                );
            });
        }
    }

    widgets::section_header(ui, "STASHES", Some(snap.stashes.len()), &mut sections.stashes);
    if sections.stashes {
        for s in snap.stashes.iter().filter(|s| shown(&s.message)) {
            let r = widgets::list_row(ui, 26.0, &s.message, None, sel == Selection::Commit(s.oid), false);
            let refname = format!("stash@{{{}}}", s.index);
            if r.clicked() {
                msgs.push(Msg::SelectCommit(s.oid));
            }
            r.context_menu(|ui| {
                menu_item(msgs, ui, "Apply", Msg::git("Apply stash", &["stash", "apply", &refname]));
                menu_item(msgs, ui, "Pop", Msg::git("Pop stash", &["stash", "pop", &refname]));
                menu_item(
                    msgs,
                    ui,
                    "Delete",
                    Msg::confirm(
                        "Delete stash",
                        format!("Delete \"{}\"? This cannot be undone.", s.message),
                        "Delete",
                        Msg::git("Drop stash", &["stash", "drop", &refname]),
                    ),
                );
            });
        }
    }
    ui.add_space(12.0);
}

fn pull_requests(ui: &mut egui::Ui, f: &ForgeState, sel: &Selection, shown: &dyn Fn(&str) -> bool, msgs: &mut Vec<Msg>) {
    if let Status::Error(e) = &f.host_status {
        forge_error_rows(ui, e, msgs);
    } else if f.prs.is_empty() {
        hint_row(ui, if f.loading > 0 { "Loading…".to_owned() } else { format!("No open {}s", f.pr_noun()) });
    }
    for pr in f.prs.iter().filter(|p| shown(&p.title) || shown(&p.source) || shown(&p.id.to_string())) {
        let title = if pr.draft { format!("[Draft] {}", pr.title) } else { pr.title.clone() };
        let votes = pr.approvals();
        let right = format!("{}{}", f.pr_label(pr.id), if votes > 0 { format!(" ✔{votes}") } else { String::new() });
        let tip = f.pr_heads.get(&pr.id).copied();
        let selected = tip.is_some_and(|o| *sel == Selection::Commit(o));
        let r = widgets::list_row(ui, 26.0, &title, Some((&right, color::purple())), selected, false);
        let mut tooltip = format!("{} → {}\nby {}", pr.source, pr.target, pr.author);
        for rv in &pr.reviewers {
            tooltip.push_str(&format!("\n{}: {}{}", rv.name, rv.state, if rv.required { " (required)" } else { "" }));
        }
        if let Some(ms) = &pr.merge_status {
            tooltip.push_str(&format!("\nmerge: {ms}"));
        }
        let d = pr.description.trim();
        if !d.is_empty() {
            let d: String = d.chars().take(300).collect();
            tooltip.push_str(&format!("\n\n{d}"));
        }
        let r = r.on_hover_text(tooltip);
        if r.double_clicked() {
            msgs.push(Msg::OpenUrl(pr.url.clone()));
        } else if r.clicked()
            && let Some(o) = tip
        {
            msgs.push(Msg::JumpTo(o));
        }
        r.context_menu(|ui| {
            menu_item(msgs, ui, "Open in browser", Msg::OpenUrl(pr.url.clone()));
            menu_item(msgs, ui, format!("Checkout {}", pr.source), Msg::git("Checkout", &["checkout", &pr.source]));
            menu_item(msgs, ui, "Copy link", Msg::Copy(pr.url.clone()));
        });
    }
}

fn issues(ui: &mut egui::Ui, f: &ForgeState, shown: &dyn Fn(&str) -> bool, msgs: &mut Vec<Msg>) {
    hint_row(ui, format!("{} · assigned to me", f.tracker_name()));
    if let Status::Error(e) = &f.tracker_status {
        forge_error_rows(ui, e, msgs);
    }
    for issue in f.my_issues.iter().filter(|i| shown(&i.title) || shown(&i.key)) {
        let text = format!("{}  {}", f.mention(&issue.key), issue.title);
        let r = widgets::list_row(ui, 26.0, &text, Some((&issue.state, state_color(&issue.state))), false, false);
        let assignee = issue.assignee.as_deref().unwrap_or("unassigned");
        let r = r.on_hover_text(format!("{} {}: {}\n{} · {assignee}", issue.kind, issue.key, issue.title, issue.state));
        if r.double_clicked() {
            msgs.push(Msg::OpenUrl(issue.url.clone()));
        }
        r.context_menu(|ui| {
            menu_item(
                msgs,
                ui,
                format!("Create branch from {}…", f.issue_noun()),
                Msg::NewBranch { start: None, name: forge::branch_name_for(issue) },
            );
            menu_item(msgs, ui, "Open in browser", Msg::OpenUrl(issue.url.clone()));
            let m = f.mention(&issue.key);
            menu_item(msgs, ui, format!("Copy {m}"), Msg::Copy(m.clone()));
        });
    }
}
