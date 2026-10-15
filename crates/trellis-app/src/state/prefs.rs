//! Preferences screen state: the open page, editable mirrors of git config values, and the
//! Integrations connect form.

use trellis_core::forge::{Account, Provider};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PrefPage {
    General,
    Profile,
    Integrations,
    Ui,
    /// Diff text: font size, tab size, line numbers, syntax and word highlights.
    Editor,
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
            PrefPage::Profile => "Profile",
            PrefPage::Integrations => "Integrations",
            PrefPage::Ui => "UI Customization",
            PrefPage::Editor => "Editor",
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

/// Git config values shown in Preferences, read in the background when Preferences opens.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GitConfigValues {
    pub longpaths: bool,
    pub autocrlf: bool,
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
    pub signing: SigningState,
    pub lfs: crate::state::lfs::LfsPage,
    pub gitflow: crate::state::gitflow::GitFlowPage,
    pub sparse: crate::state::sparse::SparsePage,
    pub ssh: crate::state::ssh::SshPage,
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
            signing: SigningState::default(),
            lfs: Default::default(),
            gitflow: Default::default(),
            sparse: Default::default(),
            ssh: Default::default(),
        }
    }
}

/// What a "Host Domain" field shows for a saved account.
pub fn host_text(a: &Account) -> String {
    if a.provider == Provider::AzureDevOps { format!("{}/{}", a.host, a.org) } else { a.host.clone() }
}
