//! Right panel: staging and committing (WIP), or the selected commit's details.

use super::UiCache;
use super::common::{issue_chip, lane_color, menu_item, short, state_actions, status_color, status_label};
use super::graph::draw_avatar;
use super::spell_edit::spell_text_edit;
use super::theme::color;
use crate::msg::Msg;
use crate::state::{AppState, Selection};
use eframe::egui::{self, Align, Color32, Layout, RichText, Sense, vec2};
use trellis_core::repo::{DiffTarget, FileEntry};
use trellis_core::undo::DISCARD_STASH_MESSAGE;

pub fn show(ctx: &egui::Context, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    egui::SidePanel::right("detail")
        .resizable(true)
        .default_width(400.0)
        .max_width(640.0)
        .frame(egui::Frame::new().fill(color::panel()).inner_margin(egui::Margin::same(10)))
        .show(ctx, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            match st.selection {
                Selection::Wip => wip_panel(ui, st, cache, msgs),
                Selection::Commit(_) => commit_panel(ui, st, cache, msgs),
                Selection::None => {
                    ui.label(RichText::new("Select a commit to see its details.").color(color::weak()));
                    if st.snap.as_ref().is_some_and(|s| s.is_dirty()) {
                        ui.add_space(8.0);
                        if ui.button("View uncommitted changes").clicked() {
                            msgs.push(Msg::SelectWip);
                        }
                    }
                }
            }
        });
}

fn wip_panel(ui: &mut egui::Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    let open = OpenFile { diff: st.diff.as_ref().map(|(t, _)| t), conflict: st.conflict.as_ref().map(|c| c.path.as_str()) };

    // An operation in progress, or conflicts left by one that has none (a stash apply).
    let conflicted: Vec<&FileEntry> = snap.unstaged.iter().filter(|f| f.status == '!').collect();
    let in_progress = state_actions(snap.state);
    if in_progress.is_some() || !conflicted.is_empty() {
        egui::Frame::group(ui.style()).fill(color::banner()).show(ui, |ui| {
            if let Some((name, _)) = in_progress {
                ui.label(RichText::new(format!("{name} in progress")).strong());
            }
            if let Some(first) = conflicted.first() {
                ui.horizontal(|ui| {
                    let n = conflicted.len();
                    ui.label(format!("{n} conflicted file{}", if n == 1 { "" } else { "s" }));
                    if ui.button("View conflicts").clicked() {
                        msgs.push(Msg::OpenConflict(first.path.clone()));
                    }
                });
            }
            let Some((_, actions)) = in_progress else { return };
            ui.horizontal(|ui| {
                for (label, a) in actions {
                    if ui.button(*label).clicked() {
                        msgs.push(Msg::git(label, a));
                    }
                }
            });
        });
        ui.add_space(6.0);
    }

    let list_h = ((ui.available_height() - 300.0) / 2.0).max(80.0);
    file_list(ui, "Unstaged Files", &snap.unstaged, false, &open, list_h, msgs);
    ui.add_space(8.0);
    file_list(ui, "Staged Files", &snap.staged, true, &open, list_h, msgs);

    ui.add_space(6.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Commit").strong().color(color::text()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.checkbox(&mut st.draft.amend, "Amend").changed() && st.draft.amend {
                msgs.push(Msg::PrefillAmend);
            }
        });
    });

    // Link the issue encoded in the branch name (e.g. 1234-login or PROJ-12-crash).
    if let Some((fs, key)) = st.forge.as_ref().and_then(|f| Some((f, f.branch_issue(snap.head_branch.as_deref()?)?))) {
        let tag = fs.mention(&key);
        let mentioned = st.draft.summary.contains(&tag) || st.draft.description.contains(&tag);
        ui.horizontal(|ui| {
            issue_chip(ui, fs, &key, msgs);
            if !mentioned && ui.small_button(format!("Link {tag}")).on_hover_text("Adds the mention to the description").clicked() {
                msgs.push(Msg::AppendToDescription(tag.clone()));
            }
        });
    }

    let spell = st.settings.spellcheck;
    spell_text_edit(ui, &mut st.draft.summary, "Commit summary", 1, &mut cache.speller, spell);
    let len = st.draft.summary.chars().count();
    let c = if len > 72 { color::red() } else { color::faint() };
    ui.label(RichText::new(format!("{len}/72")).small().color(c));
    spell_text_edit(ui, &mut st.draft.description, "Description", 4, &mut cache.speller, spell);
    let n = snap.staged.len();
    let label = if st.draft.amend {
        "Amend previous commit".to_owned()
    } else if n == 0 {
        "Stage files to commit".to_owned()
    } else {
        format!("Commit changes to {n} file{}", if n == 1 { "" } else { "s" })
    };
    let enabled = st.job.is_none() && !st.draft.summary.trim().is_empty() && (n > 0 || st.draft.amend);
    let button = egui::Button::new(RichText::new(label).strong().color(if enabled { Color32::WHITE } else { color::faint() }))
        .fill(if enabled { color::primary() } else { color::toolbar() })
        .min_size(vec2(ui.available_width(), 34.0));
    if ui.add_enabled(enabled, button).clicked() {
        msgs.push(Msg::Commit);
    }
}

