//! Headless tests for glowfly:// links: which URL and name a copied link carries, the toast and
//! clipboard after the runner builds it, reading a link that arrives (refused text, the search
//! candidates, a dialog in the way, a newer link, one whose URL is a file or network path), and
//! the scheme registration's status.

use super::branch_tests::with_origin;
use super::tests::{drive, git, state};
use super::update;
use crate::deep_link::{DeepLinkEffect, DeepLinkMsg, FindOrigin, LinkTarget};
use crate::effect::{Effect, UiEffect};
use crate::msg::{Msg, PrefsMsg, RepoMsg};
use crate::state::deep_link::{LINK_WAITING, LinkDialog, SchemeView};
use crate::state::{AppState, Modal, PrefPage, Tab};
use glowfly_core::link::{self, Link, Target};
use glowfly_core::url_scheme::SchemeStatus;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(super) const ROOT: &str = "0123456789abcdef0123456789abcdef01234567";

pub(super) fn send(st: &mut AppState, m: DeepLinkMsg) -> Vec<Effect> {
    update(st, Msg::DeepLink(m), Instant::now())
}

pub(super) fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// The Build effect `fx` holds: (url, target, what).
fn build(fx: &[Effect]) -> (Option<String>, Target, String) {
    match fx {
        [Effect::DeepLink(DeepLinkEffect::Build { url, target, what, .. })] => (url.clone(), target.clone(), what.clone()),
        other => panic!("expected one Build, got {other:?}"),
    }
}

fn copy(st: &mut AppState, t: LinkTarget) -> (Option<String>, Target, String) {
    build(&send(st, DeepLinkMsg::Copy(t)))
}

fn url_of(st: &AppState, remote: &str) -> Option<String> {
    st.snap.as_ref().unwrap().remote_urls.iter().find(|(n, _)| n == remote).map(|(_, u)| u.clone())
}

/// Points origin at an https URL in the snapshot (the fixture's is a folder); returns it.
fn remote_at_https(st: &mut AppState) -> String {
    let url = "https://example.com/me/app.git".to_owned();
    let snap = st.snap.as_mut().unwrap();
    snap.remote_urls.iter_mut().filter(|(n, _)| n == "origin").for_each(|(_, u)| *u = url.clone());
    url
}

