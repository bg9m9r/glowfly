//! Headless tests for the issue panel's comment box: posting the draft and why it can't be
//! posted, the answer appended or the draft kept, answers that find no panel or a reopened one,
//! one for a parked tab, a reload in flight, and Reply quoting a comment into the draft.

use super::issue_tests::{details, done, err, github, jobs, open_repo, reply, run};
use super::update;
use crate::effect::{ForgeError, IssueCall, IssueReply};
use crate::msg::{IssueMsg, Msg, TabsMsg};
use crate::state::AppState;
use crate::state::issue::{Details, IssueView};
use crate::state::issue_comment::{FIELD, POSTING, comment_blocked};
use glowfly_core::forge::{Auth, Entry, EntryKind, Provider, Target};
use glowfly_core::settings::RepoTrust;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn view(st: &AppState) -> &IssueView {
    st.session.issue.view.as_ref().expect("the panel is open")
}

fn view_mut(st: &mut AppState) -> &mut IssueView {
    st.session.issue.view.as_mut().expect("the panel is open")
}

/// Issue 12's panel with its details loaded.
fn loaded_panel(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = open_repo(name);
    run(&mut st, IssueMsg::Open("12".into()));
    let msg = reply(&st, IssueReply::Loaded { key: "12".into(), result: Ok(details("12")) });
    run(&mut st, msg);
    (st, dir)
}

fn posted(body: &str) -> Entry {
    Entry { author: "me".into(), time: 1_767_607_200, kind: EntryKind::Comment, body: body.into(), url: String::new() }
}

/// A comment's answer for issue `key` from the repository at `workdir`'s GitHub tracker.
fn commented(workdir: &Path, generation: u64, key: &str, result: Result<Entry, ForgeError>) -> IssueMsg {
    let reply = IssueReply::Commented { key: key.into(), result };
    IssueMsg::Replied { workdir: workdir.to_owned(), target: github(), generation, reply: Box::new(reply) }
}

/// Feeds `st` the answer for issue `key` from the repository at `workdir`.
fn answer(st: &mut AppState, workdir: &Path, key: &str, result: Result<Entry, ForgeError>) -> Vec<crate::effect::Effect> {
    let msg = commented(workdir, st.session.issue.generation, key, result);
    run(st, msg)
}

fn post(st: &mut AppState, text: &str) -> Vec<crate::effect::Effect> {
    view_mut(st).comment.text = text.into();
    run(st, IssueMsg::PostComment)
}

#[test]
fn post_sends_the_draft() {
    let (mut st, dir) = loaded_panel("issue-comment-post");
    let fx = post(&mut st, "Thanks");
    let j = jobs(&fx);
    assert_eq!(j.len(), 1);
    assert!(matches!(&j[0].call, IssueCall::Comment { key, body } if key == "12" && body == "Thanks"), "{:?}", j[0].call);
    assert_eq!((&j[0].req.target, &j[0].req.workdir), (&github(), &st.workdir().unwrap()));
    assert!(view(&st).comment.posting);
    assert!(jobs(&run(&mut st, IssueMsg::PostComment)).is_empty(), "a second click sends nothing");
    done(st, dir);
}

#[test]
fn post_is_blocked_with_reasons() {
    let (mut st, dir) = open_repo("issue-comment-blocked");
    run(&mut st, IssueMsg::Open("12".into()));
    assert!(jobs(&post(&mut st, "Thanks")).is_empty(), "not loaded yet");
    assert_eq!(comment_blocked(view(&st), true).as_deref(), Some("The issue hasn't loaded yet"));
    let msg = reply(&st, IssueReply::Loaded { key: "12".into(), result: Ok(details("12")) });
    run(&mut st, msg);

    assert!(jobs(&post(&mut st, "  \n")).is_empty(), "blank");
    assert!(view(&st).comment.touched.shows(FIELD), "the try shows the field's problem");
    assert_eq!(comment_blocked(view(&st), true).as_deref(), Some("Write a comment first"));
    assert!(jobs(&post(&mut st, &"x".repeat(65_537))).is_empty(), "too long");
    assert_eq!(comment_blocked(view(&st), true).as_deref(), Some("Comments on GitHub can have at most 65,536 characters"));

    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    crate::state::trust::record(&mut st.settings, &real, RepoTrust::NotTrusted);
    assert!(jobs(&post(&mut st, "Thanks")).is_empty(), "untrusted");
    assert_eq!(comment_blocked(view(&st), st.runs_git_unasked()).as_deref(), Some("Trust this repository to comment"));
    crate::state::trust::record(&mut st.settings, &real, RepoTrust::Trusted);

    view_mut(&mut st).comment.posting = true;
    assert!(jobs(&post(&mut st, "Thanks")).is_empty(), "posting");
    assert_eq!(comment_blocked(view(&st), true).as_deref(), Some(POSTING));
    view_mut(&mut st).comment.posting = false;

    // Bitbucket Data Center has no tracker: its panel never loads.
    run(&mut st, IssueMsg::Close);
    let dc = Target { provider: Provider::BitbucketDataCenter, host: "bitbucket.acme.io".into(), ..github() };
    st.forge.as_mut().unwrap().tracker = Some(dc);
    run(&mut st, IssueMsg::Open("12".into()));
    assert_eq!(view(&st).details, Details::Unsupported);
    assert!(jobs(&post(&mut st, "Thanks")).is_empty(), "no tracker");
    done(st, dir);
}

