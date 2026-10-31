//! The pull request panel's tests (`update/pull_request/`): the glob import keeps their
//! `super::…` paths to the panel's handlers working.

#[allow(unused_imports)]
use crate::update::pull_request::*;

mod bitbucket_tests;
mod calls_tests;
mod checks_tests;
mod cleanup_tests;
mod commits_tests;
mod gitlab_tests;
mod line_comment_leave_tests;
mod line_comment_reply_tests;
mod line_comment_tests;
mod line_map_tests;
mod merge_tests;
mod source_tests;
pub(crate) mod tests;
mod threads_tests;
