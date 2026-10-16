//! Calls of pull request and issue filters (`ForgeCall::Filter`): the signed-in user, issue
//! searches and the filter dialog's options.

use crate::effect::{FilterCall, FilterResult, ForgeError};
use trellis_core::forge::filters::{filter_options, find_issues, matching, me};
use trellis_core::forge::{Auth, FilterKind, Target};

pub(super) fn call(t: &Target, auth: &Auth, c: FilterCall) -> FilterResult {
    match c {
        FilterCall::Me { epoch } => FilterResult::Me { epoch, result: me(t, auth).map_err(ForgeError::from) },
        FilterCall::Issues { choice, conditions, me: known, epoch } => {
            // A search that names the user looks them up once and hands them back, so later
            // searches and the pull request filters don't ask again.
            let lookup = known.is_none()
                && matching::needs_me(FilterKind::Issues, &conditions)
                && matching::issue_search_names_user(t.provider)
                && matching::unsupported(t.provider, FilterKind::Issues, &conditions).is_none();
            let looked_up = match lookup.then(|| me(t, auth)) {
                Some(Err(e)) => return FilterResult::Issues { choice, conditions, epoch, me: None, result: Err(e.into()) },
                Some(Ok(m)) => Some(m),
                None => None,
            };
            let result = find_issues(t, auth, &conditions, known.as_ref().or(looked_up.as_ref())).map_err(ForgeError::from);
            FilterResult::Issues { choice, conditions, epoch, me: looked_up, result }
        }
        FilterCall::Options(kind) => FilterResult::Options(kind, filter_options(t, auth, kind).map_err(ForgeError::from)),
    }
}
