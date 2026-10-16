//! What a pull request or issue filter asks a hosting service (`ForgeCall::Filter`), and what
//! comes back (`ForgeResult::Filter`).

use super::ForgeError;
use trellis_core::forge::{Conditions, FilterChoice, FilterKind, FilterOptions, Issue, Me};

#[derive(Debug)]
pub enum FilterCall {
    /// The signed-in user, for conditions on "me".
    Me,
    /// Open issues matching `conditions` (the filter `choice`); `me` saves looking the user up.
    Issues { choice: FilterChoice, conditions: Conditions, me: Option<Me> },
    /// Labels, milestones and sprints for the filter dialog.
    Options(FilterKind),
}

#[derive(Debug)]
pub enum FilterResult {
    Me(Result<Me, ForgeError>),
    /// Carries what was asked, so a reply for a filter no longer shown is dropped.
    Issues {
        choice: FilterChoice,
        conditions: Conditions,
        result: Result<Vec<Issue>, ForgeError>,
    },
    Options(FilterKind, Result<FilterOptions, ForgeError>),
}
