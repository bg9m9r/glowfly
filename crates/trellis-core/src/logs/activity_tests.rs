use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("trellis-activity-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn entry(at: i64, text: &str) -> ActivityEntry {
    ActivityEntry { session: 100, at, ok: at % 2 == 0, text: text.into() }
}

fn line_count(path: &Path) -> usize {
    std::fs::read_to_string(path).map(|s| s.lines().count()).unwrap_or(0)
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

#[test]
fn activity_line_round_trips() {
    let e = ActivityEntry {
        session: 1_700_000_000,
        at: 1_700_000_042,
        ok: false,
        text: "Push failed:\n\"rejected\" — naïve 日本\r\nend".into(),
    };
    let line = activity_line(&e);
    assert!(line.ends_with('\n'));
    assert_eq!(line.matches('\n').count(), 1, "one line per entry: {line}");
    assert!(line.starts_with(r#"{"v":1,"session":1700000000,"at":1700000042,"ok":false,"text":"#), "{line}");
    assert_eq!(parse_activity(&line), (vec![e], 0));
}

#[test]
fn parse_skips_torn_and_foreign_lines() {
    let good = activity_line(&entry(1, "one"));
    let other = activity_line(&entry(2, "two")).replace(r#""v":1"#, r#""v":2"#);
    let torn = &activity_line(&entry(3, "three"))[..20];
    let text = format!("{good}not json\n\n{other}{torn}");
    let (entries, skipped) = parse_activity(&text);
    assert_eq!(entries, vec![entry(1, "one")]);
    assert_eq!(skipped, 4, "a non-JSON line, a blank one, a v:2 one and the torn last one");
}

#[test]
fn long_text_is_capped_on_a_char_boundary() {
    // Three-byte chars put a char boundary off the cap's exact byte.
    for pad in 0..3 {
        let text = format!("{}{}", "a".repeat(pad), "語".repeat(MAX_ENTRY_BYTES));
        let line = activity_line(&ActivityEntry { session: 1, at: 2, ok: true, text });
        let (back, _) = parse_activity(&line);
        let kept = &back[0].text;
        assert!(kept.len() <= MAX_ENTRY_BYTES, "{}", kept.len());
        assert!(kept.len() > MAX_ENTRY_BYTES - 8);
        assert!(kept.ends_with('…'));
    }
    let short = "x".repeat(MAX_ENTRY_BYTES);
    let (back, _) = parse_activity(&activity_line(&ActivityEntry { session: 1, at: 2, ok: true, text: short.clone() }));
    assert_eq!(back[0].text, short, "text at the cap is kept whole");
}

#[test]
fn open_missing_file_is_empty() {
    let dir = scratch("missing");
    let (_, entries) = ActivityFile::open(dir.join("activity.jsonl"), 5).unwrap();
    assert!(entries.is_empty());
    assert!(names(&dir).is_empty(), "nothing is written");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn open_trims_to_cap_and_rewrites() {
    let dir = scratch("trim");
    let path = dir.join("activity.jsonl");
    let all: String = (0..12).map(|i| activity_line(&entry(i, &format!("line {i}")))).collect();
    std::fs::write(&path, all).unwrap();
    let (_, entries) = ActivityFile::open(path.clone(), 5).unwrap();
    assert_eq!(entries, (7..12).map(|i| entry(i, &format!("line {i}"))).collect::<Vec<_>>());
    assert_eq!(line_count(&path), 5);
    assert_eq!(names(&dir), ["activity.jsonl"], "no temporary files left behind");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn append_trims_past_twice_the_cap() {
    let dir = scratch("append");
    let path = dir.join("activity.jsonl");
    let (mut file, _) = ActivityFile::open(path.clone(), 3).unwrap();
    for i in 0..7 {
        file.append(&[entry(i, &format!("n{i}"))]).unwrap();
        assert!(line_count(&path) <= 6, "after append {i}: {} lines", line_count(&path));
    }
    let (entries, _) = parse_activity(&std::fs::read_to_string(&path).unwrap());
    assert_eq!(entries, (4..7).map(|i| entry(i, &format!("n{i}"))).collect::<Vec<_>>());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clear_removes_the_file_and_append_recreates_it() {
    let dir = scratch("clear");
    let path = dir.join("activity.jsonl");
    let (mut file, _) = ActivityFile::open(path.clone(), 3).unwrap();
    file.append(&[entry(1, "a"), entry(2, "b")]).unwrap();
    file.clear().unwrap();
    assert!(!path.exists());
    file.clear().unwrap();
    file.append(&[entry(3, "c")]).unwrap();
    let (_, entries) = ActivityFile::open(path, 3).unwrap();
    assert_eq!(entries, vec![entry(3, "c")]);
    let _ = std::fs::remove_dir_all(&dir);
}
