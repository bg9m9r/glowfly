//! `push_target`'s pure rules: which remote a plain push picks, `push.default`'s answer for each
//! mode against the same or another remote and upstream name, and refspec mapping.

use super::refspecs::{Spec, push_dest, tracking_ref};
use super::*;

fn upstream(remote: &str, merge: &str) -> Tracking {
    Tracking { remote: remote.into(), merge: merge.into() }
}

fn cfg(default: Option<&str>, branch: Option<&str>) -> PushRemotes {
    PushRemotes {
        default: default.map(str::to_owned),
        branches: branch.map(|r| ("main".to_owned(), r.to_owned())).into_iter().collect(),
        mode: PushMode::Simple,
    }
}

#[test]
fn choose_remote_follows_gits_order() {
    let up = upstream("origin", "main");
    let remotes = ["origin", "fork"];
    let pick = |c: &PushRemotes| choose_remote(c, "main", &up, &remotes);
    assert_eq!(pick(&cfg(None, None)), Chosen { remote: "origin".into(), key: None, missing: None });
    assert_eq!(pick(&cfg(Some("fork"), None)), Chosen { remote: "fork".into(), key: Some("remote.pushDefault".into()), missing: None });
    let branch = pick(&cfg(Some("fork"), Some("origin")));
    assert_eq!(branch, Chosen { remote: "origin".into(), key: Some("branch.main.pushRemote".into()), missing: None });
}

#[test]
fn choose_remote_keeps_urls_and_passes_over_missing_names() {
    let up = upstream("origin", "main");
    for kept in ["git@host:me/r.git", "https://example.com/r.git", "../fork", ".", r"..\fork"] {
        let c = choose_remote(&cfg(Some(kept), None), "main", &up, &["origin"]);
        assert_eq!((c.remote.as_str(), c.missing), (kept, None), "{kept}");
    }
    let c = choose_remote(&cfg(None, Some("gone")), "main", &up, &["origin"]);
    assert_eq!(c, Chosen { remote: "origin".into(), key: None, missing: Some(("branch.main.pushRemote".into(), "gone".into())) });
}

#[test]
fn by_mode_table() {
    use PushMode as M;
    use PushRefusal as R;
    let main = "refs/heads/main".to_owned();
    let other = "refs/heads/other".to_owned();
    // (mode, push remote, upstream merge, answer), the upstream always on origin.
    let rows: Vec<(M, &str, &str, Result<String, R>)> = vec![
        (M::Nothing, "origin", "main", Err(R::Nothing)),
        (M::Nothing, "fork", "main", Err(R::Nothing)),
        (M::Current, "origin", "main", Ok(main.clone())),
        (M::Current, "origin", "other", Ok(main.clone())),
        (M::Current, "fork", "other", Ok(main.clone())),
        (M::Upstream, "origin", "main", Ok(main.clone())),
        (M::Upstream, "origin", "other", Ok(other.clone())),
        (M::Upstream, "fork", "main", Err(R::NotUpstreamRemote { upstream_remote: "origin".into() })),
        (M::Upstream, "fork", "other", Err(R::NotUpstreamRemote { upstream_remote: "origin".into() })),
        (M::Simple, "origin", "main", Ok(main.clone())),
        (M::Simple, "origin", "other", Err(R::NameDiffers { merge: "other".into() })),
        (M::Simple, "fork", "main", Ok(main.clone())),
        (M::Simple, "fork", "other", Ok(main.clone())),
        (M::Matching, "origin", "other", Ok(main.clone())),
        (M::Matching, "fork", "main", Ok(main.clone())),
        (M::Invalid("x".into()), "origin", "main", Err(R::Invalid("x".into()))),
        (M::Invalid(String::new()), "fork", "other", Err(R::Invalid(String::new()))),
    ];
    for (mode, remote, merge, answer) in rows {
        assert_eq!(by_mode(&mode, "main", remote, &upstream("origin", merge)), answer, "{mode:?} to {remote}, upstream {merge}");
    }
}

#[test]
fn refspecs_map_names_as_git_does() {
    let push = [Spec::new("refs/heads/main", Some("refs/heads/review")), Spec::new("refs/heads/feat/*", Some("refs/heads/f/*"))];
    assert_eq!(push_dest(&push, "refs/heads/main", false).as_deref(), Some("refs/heads/review"));
    assert_eq!(push_dest(&push, "refs/heads/feat/x", false).as_deref(), Some("refs/heads/f/x"));
    assert_eq!(push_dest(&push, "refs/heads/other", true), None);
    let short = [Spec::new("main", Some("review")), Spec::new("topic", None)];
    assert_eq!(push_dest(&short, "refs/heads/main", false).as_deref(), Some("refs/heads/review"), "short names are branches");
    assert_eq!(push_dest(&short, "refs/heads/topic", false).as_deref(), Some("refs/heads/topic"), "no destination: the same ref");
    let head = [Spec::new("HEAD", Some("refs/for/main"))];
    assert_eq!(push_dest(&head, "refs/heads/x", true).as_deref(), Some("refs/for/main"));
    assert_eq!(push_dest(&head, "refs/heads/x", false), None);
    let fetch = [Spec::new("refs/heads/*", Some("refs/remotes/fork/*"))];
    assert_eq!(tracking_ref(&fetch, "refs/heads/main").as_deref(), Some("refs/remotes/fork/main"));
    assert_eq!(tracking_ref(&fetch, "refs/for/main"), None);
}

#[test]
fn labels_name_where_the_push_lands() {
    let target = |remote: &str| PushTarget { remote: remote.into(), key: None, missing: None, dest: Err(PushRefusal::Nothing) };
    let dest = |r: &str, tracking: Option<&str>| PushDest {
        dest_ref: r.into(),
        tracking_ref: tracking.map(str::to_owned),
        tip: None,
        ahead: 0,
        behind: 0,
        rule: PushRule::Default,
    };
    assert_eq!(target("fork").label(&dest("refs/heads/main", Some("refs/remotes/fork/main"))), "fork/main");
    assert_eq!(target("fork").label(&dest("refs/heads/main", None)), "fork/main", "not fetched yet");
    assert_eq!(target(".").label(&dest("refs/heads/main", Some("refs/heads/main"))), "main");
    assert_eq!(target("origin").label(&dest("refs/for/main", None)), "origin refs/for/main");
    assert_eq!(target("git@host:me/r.git").label(&dest("refs/heads/main", None)), "git@host:me/r.git");
}
