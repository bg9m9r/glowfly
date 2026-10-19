//! Headless tests for the external editor and a custom terminal: the launches planned from
//! Preferences > External tools, Preferences opening when no editor is set, the refusals and
//! failures (log line and red toast), the program
//! pickers, Open in terminal with and without a custom terminal, and the palette entries.

use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, PaletteMsg, RepoMsg};
use crate::state::AppState;
use crate::state::PrefPage;
use crate::state::palette::{PaletteCommand as C, entries};
use crate::tools::{LaunchFailure, ToolEffect, ToolKind, ToolsMsg};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::external_tools::Launch;
use trellis_core::native_path::native;

fn run(st: &mut AppState, m: ToolsMsg) -> Vec<Effect> {
    update(st, Msg::Tools(m), Instant::now())
}

/// `scratch_repo` open in a fresh state, with `editor` as the editor program.
fn open(name: &str, editor: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.is_some());
    st.settings.external_tools.editor = editor.into();
    (st, dir)
}

fn open_file(path: &str, line: Option<u32>) -> ToolsMsg {
    ToolsMsg::OpenFile { path: path.into(), line }
}

/// The launch of the only effect, which must be a Launch of `kind`; also returns its check.
fn launch(fx: Vec<Effect>, kind: ToolKind) -> (Launch, Option<PathBuf>) {
    match <[Effect; 1]>::try_from(fx) {
        Ok([Effect::Tool(ToolEffect::Launch { kind: k, launch, check, .. })]) if k == kind => (launch, check),
        Ok(other) => panic!("expected one {kind:?} launch, got {other:?}"),
        Err(fx) => panic!("expected one {kind:?} launch, got {fx:?}"),
    }
}

/// The arguments for `v`, as they are filled in: in this system's spelling (`C:\x\y`, not
/// libgit2's `C:/x/y/`).
fn os(v: &[&Path]) -> Vec<OsString> {
    v.iter().map(|p| native(p).into_os_string()).collect()
}

/// The last log line and toast, which must both be failures with the same text.
fn failure(st: &AppState) -> String {
    let log = st.log.last().expect("a log line");
    let toast = st.toasts.last().expect("a toast");
    assert!(!log.ok && !toast.ok);
    assert_eq!(log.text, toast.text);
    log.text.clone()
}

/// Whether `fx` launches anything.
fn launches(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Tool(ToolEffect::Launch { .. })))
}

#[test]
fn opening_without_an_editor_opens_preferences() {
    let (mut st, dir) = open("tools-no-editor", "");
    let fx = run(&mut st, open_file("a.txt", None));
    assert!(!launches(&fx), "{fx:?}");
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::Tools));
    assert!(st.toasts.is_empty(), "no failure toast");
    // Only whitespace or an empty pair of quotes is no editor either.
    st.prefs = None;
    st.settings.external_tools.editor = " \"\" ".into();
    let fx = run(&mut st, ToolsMsg::OpenRepo);
    assert!(!launches(&fx), "{fx:?}");
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::Tools));
    assert!(st.toasts.is_empty(), "no failure toast");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn launch_failures_still_point_to_preferences() {
    let mut st = state();
    let failed = ToolsMsg::LaunchFailed {
        kind: ToolKind::DiffTool,
        program: "meld".into(),
        target: "a.txt".into(),
        failure: LaunchFailure::NotFound,
    };
    assert!(run(&mut st, failed).is_empty());
    failure(&st);
    let action = st.toasts.last().and_then(|t| t.action.as_ref()).map(|a| a.label.as_str());
    assert_eq!(action, Some("Open preferences"));
}

