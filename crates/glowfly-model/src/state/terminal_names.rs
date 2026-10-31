//! Terminal tab names, worked out every frame from the session: the user's rename, else the
//! running command from the window title, else "<program> · <folder>". Tabs of one repository
//! that would show the same name get their number after it.

use super::AppState;
use super::terminal::TermSession;
use glowfly_core::settings::{ShellChoice, ShellSetting};
use glowfly_core::terminal::command_from_title;
use std::path::Path;

/// The longest rename kept, in chars.
const MAX_RENAME: usize = 64;

/// The shell's short name before it has reported one: what `s` asks for.
pub fn program_hint(s: &ShellSetting) -> String {
    let name = match s.choice {
        ShellChoice::Pwsh => "pwsh",
        ShellChoice::WindowsPowerShell => "powershell",
        ShellChoice::Cmd => "cmd",
        ShellChoice::Bash => "bash",
        ShellChoice::Auto => "shell",
        ShellChoice::Custom => {
            let stem = glowfly_core::external_tools::program(&s.program)
                .and_then(|p| p.file_stem().map(|f| f.to_string_lossy().into_owned()))
                .filter(|f| !f.is_empty());
            return stem.unwrap_or_else(|| "shell".to_owned());
        }
    };
    name.to_owned()
}

/// The folder part of a name: `workdir`'s last component, or the whole path when it has none.
pub fn folder(workdir: &Path) -> String {
    workdir.file_name().map_or_else(|| workdir.display().to_string(), |f| f.to_string_lossy().into_owned())
}

/// The name a tab gets when the user hasn't named it: the command the window title shows,
/// else "<program> · <folder>".
pub fn auto_name(s: &TermSession) -> String {
    let program = if s.program.is_empty() { program_hint(&s.shell) } else { s.program.clone() };
    if let Some(command) = s.screen.title().and_then(|t| command_from_title(t, &program)) {
        return command;
    }
    format!("{program} \u{b7} {}", folder(&s.workdir))
}

/// The user's name for the tab, else its automatic one.
pub fn display_name(s: &TermSession) -> String {
    s.name.clone().unwrap_or_else(|| auto_name(s))
}

/// The names of one repository's `tabs`, in order: a name an earlier tab already shows gets
/// " (N)" with the session's number.
pub fn tab_names(tabs: &[&TermSession]) -> Vec<String> {
    let bases: Vec<String> = tabs.iter().map(|s| display_name(s)).collect();
    bases.iter().enumerate().map(|(i, b)| if bases[..i].contains(b) { format!("{b} ({})", tabs[i].number) } else { b.clone() }).collect()
}

/// Session `id`'s name among its repository's tabs.
pub fn name_of(st: &AppState, id: u64) -> Option<String> {
    let s = st.terminal.sessions.iter().find(|s| s.id == id)?;
    let tabs: Vec<&TermSession> = st.terminal.sessions_for(&s.workdir).collect();
    let i = tabs.iter().position(|t| t.id == id)?;
    tab_names(&tabs).into_iter().nth(i)
}

/// A typed rename without control characters, trimmed and at most `MAX_RENAME` chars; None
/// when nothing is left.
pub fn clean_rename(raw: &str) -> Option<String> {
    let text: String = raw.chars().filter(|c| !c.is_control()).collect();
    let text: String = text.trim().chars().take(MAX_RENAME).collect();
    let text = text.trim_end();
    (!text.is_empty()).then(|| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::terminal::SessionStatus;
    use glowfly_core::terminal::TermScreen;
    use std::path::PathBuf;

    fn demo() -> PathBuf {
        glowfly_core::paths::temp_dir().join("demo")
    }

    fn session(id: u64, number: u32, program: &str) -> TermSession {
        TermSession {
            id,
            number,
            workdir: demo(),
            program: program.into(),
            shell: ShellSetting::default(),
            name: None,
            screen: TermScreen::new(24, 80),
            status: SessionStatus::Running,
            selection: None,
        }
    }

    #[test]
    fn the_automatic_name_is_the_command_or_the_program_and_folder() {
        let mut s = session(1, 1, "pwsh");
        assert_eq!(auto_name(&s), "pwsh \u{b7} demo");
        s.screen.feed(b"\x1b]0;cargo test\x07");
        assert_eq!(auto_name(&s), "cargo test");
        s.screen.feed(b"\x1b]0;/usr/bin/pwsh\x07");
        assert_eq!(auto_name(&s), "pwsh \u{b7} demo", "the shell's own path");
        s.name = Some("build".into());
        assert_eq!(display_name(&s), "build");
    }

    #[test]
    fn before_it_starts_the_program_comes_from_the_shell_choice() {
        let hint = |choice, program: &str| program_hint(&ShellSetting { choice, program: program.into(), args: String::new() });
        assert_eq!(hint(ShellChoice::Pwsh, ""), "pwsh");
        assert_eq!(hint(ShellChoice::Cmd, ""), "cmd");
        assert_eq!(hint(ShellChoice::WindowsPowerShell, ""), "powershell");
        assert_eq!(hint(ShellChoice::Custom, "/opt/nu/bin/nu"), "nu");
        assert_eq!(hint(ShellChoice::Custom, ""), "shell");
        assert_eq!(hint(ShellChoice::Auto, ""), "shell");
        let mut s = session(1, 1, "");
        s.shell.choice = ShellChoice::Bash;
        assert_eq!(auto_name(&s), "bash \u{b7} demo");
    }

    #[test]
    fn repeated_names_get_their_number() {
        let (a, b, mut c) = (session(1, 1, "pwsh"), session(2, 2, "pwsh"), session(3, 3, "bash"));
        assert_eq!(tab_names(&[&a, &b, &c]), ["pwsh \u{b7} demo", "pwsh \u{b7} demo (2)", "bash \u{b7} demo"]);
        c.program = "pwsh".into();
        assert_eq!(tab_names(&[&b, &c]), ["pwsh \u{b7} demo", "pwsh \u{b7} demo (3)"]);
    }

    #[test]
    fn a_path_without_a_file_name_is_its_own_folder() {
        assert_eq!(folder(&demo()), "demo");
        let root = demo().ancestors().last().unwrap().to_path_buf();
        assert_eq!(folder(&root), root.display().to_string());
    }

    #[test]
    fn renames_are_cleaned() {
        assert_eq!(clean_rename("  build \u{7}"), Some("build".into()));
        assert_eq!(clean_rename("a\tb"), Some("ab".into()));
        assert_eq!(clean_rename("   "), None);
        assert_eq!(clean_rename(&"x".repeat(100)).map(|s| s.len()), Some(64));
    }
}
