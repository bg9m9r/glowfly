# Custom commands

Your own git and shell actions, run from the command palette, from a Commands submenu in the
commit, branch and file menus, and from keys you choose. They run like Trellis's own jobs: the
toolbar shows them running, the activity log keeps their output, and the repository is
reloaded when they finish.

## Where they come from

Two files, both optional, with the same format:

| File | For |
| --- | --- |
| `commands.toml` in Trellis's config folder (`%APPDATA%\Trellis` on Windows, `~/Library/Application Support/Trellis` on macOS, `$XDG_CONFIG_HOME/trellis`, usually `~/.config/trellis`, on Linux) | Your own commands, in every repository. |
| `.trellis/commands.toml` in a repository's working tree | A repository's commands, committed so a team can share them. |

The palette lists your commands first, then the repository's. Trellis reads both files when a
repository opens and on Refresh. It reads the repository's file again when anything under
`.trellis/` changes. Use **Reload custom commands** in the palette after editing your own file.

## The format

Each command is one `[[command]]` table:

```toml
[[command]]
name = "Log of this branch"              # shown in the palette and menus (required)
kind = "git"                             # "git" or "shell" (required)
template = "log --oneline -20 {branch}"  # what runs (required)
show = ["palette", "branch"]             # where it is offered; default ["palette"]
confirm = "Show the log of {branch}?"    # false (default), true, or your own question
key = "Mod+Alt+L"                        # optional shortcut

[[command]]
name = "Format this file"
kind = "shell"
template = "npx prettier --write {files}"
show = ["file"]
```

- **name**: up to 80 characters, unique within its file.
- **kind**:
  - `git` runs git with the template's words as arguments. A leading `git` word is optional.
  - `shell` runs the template as a POSIX `sh` script: `/bin/sh` on macOS and Linux, and the
    `sh.exe` that comes with Git for Windows on Windows. One syntax works on every OS, so a
    shared file behaves the same for everyone.
- **show**: any of `palette`, `commit` (the commit menu), `branch` (a local branch's menu) and
  `file` (a file's menu, in a commit or in the working tree).
- **confirm**: `true` asks "Run <name>?" and shows what will run. A string asks that question
  instead, with the placeholders filled in.
- **key**: a key with Mod, Ctrl or Alt, written as in Preferences > Keyboard (`Mod` is Ctrl on
  Windows and Linux and Cmd on macOS). A key that is already bound to a Trellis action or to an
  earlier command is ignored and reported.

A file holds at most 200 commands and 256 KiB. An unknown field or a value of the wrong type is
reported, not ignored.

## Placeholders

| Placeholder | Holds |
| --- | --- |
| `{branch}` | The branch: the one whose menu you opened, else the checked-out branch. |
| `{sha}` | The full commit id: the commit whose menu you opened, the commit a file row belongs to, the commit selected when you run from the palette, else HEAD. |
| `{files}` | The file whose menu you opened, or from the palette, the file whose diff is open. |
| `{repo}` | The repository's working folder. |

A command that needs a value the row can't give (`{branch}` with HEAD detached, `{files}` with no
file) is greyed in the menu, and the palette says what it needs.

Values are passed safely:

- In a git command, every value is one argument, so spaces and quotes in a branch or file name
  can't split it. `{files}` must stand alone as a word.
- In a shell command, every value is single-quoted for you. Don't put quotes around a
  placeholder; Trellis reports that as a problem.
- A file name starting with `-` (or `:` for git) gets `./` in front, so it is never read as an
  option.

A `{word}` right after `@` or `$` is left alone, so git's `@{upstream}` and the shell's `${HOME}`
work as usual. Any other `{word}` that isn't a placeholder is reported.

## Running

Commands run in the repository's working folder, one job at a time like other operations.
Credential prompts appear in Trellis's dialog. Git never waits for an editor, and a shell
command's input is closed. A shell command's output and errors are kept together; for long
output, the activity log keeps the last 512 KiB.

## Trust

A repository's own commands are code from whoever wrote the file:

- **Trusted repository**: they run like your own.
- **Any other repository**: running one first asks you to confirm it, naming
  `.trellis/commands.toml` as its source and showing the exact command or script it runs (its
  own `confirm` question, if it has one, is asked in the same dialog).
- **Repository you haven't decided on**: once confirmed, the trust prompt opens. The command
  runs once you trust the repository, and is dropped if you don't.
- **Repository you chose not to trust, or one that changed since you trusted it**: every
  custom command, yours included, waits for the trust prompt, like the other operations that
  run the repository's hooks or filters.

The palette and the menus say when a command will ask first. A `.trellis` folder or
`commands.toml` that is a link is never read, so a repository can't point Trellis at a file
outside it.

## Problems

A mistake in a file is reported in the activity log with the file and line, and one toast says
how many problems there are. The toast comes back only when the problems change.

- A command with a mistake in its values (an unknown placeholder, a duplicate name, a quoted
  placeholder) is left out, and the rest of the file loads.
- A file that isn't valid TOML, or has an unknown field or a value of the wrong type, loads no
  commands until it is fixed.
- A key that can't be used is ignored; its command still loads.
