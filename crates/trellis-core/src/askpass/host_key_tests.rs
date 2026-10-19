use super::host_key::{HostKey, parse};

const FIRST: &str = "The authenticity of host 'example.com (192.0.2.10)' can't be established.\n\
                     ED25519 key fingerprint is SHA256:Xq7fV1bQ2b3kq0r1T2c3d4e5f6g7h8i9j0k1l2m3n4o.\n\
                     This key is not known by any other names.\n\
                     Are you sure you want to continue connecting (yes/no/[fingerprint])? ";

#[test]
fn reads_the_first_connection_question() {
    assert_eq!(
        parse(FIRST),
        Some(HostKey {
            host: "example.com".into(),
            port: None,
            address: Some("192.0.2.10".into()),
            key_type: "ED25519".into(),
            fingerprint: "SHA256:Xq7fV1bQ2b3kq0r1T2c3d4e5f6g7h8i9j0k1l2m3n4o".into(),
        })
    );
}

#[test]
fn custom_ports_and_bare_addresses() {
    let text = "The authenticity of host '[git.example.com]:2222 ([192.0.2.7]:2222)' can't be established.\n\
                ECDSA key fingerprint is SHA256:abc.\n";
    let k = parse(text).unwrap();
    assert_eq!((k.host.as_str(), k.port, k.address.as_deref()), ("git.example.com", Some(2222), Some("192.0.2.7")));
    assert_eq!(k.key_type, "ECDSA");

    let text = "The authenticity of host '192.0.2.1 (192.0.2.1)' can't be established.\n\
                ED25519 key fingerprint is SHA256:def.\n";
    let k = parse(text).unwrap();
    assert_eq!((k.host.as_str(), k.port, k.address), ("192.0.2.1", None, None));
}

#[test]
fn older_wording_crlf_and_extra_lines() {
    let text = "The authenticity of host 'old.example.org (198.51.100.4)' can't be established.\r\n\
                RSA key fingerprint is MD5:16:27:ac:a5:76:28:2d:36:63:1b:56:4d:eb:df:a6:48.\r\n\
                +---[ED25519 256]---+\r\n\
                |      .o+*.      |\r\n\
                +----[SHA256]-----+\r\n\
                No matching host key fingerprint found in DNS.\r\n\
                Are you sure you want to continue connecting (yes/no)? ";
    let k = parse(text).unwrap();
    assert_eq!(k.host, "old.example.org");
    assert_eq!(k.address.as_deref(), Some("198.51.100.4"));
    assert_eq!(k.key_type, "RSA");
    assert_eq!(k.fingerprint, "MD5:16:27:ac:a5:76:28:2d:36:63:1b:56:4d:eb:df:a6:48");
}

#[test]
fn other_questions_are_not_read() {
    let no_fingerprint = "The authenticity of host 'example.com (192.0.2.10)' can't be established.\n\
                          Are you sure you want to continue connecting (yes/no)? ";
    assert_eq!(parse(no_fingerprint), None);
    assert_eq!(parse("Allow use of key /home/ada/.ssh/id_ed25519?\nKey fingerprint SHA256:abc."), None);
    assert_eq!(parse("Verification code: "), None);
    let differs = "Warning: the ED25519 host key for 'example.com' differs from the key for the IP address '192.0.2.10'\n\
                   The authenticity of host 'example.com (192.0.2.10)' can't be established.\n\
                   ED25519 key fingerprint is SHA256:abc.\n\
                   Are you sure you want to continue connecting (yes/no)? ";
    assert_eq!(parse(differs), None);
}
