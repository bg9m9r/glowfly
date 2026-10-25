//! The effects the UI shell carries out (`Effect::Ui`): none of them reaches the runner.

use super::execute::{Cx, Execute};
use crate::effect::UiEffect;
use crate::msg::Msg;

/// Carried out by the UI shell; it never reaches the runner.
impl Execute for UiEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        tracing::warn!("UI effect reached the runner; it should be handled by the UI shell");
        None
    }
}