#[test]
fn opening_a_file_passes_its_absolute_path_as_one_argument() {
    let (mut st, dir) = open("tools-open-file", "code");
    let w = st.workdir().unwrap();
    let (l, check) = launch(run(&mut st, open_file("my file.txt", None)), ToolKind::Editor);
    let file = w.join("my file.txt");
    assert_eq!(l, Launch { program: "code".into(), args: os(&[&file]), cwd: w.clone(), own_console: false });
    assert_eq!(check, Some(file));
    assert!(st.log.is_empty() && st.toasts.is_empty(), "success is silent until the runner reports");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn custom_file_arguments_fill_in_the_line() {
    let (mut st, dir) = open("tools-file-args", "\"C:/Tools/Ed/ed.exe\"");
    st.settings.external_tools.editor_file_args = r#"--goto "{path}:{line}""#.into();
    let w = st.workdir().unwrap();
    let at = |line: u32| OsString::from(format!("{}:{line}", native(&w.join("src/a.rs")).display()));
    let (l, _) = launch(run(&mut st, open_file("src/a.rs", None)), ToolKind::Editor);
    assert_eq!(l.program, PathBuf::from("C:/Tools/Ed/ed.exe"), "the quotes are not part of the program");
    assert_eq!(l.args, [OsString::from("--goto"), at(1)]);
    let (l, _) = launch(run(&mut st, open_file("src/a.rs", Some(42))), ToolKind::Editor);
    assert_eq!(l.args, [OsString::from("--goto"), at(42)]);

    st.settings.external_tools.editor_file_args = r#"--goto "{path}"#.into();
    assert!(run(&mut st, open_file("src/a.rs", None)).is_empty());
    assert!(failure(&st).contains("Open file arguments"), "{}", failure(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_the_repository_passes_its_folder() {
    let (mut st, dir) = open("tools-open-repo", "code");
    let w = st.workdir().unwrap();
    let (l, check) = launch(run(&mut st, ToolsMsg::OpenRepo), ToolKind::Editor);
    assert_eq!(l, Launch { program: "code".into(), args: os(&[&w]), cwd: w.clone(), own_console: false });
    assert_eq!(check, None);
    st.settings.external_tools.editor_repo_args = "\"".into();
    assert!(run(&mut st, ToolsMsg::OpenRepo).is_empty());
    assert!(failure(&st).contains("Open repository arguments"));
    let _ = std::fs::remove_dir_all(&dir);

    // Without a repository nothing is launched.
    let mut st = state();
    st.settings.external_tools.editor = "code".into();
    assert!(run(&mut st, ToolsMsg::OpenRepo).is_empty());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text.contains("Open a repository")));
    assert!(run(&mut st, open_file("a.txt", None)).is_empty());
}

#[test]
fn open_in_terminal_uses_the_custom_terminal_when_set() {
    let (mut st, dir) = open("tools-terminal", "");
    let w = st.workdir().unwrap();
    let fx = update(&mut st, Msg::OpenTerminal, Instant::now());
    assert!(matches!(&fx[..], [Effect::OpenTerminal(p)] if *p == w), "detected as before: {fx:?}");

    st.settings.external_tools.terminal = "wt".into();
    st.settings.external_tools.terminal_args = r#"-d "{repo}""#.into();
    let (l, check) = launch(update(&mut st, Msg::OpenTerminal, Instant::now()), ToolKind::Terminal);
    assert_eq!(
        l,
        Launch { program: "wt".into(), args: vec![OsString::from("-d"), native(&w).into_os_string()], cwd: w, own_console: true }
    );
    assert_eq!(check, None);

    st.settings.external_tools.terminal_args = "-d \"{repo}".into();
    assert!(update(&mut st, Msg::OpenTerminal, Instant::now()).is_empty(), "no fallback to the detected terminal");
    assert!(failure(&st).contains("Terminal arguments"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn open_in_terminal_can_use_the_panel() {
    let (mut st, dir) = open("tools-terminal-panel", "");
    st.settings.terminal.open_in_panel = true;
    st.settings.external_tools.terminal = "wt".into();
    let spawns = |fx: &[Effect]| fx.iter().filter(|e| matches!(e, Effect::Terminal(crate::terminal::TerminalEffect::Spawn { .. }))).count();
    let external = |fx: &[Effect]| fx.iter().any(|e| matches!(e, Effect::OpenTerminal(_) | Effect::Tool(_)));
    let fx = update(&mut st, Msg::OpenTerminal, Instant::now());
    assert_eq!(spawns(&fx), 1, "{fx:?}");
    assert!(!external(&fx));
    assert!(st.terminal.open);
    // The palette's Open in terminal does the same: a second tab.
    let fx = update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::OpenTerminal)), Instant::now());
    assert_eq!(spawns(&fx), 1, "{fx:?}");
    assert!(!external(&fx));
    assert_eq!(st.terminal.sessions.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn browsing_picks_and_saves_a_program() {
    let mut st = state();
    assert!(matches!(&run(&mut st, ToolsMsg::Browse(ToolKind::Editor))[..], [Effect::Tool(ToolEffect::PickProgram(ToolKind::Editor))]));
    let picked = PathBuf::from("C:/Tools/Ed/ed.exe");
    let fx = run(&mut st, ToolsMsg::ProgramPicked { kind: ToolKind::Editor, path: Some(picked.clone()) });
    assert_eq!(st.settings.external_tools.editor, picked.display().to_string());
    assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if s.external_tools.editor == picked.display().to_string()));
    assert!(run(&mut st, ToolsMsg::ProgramPicked { kind: ToolKind::Terminal, path: None }).is_empty());
    assert!(st.settings.external_tools.terminal.is_empty(), "a cancelled dialog changes nothing");
    let fx = run(&mut st, ToolsMsg::ProgramPicked { kind: ToolKind::Terminal, path: Some("/usr/bin/kitty".into()) });
    assert_eq!(st.settings.external_tools.terminal, PathBuf::from("/usr/bin/kitty").display().to_string());
    assert!(matches!(&fx[..], [Effect::SaveSettings(_)]));
}

#[test]
fn failed_launches_are_logged_and_toasted() {
    let mut st = state();
    let failed = |kind, failure| ToolsMsg::LaunchFailed { kind, program: "subl".into(), target: "a.txt".into(), failure };
    assert!(run(&mut st, failed(ToolKind::Editor, LaunchFailure::NotFound)).is_empty());
    let text = failure(&st);
    assert!(text.contains("\"subl\"") && text.contains("Preferences > External tools") && text.contains("editor"), "{text}");
    run(&mut st, failed(ToolKind::Terminal, LaunchFailure::NotFound));
    assert!(failure(&st).contains("terminal"));
    run(&mut st, failed(ToolKind::Editor, LaunchFailure::Missing("C:/r/a.txt".into())));
    assert_eq!(failure(&st), format!("{} is not in the working tree", native(Path::new("C:/r/a.txt")).display()));
    run(&mut st, failed(ToolKind::Editor, LaunchFailure::Other("Access is denied.".into())));
    assert_eq!(failure(&st), "Could not open a.txt in subl: Access is denied.");
    run(&mut st, failed(ToolKind::Terminal, LaunchFailure::Other("boom".into())));
    assert_eq!(failure(&st), "Could not start the terminal subl: boom");

    // Every message is unscoped: nothing needs keeping for an inactive tab.
    for m in [
        open_file("a.txt", None),
        ToolsMsg::OpenRepo,
        ToolsMsg::ShowPage,
        ToolsMsg::Browse(ToolKind::Terminal),
        ToolsMsg::ProgramPicked { kind: ToolKind::Editor, path: None },
        failed(ToolKind::Editor, LaunchFailure::NotFound),
    ] {
        assert!(m.on_inactive().is_none());
    }
}

#[test]
fn show_page_opens_external_tools() {
    let mut st = state();
    let fx = run(&mut st, ToolsMsg::ShowPage);
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::Tools));
    assert_eq!(PrefPage::Tools.title(trellis_core::i18n::Lang::En), "External tools");
    assert!(fx.iter().any(|e| matches!(e, Effect::ReadGitConfig { .. })), "{fx:?}");
}

