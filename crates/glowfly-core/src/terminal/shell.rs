//! Which shell a built-in terminal starts: the automatic choice, or the one chosen in
//! Preferences > Terminal. A chosen shell that can't be found is an error, never
//! replaced by another one.

use super::pty::ShellCommand;
use crate::external_tools;
use crate::settings::{ShellChoice, ShellSetting};
use std::path::{Path, PathBuf};

/// The automatic shell. Windows: PowerShell 7 (pwsh) if it is on PATH, else Windows
/// PowerShell, else COMSPEC (cmd.exe). Elsewhere: $SHELL, else /bin/sh.
pub fn choose_shell(windows: bool, env: impl Fn(&str) -> Option<String>, on_path: impl Fn(&str) -> bool) -> ShellCommand {
    if windows {
        if on_path("pwsh.exe") {
            nologo("pwsh.exe")
        } else if on_path("powershell.exe") {
            nologo("powershell.exe")
        } else {
            plain(&comspec(&env))
        }
    } else {
        plain(&env("SHELL").filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/sh".into()))
    }
}

fn nologo(program: &str) -> ShellCommand {
    ShellCommand { program: program.into(), args: vec!["-NoLogo".into()] }
}

fn plain(program: &str) -> ShellCommand {
    ShellCommand { program: program.into(), args: Vec::new() }
}

fn comspec(env: &impl Fn(&str) -> Option<String>) -> String {
    env("COMSPEC").filter(|s| !s.is_empty()).unwrap_or_else(|| "cmd.exe".into())
}

/// The shell for `s`. `env` reads environment variables, `find` answers where a program name
/// (searched on PATH) or path is, and `git` is the git found on PATH, next to which Git Bash
/// lives on Windows. Windows-only choices act as Automatic elsewhere.
pub fn for_choice(
    s: &ShellSetting,
    windows: bool,
    env: impl Fn(&str) -> Option<String>,
    find: impl Fn(&str) -> Option<PathBuf>,
    git: Option<PathBuf>,
) -> Result<ShellCommand, String> {
    let found = |p: PathBuf| p.to_string_lossy().into_owned();
    match s.choice {
        ShellChoice::Pwsh => {
            let name = if windows { "pwsh.exe" } else { "pwsh" };
            let fallback = || {
                let dir = env("ProgramFiles").filter(|_| windows)?;
                find(&Path::new(&dir).join("PowerShell").join("7").join("pwsh.exe").to_string_lossy())
            };
            let program = find(name).or_else(fallback).ok_or("pwsh was not found on PATH")?;
            Ok(ShellCommand { program: found(program), args: vec!["-NoLogo".into()] })
        }
        ShellChoice::WindowsPowerShell if windows => Ok(nologo("powershell.exe")),
        ShellChoice::Cmd if windows => Ok(plain(&comspec(&env))),
        ShellChoice::Bash if windows => {
            let program = git_bash(git.as_deref(), &find)
                .or_else(|| find("bash.exe"))
                .ok_or("Git Bash was not found; install Git for Windows or put bash.exe on PATH")?;
            Ok(ShellCommand { program: found(program), args: vec!["--login".into(), "-i".into()] })
        }
        ShellChoice::Bash => Ok(plain(&found(find("bash").ok_or("bash was not found on PATH")?))),
        ShellChoice::Custom => {
            let program = external_tools::program(&s.program).ok_or("no custom shell program is set")?;
            let args = external_tools::split(&s.args).map_err(|_| "the custom shell arguments have an unmatched quote")?;
            let text = program.to_string_lossy().into_owned();
            let program = find(&text).ok_or_else(|| format!("{text} was not found"))?;
            Ok(ShellCommand { program: found(program), args })
        }
        ShellChoice::Auto | ShellChoice::WindowsPowerShell | ShellChoice::Cmd => {
            Ok(choose_shell(windows, env, |name| find(name).is_some()))
        }
    }
}

/// Git for Windows' bash.exe, from the git found on PATH: `<Git>\cmd\git.exe`,
/// `<Git>\bin\git.exe` and `<Git>\mingw64\bin\git.exe` all have it at `<Git>\bin\bash.exe`.
fn git_bash(git: Option<&Path>, find: &impl Fn(&str) -> Option<PathBuf>) -> Option<PathBuf> {
    git?.ancestors().skip(1).take(3).find_map(|dir| find(&dir.join("bin").join("bash.exe").to_string_lossy()))
}

/// The shell for `s` on this system, from the environment and PATH. Runner threads only.
pub fn resolve(s: &ShellSetting) -> Result<ShellCommand, String> {
    let find = |name: &str| external_tools::find_on_path(Path::new(name));
    let git = find("git");
    for_choice(s, cfg!(windows), |k| std::env::var(k).ok(), find, git)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let vars: Vec<(String, String)> = vars.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |k: &str| vars.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone())
    }

    /// A `find` that knows `known` (names or paths, each found as itself).
    fn finder(known: Vec<String>) -> impl Fn(&str) -> Option<PathBuf> {
        move |n: &str| known.iter().any(|k| k == n).then(|| PathBuf::from(n))
    }

    fn setting(choice: ShellChoice) -> ShellSetting {
        ShellSetting { choice, ..Default::default() }
    }

    fn path(p: &Path) -> String {
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn chooses_a_shell() {
        let path = |names: &'static [&'static str]| move |n: &str| names.contains(&n);
        let nologo = vec!["-NoLogo".to_owned()];

        let s = choose_shell(true, env(&[]), path(&["pwsh.exe", "powershell.exe"]));
        assert_eq!((s.program.as_str(), &s.args), ("pwsh.exe", &nologo));
        let s = choose_shell(true, env(&[]), path(&["powershell.exe"]));
        assert_eq!((s.program.as_str(), &s.args), ("powershell.exe", &nologo));
        let s = choose_shell(true, env(&[("COMSPEC", r"C:\Windows\system32\cmd.exe")]), path(&[]));
        assert_eq!(s, ShellCommand { program: r"C:\Windows\system32\cmd.exe".into(), args: vec![] });
        assert_eq!(choose_shell(true, env(&[]), path(&[])).program, "cmd.exe");

        let s = choose_shell(false, env(&[("SHELL", "/bin/zsh")]), path(&["pwsh.exe"]));
        assert_eq!(s, ShellCommand { program: "/bin/zsh".into(), args: vec![] });
        assert_eq!(choose_shell(false, env(&[]), path(&[])).program, "/bin/sh");
        assert_eq!(choose_shell(false, env(&[("SHELL", "")]), path(&[])).program, "/bin/sh");
    }

    #[test]
    fn automatic_and_windows_only_choices() {
        let known = || finder(vec!["powershell.exe".into()]);
        let auto = for_choice(&setting(ShellChoice::Auto), true, env(&[]), known(), None).unwrap();
        assert_eq!(auto, choose_shell(true, env(&[]), |n| n == "powershell.exe"));
        let unix = env(&[("SHELL", "/bin/zsh")]);
        for c in [ShellChoice::Auto, ShellChoice::WindowsPowerShell, ShellChoice::Cmd] {
            let s = for_choice(&setting(c), false, &unix, known(), None).unwrap();
            assert_eq!(s, plain("/bin/zsh"), "{c:?} acts as Automatic off Windows");
        }
        let s = for_choice(&setting(ShellChoice::WindowsPowerShell), true, env(&[]), finder(vec![]), None).unwrap();
        assert_eq!(s, nologo("powershell.exe"));
        let s = for_choice(&setting(ShellChoice::Cmd), true, env(&[("COMSPEC", "cmd-here.exe")]), finder(vec![]), None).unwrap();
        assert_eq!(s, plain("cmd-here.exe"));
    }

    #[test]
    fn pwsh_found_or_reported() {
        let s = for_choice(&setting(ShellChoice::Pwsh), false, env(&[]), finder(vec!["pwsh".into()]), None).unwrap();
        assert_eq!(s, nologo("pwsh"));
        let err = for_choice(&setting(ShellChoice::Pwsh), true, env(&[]), finder(vec![]), None).unwrap_err();
        assert_eq!(err, "pwsh was not found on PATH");

        let pf = crate::paths::temp_dir().join("glowfly-shell-pf");
        let installed = path(&pf.join("PowerShell").join("7").join("pwsh.exe"));
        let vars = env(&[("ProgramFiles", &path(&pf))]);
        let s = for_choice(&setting(ShellChoice::Pwsh), true, vars, finder(vec![installed.clone()]), None).unwrap();
        assert_eq!(s.program, installed, "the usual install folder when it isn't on PATH");
    }

    #[test]
    fn git_bash_next_to_git() {
        let root = crate::paths::temp_dir().join("glowfly-shell-git");
        let bash = path(&root.join("bin").join("bash.exe"));
        let login = vec!["--login".to_owned(), "-i".to_owned()];
        for git in [root.join("cmd").join("git.exe"), root.join("bin").join("git.exe"), root.join("mingw64").join("bin").join("git.exe")] {
            let s = for_choice(&setting(ShellChoice::Bash), true, env(&[]), finder(vec![bash.clone()]), Some(git.clone())).unwrap();
            assert_eq!((s.program.as_str(), &s.args), (bash.as_str(), &login), "{git:?}");
        }
        let s = for_choice(&setting(ShellChoice::Bash), true, env(&[]), finder(vec!["bash.exe".into()]), None).unwrap();
        assert_eq!(s.program, "bash.exe", "bash.exe on PATH");
        let err = for_choice(&setting(ShellChoice::Bash), true, env(&[]), finder(vec![]), Some(root.join("cmd").join("git.exe")));
        assert_eq!(err.unwrap_err(), "Git Bash was not found; install Git for Windows or put bash.exe on PATH");

        let s = for_choice(&setting(ShellChoice::Bash), false, env(&[]), finder(vec!["bash".into()]), None).unwrap();
        assert_eq!(s, plain("bash"));
        let err = for_choice(&setting(ShellChoice::Bash), false, env(&[]), finder(vec![]), None).unwrap_err();
        assert_eq!(err, "bash was not found on PATH");
    }

    #[test]
    fn custom_program_and_arguments() {
        let custom = |program: &str, args: &str| ShellSetting { choice: ShellChoice::Custom, program: program.into(), args: args.into() };
        let s = for_choice(&custom(" \"nu\" ", "-l \"a b\""), false, env(&[]), finder(vec!["nu".into()]), None).unwrap();
        assert_eq!(s, ShellCommand { program: "nu".into(), args: vec!["-l".into(), "a b".into()] });
        let err = |s: &ShellSetting| for_choice(s, false, env(&[]), finder(vec!["nu".into()]), None).unwrap_err();
        assert_eq!(err(&custom("  ", "")), "no custom shell program is set");
        assert_eq!(err(&custom("nu", "\"open")), "the custom shell arguments have an unmatched quote");
        assert_eq!(err(&custom("fish", "")), "fish was not found");
    }
}
