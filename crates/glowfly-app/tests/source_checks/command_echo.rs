//! Every `$ git …` echo in a job's output is built by glowfly-core's `text::command_echo`. A hand
//! join (`format!("$ git {}", args.join(" "))`) writes an argument with a space or a line break
//! as if it were several arguments, can split one echo over several lines (which the readers of
//! the log count) and may forget to hide the credentials in a URL.
//!
//! Two shapes are flagged: a format string that puts arguments straight after `git`
//! (`"$ git {}\n"`, `"\n$ git {shown}"`), and a literal that ends with `$ git ` unless it is read
//! (`starts_with(` / `strip_prefix(`), which is an echo's start glued to hand-joined arguments.
//! Test code counts too; comments don't.

use crate::hygiene::all_sources;
use crate::lexer::{Kind, literals};

/// Sources left alone on purpose: (path, why). The list only shrinks; an entry that no longer
/// matches fails.
const ALLOWED: &[(&str, &str)] = &[];

/// The 1-based lines of `src` that build an echo by hand.
fn hand_joined(src: &str) -> Vec<usize> {
    literals(src)
        .into_iter()
        .filter(|l| matches!(l.piece.kind, Kind::Str))
        .filter(|l| {
            let glued = l.text.ends_with("$ git ") && {
                let before = src[..l.piece.span.start].trim_end();
                !before.ends_with("starts_with(") && !before.ends_with("strip_prefix(")
            };
            l.text.contains("$ git {") || glued
        })
        .map(|l| l.line)
        .collect()
}

#[test]
fn git_echoes_are_built_by_command_echo() {
    let mut problems = Vec::new();
    let mut used = Vec::new();
    for source in all_sources() {
        let lines = hand_joined(&source.text);
        if lines.is_empty() {
            continue;
        }
        if ALLOWED.iter().any(|(p, _)| *p == source.path) {
            used.push(source.path.clone());
        } else {
            problems.extend(lines.iter().map(|line| format!("{}:{line}", source.path)));
        }
    }
    assert!(
        problems.is_empty(),
        "build the `$ git …` echo with glowfly_core::text::command_echo(&args), which quotes arguments with spaces and hides credentials: {}",
        problems.join(", ")
    );
    let stale: Vec<String> = ALLOWED
        .iter()
        .filter(|(p, _)| !used.contains(&(*p).to_owned()))
        .map(|(p, _)| format!("{p} no longer builds an echo by hand; remove it from ALLOWED in command_echo.rs"))
        .collect();
    assert!(stale.is_empty(), "{}", stale.join("\n"));
}

#[test]
fn the_echo_check_tells_a_hand_join_from_a_reader() {
    let flagged = [
        r#"let s = format!("$ git {}\n", c.join(" "));"#,
        r#"out.push_str(&format!("\n$ git {shown}"));"#,
        r#"out.push_str("$ git "); out.push_str(&c.join(" "));"#,
    ];
    for src in flagged {
        assert_eq!(hand_joined(src), [1], "{src}");
    }
    let fine = [
        r#"let n = l.starts_with("$ git ");"#,
        r#"let d = l.strip_prefix("$ git diff ");"#,
        r#"let w = format!("from\n$ git diff {from}\noutput");"#,
        r#"let e = glowfly_core::text::command_echo(&c);"#,
        r#"// format!("$ git {}", x)"#,
    ];
    for src in fine {
        assert!(hand_joined(src).is_empty(), "{src}");
    }
}
