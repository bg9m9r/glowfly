use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("glowfly-bundle-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn header() -> Vec<(String, String)> {
    vec![("Exported".into(), "2026-03-04 07:06:07 +02:00".into()), ("Glowfly".into(), "0.1.0".into())]
}

#[test]
fn bundle_holds_header_and_every_log_in_order() {
    let dir = scratch("order");
    let out = scratch("order-out");
    let files = [
        ("activity.jsonl", "{\"v\":1,\"text\":\"Fetched\"}\n"),
        ("errors.2.log", "oldest error\n"),
        ("errors.1.log", "older error\n"),
        ("errors.log", "newest error\n"),
        ("performance.log", "slow thing\n"),
    ];
    for (name, text) in files {
        std::fs::write(dir.join(name), text).unwrap();
    }
    let dest = out.join("bundle.txt");
    let report = write_bundle(&dir, &header(), &dest).unwrap();
    let bytes: u64 = files.iter().map(|(_, t)| t.len() as u64).sum();
    assert_eq!(report, BundleReport { files: 5, bytes });
    let text = std::fs::read_to_string(&dest).unwrap();
    let start = "Glowfly logs\nExported: 2026-03-04 07:06:07 +02:00\nGlowfly: 0.1.0\n\
                 Files: activity.jsonl, errors.2.log, errors.1.log, errors.log, performance.log\n\n";
    assert!(text.starts_with(start), "{text}");
    let mut at = 0;
    for (name, content) in files {
        let section = format!("===== {name} ({} bytes) =====\n{content}", content.len());
        let found = text[at..].find(&section).unwrap_or_else(|| panic!("{name} in order: {text}"));
        at += found + section.len();
    }
    assert_eq!(at, text.len());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn missing_logs_are_left_out() {
    let dir = scratch("missing");
    let (text, report) = bundle_text(&dir, &header());
    assert_eq!(report, BundleReport { files: 0, bytes: 0 });
    assert_eq!(text, "Glowfly logs\nExported: 2026-03-04 07:06:07 +02:00\nGlowfly: 0.1.0\nFiles: none\n\n");
    std::fs::write(dir.join("errors.log"), "e\n").unwrap();
    let (text, report) = bundle_text(&dir, &[]);
    assert_eq!(report, BundleReport { files: 1, bytes: 2 });
    assert_eq!(text, "Glowfly logs\nFiles: errors.log\n\n===== errors.log (2 bytes) =====\ne\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn content_without_final_newline_and_invalid_utf8() {
    let dir = scratch("bytes");
    std::fs::write(dir.join("errors.log"), b"no end").unwrap();
    std::fs::write(dir.join("performance.log"), b"bad \xFF byte\n").unwrap();
    let h = vec![("Git".to_owned(), "2.47.1\r\n(here)".to_owned())];
    let (text, report) = bundle_text(&dir, &h);
    assert_eq!(report.files, 2);
    assert!(text.contains("Git: 2.47.1  (here)\n"), "{text}");
    assert!(text.contains("no end\n===== performance.log (11 bytes) =====\n"), "{text}");
    assert!(text.contains("bad \u{FFFD} byte\n"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refuses_a_log_file_as_target() {
    let dir = scratch("refuse");
    let errors = dir.join("errors.log");
    std::fs::write(&errors, "keep me\n").unwrap();
    let e = write_bundle(&dir, &header(), &errors).unwrap_err();
    assert_eq!(e.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(e.to_string(), "Choose another file: errors.log is one of the logs being exported.");
    assert_eq!(std::fs::read_to_string(&errors).unwrap(), "keep me\n");
    // The same file under another spelling, where the file system ignores case.
    let upper = dir.join("ERRORS.LOG");
    if upper.exists() {
        let e = write_bundle(&dir, &header(), &upper).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(std::fs::read_to_string(&errors).unwrap(), "keep me\n");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn overwrites_an_existing_target_atomically() {
    let dir = scratch("overwrite");
    let out = scratch("overwrite-out");
    std::fs::write(dir.join("errors.log"), "e\n").unwrap();
    let dest = out.join("bundle.txt");
    std::fs::write(&dest, "old bundle").unwrap();
    write_bundle(&dir, &[], &dest).unwrap();
    assert!(std::fs::read_to_string(&dest).unwrap().contains("===== errors.log (2 bytes) ====="));
    let names: Vec<_> = std::fs::read_dir(&out).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, ["bundle.txt"], "no temporary files are left behind");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn bundle_name_and_stamp() {
    // 2026-03-04 05:06:07 UTC.
    let at = 1_772_600_767;
    assert_eq!(bundle_name(at, 7200), "glowfly-logs-20260304-070607.txt");
    assert_eq!(stamp(at, 7200), "2026-03-04 07:06:07 +02:00");
    assert_eq!(stamp(at, -5 * 3600), "2026-03-04 00:06:07 -05:00");
    assert_eq!(stamp(at, 0), "2026-03-04 05:06:07 +00:00");
    assert_eq!(platform(), format!("{} {}", std::env::consts::OS, std::env::consts::ARCH));
}

#[test]
fn bundle_sources_order() {
    let dir = crate::paths::temp_dir().join("glowfly-logs");
    let names: Vec<String> = bundle_sources(&dir).iter().map(|p| file_name(p)).collect();
    assert_eq!(
        names,
        [
            "activity.jsonl",
            "errors.3.log",
            "errors.2.log",
            "errors.1.log",
            "errors.log",
            "performance.3.log",
            "performance.2.log",
            "performance.1.log",
            "performance.log"
        ]
    );
    assert!(bundle_sources(&dir).iter().all(|p| p.parent() == Some(dir.as_path())));
}
