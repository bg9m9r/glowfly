//! Clones and the hosting accounts the user connected: an https clone from a connected host
//! fetches that host's SSH-named submodules over https too (`rewrites_for`), and an SSH clone
//! that fails for want of a key or a known host key, on a connected host, offers Clone over
//! HTTPS as <user> (`offer`), which runs the same clone over https.

use crate::msg::{JobMsg, Msg};
use crate::state::job_spec::Retry;
use crate::state::{AppState, ToastAction};
use glowfly_core::clone_https::{https_for_ssh, https_rewrites};
use glowfly_core::forge::{self, Connection};
use glowfly_core::i18n::Lang;

/// The job label every Clone dialog clone runs under (update/clone.rs).
const CLONE: &str = "Clone";

/// The connection for `host` whose service Glowfly clones from (the ones the Clone dialog
/// lists), first as Integrations lists them.
fn connection_for<'a>(connections: &'a [Connection], host: &str) -> Option<&'a Connection> {
    connections.iter().find(|c| forge::lists_my_repos(c.account.provider) && c.account.host.eq_ignore_ascii_case(host))
}

/// The `-c` options for a clone of `url`: when it is https on a host with a connected account,
/// that host's SSH URLs (in `.gitmodules`) are fetched over https as well, with the same sign-in.
/// SSH stays the user's explicit choice: an SSH clone gets none.
pub(super) fn rewrites_for(connections: &[Connection], url: &str) -> Vec<String> {
    match forge::https_host(url.trim()) {
        Some(host) if connection_for(connections, &host).is_some() => https_rewrites(&host),
        _ => Vec::new(),
    }
}

/// Where a clone command's URL is (`clone_cmd`'s `-- <url> <name>`).
fn url_index(cmd: &[String]) -> Option<usize> {
    let sub = cmd.iter().position(|a| a == "clone")?;
    let dash = cmd[sub..].iter().position(|a| a == "--")? + sub;
    (dash + 1 < cmd.len()).then_some(dash + 1)
}

/// Longer user names are cut, so the button stays a button.
const MAX_USER: usize = 20;

fn short_user(user: &str) -> String {
    let user = user.trim();
    if user.chars().count() <= MAX_USER { user.to_owned() } else { format!("{}…", user.chars().take(MAX_USER - 1).collect::<String>()) }
}

/// Clone over HTTPS as <user>, for a failed Clone job (`again`, its kept spec) whose SSH URL is on
/// a host with a connected account. Not when the clone got as far as its submodules: the new
/// folder is there, so cloning again would fail.
pub(super) fn offer(st: &AppState, t: Lang, again: Option<&Retry>, output: &str) -> Option<ToastAction> {
    let r = again.filter(|r| r.spec.label == CLONE && r.spec.cmds.len() == 1)?;
    if output.contains("registered for path") {
        return None;
    }
    let cmd = r.spec.cmds.first()?;
    let at = url_index(cmd)?;
    let (host, https) = https_for_ssh(&cmd[at])?;
    let c = connection_for(&st.settings.connections, &host)?;
    let user = if c.user.trim().is_empty() { host.clone() } else { short_user(&c.user) };
    let mut url_swapped = cmd.clone();
    url_swapped[at] = https;
    let over_https = [https_rewrites(&host), url_swapped].concat();
    let mut retry = r.clone();
    retry.spec.cmds = vec![over_https];
    let label = t.trf("git_errors.clone_https.button", &[("user", &user as &dyn std::fmt::Display)]);
    Some(ToastAction { label, msg: Box::new(Msg::Job(JobMsg::Retry(Box::new(retry)))), workdir: r.workdir.clone() })
}
