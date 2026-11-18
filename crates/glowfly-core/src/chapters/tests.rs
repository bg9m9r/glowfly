//! `chapters::group` over fixture repositories: merge, squash and rebase-landed histories, the
//! run rules, release tags, and what a longer load changes.

use super::*;
use crate::repo::{LoadOptions, load};
use crate::testkit::{git, git_env};
use crate::testutil::scratch_repo;
use std::path::Path;

const H: i64 = 3600;
const T0: i64 = 1_700_000_000;

/// The name and email of the test author `who`.
fn person(who: &str) -> (String, String) {
    (who.to_uppercase(), format!("{who}@example.com"))
}

/// An empty commit by `who` at `secs`, with an optional body.
fn put(dir: &Path, who: &str, secs: i64, msg: &str, body: Option<&str>) {
    let (name, email) = person(who);
    let date = format!("@{secs} +0000");
    let env = [
        ("GIT_AUTHOR_NAME", name.as_str()),
        ("GIT_AUTHOR_EMAIL", email.as_str()),
        ("GIT_COMMITTER_NAME", name.as_str()),
        ("GIT_COMMITTER_EMAIL", email.as_str()),
        ("GIT_AUTHOR_DATE", date.as_str()),
        ("GIT_COMMITTER_DATE", date.as_str()),
    ];
    let mut args = vec!["commit", "-q", "--allow-empty", "-m", msg];
    if let Some(b) = body {
        args.extend(["-m", b]);
    }
    git_env(dir, &args, &env);
}

/// `git merge --no-ff <branch> -m <msg>` by `who` at `secs`.
fn merge(dir: &Path, who: &str, secs: i64, branches: &[&str], msg: &str) {
    let (name, email) = person(who);
    let date = format!("@{secs} +0000");
    let env = [
        ("GIT_AUTHOR_NAME", name.as_str()),
        ("GIT_AUTHOR_EMAIL", email.as_str()),
        ("GIT_COMMITTER_NAME", name.as_str()),
        ("GIT_COMMITTER_EMAIL", email.as_str()),
        ("GIT_AUTHOR_DATE", date.as_str()),
        ("GIT_COMMITTER_DATE", date.as_str()),
    ];
    let mut args = vec!["merge", "--no-ff", "-q", "-m", msg];
    args.extend(branches);
    git_env(dir, &args, &env);
}

/// A linear history of `(who, seconds after T0, summary, body)` commits, oldest first.
fn linear(name: &str, commits: &[(&str, i64, &str, &str)]) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    for &(who, at, msg, body) in commits {
        put(&dir, who, T0 + at, msg, (!body.is_empty()).then_some(body));
    }
    dir
}

fn snap_of(dir: &Path, n: usize) -> Snapshot {
    load(dir, &LoadOptions::all(n)).unwrap()
}

fn chapters_of(dir: &Path) -> (Snapshot, Chapters) {
    let snap = snap_of(dir, 500);
    let ch = group(&History::of(&snap));
    (snap, ch)
}

fn row(s: &Snapshot, summary: &str) -> u32 {
    s.commits.iter().position(|c| &*c.summary == summary).unwrap_or_else(|| panic!("no commit {summary}")) as u32
}

fn titles(c: &Chapters) -> Vec<&str> {
    c.list.iter().map(|c| c.title.as_str()).collect()
}

fn kinds(c: &Chapters) -> Vec<Kind> {
    c.list.iter().map(|c| c.kind).collect()
}

fn names(ch: &Chapter) -> Vec<&str> {
    ch.authors.iter().map(|a| a.name()).collect()
}

#[test]
fn merge_is_one_chapter_with_what_it_brought_in() {
    let dir = scratch_repo("chapters-merge");
    put(&dir, "ada", T0, "a", None);
    git(&dir, &["checkout", "-q", "-b", "side"]);
    put(&dir, "bob", T0 + 2000, "s1", None);
    put(&dir, "bob", T0 + 3000, "s2", None);
    git(&dir, &["checkout", "-q", "main"]);
    put(&dir, "ada", T0 + 4000, "b", None);
    merge(&dir, "ada", T0 + 5000, &["side"], "Merge branch 'side'");
    let (snap, c) = chapters_of(&dir);
    assert_eq!(kinds(&c), [Kind::Merge, Kind::Run]);
    let m = &c.list[0];
    assert_eq!(m.title, "side");
    assert_eq!(m.line, [0]);
    assert_eq!(m.side, [row(&snap, "s2"), row(&snap, "s1")]);
    assert_eq!(m.count(), 3);
    assert_eq!(names(m), ["ADA", "BOB"]);
    assert_eq!((m.oldest, m.newest), (T0 + 2000, T0 + 5000));
    assert!(!m.partial);
    assert_eq!(m.inside(), &m.side[..]);
    let run = &c.list[1];
    assert_eq!(run.line, [row(&snap, "b"), row(&snap, "a")]);
    assert_eq!(run.title, "a");
    assert!(!c.open_end);
}

