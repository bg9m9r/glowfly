# Privacy

Glowfly is a desktop app. It has no account, no server of its own, and it doesn't collect
anything about you. This page lists every way it talks to the network, what each one sends,
and how to turn it off.

## No telemetry

- No telemetry, no usage analytics, no crash reports. Nothing is sent to the Glowfly project.
- Errors and crashes are written to log files on your computer (below). They stay there.

## The update check

- **What:** once a day, Glowfly asks GitHub's releases API
  (`api.github.com/repos/bg9m9r/glowfly/releases/latest`) whether there is a newer release.
  Check now in Preferences > General asks straight away.
- **What GitHub sees:** your IP address and Glowfly's version (in the request's User-Agent).
  Nothing about you or your repositories.
- **Downloads:** only when you click Download. The file and its checksums come from the
  release on GitHub, which hands them out from its own download servers.
- **Turn it off:** Preferences > General > Updates, "Check for updates once a day".

## Author pictures (avatars)

- **Off unless you turn it on:** Preferences > UI Customization > Author pictures, "Look up
  avatars online" (or the checkbox on the welcome's last step).
- **What is sent:** for each commit author, a SHA-256 hash of their email address goes to
  Gravatar (`gravatar.com`). For GitHub's noreply addresses (`…@users.noreply.github.com`),
  the GitHub user name or id in the address goes to GitHub instead. Either service also sees
  your IP address. The email address itself is never sent.
- **Only for public hosting:** lookups happen only for repositories with a remote on
  github.com, gitlab.com, bitbucket.org or codeberg.org. A repository with no remote, or whose
  remotes are all on a self-hosted server, never has its authors looked up.
- Pictures (and "no picture" answers) are cached on your computer, in Glowfly's cache folder.

## Hosting services and issue trackers

- Glowfly talks to a service's API (GitHub, GitLab, Bitbucket, Azure DevOps, Jira, Trello) to
  show pull requests, issues, build checks, forks and your repositories, and to do what you ask
  there (create a pull request, post a comment, add an SSH key).
- These requests go only to that service, carrying your sign-in for it. What the service keeps
  is up to its own privacy policy.
- A service you connected in Preferences > Integrations is used with the token you gave it.
- For a repository on github.com, gitlab.com, bitbucket.org or Azure DevOps, Glowfly also uses
  a sign-in you already have: a `GITHUB_TOKEN`, `GH_TOKEN`, `GITLAB_TOKEN` or
  `AZURE_DEVOPS_EXT_PAT` environment variable, or one Git Credential Manager already holds. It
  then refreshes that repository's pull requests and checks in the background (about every
  5 minutes). Without a sign-in, it doesn't call the service.
- A self-hosted server is contacted only after you connect it in Preferences.

## Where sign-ins are kept

- Tokens you enter are saved in your system's credential store: Windows Credential Manager on
  Windows, the Keychain on macOS, and the Secret Service (GNOME Keyring, KWallet) on Linux.
  They are never written to Glowfly's settings file.
- A username and password or token typed into Glowfly's git sign-in prompt is saved with git's
  own credential helper when "Remember HTTPS sign-ins typed in Glowfly" is on (Preferences >
  General).
- "Forget all" in Preferences > General disconnects every integration and deletes their saved
  tokens.

## Git

- Fetch, pull, push, clone, submodule updates and Git LFS transfers go to your repositories'
  remotes, through the git you have installed, with git's own configuration and credentials.
- Auto-fetch (Preferences > General, every minute by default; 0 turns it off) fetches every
  remote of the open repository in the background, with its Git LFS files when it uses LFS.

## Things you run yourself

- The terminal panel, custom commands, external tools and git hooks run programs on your
  computer. Whatever those programs send is up to them.
- Links Glowfly shows (a pull request, the website, this page) open in your web browser.

## Files on your computer

- Settings are kept in `settings.json` in Glowfly's config folder (`%APPDATA%\Glowfly` on
  Windows, `~/Library/Application Support/Glowfly` on macOS, `$XDG_CONFIG_HOME/glowfly`, usually
  `~/.config/glowfly`, on Linux).
- Log files (activity, errors, slow operations) are kept in Glowfly's logs folder and never
  uploaded.
- Help > Export logs writes them into one file for a bug report. It holds repository paths,
  branch names and git output, and it goes nowhere unless you share it, so read it before you
  post it anywhere public.

Questions or concerns: open an issue at <https://github.com/bg9m9r/glowfly/issues>.
