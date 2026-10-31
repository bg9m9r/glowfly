//! Building the confirmation dialogs (`Modal::Confirm`, `Modal::ConfirmDanger` and
//! `Modal::ConfirmChoice`) outside `update`, and whether one may run its step.

use super::Modal;
use crate::msg::Msg;

impl Modal {
    /// A danger confirmation for a step that loses work (no acknowledgement).
    pub fn danger(title: impl Into<String>, body: impl Into<String>, button: impl Into<String>, then: Msg) -> Modal {
        Modal::ConfirmDanger {
            title: title.into(),
            body: body.into(),
            button: button.into(),
            ack: None,
            acked: false,
            then: Box::new(then),
        }
    }

    /// This danger confirmation with an acknowledgement labelled `label` to tick first.
    pub fn with_ack(self, label: impl Into<String>) -> Modal {
        match self {
            Modal::ConfirmDanger { title, body, button, then, .. } => {
                Modal::ConfirmDanger { title, body, button, ack: Some(label.into()), acked: false, then }
            }
            other => other,
        }
    }

    /// Whether this is a confirmation whose step may run now: any `Confirm`, and a
    /// `ConfirmDanger` whose acknowledgement (if it has one) is ticked.
    pub fn confirm_ready(&self) -> bool {
        match self {
            Modal::Confirm { .. } | Modal::ConfirmDetails { .. } | Modal::ConfirmChoice { .. } => true,
            Modal::ConfirmDanger { ack, acked, .. } => ack.is_none() || *acked,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_acknowledgement_holds_the_step_until_ticked() {
        let m = Modal::danger("Delete?", "Gone for good.", "Delete", Msg::ClearLog);
        assert!(m.confirm_ready());
        let mut m = m.with_ack("I understand");
        assert!(matches!(&m, Modal::ConfirmDanger { ack: Some(a), acked: false, .. } if a == "I understand"));
        assert!(!m.confirm_ready());
        if let Modal::ConfirmDanger { acked, .. } = &mut m {
            *acked = true;
        }
        assert!(m.confirm_ready());
        assert!(!Modal::About.confirm_ready());
    }
}
