//! Reading how a gpg run ended from its `[GNUPG:]` status lines (git passes `--status-fd=2`,
//! so they are on stderr). Pure.

/// The outcome of one signing run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignResult {
    Created,
    /// The agent needs a passphrase and wasn't allowed to start a pinentry.
    NeedsPassphrase,
    BadPassphrase,
    Cancelled,
    /// This GnuPG can't do what was asked (no `--pinentry-mode`, or loopback refused).
    Unsupported,
    Failed,
}

/// libgpg-error codes (the low 16 bits of a status line's error value).
const NO_PIN_ENTRY: u32 = 85;
const BAD_PASSPHRASE: u32 = 11;
const CANCELED: u32 = 99;
const NOT_SUPPORTED: u32 = 60;

/// Classifies a run from its exit status and stderr.
pub fn classify(exit_ok: bool, stderr: &str) -> SignResult {
    let status: Vec<&str> = stderr.lines().filter_map(|l| l.trim_end().strip_prefix("[GNUPG:] ")).collect();
    if exit_ok && status.iter().any(|s| s.starts_with("SIG_CREATED ")) {
        return SignResult::Created;
    }
    if status.iter().any(|s| s.split_whitespace().next() == Some("BAD_PASSPHRASE")) {
        return SignResult::BadPassphrase;
    }
    for s in &status {
        let mut words = s.split_whitespace();
        if !matches!(words.next(), Some("FAILURE" | "ERROR")) {
            continue;
        }
        // `FAILURE <where> <code>`.
        let Some(code) = words.nth(1).and_then(|c| c.parse::<u32>().ok()) else { continue };
        match code & 0xFFFF {
            NO_PIN_ENTRY => return SignResult::NeedsPassphrase,
            BAD_PASSPHRASE => return SignResult::BadPassphrase,
            CANCELED => return SignResult::Cancelled,
            NOT_SUPPORTED => return SignResult::Unsupported,
            _ => {}
        }
    }
    let lower = stderr.to_ascii_lowercase();
    if status.is_empty() && (lower.contains("invalid option") || lower.contains("unknown option")) {
        return SignResult::Unsupported;
    }
    SignResult::Failed
}
