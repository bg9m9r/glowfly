use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("glowfly-persist-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn leftovers(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

#[test]
fn atomic_write_creates_and_replaces() {
    let dir = scratch("replace");
    let file = dir.join("sub").join("settings.json");
    atomic_write(&file, b"first").unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"first");
    // An existing target is replaced, and no temporary file is left behind.
    atomic_write(&file, b"second, longer").unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"second, longer");
    assert_eq!(leftovers(&dir.join("sub")), ["settings.json"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn atomic_write_failure_keeps_the_old_file() {
    let dir = scratch("fail");
    let file = dir.join("target");
    // A folder in the way can't be replaced by a file: the write fails and cleans up.
    std::fs::create_dir_all(&file).unwrap();
    assert!(atomic_write(&file, b"x").is_err());
    assert!(file.is_dir());
    assert_eq!(leftovers(&dir), ["target"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn append_creates_folder_and_appends() {
    let dir = scratch("append");
    let file = dir.join("logs").join("activity.jsonl");
    append(
        &file, b"one
",
    )
    .unwrap();
    append(
        &file, b"two
",
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "one
two
"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn append_follows_a_link() {
    let dir = scratch("append-link");
    let real = dir.join("real.log");
    let link = dir.join("link.log");
    std::fs::write(
        &real, "a
",
    )
    .unwrap();
    if !symlink_file(&real, &link) {
        eprintln!("skipped: this system refused to create a symbolic link");
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    append(
        &link, b"b
",
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(&real).unwrap(),
        "a
b
"
    );
    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "the link stays a link");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn move_aside_picks_a_free_name() {
    let dir = scratch("aside");
    let file = dir.join("settings.json");
    std::fs::write(&file, "one").unwrap();
    assert_eq!(move_aside(&file).unwrap(), dir.join("settings.json.bak"));
    std::fs::write(&file, "two").unwrap();
    assert_eq!(move_aside(&file).unwrap(), dir.join("settings.json.bak-2"));
    assert!(!file.exists());
    assert_eq!(std::fs::read_to_string(dir.join("settings.json.bak")).unwrap(), "one");
    assert_eq!(std::fs::read_to_string(dir.join("settings.json.bak-2")).unwrap(), "two");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn latest_only_skips_older_snapshots() {
    let order = LatestOnly::default();
    let (a, b) = (order.ticket(), order.ticket());
    assert_eq!(order.write(b, || "b"), Some("b"));
    // The older save arrives last and is dropped.
    assert_eq!(order.write(a, || "a"), None);
}

#[test]
fn concurrent_saves_newest_wins() {
    let dir = scratch("concurrent");
    let file = dir.join("settings.json");
    let order = std::sync::Arc::new(LatestOnly::default());
    let tickets: Vec<u64> = (0..32).map(|_| order.ticket()).collect();
    let last = *tickets.last().unwrap();
    // Start the threads newest first so older snapshots tend to land later.
    let threads: Vec<_> = tickets
        .into_iter()
        .rev()
        .map(|t| {
            let (order, file) = (order.clone(), file.clone());
            std::thread::spawn(move || {
                order.write(t, || atomic_write(&file, t.to_string().as_bytes()).unwrap());
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(std::fs::read_to_string(&file).unwrap(), last.to_string());
    assert_eq!(leftovers(&dir), ["settings.json"]);
    let _ = std::fs::remove_dir_all(&dir);
}

fn set_readonly(path: &Path, on: bool) {
    let mut p = std::fs::metadata(path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    p.set_readonly(on);
    std::fs::set_permissions(path, p).unwrap();
}

#[test]
fn a_read_only_target_is_replaced_and_stays_read_only() {
    let dir = scratch("readonly");
    let file = dir.join("notes.txt");
    std::fs::write(&file, "old").unwrap();
    set_readonly(&file, true);
    atomic_write(&file, b"new").unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"new");
    assert!(std::fs::metadata(&file).unwrap().permissions().readonly(), "keeps the old file's read-only state");
    assert_eq!(leftovers(&dir), ["notes.txt"]);
    // Removing one works too.
    remove_file(&file).unwrap();
    assert!(!file.exists());
    remove_file(&file).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Makes `link` a symbolic link to `target`; false where links can't be made (Windows without
/// Developer Mode or the privilege).
fn symlink_file(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(target, link);
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_file(target, link);
    made.is_ok()
}

#[test]
fn a_symlinked_file_stays_a_link_and_its_target_gets_the_bytes() {
    let dir = scratch("symlink");
    let real = dir.join("dotfiles").join("settings.json");
    std::fs::create_dir_all(real.parent().unwrap()).unwrap();
    std::fs::write(&real, "{}").unwrap();
    let link = dir.join("settings.json");
    // A relative link, as dotfile managers make them.
    if !symlink_file(Path::new("dotfiles/settings.json"), &link) {
        eprintln!("skipped: this system refused to create a symbolic link");
        return;
    }
    atomic_write(&link, b"{\"a\":1}").unwrap();
    assert!(link.symlink_metadata().unwrap().file_type().is_symlink(), "still a link");
    assert_eq!(std::fs::read(&real).unwrap(), b"{\"a\":1}");
    assert_eq!(leftovers(&dir), ["dotfiles", "settings.json"]);
    assert_eq!(leftovers(&dir.join("dotfiles")), ["settings.json"]);
    // A working-tree write refuses the link and changes nothing.
    let refused = write_file(&link, b"x", WriteOptions { refuse_links: true, executable: None, private: false });
    assert!(refused.unwrap_err().to_string().contains("symbolic link"));
    assert_eq!(std::fs::read(&real).unwrap(), b"{\"a\":1}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn temp_names_are_recognised() {
    for (candidate, ours) in [
        (".settings.json.123.0.tmp", true),
        (".settings.json.1.99.tmp", true),
        (".settings.json.123.tmp", false),
        (".settings.json.a.0.tmp", false),
        (".settings.json..0.tmp", false),
        ("settings.json.123.0.tmp", false),
        (".settings.json.123.0.tmp.bak", false),
        (".other.json.123.0.tmp", false),
        (".settings.json.x.123.0.tmp", false),
    ] {
        assert_eq!(is_temp_of("settings.json", candidate), ours, "{candidate}");
    }
}

#[test]
fn stale_temp_files_are_cleaned_up() {
    let dir = scratch("stale");
    let file = dir.join("tabs.txt");
    let stale = dir.join(".tabs.txt.4242.7.tmp");
    let fresh = dir.join(".tabs.txt.4243.0.tmp");
    let unrelated = dir.join(".other.txt.4242.7.tmp");
    for p in [&stale, &fresh, &unrelated] {
        std::fs::write(p, "half").unwrap();
    }
    let long_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    for p in [&stale, &unrelated] {
        std::fs::File::options().write(true).open(p).unwrap().set_modified(long_ago).unwrap();
    }
    atomic_write(&file, b"a").unwrap();
    assert_eq!(leftovers(&dir), [".other.txt.4242.7.tmp", ".tabs.txt.4243.0.tmp", "tabs.txt"], "only the old one for this file goes");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn modes_follow_the_old_file_and_the_executable_bit() {
    // (old, created, executable) -> wanted
    for (old, created, exec, want) in [
        (Some(0o100600), 0o644, None, 0o600),
        (Some(0o755), 0o644, Some(false), 0o644),
        (Some(0o644), 0o600, Some(true), 0o755),
        (Some(0o640), 0o600, Some(true), 0o750),
        (None, 0o644, None, 0o644),
        (None, 0o644, Some(true), 0o755),
        (None, 0o600, Some(true), 0o700),
    ] {
        assert_eq!(wanted_mode(old, created, exec), want, "{old:?} {created:o} {exec:?}");
    }
    assert!(!needs_chmod(0o100644, 0o644), "the file type bits don't count");
    assert!(!needs_chmod(0o755, 0o755));
    assert!(needs_chmod(0o644, 0o755));
}

#[cfg(unix)]
#[test]
fn the_old_mode_is_kept_and_the_executable_bit_set_on_unix() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("mode");
    let file = dir.join("run.sh");
    std::fs::write(&file, "old").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    atomic_write(&file, b"new").unwrap();
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o7777, 0o600);
    write_file(&file, b"newer", WriteOptions { refuse_links: true, executable: Some(true), private: false }).unwrap();
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o7777, 0o700);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Makes `link` a link to the folder `target`: a symbolic link, or on Windows a junction where
/// symbolic links need rights the account lacks. False when neither can be made.
fn dir_link(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(target, link).is_ok();
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_dir(target, link).is_ok()
        || std::process::Command::new("cmd").args(["/c", "mklink", "/J"]).arg(link).arg(target).output().is_ok_and(|o| o.status.success());
    made
}

#[test]
fn remove_tree_removes_nested_folders_and_read_only_files() {
    let dir = scratch("tree");
    let tree = dir.join("modules").join("lib");
    std::fs::create_dir_all(tree.join("objects").join("pack")).unwrap();
    std::fs::write(tree.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    let pack = tree.join("objects").join("pack").join("pack-1.pack");
    std::fs::write(&pack, "pack").unwrap();
    set_readonly(&pack, true);
    remove_tree(&tree).unwrap();
    assert!(tree.symlink_metadata().is_err());
    assert!(dir.join("modules").is_dir(), "only the tree itself goes");
    // Nothing there is fine.
    remove_tree(&tree).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn remove_tree_removes_a_link_but_not_its_target() {
    let dir = scratch("tree-link");
    let outside = dir.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("keep.txt"), "keep").unwrap();
    let tree = dir.join("tree");
    std::fs::create_dir_all(&tree).unwrap();
    if !dir_link(&outside, &tree.join("link")) {
        eprintln!("skipped: this account can't make folder links");
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    remove_tree(&tree).unwrap();
    assert!(tree.symlink_metadata().is_err());
    assert_eq!(std::fs::read_to_string(outside.join("keep.txt")).unwrap(), "keep", "the target is untouched");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_private_write_keeps_the_file_to_its_user() {
    let dir = scratch("private");
    let file = dir.join("run").join("instance.json");
    let private = WriteOptions { private: true, ..Default::default() };
    write_file(&file, b"one", private).unwrap();
    write_file(&file, b"two", private).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"two");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&file), 0o600, "a new file and its replacement");
        assert_eq!(mode(&dir.join("run")), 0o700, "the folder it made");
        // A file that was readable by others is private once replaced.
        let open = dir.join("open.json");
        std::fs::write(&open, "x").unwrap();
        std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o644)).unwrap();
        write_file(&open, b"y", private).unwrap();
        assert_eq!(mode(&open), 0o600);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_streamed_write_replaces_atomically() {
    let dir = scratch("streamed");
    let file = dir.join("big.idx");
    atomic_write(&file, b"old").unwrap();
    write_streamed(&file, WriteOptions::default(), |f| f.write_all(b"new bytes")).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"new bytes");
    // A fill that fails half way leaves the old file and no temporary sibling.
    let failed = write_streamed(&file, WriteOptions::default(), |f| {
        f.write_all(b"half")?;
        Err(std::io::Error::other("disk full"))
    });
    assert!(failed.is_err());
    assert_eq!(std::fs::read(&file).unwrap(), b"new bytes");
    assert_eq!(leftovers(&dir), ["big.idx"]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let private = dir.join("private.idx");
        write_streamed(&private, WriteOptions { private: true, ..Default::default() }, |f| f.write_all(b"x")).unwrap();
        assert_eq!(std::fs::metadata(&private).unwrap().permissions().mode() & 0o777, 0o600);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
