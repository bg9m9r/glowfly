//! The reason shared by every action that can't start while another one runs.

/// Why an action can't start while another one runs.
/// In English for the preference pages and dialogs that aren't translated yet; `busy` otherwise.
pub const BUSY: &str = "Another operation is still running.";

/// `BUSY` in the language.
pub fn busy(t: glowfly_core::i18n::Lang) -> &'static str {
    t.tr("common.busy")
}
