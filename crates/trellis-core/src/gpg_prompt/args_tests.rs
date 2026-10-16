use super::args::*;
use std::ffi::OsString;

fn os(a: &[&str]) -> Vec<OsString> {
    a.iter().map(OsString::from).collect()
}

#[test]
fn reads_gits_sign_call_and_its_key() {
    let call = parse(&os(&["--status-fd=2", "-bsau", "ABCDEF0123456789"])).unwrap();
    assert_eq!(call.key.as_deref(), Some("ABCDEF0123456789"));
    assert_eq!(call.original, os(&["--status-fd=2", "-bsau", "ABCDEF0123456789"]));
    for a in [
        &["--status-fd=2", "-bsa", "-u", "ABCDEF0123456789"][..],
        &["--status-fd=2", "--detach-sign", "--local-user", "ABCDEF0123456789"],
        &["--status-fd=2", "--sign", "--local-user=ABCDEF0123456789"],
    ] {
        assert_eq!(parse(&os(a)).and_then(|c| c.key).as_deref(), Some("ABCDEF0123456789"), "{a:?}");
    }
    // Without user.signingkey git passes the committer.
    let call = parse(&os(&["--status-fd=2", "-bsau", "Ada <ada@example.com>"])).unwrap();
    assert_eq!(call.key.as_deref(), Some("Ada <ada@example.com>"));
}

#[test]
fn other_calls_are_not_sign_calls() {
    assert_eq!(parse(&os(&["--keyid-format=long", "--status-fd=1", "--verify", "f", "-"])), None);
    assert_eq!(parse(&os(&["--status-fd=2", "--verify", "f", "-"])), None);
    assert_eq!(parse(&os(&["-bsau", "K"])), None, "no status fd: not git's call");
    assert_eq!(parse(&os(&["--status-fd=2", "--list-keys"])), None);
    assert_eq!(parse(&os(&[])), None);
}

#[test]
fn probe_and_loopback_put_their_options_first() {
    let call = parse(&os(&["--status-fd=2", "-bsau", "K"])).unwrap();
    assert_eq!(probe(&call), os(&["--pinentry-mode", "error", "--status-fd=2", "-bsau", "K"]));
    assert_eq!(loopback(&call), os(&["--batch", "--pinentry-mode", "loopback", "--passphrase-fd", "0", "--status-fd=2", "-bsau", "K"]));
    assert_eq!(list_key("K"), os(&["--batch", "--list-secret-keys", "--with-colons", "--keyid-format=long", "--", "K"]));
}
