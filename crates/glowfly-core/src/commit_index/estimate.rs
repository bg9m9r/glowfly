//! About how many commits a repository has, read from git's commit-graph files without walking
//! anything (the format is gitformat-commit-graph(5)). git writes them on gc and, when asked, on
//! fetch, so the count may be stale and may include commits no ref reaches: callers show it as an
//! estimate. Read-only; no git process runs.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// The commit count the repository's commit-graph files give (a single file, or the sum over a
/// split chain), or `None` when there are none or one can't be read as a commit-graph.
pub fn commit_count(common_dir: &Path) -> Option<usize> {
    let info = common_dir.join("objects").join("info");
    let single = info.join("commit-graph");
    if single.is_file() {
        return graph_count(&single);
    }
    let dir = info.join("commit-graphs");
    let chain = std::fs::read_to_string(dir.join("commit-graph-chain")).ok()?;
    let mut total = 0usize;
    let mut files = 0;
    for hash in chain.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        total = total.checked_add(graph_count(&dir.join(format!("graph-{hash}.graph")))?)?;
        files += 1;
    }
    (files > 0).then_some(total)
}

/// The number of commits in one commit-graph file: the last entry of its OID fanout chunk.
fn graph_count(path: &Path) -> Option<usize> {
    let mut f = File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    // Signature, version 1, hash version (1 SHA-1, 2 SHA-256), chunk count, base-graph count.
    let mut head = [0u8; 8];
    f.read_exact(&mut head).ok()?;
    if &head[..4] != b"CGPH" || head[4] != 1 || !matches!(head[5], 1 | 2) {
        return None;
    }
    let chunks = usize::from(head[6]);
    // (chunks + 1) entries of a 4-byte id and an 8-byte offset; the last has id 0.
    let mut table = vec![0u8; (chunks + 1) * 12];
    f.read_exact(&mut table).ok()?;
    if table[chunks * 12..chunks * 12 + 4] != [0; 4] {
        return None;
    }
    let oidf = table.as_chunks::<12>().0.iter().take(chunks).find(|e| &e[..4] == b"OIDF")?;
    let offset = u64::from_be_bytes(oidf[4..12].try_into().ok()?);
    if offset.checked_add(1024)? > len {
        return None;
    }
    f.seek(SeekFrom::Start(offset)).ok()?;
    let mut fanout = [0u8; 1024];
    f.read_exact(&mut fanout).ok()?;
    let counts: Vec<u32> = fanout.as_chunks::<4>().0.iter().map(|b| u32::from_be_bytes(*b)).collect();
    // Cumulative counts never go down.
    if counts.windows(2).any(|w| w[1] < w[0]) {
        return None;
    }
    usize::try_from(counts[255]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    fn commits(dir: &Path, names: &[&str]) {
        for n in names {
            write(dir, "f.txt", n);
            git(dir, &["add", "."]);
            git(dir, &["commit", "-q", "-m", n]);
        }
    }

    fn graph_dir(dir: &Path) -> std::path::PathBuf {
        dir.join(".git").join("objects").join("info")
    }

    #[test]
    fn counts_come_from_the_commit_graph_files() {
        let dir = scratch_repo("estimate");
        commits(&dir, &["a", "b", "c"]);
        let common = dir.join(".git");
        assert_eq!(commit_count(&common), None, "no commit-graph yet");

        git(&dir, &["commit-graph", "write", "--reachable"]);
        assert_eq!(commit_count(&common), Some(3));

        // A split chain: two layers, summed.
        let _ = std::fs::remove_file(graph_dir(&dir).join("commit-graph"));
        git(&dir, &["commit-graph", "write", "--reachable", "--split=no-merge"]);
        commits(&dir, &["d", "e"]);
        git(&dir, &["commit-graph", "write", "--reachable", "--split=no-merge"]);
        assert!(!graph_dir(&dir).join("commit-graph").exists());
        let chain = std::fs::read_to_string(graph_dir(&dir).join("commit-graphs").join("commit-graph-chain")).unwrap();
        assert_eq!(chain.lines().count(), 2, "two layers: {chain}");
        assert_eq!(commit_count(&common), Some(5));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_damaged_file_gives_none() {
        let dir = scratch_repo("estimate-bad");
        commits(&dir, &["a", "b"]);
        git(&dir, &["commit-graph", "write", "--reachable"]);
        let file = graph_dir(&dir).join("commit-graph");
        let bytes = std::fs::read(&file).unwrap();
        let common = dir.join(".git");
        assert_eq!(commit_count(&common), Some(2));
        // Some platforms keep git's written files read-only.
        let writable = |f: &Path| {
            let mut p = std::fs::metadata(f).unwrap().permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            p.set_readonly(false);
            std::fs::set_permissions(f, p).unwrap();
        };
        writable(&file);
        std::fs::write(&file, &bytes[..40]).unwrap();
        assert_eq!(commit_count(&common), None, "cut short");
        std::fs::write(&file, b"not a commit graph at all, just some text that is long enough").unwrap();
        assert_eq!(commit_count(&common), None, "garbage");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
