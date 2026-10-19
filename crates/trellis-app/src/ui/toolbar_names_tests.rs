//! Accessible names of painted widgets and hint-only fields: every toolbar tool button by its
//! descriptive name (disabled exactly when it can't be used, and never the same as a palette
//! row's), graph rows by their summary ("Uncommitted changes" for
//! the working tree) and the commit box's fields by their hint. No image comparison.

use crate::ui::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::ui::snapshot_tests_remotes::with_remotes;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use trellis_core::settings::ThemeChoice;
use trellis_core::undo::Action;

const UNDO: &str = "Undo the last action";
const REDO: &str = "Redo the undone action";
const PULL: &str = "Pull the current branch";

/// Whether the toolbar's button `label` is disabled; panics unless exactly one button has it.
fn disabled(h: &egui_kittest::Harness<'_, (crate::state::AppState, Vec<crate::msg::Msg>)>, label: &str) -> bool {
    let buttons: Vec<_> = h.query_all_by_role_and_label(Role::Button, label).collect();
    assert_eq!(buttons.len(), 1, "one {label:?} button");
    buttons[0].is_disabled()
}

#[test]
fn tool_buttons_are_named_and_report_whether_they_work() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "toolbar-names");
    let snap = st.snap.as_ref().unwrap();
    let (dirty, stashed) = (snap.is_dirty(), !snap.stashes.is_empty());
    let mut h = harness(st, true);
    h.run();
    for label in [
        "Push the current branch",
        "Create a branch",
        "Show the terminal panel",
        "Open the repository folder",
        "Open settings",
        "Search and run commands",
        "Show or hide the commit search",
        "Show or hide the activity log",
        "Refresh the repository",
    ] {
        assert!(!disabled(&h, label), "{label}");
    }
    assert!(disabled(&h, UNDO) && disabled(&h, REDO), "nothing to undo or redo yet");
    assert!(disabled(&h, PULL), "no remote to pull from");
    assert_eq!(disabled(&h, "Stash all changes"), !dirty);
    assert_eq!(disabled(&h, "Pop the latest stash"), !stashed);
    drop(h);

    // With something to undo, Undo works.
    let (mut st, _) = with_repo(ThemeChoice::Dark, "toolbar-names");
    let snap = st.snap.as_ref().unwrap();
    let to = snap.head_oid.unwrap();
    let from = snap.commits[snap.row_of[&to]].parents[0];
    st.undo.record(Action::Commit { branch: Some("main".into()), from, to, message: "Last".into(), amend: false });
    let mut h = harness(st, true);
    h.run();
    assert!(!disabled(&h, UNDO) && disabled(&h, REDO));
    drop(h);

    // With an upstream for main, Pull works.
    let (mut st, _) = with_remotes(ThemeChoice::Dark, "toolbar-names");
    let main = st.snap.as_mut().unwrap().locals.iter_mut().find(|b| b.name == "main").unwrap();
    main.upstream = Some("origin/main".into());
    main.tracking = Some(trellis_core::repo::Tracking { remote: "origin".into(), merge: "main".into() });
    let mut h = harness(st, true);
    h.run();
    assert!(!disabled(&h, PULL));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn toolbar_names_differ_from_palette_rows() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "toolbar-palette-names");
    let rows: Vec<String> = crate::state::palette::shown(&st, "").into_iter().map(|s| s.entry.label).collect();
    let mut h = harness(st, true);
    h.run();
    let bar = eframe::egui::containers::panel::PanelState::load(&h.ctx, eframe::egui::Id::new("toolbar")).expect("the toolbar").rect;
    let names: Vec<String> = h
        .query_all_by_role(Role::Button)
        .filter(|n| {
            n.raw_bounds().is_some_and(|b| bar.contains(eframe::egui::pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)))
        })
        .filter_map(|n| n.label())
        .collect();
    assert!(names.contains(&"Refresh the repository".to_owned()), "{names:?}");
    for name in &names {
        assert!(!rows.contains(name), "{name:?} is both a toolbar button and a palette row");
    }
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn graph_rows_and_commit_fields_are_named() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "graph-names");
    let summary = st.snap.as_ref().unwrap().commits[0].summary.clone();
    let mut h = harness(st, true);
    h.run();
    assert_eq!(h.query_all_by_role_and_label(Role::Button, "Uncommitted changes").count(), 1);
    assert!(h.query_all_by_role_and_label(Role::Button, &summary).next().is_some(), "{summary}");
    let field = h.get_by_role_and_label(Role::TextInput, "Commit summary");
    field.type_text("Name the fields");
    h.run();
    assert_eq!(h.state().0.draft.summary, "Name the fields");
    assert!(h.query_by_label("Description").is_some());
    drop(h);
    remove_fixture(&dir);
}

/// Every tool button's name holds its caption in every language, so speech input finds the
/// button by the word it shows ("click PR", "click Commands").
#[test]
fn tool_button_names_hold_their_captions() {
    use crate::state::ForgeState;
    use trellis_core::forge::{Provider, Target};
    use trellis_core::i18n::Lang;
    for lang in Lang::ALL {
        let (mut st, dir) = with_repo(ThemeChoice::Dark, &format!("toolbar-captions-{}", lang.code()));
        st.settings.language = lang.code().into();
        let github =
            Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
        st.forge = Some(ForgeState::new(Some(github), "origin".into(), None));
        let pr = lang.trf("toolbar.pr.name_pull_request", &[("service", &"GitHub")]);
        let mut h = harness(st, true);
        h.run();
        for (caption, name) in [
            ("toolbar.undo.button", lang.tr("toolbar.undo.name")),
            ("toolbar.redo.button", lang.tr("toolbar.redo.name")),
            ("toolbar.pull.button", lang.tr("toolbar.pull.name")),
            ("toolbar.push.button", lang.tr("toolbar.push.name")),
            ("toolbar.branch.button", lang.tr("toolbar.branch.name")),
            ("toolbar.stash.button", lang.tr("toolbar.stash.name")),
            ("toolbar.pop.button", lang.tr("toolbar.pop.name")),
            ("toolbar.pr.button", &pr),
            ("toolbar.terminal.button", lang.tr("toolbar.terminal.name_show")),
            ("toolbar.folder.button", lang.tr("toolbar.folder.name")),
            ("toolbar.settings.button", lang.tr("toolbar.settings.name")),
            ("toolbar.commands.button", lang.tr("toolbar.commands.name")),
            ("toolbar.search.button", lang.tr("toolbar.search.name")),
            ("toolbar.activity.button", lang.tr("toolbar.activity.name")),
            ("toolbar.refresh.button", lang.tr("toolbar.refresh.name")),
        ] {
            let caption = lang.tr(caption);
            assert_eq!(h.query_all_by_role_and_label(Role::Button, name).count(), 1, "{lang:?}: one {name:?} button");
            assert!(name.to_lowercase().contains(&caption.to_lowercase()), "{lang:?}: {name:?} lacks its caption {caption:?}");
        }
        let mr = lang.trf("toolbar.pr.name_merge_request", &[("service", &"GitLab")]);
        assert!(mr.contains(lang.tr("toolbar.pr.button")), "{lang:?}: {mr:?}");
        drop(h);
        remove_fixture(&dir);
    }
}