#[test]
fn palette_entries_open_the_repository_and_the_page() {
    let label = "Open repository in external editor";
    let mut st = state();
    let find = |st: &AppState, l: &str| entries(st).into_iter().find(|e| e.label == l);
    assert!(find(&st, label).is_none(), "needs a repository");
    assert!(find(&st, "Configure external tools").is_some(), "always offered");

    let (mut st2, dir) = open("tools-palette", "");
    assert_eq!(find(&st2, label).and_then(|e| e.detail).as_deref(), Some("no editor set"));
    st2.settings.external_tools.editor = "code".into();
    assert_eq!(find(&st2, label).and_then(|e| e.detail).as_deref(), Some("code"));

    let w = st2.workdir().unwrap();
    let fx = update(&mut st2, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Tools(ToolsMsg::OpenRepo))), Instant::now());
    let (l, _) = launch(fx, ToolKind::Editor);
    assert_eq!(l.args, os(&[&w]));
    assert_eq!(st2.palette_recent.position("open-repo-in-editor"), Some(0));
    let _ = std::fs::remove_dir_all(&dir);

    update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Tools(ToolsMsg::ShowPage))), Instant::now());
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::Tools));
    assert_eq!(st.palette_recent.position("tools-configure"), Some(0));
}

#[test]
fn diff_and_merge_tool_programs_are_picked_and_saved() {
    let mut st = state();
    for kind in [ToolKind::DiffTool, ToolKind::MergeTool] {
        assert!(matches!(&run(&mut st, ToolsMsg::Browse(kind))[..], [Effect::Tool(ToolEffect::PickProgram(k))] if *k == kind));
    }
    let fx = run(&mut st, ToolsMsg::ProgramPicked { kind: ToolKind::DiffTool, path: Some("/usr/bin/meld".into()) });
    let meld = PathBuf::from("/usr/bin/meld").display().to_string();
    assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if s.external_tools.diff_tool == meld));
    let fx = run(&mut st, ToolsMsg::ProgramPicked { kind: ToolKind::MergeTool, path: Some("/usr/bin/kdiff3".into()) });
    let kdiff3 = PathBuf::from("/usr/bin/kdiff3").display().to_string();
    assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if s.external_tools.merge_tool == kdiff3 && s.external_tools.diff_tool == meld));
}

