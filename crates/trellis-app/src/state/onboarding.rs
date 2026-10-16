//! The first-run welcome (its steps and the identity form) and where the tour of the main window
//! is (`onboarding_tour.rs`). Plain data and the pure rules of the identity step; only
//! `settings.welcome_done` is ever saved.

use super::onboarding_tour::TourStep;
use trellis_core::identity::Identity;

/// The activity log line after the identity step wrote the name and email.
pub const SAVED_LOG: &str = "Saved your name and email to the global git config";
pub const NAME_BLANK: &str = "Enter the name to put on your commits.";
pub const EMAIL_BLANK: &str = "Enter the email address to put on your commits.";
pub const EMAIL_NO_AT: &str = "An email address has an @ in it.";
pub const EMAIL_SPACE: &str = "An email address can't contain spaces.";
/// Why the identity step won't write while a profile switch runs.
pub const NOT_DURING_SWITCH: &str = "A profile switch is writing your git config. Try again when it ends.";
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
    pub fn caption(self) -> &'static str {
        match self {
            WelcomeStep::Identity => "You",
            WelcomeStep::Connect => "Services",
            WelcomeStep::Git => "Git",
            WelcomeStep::Start => "Start",
        }
    }

    /// The step's heading.
    pub fn title(self) -> &'static str {
        match self {
            WelcomeStep::Identity => "Who are you?",
            WelcomeStep::Connect => "Connect a hosting service",
            WelcomeStep::Git => "Checking git",
            WelcomeStep::Start => "You're ready",
        }
    }
}

/// The open welcome.
pub struct Welcome {
    pub step: WelcomeStep,
    pub identity: IdentityForm,
    /// "Show me around once it opens" (edited in place by the view).
    pub tour_after: bool,
}

impl Welcome {
    /// At the first step, waiting for the identity read.
    pub fn new() -> Self {
        Welcome { step: WelcomeStep::Identity, identity: IdentityForm { reading: true, ..IdentityForm::default() }, tour_after: true }
    }
}

impl Default for Welcome {
    fn default() -> Self {
        Self::new()
    }
}

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
    /// The first problem with the fields, or git's error.
    pub error: Option<String>,
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

    /// Whether the trimmed fields differ from what git had.
    pub fn needs_write(&self) -> bool {
        self.name.trim() != self.had.name.trim() || self.email.trim() != self.had.email.trim()
    }
}

/// The first problem with a name and email for commits, or None when they can be written.
pub fn identity_problem(name: &str, email: &str) -> Option<&'static str> {
    let email = email.trim();
    if name.trim().is_empty() {
        Some(NAME_BLANK)
    } else if email.is_empty() {
        Some(EMAIL_BLANK)
    } else if !email.contains('@') {
        Some(EMAIL_NO_AT)
    } else if email.chars().any(char::is_whitespace) {
        Some(EMAIL_SPACE)
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
        assert_eq!(WelcomeStep::ALL.map(WelcomeStep::caption), ["You", "Services", "Git", "Start"]);
    }

    #[test]
    fn identity_checks() {
        assert_eq!(identity_problem(" ", "a@b"), Some(NAME_BLANK));
        assert_eq!(identity_problem("Ada", " "), Some(EMAIL_BLANK));
        assert_eq!(identity_problem("Ada", "ada"), Some(EMAIL_NO_AT));
        assert_eq!(identity_problem("Ada", "ada @x"), Some(EMAIL_SPACE));
        assert_eq!(identity_problem(" Ada ", " ada@example.com "), None);
        let mut f = IdentityForm::default();
        f.seed(Identity { name: "Ada".into(), email: "ada@example.com".into(), signing_key: None });
        assert!(!f.needs_write() && f.git_knows());
        f.name = " Ada ".into();
        assert!(!f.needs_write(), "only trimmed text counts");
        f.email = "ada@work.example".into();
        assert!(f.needs_write());
    }
}
