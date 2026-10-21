//! Preferences > SSH on Windows: whether to offer pointing git at Windows' own OpenSSH (which
//! reads keys from the OpenSSH Authentication Agent service) through the global
//! core.sshCommand, or to undo that. Decided purely from the last agent check (with its
//! `WinSshFacts`) and the global choice; on other systems the facts are None, so it is Hidden.

use super::ssh::SshPage;
use glowfly_core::i18n::Lang;
use glowfly_core::ssh_keys::{GitSsh, KeyChoice, WinService, git_runs};
use std::path::PathBuf;

/// The first OpenSSH version that honours `SSH_ASKPASS_REQUIRE=force`.
const ASKPASS_VERSION: (u32, u32) = (8, 4);

/// What the Agent section's Windows' OpenSSH row shows.
#[derive(Clone, Debug, PartialEq)]
pub enum WindowsOffer {
    Hidden,
    /// git runs its bundled ssh and nothing is chosen for all repositories: offer the switch.
    /// `old` is Windows' ssh's version when it is older than 8.4.
    Offer {
        windows: PathBuf,
        bundled: PathBuf,
        old: Option<(u32, u32)>,
    },
    /// As Offer, but core.sshCommand for all repositories is set (`why`, in the language asked
    /// for).
    Blocked {
        windows: PathBuf,
        bundled: PathBuf,
        why: &'static str,
    },
    /// The global core.sshCommand is Trellis's Windows' OpenSSH value: offer the undo.
    On {
        windows: PathBuf,
        service_running: bool,
        old: Option<(u32, u32)>,
    },
    /// git already runs Windows' OpenSSH some other way.
    Already,
}

/// The row's state for `page`, its reasons in `t`.
pub fn windows_offer(page: &SshPage, t: Lang) -> WindowsOffer {
    let Some(agent) = &page.agent else { return WindowsOffer::Hidden };
    let Some(facts) = &agent.win_ssh else { return WindowsOffer::Hidden };
    let Some(windows) = facts.windows.clone() else { return WindowsOffer::Hidden };
    let Some(global) = page.choice.as_ref().and_then(|c| c.global.as_ref().ok()) else { return WindowsOffer::Hidden };
    let old = facts.windows_version.filter(|v| *v < ASKPASS_VERSION);
    let service_running = agent.service == Some(WinService::Running);
    if let KeyChoice::WindowsSsh(p) = global {
        return WindowsOffer::On { windows: p.clone(), service_running, old };
    }
    if agent.git_ssh_command_set || !service_running {
        return WindowsOffer::Hidden;
    }
    match git_runs(false, global, facts) {
        GitSsh::Bundled => {
            let Some(bundled) = facts.bundled.clone() else { return WindowsOffer::Hidden };
            match global {
                KeyChoice::Unset => WindowsOffer::Offer { windows, bundled, old },
                KeyChoice::Key(_) => WindowsOffer::Blocked { windows, bundled, why: t.tr("prefs.ssh.windows.blocked.key.tip") },
                _ => WindowsOffer::Blocked { windows, bundled, why: t.tr("prefs.ssh.windows.blocked.other.tip") },
            }
        }
        GitSsh::WindowsOwn => WindowsOffer::Already,
        GitSsh::EnvCommand | GitSsh::Other(_) | GitSsh::Unknown => WindowsOffer::Hidden,
    }
}

/// Why the row's button can't run now, in `t`: None only for an Offer or On while no
/// core.sshCommand write runs.
pub fn windows_blocked(page: &SshPage, t: Lang) -> Option<&'static str> {
    if page.choice_busy {
        return Some(t.tr("prefs.ssh.which.busy.tip"));
    }
    match windows_offer(page, t) {
        WindowsOffer::Offer { .. } | WindowsOffer::On { .. } => None,
        WindowsOffer::Blocked { why, .. } => Some(why),
        WindowsOffer::Hidden | WindowsOffer::Already => Some(t.tr("prefs.ssh.windows.unavailable.tip")),
    }
}

#[cfg(test)]
#[path = "ssh_windows_tests.rs"]
mod tests;
