//! Clone from a local bare remote through the Clone dialog, stage one hunk of two and a new
//! file, commit, push, then pull a commit another clone pushed.

use super::find::{Pick, Region};
use super::fixture::{self as disk, Fixture, commit_files, configure_clone, git, shown, write};
use super::harness::Journey;

/// notes.txt: twenty numbered lines.
fn notes() -> String {
    (1..=20).map(|n| format!("line {n}\n")).collect()
}

#[test]
fn clone_stage_commit_push_pull() {
    let fx = Fixture::new("clone");
    let (bare, _seed) = fx.seeded_remote("shop", &[("notes.txt", &notes()), ("a.txt", "a\n")]);
    let clone = fx.work().join("shop");
    let work = fx.work();
    let mut j = Journey::start(fx, None, vec![clone.clone()]);

    // step 1: the welcome offers a clone; the dialog takes the bare remote and the folder.
    j.see("Clone a repository", Region::Central);
    j.click("Clone a repository", Region::Central, Pick::Only);
    j.type_into("Repository URL", &shown(&bare));
    j.type_into("Clone into folder", &shown(&work));
    j.click("Clone", Region::Anywhere, Pick::Only);
    j.toast(true, "Clone succeeded");
    j.see("shop", Region::Toolbar);
    j.see("✔  main", Region::Sidebar);
    assert!(clone.is_dir());
    assert_eq!(disk::head_branch(&clone).as_deref(), Some("main"));
    assert_eq!(Some(disk::head_oid(&clone)), disk::remote_oid(&bare, "main"));
    let recent = std::fs::read_to_string(j.config_file("recent.txt")).unwrap();
    assert!(crate::platform::parse_recent(&recent).iter().any(|p| Journey::same(p, &clone)), "{recent}");

    // step 2: two edits far apart in notes.txt (two hunks) and a new file.
    configure_clone(&clone);
    let edited = notes().replace("line 2\n", "line 2 edited\n").replace("line 19\n", "line 19 edited\n");
    write(&clone, "notes.txt", &edited);
    write(&clone, "todo.txt", "buy milk\n");
    j.refresh();
    j.select_wip();
    j.see("Unstaged Files (2)", Region::Details);
    assert_eq!(disk::index_text(&clone, "notes.txt"), Some(notes()), "nothing staged yet");
    assert_eq!(disk::index_text(&clone, "todo.txt"), None);

    // step 3: stage only the first hunk of notes.txt.
    j.click("notes.txt", Region::Details, Pick::Only);
    j.click("Stage Hunk", Region::Central, Pick::First);
    let staged = disk::index_text(&clone, "notes.txt").unwrap();
    assert!(staged.contains("line 2 edited\n") && staged.contains("line 19\n"), "{staged}");
    assert_eq!(j.count("notes.txt", Region::Details), 2, "in both lists");
    j.see("Staged Files (1)", Region::Details);

    // step 4: stage the whole new file from its row.
    j.click("Stage", Region::Details, Pick::InRowOf("todo.txt"));
    assert_eq!(disk::index_text(&clone, "todo.txt").as_deref(), Some("buy milk\n"));
    j.see("Staged Files (2)", Region::Details);

    // step 5: commit the staged changes; the second hunk stays in the working tree.
    j.type_into("Commit summary", "Edit notes and add a todo list");
    j.click("Commit changes to 2 files", Region::Details, Pick::Only);
    j.toast(true, "Commit succeeded");
    let head = disk::head_oid(&clone);
    assert_eq!(disk::summary(&clone, head), "Edit notes and add a todo list");
    assert_eq!(disk::tree_text(&clone, head, "todo.txt").as_deref(), Some("buy milk\n"));
    let committed = disk::tree_text(&clone, head, "notes.txt").unwrap();
    assert!(committed.contains("line 2 edited\n") && committed.contains("line 19\n"), "{committed}");
    assert!(disk::work_text(&clone, "notes.txt").unwrap().contains("line 19 edited\n"));
    j.see("Staged Files (0)", Region::Details);
    // The diff still fills the centre; closing it shows the new commit in the graph.
    j.click_containing("Close", Region::Central, Pick::Only);
    j.see("Edit notes and add a todo list", Region::Central);

    // step 6: push to the bare remote.
    j.click("Push the current branch", Region::Toolbar, Pick::Only);
    j.toast(true, "Push succeeded");
    assert_eq!(disk::remote_oid(&bare, "main"), Some(head));

    // step 7: someone else pushes a commit; pull brings it in and keeps the unstaged edit.
    let other = j.fx.root.join("other");
    j.fx.clone_cli(&bare, &other);
    commit_files(&other, &[("remote.txt", "from elsewhere\n")], "Add remote.txt");
    git(&other, &["push", "-q", "origin", "main"]);
    j.click("Pull the current branch", Region::Toolbar, Pick::Only);
    j.toast(true, "Pull succeeded");
    assert_eq!(Some(disk::head_oid(&clone)), disk::remote_oid(&bare, "main"));
    assert_eq!(disk::work_text(&clone, "remote.txt").as_deref(), Some("from elsewhere\n"));
    assert!(disk::work_text(&clone, "notes.txt").unwrap().contains("line 19 edited\n"));
    j.see("Add remote.txt", Region::Central);
}
