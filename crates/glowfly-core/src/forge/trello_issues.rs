//! Trello cards in the issue panel and the Create dialog: one card with its description,
//! labels, members and comments; the member's open boards, and a board's lists, labels and
//! members to choose from; creating a card; and posting a comment. A token allowed only to read
//! is told how to get one that can also write.

use super::filters::Named;
use super::issue_details::{CreateOptions, IssueDetails, NewIssue, Person, api_time, not_found};
use super::trello::{API, ApiCard, ApiList, ME, Member, Trello, authorize_url, call, card_url, is_board_id, is_short_link, looked_up};
use super::trello::{open_boards, trello_error, unreadable_card};
use super::{Auth, Entry, EntryKind, Forge, Issue, Target, enc, get, post};
use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::{Map, Value, json};

/// Why a token that may only read can't `verb` ("create them"), and what to do.
fn read_only(verb: &str) -> String {
    format!(
        "this Trello token can read cards but not {verb} (or you only observe this board). Allow a token that can also write, then \
         paste it in Preferences > Integrations."
    )
}

/// A refusal to write: a token that can still read may only read; else Trello's own words.
fn refused(auth: &Auth, e: Error, verb: &str) -> Error {
    match get::<Member>(auth, &format!("{API}/{ME}")) {
        Ok(_) => Error::Forbidden(read_only(verb)),
        Err(_) => trello_error(e),
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Label {
    id: String,
    name: String,
    color: Option<String>,
}

/// How a label is shown and chosen: its name, else its color.
fn label_shown(l: &Label) -> String {
    let name = l.name.trim();
    if !name.is_empty() {
        return name.to_owned();
    }
    l.color.as_deref().map(str::trim).filter(|c| !c.is_empty()).unwrap_or("(no name)").to_owned()
}

/// A card as its own page reads it.
#[derive(Deserialize)]
struct Full {
    #[serde(flatten)]
    base: ApiCard,
    #[serde(default)]
    id: String,
    #[serde(default)]
    desc: String,
    #[serde(default)]
    labels: Vec<Label>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Action {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    date: String,
    data: ActionData,
    member_creator: Option<Member>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ActionData {
    text: String,
}

fn creator(a: &Action) -> String {
    a.member_creator.as_ref().map(Member::display).unwrap_or_default()
}

/// A comment action on card `key` as an entry, linked to its place on the card.
fn action_entry(key: &str, a: &Action) -> Entry {
    Entry {
        author: creator(a),
        time: api_time(&a.date),
        kind: EntryKind::Comment,
        body: a.data.text.clone(),
        url: format!("{}#comment-{}", card_url(key), enc(&a.id)),
    }
}

/// The time encoded in a Trello object id: its first 8 hex digits are Unix seconds.
fn id_time(id: &str) -> i64 {
    id.get(..8).and_then(|h| i64::from_str_radix(h, 16).ok()).unwrap_or(0)
}

/// Card `key` with its description, labels, members and comments, oldest first; who created
/// it comes from its create (or copy) action, else only when from its id.
pub(super) fn details(t: &Target, auth: &Auth, key: &str) -> Result<IssueDetails> {
    if !is_short_link(key) {
        return Err(not_found(t, key));
    }
    let fields = "fields=id,name,desc,shortLink,closed,idBoard,idList,labels&list=true&list_fields=name&board=true&board_fields=name\
                  &members=true&member_fields=fullName,username";
    let full: Full = match get(auth, &format!("{API}/cards/{}?{fields}", enc(key))) {
        Ok(c) => c,
        Err(e) if unreadable_card(&e) => return Err(not_found(t, key)),
        Err(e) => return Err(trello_error(e)),
    };
    let assignees: Vec<String> = full.base.members.iter().map(Member::display).filter(|n| !n.is_empty()).collect();
    let mut issue = looked_up(key, full.base);
    issue.facets.labels = full.labels.iter().map(label_shown).collect();
    let url = format!(
        "{API}/cards/{}/actions?filter=commentCard,createCard,copyCard&limit=1000&memberCreator=true\
         &memberCreator_fields=fullName,username",
        enc(key)
    );
    let mut actions: Vec<Action> = call(auth, &url)?;
    actions.reverse();
    let created = actions.iter().find(|a| a.kind == "createCard" || a.kind == "copyCard");
    let (author, time) = match created {
        Some(a) => (creator(a), api_time(&a.date)),
        None => (String::new(), id_time(&full.id)),
    };
    let comments = actions.iter().filter(|a| a.kind == "commentCard").map(|a| action_entry(key, a)).collect();
    Ok(IssueDetails { issue, body: full.desc, author, created: time, assignees, priority: None, epic: None, comments })
}

/// The board's labels as shown, once each.
fn board_labels(auth: &Auth, board: &str) -> Result<Vec<Label>> {
    call(auth, &format!("{API}/boards/{board}/labels?fields=name,color&limit=1000"))
}

/// The member's open boards (no board chosen), or a board's open lists (the first chosen),
/// labels and members; with Trello's page for a token that can also write.
pub(super) fn options(t: &Target, auth: &Auth, project: &str) -> Result<CreateOptions> {
    let board = if project.trim().is_empty() { t.owner.trim() } else { project.trim() };
    let write_token_url = auth.trello_key().and_then(|k| authorize_url(&k, true));
    if board.is_empty() {
        let boards = open_boards(auth)?.into_iter().map(|b| Named { key: b.id, title: b.name }).collect();
        return Ok(CreateOptions { boards, write_token_url, ..CreateOptions::default() });
    }
    if !is_board_id(board) {
        return Err(Error::Other("Choose a board.".into()));
    }
    let lists: Vec<ApiList> = call(auth, &format!("{API}/boards/{board}/lists?filter=open&fields=name"))?;
    let types: Vec<Named> = lists.into_iter().map(|l| Named { key: l.id, title: l.name }).collect();
    let mut labels: Vec<String> = Vec::new();
    for l in board_labels(auth, board)?.iter().map(label_shown) {
        if !labels.iter().any(|x| x.eq_ignore_ascii_case(&l)) {
            labels.push(l);
        }
    }
    let members: Vec<Member> = call(auth, &format!("{API}/boards/{board}/members?fields=fullName,username"))?;
    let people = members.iter().filter(|m| !m.id.is_empty()).map(|m| Person { id: m.id.clone(), name: m.display() }).collect();
    Ok(CreateOptions {
        labels,
        people,
        default_type: types.first().map(|l| l.key.clone()),
        types,
        labels_free: false,
        max_assignees: usize::MAX,
        write_token_url,
        ..CreateOptions::default()
    })
}

/// The JSON body of a new card: an empty description and empty lists are left out.
fn body(new: &NewIssue, labels: &[String]) -> Value {
    let mut m = Map::new();
    m.insert("idList".into(), json!(new.kind.as_deref().unwrap_or_default()));
    m.insert("name".into(), json!(new.title));
    if !new.description.is_empty() {
        m.insert("desc".into(), json!(new.description));
    }
    if !labels.is_empty() {
        m.insert("idLabels".into(), json!(labels));
    }
    if !new.assignees.is_empty() {
        m.insert("idMembers".into(), json!(new.assignees));
    }
    Value::Object(m)
}

/// Creates the card in its list, then reads it back as listed. A refusal while the token can
/// still read means it may only read.
pub(super) fn create(t: &Target, auth: &Auth, new: &NewIssue) -> Result<Issue> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Created {
        #[serde(default)]
        short_link: String,
        #[serde(default)]
        name: String,
    }
    let board = if new.project.trim().is_empty() { t.owner.trim() } else { new.project.trim() };
    let mut ids: Vec<String> = Vec::new();
    if !new.labels.is_empty() {
        if !is_board_id(board) {
            return Err(Error::Other("Choose a board.".into()));
        }
        let known = board_labels(auth, board)?;
        for chosen in &new.labels {
            if let Some(l) = known.iter().find(|l| label_shown(l).eq_ignore_ascii_case(chosen.trim()))
                && !ids.contains(&l.id)
            {
                ids.push(l.id.clone());
            }
        }
    }
    let created: Created = match post(auth, &format!("{API}/cards"), body(new, &ids)) {
        Ok(c) => c,
        Err(e @ (Error::Auth(_) | Error::Forbidden(_))) => return Err(refused(auth, e, "create them")),
        Err(e) => return Err(trello_error(e)),
    };
    if !is_short_link(&created.short_link) {
        return Err(Error::Other("Trello created the card but sent no link to it; refresh to see it".into()));
    }
    let read = Trello.issues(t, auth, std::slice::from_ref(&created.short_link)).unwrap_or_default();
    Ok(read.into_iter().next().unwrap_or_else(|| Issue {
        url: card_url(&created.short_link),
        key: created.short_link,
        title: if created.name.is_empty() { new.title.clone() } else { created.name },
        state: String::new(),
        kind: String::new(),
        assignee: None,
        facets: Default::default(),
    }))
}

/// Posts `body` as a comment on card `key`. A refusal while the token can still read means it
/// may only read.
pub(super) fn post_comment(t: &Target, auth: &Auth, key: &str, body: &str) -> Result<Entry> {
    if !is_short_link(key) {
        return Err(not_found(t, key));
    }
    let a: Action = match post(auth, &format!("{API}/cards/{}/actions/comments", enc(key)), json!({ "text": body })) {
        Ok(a) => a,
        Err(e @ (Error::Auth(_) | Error::Forbidden(_))) => return Err(refused(auth, e, "comment on them")),
        Err(e) if unreadable_card(&e) => return Err(not_found(t, key)),
        Err(e) => return Err(trello_error(e)),
    };
    Ok(action_entry(key, &a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_ids_read_as_shown() {
        let l = |name: &str, color: Option<&str>| Label { id: String::new(), name: name.into(), color: color.map(str::to_owned) };
        assert_eq!(label_shown(&l(" Bug ", Some("red"))), "Bug");
        assert_eq!(label_shown(&l("", Some("green"))), "green");
        assert_eq!(label_shown(&l("", None)), "(no name)");
        assert_eq!(id_time("5f1a2b3c4d5e6f7a8b9c0d1e"), 0x5f1a2b3c);
        assert_eq!((id_time("xyz"), id_time("")), (0, 0));
    }
}