#[test]
fn a_written_merge_subject_is_the_title() {
    let dir = scratch_repo("chapters-merge-title");
    put(&dir, "ada", T0, "a", None);
    git(&dir, &["checkout", "-q", "-b", "side"]);
    put(&dir, "bob", T0 + 100, "s1", None);
    git(&dir, &["checkout", "-q", "main"]);
    put(&dir, "ada", T0 + 200, "b", None);
    merge(&dir, "ada", T0 + 300, &["side"], "Land the importer");
    let (_, c) = chapters_of(&dir);
    assert_eq!(c.list[0].title, "Land the importer");
}

#[test]
fn an_octopus_merge_counts_every_other_parents_commits() {
    let dir = scratch_repo("chapters-octopus");
    put(&dir, "ada", T0, "base", None);
    for (i, b) in ["one", "two", "three"].into_iter().enumerate() {
        git(&dir, &["checkout", "-q", "-b", b, "main"]);
        put(&dir, "bob", T0 + 100 + i as i64, &format!("{b}-commit"), None);
    }
    git(&dir, &["checkout", "-q", "main"]);
    put(&dir, "ada", T0 + 500, "main-commit", None);
    merge(&dir, "ada", T0 + 600, &["one", "two", "three"], "Merge branches 'one', 'two' and 'three'");
    let (snap, c) = chapters_of(&dir);
    let m = &c.list[0];
    assert_eq!(m.kind, Kind::Merge);
    assert_eq!(m.title, "one, two, three");
    assert_eq!(m.side.len(), 3);
    for b in ["one", "two", "three"] {
        assert!(m.side.contains(&row(&snap, &format!("{b}-commit"))));
    }
}

#[test]
fn a_squash_landing_is_a_chapter_of_one() {
    let dir =
        linear("chapters-squash", &[("ada", 0, "Add importer (#12)", ""), ("ada", 60, "Fix typo", ""), ("ada", 120, "Tidy (#13)", "")]);
    let (_, c) = chapters_of(&dir);
    assert_eq!(kinds(&c), [Kind::Squash, Kind::Run, Kind::Squash]);
    assert_eq!(titles(&c), ["Tidy (#13)", "Fix typo", "Add importer (#12)"]);
    assert!(c.list.iter().all(|c| c.count() == 1 && c.inside().is_empty()));
}

#[test]
fn runs_split_on_author_gap_and_closing_line() {
    let dir = linear(
        "chapters-runs",
        &[
            ("ada", 0, "a1", ""),
            ("ada", H, "a2", ""),
            ("bob", 2 * H, "g1", ""),
            ("ada", 3 * H, "a3", ""),
            ("ada", 12 * H, "a4", ""),
            ("ada", 13 * H, "a5", "Closes #7"),
            ("ada", 14 * H, "a6", ""),
        ],
    );
    let (snap, c) = chapters_of(&dir);
    assert_eq!(titles(&c), ["a6", "a5", "a3", "g1", "a1"]);
    assert_eq!(c.list[1].line, [row(&snap, "a5"), row(&snap, "a4")]);
    assert_eq!(c.list[4].line, [row(&snap, "a2"), row(&snap, "a1")]);
    assert_eq!(c.list[4].inside().len(), 2);
}

#[test]
fn the_gap_is_eight_hours_inclusive() {
    let together = linear("chapters-gap-eq", &[("ada", 0, "x1", ""), ("ada", 8 * H, "x2", "")]);
    assert_eq!(chapters_of(&together).1.list.len(), 1);
    let apart = linear("chapters-gap-over", &[("ada", 0, "x1", ""), ("ada", 8 * H + 1, "x2", "")]);
    assert_eq!(chapters_of(&apart).1.list.len(), 2);
}