#[test]
fn a_copied_link_names_the_shared_branch_and_its_remote() {
    let (mut st, work, bare) = with_origin("link-copy");
    // A local branch under another name, tracking origin/feat-x.
    git(&work, &["branch", "feat-local"]);
    git(&work, &["push", "-q", "origin", "feat-local:feat-x"]);
    git(&work, &["branch", "-q", "--set-upstream-to=origin/feat-x", "feat-local"]);
    git(&work, &["branch", "solo"]);
    git(&work, &["tag", "v1.0"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    // The fixture's origin is a folder, which a link never carries: by root commit alone.
    assert!(url_of(&st, "origin").is_some());
    assert_eq!(copy(&mut st, LinkTarget::Repository { remote: Some("origin".into()) }).0, None);
    let origin = Some(remote_at_https(&mut st));
    assert_eq!(
        copy(&mut st, LinkTarget::LocalBranch("feat-local".into())),
        (origin.clone(), Target::Branch("feat-x".into()), "branch feat-x".into())
    );
    // Without an upstream: the preferred remote (the checked-out branch's upstream) and the
    // local name.
    assert_eq!(
        copy(&mut st, LinkTarget::LocalBranch("solo".into())),
        (origin.clone(), Target::Branch("solo".into()), "branch solo".into())
    );
    let oid = st.snap.as_ref().unwrap().head_oid.unwrap();
    let sha = oid.to_string();
    assert_eq!(copy(&mut st, LinkTarget::Commit(oid)), (origin.clone(), Target::Commit(sha.clone()), format!("commit {}", &sha[..7])));
    assert_eq!(copy(&mut st, LinkTarget::Tag("v1.0".into())), (origin.clone(), Target::Tag("v1.0".into()), "tag v1.0".into()));

    // Another remote: its own branches and its group menu use its URL.
    let snap = st.snap.as_mut().unwrap();
    snap.remote_urls.push(("upstream".into(), "https://example.com/team/app.git".into()));
    let up = Some("https://example.com/team/app.git".to_owned());
    let remote_x = LinkTarget::RemoteBranch { remote: "upstream".into(), name: "x".into() };
    assert_eq!(copy(&mut st, remote_x), (up.clone(), Target::Branch("x".into()), "branch x".into()));
    let repo_on = LinkTarget::Repository { remote: Some("upstream".into()) };
    assert_eq!(copy(&mut st, repo_on), (up.clone(), Target::Repository, "repository".into()));

    // Without the head's upstream: origin, else the first remote.
    let snap = st.snap.as_mut().unwrap();
    snap.locals.iter_mut().for_each(|b| b.tracking = None);
    snap.remote_urls.reverse();
    assert_eq!(copy(&mut st, LinkTarget::Repository { remote: None }).0, origin);
    st.snap.as_mut().unwrap().remote_urls.retain(|(n, _)| n != "origin");
    assert_eq!(copy(&mut st, LinkTarget::Repository { remote: None }).0, up);

    // No remotes: a link by root commit only.
    st.snap.as_mut().unwrap().remote_urls.clear();
    assert_eq!(copy(&mut st, LinkTarget::Repository { remote: None }).0, None);
    cleanup(&[&work, &bare]);
}

#[test]
fn the_built_link_is_copied_with_a_toast() {
    let (mut st, work, bare) = with_origin("link-built");
    remote_at_https(&mut st);
    // End to end through the runner: the link carries the URL and the root commit.
    let fx = send(&mut st, DeepLinkMsg::Copy(LinkTarget::LocalBranch("feature".into())));
    let built = crate::runner::testing::execute(fx.into_iter().next().unwrap()).unwrap();
    let fx = update(&mut st, built, Instant::now());
    let uri = match &fx[..] {
        [Effect::Ui(UiEffect::CopyText(uri))] => uri.clone(),
        other => panic!("expected the clipboard, got {other:?}"),
    };
    let parsed = link::parse(&uri).unwrap();
    assert_eq!(parsed.target, Target::Branch("feature".into()));
    assert!(parsed.url.is_some() && parsed.root.is_some(), "{uri}");
    assert_eq!(st.toasts.last().map(|t| (t.ok, t.text.as_str())), Some((true, "Copied link to branch feature")));

    let fx = send(
        &mut st,
        DeepLinkMsg::Built { what: "repository".into(), result: Err("this repository has no commits and no remote yet".into()) },
    );
    assert!(fx.is_empty(), "nothing is copied");
    let last = st.toasts.last().unwrap();
    assert!(!last.ok);
    assert_eq!(last.text, "Can't make a link: this repository has no commits and no remote yet.");
    cleanup(&[&work, &bare]);
}

fn find(fx: &[Effect]) -> (Vec<PathBuf>, u64, FindOrigin) {
    let found = fx.iter().find_map(|e| match e {
        Effect::DeepLink(DeepLinkEffect::Find { candidates, seq, origin, .. }) => Some((candidates.clone(), *seq, *origin)),
        _ => None,
    });
    found.unwrap_or_else(|| panic!("expected a Find, got {fx:?}"))
}

fn focuses(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Ui(UiEffect::FocusWindow)))
}

pub(super) fn link_text(target: &str) -> String {
    format!("glowfly://{target}?root={ROOT}")
}

#[test]
fn invalid_text_explains_itself() {
    let mut st = state();
    let fx = send(&mut st, DeepLinkMsg::Open("glowfly://draft/7?root=abc".into()));
    assert!(focuses(&fx));
    assert!(!fx.iter().any(|e| matches!(e, Effect::DeepLink(_))), "no search");
    match &st.modal {
        Some(Modal::DeepLink(LinkDialog::Invalid { text, reason, unknown_kind })) => {
            assert_eq!(text, "glowfly://draft/7?root=abc");
            assert!(reason.contains("draft"), "{reason}");
            assert_eq!(unknown_kind.as_deref(), Some("draft"), "a newer version may know it");
        }
        _ => panic!("expected the invalid-link dialog"),
    }
    assert!(st.deep_link.pending.is_none());
    send(&mut st, DeepLinkMsg::Open("not a link".into()));
    match &st.modal {
        Some(Modal::DeepLink(LinkDialog::Invalid { unknown_kind, .. })) => assert_eq!(*unknown_kind, None),
        _ => panic!("expected the invalid-link dialog"),
    }
}

