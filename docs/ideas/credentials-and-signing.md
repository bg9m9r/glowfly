# Ideas: Credentials, SSH and signing

Credential and passphrase prompts, SSH keys, signing and profiles. Untriaged; see [README.md](README.md).

### Credential prompts

- Caps Lock warning on password fields (egui doesn't expose Caps Lock state yet)
- Choose between a password, personal access token or app password per hosting service, with a
  link to create a token
- A non-blocking notice for SSH security-key touch requests (`SSH_ASKPASS_PROMPT=none`)
- Hosting-service sign-in without Git Credential Manager through the prompt (Preferences >
  Integrations)
- Approve the sign-ins for the remotes that worked in a multi-remote fetch that failed overall
- End an auto-fetch pause after a pull or a dashboard fetch of the remote works, not only fetch
  jobs
- Keep paused auto-fetch remotes across restarts
- A desktop notification when a remote needs a sign-in
- Move a Linux user from the `cache` fallback to libsecret once git-credential-libsecret is installed
- Drop rejected git LFS sign-ins too (git-lfs's failure output names its endpoint differently)
- A status-bar chip listing paused remotes once their banner lines are dismissed
- Let the dialog's Remember choice update the default in Preferences > General
- The `DISPLAY=:0` placeholder set for SSH passphrase prompts may make ssh print an X11 warning with `ForwardX11 yes` when `DISPLAY` was unset and a command-line ssh is in use; pass `-o ForwardX11=no` for Glowfly's own runs (the placeholder itself is needed for OpenSSH before 8.4)
- Make the Linux credential cache probe's path unpredictable (it sits at a fixed name in `/tmp`)
- A global `cache` credential helper also runs alongside URL-scoped helpers; say so, or scope it
- Decide whether to ask before turning on the cache in the global git config, or pass `-c credential.helper=cache` to Glowfly's own git calls only

### Signing beyond config

- Signature badges on graph rows (batch-check visible signed commits, cached by oid, throttled
  like Stats)
- Signature status in multi-commit views (details_multi, compare) and the file history list
- Flag a verified signature whose signer identity doesn't match the committer's email
- Verify signed tags in the TAGS section and tag tooltips
- Show signatures on merged tags (mergetag headers) of merge commits
- Show the SSH key type and GPG primary user ID / primary-key fingerprint in the badge tooltip
- Warn on the Commit Signing page when gpg.ssh.allowedSignersFile points to a missing file or a
  repository-local value overrides the global one

### SSH key management

- Generate RSA or ECDSA keys, and change an existing key's passphrase or comment (ssh-keygen -p
  / -c)
- A Browse button for the Generate SSH key dialog's save location
- Show in folder and delete a key locally from its row
- Flag encrypted private keys and keys whose file permissions are too open for ssh on their row
- Palette entry for the SSH page (palette_commands.rs needs splitting first)
- Start the Windows OpenSSH Authentication Agent service (needs an administrator) or an ssh-agent on Unix from the SSH page
- Per-repository key choice in a repository with no trust decision (trust it and choose the key in one step)
- Remove a key from the agent (ssh-add -d), and add with a lifetime (-t) or confirmation (-c)
- Add keys to Bitbucket Data Center and Azure DevOps through their APIs
- List and delete the keys registered on a connected service, and show on each key row which services have it
- Per-host key choice through ~/.ssh/config Host entries instead of core.sshCommand
- Palette entries for Use for git, Add to agent and Add to service (after palette_commands.rs is split)
- Link from the add-key fallback to reconnect a service with a token that may write SSH keys (Integrations, preselected)
- Resolve host-dependent IdentityFile tokens (%h, %r, %p, %n, %C, %k, %j) per literal Host pattern, or ask `ssh -G <host>`
- Read IdentityFile entries from the system ssh_config (/etc/ssh/ssh_config, %ProgramData%\ssh\ssh_config)
- Include patterns with wildcards in folder names, and tokens or environment variables in Include
- List CertificateFile entries next to their keys
- Keep Windows' OpenSSH when choosing a key for all repositories (`<Windows ssh> -i <key> -o IdentitiesOnly=yes`)
- Offer Windows' OpenSSH for one repository's own core.sshCommand
- Warn on Windows when HOME and USERPROFILE differ (Windows' OpenSSH reads USERPROFILE\.ssh, Git's ssh HOME/.ssh)
- Set up prompts for Windows' OpenSSH older than 8.4 instead of only warning
- Fill in fingerprints from the .pub file (SHA-256 of the key blob) when ssh-keygen is missing
- Match agent keys when the user's FingerprintHash is md5
- Open ~/.ssh/config in the editor from the SSH page
- Show which ~/.ssh/config Host entry and key ssh would use for the open repository's remote

### SSH preferences

- Translate the page's remaining English: the toasts, log lines and confirmations in update/ssh.rs, ssh_use.rs and ssh_service.rs
- Cancel on the Replace SSH key confirmation goes back to the Generate SSH key dialog instead of closing
- Suggest a free file name (id_ed25519_2, …) as the save location when the default key already exists
- Show remembered keys whose files have gone, with Remove from list, instead of skipping them silently
- Show the key core.sshCommand names as a card when it lives outside .ssh and wasn't added with Use existing
- Cut a long repository name in the "{repo} only" row label with … and a hover tip
- Move the section header with a right-hand slot (title, pill, button) into prefs/form.rs once another page needs it
- Offer to recreate the .pub (ssh-keygen -y) when a picked private key has none
- Key type choice (RSA 4096, ECDSA, security keys) in the Generate SSH key dialog
- Click the agent pill to check the agent again or jump to its detail lines
- Remember a picked key whose path isn't valid Unicode (settings.json keeps paths as text; such a key is listed until the page reloads)

### Profiles

- Per-profile signing format and program (gpg.format, gpg.program, commit.gpgsign) and SSH key
  (core.sshCommand)
- Profile icon or avatar picker, and a profile switcher in the toolbar or status bar
- Import and export of profiles, and sharing them between machines
- A keyboard shortcut for Switch profile
- Separate stored sign-ins per profile for the same service account (e.g. two GitHub users on github.com)
- Warn when connecting an account another profile already uses with a different user
- Per-profile diff header toggles (inline/split layout, hunks/whole file, ignore whitespace, word wrap)
- Per-profile External Tools (editor and terminal programs)
- Per-profile repository-specific settings (issue tracker, hidden and shown-only refs)
- A "Saved with profile X" caption on the General, Appearance and Diff viewer pages
- Copy or reset one profile's preferences from another profile on the Profile page
- Refresh an open Clone dialog's account list when the profile switches
- Show a profile's folder-rule count on its row in the profile list (e.g. "Ada <a@b> · 2 folders")
- Offer to unset a repository's own user.signingkey too when using a profile there, and name it in the confirmation
- Clean up Glowfly's includeIf entries and include files from the global config (a "Remove all folder rules" action, or on uninstall)
- Handle an includeIf section the user shares with a Glowfly entry (extra keys in the same subsection) instead of leaving it in place
- Rules on other conditions git offers (hasconfig:remote.*.url for a hosting-service account, onbranch:)
- Show which profile applies in the tab bar or the repository list, not only in the commit box and the Profile page
- A "Pick a profile for this folder" prompt when cloning or initializing into a folder no rule covers
- Per-rule signing settings (gpg.format, commit.gpgsign) written into the profile's include file
- Remove a repository's own identity from config.worktree too (extensions.worktreeConfig) when using a profile there
- Re-read the identity when the global or XDG git config, or a profile's include file, changes outside Glowfly (only the repository's config is watched)
- Name the committer in the commit box too when GIT_COMMITTER_NAME or GIT_COMMITTER_EMAIL is set (the line names the author)
- Warn about GIT_AUTHOR_DATE and GIT_COMMITTER_DATE in the environment as well
- Offer to keep GIT_AUTHOR_* and GIT_COMMITTER_* from Glowfly's git children (env_remove), so the config's identity applies
- Warn about the identity variables in the Repo > Identity page and the welcome identity step too
- Read author.name/author.email and committer.* (between the environment and user.*) and git's EMAIL fallback when reading the effective identity