#[test]
fn a_posted_comment_is_appended() {
    let (mut st, dir) = loaded_panel("issue-comment-ok");
    post(&mut st, "Thanks");
    let workdir = st.workdir().unwrap();
    answer(&mut st, &workdir, "12", Ok(posted("Thanks")));
    let v = view(&st);
    assert_eq!(v.loaded().unwrap().comments.last(), Some(&posted("Thanks")));
    assert_eq!((v.comment.text.as_str(), v.comment.posting, v.comment.error.as_deref()), ("", false, None));
    assert!(!v.comment.touched.shows(FIELD), "the touched state is reset");
    assert!(st.toasts.iter().any(|t| t.ok && t.text == "Comment posted"));
    assert!(st.log.last().is_some_and(|l| l.ok && l.text == "Commented on issue #12"));
    done(st, dir);
}

#[test]
fn a_failed_post_keeps_the_draft() {
    let (mut st, dir) = loaded_panel("issue-comment-failed");
    post(&mut st, "Me too");
    let locked = "Unable to create comment because issue is locked.";
    let workdir = st.workdir().unwrap();
    answer(&mut st, &workdir, "12", Err(err(locked)));
    let v = view(&st);
    assert_eq!((v.comment.text.as_str(), v.comment.posting, v.comment.error.as_deref()), ("Me too", false, Some(locked)));
    assert!(v.loaded().unwrap().comments.is_empty());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == format!("Commenting on issue #12 failed: {locked}")));

    let key = github().account().key();
    st.forge.as_mut().unwrap().auth.insert(key.clone(), Auth::from_token(Provider::GitHub, None, "tok", "test"));
    post(&mut st, "Me too");
    answer(&mut st, &workdir, "12", Err(ForgeError { message: "Bad credentials".into(), auth: true }));
    assert!(!st.forge.as_ref().unwrap().auth.contains_key(&key), "the credential is forgotten");
    done(st, dir);
}

#[test]
fn replies_without_their_panel_are_logged_and_toasted() {
    let (mut st, dir) = loaded_panel("issue-comment-closed");
    let workdir = st.workdir().unwrap();
    post(&mut st, "Thanks");
    run(&mut st, IssueMsg::Close);
    answer(&mut st, &workdir, "12", Ok(posted("Thanks")));
    assert!(st.session.issue.view.is_none());
    assert!(st.toasts.iter().any(|t| t.ok && t.text == "Commented on issue #12"));
    answer(&mut st, &workdir, "12", Err(err("timed out")));
    assert!(st.toasts.iter().any(|t| !t.ok && t.text == "Couldn't post the comment on #12: timed out"));

    // Another issue opened meanwhile is left alone.
    run(&mut st, IssueMsg::Open("14".into()));
    let msg = reply(&st, IssueReply::Loaded { key: "14".into(), result: Ok(details("14")) });
    run(&mut st, msg);
    view_mut(&mut st).comment.text = "Draft for 14".into();
    answer(&mut st, &workdir, "12", Ok(posted("Thanks")));
    let v = view(&st);
    assert_eq!((v.key.as_str(), v.comment.text.as_str()), ("14", "Draft for 14"));
    assert!(v.loaded().unwrap().comments.is_empty());

    // A reload doesn't make a comment's answer stale.
    post(&mut st, "Seen");
    let old = st.session.issue.generation;
    run(&mut st, IssueMsg::Reload);
    assert!(st.session.issue.generation > old);
    run(
        &mut st,
        IssueMsg::Replied {
            workdir: workdir.clone(),
            target: github(),
            generation: old,
            reply: Box::new(IssueReply::Commented { key: "14".into(), result: Ok(posted("Seen")) }),
        },
    );
    assert_eq!(view(&st).loaded().unwrap().comments.last(), Some(&posted("Seen")));
    done(st, dir);
}

