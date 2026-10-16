//! Preferences screen state: the open page, editable mirrors of git config values, and the
//! Integrations connect form.

use trellis_core::forge::{Account, Provider};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PrefPage {
    General,
    /// Which git program Trellis runs.
    Git,
    Profile,
    Integrations,
    Ui,
    /// Diff text: font size, tab size, line numbers, syntax and word highlights.
    Editor,
    /// Keyboard shortcuts: changing, adding and removing keys.
    Keyboard,
    /// External editor and terminal programs.
    Tools,
    Signing,
    /// SSH key pairs in ~/.ssh and generating one.
    Ssh,
    RepoIdentity,
    RepoHooks,
    RepoTracker,
    RepoLfs,
    RepoGitFlow,
    RepoSparse,
}

impl PrefPage {
    pub fn title(self) -> &'static str {
        match self {
            PrefPage::General => "General",
            PrefPage::Git => "Git",
            PrefPage::Profile => "Profile",
            PrefPage::Integrations => "Integrations",
            PrefPage::Ui => "UI Customization",
            PrefPage::Editor => "Editor",
            PrefPage::Keyboard => "Keyboard Shortcuts",
            PrefPage::Tools => "External Tools",
            PrefPage::Signing => "Commit Signing",
            PrefPage::Ssh => "SSH",
            PrefPage::RepoIdentity => "Identity",
            PrefPage::RepoHooks => "Git Hooks",
            PrefPage::RepoTracker => "Issue Tracker",
            PrefPage::RepoLfs => "Git LFS",
            PrefPage::RepoGitFlow => "Git Flow",
            PrefPage::RepoSparse => "Sparse Checkout",
        }
    }
}

/// The global `core.autocrlf`: three values git knows, or unset, or text git can't read (shown
/// as it is, never rewritten unasked).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Autocrlf {
    #[default]
    Unset,
    True,
    Input,
    False,
    Other(String),
}

impl Autocrlf {
    /// The choices Preferences offers, with their labels.
    pub const CHOICES: [(Autocrlf, &'static str); 4] = [
        (Autocrlf::Unset, "Not set (git's default: false)"),
        (Autocrlf::True, "true: check out CRLF, commit LF"),
        (Autocrlf::Input, "input: commit LF, check out as is"),
        (Autocrlf::False, "false: never convert"),
    ];

    /// How it is shown.
    pub fn label(&self) -> String {
        match self {
            Autocrlf::Other(raw) => format!("{raw} (not a value git knows)"),
            known => Self::CHOICES.iter().find(|(c, _)| c == known).map(|(_, l)| (*l).to_owned()).unwrap_or_default(),
        }
    }

    /// The value to write; None unsets it.
    pub fn value(&self) -> Option<String> {
        match self {
            Autocrlf::Unset => None,
            Autocrlf::True => Some("true".into()),
            Autocrlf::Input => Some("input".into()),
            Autocrlf::False => Some("false".into()),
            Autocrlf::Other(raw) => Some(raw.clone()),
        }
    }
}

/// Git config values shown in Preferences, read in the background when Preferences opens.
/// Booleans are read as git reads them (`yes`, `on`, `1`…).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GitConfigValues {
    pub longpaths: bool,
    pub autocrlf: Autocrlf,
    pub user_name: String,
    pub user_email: String,
    pub gpgsign: bool,
    pub signing_format: String,
    pub signing_key: String,
    pub repo_name: String,
    pub repo_email: String,
    pub hooks_path: String,
    pub gpg_program: String,
    pub ssh_program: String,
    pub x509_program: String,
    pub tag_gpgsign: bool,
    pub ssh_allowed_signers: String,
}

impl GitConfigValues {
    /// The configured program for the current signing format ("" means git's default).
    pub fn program(&self) -> &str {
        match trellis_core::signing::SigningFormat::from_config(&self.signing_format) {
            trellis_core::signing::SigningFormat::OpenPgp => &self.gpg_program,
            trellis_core::signing::SigningFormat::Ssh => &self.ssh_program,
            trellis_core::signing::SigningFormat::X509 => &self.x509_program,
        }
    }
}

/// Commit Signing page state.
#[derive(Default)]
pub struct SigningState {
    /// None while the list is loading.
    pub keys: Option<Vec<trellis_core::signing::SigningKey>>,
    pub keys_error: Option<String>,
    pub passphrase: String,
    pub generating: bool,
}

pub struct PrefsState {
    pub page: PrefPage,
    /// None until the background read finishes.
    pub git: Option<GitConfigValues>,
    pub int_provider: Provider,
    pub int_host: String,
    pub int_user: String,
    pub int_token: String,
    pub int_busy: bool,
    pub int_error: Option<String>,
    pub jira_project: String,
    pub trello_boards: crate::state::trello::BoardList,
    pub signing: SigningState,
    pub lfs: crate::state::lfs::LfsPage,
    pub gitflow: crate::state::gitflow::GitFlowPage,
    pub sparse: crate::state::sparse::SparsePage,
    pub ssh: crate::state::ssh::SshPage,
    pub profiles: crate::state::profiles::ProfilesPage,
    pub keys: crate::state::keys_page::KeysPage,
}

impl PrefsState {
    pub fn new(page: PrefPage) -> Self {
        Self {
            page,
            git: None,
            int_provider: Provider::GitHub,
            int_host: String::new(),
            int_user: String::new(),
            int_token: String::new(),
            int_busy: false,
            int_error: None,
            jira_project: String::new(),
            trello_boards: Default::default(),
            signing: SigningState::default(),
            lfs: Default::default(),
            gitflow: Default::default(),
            sparse: Default::default(),
            ssh: Default::default(),
            profiles: Default::default(),
            keys: Default::default(),
        }
    }
}

/// What a "Host Domain" field shows for a saved account.
pub fn host_text(a: &Account) -> String {
    if a.provider == Provider::AzureDevOps { format!("{}/{}", a.host, a.org) } else { a.host.clone() }
}
