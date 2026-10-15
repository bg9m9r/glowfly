//! The configured remotes with their raw fetch and push URLs, read from git config (the REMOTE
//! section's tooltips and menus, and the Edit remote dialog's prefill).

use git2::Repository;

/// One configured remote, read raw from git config (before any insteadOf rewriting).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RemoteInfo {
    pub name: String,
    /// Every remote.<name>.url value in config order (git fetches from the first).
    pub urls: Vec<String>,
    /// Every remote.<name>.pushurl value in config order; empty means pushes go to `urls`.
    pub push_urls: Vec<String>,
}

impl RemoteInfo {
    /// The first configured URL, or "" when there is none.
    pub fn fetch_url(&self) -> &str {
        self.urls.first().map(String::as_str).unwrap_or("")
    }

    /// The first configured push URL, raw (what the Edit dialog prefills).
    pub fn push_url(&self) -> Option<&str> {
        self.push_urls.first().map(String::as_str)
    }

    /// The push URL when it differs from the fetch URL.
    pub fn separate_push_url(&self) -> Option<&str> {
        self.push_url().filter(|p| *p != self.fetch_url())
    }
}

/// Every value of the multivar `key`, in config order; empty when it can't be read.
fn values(cfg: &git2::Config, key: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(mut entries) = cfg.multivar(key, None) else { return out };
    while let Some(Ok(e)) = entries.next() {
        if let Some(v) = e.value() {
            out.push(v.to_owned());
        }
    }
    out
}

/// The remotes `names` (in that order) with their URLs. libgit2 splits a key at its first and
/// last dots, so names with '.' or '/' read correctly.
pub(crate) fn remote_info(repo: &Repository, names: &[&str]) -> Vec<RemoteInfo> {
    let cfg = repo.config().ok();
    names
        .iter()
        .map(|name| {
            let (urls, push_urls) = match &cfg {
                Some(c) => (values(c, &format!("remote.{name}.url")), values(c, &format!("remote.{name}.pushurl"))),
                None => (Vec::new(), Vec::new()),
            };
            RemoteInfo { name: (*name).to_owned(), urls, push_urls }
        })
        .collect()
}

/// Each of the remotes `names` (in that order) whose `refs/remotes/<remote>/HEAD` names a
/// branch: (remote, branch without the `refs/remotes/<remote>/` prefix). That is the remote's
/// default branch as last recorded (by clone or `git remote set-head`).
pub(crate) fn remote_heads(repo: &Repository, names: &[&str]) -> Vec<(String, String)> {
    names
        .iter()
        .filter_map(|name| {
            let r = repo.find_reference(&format!("refs/remotes/{name}/HEAD")).ok()?;
            let target = r.symbolic_target()?;
            let branch = target.strip_prefix(&format!("refs/remotes/{name}/"))?;
            (!branch.is_empty()).then(|| ((*name).to_owned(), branch.to_owned()))
        })
        .collect()
}
