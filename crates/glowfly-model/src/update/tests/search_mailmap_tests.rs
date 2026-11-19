//! The author picker and author search under a `.mailmap`: one entry per mapped person, with
//! the commits made under every name counted, and picking it finds them all.

use super::tests::{scratch_repo, state};
use super::update;
use crate::msg::{Msg, SearchMsg};
use crate::state::Tab;
use glowfly_core::repo::{self, LoadOptions};
use std::time::Instant;

#[test]
fn the_author_picker_lists_a_mapped_person_once() {
    let dir = scratch_repo("search-mailmap");
    let repo = git2::Repository::open(&dir).unwrap();
    let tree = repo.find_tree(repo.treebuilder(None).unwrap().write().unwrap()).unwrap();
    let mut parent = repo.head().unwrap().peel_to_commit().unwrap();
    for (i, (name, email)) in
        [("Grace Hopper", "grace@example.com"), ("G. Hopper", "ghopper@navy.example"), ("Ada Lovelace", "ada@example.com")]
            .into_iter()
            .enumerate()
    {
        let sig = git2::Signature::new(name, email, &git2::Time::new(1_800_000_000 + i as i64, 0)).unwrap();
        let oid = repo.commit(Some("HEAD"), &sig, &sig, "work", &tree, &[&parent]).unwrap();
        parent = repo.find_commit(oid).unwrap();
    }
    std::fs::write(dir.join(".mailmap"), "Grace Hopper <grace@example.com> <ghopper@navy.example>\n").unwrap();

    let mut st = state();
    st.tabs[0] = Tab::unloaded(dir.clone());
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    let send = |st: &mut crate::state::AppState, msg| update(st, Msg::Search(msg), Instant::now());

    send(&mut st, SearchMsg::WantAuthors);
    let picker = st.session.search.authors.clone().expect("the list is worked out");
    let grace: Vec<_> = picker.iter().filter(|a| a.email == "grace@example.com").collect();
    assert_eq!(grace.len(), 1, "one entry for Grace: {picker:?}");
    assert_eq!((grace[0].name.as_str(), grace[0].commits), ("Grace Hopper", 2));
    assert!(picker.iter().all(|a| a.email != "ghopper@navy.example"));

    send(&mut st, SearchMsg::PickAuthor(Some("grace@example.com".into())));
    assert_eq!(st.view.tab.search, "author:grace@example.com");
    let snap = st.snap.as_ref().unwrap();
    let names: Vec<&str> = st.session.search.matches.iter().map(|&r| snap.commits[r].author.name()).collect();
    assert_eq!(names, ["Grace Hopper", "Grace Hopper"], "the commits made under both names");
    let _ = std::fs::remove_dir_all(&dir);
}
