//! Signature checks of the active tab's commits, and what the details panel's badge says about
//! one (label and tooltip), worked out without drawing.

use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::signature_check::{SignatureCheck, Verdict};
use glowfly_core::signing::SigningFormat;
use std::collections::{HashMap, HashSet};

/// Signature checks of the active tab's commits, by oid (parked with the tab).
#[derive(Default)]
pub struct Signatures {
    checked: HashMap<Oid, Result<SignatureCheck, String>>,
    pending: HashSet<Oid>,
    /// Bumped by `clear` and parking, and moved past other tabs' on return
    /// (`RepoSession::resume_after`): replies to requests made before carry an older one and
    /// are ignored.
    generation: u64,
}

impl Signatures {
    pub fn get(&self, oid: Oid) -> Option<&Result<SignatureCheck, String>> {
        self.checked.get(&oid)
    }

    pub fn pending(&self, oid: Oid) -> bool {
        self.pending.contains(&oid)
    }

    /// Neither known nor on its way.
    pub fn wanted(&self, oid: Oid) -> bool {
        !self.checked.contains_key(&oid) && !self.pending.contains(&oid)
    }

    /// Marks `oid` as on its way; returns the generation the request must carry.
    pub fn start(&mut self, oid: Oid) -> u64 {
        self.pending.insert(oid);
        self.generation
    }

    /// Stores a result unless the cache was cleared since (`generation` older).
    pub fn finish(&mut self, oid: Oid, generation: u64, result: Result<SignatureCheck, String>) {
        if generation != self.generation {
            return;
        }
        self.pending.remove(&oid);
        self.checked.insert(oid, result);
    }

    /// Drops what is known or on its way for `oid`.
    pub fn forget(&mut self, oid: Oid) {
        self.checked.remove(&oid);
        self.pending.remove(&oid);
    }

    /// Drops everything; replies still in flight are ignored when they arrive.
    pub fn clear(&mut self) {
        self.checked.clear();
        self.pending.clear();
        self.generation += 1;
    }

    /// The request counter, for `RepoReads::counters`.
    pub(super) fn counter(&mut self) -> &mut u64 {
        &mut self.generation
    }

    /// For parking: keeps the results, drops what was on its way; the parked and the
    /// left-behind copy carry the same newer generation, so a reply in flight is ignored by both.
    pub fn parked(&mut self) -> Signatures {
        self.generation += 1;
        let parked = Signatures { checked: std::mem::take(&mut self.checked), pending: HashSet::new(), generation: self.generation };
        self.pending.clear();
        parked
    }
}

/// What the details panel draws for a commit signed in `format`.
#[derive(Clone, Debug, PartialEq)]
pub struct Badge {
    pub verdict: Verdict,
    pub label: &'static str,
    pub tooltip: String,
    /// The finished check, if there is one (for the menu's Copy key).
    pub checked: Option<SignatureCheck>,
}

/// Where a signed commit's check stands.
pub enum BadgeState<'a> {
    /// The repository's git is held (trust): the check would run its signing program.
    Held,
    /// On its way, or about to be asked for.
    Checking,
    Done(&'a Result<SignatureCheck, String>),
}

/// The badge for a commit signed in `format`. Held and Checking read "Signed" and are drawn
/// grey; a finished check is coloured by its verdict.
pub fn badge(t: Lang, format: SigningFormat, state: BadgeState) -> Badge {
    let signed = |tooltip: String| Badge { verdict: Verdict::Signed, label: Verdict::Signed.label(t), tooltip, checked: None };
    match state {
        BadgeState::Held => signed(t.trf("signature.badge.held.tip", &[("format", &format.label())])),
        BadgeState::Checking => signed(t.tr("signature.badge.checking.tip").into()),
        BadgeState::Done(Err(e)) => {
            let error = printable(e);
            signed(t.trf("signature.badge.failed.tip", &[("format", &format.label()), ("error", &error)]))
        }
        BadgeState::Done(Ok(c)) => {
            let verdict = c.verdict();
            let mut tooltip = t.trf("signature.badge.summary", &[("verdict", &verdict.label(t)), ("format", &c.format.label())]);
            for (name, value) in c.details(t) {
                tooltip.push('\n');
                tooltip.push_str(&t.trf("signature.badge.detail", &[("name", &name), ("value", &value)]));
            }
            tooltip.push_str("\n\n");
            tooltip.push_str(&c.reason(t));
            Badge { verdict, label: verdict.label(t), tooltip, checked: Some(c.clone()) }
        }
    }
}

