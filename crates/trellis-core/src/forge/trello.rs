//! Trello, as an issue tracker: the open cards assigned to the signed-in member, optionally on
//! one board, and lookups of cards linked from commits and branch names.

use super::page;
use super::provider::{DEFAULTS, ProviderInfo};
use super::{Account, Auth, Issue, NewPullRequest, PullRequest, RefStyle, Target, enc, get};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde::de::DeserializeOwned;

pub(super) static TRELLO: ProviderInfo = ProviderInfo {
    name: "Trello",
    fixed_host: Some("trello.com"),
    username_label: Some("API Key"),
    host_hint: "",
    token_help: "Your Trello API key and a token allowed for it (read access is enough).",
    ref_style: RefStyle::Trello,
    tracker_only: true,
    service: &Trello,
    ..DEFAULTS
};

const API: &str = "https://api.trello.com/1";
const WEB: &str = "https://trello.com";
const ME: &str = "members/me?fields=fullName,username";

/// A card's short id, as in `trello.com/c/<shortLink>`: exactly 8 ASCII letters or digits.
pub(super) fn is_short_link(s: &str) -> bool {
    s.len() == 8 && s.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// A board id: 24 hex digits.
fn is_board_id(s: &str) -> bool {
    s.len() == 24 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

pub(super) fn card_url(key: &str) -> String {
    format!("{WEB}/c/{}", enc(key))
}

/// The page where Trello shows a new read-only token for `key`, for the user to paste; None
/// until the key looks like one (32 ASCII letters or digits).
pub fn authorize_url(key: &str) -> Option<String> {
    let key = key.trim();
    (key.len() == 32 && key.bytes().all(|b| b.is_ascii_alphanumeric()))
        .then(|| format!("{WEB}/1/authorize?expiration=never&scope=read&response_type=token&name=Trellis&key={key}"))
}

impl Target {
    /// A Trello account's cards; an empty `board_id` means every open board.
    pub fn trello(account: &Account, board_id: &str, board_name: &str) -> Target {
        Target {
            provider: account.provider,
            host: account.host.clone(),
            owner: board_id.to_owned(),
            project: board_name.to_owned(),
            repo: String::new(),
        }
    }
}

/// An open board, for the Board picker.
#[derive(Clone, Debug, PartialEq)]
pub struct TrelloBoard {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Member {
    full_name: String,
    username: String,
}

impl Member {
    fn display(&self) -> String {
        if self.full_name.trim().is_empty() { self.username.clone() } else { self.full_name.clone() }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Named {
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ApiCard {
    name: String,
    short_link: String,
    id_board: String,
    id_list: String,
    closed: bool,
    date_last_activity: String,
    list: Option<Named>,
    board: Option<Named>,
    members: Vec<Member>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ApiBoard {
    id: String,
    name: String,
    lists: Vec<ApiList>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ApiList {
    id: String,
    name: String,
    closed: bool,
}

fn call<T: DeserializeOwned>(auth: &Auth, url: &str) -> Result<T> {
    get(auth, url).map_err(trello_error)
}

/// Trello's answers to a bad key, an expired or revoked token and its rate limit, in words
/// that say what to do.
pub(super) fn trello_error(e: Error) -> Error {
    match e {
        Error::Auth(m) if m.to_ascii_lowercase().contains("key") => {
            Error::Auth("the Trello API key isn't valid; copy it again from your Power-Up admin page".into())
        }
        Error::Auth(_) => Error::Auth("the Trello token has expired or was revoked; allow a new token and connect again".into()),
        Error::Http { status: 429, .. } => {
            Error::Http { status: 429, message: "Trello's API rate limit was exceeded; wait a few seconds, then refresh".into() }
        }
        e => e,
    }
}

/// A card lookup that failed for that card alone: an unknown or malformed id, or a card on a
/// board the member can't see (Trello answers 401 "unauthorized card permission requested",
/// unlike a bad key or token).
fn unreadable_card(e: &Error) -> bool {
    match e {
        Error::Auth(m) => m.to_ascii_lowercase().contains("permission"),
        Error::Forbidden(_) | Error::Http { status: 400, .. } => true,
        e => e.is_not_found(),
    }
}

/// A card looked up by its short id; `key` is the spelling asked for, so caches match.
fn looked_up(key: &str, c: ApiCard) -> Issue {
    let names: Vec<String> = c.members.iter().map(Member::display).filter(|n| !n.is_empty()).collect();
    Issue {
        key: key.to_owned(),
        title: c.name,
        state: if c.closed { "Archived".into() } else { c.list.map(|l| l.name).unwrap_or_default() },
        kind: c.board.map(|b| b.name).unwrap_or_default(),
        assignee: (!names.is_empty()).then(|| names.join(", ")),
        url: card_url(key),
        facets: Default::default(),
    }
}

/// The open boards of `account`, sorted by name, for the Board picker.
pub fn trello_boards(_account: &Account, auth: &Auth) -> Result<Vec<TrelloBoard>> {
    let boards: Vec<ApiBoard> = call(auth, &format!("{API}/members/me/boards?filter=open&fields=name"))?;
    let mut out: Vec<TrelloBoard> =
        boards.into_iter().filter(|b| is_board_id(&b.id)).map(|b| TrelloBoard { id: b.id, name: b.name }).collect();
    out.sort_by_key(|b| b.name.to_lowercase());
    out.truncate(page::MAX_ITEMS);
    Ok(out)
}

/// The Trello [`Forge`](super::Forge): an issue tracker only. `Target::owner` is the board id
/// (empty: every open board) and `Target::project` its name.
pub struct Trello;

impl super::Forge for Trello {
    fn verify(&self, _account: &Account, auth: &Auth) -> Result<String> {
        Ok(call::<Member>(auth, &format!("{API}/{ME}"))?.display())
    }

    fn token_url(&self, _account: &Account) -> Option<String> {
        Some(format!("{WEB}/power-ups/admin"))
    }

    fn issue_url(&self, _t: &Target, key: &str) -> String {
        card_url(key)
    }

    fn display(&self, t: &Target) -> String {
        match (t.owner.is_empty(), t.project.is_empty()) {
            (true, _) => "All boards".into(),
            (false, true) => t.owner.clone(),
            (false, false) => t.project.clone(),
        }
    }

    fn create_pull_request(&self, _t: &Target, _auth: &Auth, _pr: &NewPullRequest) -> Result<PullRequest> {
        Err(Error::Unsupported("Trello does not host pull requests".into()))
    }

    /// Reads the member's cards directly (not through search, whose index lags), keeping those
    /// in open lists on open boards.
    fn my_issues(&self, t: &Target, auth: &Auth) -> Result<Vec<Issue>> {
        let me: Member = call(auth, &format!("{API}/{ME}"))?;
        let boards: Vec<ApiBoard> = call(auth, &format!("{API}/members/me/boards?filter=open&fields=name&lists=open&list_fields=name"))?;
        let mut cards: Vec<ApiCard> =
            call(auth, &format!("{API}/members/me/cards?filter=open&fields=name,shortLink,idBoard,idList,closed,dateLastActivity"))?;
        cards.retain(|c| !c.closed && is_short_link(&c.short_link) && (t.owner.is_empty() || c.id_board == t.owner));
        cards.sort_by(|a, b| b.date_last_activity.cmp(&a.date_last_activity));
        let assignee = me.display();
        let mut out = Vec::new();
        for c in cards {
            let Some(board) = boards.iter().find(|b| b.id == c.id_board) else { continue };
            let Some(list) = board.lists.iter().find(|l| l.id == c.id_list && !l.closed) else { continue };
            out.push(Issue {
                url: card_url(&c.short_link),
                key: c.short_link,
                title: c.name,
                state: list.name.clone(),
                kind: board.name.clone(),
                assignee: Some(assignee.clone()),
                facets: Default::default(),
            });
            if out.len() >= page::MAX_ITEMS {
                break;
            }
        }
        Ok(out)
    }

    /// Looks cards up one by one by short id; unknown and malformed ids, and cards the member
    /// can't see, are skipped.
    fn issues(&self, _t: &Target, auth: &Auth, keys: &[String]) -> Result<Vec<Issue>> {
        let mut out: Vec<Issue> = Vec::new();
        let fields = "fields=name,shortLink,closed,idBoard,idList&list=true&list_fields=name&board=true&board_fields=name\
                      &members=true&member_fields=fullName,username";
        for k in keys.iter().filter(|k| is_short_link(k)).take(30) {
            if out.iter().any(|i| i.key == *k) {
                continue;
            }
            match get::<ApiCard>(auth, &format!("{API}/cards/{}?{fields}", enc(k))) {
                Ok(c) => out.push(looked_up(k, c)),
                Err(e) if unreadable_card(&e) => {}
                Err(e) => return Err(trello_error(e)),
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::Provider;

    const BOARD: &str = "5f1a2b3c4d5e6f7a8b9c0d1e";

    fn account() -> Account {
        Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() }
    }

    #[test]
    fn trello_errors_say_what_to_do() {
        let key = trello_error(Error::Auth("the server said \u{201c}invalid key\u{201d}".into()));
        assert!(key.is_auth() && key.to_string().contains("API key isn't valid"), "{key}");
        for m in ["the server said \u{201c}invalid token\u{201d}", "expired token"] {
            let e = trello_error(Error::Auth(m.into()));
            assert!(e.is_auth() && e.to_string().contains("expired or was revoked"), "{e}");
        }
        let e = trello_error(Error::Http { status: 429, message: "x".into() });
        assert!(!e.is_auth() && e.to_string().contains("rate limit was exceeded"), "{e}");
        assert!(trello_error(Error::NotFound("gone".into())).is_not_found());
    }

    #[test]
    fn authorize_url_needs_a_whole_key() {
        let key = "0123456789abcdef0123456789abcdef";
        let url = authorize_url(&format!(" {key} ")).unwrap();
        for part in ["scope=read", "expiration=never", "response_type=token", key] {
            assert!(url.contains(part), "{url}");
        }
        assert!(url.starts_with("https://trello.com/1/authorize?"));
        for bad in [&key[1..], "0123456789abcdef 123456789abcdef", "0123456789abcdef\"123456789abcdef"] {
            assert_eq!(authorize_url(bad), None, "{bad}");
        }
    }

    #[test]
    fn converts_a_looked_up_card() {
        let c: ApiCard = serde_json::from_str(
            r#"{"id":"x","name":"Fix login","shortLink":"AbCd1234","closed":false,"idBoard":"b","idList":"l",
            "list":{"id":"l","name":"Doing"},"board":{"id":"b","name":"Website"},
            "members":[{"fullName":"Ada","username":"ada"},{"fullName":"","username":"grace"}]}"#,
        )
        .unwrap();
        let i = looked_up("AbCd1234", c);
        assert_eq!((i.key.as_str(), i.title.as_str(), i.state.as_str(), i.kind.as_str()), ("AbCd1234", "Fix login", "Doing", "Website"));
        assert_eq!((i.assignee.as_deref(), i.url.as_str()), (Some("Ada, grace"), "https://trello.com/c/AbCd1234"));
    }

    #[test]
    fn targets_name_their_board() {
        assert_eq!(Target::trello(&account(), BOARD, "Website").display(), "Website");
        assert_eq!(Target::trello(&account(), "", "").display(), "All boards");
        assert_eq!(Target::trello(&account(), "", "").issue_url("AbCd1234"), "https://trello.com/c/AbCd1234");
        assert!(is_board_id(BOARD) && !is_board_id("xyz"));
        assert!(is_short_link("AbCd1234") && !is_short_link("AbCd123") && !is_short_link("AbCd-234"));
    }
}
