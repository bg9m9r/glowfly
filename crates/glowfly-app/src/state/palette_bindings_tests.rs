//! The Repository commands bindings and their palette commands map onto each other both ways.

use super::command_for;
use crate::keymap::{BindingId, COMMAND_BINDINGS};
use crate::state::palette::PaletteCommand;

#[test]
fn binding_and_command_for_agree() {
    for id in COMMAND_BINDINGS {
        let cmd = command_for(id).unwrap_or_else(|| panic!("{id:?} runs a command"));
        assert_eq!(cmd.binding(), Some(id), "{cmd:?}");
    }
    for id in BindingId::ALL.into_iter().filter(|id| !COMMAND_BINDINGS.contains(id)) {
        assert_eq!(command_for(id), None, "{id:?}");
    }
    assert_eq!(PaletteCommand::CreateTag { annotated: true }.binding(), Some(BindingId::CreateAnnotatedTag));
    assert_eq!(PaletteCommand::CreateTag { annotated: false }.binding(), Some(BindingId::CreateTag));
    assert_eq!(PaletteCommand::Refresh.binding(), Some(BindingId::Refresh), "the older map is kept");
}
