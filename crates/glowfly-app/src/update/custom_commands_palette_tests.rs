//! The palette's custom command entries: the user's then the repository's commands shown in the
//! palette, with their details, and Reload custom commands.

use super::custom_commands_tests::{command, file, load, opened};
use super::tests::state;
use crate::custom_commands::{CommandRef, CustomCommandMsg, Target};
use crate::state::AppState;
use crate::state::palette::{PaletteCommand as C, entries};
use glowfly_core::custom_commands::Source;
use glowfly_core::settings::RepoTrust;

/// The custom entries (label, detail), Reload included.
fn custom(st: &AppState) -> Vec<(String, Option<String>)> {
    entries(st).into_iter().filter(|e| matches!(e.command, C::Custom(_))).map(|e| (e.label, e.detail)).collect()
}

fn pair(label: &str, detail: Option<&str>) -> (String, Option<String>) {
    (label.to_owned(), detail.map(str::to_owned))
}

#[test]
fn entries_list_the_users_then_the_repositorys_commands() {
    let (mut st, dir) = opened("cc-palette");
    let user = [
        command("Log", "git", "log", ""),
        command("Format", "shell", "fmt {files}", "show = [\"file\"]"),
        command("Diff", "git", "diff -- {files}", "show = [\"palette\", \"file\"]"),
    ]
    .concat();
    load(&mut st, file(&user), file(&command("Build", "shell", "make", "")));
    let want = |repo: &str| {
        vec![
            pair("Reload custom commands", None),
            pair("Log", Some("Your command")),
            pair("Diff", Some("Needs a file: open a file's diff first.")),
            pair("Build", Some(repo)),
        ]
    };
    assert_eq!(custom(&st), want("This repository's command, asks to trust it first"));
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    crate::state::trust::record(&mut st.settings, &real, RepoTrust::Trusted);
    assert_eq!(custom(&st), want("This repository's command"));
    let run = entries(&st).into_iter().find(|e| e.label == "Log").map(|e| e.command);
    let log = CommandRef { source: Source::User, name: "Log".into() };
    assert_eq!(run, Some(C::Custom(CustomCommandMsg::Run { command: log, at: Target::Palette, confirmed: false })));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_without_a_repository_and_nothing_remembered() {
    assert!(custom(&state()).is_empty());
    assert_eq!(C::Custom(CustomCommandMsg::Reload).recall_key(), None);
    let run =
        CustomCommandMsg::Run { command: CommandRef { source: Source::User, name: "x".into() }, at: Target::Palette, confirmed: false };
    assert_eq!(C::Custom(run).recall_key(), None);
}
