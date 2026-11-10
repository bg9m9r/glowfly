//! Opening a terminal window in a folder. Windows and macOS each have one obvious terminal;
//! Linux desktops vary, so a list of common ones is tried in turn. The folder is an argument of
//! its own or the process's working folder, or, for Windows PowerShell, in a variable its fixed
//! command line reads; only cmd.exe gets it in its command line, between quotes (`cmd_takes`).

use std::ffi::OsString;
use std::path::Path;

/// One way to start a terminal: the program and its arguments.
#[derive(Debug, PartialEq)]
pub(crate) struct Launch {
    pub program: OsString,
    pub args: Vec<OsString>,
    /// Windows: the rest of the command line as written, after `args` (cmd.exe and PowerShell
    /// parse their own command lines, so it is quoted for them, not for the C runtime).
    pub raw: Option<String>,
    /// Whether the process is started in the folder, for terminals that have no option for it.
    /// Not for cmd.exe, which refuses a UNC folder (`\\server\share`, `\\wsl$\...`) and starts in
    /// C:\Windows instead; the command line changes to the folder there.
    pub in_dir: bool,
    /// The folder, set in the process's `GLOWFLY_TERMINAL_DIR` for a command line that reads it
    /// from there.
    pub env_dir: Option<OsString>,
}

impl Launch {
    fn new(program: impl Into<OsString>, args: Vec<OsString>) -> Self {
        Launch { program: program.into(), args, raw: None, in_dir: true, env_dir: None }
    }
}

/// PowerShell's whole command line: the same text for every folder. It changes to the folder
/// named by its `GLOWFLY_TERMINAL_DIR` variable (`Launch::env_dir`; after the profile ran, so the window ends up there), taken as it is, and
/// drops the variable. A UNC folder works: PowerShell needs no drive letter.
const POWERSHELL_LINE: &str =
    "-NoLogo -NoExit -Command \"Set-Location -LiteralPath $env:GLOWFLY_TERMINAL_DIR; Remove-Item Env:GLOWFLY_TERMINAL_DIR\"";

/// Whether cmd.exe can be given `dir` between double quotes: there every character is part of
/// the name except `%`, which cmd.exe expands as `%name%` even inside quotes, and a `"`, which
/// would end them (no Windows file name holds one; a path that does is not written there).
fn cmd_takes(dir: &str) -> bool {
    !dir.contains(['%', '"']) && !dir.chars().any(char::is_control)
}

/// The Windows terminal for `dir`: cmd.exe, which `pushd`es into it (mapping a drive letter for
/// a UNC folder, so a share or a WSL folder works too) and stays open. A folder cmd.exe can't
/// be given between quotes (`cmd_takes`: a `%` in its name) gets Windows PowerShell instead,
/// whose command line never holds the folder: it reads it from a variable (`POWERSHELL_LINE`).
pub(crate) fn windows_launches(dir: &Path) -> Vec<Launch> {
    let dir = glowfly_core::native_path::shown(dir);
    let launch = if cmd_takes(&dir) {
        Launch { program: "cmd.exe".into(), args: Vec::new(), raw: Some(format!("/K pushd \"{dir}\"")), in_dir: false, env_dir: None }
    } else {
        Launch {
            program: "powershell.exe".into(),
            args: Vec::new(),
            raw: Some(POWERSHELL_LINE.to_owned()),
            in_dir: false,
            env_dir: Some(dir.into()),
        }
    };
    vec![launch]
}

/// Linux terminals in the order they are tried after `$TERMINAL`, each with how it is told to
/// start in a folder. `x-terminal-emulator` (Debian's choice of default) and xterm have no
/// common option, so they rely on the process's working directory.
const LINUX_TERMINALS: &[(&str, DirArg)] = &[
    ("x-terminal-emulator", DirArg::None),
    ("gnome-terminal", DirArg::Joined("--working-directory=")),
    ("konsole", DirArg::Separate("--workdir")),
    ("xfce4-terminal", DirArg::Joined("--working-directory=")),
    ("alacritty", DirArg::Separate("--working-directory")),
    ("kitty", DirArg::Separate("--directory")),
    ("xterm", DirArg::None),
];

