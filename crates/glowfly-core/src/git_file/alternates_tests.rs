//! The object stores a repository's alternates name, followed as libgit2 follows them and
//! refused when one is on a network place (`testutil::net_place()`, never touched).

use super::*;
use crate::testutil::{git_spelling, link_to, net_place, on_net_place, scratch_repo, write};

/// A scratch repository and its common dir.
fn repo(name: &str) -> (PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    let common = dir.join(".git");
    (dir, common)
}

/// Writes `objects/info/alternates` of the store `objects` (made when missing).
fn alternates(objects: &Path, text: &str) {
    let info = objects.join("info");
    std::fs::create_dir_all(&info).unwrap();
    write(&info, "alternates", text);
}

fn refused_on_net_place(r: Result<(), PathBuf>) -> bool {
    r.is_err_and(|p| on_net_place(&p))
}

#[test]
fn an_alternate_on_a_network_place_is_refused() {
    let (dir, common) = repo("alt-net");
    let objects = common.join("objects");
    let mut rows = vec![git_spelling(&net_place().join("objects"))];
    if cfg!(windows) {
        rows.push(net_place().join("objects").display().to_string());
    }
    for row in rows {
        alternates(&objects, &format!("{row}\n"));
        assert!(refused_on_net_place(reachable(&common, &dir)), "{row}");
    }
    std::fs::remove_file(objects.join("info/alternates")).unwrap();
    assert_eq!(reachable(&common, &dir), Ok(()), "no alternates");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn comments_blank_lines_and_crlf_are_skipped() {
    let (dir, common) = repo("alt-comments");
    let local = dir.join("store/objects");
    std::fs::create_dir_all(&local).unwrap();
    let net = git_spelling(&net_place().join("objects"));
    alternates(&common.join("objects"), &format!("# {net}\r\n\r\n\n{}\r\n", git_spelling(&local)));
    assert_eq!(reachable(&common, &dir), Ok(()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_relative_alternate_is_followed_from_objects() {
    let (dir, common) = repo("alt-relative");
    let other = dir.join("other/.git/objects");
    std::fs::create_dir_all(&other).unwrap();
    alternates(&common.join("objects"), "../../other/.git/objects\n");
    alternates(&other, &format!("{}\n", git_spelling(&net_place().join("objects"))));
    assert!(refused_on_net_place(reachable(&common, &dir)), "the store it names names a network place");
    std::fs::remove_file(other.join("info/alternates")).unwrap();
    assert_eq!(reachable(&common, &dir), Ok(()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_relative_line_not_starting_with_a_dot_is_refused() {
    let (dir, common) = repo("alt-cwd");
    std::fs::create_dir_all(dir.join("q/objects")).unwrap();
    // libgit2 would read both from the process's own folder.
    for line in ["objects-elsewhere".to_owned(), format!("\"{}\"", git_spelling(&dir.join("q/objects")))] {
        alternates(&common.join("objects"), &format!("{line}\n"));
        assert!(reachable(&common, &dir).is_err(), "{line}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn alternates_are_followed_to_libgit2s_depth() {
    let (dir, common) = repo("alt-depth");
    // s0 (the repository's own) names s1, which names s2, ... down to s6.
    let stores: Vec<PathBuf> = std::iter::once(common.join("objects")).chain((1..=6).map(|i| dir.join(format!("s{i}/objects")))).collect();
    for s in &stores {
        std::fs::create_dir_all(s).unwrap();
    }
    for pair in stores.windows(2) {
        alternates(&pair[0], &format!("{}\n", git_spelling(&pair[1])));
    }
    let net = git_spelling(&net_place().join("objects"));
    alternates(&stores[5], &format!("{}\n{net}\n", git_spelling(&stores[6])));
    assert!(refused_on_net_place(reachable(&common, &dir)), "depth 5's file is read");
    alternates(&stores[5], &format!("{}\n", git_spelling(&stores[6])));
    alternates(&stores[6], &format!("{net}\n"));
    assert_eq!(reachable(&common, &dir), Ok(()), "depth 6's file is never read");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_cycle_of_alternates_ends_and_too_many_stores_are_refused() {
    let (dir, common) = repo("alt-cycle");
    let s0 = common.join("objects");
    let s1 = dir.join("s1/objects");
    std::fs::create_dir_all(&s1).unwrap();
    alternates(&s0, &format!("{}\n", git_spelling(&s1)));
    alternates(&s1, &format!("{}\n", git_spelling(&s0)));
    assert_eq!(reachable(&common, &dir), Ok(()));
    let many: Vec<String> = (0..MAX_STORES).map(|i| git_spelling(&dir.join(format!("m{i}/objects")))).collect();
    alternates(&s0, &(many.join("\n") + "\n"));
    assert!(reachable(&common, &dir).is_err(), "more than {MAX_STORES} stores");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_oversized_or_linked_alternates_file_is_refused() {
    let (dir, common) = repo("alt-big");
    let objects = common.join("objects");
    alternates(&objects, &"# padding\n".repeat(8 * 1024));
    assert!(reachable(&common, &dir).is_err(), "over 64 KiB");
    let file = objects.join("info/alternates");
    std::fs::remove_file(&file).unwrap();
    if link_to(&net_place().join("alternates"), &file, false) {
        assert_eq!(reachable(&common, &dir), Err(file.clone()), "a link to a network place");
        std::fs::remove_file(&file).unwrap();
        let pack = objects.join("pack");
        let _ = std::fs::remove_dir_all(&pack);
        assert!(link_to(&net_place().join("pack"), &pack, true));
        assert_eq!(reachable(&common, &dir), Err(pack.clone()), "a store folder that links to a network place");
        let _ = std::fs::remove_dir(&pack).or_else(|_| std::fs::remove_file(&pack));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
