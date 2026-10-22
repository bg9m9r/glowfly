//! Language snapshots of the welcome, the repository lists, Find repositories and the details
//! panel: the welcome screen and its lists in German, the first-run welcome's identity step in
//! Spanish (its captions must fit their slots) and its Start step in German, Find repositories
//! in German, and the commit details, several commits (German), the WIP lists and a commit
//! compared with the working directory (Spanish). Buttons found by their translated text still
//! send their messages.

use super::common::short;
use super::details_wip::discard_all_text;
use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_compare::{compared, selected};
use super::snapshot_tests_onboarding::{ada, on};
use super::snapshot_tests_repo_finder::finder;
use super::snapshot_tests_repo_list::welcome_state;
use crate::msg::{Msg, ReposMsg};
use crate::state::onboarding::{EMAIL_FIELD, WelcomeStep};
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::repo;
use glowfly_core::settings::ThemeChoice;

/// Clicks the WIP header's button `label`, through the more-actions menu (`more`) when the
/// button moved there.
pub(super) fn click_header_action(h: &mut Harness<'_, (AppState, Vec<Msg>)>, label: &str, more: &str) {
    if h.query_by_label(label).is_none() {
        h.get_by_label(more).click();
        h.run();
    }
    h.get_by_label(label).click();
    h.run();
}

fn commit(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap().oid
}

#[test]
fn welcome_screen_german() {
    let mut st = welcome_state(ThemeChoice::Dark);
    st.settings.language = "de".into();
    let mut h = harness(st, true);
    h.run();
    for key in ["common.open_repo.button", "repos.favorites.header", "repos.recent.header", "repos.not_found.label"] {
        assert!(h.query_by_label(Lang::De.tr(key)).is_some(), "{key} is shown in German");
    }
    assert!(h.query_by_label("Open repository…").is_none(), "no English button is left");
    h.check_snapshot("welcome_screen_german");
}

#[test]
fn welcome_identity_spanish() {
    let mut st = on(ThemeChoice::Dark, WelcomeStep::Identity, Some(ada()));
    st.settings.language = "es".into();
    let f = &mut st.onboarding.welcome.as_mut().unwrap().identity;
    f.email = "ada".into();
    f.touched.mark(EMAIL_FIELD);
    let mut h = harness(st, true);
    h.run();
    for text in [
        Lang::Es.tr("welcome.title"),
        Lang::Es.tr("welcome.step.identity.title"),
        Lang::Es.tr("welcome.skip_for_now.button"),
        Lang::Es.tr("welcome.identity.email_no_at"),
    ] {
        assert!(h.query_by_label(text).is_some(), "{text} is shown");
    }
    // The captions are painted, so the snapshot shows them; they are Spanish all the same.
    assert_ne!(WelcomeStep::Identity.caption(Lang::Es), WelcomeStep::Identity.caption(Lang::En));
    h.check_snapshot("welcome_identity_spanish");
}

#[test]
fn welcome_start_german() {
    let mut st = on(ThemeChoice::Dark, WelcomeStep::Start, Some(ada()));
    st.settings.language = "de".into();
    let mut h = harness(st, true);
    h.run();
    let page = format!("{} > {}", Lang::De.tr("prefs.nav.caption"), Lang::De.tr("prefs.nav.ui"));
    let link = Lang::De.trf("welcome.start.look_link", &[("page", &page)]);
    assert!(link.contains(Lang::De.tr("prefs.nav.ui")), "{link}");
    assert!(h.query_by_label(&link).is_some(), "{link} is shown");
    assert!(h.query_by_label(Lang::De.tr("welcome.done.button")).is_some());
    for key in ["welcome.start.open.hint", "welcome.start.clone.hint", "welcome.start.init.hint"] {
        assert!(h.query_by_label(Lang::De.tr(key)).is_some(), "{key} is shown in German");
        assert!(h.query_by_label(Lang::En.tr(key)).is_none(), "{key}: no English hint is left");
    }
    assert!(h.query_by_label(Lang::De.tr("welcome.start.tour_after.check")).is_some());
    h.check_snapshot("welcome_start_german");
}

