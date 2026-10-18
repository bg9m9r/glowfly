//! Trello cards in Trellis: a card with its labels, members and comments; the Create dialog's
//! boards, lists, labels and members; creating a card; and a token that may only read.

use super::issues::new_issue;
use super::*;

const BOARD: &str = "5f1a2b3c4d5e6f7a8b9c0d1e";
const KEY: &str = "0123456789abcdef0123456789abcdef";
const CARD: &str = "GET api.trello.com/1/cards/AbCd1234?fields=id,name,desc,shortLink,closed,idBoard,idList,labels&list=true&list_fields=name\
                    &board=true&board_fields=name&members=true&member_fields=fullName,username";
const ACTIONS: &str = "GET api.trello.com/1/cards/AbCd1234/actions?filter=commentCard,createCard,copyCard&limit=1000&memberCreator=true\
                       &memberCreator_fields=fullName,username";

fn auth() -> Auth {
    Auth::from_token(Provider::Trello, Some(KEY), "tok", "test")
}

fn all_boards() -> Target {
    let account = Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() };
    Target::trello(&account, "", "")
}

fn website() -> Target {
    let account = Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() };
    Target::trello(&account, BOARD, "Website")
}

const CARD_JSON: &str = r#"{"id":"5f1a2b3c4d5e6f7a8b9c0d99","name":"Fix login","desc":"The **form** fails.","shortLink":"AbCd1234",
    "closed":false,"idBoard":"5f1a2b3c4d5e6f7a8b9c0d1e","idList":"l1","list":{"id":"l1","name":"Doing"},"board":{"id":"b","name":"Website"},
    "members":[{"id":"m1","fullName":"Ada","username":"ada"},{"id":"m2","fullName":"","username":"grace"}],
    "labels":[{"id":"x1","name":"Bug","color":"red"},{"id":"x2","name":"","color":"green"}]}"#;

/// Newest first, as Trello sends them.
const ACTIONS_JSON: &str = r#"[
    {"id":"a3","type":"commentCard","date":"2026-01-04T10:00:00.000Z","data":{"text":"Fixed?"},"memberCreator":{"fullName":"Grace","username":"grace"}},
    {"id":"a2","type":"commentCard","date":"2026-01-03T10:00:00.000Z","data":{"text":"Seen it"},"memberCreator":{"fullName":"","username":"ada"}},
    {"id":"a1","type":"createCard","date":"2026-01-02T10:00:00.000Z","data":{},"memberCreator":{"fullName":"Linus","username":"linus"}}]"#;

#[test]
fn details_read_the_card_its_labels_and_comments() {
    let m = Mock::start(|r| match r.route().as_str() {
        CARD => Reply::json(CARD_JSON),
        ACTIONS => Reply::json(ACTIONS_JSON),
        _ => Reply::not_found(),
    });
    let d = issue_details(&website(), &auth(), "AbCd1234").unwrap();
    assert_eq!(routes(&m), [CARD, ACTIONS]);
    assert!(
        m.requests()
            .iter()
            .all(|r| r.header("Authorization") == Some(&format!(r#"OAuth oauth_consumer_key="{KEY}", oauth_token="tok""#)[..]))
    );
    let i = &d.issue;
    assert_eq!((i.key.as_str(), i.title.as_str(), i.state.as_str(), i.kind.as_str()), ("AbCd1234", "Fix login", "Doing", "Website"));
    assert_eq!(i.facets.labels, ["Bug", "green"], "a label without a name shows its color");
    assert_eq!((d.body.as_str(), d.assignees.as_slice()), ("The **form** fails.", &["Ada".to_owned(), "grace".to_owned()][..]));
    assert_eq!((d.author.as_str(), d.created), ("Linus", 1_767_348_000), "from the createCard action");
    let rows: Vec<(&str, &str, &str)> = d.comments.iter().map(|c| (c.author.as_str(), c.body.as_str(), c.url.as_str())).collect();
    assert_eq!(
        rows,
        [("ada", "Seen it", "https://trello.com/c/AbCd1234#comment-a2"), ("Grace", "Fixed?", "https://trello.com/c/AbCd1234#comment-a3")],
        "oldest first"
    );
    drop(m);

    let _m = Mock::start(|r| match r.route().as_str() {
        CARD => Reply::json(CARD_JSON),
        ACTIONS => Reply::json("[]"),
        _ => Reply::not_found(),
    });
    let d = issue_details(&website(), &auth(), "AbCd1234").unwrap();
    assert_eq!((d.author.as_str(), d.created), ("", 0x5f1a2b3c), "created from the card id");
}

#[test]
fn unreadable_and_malformed_cards_are_not_found() {
    let m = Mock::start(|_| Reply::status(401, "unauthorized card permission requested"));
    let e = issue_details(&website(), &auth(), "AbCd1234").unwrap_err();
    assert!(e.is_not_found() && e.to_string().contains("Card AbCd1234 wasn't found"), "{e}");
    let before = routes(&m).len();
    for bad in ["AbCd123", "../x/abc", "AbCd-234"] {
        assert!(issue_details(&website(), &auth(), bad).unwrap_err().is_not_found(), "{bad}");
    }
    assert_eq!(routes(&m).len(), before, "a key that isn't a short link sends nothing");
}

#[test]
fn options_without_a_board_list_the_boards() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.trello.com/1/members/me/boards?filter=open&fields=name" => Reply::json(
            r#"[{"id":"6a6a6a6a6a6a6a6a6a6a6a6a","name":"Ops"},{"id":"5f1a2b3c4d5e6f7a8b9c0d1e","name":"Website"},{"id":"bad","name":"X"}]"#,
        ),
        _ => Reply::not_found(),
    });
    let o = issue_create_options(&all_boards(), &auth(), "").unwrap();
    let boards: Vec<(&str, &str)> = o.boards.iter().map(|b| (b.key.as_str(), b.title.as_str())).collect();
    assert_eq!(boards, [("6a6a6a6a6a6a6a6a6a6a6a6a", "Ops"), (BOARD, "Website")]);
    let url = o.write_token_url.unwrap_or_default();
    assert!(url.contains("scope=read,write") && url.contains(KEY), "{url}");
    assert!(o.types.is_empty());
    let before = routes(&m).len();
    let e = issue_create_options(&all_boards(), &auth(), "not-a-board").unwrap_err();
    assert!(e.to_string().contains("Choose a board."), "{e}");
    assert_eq!(routes(&m).len(), before, "a bad board id sends nothing");
}