#[test]
fn a_link_to_a_file_or_network_path_is_refused() {
    let with_url = |url: &str| Link { url: Some(url.into()), root: Some(ROOT.into()), target: Target::Repository }.to_uri();
    let paths = [
        r"\\host\share\r.git",
        "//host/share/r",
        r"\\?\UNC\host\share\r",
        r"\\.\C:\r",
        "file:///srv/r",
        "FiLe://host/r",
        r"C:\src\r",
        "/srv/r",
        "r",
    ];
    for path in paths {
        let mut st = state();
        let fx = send(&mut st, DeepLinkMsg::Open(with_url(path)));
        assert!(!fx.iter().any(|e| matches!(e, Effect::DeepLink(_))), "{path}: no search");
        assert!(st.deep_link.pending.is_none(), "{path}");
        match &st.modal {
            Some(Modal::DeepLink(LinkDialog::LocalPath { text })) => assert!(text.starts_with("glowfly://repo?url="), "{text}"),
            _ => panic!("{path}: expected the refusal"),
        }
        let logged = st.log.last().map(|l| (l.ok, l.text.as_str()));
        assert_eq!(logged, Some((false, st.settings.lang().tr("deep_link.local_path.log"))), "{path}");
    }
    for url in ["https://github.com/o/r.git", "ssh://git@host:22/o/r", "git@github.com:o/r.git"] {
        let mut st = state();
        let fx = send(&mut st, DeepLinkMsg::Open(with_url(url)));
        assert!(fx.iter().any(|e| matches!(e, Effect::DeepLink(DeepLinkEffect::Find { .. }))), "{url}: searched for");
        assert_eq!(st.deep_link.pending.as_ref().and_then(|p| p.link.url.as_deref()), Some(url));
    }
}

#[test]
fn a_link_is_looked_for_in_tabs_favorites_and_recent() {
    let base = glowfly_core::paths::temp_dir().join(format!("glowfly-link-candidates-{}", std::process::id()));
    let p = |n: &str| base.join(n);
    let mut st = state();
    st.tabs = vec![Tab::unloaded(p("other-tab")), Tab::unloaded(p("active"))];
    st.active_tab = 1;
    st.settings.favorite_repos = vec![glowfly_core::paths::encode(&p("fav")), glowfly_core::paths::encode(&p("active"))];
    st.recent = vec![p("recent"), p("gone"), p("fav")];
    st.missing_repos = vec![p("gone")];
    let fx = send(&mut st, DeepLinkMsg::Open(link_text("branch/feature%2Fx")));
    assert!(focuses(&fx));
    let (candidates, seq, origin) = find(&fx);
    assert_eq!(candidates, vec![p("active"), p("other-tab"), p("fav"), p("recent")]);
    assert_eq!((seq, origin), (st.deep_link.seq, FindOrigin::Search));
    let pending = st.deep_link.pending.as_ref().unwrap();
    assert_eq!(pending.link, Link { url: None, root: Some(ROOT.into()), target: Target::Branch("feature/x".into()) });
}

#[test]
fn an_os_link_opens_like_the_command_line() {
    let mut st = state();
    let fx = update(&mut st, crate::start_arg::StartArg::Link(link_text("tag/v1")).into_msg(), Instant::now());
    assert!(focuses(&fx));
    let (_, seq, origin) = find(&fx);
    assert_eq!((seq, origin), (st.deep_link.seq, FindOrigin::Search));
    assert_eq!(st.deep_link.pending.as_ref().map(|p| p.link.target.clone()), Some(Target::Tag("v1".into())));
}

#[test]
fn a_link_waits_while_another_dialog_is_open() {
    let mut st = state();
    st.modal = Some(Modal::Shortcuts { filter: String::new() });
    let text = link_text("repo");
    let fx = send(&mut st, DeepLinkMsg::Open(text.clone()));
    assert!(focuses(&fx));
    assert!(!fx.iter().any(|e| matches!(e, Effect::DeepLink(_))));
    assert!(matches!(st.modal, Some(Modal::Shortcuts { .. })), "the open dialog stays");
    let toast = st.toasts.last().unwrap();
    assert_eq!(toast.text, LINK_WAITING);
    let action = toast.action.as_ref().unwrap();
    assert_eq!(action.label, "Open link");
    assert!(matches!(&*action.msg, Msg::DeepLink(DeepLinkMsg::Open(t)) if *t == text));

    // Preferences counts as a dialog too.
    st.modal = None;
    st.prefs = Some(crate::state::PrefsState::new(PrefPage::General));
    let fx = send(&mut st, DeepLinkMsg::Open(text));
    assert!(!fx.iter().any(|e| matches!(e, Effect::DeepLink(_))));
}

