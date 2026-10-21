//! Reading and splitting `commit.template`.

use super::template::MAX_TEMPLATE_BYTES;
use super::*;
use crate::testutil::*;
use git2::Repository;

fn tmpl(summary: &str, description: &str, comment_char: char) -> Option<CommitTemplate> {
    Some(CommitTemplate { summary: summary.into(), description: description.into(), comment_char })
}

#[test]
fn parse_splits_summary_and_description() {
    let want = tmpl("feat:", "Why:\n# explain", '#');
    assert_eq!(CommitTemplate::parse("feat: \n\nWhy:\n# explain\n", '#'), want);
    assert_eq!(CommitTemplate::parse("feat: \r\n\r\nWhy:\r\n# explain\r\n", '#'), want);
}

#[test]
fn parse_comment_first_keeps_summary_empty() {
    assert_eq!(CommitTemplate::parse("# Summary in 50 chars\n\n# Body\n", '#'), tmpl("", "# Summary in 50 chars\n\n# Body", '#'));
    assert_eq!(CommitTemplate::parse("\n\nBody\n", '#'), tmpl("", "Body", '#'));
    assert_eq!(CommitTemplate::parse("; note\nmore\n", ';'), tmpl("", "; note\nmore", ';'));
    assert_eq!(CommitTemplate::parse(" \n\t\n", '#'), None);
    assert_eq!(CommitTemplate::parse("", '#'), None);
}

#[test]
fn commit_template_reads_config() {
    let dir = scratch_repo("template");
    let read = || commit_template(&Repository::open(&dir).unwrap(), &dir);
    assert_eq!(read(), None, "no commit.template");

    write(&dir, "tmpl.txt", "fix: \n\n# why\n");
    git(&dir, &["config", "commit.template", "tmpl.txt"]);
    assert_eq!(read(), tmpl("fix:", "# why", '#'), "relative to the working directory");

    let abs = dir.with_file_name(format!("glowfly-test-template-abs-{}.txt", std::process::id()));
    std::fs::write(&abs, "docs: \n").unwrap();
    git(&dir, &["config", "commit.template", &abs.display().to_string().replace('\\', "/")]);
    assert_eq!(read(), tmpl("docs:", "", '#'), "absolute path");
    let _ = std::fs::remove_file(&abs);
    assert_eq!(read(), None, "missing file");

    write(&dir, "big.txt", &"x".repeat(MAX_TEMPLATE_BYTES as usize + 1));
    git(&dir, &["config", "commit.template", "big.txt"]);
    assert_eq!(read(), None, "too large");

    git(&dir, &["config", "commit.template", "tmpl.txt"]);
    git(&dir, &["config", "core.commentChar", ";"]);
    assert_eq!(read().unwrap().comment_char, ';');
    git(&dir, &["config", "core.commentChar", "auto"]);
    assert_eq!(read().unwrap().comment_char, '#');
}
