//! The first-run welcome (its steps and the identity form) and where the tour of the main window
//! is (`onboarding_tour.rs`). Plain data and the pure rules of the identity step; only
//! `settings.welcome_done` is ever saved.

use super::onboarding_tour::TourStep;
use super::touched::Touched;
use glowfly_core::i18n::Lang;
use glowfly_core::identity::Identity;

/// The activity log line after the identity step wrote the name and email.
pub const SAVED_LOG: &str = "Saved your name and email to the global git config";
/// The toast after the tour's last step.
pub const TOUR_DONE: &str = "That's the tour. The command palette can run it again.";

/// The welcome and the tour (global, not per tab).
#[derive(Default)]
pub struct Onboarding {
    /// The welcome, while it is open.
    pub welcome: Option<Welcome>,
    /// The tour's step, while it runs.
    pub tour: Option<TourStep>,
    /// The step the tour was closed on, for "Resume the tour" (this session only).
    pub tour_resume: Option<TourStep>,
    /// Start the tour when the next repository finishes opening (the welcome's checkbox).
    pub tour_after_open: bool,
}

/// The welcome's steps, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WelcomeStep {
    Identity,
    Connect,
    Git,
    Start,
}

impl WelcomeStep {
    pub const ALL: [WelcomeStep; 4] = [WelcomeStep::Identity, WelcomeStep::Connect, WelcomeStep::Git, WelcomeStep::Start];

    /// Its place, from 0.
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    pub fn next(self) -> Option<WelcomeStep> {
        Self::ALL.get(self.index() + 1).copied()
    }

    pub fn prev(self) -> Option<WelcomeStep> {
        self.index().checked_sub(1).map(|i| Self::ALL[i])
    }

    /// The short caption under its progress dot.
    pub fn caption(self, t: Lang) -> &'static str {
        match self {
            WelcomeStep::Identity => t.tr("welcome.step.identity.caption"),
            WelcomeStep::Connect => t.tr("welcome.step.connect.caption"),
            WelcomeStep::Git => t.tr("welcome.step.git.caption"),
            WelcomeStep::Start => t.tr("welcome.step.start.caption"),
        }
    }

    /// The step's heading.
    pub fn title(self, t: Lang) -> &'static str {
        match self {
            WelcomeStep::Identity => t.tr("welcome.step.identity.title"),
            WelcomeStep::Connect => t.tr("welcome.step.connect.title"),
            WelcomeStep::Git => t.tr("welcome.step.git.title"),
            WelcomeStep::Start => t.tr("welcome.step.start.title"),
        }
    }
}

/// The open welcome.
pub struct Welcome {
    pub step: WelcomeStep,
    pub identity: IdentityForm,
    /// "Show me around once it opens" (edited in place by the view).
    pub tour_after: bool,
    /// "Show authors' pictures from Gravatar" (edited in place by the view): seeded from
    /// `settings.avatar_lookups` and written back when the welcome closes.
    pub avatars: bool,
}

impl Welcome {
    /// At the first step, waiting for the identity read.
    pub fn new() -> Self {
        Welcome {
            step: WelcomeStep::Identity,
            identity: IdentityForm { reading: true, ..IdentityForm::default() },
            tour_after: true,
            avatars: false,
        }
    }

    /// The name the header greets, after the identity step: the one git now holds (seeded from
    /// the global config or just written), never a typed name that was skipped.
    pub fn greeting_name(&self) -> Option<&str> {
        let name = self.identity.had.name.trim();
        (self.step != WelcomeStep::Identity && !name.is_empty()).then_some(name)
    }
}

impl Default for Welcome {
    fn default() -> Self {
        Self::new()
    }
}

/// The identity form's field names, for its `Touched`.
pub const NAME_FIELD: &str = "name";
pub const EMAIL_FIELD: &str = "email";

/// The identity step's form.
#[derive(Default)]
pub struct IdentityForm {
    /// The global config is being read.
    pub reading: bool,
    /// Why the read failed.
    pub read_error: Option<String>,
    /// What git had (empty when the read failed).
    pub had: Identity,
    /// Text buffers, edited in place.
    pub name: String,
    pub email: String,
    /// The write is running.
    pub saving: bool,
    /// The answer to the last try: git's write error or a refusal (profile switch); field
    /// problems are worked out by `field_problems`.
    pub error: Option<String>,
    /// Which fields were edited, and whether Continue was pressed with a problem.
    pub touched: Touched,
}

impl IdentityForm {
    /// Fills the form from git's identity.
    pub fn seed(&mut self, id: Identity) {
        self.name = id.name.trim().to_owned();
        self.email = id.email.trim().to_owned();
        self.had = id;
        self.reading = false;
    }

    /// Whether git had both a name and an email.
    pub fn git_knows(&self) -> bool {
        !self.had.name.trim().is_empty() && !self.had.email.trim().is_empty()
    }

    /// The name's problem and the email's, from the current text.
    pub fn field_problems(&self, t: Lang) -> (Option<&'static str>, Option<&'static str>) {
        (name_problem(t, &self.name), email_problem(t, &self.email))
    }

