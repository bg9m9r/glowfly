//! Custom commands: loading both commands files with each full load and when `.trellis/`
//! changes (older answers dropped), announcing their problems once, and their keys. Running
//! them is in custom_commands_run_tests.rs, the palette in custom_commands_palette_tests.rs.

use super::tests::{finish_keeping, scratch_repo, state};
use super::update;
use crate::custom_commands::{CommandRef, CustomCommandEffect, CustomCommandMsg};
use crate::effect::Effect;
use crate::keymap::{BindingId, Chord};
use crate::msg::{Msg, PaletteMsg, RepoMsg, WatchMsg};
use crate::state::AppState;
use crate::state::palette::PaletteCommand;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::custom_commands::{FileRead, Source, parse};

/// The TOML of one command.
pub(super) fn command(name: &str, kind: &str, template: &str, extra: &str) -> String {
    format!("[[command]]\nname = \"{name}\"\nkind = \"{kind}\"\ntemplate = '{template}'\n{extra}\n")
}

/// What a file holding `text` reads as.
pub(super) fn file(text: &str) -> FileRead {
    let (commands, problems) = parse(text);
    FileRead { path: None, commands, problems }
}

/// `scratch_repo` opened in a fresh state; the commands load it asked for is not run.
pub(super) fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    let fx = update(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())), Instant::now());
    finish_keeping(&mut st, fx);
    assert!(st.snap.is_some());
    (st, dir)
}

/// The load `fx` asks for, if any.
pub(super) fn load_of(fx: &[Effect]) -> Option<(u64, PathBuf)> {
    fx.iter().find_map(|e| match e {
        Effect::CustomCommand(CustomCommandEffect::Load { generation, workdir }) => Some((*generation, workdir.clone())),
        _ => None,
    })
}

/// Answers the latest load with these files.
pub(super) fn load(st: &mut AppState, user: FileRead, repo: FileRead) -> Vec<Effect> {
    let msg = CustomCommandMsg::Loaded { generation: st.custom_commands.generation, workdir: st.workdir().unwrap(), user, repo };
    update(st, Msg::CustomCommand(msg), Instant::now())
}

#[test]
fn loads_follow_full_loads_and_drop_older_answers() {
    let dir = scratch_repo("cc-load");
    let mut st = state();
    let fx = update(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())), Instant::now());
    let kept = finish_keeping(&mut st, fx);
    let workdir = st.workdir().unwrap();
    assert_eq!(load_of(&kept), Some((1, workdir.clone())), "{kept:?}");
    let fx = update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    let kept = finish_keeping(&mut st, fx);
    assert_eq!(load_of(&kept), Some((2, workdir.clone())), "a refresh reads them again");
    let text = command("Log", "git", "log", "");
    let old = CustomCommandMsg::Loaded { generation: 1, workdir: workdir.clone(), user: file(&text), repo: FileRead::default() };
    update(&mut st, Msg::CustomCommand(old), Instant::now());
    assert!(st.custom_commands.user.is_empty(), "an older load is dropped");
    let other = CustomCommandMsg::Loaded { generation: 2, workdir: dir.join("elsewhere"), user: file(&text), repo: FileRead::default() };
    update(&mut st, Msg::CustomCommand(other), Instant::now());
    assert!(st.custom_commands.user.is_empty(), "a load for another folder is dropped");
    load(&mut st, file(&text), FileRead::default());
    assert_eq!(st.custom_commands.user.len(), 1);
    assert!(st.custom_commands.repo_for.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn problems_are_announced_once() {
    let (mut st, dir) = opened("cc-problems");
    let user_path = dir.join("my-commands.toml");
    let bad = format!("{}{}", command("A", "git", "log {brnach}", ""), command("B", "shell", "echo \"{sha}\"", ""));
    let user = || FileRead { path: Some(user_path.clone()), ..file(&bad) };
    let repo = || file("[[command]]\nname = 1\n");
    load(&mut st, user(), repo());
    let shown = trellis_core::native_path::shown(&user_path);
    let lines: Vec<&str> = st.log.iter().map(|l| l.text.as_str()).collect();
    assert!(lines.iter().any(|l| l.starts_with(&format!("{shown}, line 4: {{brnach}} isn't a placeholder"))), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with(&format!("{shown}, line 9: Remove the quotes around {{sha}}"))), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with(".trellis/commands.toml, line 2: ")), "{lines:?}");
    assert_eq!(st.toasts.len(), 1);
    assert_eq!(st.toasts[0].text, "3 problems in your custom commands. See the activity log.");
    let logged = st.log.len();
    load(&mut st, user(), repo());
    assert_eq!((st.log.len(), st.toasts.len()), (logged, 1), "the same problems aren't announced again");
    load(&mut st, file(""), FileRead::default());
    assert!(st.custom_commands.reported.is_empty(), "fixed files clear them");
    load(&mut st, user(), repo());
    assert_eq!(st.toasts.len(), 2, "problems that come back are announced again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn changes_under_trellis_and_reload_read_the_files_again() {
    let (mut st, dir) = opened("cc-watch");
    let changed = |st: &mut AppState, paths: &[&str]| {
        let paths = Some(paths.iter().map(|p| p.to_string()).collect());
        update(st, Msg::Watch(WatchMsg::RepoChanged { full: false, paths, last_event: Instant::now() }), Instant::now())
    };
    let before = st.custom_commands.generation;
    assert_eq!(load_of(&changed(&mut st, &[".trellis/commands.toml"])).map(|(g, _)| g), Some(before + 1));
    assert_eq!(load_of(&changed(&mut st, &["src/main.rs"])), None);
    let reload = PaletteMsg::RunPaletteCommand(PaletteCommand::Custom(CustomCommandMsg::Reload));
    assert!(load_of(&update(&mut st, Msg::Palette(reload), Instant::now())).is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keys_are_claimed_once_and_checked() {
    let (mut st, dir) = opened("cc-keys");
    let taken = st.keymap.chords(BindingId::NewTab)[0].config();
    let text = [
        command("Log", "git", "log", "key = \"Mod+Alt+L\""),
        command("Plain", "git", "status", "key = \"L\""),
        command("Taken", "git", "status", &format!("key = \"{taken}\"")),
        command("Again", "git", "status", "key = \"Mod+Alt+L\""),
        command("Odd", "git", "status", "key = \"Mod+Banana\""),
    ]
    .concat();
    load(&mut st, file(&text), FileRead::default());
    let mod_alt_l = Chord::parse("Mod+Alt+L").unwrap();
    let keys = &st.custom_commands.keys;
    assert_eq!(keys.len(), 1, "{:?}", keys.iter().map(|k| &k.1).collect::<Vec<_>>());
    assert_eq!(keys[0], (mod_alt_l, CommandRef { source: Source::User, name: "Log".into() }, mod_alt_l.text()));
    let lines: Vec<&str> = st.log.iter().map(|l| l.text.as_str()).collect();
    let has = |s: &str| lines.iter().any(|l| l.contains(s));
    assert!(has("line 10: A command's key needs Mod, Ctrl or Alt; L was ignored."), "{lines:?}");
    let new_tab = Chord::parse(&taken).unwrap().text();
    assert!(has(&format!("line 15: {new_tab} is already used by New tab, so it was ignored.")), "{lines:?}");
    assert!(has(&format!("line 20: {} is already used by Log, so it was ignored.", mod_alt_l.text())), "{lines:?}");
    assert!(has("line 25: Mod+Banana isn't a key Trellis knows"), "{lines:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