#[test]
fn a_reply_leaves_a_reopened_panel_alone() {
    let (mut st, dir) = loaded_panel("issue-comment-reopened");
    let workdir = st.workdir().unwrap();
    post(&mut st, "Thanks");
    run(&mut st, IssueMsg::Close);
    run(&mut st, IssueMsg::Open("12".into()));
    let msg = reply(&st, IssueReply::Loaded { key: "12".into(), result: Ok(details("12")) });
    run(&mut st, msg);
    if let Details::Loaded(d) = &mut view_mut(&mut st).details {
        d.comments.push(posted("Thanks"));
    }
    view_mut(&mut st).comment.text = "New draft".into();
    answer(&mut st, &workdir, "12", Ok(posted("Thanks")));
    let v = view(&st);
    assert_eq!(v.comment.text, "New draft", "the reopened panel's draft is kept");
    assert_eq!(v.loaded().unwrap().comments, vec![posted("Thanks")], "the comment isn't shown twice");
    assert!(st.toasts.iter().any(|t| t.ok && t.text == "Commented on issue #12"));

    answer(&mut st, &workdir, "12", Err(err("timed out")));
    let v = view(&st);
    assert_eq!((v.comment.text.as_str(), v.comment.error.as_deref()), ("New draft", None));
    assert!(st.toasts.iter().any(|t| !t.ok && t.text == "Couldn't post the comment on #12: timed out"));
    done(st, dir);
}

#[test]
fn a_reply_reaches_the_parked_tab() {
    let (mut st, dir) = loaded_panel("issue-comment-parked");
    let workdir = st.workdir().unwrap();
    post(&mut st, "Thanks");
    update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    assert!(st.session.issue.view.is_none());
    answer(&mut st, &workdir, "12", Ok(posted("Thanks")));
    let parked = st.tabs[0].parked.as_ref().and_then(|p| p.session.issue.view.as_ref()).expect("the parked panel");
    assert_eq!(parked.loaded().unwrap().comments.last(), Some(&posted("Thanks")));
    assert_eq!((parked.comment.text.as_str(), parked.comment.posting), ("", false));
    assert!(st.tabs[0].log.last().is_some_and(|l| l.ok && l.text == "Commented on issue #12"), "logged in that tab");
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert_eq!(view(&st).loaded().unwrap().comments.last(), Some(&posted("Thanks")));

    post(&mut st, "Again");
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)), Instant::now());
    answer(&mut st, &workdir, "12", Err(err("timed out")));
    let parked = st.tabs[0].parked.as_ref().and_then(|p| p.session.issue.view.as_ref()).expect("the parked panel");
    assert_eq!((parked.comment.text.as_str(), parked.comment.posting), ("Again", false));
    assert_eq!(parked.comment.error.as_deref(), Some("timed out"));
    done(st, dir);
}

#[test]
fn success_during_a_reload_loads_again() {
    let (mut st, dir) = loaded_panel("issue-comment-reload");
    run(&mut st, IssueMsg::Reload);
    assert!(view(&st).reloading);
    post(&mut st, "Thanks");
    let before = st.session.issue.generation;
    let workdir = st.workdir().unwrap();
    let fx = answer(&mut st, &workdir, "12", Ok(posted("Thanks")));
    let loads: Vec<_> = jobs(&fx).into_iter().filter(|j| matches!(j.call, IssueCall::Load(_))).collect();
    assert_eq!(loads.len(), 1);
    assert!(loads[0].generation == st.session.issue.generation && st.session.issue.generation > before, "the older answer is now stale");
    done(st, dir);
}

#[test]
fn quote_fills_the_draft_and_focuses() {
    let (mut st, dir) = loaded_panel("issue-comment-quote");
    let comment = |author: &str, body: &str| Entry { author: author.into(), body: body.into(), ..posted("") };
    if let Details::Loaded(d) = &mut view_mut(&mut st).details {
        d.comments = vec![comment("ada", "I can reproduce"), comment("linus", "Fixed\non branch")];
    }
    run(&mut st, IssueMsg::Quote(1));
    assert_eq!(view(&st).comment.text, "linus wrote:\n> Fixed\n> on branch\n\n");
    assert!(st.view.issue_comment_focus);
    view_mut(&mut st).comment.text = "Agree ".into();
    run(&mut st, IssueMsg::Quote(0));
    assert_eq!(view(&st).comment.text, "Agree\n\nada wrote:\n> I can reproduce\n\n");

    st.view.issue_comment_focus = false;
    run(&mut st, IssueMsg::Quote(9));
    view_mut(&mut st).comment.posting = true;
    run(&mut st, IssueMsg::Quote(0));
    assert_eq!(view(&st).comment.text, "Agree\n\nada wrote:\n> I can reproduce\n\n", "out of range and while posting");
    assert!(!st.view.issue_comment_focus);
    done(st, dir);
}
