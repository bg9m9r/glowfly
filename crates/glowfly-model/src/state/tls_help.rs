//! The TLS settings a certificate or revocation failure's Details show (ui/activity_fold.rs):
//! the note kept per failed run, and its lines in words, each setting with its value and where
//! it is set, plus whether git ignores `http.sslCAInfo` under schannel. Session memory only:
//! the activity file and Copy entry don't carry it. Pure.

use super::logs::{EntryId, FIX_LIMIT, LogsState};
use glowfly_core::error::ReadFailed;
use glowfly_core::i18n::Lang;
use glowfly_core::tls_config::{Origin, Scope, Setting, TlsFailure, TlsSettings, shown_key};

/// What the runner read for a failed run: the kind of failure and the settings (or why they
/// couldn't be read).
#[derive(Clone, Debug, PartialEq)]
pub struct TlsNote {
    pub failure: TlsFailure,
    pub settings: Result<TlsSettings, ReadFailed>,
}

/// Keeps `note` for run `entry`, replacing one it had; the newest `FIX_LIMIT` are kept.
pub fn keep(logs: &mut LogsState, entry: EntryId, note: TlsNote) {
    logs.tls.retain(|(i, _)| *i != entry);
    logs.tls.push((entry, note));
    if logs.tls.len() > FIX_LIMIT {
        logs.tls.drain(..logs.tls.len() - FIX_LIMIT);
    }
}

/// Run `entry`'s note, when the settings were read for it.
pub fn note(logs: &LogsState, entry: EntryId) -> Option<&TlsNote> {
    logs.tls.iter().find(|(i, _)| *i == entry).map(|(_, n)| n)
}

/// One line of the settings block; `warn` draws it in the warning colour.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlsLine {
    pub text: String,
    pub warn: bool,
}

fn plain(text: String) -> TlsLine {
    TlsLine { text, warn: false }
}

/// The settings block's lines for `note`: for a certificate failure the backend, the CA file
/// (`GIT_SSL_CAINFO` first when set) and schannelUseSSLCAInfo, then what schannel does with
/// them; for a revocation failure the backend and schannelCheckRevoke.
pub fn lines(t: Lang, note: &TlsNote) -> Vec<TlsLine> {
    let s = match &note.settings {
        Ok(s) => s,
        Err(e) => return vec![plain(t.trf("tls.settings.unreadable", &[("error", &super::error_words::typed::read(t, e))]))],
    };
    let mut out = Vec::new();
    out.push(plain(match &s.backend {
        Some(b) => setting_text(t, "sslbackend", b),
        None => t.tr("tls.settings.backend_unset").to_owned(),
    }));
    match note.failure {
        TlsFailure::Certificate => {
            let ca = match (&s.ca_info_env, &s.ca_info) {
                (Some(env), _) => {
                    let key = shown_key("sslcainfo");
                    t.trf("tls.settings.value", &[("key", &key), ("value", env), ("origin", &t.tr("tls.origin.env"))])
                }
                (None, Some(c)) => setting_text(t, "sslcainfo", c),
                (None, None) => unset(t, "sslcainfo"),
            };
            out.push(plain(ca));
            out.push(plain(
                s.use_ca_info.as_ref().map_or_else(|| unset(t, "schannelusesslcainfo"), |u| setting_text(t, "schannelusesslcainfo", u)),
            ));
            if s.ca_info_ignored() {
                out.push(TlsLine { text: t.tr("tls.settings.ca_ignored").to_owned(), warn: true });
            } else if s.schannel() && s.ca_info.is_none() && s.ca_info_env.is_none() {
                out.push(plain(t.tr("tls.settings.windows_store").to_owned()));
            }
        }
        TlsFailure::Revocation => {
            out.push(plain(
                s.check_revoke.as_ref().map_or_else(|| unset(t, "schannelcheckrevoke"), |c| setting_text(t, "schannelcheckrevoke", c)),
            ));
        }
    }
    out
}

/// "{key} isn't set".
fn unset(t: Lang, var: &str) -> String {
    t.trf("tls.settings.unset", &[("key", &shown_key(var))])
}

/// "{key} = {value} ({origin})", with "for {url}" for a URL's own key.
fn setting_text(t: Lang, var: &str, s: &Setting) -> String {
    let (key, origin) = (shown_key(var), origin_text(t, &s.origin));
    if origin.is_empty() {
        return t.trf("tls.settings.value_bare", &[("key", &key), ("value", &s.value)]);
    }
    match &s.url {
        Some(url) => t.trf("tls.settings.value_for_url", &[("key", &key), ("value", &s.value), ("url", url), ("origin", &origin)]),
        None => t.trf("tls.settings.value", &[("key", &key), ("value", &s.value), ("origin", &origin)]),
    }
}

