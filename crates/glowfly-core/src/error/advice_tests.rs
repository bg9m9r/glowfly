use super::{Ask, service};
use crate::error::Error;

const CREATE: Ask = Ask::CreateRepo { owner: "ada", name: "demo" };

fn say(ask: Ask, e: Error) -> Option<String> {
    service("GitHub", ask, &e.to_string())
}

#[test]
fn a_taken_name_says_to_choose_another() {
    let e = Error::Http {
        status: 422,
        message: "the server said “Repository creation failed. name already exists on this account”".into(),
    };
    assert_eq!(say(CREATE, e).as_deref(), Some("A repository called demo already exists on ada. Choose another name."));
}

#[test]
fn sign_in_and_limits() {
    let auth = say(Ask::Owners, Error::Auth("bad token".into())).unwrap();
    assert_eq!(auth, "GitHub didn't accept your sign-in. Sign in again, or reconnect the account in Preferences > Integrations.");
    let limited = say(CREATE, Error::Forbidden("the server said “API rate limit exceeded for user”".into())).unwrap();
    assert!(limited.starts_with("GitHub is limiting requests"), "{limited}");
    let too_many = say(Ask::Owners, Error::Http { status: 429, message: "slow down".into() }).unwrap();
    assert_eq!(too_many, limited);
}

#[test]
fn refusals_name_the_owner() {
    let denied = say(CREATE, Error::Forbidden("the server said “Must have admin rights”".into())).unwrap();
    assert!(denied.contains("can't create repositories on ada"), "{denied}");
    let denied = say(Ask::Owners, Error::Forbidden("nope".into())).unwrap();
    assert!(denied.contains("didn't allow this"), "{denied}");
    let missing = say(CREATE, Error::NotFound("the server said “Not Found”".into())).unwrap();
    assert!(missing.contains("can't find ada"), "{missing}");
    assert_eq!(say(Ask::Owners, Error::NotFound("x".into())), None, "only a create names an owner");
}

#[test]
fn network_and_server_trouble() {
    let net = say(CREATE, Error::Network("timed out".into())).unwrap();
    assert!(net.contains("couldn't reach GitHub"), "{net}");
    let server = say(CREATE, Error::Http { status: 502, message: "Bad Gateway".into() }).unwrap();
    assert!(server.contains("problem on its side"), "{server}");
}

#[test]
fn other_text_is_left_alone() {
    assert_eq!(say(CREATE, Error::Decode("x".into())), None);
    assert_eq!(say(Ask::Owners, Error::Http { status: 422, message: "Validation failed".into() }), None);
    assert_eq!(service("GitHub", Ask::Owners, "name already exists"), None, "only a create is about a name");
}
