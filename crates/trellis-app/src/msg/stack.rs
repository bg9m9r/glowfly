//! Pushing a stack and creating its pull requests (update/stack_push.rs, update/stack_prs.rs).
//! Restacking is `RebaseMsg::Restack`.

use crate::effect::StackPushJob;
use crate::msg::Inactive;
use trellis_core::stack::StackPush;

pub enum StackMsg {
    /// Push every layer of the stack `branch` is in (branch menu, palette).
    Push { branch: String },
    /// Push the stack to the code host's remote where needed, then open the create dialog for
    /// each layer in turn.
    CreatePrs { branch: String },
    /// The push job ended (runner/stack.rs); unscoped like RunGit.
    Pushed { result: Result<StackPush, String>, job: Box<StackPushJob> },
    /// The stack step's Skip in the create dialog.
    SkipPr,
    /// The stack step's Cancel in the create dialog.
    StopPrs,
}

impl StackMsg {
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Push { .. } | Self::CreatePrs { .. } | Self::Pushed { .. } | Self::SkipPr | Self::StopPrs => None,
        }
    }
}