fn printable(s: &str) -> String {
    glowfly_core::text::printable(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::signature_check::{SigStatus, parse_check};

    fn check(format: SigningFormat, stdout: &str, stderr: &str) -> Result<SignatureCheck, String> {
        Ok(parse_check(format, stdout, stderr).unwrap())
    }

    #[test]
    fn held_and_checking_read_signed() {
        let b = badge(Lang::En, SigningFormat::Ssh, BadgeState::Held);
        assert_eq!((b.verdict, b.label), (Verdict::Signed, "Signed"));
        assert!(b.tooltip.starts_with("Signed with an SSH key. Not checked:"), "{}", b.tooltip);
        assert!(b.tooltip.contains("you haven't trusted this repository"));
        let b = badge(Lang::En, SigningFormat::OpenPgp, BadgeState::Checking);
        assert_eq!(b.label, "Signed");
        assert_eq!(b.tooltip, "Checking the signature…");
        assert!(b.checked.is_none());
    }

    #[test]
    fn verified_tooltip_lists_signer_key_and_reason() {
        let fpr = "0123456789ABCDEF0123456789ABCDEF01234567";
        let r = check(SigningFormat::OpenPgp, &format!("G\0Ada <ada@example.com>\0ABCDEF0123456789\0{fpr}\0ultimate\0gpg: Good\n"), "");
        let b = badge(Lang::En, SigningFormat::OpenPgp, BadgeState::Done(&r));
        assert_eq!((b.verdict, b.label), (Verdict::Verified, "Verified"));
        let de = badge(Lang::De, SigningFormat::OpenPgp, BadgeState::Done(&r));
        assert_eq!(de.label, "Verifiziert");
        assert!(de.tooltip.contains("Fingerabdruck: ") && de.tooltip.contains(fpr), "{}", de.tooltip);
        let lines: Vec<&str> = b.tooltip.lines().collect();
        assert_eq!(
            lines,
            [
                "Verified · OpenPGP signature",
                "Signer: Ada <ada@example.com>",
                "Key: ABCDEF0123456789",
                &format!("Fingerprint: {fpr}"),
                "Trust: ultimate",
                "",
                "Good signature from a key you trust.",
            ]
        );
        assert_eq!(b.checked.unwrap().key, "ABCDEF0123456789");
    }

    #[test]
    fn each_verdict_has_its_label() {
        for (letter, label) in [('U', "Signed"), ('X', "Expired"), ('Y', "Expired"), ('R', "Revoked"), ('B', "Bad signature")] {
            let r = check(SigningFormat::OpenPgp, &format!("{letter}\0\0K\0\0\0x\n"), "");
            let b = badge(Lang::En, SigningFormat::OpenPgp, BadgeState::Done(&r));
            assert_eq!(b.label, label, "{letter}");
            assert!(b.tooltip.starts_with(&format!("{label} · OpenPGP signature\nKey: K\n\n")), "{}", b.tooltip);
        }
    }

    #[test]
    fn no_allowed_signers_points_at_the_settings() {
        let stderr = "error: gpg.ssh.allowedSignersFile needs to be configured and exist for ssh signature verification";
        let r = check(SigningFormat::Ssh, "N\0\0\0\0undefined\0\n", stderr);
        assert_eq!(r.as_ref().unwrap().status, SigStatus::NoAllowedSigners);
        let b = badge(Lang::En, SigningFormat::Ssh, BadgeState::Done(&r));
        assert_eq!(b.label, "Signed");
        assert!(b.tooltip.starts_with("Signed · SSH signature\n\n"), "{}", b.tooltip);
        assert!(b.tooltip.contains("Preferences > Commit signing"));
    }

    #[test]
    fn a_failed_check_says_why() {
        let r = Err("git log failed: fatal: bad object\u{202E}".to_owned());
        let b = badge(Lang::En, SigningFormat::X509, BadgeState::Done(&r));
        assert_eq!((b.verdict, b.label), (Verdict::Signed, "Signed"));
        assert_eq!(b.tooltip, "Signed · X.509 signature\n\nGlowfly couldn't check this signature: git log failed: fatal: bad object");
    }

    #[test]
    fn cache_generations() {
        let oid = Oid::from_bytes(&[1; 20]).unwrap();
        let mut s = Signatures::default();
        assert!(s.wanted(oid));
        let g = s.start(oid);
        assert!(s.pending(oid) && !s.wanted(oid));
        s.clear();
        assert!(s.wanted(oid));
        s.finish(oid, g, Err("old".into()));
        assert!(s.get(oid).is_none(), "a reply from before the clear is ignored");
        let g = s.start(oid);
        s.finish(oid, g, Err("x".into()));
        assert!(s.get(oid).is_some() && !s.pending(oid));
        s.forget(oid);
        assert!(s.wanted(oid));
        let other = Oid::from_bytes(&[2; 20]).unwrap();
        let g = s.start(oid);
        s.finish(oid, g, Err("x".into()));
        let g = s.start(other);
        let mut parked = s.parked();
        assert!(parked.get(oid).is_some() && parked.wanted(other), "results kept, pending dropped");
        assert!(s.wanted(oid) && s.wanted(other));
        assert_eq!(*parked.counter(), *s.counter(), "both copies carry the same newer generation");
        assert!(*s.counter() > g);
        parked.finish(other, g, Err("old".into()));
        s.finish(other, g, Err("old".into()));
        assert!(parked.get(other).is_none() && s.get(other).is_none(), "a reply from before the park is ignored by both");
    }
}
