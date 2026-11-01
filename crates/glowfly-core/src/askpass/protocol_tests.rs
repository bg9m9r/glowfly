use super::Secret;
use super::protocol::*;

#[test]
fn requests_and_responses_round_trip_on_one_line() {
    let prompt = "Host key for \"example.com\"\nfingerprint: ✓ é\nContinue (yes/no)? ";
    let line = encode_request("abc", prompt, Some("confirm"));
    assert_eq!(line.matches('\n').count(), 1, "one line: {line}");
    assert!(line.ends_with('\n'));
    let req = decode_request(&line).unwrap();
    assert_eq!((req.token.as_str(), req.prompt.as_str(), req.hint.as_deref()), ("abc", prompt, Some("confirm")));

    for r in [Response::Answer("pa\"ss\nwörd".into()), Response::Cancelled, Response::TimedOut, Response::Rejected] {
        let line = encode_response(&r);
        assert_eq!(line.matches('\n').count(), 1);
        assert!(decode_response(&line).unwrap() == r);
    }
}

#[test]
fn signing_requests_round_trip_and_old_requests_still_decode() {
    let ask = GpgAsk { key: "ABCDEF0123456789".into(), user: Some("Ada Lovelace <ada@example.com>".into()), retry: true, extend: false };
    let line = encode_gpg_request("abc", &ask);
    assert_eq!(line.matches('\n').count(), 1);
    let req = decode_request(&line).unwrap();
    assert_eq!(req.gpg.as_ref(), Some(&ask));
    assert_eq!(req.prompt, "Passphrase for OpenPGP key ABCDEF0123456789");
    assert_eq!(req.hint, None);

    let old = r#"{"v":1,"token":"a","prompt":"Password for 'https://x': ","hint":null}"#;
    let req = decode_request(old).unwrap();
    assert!(req.gpg.is_none());
    // A signing request from a helper older than `extend` is the wrapper's, as before.
    let old_gpg = r#"{"v":1,"token":"a","prompt":"p","hint":null,"gpg":{"key":"K","user":null,"retry":false}}"#;
    assert_eq!(decode_request(old_gpg).unwrap().gpg.map(|g| g.extend), Some(false));
    assert!(!encode_request("a", "p", None).contains("gpg"), "askpass requests leave the field out");

    let line = encode_response(&Response::UseSystem);
    assert!(decode_response(&line).unwrap() == Response::UseSystem);
}

#[test]
fn decoding_rejects_bad_input() {
    let v2 = r#"{"v":2,"token":"a","prompt":"p","hint":null}"#;
    assert!(matches!(decode_request(v2), Err(ProtocolError::Version(2))));
    assert!(matches!(decode_request("not json\n"), Err(ProtocolError::Json(_))));
    let long = encode_request("a", &"x".repeat(MAX_LINE as usize), None);
    assert!(matches!(decode_request(&long), Err(ProtocolError::TooLong)));
}

#[test]
fn tokens() {
    assert!(token_matches("abcd", "abcd"));
    assert!(!token_matches("abcd", "abce"));
    assert!(!token_matches("abcd", "abc"));
    let (a, b) = (new_token().unwrap(), new_token().unwrap());
    assert_eq!(a.len(), 32);
    assert!(a.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_ne!(a, b);
}

#[test]
fn secrets_are_redacted() {
    let s = Secret::new("hunter2");
    assert!(!format!("{s:?}").contains("hunter2"));
    assert!(!format!("{:?}", super::Reply::Answer(s.clone())).contains("hunter2"));
    assert_eq!(s.expose(), "hunter2");
    let mut buf = String::from("hunter2");
    super::wipe(&mut buf);
    assert!(buf.is_empty());
}

#[test]
fn ssh_signing_requests_round_trip() {
    let ask = SshSignAsk { key: "~/.ssh/id_ed25519".into() };
    let line = encode_ssh_sign_request("abc", &ask);
    assert_eq!(line.matches('\n').count(), 1);
    let req = decode_request(&line).unwrap();
    assert_eq!(req.ssh_sign.as_ref(), Some(&ask));
    assert!(req.gpg.is_none());
    assert_eq!(req.prompt, "Passphrase for SSH signing key ~/.ssh/id_ed25519");

    assert!(!encode_request("a", "p", None).contains("ssh_sign"), "plain requests leave the field out");
    let old = r#"{"v":1,"token":"a","prompt":"Enter passphrase: ","hint":null}"#;
    assert!(decode_request(old).unwrap().ssh_sign.is_none());
}
