use super::*;
use crate::path_query::PathTerm;
use crate::testutil::*;
use git2::Repository;
use std::path::Path;

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn commit(dir: &Path, message: &str) -> Oid {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    head(dir)
}

fn put(dir: &Path, file: &str, text: &str) {
    let path = dir.join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

struct History {
    dir: std::path::PathBuf,
    root: Oid,
    view: Oid,
    lib: Oid,
    delete: Oid,
    rename: Oid,
    guide: Oid,
    edit: Oid,
    merge: Oid,
}

impl History {
    /// Newest first, as the graph lists them.
    fn all(&self) -> Vec<Oid> {
        vec![self.merge, self.edit, self.guide, self.rename, self.delete, self.lib, self.view, self.root]
    }
}

fn history(name: &str) -> History {
    let dir = scratch_repo(name);
    put(&dir, "README.md", "hello\n");
    put(&dir, "src/main.rs", "fn main() {}\n");
    let root = commit(&dir, "root");
    put(&dir, "src/ui/view.rs", "// view\n");
    let view = commit(&dir, "add view");
    put(&dir, "Lib/Core.rs", "// core\n");
    let lib = commit(&dir, "add lib");
    std::fs::remove_file(dir.join("README.md")).unwrap();
    let delete = commit(&dir, "delete readme");
    git(&dir, &["mv", "src/main.rs", "src/app.rs"]);
    let rename = commit(&dir, "rename main");
    git(&dir, &["checkout", "-q", "-b", "docs"]);
    put(&dir, "docs/guide.md", "# guide\n");
    let guide = commit(&dir, "write guide");
    git(&dir, &["checkout", "-q", "main"]);
    put(&dir, "src/ui/view.rs", "// view, edited\n");
    let edit = commit(&dir, "edit view");
    git(&dir, &["merge", "-q", "--no-ff", "-m", "merge docs", "docs"]);
    let merge = head(&dir);
    History { dir, root, view, lib, delete, rename, guide, edit, merge }
}

/// The commits of `h` touching `values` (each a path: value).
fn touching(h: &History, values: &[&str]) -> Vec<Oid> {
    let terms: Vec<PathTerm> = values.iter().map(|v| PathTerm::parse(v).unwrap()).collect();
    let t = commits_touching(&h.dir, &h.all(), &terms, &|| false).unwrap();
    assert_eq!(t.gone_through, 8);
    t.touching
}

#[test]
fn commits_touching_diff_against_the_first_parent() {
    let h = history("touching-basic");
    // The root against the empty tree; the delete through its old path.
    assert_eq!(touching(&h, &["README.md"]), [h.delete, h.root]);
    // A rename is a delete and an add: either name finds it.
    assert_eq!(touching(&h, &["src/main.rs"]), [h.rename, h.root]);
    assert_eq!(touching(&h, &["src/app.rs"]), [h.rename]);
    // The merge brought docs/ in on its first-parent diff.
    assert_eq!(touching(&h, &["docs"]), [h.merge, h.guide]);
    assert_eq!(touching(&h, &["SRC/UI"]), [h.edit, h.view]);
    // The literal prunes trees with a pathspec; the glob reads them all. Same answer.
    assert_eq!(touching(&h, &["src/ui"]), touching(&h, &["src/ui/**"]));
    // Pruning ignores case too.
    assert_eq!(touching(&h, &["lib/core.rs"]), [h.lib]);
    assert_eq!(touching(&h, &["LIB/"]), [h.lib]);
    // Several terms are alternatives; a literal beside a glob drops the pruning.
    assert_eq!(touching(&h, &["docs/", "lib"]), [h.merge, h.guide, h.lib]);
    assert_eq!(touching(&h, &["src/ui", "*.md"]), [h.merge, h.edit, h.guide, h.delete, h.view, h.root]);
    assert!(touching(&h, &["nothing/here"]).is_empty());
    let _ = std::fs::remove_dir_all(&h.dir);
}

#[test]
fn commits_touching_stops_and_skips_what_it_cannot_read() {
    let h = history("touching-stop");
    let terms = [PathTerm::parse("src").unwrap()];
    let t = commits_touching(&h.dir, &h.all(), &terms, &|| true).unwrap();
    assert_eq!(t, Touching { gone_through: 1, touching: vec![] }, "the merge doesn't change src");
    let t = commits_touching(&h.dir, &[h.edit, h.merge], &terms, &|| true).unwrap();
    assert_eq!(t, Touching { gone_through: 1, touching: vec![h.edit] });

    let unknown = Oid::from_bytes(&[7; 20]).unwrap();
    let t = commits_touching(&h.dir, &[unknown, h.root], &terms, &|| false).unwrap();
    assert_eq!(t, Touching { gone_through: 2, touching: vec![h.root] });

    let not_repo = crate::paths::temp_dir().join(format!("glowfly-touching-none-{}", std::process::id()));
    std::fs::create_dir_all(&not_repo).unwrap();
    assert!(commits_touching(&not_repo, &[h.root], &terms, &|| false).is_err());
    let _ = std::fs::remove_dir_all(&not_repo);
    let _ = std::fs::remove_dir_all(&h.dir);
}