fn commit_panel(ui: &mut egui::Ui, st: &AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(d) = &st.detail else {
        ui.label(RichText::new("Loading…").color(color::weak()));
        return;
    };
    let open = st.diff.as_ref().map(|(t, _)| t);
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("commit {}", short(d.oid))).monospace().color(color::weak()));
        if ui.small_button("Copy SHA").clicked() {
            msgs.push(Msg::Copy(d.oid.to_string()));
        }
    });
    let (summary, body) = d.message.split_once('\n').unwrap_or((&d.message, ""));
    ui.add(egui::Label::new(RichText::new(summary).strong().size(15.0).color(color::text())).wrap());
    if !body.trim().is_empty() {
        egui::ScrollArea::vertical().id_salt("commit-body").max_height(180.0).show(ui, |ui| {
            ui.add(egui::Label::new(RichText::new(body.trim()).color(color::weak())).wrap());
        });
    }

    if let Some(fs) = &st.forge {
        let keys = fs.issue_refs(&d.message);
        if !keys.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new(format!("{}S", fs.issue_noun().to_uppercase())).size(11.0).color(color::weak()));
            for k in keys {
                issue_chip(ui, fs, &k, msgs);
            }
        }
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
        let av = if st.settings.use_initials { None } else { Some(&mut cache.avatars) };
        draw_avatar(ui.painter(), rect.center(), 15.0, &d.author, &d.email, lane_color(0), av);
        ui.vertical(|ui| {
            ui.label(RichText::new(format!("{} <{}>", d.author, d.email)).color(color::text()));
            let mut when = st.settings.format_verbose(d.time, st.view.utc_offset);
            if d.committer != d.author {
                when.push_str(&format!(" · committed by {}", d.committer));
            }
            ui.label(RichText::new(when).color(color::weak()));
        });
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("parents").color(color::weak()));
        for p in &d.parents {
            if ui.link(RichText::new(short(*p)).monospace()).clicked() {
                msgs.push(Msg::JumpTo(*p));
            }
        }
    });
    ui.separator();
    let n = d.files.len();
    ui.label(RichText::new(format!("{n} changed file{}", if n == 1 { "" } else { "s" })).strong().color(color::text()));
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for f in &d.files {
            let target = DiffTarget::Commit(d.oid, f.path.clone());
            ui.horizontal(|ui| {
                ui.label(RichText::new(status_label(f.status).to_string()).monospace().strong().color(status_color(f.status)));
                let r = ui.selectable_label(open == Some(&target), &f.path);
                if r.clicked() {
                    msgs.push(Msg::OpenDiff(target));
                }
                r.context_menu(|ui| {
                    menu_item(msgs, ui, "File history", Msg::OpenFileHistory { path: f.path.clone(), at: Some(d.oid) });
                    menu_item(msgs, ui, "Copy path", Msg::Copy(f.path.clone()));
                });
            });
        }
    });
}

