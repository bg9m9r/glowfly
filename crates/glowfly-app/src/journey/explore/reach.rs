//! Rule 3, no lost work: whether the text a session wrote is still somewhere a user could get it
//! back from.

use super::shadow::contains_marker;
use git2::{ObjectType, Oid, Repository};
use std::collections::BTreeSet;
use std::path::Path;

/// What is still to be found, and what has been looked through.
struct Search {
    markers: BTreeSet<String>,
    summaries: BTreeSet<String>,
    blobs: BTreeSet<Oid>,
    trees: BTreeSet<Oid>,
}

impl Search {
    fn done(&self) -> bool {
        self.markers.is_empty() && self.summaries.is_empty()
    }

    fn text(&mut self, text: &str) {
        let text = text.replace('\r', "");
        self.markers.retain(|m| !contains_marker(&text, m));
    }

    fn blob(&mut self, repo: &Repository, id: Oid) {
        if self.markers.is_empty() || !self.blobs.insert(id) {
            return;
        }
        if let Ok(blob) = repo.find_blob(id) {
            self.text(&String::from_utf8_lossy(blob.content()));
        }
    }

    fn tree(&mut self, repo: &Repository, id: Oid) {
        if self.markers.is_empty() || !self.trees.insert(id) {
            return;
        }
        let Ok(tree) = repo.find_tree(id) else { return };
        for entry in tree.iter() {
            match entry.kind() {
                Some(ObjectType::Blob) => self.blob(repo, entry.id()),
                Some(ObjectType::Tree) => self.tree(repo, entry.id()),
                _ => {}
            }
        }
    }

    fn commit(&mut self, repo: &Repository, id: Oid) {
        let Ok(commit) = repo.find_commit(id) else { return };
        let message = String::from_utf8_lossy(commit.message_bytes()).into_owned();
        self.summaries.retain(|s| !message.contains(s.as_str()));
        self.tree(repo, commit.tree_id());
    }

    fn work_tree(&mut self, dir: &Path) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            match entry.file_type() {
                Ok(t) if t.is_dir() => self.work_tree(&path),
                Ok(t) if t.is_file() => {
                    if let Ok(bytes) = std::fs::read(&path) {
                        self.text(&String::from_utf8_lossy(&bytes));
                    }
                }
                _ => {}
            }
        }
    }
}

/// The markers (searched in blob text, CR removed) and summaries (searched in commit
/// messages) found nowhere reachable: the work tree, the index, any commit reachable from any
/// ref (refs/glowfly/* and refs/stash included) or from any reflog entry.
pub(super) fn missing(repo: &Path, markers: &BTreeSet<String>, summaries: &BTreeSet<String>) -> Vec<String> {
    let mut search = Search { markers: markers.clone(), summaries: summaries.clone(), blobs: BTreeSet::new(), trees: BTreeSet::new() };
    let repository = Repository::open(repo).unwrap_or_else(|e| panic!("{} is not a repository: {e}", repo.display()));
    let repo_ref = &repository;
    if let Some(dir) = repository.workdir() {
        search.work_tree(dir);
    }
    if let Ok(index) = repository.index() {
        for entry in index.iter() {
            search.blob(repo_ref, entry.id);
        }
    }
    let mut starts: Vec<Oid> = Vec::new();
    let mut names: Vec<String> = vec!["HEAD".to_owned()];
    if let Ok(refs) = repository.references() {
        for reference in refs.flatten() {
            names.extend(reference.name().map(str::to_owned));
            starts.extend(reference.resolve().ok().and_then(|r| r.target()));
        }
    }
    for name in &names {
        if let Ok(log) = repository.reflog(name) {
            for entry in log.iter() {
                starts.extend([entry.id_new(), entry.id_old()].into_iter().filter(|id| !id.is_zero()));
            }
        }
    }
    let mut walk = repository.revwalk().unwrap();
    for id in starts {
        // A ref can point at anything: a tag at a commit, a saved file at a bare blob.
        let Ok(object) = repository.find_object(id, None) else { continue };
        match object.kind() {
            Some(ObjectType::Blob) => search.blob(repo_ref, id),
            Some(ObjectType::Tree) => search.tree(repo_ref, id),
            Some(ObjectType::Commit | ObjectType::Tag) => {
                if let Ok(commit) = object.peel_to_commit() {
                    let _ = walk.push(commit.id());
                }
            }
            _ => {}
        }
    }
    for id in walk.flatten() {
        if search.done() {
            break;
        }
        search.commit(repo_ref, id);
    }
    search.markers.into_iter().chain(search.summaries).collect()
}

#[cfg(test)]
#[path = "reach_tests.rs"]
mod tests;