#[test]
fn closing_words_end_a_chapter_but_a_mention_does_not() {
    let fixes = linear("chapters-fixes", &[("ada", 0, "y1", "Fixes #3"), ("ada", 60, "y2", "")]);
    assert_eq!(titles(&chapters_of(&fixes).1), ["y2", "y1"]);
    let newest = linear("chapters-fixes-newest", &[("ada", 0, "y1", ""), ("ada", 60, "y2", "Fixes #3")]);
    assert_eq!(titles(&chapters_of(&newest).1), ["y2"], "the closing commit heads its chapter and titles it");
    let mention = linear("chapters-mention", &[("ada", 0, "y1", "Related to #5"), ("ada", 60, "y2", "")]);
    assert_eq!(titles(&chapters_of(&mention).1), ["y1"]);
}

#[test]
fn rebase_landed_linear_history() {
    let mut batches = Vec::new();
    for (i, who) in ["ada", "ada", "ada", "bob", "bob", "bob"].into_iter().enumerate() {
        batches.push((who, 100, ["p1", "p2", "p3", "q1", "q2", "q3"][i], ""));
    }
    let dir = linear("chapters-rebase-authors", &batches);
    let (_, c) = chapters_of(&dir);
    assert_eq!(titles(&c), ["q1", "p1"]);
    assert_eq!(c.list.iter().map(|c| c.count()).collect::<Vec<_>>(), [3, 3]);

    let mut batches = Vec::new();
    for (i, name) in ["p1", "p2", "p3", "q1", "q2", "q3"].into_iter().enumerate() {
        batches.push(("ada", if i < 3 { 100 } else { 100 + 9 * H }, name, ""));
    }
    let dir = linear("chapters-rebase-gap", &batches);
    let (_, c) = chapters_of(&dir);
    assert_eq!(titles(&c), ["q1", "p1"]);
    assert_eq!(c.list.iter().map(|c| c.count()).collect::<Vec<_>>(), [3, 3]);
}

#[test]
fn an_empty_repository_has_no_chapters() {
    let dir = scratch_repo("chapters-empty");
    let (_, c) = chapters_of(&dir);
    assert_eq!(c, Chapters::default());
}

#[test]
fn a_single_commit_is_one_chapter() {
    let dir = linear("chapters-single", &[("ada", 0, "only", "")]);
    let (_, c) = chapters_of(&dir);
    assert_eq!(kinds(&c), [Kind::Run]);
    assert_eq!(c.list[0].count(), 1);
    assert!(!c.open_end);
}

#[test]
fn an_unborn_branch_has_no_chapters() {
    let dir = linear("chapters-unborn", &[("ada", 0, "one", ""), ("ada", 60, "two", "")]);
    git(&dir, &["checkout", "-q", "--orphan", "fresh"]);
    let (snap, c) = chapters_of(&dir);
    assert!(snap.head_oid.is_none());
    assert!(!snap.commits.is_empty());
    assert!(c.list.is_empty());
}

#[test]
fn a_detached_head_follows_head() {
    let dir = linear("chapters-detached", &[("ada", 0, "one", ""), ("bob", 60, "two", ""), ("ada", 120, "three", "")]);
    git(&dir, &["checkout", "-q", "--detach", "HEAD~1"]);
    let (snap, c) = chapters_of(&dir);
    assert_eq!(snap.head_oid, Some(snap.commits[row(&snap, "two") as usize].oid));
    assert_eq!(titles(&c), ["two", "one"]);
}

#[test]
fn release_tags_head_their_chapter() {
    let dir = linear(
        "chapters-release",
        &[("ada", 0, "c1", ""), ("ada", 60, "c2", ""), ("ada", 120, "c3", ""), ("ada", 180, "c4", ""), ("ada", 240, "c5", "")],
    );
    git(&dir, &["tag", "v1.0.0", "HEAD~2"]);
    git(&dir, &["tag", "nightly", "HEAD~3"]);
    let (snap, c) = chapters_of(&dir);
    assert_eq!(titles(&c), ["c4", "c1"], "split at the tagged commit; nightly splits nothing");
    assert!(c.list[0].releases.is_empty());
    assert_eq!(c.list[1].releases, ["v1.0.0"]);
    assert_eq!(c.list[1].line[0], row(&snap, "c3"));

    git(&dir, &["tag", "-a", "-m", "second", "v1.0.0-rc.1", "HEAD~2"]);
    git(&dir, &["tag", "1.0", "HEAD~2"]);
    let (_, c) = chapters_of(&dir);
    assert_eq!(c.list[1].releases, ["1.0", "v1.0.0", "v1.0.0-rc.1"]);
}

