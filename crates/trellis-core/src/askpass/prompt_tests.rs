use super::prompt::{PromptKind, classify, host, user_and_host};

#[test]
fn https_username_prompts_keep_the_url() {
    assert_eq!(classify("Username for 'https://github.com': ", None), PromptKind::Username { url: "https://github.com".into() });
    // credential.useHttpPath puts the repository path in the prompt.
    assert_eq!(
        classify("Username for 'https://host.example/org/repo.git': ", None),
        PromptKind::Username { url: "https://host.example/org/repo.git".into() }
    );
    assert_eq!(host(&classify("Username for 'https://GitHub.com:8443/o/r': ", None)).as_deref(), Some("github.com:8443"));
}

#[test]
fn https_password_prompts_decode_the_user() {
    let kind = classify("Password for 'https://ada%40corp.com@dev.azure.com': ", None);
    assert_eq!(kind, PromptKind::Password { target: "https://ada%40corp.com@dev.azure.com".into(), user: Some("ada@corp.com".into()) });
    assert_eq!(host(&kind).as_deref(), Some("dev.azure.com"));
    assert_eq!(user_and_host("https://ada@github.com/o/r.git"), (Some("ada".into()), Some("github.com".into())));
    assert_eq!(user_and_host("https://github.com"), (None, Some("github.com".into())));
}

#[test]
fn ssh_passphrases_and_passwords() {
    assert_eq!(
        classify("Enter passphrase for key '/home/ada/.ssh/id_ed25519': ", None),
        PromptKind::Passphrase { key: "/home/ada/.ssh/id_ed25519".into() }
    );
    assert_eq!(
        classify("Enter passphrase for C:/Users/ada/.ssh/id_rsa:", None),
        PromptKind::Passphrase { key: "C:/Users/ada/.ssh/id_rsa".into() }
    );
    let kind = classify("git@example.com's password: ", None);
    assert_eq!(kind, PromptKind::Password { target: "git@example.com".into(), user: Some("git".into()) });
    assert_eq!(host(&kind).as_deref(), Some("example.com"));
}

#[test]
fn confirmations_and_other_prompts() {
    let host_key = "The authenticity of host 'example.com (192.0.2.1)' can't be established.\n\
                    ED25519 key fingerprint is SHA256:abc.\n\
                    Are you sure you want to continue connecting (yes/no/[fingerprint])? ";
    assert_eq!(classify(host_key, None), PromptKind::Confirm);
    assert_eq!(classify("Allow use of key?", Some("confirm")), PromptKind::Confirm);
    assert_eq!(classify("Verification code: ", None), PromptKind::Other { secret: false });
    assert_eq!(classify("Enter PIN for authenticator: ", None), PromptKind::Other { secret: true });
    assert_eq!(host(&PromptKind::Confirm), None);
}

#[test]
fn only_structured_requests_are_signing_prompts() {
    let ask = super::GpgAsk { key: "ABCDEF0123456789".into(), user: None, retry: false };
    let p = super::Prompt::gpg(ask);
    assert_eq!(p.text, "Passphrase for OpenPGP key ABCDEF0123456789");
    assert_eq!(p.kind, PromptKind::GpgPassphrase { key: "ABCDEF0123456789".into(), user: None, retry: false });
    assert_eq!(host(&p.kind), None);
    // The same text from git or ssh is just a secret question.
    assert_eq!(classify(&p.text, None), PromptKind::Other { secret: true });
}