/// What the central panel shows, so its file row is drawn selected.
struct OpenFile<'a> {
    diff: Option<&'a DiffTarget>,
    conflict: Option<&'a str>,
}

fn file_list(ui: &mut egui::Ui, title: &str, files: &[FileEntry], staged: bool, open: &OpenFile, max_h: f32, msgs: &mut Vec<Msg>) {
    egui::Frame::new().fill(color::toolbar()).inner_margin(egui::Margin::symmetric(8, 5)).corner_radius(3).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{title} ({})", files.len())).strong().color(color::text()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if files.is_empty() {
                    return;
                }
                if staged {
                    if ui.small_button("Unstage all").clicked() {
                        msgs.push(Msg::UnstageAll);
                    }
                } else {
                    if ui.small_button("Stage all").clicked() {
                        msgs.push(Msg::StageAll);
                    }
                    if ui.small_button("Discard all").clicked() {
                        // Discarding goes through a stash so it can be recovered.
                        msgs.push(Msg::confirm(
                            "Discard all changes",
                            format!(
                                "All uncommitted changes will be moved into a stash named \"{DISCARD_STASH_MESSAGE}\". Undo (Ctrl+Z) \
                                 or applying that stash brings them back; dropping the stash deletes them permanently."
                            ),
                            "Discard all",
                            Msg::git("Discard all changes", &["stash", "push", "-u", "-m", DISCARD_STASH_MESSAGE]),
                        ));
                    }
                }
            });
        });
    });
    egui::ScrollArea::vertical().id_salt(title).max_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
        if files.is_empty() {
            ui.label(RichText::new("None").color(color::faint()));
        }
        for f in files {
            let target = if staged { DiffTarget::Staged(f.path.clone()) } else { DiffTarget::Unstaged(f.path.clone()) };
            let toggle = || if staged { Msg::UnstageFile(f.path.clone()) } else { Msg::StageFile(f.path.clone()) };
            // A conflicted file opens in the conflict view; staging it marks it resolved.
            let conflicted = !staged && f.status == '!';
            ui.horizontal(|ui| {
                ui.label(RichText::new(status_label(f.status).to_string()).monospace().strong().color(status_color(f.status)));
                let action = match (staged, conflicted) {
                    (true, _) => "Unstage",
                    (false, true) => "Mark resolved",
                    (false, false) => "Stage",
                };
                if ui.small_button(action).clicked() {
                    msgs.push(toggle());
                }
                let selected = if conflicted { open.conflict == Some(f.path.as_str()) } else { open.diff == Some(&target) };
                let r = ui.selectable_label(selected, &f.path);
                if r.clicked() {
                    msgs.push(if conflicted { Msg::OpenConflict(f.path.clone()) } else { Msg::OpenDiff(target.clone()) });
                }
                r.context_menu(|ui| {
                    if conflicted {
                        menu_item(msgs, ui, "Open conflict", Msg::OpenConflict(f.path.clone()));
                    }
                    menu_item(msgs, ui, action, toggle());
                    // Checking out the index version of a conflicted path means nothing.
                    if !staged && !conflicted {
                        let untracked = f.status == '?';
                        let what = if untracked { "Delete untracked file" } else { "Discard changes to" };
                        menu_item(
                            msgs,
                            ui,
                            "Discard changes",
                            Msg::confirm(
                                "Discard changes",
                                format!("{what} {}? This cannot be undone.", f.path),
                                "Discard",
                                Msg::DiscardFile(f.path.clone(), untracked),
                            ),
                        );
                    }
                    if f.status != '?' {
                        menu_item(msgs, ui, "File history", Msg::OpenFileHistory { path: f.path.clone(), at: None });
                    }
                    menu_item(msgs, ui, "Copy path", Msg::Copy(f.path.clone()));
                });
            });
        }
    });
}
