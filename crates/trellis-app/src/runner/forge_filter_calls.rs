//! Calls of pull request and issue filters (`ForgeCall::Filter`): the signed-in user, issue
//! searches and the filter dialog's options.

use crate::effect::{FilterCall, FilterResult, ForgeError};
use trellis_core::forge::filters::{filter_options, find_issues, me};
use trellis_core::forge::{Auth, Target};

pub(super) fn call(t: &Target, auth: &Auth, c: FilterCall) -> FilterResult {
    match c {
        FilterCall::Me => FilterResult::Me(me(t, auth).map_err(ForgeError::from)),
        FilterCall::Issues { choice, conditions, me } => {
            let result = find_issues(t, auth, &conditions, me.as_ref()).map_err(ForgeError::from);
            FilterResult::Issues { choice, conditions, result }
        }
        FilterCall::Options(kind) => FilterResult::Options(kind, filter_options(t, auth, kind).map_err(ForgeError::from)),
    }
}
