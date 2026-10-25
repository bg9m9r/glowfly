//! Which service answers each review call; services without in-app reviews answer
//! `Error::Unsupported` without sending anything.

use super::super::{Auth, Provider, Target};
use super::{
    Entry, MergeRequest, Merged, PrPage, ReviewRequest, bitbucket, bitbucket_dc, bitbucket_merge, checked, github, gitlab, thread,
    unsupported,
};
use crate::error::Result;

/// The services with in-app reviews.
enum Service {
    GitHub,
    GitLab,
    Cloud,
    DataCenter,
}

fn service(p: Provider) -> Option<Service> {
    match p {
        Provider::GitHub | Provider::GitHubEnterprise => Some(Service::GitHub),
        Provider::GitLab | Provider::GitLabSelfManaged => Some(Service::GitLab),
        Provider::Bitbucket => Some(Service::Cloud),
        Provider::BitbucketDataCenter => Some(Service::DataCenter),
        _ => None,
    }
}

/// Whether comments, reviews and merging work inside Glowfly for this service.
pub fn reviews_in_app(p: Provider) -> bool {
    service(p).is_some()
}

/// Whether code comment threads (their replies, replying and resolving) work inside Glowfly
/// for this service.
pub fn threads_in_app(p: Provider) -> bool {
    matches!(p, Provider::GitHub | Provider::GitHubEnterprise)
}

/// The pull request's page: its detail and conversation.
pub fn pr_page(t: &Target, auth: &Auth, id: u64) -> Result<PrPage> {
    let mut page = match service(t.provider).ok_or_else(|| unsupported(t))? {
        Service::GitHub => github::pr_page(t, auth, id)?,
        Service::GitLab => gitlab::pr_page(t, auth, id)?,
        Service::Cloud => bitbucket::pr_page(t, auth, id)?,
        Service::DataCenter => bitbucket_dc::pr_page(t, auth, id)?,
    };
    page.entries = page.entries.into_iter().map(|e| checked(t, e)).collect();
    page.threads = page.threads.into_iter().map(|th| thread::checked_thread(t, th)).collect();
    Ok(page)
}

/// Adds a comment to the conversation; returns it as the service stored it.
pub fn post_comment(t: &Target, auth: &Auth, id: u64, body: &str) -> Result<Entry> {
    let e = match service(t.provider).ok_or_else(|| unsupported(t))? {
        Service::GitHub => github::post_comment(t, auth, id, body)?,
        Service::GitLab => gitlab::post_comment(t, auth, id, body)?,
        Service::Cloud => bitbucket::post_comment(t, auth, id, body)?,
        Service::DataCenter => bitbucket_dc::post_comment(t, auth, id, body)?,
    };
    Ok(checked(t, e))
}

/// Submits a review, or finishes the user's `pending` one (with its comments).
pub fn submit_review(t: &Target, auth: &Auth, id: u64, req: &ReviewRequest) -> Result<()> {
    match service(t.provider).ok_or_else(|| unsupported(t))? {
        Service::GitHub => github::submit_review(t, auth, id, req),
        Service::GitLab => gitlab::submit_review(t, auth, id, req),
        Service::Cloud => bitbucket::submit_review(t, auth, id, req),
        Service::DataCenter => bitbucket_dc::submit_review(t, auth, id, req),
    }
}

pub fn merge(t: &Target, auth: &Auth, id: u64, req: &MergeRequest) -> Result<Merged> {
    match service(t.provider).ok_or_else(|| unsupported(t))? {
        Service::GitHub => github::merge(t, auth, id, req),
        Service::GitLab => gitlab::merge(t, auth, id, req),
        Service::Cloud => bitbucket_merge::cloud(t, auth, id, req),
        Service::DataCenter => bitbucket_merge::data_center(t, auth, id, req),
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::mock::{Mock, Reply};
    use super::super::*;
    use super::*;
    use crate::error::Error;

    #[test]
    fn reviews_in_app_for_github_gitlab_and_bitbucket() {
        use Provider::*;
        for p in Provider::ALL {
            assert_eq!(
                reviews_in_app(p),
                matches!(p, GitHub | GitHubEnterprise | GitLab | GitLabSelfManaged | Bitbucket | BitbucketDataCenter),
                "{p:?}"
            );
        }
    }

    #[test]
    fn threads_in_app_only_for_github() {
        for p in Provider::ALL {
            assert_eq!(threads_in_app(p), matches!(p, Provider::GitHub | Provider::GitHubEnterprise), "{p:?}");
        }
    }

    #[test]
    fn other_services_send_nothing() {
        let m = Mock::start(|_| Reply::not_found());
        for p in [Provider::AzureDevOps, Provider::JiraCloud] {
            let t = Target { provider: p, host: "example.com".into(), owner: "o".into(), project: "p".into(), repo: "r".into() };
            let auth = Auth::from_token(p, None, "tok", "test");
            let req = MergeRequest { method: MergeMethod::Merge, head_sha: "abc".into(), title: String::new(), message: String::new() };
            let review = ReviewRequest { event: ReviewEvent::Approve, body: String::new(), pending: None, head_sha: "abc".into() };
            let errors = [
                pr_page(&t, &auth, 1).unwrap_err(),
                post_comment(&t, &auth, 1, "hi").unwrap_err(),
                submit_review(&t, &auth, 1, &review).unwrap_err(),
                merge(&t, &auth, 1, &req).unwrap_err(),
                reply_to_thread(&t, &auth, 1, 2, "hi").unwrap_err(),
                resolve_thread(&t, &auth, "T_1", true).unwrap_err(),
            ];
            for e in errors {
                assert!(matches!(&e, Error::Unsupported(m) if m.contains(p.name())), "{p:?}: {e}");
            }
        }
        assert!(m.requests().is_empty());
    }

    /// Services with in-app reviews but no code comment threads refuse thread calls unsent.
    fn threads_send_nothing(providers: &[Provider], host: &str, name: &str) {
        let m = Mock::start(|_| Reply::not_found());
        for &p in providers {
            let t = Target { provider: p, host: host.into(), owner: "g".into(), project: String::new(), repo: "app".into() };
            let auth = Auth::from_token(p, None, "tok", "test");
            let errors = [reply_to_thread(&t, &auth, 1, 2, "hi").unwrap_err(), resolve_thread(&t, &auth, "T_1", true).unwrap_err()];
            for e in errors {
                assert!(matches!(&e, Error::Unsupported(m) if m.contains(name)), "{p:?}: {e}");
            }
        }
        assert!(m.requests().is_empty());
    }

    #[test]
    fn gitlab_threads_send_nothing() {
        threads_send_nothing(&[Provider::GitLab, Provider::GitLabSelfManaged], "gitlab.com", "GitLab");
    }

    #[test]
    fn bitbucket_threads_send_nothing() {
        threads_send_nothing(&[Provider::Bitbucket, Provider::BitbucketDataCenter], "bitbucket.org", "Bitbucket");
    }

    #[test]
    fn the_author_is_matched_without_case() {
        let mut d = github::tests::detail();
        d.author = "Ada".into();
        d.me = "ada".into();
        assert!(d.is_author());
        d.me = String::new();
        d.author = String::new();
        assert!(!d.is_author(), "an unknown user is nobody's author");
    }
}