#[test]
fn a_newer_link_wins() {
    let mut st = state();
    st.recent = vec![glowfly_core::paths::temp_dir().join("somewhere")];
    let (_, first, _) = find(&send(&mut st, DeepLinkMsg::Open(link_text("tag/v1"))));
    let (_, second, _) = find(&send(&mut st, DeepLinkMsg::Open(link_text("tag/v2"))));
    assert!(second > first);
    // The first search's answer is dropped: no dialog.
    send(&mut st, DeepLinkMsg::Found { seq: first, origin: FindOrigin::Search, result: Ok(Vec::new()) });
    assert!(st.modal.is_none());
    assert_eq!(st.deep_link.pending.as_ref().unwrap().link.target, Target::Tag("v2".into()));
    send(&mut st, DeepLinkMsg::Found { seq: second, origin: FindOrigin::Search, result: Ok(Vec::new()) });
    assert!(matches!(st.modal, Some(Modal::DeepLink(LinkDialog::NotFound { .. }))));
}

#[test]
fn preferences_read_the_scheme_status() {
    let mut st = state();
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::DeepLink(DeepLinkEffect::SchemeStatus))));
    assert_eq!(st.deep_link.scheme, SchemeView::Checking);
    send(&mut st, DeepLinkMsg::SchemeChecked(Ok(SchemeStatus::Registered)));
    assert_eq!(st.deep_link.scheme, SchemeView::Known(SchemeStatus::Registered));
    assert!(matches!(&send(&mut st, DeepLinkMsg::Register)[..], [Effect::DeepLink(DeepLinkEffect::Register)]));
    assert_eq!(st.deep_link.scheme, SchemeView::Checking);
    assert!(matches!(&send(&mut st, DeepLinkMsg::Unregister)[..], [Effect::DeepLink(DeepLinkEffect::Unregister)]));
    send(&mut st, DeepLinkMsg::SchemeChecked(Err("xdg-mime wasn't found".into())));
    assert_eq!(st.deep_link.scheme, SchemeView::Failed("xdg-mime wasn't found".into()));
    assert!(matches!(&send(&mut st, DeepLinkMsg::CheckScheme)[..], [Effect::DeepLink(DeepLinkEffect::SchemeStatus)]));
}

#[test]
fn the_palette_copies_links_to_the_repository_and_branch() {
    use crate::msg::PaletteMsg;
    use crate::state::palette::PaletteCommand;
    let has = |st: &AppState, label: &str| crate::state::palette_commands::entries(st).iter().any(|e| e.label == label);
    let dir = super::tests::scratch_repo("link-palette");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(has(&st, "Copy link to repository"));
    assert!(has(&st, "Copy link to branch main"));
    let run = PaletteMsg::RunPaletteCommand(PaletteCommand::CopyLink(LinkTarget::LocalBranch("main".into())));
    let fx = update(&mut st, Msg::Palette(run), Instant::now());
    assert!(
        fx.iter().any(|e| matches!(e, Effect::DeepLink(DeepLinkEffect::Build { url: None, target: Target::Branch(b), .. }) if b == "main"))
    );
    assert_eq!(PaletteCommand::CopyLink(LinkTarget::Repository { remote: None }).recall_key(), Some("copy-repo-link"));
    assert_eq!(PaletteCommand::CopyLink(LinkTarget::LocalBranch("main".into())).recall_key(), None);

    // No commits and no remotes: nothing a link could name.
    let empty = glowfly_core::paths::temp_dir().join(format!("glowfly-app-link-palette-empty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&empty);
    std::fs::create_dir_all(&empty).unwrap();
    git(&empty, &["init", "-q", "-b", "main"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(empty.clone())));
    assert!(st.snap.is_some());
    assert!(!has(&st, "Copy link to repository"));
    assert!(!has(&st, "Copy link to branch main"));
    cleanup(&[&dir, &empty]);
}

#[test]
fn a_copied_link_leaves_out_credentials() {
    let (mut st, work, bare) = with_origin("link-secret");
    let snap = st.snap.as_mut().unwrap();
    snap.remote_urls = vec![
        ("origin".into(), "https://me:ghp_SECRET@github.com/o/r.git".into()),
        ("mirror".into(), "ssh://git:pw@example.com/o/r".into()),
    ];
    let oid = snap.head_oid.unwrap();
    assert_eq!(copy(&mut st, LinkTarget::Commit(oid)).0.as_deref(), Some("https://github.com/o/r.git"));
    assert_eq!(copy(&mut st, LinkTarget::Tag("v1".into())).0.as_deref(), Some("https://github.com/o/r.git"));
    let mirror = LinkTarget::Repository { remote: Some("mirror".into()) };
    assert_eq!(copy(&mut st, mirror).0.as_deref(), Some("ssh://git@example.com/o/r"));
    cleanup(&[&work, &bare]);
}