### GPG passphrase prompts

- Passphrase prompts for X.509 (gpgsm) signing through gpg.x509.program
- A git banner reason saying git older than 2.31 keeps GnuPG's own passphrase window
- Use Glowfly's prompt when Glowfly was started with GIT_CONFIG_COUNT set (append at the next
  index)
- Wording for smartcard PIN and touch requests (loopback with a card is unverified)
- Show the key's expiry date in the passphrase dialog
- Signing passphrase prompts for git commands typed in the terminal panel
- Ask again after a wrong SSH signing passphrase (ssh-keygen stops at the first; would need
  Glowfly as gpg.ssh.program re-running -Y sign on git's payload file)
- Reuse the SSH signing passphrase for the rest of a job that signs many commits (rebase,
  cherry-pick of a range)
- Offer to add the signing key to ssh-agent from the SSH signing passphrase dialog
- Wording for FIDO (sk) SSH signing keys: ssh-keygen's PIN prompt and touch notice
- Check that Windows' own OpenSSH ssh-keygen (System32) honours SSH_ASKPASS when set as
  gpg.ssh.program
- SSH signing passphrase marker for push --signed (push certificates)
- Map MSYS-style gpg.ssh.program and gpg.x509.program values on Windows (git runs those itself,
  not the wrapper)
- Find an MSYS2 or Cygwin git's install root from the git program or PATH when GIT_EXEC_PATH
  arrives in POSIX form
- Map MSYS /tmp in gpg.program to the user's temp folder
