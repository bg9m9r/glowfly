//! What a pull request or issue filter asks a hosting service (`ForgeCall::Filter`), and what
//! comes back (`ForgeResult::Filter`).

use super::ForgeError;
use trellis_core::forge::{Conditions, FilterChoice, FilterKind, FilterOptions, Issue, Me};

#[derive(Debug)]
pub enum FilterCall {
    /// The signed-in user, for conditions on "me"; `epoch` is the code host's account epoch.
    Me { epoch: u32 },
    /// Open issues matching `conditions` (the filter `choice`); `me` saves looking the user up.
    /// `epoch` is the tracker's account epoch.
    Issues { choice: FilterChoice, conditions: Conditions, me: Option<Me>, epoch: u32 },
    /// Labels, milestones and sprints for the filter dialog.
    Options(FilterKind),
}

#[derive(Debug)]
pub enum FilterResult {
    /// Carries the call's `epoch`, so a reply asked under an earlier account is dropped.
    Me {
        epoch: u32,
        result: Result<Me, ForgeError>,
    },
    /// Carries what was asked, so a reply for a filter no longer shown (or asked under an
    /// earlier account) is dropped. `me` is the user the search had to look up, if it did.
    Issues {
        choice: FilterChoice,
        conditions: Conditions,
        epoch: u32,
        me: Option<Me>,
        result: Result<Vec<Issue>, ForgeError>,
    },
    Options(FilterKind, Result<FilterOptions, ForgeError>),
}
