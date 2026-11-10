//! Which steps left in a rebase, cherry-pick or revert in progress the scan lists: the todo's
//! `exec` steps and a merge strategy that isn't git's own, how a decision covers them, the
//! options file read against git's own config parser, and files that can't be read.

use super::super::{allows, scan_at, stamp};
use super::{FILES, MAX_BYTES, STRATEGIES};
use crate::settings::SavedTrust;
use crate::testutil::*;
use std::path::Path;

/// Writes `text` as the file `name` in the git dir of the repository at `dir`.
fn state_file(dir: &Path, name: &str, text: &[u8]) {
    let path = dir.join(".git").join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn steps(dir: &Path) -> Vec<String> {
    scan_at(dir).unwrap().steps
}

#[test]
fn a_todo_with_exec_steps_is_a_reason_to_ask() {
    let dir = scratch_repo("trust-steps-exec");
    let todo = "pick 1234567 one\nexec make test\n\tx\t touch done.txt\r\n# exec in a comment\nbreak\nexec make test\n";
    state_file(&dir, "rebase-merge/git-rebase-todo", todo.as_bytes());
    let s = scan_at(&dir).unwrap();
    assert_eq!(s.steps, ["exec make test", "exec touch done.txt"]);
    assert!(!s.is_empty());
    assert!(!allows(None, &s), "nothing was decided, and continuing runs a command");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_todo_without_exec_steps_runs_nothing() {
    let dir = scratch_repo("trust-steps-none");
    let todo = "pick 1234567 exec is only a word here\nreword 89abcde x\nlabel exec\nbreak\nupdate-ref refs/heads/x\n# exec make\n";
    state_file(&dir, "rebase-merge/git-rebase-todo", todo.as_bytes());
    state_file(&dir, "sequencer/todo", b"pick 1234567 one\nrevert 89abcde two\n");
    state_file(&dir, "rebase-merge/strategy", b"ort\n");
    state_file(&dir, "sequencer/opts", b"[options]\n\tstrategy = \"recursive\"\n\tstrategy-option = theirs\n\tmainline = 1\n");
    let s = scan_at(&dir).unwrap();
    assert!(s.steps.is_empty() && s.is_empty(), "{s:?}");
    assert!(allows(None, &s));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_pick_sequences_exec_steps_and_bytes_that_arent_utf8_are_listed() {
    let dir = scratch_repo("trust-steps-sequencer");
    state_file(&dir, "sequencer/todo", b"pick 1234567 one\nexec sh \xff.sh\nexec\n");
    assert_eq!(steps(&dir), ["exec sh \u{FFFD}FF.sh", "exec "]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_merge_strategy_that_isnt_gits_own_is_listed() {
    let dir = scratch_repo("trust-steps-strategy");
    state_file(&dir, "rebase-merge/strategy", b"mine\n");
    assert_eq!(steps(&dir), ["--strategy=mine"]);
    // Git compares the name as written: another letter case or a space is another program.
    state_file(&dir, "rebase-merge/strategy", b" ORT\n");
    assert_eq!(steps(&dir), ["--strategy= ORT"]);
    state_file(&dir, "rebase-merge/strategy", b"");
    assert_eq!(steps(&dir), ["--strategy="]);
    std::fs::remove_dir_all(dir.join(".git/rebase-merge")).unwrap();
    state_file(&dir, "sequencer/opts", b"[options] strategy = ort\n[options]\n\tStrategy=theirs-first\n");
    assert_eq!(steps(&dir), ["--strategy=theirs-first"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_decision_covers_the_steps_it_was_made_on_and_asks_again_for_a_new_one() {
    let dir = scratch_repo("trust-steps-print");
    state_file(&dir, "rebase-merge/git-rebase-todo", b"exec make test\npick 1234567 one\n");
    let seen = scan_at(&dir).unwrap();
    let saved = SavedTrust::trusted(Some(seen.print()));
    assert!(allows(Some(&saved), &scan_at(&dir).unwrap()), "nothing changed since the decision");
    // The step ran and left the todo: still covered.
    state_file(&dir, "rebase-merge/git-rebase-todo", b"pick 1234567 one\n");
    assert!(allows(Some(&saved), &scan_at(&dir).unwrap()));
    // The todo gained a command since.
    state_file(&dir, "rebase-merge/git-rebase-todo", b"exec make test\npick 1234567 one\nexec make install\n");
    let now = scan_at(&dir).unwrap();
    assert!(!allows(Some(&saved), &now));
    let gained = now.gained_since(&seen.print());
    assert_eq!(gained.steps, ["exec make install"]);
    assert!(!gained.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_steps_text_is_kept_byte_for_byte() {
    let dir = scratch_repo("trust-steps-bytes");
    state_file(&dir, "rebase-merge/git-rebase-todo", b"exec ./build\n");
    let plain = scan_at(&dir).unwrap();
    assert_eq!(plain.steps, ["exec ./build"]);
    // A no-break space, a form feed or a space after it is part of the command to sh.
    for tail in ["\u{a0}", "\x0c", " ", "\u{2028}"] {
        state_file(&dir, "rebase-merge/git-rebase-todo", format!("exec ./build{tail}\n").as_bytes());
        let other = scan_at(&dir).unwrap();
        assert_eq!(other.steps, [format!("exec ./build{tail}")]);
        assert!(!plain.print().covers(&other.print()) && !other.print().covers(&plain.print()), "{tail:?}");
    }
    // Only spaces and tabs go before the line and after the keyword, and one CR at the end.
    state_file(&dir, "rebase-merge/git-rebase-todo", b" \texec \t ./build\r\nexec\x0c./build\n\x0cexec ./build\nx\r./build\r\r\n");
    assert_eq!(steps(&dir), ["exec ./build", "exec \r./build\r"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The values `git config -f <file> --list` gives `options.strategy`; None when git refuses
/// the file.
fn gits_strategies(dir: &Path) -> Option<Vec<String>> {
    let (ok, text) = crate::gitcli::run(dir, &crate::gitcli::args(&["config", "-f", ".git/sequencer/opts", "--list"]));
    ok.then(|| text.lines().filter_map(|l| l.strip_prefix("options.strategy=")).map(str::to_owned).collect())
}

#[test]
fn the_options_file_is_read_at_least_as_git_reads_it() {
    let dir = scratch_repo("trust-steps-opts");
    let listed: [&[u8]; 12] = [
        b"[a][options] strategy = mine\n",
        b"\xEF\xBB\xBF[options]\n\tstrategy = mine\n",
        b"[options]\n\tstrategy = mi\\\nne\n",
        b"[options]\n\tstrategy = \"mine\"\n",
        b"[options]\n\tstrategy=mine\n",
        b"[options]\n\t\tStrategy\t=\tmine\n",
        b"[options]\r\n\tstrategy = mine\r\n",
        b"[options] strategy = mine\n",
        b"[a \"=\"][options] strategy = mine\n",
        b"[options]\n\tstrategy = or\"t\"x\n",
        b"[options]\n\tstrategy = ort\n\tstrategy = mine\n",
        b"[options]\n\tstrategy = ort\\\nx\n",
    ];
    for bytes in listed {
        state_file(&dir, "sequencer/opts", bytes);
        let name = String::from_utf8_lossy(bytes).into_owned();
        assert!(!steps(&dir).is_empty(), "{name:?}");
        // The forms above are ones git reads a strategy of its own from, too.
        let git = gits_strategies(&dir).unwrap_or_else(|| panic!("git refused {name:?}"));
        assert!(git.iter().any(|s| !STRATEGIES.contains(&s.as_str())), "{name:?}: {git:?}");
    }
    let unlisted: [&[u8]; 7] = [
        b"[options]\n\tstrategy = ort\n",
        b"\xEF\xBB\xBF[a][options] strategy = \"recursive\"\n",
        b"[options]\r\n\tstrategy=resolve\r\n",
        b"[options]\n\t\tSTRATEGY\t=\toctopus\t\n",
        b"[options] strategy = ours\n",
        b"[options]\n\tstrategy-option = mine\n\tmainline = 1\n",
        b"[options]\n\tstrategy = subtree\n\tgpg-sign = strategy\n",
    ];
    for bytes in unlisted {
        state_file(&dir, "sequencer/opts", bytes);
        let name = String::from_utf8_lossy(bytes).into_owned();
        assert_eq!(steps(&dir), [""; 0], "{name:?}");
        let git = gits_strategies(&dir).unwrap_or_else(|| panic!("git refused {name:?}"));
        assert!(git.iter().all(|s| STRATEGIES.contains(&s.as_str())), "{name:?}: {git:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_strategy_file_with_more_than_a_name_and_a_line_ending_is_listed() {
    let dir = scratch_repo("trust-steps-strategy-bytes");
    for (bytes, listed) in
        [(&b"ort"[..], false), (b"ort\n", false), (b"ort\r\n", false), (b"ort\n\n", true), (b"ort \n", true), (b"\tort\n", true)]
    {
        state_file(&dir, "rebase-merge/strategy", bytes);
        assert_eq!(!steps(&dir).is_empty(), listed, "{bytes:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_that_cant_be_read_whole_is_a_reason_to_ask() {
    let dir = scratch_repo("trust-steps-unread");
    for name in FILES {
        // Larger than is read: listed by name, and again when another takes its place.
        let mut big = vec![b'#'; MAX_BYTES as usize + 1];
        state_file(&dir, name, &big);
        let first = scan_at(&dir).unwrap();
        assert_eq!(first.steps.len(), 1, "{name}");
        assert!(first.steps[0].starts_with(&format!("{name} ")) && !first.is_empty() && !allows(None, &first), "{first:?}");
        big.push(b'#');
        state_file(&dir, name, &big);
        assert!(!first.print().covers(&scan_at(&dir).unwrap().print()), "{name}");
        // A folder in its place.
        std::fs::remove_file(dir.join(".git").join(name)).unwrap();
        std::fs::create_dir(dir.join(".git").join(name)).unwrap();
        let folder = steps(&dir);
        assert!(folder.len() == 1 && folder[0].starts_with(&format!("{name} ")), "{folder:?}");
        std::fs::remove_dir(dir.join(".git").join(name)).unwrap();
        assert!(scan_at(&dir).unwrap().is_empty(), "{name}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_stamp_changes_with_each_file_read() {
    let dir = scratch_repo("trust-steps-stamp");
    let repo = git2::Repository::open(&dir).unwrap();
    let scan = scan_at(&dir).unwrap();
    for name in FILES {
        let before = stamp(&repo, &scan);
        state_file(&dir, name, b"ort\n");
        let made = stamp(&repo, &scan);
        assert_ne!(before, made, "{name} appeared");
        state_file(&dir, name, b"mine and more\n");
        assert_ne!(made, stamp(&repo, &scan), "{name} changed");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
