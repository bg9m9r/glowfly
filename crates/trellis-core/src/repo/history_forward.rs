//! Following a file forwards: the name a file in one commit has in a later commit, through
//! the renames between them (the file history opened at an older commit starts at HEAD under
//! that name).

use super::history::{Kind, present, renamed_to};
use crate::error::Result;
use git2::{Oid, Repository, Sort};
use std::collections::HashMap;

/// How many commits between the two the search reads before giving up.
const FORWARD_MAX: usize = 20_000;

/// The name `path` (a file in commit `from`) has in commit `head`, a descendant of `from`,
/// following renames forwards; `None` when no line of descent keeps the file (it was deleted)
/// or the search gave up.
pub(super) fn name_at(repo: &Repository, from: Oid, head: Oid, path: &str) -> Result<Option<String>> {
    if from == head {
        return Ok(present(&repo.find_commit(head)?.tree()?, path, Kind::File).map(|_| path.to_owned()));
    }
    let mut walk = repo.revwalk()?;
    walk.push(head)?;
    walk.hide(from)?;
    // Parents before their children, so each commit's parents are named before it is looked at.
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;
    let mut names: HashMap<Oid, String> = HashMap::from([(from, path.to_owned())]);
    for (i, oid) in walk.enumerate() {
        if i >= FORWARD_MAX {
            return Ok(None);
        }
        let oid = oid?;
        let c = repo.find_commit(oid)?;
        // The parents that hold the file, first parent first; the others have nothing to follow.
        let named: Vec<(Oid, String)> = c.parent_ids().filter_map(|p| names.get(&p).map(|n| (p, n.clone()))).collect();
        if named.is_empty() {
            continue;
        }
        let tree = c.tree()?;
        for (p, n) in named {
            let got = if present(&tree, &n, Kind::File).is_some() {
                Some(n)
            } else {
                renamed_to(repo, &repo.find_commit(p)?.tree()?, &tree, &n)?
            };
            if let Some(got) = got {
                names.insert(oid, got);
                break;
            }
        }
    }
    Ok(names.remove(&head))
}