#[derive(Clone, Copy)]
enum DirArg {
    None,
    /// `--option=<dir>`
    Joined(&'static str),
    /// `--option <dir>`
    Separate(&'static str),
}

impl DirArg {
    fn args(self, dir: &Path) -> Vec<OsString> {
        match self {
            DirArg::None => Vec::new(),
            DirArg::Joined(opt) => {
                let mut a = OsString::from(opt);
                a.push(dir.as_os_str());
                vec![a]
            }
            DirArg::Separate(opt) => vec![opt.into(), dir.into()],
        }
    }
}

/// The Linux candidates for `dir`. `terminal` is `$TERMINAL`: a known terminal's name (or path)
/// gets its folder option; anything else is started as it is. It goes first, and is not tried
/// again later in the list.
pub(crate) fn linux_launches(dir: &Path, terminal: Option<&str>) -> Vec<Launch> {
    let known = |name: &str| LINUX_TERMINALS.iter().find(|(n, _)| *n == name).map(|(_, d)| *d);
    let mut out = Vec::new();
    let mut first = None;
    if let Some(t) = terminal.map(str::trim).filter(|t| !t.is_empty()) {
        let base = Path::new(t).file_name().and_then(|n| n.to_str()).unwrap_or(t);
        out.push(Launch::new(t, known(base).unwrap_or(DirArg::None).args(dir)));
        first = Some(base.to_string());
    }
    for (name, arg) in LINUX_TERMINALS {
        if first.as_deref() != Some(*name) {
            out.push(Launch::new(*name, arg.args(dir)));
        }
    }
    out
}

/// What is tried on this platform, in order.
fn launches(dir: &Path) -> Vec<Launch> {
    if cfg!(windows) {
        windows_launches(dir)
    } else if cfg!(target_os = "macos") {
        vec![Launch::new("open", vec!["-a".into(), "Terminal".into(), glowfly_core::native_path::native(dir).into()])]
    } else {
        linux_launches(dir, std::env::var("TERMINAL").ok().as_deref())
    }
}

/// Opens a terminal in `dir`: the first candidate that is installed. Fails with the list of
/// terminals tried when none is.
pub fn open_terminal(dir: &Path) -> std::io::Result<()> {
    let all = launches(dir);
    for l in &all {
        let mut cmd = std::process::Command::new(&l.program);
        cmd.args(&l.args);
        if l.in_dir {
            cmd.current_dir(dir);
        }
        if let Some(folder) = &l.env_dir {
            cmd.env("GLOWFLY_TERMINAL_DIR", folder);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // A console window of its own, opened straight away: no helper console flashes.
            const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
            cmd.creation_flags(CREATE_NEW_CONSOLE);
            if let Some(raw) = &l.raw {
                cmd.raw_arg(raw);
            }
        }
        match cmd.spawn() {
            Ok(_) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::NotFound, not_found_message(&all)))
}

fn not_found_message(tried: &[Launch]) -> String {
    let names: Vec<String> = tried.iter().map(|l| l.program.to_string_lossy().into_owned()).collect();
    format!("no terminal found (tried {}); install one or set $TERMINAL", names.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn shown(l: &[Launch]) -> Vec<String> {
        l.iter()
            .map(|l| {
                let mut s = l.program.to_string_lossy().into_owned();
                for a in &l.args {
                    s.push(' ');
                    s.push_str(&a.to_string_lossy());
                }
                s
            })
            .collect()
    }

    #[test]
    fn each_linux_terminal_gets_its_folder_option() {
        let dir = PathBuf::from("/home/me/my repo");
        assert_eq!(
            shown(&linux_launches(&dir, None)),
            [
                "x-terminal-emulator",
                "gnome-terminal --working-directory=/home/me/my repo",
                "konsole --workdir /home/me/my repo",
                "xfce4-terminal --working-directory=/home/me/my repo",
                "alacritty --working-directory /home/me/my repo",
                "kitty --directory /home/me/my repo",
                "xterm",
            ]
        );
        // A folder with spaces stays one argument.
        let l = linux_launches(&dir, None);
        assert_eq!(l[2].args, vec![OsString::from("--workdir"), OsString::from("/home/me/my repo")]);
    }

    #[test]
    fn terminal_variable_goes_first() {
        let dir = PathBuf::from("/r");
        let l = shown(&linux_launches(&dir, Some("/usr/bin/kitty")));
        assert_eq!(l[0], "/usr/bin/kitty --directory /r");
        assert_eq!(l.iter().filter(|s| s.contains("kitty")).count(), 1, "kitty is not tried twice");
        assert_eq!(l.len(), LINUX_TERMINALS.len());

        // An unknown terminal is started as it is, before the usual list.
        let l = shown(&linux_launches(&dir, Some("wezterm")));
        assert_eq!(l[0], "wezterm");
        assert_eq!(l.len(), LINUX_TERMINALS.len() + 1);

        // Blank means unset.
        assert_eq!(linux_launches(&dir, Some("  ")), linux_launches(&dir, None));
    }

    #[test]
    fn windows_terminals_change_to_the_folder_themselves() {
        // cmd.exe refuses a UNC working folder; pushd maps a drive letter for it.
        let l = windows_launches(Path::new(r"\\wsl.localhost\Ubuntu\home\me\repo"));
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].program, OsString::from("cmd.exe"));
        assert_eq!(l[0].raw.as_deref(), Some(r#"/K pushd "\\wsl.localhost\Ubuntu\home\me\repo""#));
        assert!(!l[0].in_dir && l[0].args.is_empty());
        // Quoted, so `&` and spaces are part of the name.
        let l = windows_launches(Path::new(r"\\server\share\R&D tools"));
        assert_eq!(l[0].raw.as_deref(), Some(r#"/K pushd "\\server\share\R&D tools""#));
        // cmd.exe would expand %name%; PowerShell takes the folder literally.
        let l = windows_launches(Path::new(r"\\server\share\100% done's"));
        assert_eq!(l[0].program, OsString::from("powershell.exe"));
        assert_eq!(l[0].raw.as_deref(), Some(POWERSHELL_LINE));
        assert!(!l[0].in_dir && l[0].args.is_empty());
    }

    #[test]
    fn powershell_reads_the_folder_from_a_variable_and_its_command_line_never_changes() {
        // `%` sends the folder to PowerShell; each of these means something to it in a script.
        let marks = ["'", "\u{2018}", "\u{2019}", "\u{201a}", "\u{201b}", "`", "$", ";", "&", "\"", "$(x)", "'; x; '"];
        let sep = if cfg!(windows) { '\\' } else { '/' };
        for root in [r"C:\work", r"\\server\share", r"\\wsl.localhost\Ubuntu\home\me"] {
            for mark in marks {
                let name = format!("100% a{mark}b{mark}");
                let dir = format!("{root}{sep}{name}");
                let l = windows_launches(Path::new(&dir));
                assert_eq!(l.len(), 1);
                assert_eq!(l[0].program, OsString::from("powershell.exe"), "{dir}");
                assert_eq!(l[0].raw.as_deref(), Some(POWERSHELL_LINE), "the same text for {dir}");
                assert!(l[0].args.is_empty() && !l[0].in_dir);
                let given = l[0].env_dir.as_ref().expect("the folder, in the variable").to_string_lossy().into_owned();
                assert!(given.ends_with(&name), "as it is: {given}");
            }
        }
        // Nothing of a folder is in the text: no name, no quote character of either kind.
        assert!(!POWERSHELL_LINE.contains(['\'', '%', '\u{2018}', '\u{2019}', '\u{201a}', '\u{201b}', '`']));
        assert_eq!(POWERSHELL_LINE.matches('"').count(), 2, "only the quotes around the command");
        assert!(POWERSHELL_LINE.contains("-LiteralPath $env:GLOWFLY_TERMINAL_DIR;"), "the variable it is given");
    }

    #[test]
    fn cmd_gets_only_folders_it_takes_literally_between_quotes() {
        // `%name%` is expanded inside quotes too, so `%` never reaches cmd.exe.
        for dir in [r"C:\work\100%", r"C:\work\%PATH%", r"\\server\share\a%b"] {
            assert!(!cmd_takes(dir));
            assert_eq!(windows_launches(Path::new(dir))[0].program, OsString::from("powershell.exe"), "{dir}");
        }
        // A `"` would end the quotes. No Windows name holds one; a path that does isn't written.
        assert!(!cmd_takes("C:\\work\\a\" & x & \"b"));
        assert!(!cmd_takes("C:\\work\\a\nb"));
        // Everything else is part of the name between the quotes, the other quote marks too.
        for name in ["R&D", "a|b", "a^b", "(x)", "it's", "a\u{2019}b", "a;b", "$x", "`x`", "!x!", "a b"] {
            let dir = format!(r"C:\work\{name}");
            assert!(cmd_takes(&dir), "{dir}");
            let l = windows_launches(Path::new(&dir));
            assert_eq!(l[0].program, OsString::from("cmd.exe"));
            let raw = l[0].raw.as_deref().unwrap();
            assert!(raw.starts_with("/K pushd \"") && raw.ends_with(&format!("{name}\"")), "{raw}");
            assert_eq!(raw.matches('"').count(), 2, "one pair of quotes: {raw}");
            assert!(l[0].env_dir.is_none());
        }
    }

    /// Runs Windows PowerShell with the command line of `windows_launches` (without the window
    /// staying open) in a folder whose name would end a quoted string: it ends up in the folder.
    #[cfg(windows)]
    #[test]
    fn powershell_ends_up_in_a_folder_named_with_quote_marks() {
        use std::os::windows::process::CommandExt;
        let base = glowfly_core::paths::temp_dir().join(format!("glowfly-terminal-{}", std::process::id()));
        let dir = base.join("100% a\u{2019}; exit 7; \u{2018}b's $(exit 7) `x");
        std::fs::create_dir_all(&dir).unwrap();
        let l = windows_launches(&dir).remove(0);
        assert_eq!(l.program, OsString::from("powershell.exe"));
        let line = l.raw.unwrap().replace("-NoExit", "-NoProfile").replace("\"Set-", "\"$ErrorActionPreference = 'Stop'; Set-");
        let line = line.replacen("GLOWFLY_TERMINAL_DIR\"", "GLOWFLY_TERMINAL_DIR; [Console]::Out.Write((Get-Location).ProviderPath)\"", 1);
        let mut cmd = std::process::Command::new(&l.program);
        cmd.raw_arg(&line).env("GLOWFLY_TERMINAL_DIR", l.env_dir.unwrap()).current_dir(&base);
        let out = cmd.output().unwrap();
        let _ = std::fs::remove_dir_all(&base);
        assert!(out.status.success(), "{line}: {}", String::from_utf8_lossy(&out.stderr));
        let at = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(at.ends_with("b's $(exit 7) `x") && at.contains("100% a"), "it is in the folder: {at}");
    }

    #[cfg(windows)]
    #[test]
    fn windows_terminals_get_backslashes() {
        let l = windows_launches(Path::new("C:/work/app/"));
        assert_eq!(l[0].raw.as_deref(), Some(r#"/K pushd "C:\work\app""#));
    }

    #[test]
    fn missing_terminals_are_named_in_the_error() {
        let msg = not_found_message(&linux_launches(Path::new("/r"), None));
        assert!(msg.contains("gnome-terminal, konsole"), "{msg}");
        assert!(msg.contains("$TERMINAL"), "{msg}");
    }
}
