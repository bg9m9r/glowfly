//! THIRD-PARTY-NOTICES.md lists the licenses of the bundled syntax definitions. Run with
//! `UPDATE_NOTICES=1` to regenerate that list after updating two-face.

use std::path::Path;

const BEGIN: &str = "<!-- BEGIN GENERATED SYNTAX LICENSES -->\n";
const END: &str = "<!-- END GENERATED SYNTAX LICENSES -->";

fn generated() -> String {
    let mut md = String::new();
    for license in two_face::acknowledgement::listing().for_syntaxes() {
        license.write_md(&mut md);
    }
    md.replace("\r\n", "\n")
}

#[test]
fn third_party_notice_lists_bundled_syntax_licenses() {
    // Read at run time: a binary built in another worktree (shared target dir) keeps that
    // worktree's compile-time path.
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").into());
    let path = Path::new(&dir).join("../../THIRD-PARTY-NOTICES.md");
    let notice = std::fs::read_to_string(&path).expect("THIRD-PARTY-NOTICES.md exists").replace("\r\n", "\n");
    assert!(notice.contains("PowerShell.sublime-syntax"), "the notice names Trellis's own grammar");
    let begin = notice.find(BEGIN).expect("the notice has the BEGIN marker") + BEGIN.len();
    let end = notice.find(END).expect("the notice has the END marker");
    assert!(begin <= end, "BEGIN comes before END");
    let want = generated();
    if std::env::var_os("UPDATE_NOTICES").is_some() {
        let updated = format!("{}{want}{}", &notice[..begin], &notice[end..]);
        std::fs::write(&path, updated).expect("write THIRD-PARTY-NOTICES.md");
        return;
    }
    assert!(notice[begin..end] == want, "the syntax licenses are out of date: run UPDATE_NOTICES=1 cargo test -p glowfly-core notice");
}