#[test]
fn repo_finder_german() {
    let mut st = finder(ThemeChoice::Dark, "");
    st.settings.language = "de".into();
    let mut h = harness(st, true);
    h.run();
    for key in ["finder.title", "finder.folders.header", "finder.add_folder.button", "finder.look_again.button"] {
        assert!(h.query_by_label(Lang::De.tr(key)).is_some(), "{key} is shown in German");
    }
    h.check_snapshot("repo_finder_german");
    h.get_by_label(Lang::De.tr("finder.look_again.button")).click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Repos(ReposMsg::Rescan))));
}

#[test]
fn details_commit_german() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "lang-details-commit");
    st.settings.language = "de".into();
    let oid = commit(&st, "Merge branch 'login-page'");
    st.selection = Selection::Commit(oid);
    let detail = repo::commit_detail(&dir, oid).unwrap();
    let n = detail.files.len();
    st.detail = Some(detail);
    let mut h = harness(st, true);
    h.run();
    let short = crate::ui::common::short(oid);
    for text in [Lang::De.trn("details.commit.changed_files", n, &[]), Lang::De.trf("details.commit.sha", &[("sha", &short)])] {
        assert!(h.query_by_label(&text).is_some(), "{text} is shown");
    }
    h.check_snapshot("details_commit_german");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn details_wip_spanish() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "lang-details-wip");
    st.settings.language = "es".into();
    let count = st.snap.as_ref().unwrap().unstaged.iter().filter(|f| f.status != '!').count();
    let mut h = harness(st, true);
    h.run();
    let header = Lang::Es.trf("details.wip.unstaged.header", &[("count", &count)]);
    // The header and its list's focus target share the name.
    assert!(h.query_all_by_label(&header).next().is_some(), "{header} is shown");
    h.check_snapshot("details_wip_spanish");
    click_header_action(&mut h, Lang::Es.tr("details.wip.discard_all.button"), Lang::Es.tr("details.wip.more.tip"));
    let body = discard_all_text(Lang::Es, &h.state().0.keymap);
    let asked = h.state().1.iter().any(
        |m| matches!(m, Msg::ConfirmDanger { title, body: b, .. } if title == Lang::Es.tr("details.wip.discard_all.title") && *b == body),
    );
    assert!(asked, "Discard all asks in Spanish");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn details_multi_german() {
    let three = ["Search results page", "Merge branch 'login-page'", "Fix build on Windows"];
    let (mut st, dir) = selected(ThemeChoice::Dark, "lang-details-multi", &three, false);
    st.settings.language = "de".into();
    let Selection::Commits(m) = &st.selection else { unreachable!() };
    let (from, to) = m.compare_ends().expect("a line of commits");
    let mut h = harness(st, true);
    h.run();
    let combined = Lang::De.trn("details.multi.combined", 3, &[]);
    for text in [short(from).to_string(), short(to).to_string(), Lang::De.tr("details.multi.swap.button").to_owned(), combined] {
        assert!(h.query_all_by_label(&text).next().is_some(), "{text} is shown");
    }
    let title = format!("{} · {}", Lang::De.trn("details.multi.selected", 3, &[]), Lang::De.tr("details.multi.esc_clear"));
    assert!(h.query_by_label(&title).is_some(), "{title} is shown");
    h.check_snapshot("details_multi_german");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn details_workdir_spanish() {
    let (mut st, dir) = compared(ThemeChoice::Dark, "lang-details-workdir", false);
    st.settings.language = "es".into();
    let oid = commit(&st, "Merge branch 'login-page'");
    let mut h = harness(st, true);
    h.run();
    let changes = Lang::Es.trf("details.workdir.changes", &[("sha", &short(oid))]);
    assert!(h.query_by_label(&changes).is_some(), "{changes} is shown");
    h.check_snapshot("details_workdir_spanish");
    drop(h);
    remove_fixture(&dir);
}
