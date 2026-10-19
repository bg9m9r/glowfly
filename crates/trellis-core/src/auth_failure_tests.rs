use super::*;

fn remotes(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(n, u)| (n.to_string(), u.to_string())).collect()
}

fn failure(remote: &str, need: AuthNeed) -> FetchFailure {
    FetchFailure { remote: remote.into(), need }
}

#[test]
fn finds_what_a_remote_needs() {
    for https in [
        "fatal: could not read Username for 'https://github.com': terminal prompts disabled",
        "fatal: could not read Password for 'https://ada@github.com': No such device or address",
        "fatal: Authentication failed for 'https://github.com/o/r.git/'",
        "fatal: unable to access 'https://h/r.git/': The requested URL returned error: 401",
        "fatal: Cannot prompt because user interactivity has been disabled.",
    ] {
        assert_eq!(need(https), Some(AuthNeed::Https), "{https}");
    }
    for ssh in [
        "git@github.com: Permission denied (publickey).",
        "ada@host: Permission denied (publickey,password).",
        "ada@host: Permission denied (keyboard-interactive).",
        "Permission denied, please try again.",
        "Received disconnect from 10.0.0.1 port 22:2: Too many authentication failures",
    ] {
        assert_eq!(need(ssh), Some(AuthNeed::Ssh), "{ssh}");
    }
    assert_eq!(need("Host key verification failed.\nfatal: Could not read from remote repository."), Some(AuthNeed::HostKey));
    assert_eq!(need("fatal: unable to access 'https://h/': Could not resolve host: h"), None);
    assert_eq!(need("error: cannot open .git/FETCH_HEAD: Permission denied"), None, "a local file");
    let changed = "@@@@@@@@@@@\n@    WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!     @\n@@@@@@@@@@@\n\
                   Host key verification failed.\nfatal: Could not read from remote repository.";
    assert_eq!(need(changed), None, "a changed host key is never offered as a prompt");
}

#[test]
fn tells_a_changed_host_key() {
    let changed = "@@@@@@@@@@@
@    WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!     @
@@@@@@@@@@@
Host key verification failed.";
    assert!(host_key_changed(changed));
    assert_eq!(need(changed), None);
    assert!(!host_key_changed(
        "Host key verification failed.
fatal: Could not read from remote repository."
    ));
    assert!(!host_key_changed(""));
}

#[test]
fn finds_rejected_credentials() {
    let out = "$ git push\nfatal: Authentication failed for 'https://github.com/o/r.git/'";
    assert_eq!(rejected_urls(out), ["https://github.com/o/r.git/"]);
    assert!(credentials_rejected(out));
    assert!(credentials_rejected("fatal: unable to access 'https://h/': The requested URL returned error: 401"));
    assert!(credentials_rejected("remote: HTTP Basic: Access denied"));
    assert!(rejected_urls("remote: HTTP Basic: Access denied").is_empty());
    assert!(!credentials_rejected("fatal: could not read Username for 'https://h': terminal prompts disabled"));
    assert!(!credentials_rejected(" ! [rejected]        main -> main (non-fast-forward)"));
}

#[test]
fn reads_the_host_of_a_remote_url() {
    assert_eq!(url_host("https://ada@GitHub.com/o/r.git").as_deref(), Some("github.com"));
    assert_eq!(url_host("ssh://git@example.com:2222/o/r.git").as_deref(), Some("example.com:2222"));
    assert_eq!(url_host("git@github.com:o/r.git").as_deref(), Some("github.com"));
    assert_eq!(url_host("host.example:repo.git").as_deref(), Some("host.example"));
    assert_eq!(url_host("/srv/git/r.git"), None);
    assert_eq!(url_host("../r.git"), None);
    assert_eq!(url_host("C:/repos/r.git"), None, "a drive letter");
}

#[test]
fn groups_by_fetching_headers() {
    let r = remotes(&[("origin", "https://github.com/me/r.git"), ("upstream", "git@github.com:them/r.git")]);
    let out = "$ git fetch --all --prune\nFetching origin\nFetching upstream\ngit@github.com: Permission denied (publickey).\n\
               fatal: Could not read from remote repository.\nerror: could not fetch upstream";
    assert_eq!(fetch_failures(out, &r), [failure("upstream", AuthNeed::Ssh)]);
}

#[test]
fn a_single_remote_needs_no_header() {
    let r = remotes(&[("origin", "https://github.com/me/r.git")]);
    let out = "$ git fetch --all\nfatal: could not read Username for 'https://github.com': terminal prompts disabled";
    assert_eq!(fetch_failures(out, &r), [failure("origin", AuthNeed::Https)]);
    assert!(fetch_failures("$ git fetch --all\nfatal: unable to access: Could not resolve host: github.com", &r).is_empty());
}

#[test]
fn header_less_output_is_attributed_by_host_then_by_name() {
    let r = remotes(&[("origin", "https://github.com/me/r.git"), ("backup", "ssh://git@backup.example:2222/r.git")]);
    let out = "fatal: could not read Username for 'https://github.com': terminal prompts disabled";
    assert_eq!(fetch_failures(out, &r), [failure("origin", AuthNeed::Https)]);
    let out = "git@backup.example: Permission denied (publickey).";
    assert_eq!(fetch_failures(out, &r), [failure("backup", AuthNeed::Ssh)], "the port doesn't matter");
    let out = "Host key verification failed.\nfatal: Could not read from remote repository.\n\
               error: could not fetch 'backup' (exit code: 128)";
    assert_eq!(fetch_failures(out, &r), [failure("backup", AuthNeed::HostKey)]);
    let out = "Host key verification failed.";
    assert!(fetch_failures(out, &r).is_empty(), "two remotes and nothing to say which");
}

#[test]
fn failures_are_listed_once_in_remote_order() {
    let r = remotes(&[("origin", "https://github.com/me/r.git"), ("upstream", "https://gitlab.com/them/r.git")]);
    let out = "Fetching origin\nfatal: Authentication failed for 'https://github.com/me/r.git/'\n\
               fatal: Authentication failed for 'https://github.com/me/r.git/'\n\
               Fetching upstream\nfatal: could not read Username for 'https://gitlab.com': terminal prompts disabled";
    assert_eq!(fetch_failures(out, &r), [failure("origin", AuthNeed::Https), failure("upstream", AuthNeed::Https)]);
    let twice = "fatal: Authentication failed for 'https://gitlab.com/them/r.git/'\nremote: HTTP Basic: Access denied\n\
                 fatal: Authentication failed for 'https://gitlab.com/them/r.git/'";
    assert_eq!(fetch_failures(twice, &r), [failure("upstream", AuthNeed::Https)]);
}
