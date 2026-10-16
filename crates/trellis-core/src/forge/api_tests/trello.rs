use super::*;

const BOARD: &str = "5f1a2b3c4d5e6f7a8b9c0d1e";
const ME: &str = "GET api.trello.com/1/members/me?fields=fullName,username";
const BOARDS: &str = "GET api.trello.com/1/members/me/boards?filter=open&fields=name&lists=open&list_fields=name";
const CARDS: &str = "GET api.trello.com/1/members/me/cards?filter=open&fields=name,shortLink,idBoard,idList,closed,dateLastActivity";

fn auth() -> Auth {
    Auth::from_token(Provider::Trello, Some("k3y"), "tok", "test")
}

fn account() -> Account {
    Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() }
}

/// Two open boards (Website's archived list is left out by `lists=open`) and the member's cards.
fn board_server() -> Mock {
    Mock::start(|r| match r.route().as_str() {
        ME => Reply::json(r#"{"id":"m","fullName":"Ada Lovelace","username":"ada"}"#),
        BOARDS => Reply::json(
            r#"[{"id":"5f1a2b3c4d5e6f7a8b9c0d1e","name":"Website","lists":[{"id":"l1","name":"Doing"},{"id":"l2","name":"Review"}]},
                {"id":"6a6a6a6a6a6a6a6a6a6a6a6a","name":"Ops","lists":[{"id":"l3","name":"Inbox"}]}]"#,
        ),
        CARDS => Reply::json(
            r#"[{"name":"Fix login","shortLink":"AbCd1234","idBoard":"5f1a2b3c4d5e6f7a8b9c0d1e","idList":"l1","dateLastActivity":"2026-10-10T09:00:00.000Z"},
                {"name":"Speed up","shortLink":"Zz9Yy8Xx","idBoard":"5f1a2b3c4d5e6f7a8b9c0d1e","idList":"l2","dateLastActivity":"2026-10-12T09:00:00.000Z"},
                {"name":"Rotate keys","shortLink":"Op5Op5Op","idBoard":"6a6a6a6a6a6a6a6a6a6a6a6a","idList":"l3","dateLastActivity":"2026-10-11T09:00:00.000Z"},
                {"name":"In an archived list","shortLink":"Arch1ved","idBoard":"5f1a2b3c4d5e6f7a8b9c0d1e","idList":"l9","dateLastActivity":"2026-10-13T09:00:00.000Z"},
                {"name":"Malformed","shortLink":"bad-link","idBoard":"5f1a2b3c4d5e6f7a8b9c0d1e","idList":"l1","dateLastActivity":"2026-10-14T09:00:00.000Z"},
                {"name":"Closed board","shortLink":"Gone1234","idBoard":"7b7b7b7b7b7b7b7b7b7b7b7b","idList":"l4","dateLastActivity":"2026-10-15T09:00:00.000Z"}]"#,
        ),
        _ => Reply::not_found(),
    })
}