fn board_server() -> Mock {
    Mock::start(|r| {
        let route = r.route();
        match route.as_str() {
            "GET api.trello.com/1/boards/5f1a2b3c4d5e6f7a8b9c0d1e/lists?filter=open&fields=name" => {
                Reply::json(r#"[{"id":"l1","name":"To do"},{"id":"l2","name":"Doing"}]"#)
            }
            "GET api.trello.com/1/boards/5f1a2b3c4d5e6f7a8b9c0d1e/labels?fields=name,color&limit=1000" => Reply::json(
                r#"[{"id":"x1","name":"Bug","color":"red"},{"id":"x2","name":"","color":"green"},{"id":"x3","name":"bug","color":"blue"}]"#,
            ),
            "GET api.trello.com/1/boards/5f1a2b3c4d5e6f7a8b9c0d1e/members?fields=fullName,username" => {
                Reply::json(r#"[{"id":"m1","fullName":"Ada","username":"ada"},{"id":"m2","fullName":"","username":"grace"}]"#)
            }
            "POST api.trello.com/1/cards" => Reply::json(r#"{"id":"c1","name":"Fix login","shortLink":"Zz9Yy8Xx"}"#),
            r if r.starts_with("GET api.trello.com/1/cards/Zz9Yy8Xx?") => Reply::json(
                r#"{"name":"Fix login","shortLink":"Zz9Yy8Xx","list":{"name":"To do"},"board":{"name":"Website"},"members":[{"fullName":"Ada"}]}"#,
            ),
            _ => Reply::not_found(),
        }
    })
}

#[test]
fn options_for_a_board_read_its_lists_labels_and_members() {
    let _m = board_server();
    let o = issue_create_options(&website(), &auth(), "").unwrap();
    let lists: Vec<(&str, &str)> = o.types.iter().map(|l| (l.key.as_str(), l.title.as_str())).collect();
    assert_eq!(lists, [("l1", "To do"), ("l2", "Doing")]);
    assert_eq!(o.default_type.as_deref(), Some("l1"));
    assert_eq!(o.labels, ["Bug", "green"], "once each");
    assert_eq!(o.people, [Person { id: "m1".into(), name: "Ada".into() }, Person { id: "m2".into(), name: "grace".into() }]);
    assert_eq!((o.labels_free, o.max_assignees), (false, usize::MAX));
    assert!(o.write_token_url.is_some_and(|u| u.contains("scope=read,write")));
    assert!(o.boards.is_empty());
}

#[test]
fn create_maps_labels_and_reads_the_card_back() {
    let m = board_server();
    let new = NewIssue {
        project: BOARD.into(),
        description: "Steps".into(),
        labels: vec!["BUG".into(), "green".into(), "unknown".into()],
        assignees: vec!["m1".into()],
        kind: Some("l1".into()),
        ..new_issue("Fix login")
    };
    let i = create_issue(&all_boards(), &auth(), &new).unwrap();
    assert_eq!((i.key.as_str(), i.state.as_str(), i.url.as_str()), ("Zz9Yy8Xx", "To do", "https://trello.com/c/Zz9Yy8Xx"));
    let body = find(&m, "POST api.trello.com/1/cards").json();
    assert_eq!(
        body,
        serde_json::json!({ "idList": "l1", "name": "Fix login", "desc": "Steps", "idLabels": ["x1", "x2"], "idMembers": ["m1"] })
    );
    let bare = NewIssue { project: BOARD.into(), kind: Some("l2".into()), ..new_issue("Bare") };
    create_issue(&website(), &auth(), &bare).unwrap();
    let posts: Vec<_> = m.requests().into_iter().filter(|r| r.method == "POST").collect();
    assert_eq!(posts[1].json(), serde_json::json!({ "idList": "l2", "name": "Bare" }), "empty fields are left out");
}

#[test]
fn a_read_only_token_is_told_how_to_write() {
    let _m = Mock::start(|r| match r.route().as_str() {
        "POST api.trello.com/1/cards" => Reply::status(401, "unauthorized permission requested"),
        "GET api.trello.com/1/members/me?fields=fullName,username" => Reply::json(r#"{"fullName":"Ada"}"#),
        _ => Reply::not_found(),
    });
    let new = NewIssue { project: BOARD.into(), kind: Some("l1".into()), ..new_issue("Fix") };
    let e = create_issue(&website(), &auth(), &new).unwrap_err();
    assert!(!e.is_auth() && matches!(&e, Error::Forbidden(m) if m.contains("can read cards but not create them")), "{e}");
    assert!(e.to_string().contains("Preferences > Integrations"), "{e}");
    drop(_m);

    let _m = Mock::start(|_| Reply::status(401, "invalid token"));
    let e = create_issue(&website(), &auth(), &new).unwrap_err();
    assert!(e.is_auth() && e.to_string().contains("expired or was revoked"), "{e}");
}