/// Where a value is set, in words: "global settings, <path>", "the command line", or the bare
/// path when git didn't say the scope.
fn origin_text(t: Lang, o: &Origin) -> String {
    match o {
        Origin::File { scope, path } => {
            let p: [(&str, &dyn std::fmt::Display); 1] = [("path", path)];
            match scope {
                Some(Scope::System) => t.trf("tls.origin.system", &p),
                Some(Scope::Global) => t.trf("tls.origin.global", &p),
                Some(Scope::Local) => t.trf("tls.origin.local", &p),
                Some(Scope::Worktree) => t.trf("tls.origin.worktree", &p),
                Some(Scope::Command) => t.tr("tls.origin.command").to_owned(),
                None => t.trf("tls.origin.file", &p),
            }
        }
        Origin::CommandLine => t.tr("tls.origin.command").to_owned(),
        Origin::Other(s) => s.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EN: Lang = Lang::En;

    fn set(value: &str, scope: Scope, path: &str) -> Option<Setting> {
        Some(Setting { value: value.into(), origin: Origin::File { scope: Some(scope), path: path.into() }, url: None })
    }

    fn texts(note: &TlsNote) -> Vec<String> {
        lines(EN, note).into_iter().map(|l| l.text).collect()
    }

    #[test]
    fn certificate_lines_say_when_the_ca_file_is_ignored() {
        let s = TlsSettings {
            backend: set("schannel", Scope::System, "/etc/gitconfig"),
            ca_info: set("/home/ada/certs/corp.pem", Scope::Global, "/home/ada/.gitconfig"),
            ..TlsSettings::default()
        };
        let note = TlsNote { failure: TlsFailure::Certificate, settings: Ok(s.clone()) };
        let all = lines(EN, &note);
        assert_eq!(all[0].text, "http.sslBackend = schannel (system settings, /etc/gitconfig)");
        assert_eq!(all[1].text, "http.sslCAInfo = /home/ada/certs/corp.pem (global settings, /home/ada/.gitconfig)");
        assert_eq!(all[2].text, "http.schannelUseSSLCAInfo isn't set");
        assert!(all[3].warn && all[3].text.starts_with("Git ignores http.sslCAInfo here"), "{:?}", all[3]);
        // GIT_SSL_CAINFO overrides the config value.
        let env =
            TlsNote { failure: TlsFailure::Certificate, settings: Ok(TlsSettings { ca_info_env: Some("/e.pem".into()), ..s.clone() }) };
        assert_eq!(texts(&env)[1], "http.sslCAInfo = /e.pem (the GIT_SSL_CAINFO environment variable)");
        // No CA file: the Windows store.
        let store = TlsNote { failure: TlsFailure::Certificate, settings: Ok(TlsSettings { ca_info: None, ..s }) };
        let all = lines(EN, &store);
        assert_eq!(all[1].text, "http.sslCAInfo isn't set");
        assert_eq!(
            all.last().map(|l| (l.text.as_str(), l.warn)),
            Some(("With schannel, Git trusts the authorities in the Windows certificate store.", false))
        );
        let none = TlsNote { failure: TlsFailure::Certificate, settings: Ok(TlsSettings::default()) };
        assert_eq!(
            texts(&none),
            [
                "http.sslBackend isn't set: curl's default TLS backend is used",
                "http.sslCAInfo isn't set",
                "http.schannelUseSSLCAInfo isn't set"
            ]
        );
    }

    #[test]
    fn revocation_lines_and_unreadable_settings() {
        let url = Setting { value: "true".into(), origin: Origin::CommandLine, url: Some("https://h.example/".into()) };
        let s = TlsSettings { backend: set("schannel", Scope::Local, ".git/config"), check_revoke: Some(url), ..TlsSettings::default() };
        let note = TlsNote { failure: TlsFailure::Revocation, settings: Ok(s) };
        assert_eq!(
            texts(&note),
            [
                "http.sslBackend = schannel (this repository's settings, .git/config)",
                "http.schannelCheckRevoke = true for https://h.example/ (the command line)"
            ]
        );
        let failed = TlsNote { failure: TlsFailure::Revocation, settings: Err(ReadFailed::Said { said: "fatal: bad config".into() }) };
        assert_eq!(texts(&failed), ["The settings couldn't be read: fatal: bad config"]);
    }

    #[test]
    fn keep_caps_and_replaces() {
        let mut logs = LogsState::default();
        let note = |f| TlsNote { failure: f, settings: Err(ReadFailed::Stopped) };
        let ids: Vec<EntryId> = (0..FIX_LIMIT + 3).map(|_| EntryId::next()).collect();
        for id in &ids {
            keep(&mut logs, *id, note(TlsFailure::Certificate));
        }
        assert_eq!(logs.tls.len(), FIX_LIMIT);
        assert!(super::note(&logs, ids[0]).is_none(), "the oldest go first");
        let last = *ids.last().unwrap();
        keep(&mut logs, last, note(TlsFailure::Revocation));
        assert_eq!(logs.tls.len(), FIX_LIMIT);
        assert_eq!(super::note(&logs, last).map(|n| n.failure), Some(TlsFailure::Revocation));
    }
}