    /// Whether the trimmed fields differ from what git had.
    pub fn needs_write(&self) -> bool {
        self.name.trim() != self.had.name.trim() || self.email.trim() != self.had.email.trim()
    }
}

/// The first problem with a name and email for commits, or None when they can be written.
pub fn identity_problem(t: Lang, name: &str, email: &str) -> Option<&'static str> {
    name_problem(t, name).or_else(|| email_problem(t, email))
}

/// The problem with a name for commits: only a blank one.
pub fn name_problem(t: Lang, name: &str) -> Option<&'static str> {
    name.trim().is_empty().then(|| t.tr("welcome.identity.name_blank"))
}

/// The first problem with an email for commits: blank, no @, or spaces inside.
pub fn email_problem(t: Lang, email: &str) -> Option<&'static str> {
    let email = email.trim();
    if email.is_empty() {
        Some(t.tr("welcome.identity.email_blank"))
    } else if !email.contains('@') {
        Some(t.tr("welcome.identity.email_no_at"))
    } else if email.chars().any(char::is_whitespace) {
        Some(t.tr("welcome.identity.email_space"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_in_order() {
        assert_eq!(WelcomeStep::Identity.prev(), None);
        assert_eq!(WelcomeStep::Identity.next(), Some(WelcomeStep::Connect));
        assert_eq!(WelcomeStep::Start.next(), None);
        assert_eq!(WelcomeStep::Start.prev(), Some(WelcomeStep::Git));
        assert_eq!(WelcomeStep::ALL.map(|s| s.caption(Lang::En)), ["You", "Services", "Git", "Start"]);
    }

    #[test]
    fn identity_checks() {
        let en = |key| Some(Lang::En.tr(key));
        assert_eq!(identity_problem(Lang::En, " ", "a@b"), en("welcome.identity.name_blank"));
        assert_eq!(identity_problem(Lang::En, "Ada", " "), en("welcome.identity.email_blank"));
        assert_eq!(identity_problem(Lang::En, "Ada", "ada"), en("welcome.identity.email_no_at"));
        assert_eq!(identity_problem(Lang::En, "Ada", "ada @x"), en("welcome.identity.email_space"));
        assert_eq!(identity_problem(Lang::En, " Ada ", " ada@example.com "), None);
        assert_eq!(identity_problem(Lang::En, "Ada", "ada"), Some("An email address has an @ in it."));
        assert_eq!(identity_problem(Lang::De, "", "a@b"), Some(Lang::De.tr("welcome.identity.name_blank")));
        assert_ne!(identity_problem(Lang::De, "", "a@b"), en("welcome.identity.name_blank"));
        assert_eq!(identity_problem(Lang::En, "", "ada"), en("welcome.identity.name_blank"), "the name's problem first");
        assert_eq!(name_problem(Lang::En, " "), en("welcome.identity.name_blank"));
        assert_eq!(name_problem(Lang::En, "Ada"), None);
        assert_eq!(email_problem(Lang::En, " "), en("welcome.identity.email_blank"));
        assert_eq!(email_problem(Lang::En, "ada"), en("welcome.identity.email_no_at"));
        assert_eq!(email_problem(Lang::En, "ada @x"), en("welcome.identity.email_space"));
        assert_eq!(email_problem(Lang::En, "ada x"), en("welcome.identity.email_no_at"), "no @ is named before spaces");
        assert_eq!(email_problem(Lang::En, " ada@example.com "), None);
        let mut f = IdentityForm::default();
        f.seed(Identity { name: "Ada".into(), email: "ada@example.com".into(), signing_key: None });
        assert!(!f.needs_write() && f.git_knows());
        f.name = " Ada ".into();
        assert!(!f.needs_write(), "only trimmed text counts");
        f.email = "ada@work.example".into();
        assert!(f.needs_write());
    }

    #[test]
    fn field_problems() {
        let mut f = IdentityForm::default();
        f.seed(Identity::default());
        let tr = |t: Lang, k| Some(t.tr(k));
        assert_eq!(f.field_problems(Lang::En), (tr(Lang::En, "welcome.identity.name_blank"), tr(Lang::En, "welcome.identity.email_blank")));
        (f.name, f.email) = ("Ada".into(), "ada".into());
        assert_eq!(f.field_problems(Lang::En), (None, tr(Lang::En, "welcome.identity.email_no_at")));
        assert_eq!(f.field_problems(Lang::De), (None, tr(Lang::De, "welcome.identity.email_no_at")));
    }

    #[test]
    fn greeting_name() {
        let ada = Identity { name: "Ada Lovelace".into(), email: "ada@example.com".into(), signing_key: None };
        let mut w = Welcome::new();
        w.identity.seed(ada.clone());
        assert_eq!(w.greeting_name(), None, "not on the identity step");
        w.step = WelcomeStep::Connect;
        assert_eq!(w.greeting_name(), Some("Ada Lovelace"));
        w.identity.had.name = "  ".into();
        assert_eq!(w.greeting_name(), None, "whitespace only");
        let mut w = Welcome::new();
        w.identity.seed(Identity::default());
        w.identity.name = "Ada".into();
        w.step = WelcomeStep::Connect;
        assert_eq!(w.greeting_name(), None, "a typed name isn't git's");
    }
}
