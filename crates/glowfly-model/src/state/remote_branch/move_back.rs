//! The fast-forward-then-checkout job of a tracker that is only behind its upstream, kept while
//! it runs so that a checkout failing after the branch moved can put the branch back: whether
//! a failed job's output shows the branch moved, the command and lease for moving it back, and
//! the log and toast texts that say where the branch ended up. Pure.

use git2::Oid;
use glowfly_core::branch_ops;
use glowfly_core::i18n::Lang;
use glowfly_core::text::command_echo;

/// Which job of the pair is running.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FfPhase {
    /// The fast-forward and the checkout.
    Checkout,
    /// The quiet job moving the branch back after the checkout failed.
    MovingBack,
}

/// The running fast-forward-then-checkout job of a tracker (update/branch_checkout.rs), or the
/// job moving the branch back after its checkout failed (update/branch_checkout_back.rs).
#[derive(Clone, Debug, PartialEq)]
pub struct FastForwardCheckout {
    pub branch: String,
    pub upstream: String,
    pub from: Oid,
    pub to: Oid,
    /// The job's `update-ref` command, as echoed in its output.
    pub update_ref: Vec<String>,
    /// The job's `checkout` command, as echoed in its output.
    pub checkout: Vec<String>,
    pub phase: FfPhase,
}

impl FastForwardCheckout {
    /// Whether the failed job's `output` shows that the branch was fast-forwarded: the
    /// `update-ref` ran and did not fail. The runner echoes each command it runs and stops at
    /// the first failure, so the update-ref counts when the checkout's echo follows it, or when
    /// no failure line does (the trust gate held the checkout and says so in plain words).
    pub fn moved(&self, output: &str) -> bool {
        let (update_ref, checkout) = (command_echo(&self.update_ref), command_echo(&self.checkout));
        let mut lines = output.lines().skip_while(|l| *l != update_ref);
        if lines.next().is_none() {
            return false;
        }
        let after: Vec<&str> = lines.collect();
        after.contains(&checkout.as_str()) || !after.iter().any(|l| l.starts_with("fatal: ") || l.starts_with("error: "))
    }

    /// The command that moves the branch back.
    pub fn move_back_cmd(&self) -> Vec<String> {
        branch_ops::move_back_cmd(&self.branch, self.from)
    }

    /// The branch's ref and where the fast-forward left it: the move-back runs only while it
    /// still is there.
    pub fn expect(&self) -> (String, Oid) {
        (format!("refs/heads/{}", self.branch), self.to)
    }

    /// The log line when the branch is back where it was.
    pub fn moved_back(&self, t: Lang) -> String {
        t.trf(
            "remote_branch.checkout.moved_back",
            &[("branch", &self.branch), ("upstream", &self.upstream), ("from", &short(self.from)), ("to", &short(self.to))],
        )
    }

    /// The log line when the branch could not be moved back, with git's `reason`.
    pub fn not_moved_back(&self, t: Lang, reason: &str) -> String {
        t.trf(
            "remote_branch.checkout.not_moved_back.log",
            &[
                ("branch", &self.branch),
                ("upstream", &self.upstream),
                ("from", &short(self.from)),
                ("to", &short(self.to)),
                ("reason", &reason),
            ],
        )
    }

    /// The toast when the branch could not be moved back.
    pub fn not_moved_back_toast(&self, t: Lang) -> String {
        t.trf("remote_branch.checkout.not_moved_back.toast", &[("branch", &self.branch), ("from", &short(self.from))])
    }
}

/// The first 7 characters of `o`.
fn short(o: Oid) -> String {
    o.to_string()[..7].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::gitcli::args;

    fn pending() -> FastForwardCheckout {
        let branch = "feature";
        let (from, to) = (Oid::from_str(&"a".repeat(40)).unwrap(), Oid::from_str(&"b".repeat(40)).unwrap());
        let ff = branch_ops::fast_forward_ref_cmds(branch, from, to, "origin/feature");
        FastForwardCheckout {
            branch: branch.into(),
            upstream: "origin/feature".into(),
            from,
            to,
            update_ref: ff[1].clone(),
            checkout: args(&["checkout", branch, "--"]),
            phase: FfPhase::Checkout,
        }
    }

    fn echoes(p: &FastForwardCheckout) -> (String, String) {
        (command_echo(&p.update_ref), command_echo(&p.checkout))
    }

    #[test]
    fn moved_when_the_checkout_failed_after_the_update_ref() {
        let p = pending();
        let (u, c) = echoes(&p);
        assert!(p.moved(&format!("$ x\n{u}\n{c}\nerror: untracked\n")));
    }

    #[test]
    fn moved_when_the_gate_held_the_checkout() {
        let p = pending();
        let (u, _) = echoes(&p);
        assert!(p.moved(&format!("{u}\nStopped: trust\n")));
    }

    #[test]
    fn not_moved_when_the_update_ref_failed_or_never_ran() {
        let p = pending();
        let (u, _) = echoes(&p);
        assert!(!p.moved(&format!("{u}\nfatal: cannot lock ref 'refs/heads/feature'\n")));
        assert!(!p.moved("$ git merge-base --is-ancestor a b\n"));
        assert!(!p.moved(""));
    }
}
