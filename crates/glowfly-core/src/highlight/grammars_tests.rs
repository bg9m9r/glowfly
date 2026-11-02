//! The parse set's holder: it drops the set (and what was compiled into it) once idle, loads it
//! again on the next highlight, and a reloaded set colours exactly as the first one did.

use super::file::highlight_file;
use super::grammars::{GRAMMARS, Holder, IDLE, Kept, grammars};
use super::parse::{Classifier, Side};
use super::syntaxes::{find_syntax, load_set};
use super::{Span, SyntaxHint};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

const MIN: Duration = Duration::from_secs(60);

#[test]
fn the_value_is_kept_while_used_and_dropped_once_idle() {
    let loads = AtomicUsize::new(0);
    let load = || {
        loads.fetch_add(1, Ordering::SeqCst);
        vec![0u8; 1024]
    };
    let holder = Holder::new();
    let t0 = Instant::now();
    assert_eq!(holder.release_idle(t0, IDLE), Kept::Empty);
    let (first, loaded) = holder.get(t0, load);
    assert!(loaded);
    let weak: Weak<Vec<u8>> = Arc::downgrade(&first);
    drop(first);
    // Used again a minute later: the same value, and the idle time counts from then.
    let (again, loaded) = holder.get(t0 + MIN, load);
    assert!(!loaded && Weak::ptr_eq(&weak, &Arc::downgrade(&again)));
    drop(again);
    assert_eq!(holder.release_idle(t0 + IDLE, IDLE), Kept::Held, "only {:?} since the last use", IDLE - MIN);
    assert_eq!(holder.release_idle(t0 + MIN + IDLE, IDLE), Kept::Released);
    assert!(weak.upgrade().is_none(), "the value is freed, not just forgotten");
    assert_eq!(holder.release_idle(t0 + MIN + IDLE, IDLE), Kept::Empty);
    let (_, loaded) = holder.get(t0 + 2 * IDLE, load);
    assert!(loaded);
    assert_eq!(loads.load(Ordering::SeqCst), 2, "loaded again after the drop, and only then");
}

#[test]
fn a_pass_running_at_the_drop_keeps_its_value_until_it_ends() {
    let holder = Holder::new();
    let t0 = Instant::now();
    let (running, _) = holder.get(t0, || String::from("grammars"));
    let weak = Arc::downgrade(&running);
    assert_eq!(holder.release_idle(t0 + IDLE, IDLE), Kept::Released);
    assert_eq!(running.as_str(), "grammars");
    drop(running);
    assert!(weak.upgrade().is_none());
}

/// `lines` parsed with `grammars`, continuing from `side`.
fn parse(grammars: Arc<syntect::parsing::SyntaxSet>, side: &mut Side, lines: &[&str]) -> Vec<Vec<Span>> {
    let mut cx = Classifier::on(grammars);
    lines.iter().map(|l| side.line(l, &mut cx, &|| false).unwrap_or_else(|_| panic!("{l}"))).collect()
}

const SAMPLES: [(&str, &str); 3] = [
    ("src/lib.rs", "/* a block\n   comment */\nfn main() {\n    let s = \"hi\"; // x\n    let n = 0x1f;\n}\n"),
    ("README.md", "# Title\n\nSome *text* and `code`.\n\n```rust\nfn f() -> u8 { 1 }\n```\n\n- a [link](http://x)\n"),
    ("ci.yml", "on:\n  push:\n    branches: [main] # c\njobs:\n  a: 'b'\n  n: 3\n"),
];

#[test]
fn a_reloaded_set_colours_the_same_and_resumes_states_from_the_old_one() {
    for (path, text) in SAMPLES {
        let lines: Vec<&str> = text.lines().collect();
        let syntax = find_syntax(path.into(), None).expect(path);
        // Detection's syntax reference starts a parse on a separately loaded set.
        let whole = parse(Arc::new(load_set()), &mut Side::new(syntax), &lines);
        assert!(whole.iter().flatten().any(|s| s.kind != super::TokenKind::Plain), "{path}: {whole:?}");
        // A state (as a checkpoint in `memo` keeps it) carried from one set into a fresh one.
        let mid = lines.len() / 2;
        let mut side = Side::new(syntax);
        let mut split = parse(Arc::new(load_set()), &mut side, &lines[..mid]);
        split.extend(parse(Arc::new(load_set()), &mut side, &lines[mid..]));
        assert_eq!(split, whole, "{path}");
    }
}

#[test]
fn highlighting_after_a_drop_loads_a_new_set_with_the_same_colours() {
    let highlight = |(path, text): (&str, &str)| {
        let lines: Vec<String> = text.lines().map(str::to_owned).collect();
        highlight_file(SyntaxHint { path, language: None }, &lines).expect(path)
    };
    let before: Vec<_> = SAMPLES.into_iter().map(highlight).collect();
    let old = grammars();
    assert_eq!(GRAMMARS.release_idle(Instant::now() + 2 * IDLE, IDLE), Kept::Released);
    let after: Vec<_> = SAMPLES.into_iter().map(highlight).collect();
    assert!(!Arc::ptr_eq(&old, &grammars()), "a new set was loaded");
    assert_eq!(after, before);
}
