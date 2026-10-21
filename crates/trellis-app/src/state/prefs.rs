//! Preferences screen state: the open page, editable mirrors of git config values (and the
//! values as read, for the Save / Revert bar), the Integrations connect form and the leave
//! waiting on the unsaved-changes question.

use trellis_core::forge::{Account, Provider};
use trellis_core::i18n::Lang;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PrefPage {
    General,
    /// Which git program Trellis runs.
    Git,
    Profile,
    Integrations,
    Ui,
    /// The interface language and how dates and times are written.
    Language,
    /// Diff text: font size, tab size, line numbers, syntax and word highlights.
    DiffViewer,
    /// Keyboard shortcuts: changing, adding and removing keys.
    Keyboard,
    /// External editor, diff and merge tools.
    Tools,
    /// The built-in terminal and the external terminal Open in terminal starts.
    Terminal,
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
    /// The title above the page's form.
    pub fn title(self, t: Lang) -> &'static str {
        match self {
            PrefPage::Keyboard => t.tr("prefs.title.keyboard"),
            other => other.nav_label(t),
        }
    }

    /// The page's entry in the navigation column.
    pub fn nav_label(self, t: Lang) -> &'static str {
        match self {
            PrefPage::General => t.tr("prefs.nav.general"),
            PrefPage::Git => t.tr("prefs.nav.git"),
            PrefPage::Profile => t.tr("prefs.nav.profile"),
            PrefPage::Integrations => t.tr("prefs.nav.integrations"),
            PrefPage::Ui => t.tr("prefs.nav.ui"),
            PrefPage::Language => t.tr("prefs.nav.language"),
            PrefPage::DiffViewer => t.tr("prefs.nav.diff_viewer"),
            PrefPage::Keyboard => t.tr("prefs.nav.keyboard"),
            PrefPage::Tools => t.tr("prefs.nav.tools"),
            PrefPage::Terminal => t.tr("prefs.nav.terminal"),
            PrefPage::Signing => t.tr("prefs.nav.signing"),
            PrefPage::Ssh => t.tr("prefs.nav.ssh"),
            PrefPage::RepoIdentity => t.tr("prefs.nav.identity"),
            PrefPage::RepoHooks => t.tr("prefs.nav.hooks"),
            PrefPage::RepoTracker => t.tr("prefs.nav.tracker"),
            PrefPage::RepoLfs => t.tr("prefs.nav.lfs"),
            PrefPage::RepoGitFlow => t.tr("prefs.nav.gitflow"),
            PrefPage::RepoSparse => t.tr("prefs.nav.sparse"),
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
    pub fn choices(t: Lang) -> [(Autocrlf, &'static str); 4] {
        [
            (Autocrlf::Unset, t.tr("prefs.general.autocrlf.unset")),
            (Autocrlf::True, t.tr("prefs.general.autocrlf.true")),
            (Autocrlf::Input, t.tr("prefs.general.autocrlf.input")),
            (Autocrlf::False, t.tr("prefs.general.autocrlf.false")),
        ]
    }

    /// How it is shown.
    pub fn label(&self, t: Lang) -> String {
        match self {
            Autocrlf::Other(raw) => t.trf("prefs.general.autocrlf.unknown", &[("value", raw)]),
            known => Self::choices(t).iter().find(|(c, _)| c == known).map(|(_, l)| (*l).to_owned()).unwrap_or_default(),
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
    /// The git config values as last read or saved: what the Save / Revert bar compares with.
    pub git_saved: Option<GitConfigValues>,
    /// The leave waiting on the "Save your changes?" question.
    pub leaving: Option<Box<crate::msg::Msg>>,
    /// Lets the one replayed leave through the unsaved-changes guard.
    pub leave_approved: bool,
    /// The "Saved" tick beside the page title.
    pub saved: crate::state::prefs_save::SavedFlash,
    /// The Integrations sign-in form is shown beside connected accounts.
    pub int_adding: bool,
    /// git config's diff and merge tools for the External tools page; None until read.
    pub git_tools: Option<trellis_core::external_tools::git_config::GitTools>,
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
            git_saved: None,
            leaving: None,
            leave_approved: false,
            saved: Default::default(),
            int_adding: false,
            git_tools: None,
        }
    }
}

/// What a "Host domain" field shows for a saved account.
pub fn host_text(a: &Account) -> String {
    if a.provider == Provider::AzureDevOps { format!("{}/{}", a.host, a.org) } else { a.host.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autocrlf_labels_follow_the_language_and_keep_raw_values() {
        assert_eq!(Autocrlf::Input.label(Lang::En), "input: commit LF, check out as is");
        assert_eq!(Autocrlf::Other("weird".into()).label(Lang::En), "weird (not a value git knows)");
        let de = Autocrlf::Other("weird".into()).label(Lang::De);
        assert!(de.starts_with("weird ") && de != "weird (not a value git knows)", "{de}");
        assert_eq!(PrefPage::Keyboard.title(Lang::En), "Keyboard shortcuts");
        assert_eq!(PrefPage::Keyboard.nav_label(Lang::En), "Keyboard");
        assert_eq!(PrefPage::General.title(Lang::De), PrefPage::General.nav_label(Lang::De));
    }
}
