use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("trellis-persist-{name}-{}", std::process::id()));
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
