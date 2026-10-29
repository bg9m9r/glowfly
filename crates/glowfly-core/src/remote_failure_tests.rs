use super::{RemoteFailure, read};

const TAIL: &str =
    "fatal: Could not read from remote repository.\n\nPlease make sure you have the correct access rights\nand the repository exists.";

fn unreachable(host: &str) -> Option<RemoteFailure> {
    Some(RemoteFailure::Unreachable { host: Some(host.into()) })
}

#[test]
fn unreachable_hosts_are_named() {
    let curl = "fatal: unable to access 'https://git.example.com/a.git/': Could not resolve host: git.example.com";
    assert_eq!(read(curl), unreachable("git.example.com"));
    let ssh = format!("ssh: Could not resolve hostname gitlab.internal: Name or service not known\n{TAIL}");
    assert_eq!(read(&ssh), unreachable("gitlab.internal"));
    let refused = "fatal: unable to access 'https://example.com:8443/r.git/': Failed to connect to example.com port 8443 after 2 ms: Connection refused";
    assert_eq!(read(refused), unreachable("example.com"));
    let timed_out = format!("ssh: connect to host github.com port 22: Connection timed out\n{TAIL}");
    assert_eq!(read(&timed_out), unreachable("github.com"));
}

#[test]
fn an_unnamed_failure_takes_the_host_from_the_url() {
    let out =
        "fatal: unable to access 'https://***@h.example:8443/r.git/': Operation timed out after 300000 milliseconds with 0 bytes received";
    assert_eq!(read(out), unreachable("h.example"));
    assert_eq!(read("Couldn't connect to server"), Some(RemoteFailure::Unreachable { host: None }));
}

#[test]
fn missing_repositories() {
    let github = "remote: Repository not found.\nfatal: repository 'https://github.com/ada/x.git/' not found";
    assert_eq!(read(github), Some(RemoteFailure::NotFound));
    assert_eq!(read("fatal: repository 'https://github.com/ada/x.git/' not found"), Some(RemoteFailure::NotFound));
    let gitlab = format!(
        "remote: \nremote: The project you were looking for could not be found or you don't have permission to view it.\nremote: \n{TAIL}"
    );
    assert_eq!(read(&gitlab), Some(RemoteFailure::NotFound));
    let local = format!("fatal: '/srv/no-such.git' does not appear to be a git repository\n{TAIL}");
    assert_eq!(read(&local), Some(RemoteFailure::NotFound));
    let http = "fatal: unable to access 'https://h.example/r.git/': The requested URL returned error: 404";
    assert_eq!(read(http), Some(RemoteFailure::NotFound));
}

#[test]
fn refused_access() {
    let out = "remote: Permission to acme/app.git denied to bob.\nfatal: unable to access 'https://github.com/acme/app.git/': The requested URL returned error: 403";
    assert_eq!(read(out), Some(RemoteFailure::NoAccess));
    assert_eq!(read("remote: Permission to acme/app.git denied to bob."), Some(RemoteFailure::NoAccess));
}

#[test]
fn only_the_bare_line_is_unreadable() {
    assert_eq!(read(TAIL), Some(RemoteFailure::Unreadable));
}

#[test]
fn other_failures_say_nothing() {
    assert_eq!(read("fatal: offline"), None);
    assert_eq!(read(""), None);
    let rejected =
        " ! [rejected]        main -> main (non-fast-forward)\nerror: failed to push some refs to 'https://github.com/ada/x.git'";
    assert_eq!(read(rejected), None);
}

#[test]
fn certificates_are_not_unreachable_hosts() {
    let corp =
        "fatal: unable to access 'https://git.corp.example:8443/app.git/': SSL certificate problem: unable to get local issuer certificate";
    assert_eq!(read(corp), Some(RemoteFailure::Certificate { host: Some("git.corp.example".into()) }));
    assert_eq!(read("x509: certificate signed by unknown authority"), Some(RemoteFailure::Certificate { host: None }));
    assert_eq!(
        read("fatal: unable to access 'https://h/': Couldn't connect to server"),
        Some(RemoteFailure::Unreachable { host: Some("h".into()) })
    );
}