#[test]
fn verify_sends_key_and_token_in_an_oauth_header() {
    let m = Mock::start(|r| match r.route().as_str() {
        ME => Reply::json(r#"{"fullName":"Ada Lovelace","username":"ada"}"#),
        _ => Reply::not_found(),
    });
    assert_eq!(verify(&account(), &auth()).unwrap(), "Ada Lovelace");
    let sent = find(&m, ME);
    assert_eq!(sent.header("Authorization"), Some(r#"OAuth oauth_consumer_key="k3y", oauth_token="tok""#));
    assert!(!sent.path.contains("key=") && !sent.path.contains("token="), "{}", sent.path);
    drop(m);
    let _m = Mock::start(|_| Reply::json(r#"{"fullName":"","username":"ada"}"#));
    assert_eq!(verify(&account(), &auth()).unwrap(), "ada");
}

#[test]
fn assigned_cards_on_the_chosen_board() {
    let m = board_server();
    let cards = my_issues(&Target::trello(&account(), BOARD, "Website"), &auth()).unwrap();
    let rows: Vec<_> =
        cards.iter().map(|i| (i.key.as_str(), i.state.as_str(), i.kind.as_str(), i.assignee.as_deref(), i.url.as_str())).collect();
    assert_eq!(
        rows,
        [
            ("Zz9Yy8Xx", "Review", "Website", Some("Ada Lovelace"), "https://trello.com/c/Zz9Yy8Xx"),
            ("AbCd1234", "Doing", "Website", Some("Ada Lovelace"), "https://trello.com/c/AbCd1234"),
        ]
    );
    assert_eq!(cards[1].title, "Fix login");
    for route in [ME, BOARDS, CARDS] {
        find(&m, route);
    }
    assert_eq!(routes(&m).len(), 3);
}

#[test]
fn all_boards_when_no_board_is_chosen() {
    let _m = board_server();
    let cards = my_issues(&Target::trello(&account(), "", ""), &auth()).unwrap();
    let rows: Vec<_> = cards.iter().map(|i| (i.key.as_str(), i.kind.as_str())).collect();
    assert_eq!(rows, [("Zz9Yy8Xx", "Website"), ("Op5Op5Op", "Ops"), ("AbCd1234", "Website")]);
}

#[test]
fn cards_are_looked_up_by_short_link() {
    let m = Mock::start(|r| {
        let (path, _) = r.path.split_once('?').unwrap_or((r.path.as_str(), ""));
        match path {
            "/1/cards/AbCd1234" => Reply::json(
                r#"{"name":"Fix login","shortLink":"AbCd1234","closed":false,"list":{"name":"Doing"},"board":{"name":"Website"},
                    "members":[{"fullName":"Ada","username":"ada"},{"fullName":"Grace","username":"grace"}]}"#,
            ),
            "/1/cards/0ld0Card" => Reply::json(
                r#"{"name":"Done long ago","shortLink":"0ld0Card","closed":true,"list":{"name":"Done"},"board":{"name":"Ops"}}"#,
            ),
            "/1/cards/Bad00001" => Reply::status(400, "invalid id"),
            _ => Reply::not_found(),
        }
    });
    let keys = ["AbCd1234", "Gone0001", "Bad00001", "not-a-link", "0ld0Card", "AbCd1234"].map(String::from);
    let found = issues(&Target::trello(&account(), "", ""), &auth(), &keys).unwrap();
    let rows: Vec<_> = found.iter().map(|i| (i.key.as_str(), i.state.as_str(), i.kind.as_str(), i.assignee.as_deref())).collect();
    assert_eq!(rows, [("AbCd1234", "Doing", "Website", Some("Ada, Grace")), ("0ld0Card", "Archived", "Ops", None)]);
    assert_eq!(found[0].url, "https://trello.com/c/AbCd1234");
    let sent = find(&m, "GET api.trello.com/1/cards/AbCd1234?");
    for part in ["list=true", "board=true", "members=true", "fields=name,shortLink,closed,idBoard,idList"] {
        assert!(sent.path.contains(part), "{}", sent.path);
    }
    assert!(!routes(&m).iter().any(|r| r.contains("not-a-link")));
    assert_eq!(routes(&m).len(), 4, "each id once: {:?}", routes(&m));
    drop(m);
    let e = error_for(429, r#"{"error":"API_TOKEN_LIMIT_EXCEEDED","message":"Rate limit exceeded"}"#, || {
        issues(&Target::trello(&account(), "", ""), &auth(), &keys).map(drop)
    });
    assert!(!e.is_auth() && e.to_string().contains("rate limit"), "{e}");
}

#[test]
fn cards_the_member_cannot_see_dont_hide_the_others() {
    let m = Mock::start(|r| {
        let (path, _) = r.path.split_once('?').unwrap_or((r.path.as_str(), ""));
        match path {
            "/1/cards/AbCd1234" => {
                Reply::json(r#"{"name":"Fix login","shortLink":"AbCd1234","list":{"name":"Doing"},"board":{"name":"Website"}}"#)
            }
            "/1/cards/Pr1vate9" => Reply::status(401, "unauthorized card permission requested"),
            "/1/cards/F0rbidd3" => Reply::status(403, "forbidden"),
            _ => Reply::not_found(),
        }
    });
    let t = Target::trello(&account(), "", "");
    let keys = ["Pr1vate9", "F0rbidd3", "AbCd1234"].map(String::from);
    let found = issues(&t, &auth(), &keys).unwrap();
    assert_eq!(found.iter().map(|i| i.key.as_str()).collect::<Vec<_>>(), ["AbCd1234"]);
    drop(m);
    // A token Trello no longer accepts still fails the lookup, in words that say so.
    let e = error_for(401, "invalid token", || issues(&t, &auth(), &keys).map(drop));
    assert!(e.is_auth() && e.to_string().contains("expired or was revoked"), "{e}");
}

#[test]
fn boards_for_the_picker() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.trello.com/1/members/me/boards?filter=open&fields=name" => Reply::json(
            r#"[{"id":"5f1a2b3c4d5e6f7a8b9c0d1e","name":"website"},{"id":"not-an-id","name":"Bad"},
                {"id":"6a6a6a6a6a6a6a6a6a6a6a6a","name":"Apps"},{"id":"7b7b7b7b7b7b7b7b7b7b7b7b","name":"Ops"}]"#,
        ),
        _ => Reply::not_found(),
    });
    let boards = trello_boards(&account(), &auth()).unwrap();
    let names: Vec<_> = boards.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["Apps", "Ops", "website"]);
    assert_eq!(boards[2].id, BOARD);
    find(&m, "GET api.trello.com/1/members/me/boards?filter=open&fields=name");
}

#[test]
fn rate_limits_and_expired_tokens_get_clear_errors() {
    let t = Target::trello(&account(), "", "");
    let mine = || my_issues(&t, &auth()).map(drop);
    let e = error_for(429, r#"{"error":"API_TOKEN_LIMIT_EXCEEDED","message":"Rate limit exceeded"}"#, mine);
    assert!(!e.is_auth() && e.to_string().contains("rate limit"), "{e}");
    let e = error_for(401, "invalid token", mine);
    assert!(e.is_auth() && e.to_string().contains("expired or was revoked"), "{e}");
    let e = error_for(401, "invalid key", || verify(&account(), &auth()).map(drop));
    assert!(e.is_auth() && e.to_string().contains("API key"), "{e}");
}