#[test]
fn palette_offers_the_diff_and_merge_tools_for_what_is_open() {
    use trellis_core::repo::DiffTarget;
    let (diff_label, merge_label) = ("Open file in external diff tool", "Resolve conflict in external merge tool");
    let find = |st: &AppState, l: &str| entries(st).into_iter().find(|e| e.label == l);
    let (mut st, dir) = open("tools-palette-diff", "");
    assert!(find(&st, diff_label).is_none(), "needs an open diff");
    let target = DiffTarget::Unstaged("a.txt".into());
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(target.clone())));
    assert_eq!(find(&st, diff_label).and_then(|e| e.detail).as_deref(), Some("no diff tool set"));
    st.settings.external_tools.diff_tool = "/opt/meld/bin/meld".into();
    assert_eq!(find(&st, diff_label).and_then(|e| e.detail).as_deref(), Some("meld"));
    assert!(find(&st, merge_label).is_none(), "needs the conflict view");
    // Run before a tool is set, it opens Preferences > External Tools.
    st.settings.external_tools.diff_tool.clear();
    let fx = update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Tools(ToolsMsg::OpenDiff(target.clone())))), Instant::now());
    assert!(!fx.iter().any(|e| matches!(e, Effect::Tool(_))), "{fx:?}");
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::Tools));
    assert_eq!(st.palette_recent.position("open-in-diff-tool"), Some(0));
    st.prefs = None;
    st.settings.external_tools.diff_tool = "/opt/meld/bin/meld".into();
    let fx = update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::Tools(ToolsMsg::OpenDiff(target.clone())))), Instant::now());
    assert!(matches!(&fx[..], [Effect::Tool(ToolEffect::OpenDiff(r))] if r.target == target), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);

    let dir = super::conflict_tests::conflicted_repo("tools-palette-merge");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Conflict(crate::msg::ConflictMsg::OpenConflict("a.txt".into())));
    assert_eq!(find(&st, merge_label).and_then(|e| e.detail).as_deref(), Some("no merge tool set"));
    st.settings.external_tools.merge_tool = "kdiff3".into();
    let entry = find(&st, merge_label).expect("offered");
    assert_eq!(entry.detail.as_deref(), Some("kdiff3"));
    assert_eq!(entry.command, C::Tools(ToolsMsg::ResolveInMergeTool("a.txt".into())));
    assert!(entry.shortcut(&st.keymap).is_none(), "no keyboard shortcut");
    let fx = update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(entry.command)), Instant::now());
    assert!(matches!(&fx[..], [Effect::Tool(ToolEffect::RunMerge(r))] if r.path == "a.txt"), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