#[test]
fn a_release_tag_on_a_merged_in_commit_gives_no_heading() {
    let dir = scratch_repo("chapters-release-side");
    put(&dir, "ada", T0, "a", None);
    git(&dir, &["checkout", "-q", "-b", "side"]);
    put(&dir, "bob", T0 + 100, "s1", None);
    git(&dir, &["tag", "v2.0.0"]);
    git(&dir, &["checkout", "-q", "main"]);
    put(&dir, "ada", T0 + 200, "b", None);
    merge(&dir, "ada", T0 + 300, &["side"], "Merge branch 'side'");
    let (_, c) = chapters_of(&dir);
    assert!(c.list.iter().all(|c| c.releases.is_empty()));
}

/// A history of about 40 commits mixing merges, squash landings, runs with gaps, closing lines
/// and a release tag.
fn mixed(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    let mut t = T0;
    for i in 0..30 {
        t += 600;
        match i % 7 {
            3 => {
                let b = format!("topic{i}");
                git(&dir, &["checkout", "-q", "-b", &b]);
                for k in 0..2 {
                    t += 30;
                    put(&dir, "bob", t, &format!("{b}-{k}"), None);
                }
                git(&dir, &["checkout", "-q", "main"]);
                t += 30;
                merge(&dir, "ada", t, &[&b], &format!("Merge branch '{b}'"));
            }
            5 => put(&dir, "ada", t, &format!("Thing (#{i})"), None),
            _ => {
                if i % 9 == 0 {
                    t += 10 * H;
                }
                let who = if i % 11 == 0 { "cy" } else { "ada" };
                put(&dir, who, t, &format!("work {i}"), (i % 10 == 8).then_some("Closes #1"));
            }
        }
        if i == 12 {
            git(&dir, &["tag", "v1.4.0"]);
        }
    }
    dir
}

#[test]
fn a_longer_load_changes_only_the_oldest_chapter() {
    let dir = mixed("chapters-prefix");
    let full = snap_of(&dir, 500);
    let all = group(&History::of(&full));
    assert!(all.list.len() > 10 && !all.open_end);
    for n in [5, 9, 14, 23, 31] {
        let short = snap_of(&dir, n);
        let c = group(&History::of(&short));
        let last = c.list.len() - 1;
        assert!(c.open_end, "{n}");
        for (i, ch) in c.list.iter().enumerate() {
            let long = &all.list[i];
            assert_eq!(ch.id, long.id, "{n}: chapter {i}");
            if i == last {
                continue;
            }
            assert_eq!(
                (ch.kind, &ch.title, &ch.line, &ch.releases),
                (long.kind, &long.title, &long.line, &long.releases),
                "{n}: chapter {i}"
            );
            if ch.partial {
                assert!(ch.count() <= long.count() && ch.oldest >= long.oldest && ch.newest == long.newest, "{n}: chapter {i}");
            } else {
                assert_eq!(ch, long, "{n}: chapter {i}");
            }
        }
    }
}

#[test]
fn a_merge_whose_history_reaches_below_the_load_is_partial() {
    let dir = scratch_repo("chapters-partial");
    put(&dir, "ada", T0, "m0", None);
    git(&dir, &["checkout", "-q", "-b", "long"]);
    for i in 1..=10 {
        put(&dir, "bob", T0 + 100 * i, &format!("s{i}"), None);
    }
    git(&dir, &["checkout", "-q", "main"]);
    for i in 1..=3 {
        put(&dir, "ada", T0 + 5000 + 100 * i, &format!("m{i}"), None);
    }
    merge(&dir, "ada", T0 + 9000, &["long"], "Merge branch 'long'");
    let short = snap_of(&dir, 6);
    let c = group(&History::of(&short));
    assert!(c.list[0].partial && c.open_end);
    assert!(c.list[0].count() < 11);
    let (_, c) = chapters_of(&dir);
    assert!(!c.list[0].partial && !c.open_end);
    assert_eq!(c.list[0].count(), 11);
}
