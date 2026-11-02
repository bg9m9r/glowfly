//! The patch builder's own cases: which lines each image keeps, reversed patches, new files,
//! a missing newline at the end, CRLF lines, whole-file replacements and quoted paths.

use super::*;

fn set(i: &[usize]) -> BTreeSet<usize> {
    i.iter().copied().collect()
}

fn build(lines: &[(char, &str)], sel: &[usize], reverse: bool, target: Patched, start: u32) -> Option<String> {
    let raw: Vec<(char, &[u8])> = lines.iter().map(|(o, t)| (*o, t.as_bytes())).collect();
    build_patch("f.txt", target, start, &raw, &set(sel), reverse).map(|b| String::from_utf8(b).unwrap())
}

const KEEP: Patched = Patched::Existing(0o100644);
const HEAD: &str = "diff --git a/f.txt b/f.txt\nindex 0000000..0000000 100644\n--- a/f.txt\n+++ b/f.txt\n";

#[test]
fn forward_one_of_two_additions() {
    let lines = [(' ', "a\n"), ('+', "b\n"), ('+', "c\n"), (' ', "d\n")];
    let p = build(&lines, &[2], false, KEEP, 4).unwrap();
    assert_eq!(p, format!("{HEAD}@@ -4,2 +4,3 @@\n-a\n-d\n+a\n+c\n+d\n"));
}

#[test]
fn removed_line_only_leaves_unselected_addition_out() {
    let lines = [(' ', "a\n"), ('-', "b\n"), ('+', "B\n")];
    let p = build(&lines, &[1], false, KEEP, 1).unwrap();
    assert_eq!(p, format!("{HEAD}@@ -1,2 +1,1 @@\n-a\n-b\n+a\n"));
}

#[test]
fn unselected_removal_stays_in_post_image() {
    let lines = [('-', "b\n"), ('+', "B\n")];
    let p = build(&lines, &[1], false, KEEP, 3).unwrap();
    assert_eq!(p, format!("{HEAD}@@ -3,1 +3,2 @@\n-b\n+b\n+B\n"));
}

#[test]
fn reverse_swaps_roles() {
    // Discarding `+B` from the working tree removes it; discarding `-b` brings it back.
    let lines = [(' ', "a\n"), ('-', "b\n"), ('+', "B\n")];
    let p = build(&lines, &[2], true, KEEP, 1).unwrap();
    assert_eq!(p, format!("{HEAD}@@ -1,2 +1,1 @@\n-a\n-B\n+a\n"));
    let p = build(&lines, &[1], true, KEEP, 1).unwrap();
    assert_eq!(p, format!("{HEAD}@@ -1,2 +1,3 @@\n-a\n-B\n+a\n+b\n+B\n"));
}

#[test]
fn empty_region_creates_a_new_file() {
    let lines = [('+', "a\n"), ('+', "b\n"), ('+', "c\n")];
    let p = build(&lines, &[0, 2], false, Patched::Create(0o100755), 0).unwrap();
    let head = "diff --git a/f.txt b/f.txt\nnew file mode 100755\n--- /dev/null\n+++ b/f.txt\n";
    assert_eq!(p, format!("{head}@@ -0,0 +1,2 @@\n+a\n+c\n"));
}

#[test]
fn nothing_effective_selected() {
    let lines = [(' ', "a\n"), ('+', "b\n")];
    assert_eq!(build(&lines, &[], false, KEEP, 1), None);
    assert_eq!(build(&lines, &[0], false, KEEP, 1), None, "context only");
}

#[test]
fn missing_newline_moves_to_the_real_last_line() {
    // Old "x\na", new "x\nb", neither ending in a newline: staging only `+b`.
    let lines =
        [(' ', "x\n"), ('-', "a"), ('>', "\n\\ No newline at end of file\n"), ('+', "b"), ('<', "\n\\ No newline at end of file\n")];
    let p = build(&lines, &[3], false, KEEP, 1).unwrap();
    let nl = "\\ No newline at end of file\n";
    assert_eq!(p, format!("{HEAD}@@ -1,2 +1,3 @@\n-x\n-a\n{nl}+x\n+a\n+b\n{nl}"));
}

#[test]
fn crlf_lines_keep_crlf() {
    let lines = [(' ', "a\r\n"), ('+', "b\r\n"), ('+', "c\r\n")];
    let p = build(&lines, &[1], false, KEEP, 1).unwrap();
    assert_eq!(p, format!("{HEAD}@@ -1,1 +1,2 @@\n-a\r\n+a\r\n+b\r\n"));
}

fn replace(old: &str, new: &str) -> Option<String> {
    replace_patch("f.txt", KEEP, old.as_bytes(), new.as_bytes()).map(|b| String::from_utf8(b).unwrap())
}

#[test]
fn replace_patch_whole_file() {
    assert_eq!(replace("a\nb\n", "a\nb\n"), None);
    assert_eq!(replace("", "a\nb\n").unwrap(), format!("{HEAD}@@ -0,0 +1,2 @@\n+a\n+b\n"));
    assert_eq!(replace("a\nb\n", "a\n").unwrap(), format!("{HEAD}@@ -1,2 +1,1 @@\n-a\n-b\n+a\n"));
    let nl = "\\ No newline at end of file\n";
    assert_eq!(replace("a", "a\n").unwrap(), format!("{HEAD}@@ -1,1 +1,1 @@\n-a\n{nl}+a\n"));
    assert_eq!(replace("a\n", "b").unwrap(), format!("{HEAD}@@ -1,1 +1,1 @@\n-a\n+b\n{nl}"));
    assert_eq!(replace("a\r\nb\r\n", "a\r\n").unwrap(), format!("{HEAD}@@ -1,2 +1,1 @@\n-a\r\n-b\r\n+a\r\n"));
}

#[test]
fn unusual_paths_are_quoted() {
    let raw: Vec<(char, &[u8])> = vec![('+', b"x\n")];
    let p = build_patch("say \"hi\".txt", KEEP, 0, &raw, &set(&[0]), false).unwrap();
    assert!(String::from_utf8(p).unwrap().starts_with("diff --git \"a/say \\\"hi\\\".txt\" \"b/say \\\"hi\\\".txt\"\n"));
    assert_eq!(quote_path("a/my file.txt"), "a/my file.txt");
    assert_eq!(quote_path("a/café"), "\"a/caf\\303\\251\"");
    assert_eq!(quote_path("a/back\\slash"), "\"a/back\\\\slash\"");
}
