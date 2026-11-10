use super::*;
use crate::state::pull_request::Tips;
use crate::state::pull_request::tests::{oid, pr, view};
use glowfly_core::diff_rows::LineSide;
use glowfly_core::forge::{LineAnchor, Provider};
use glowfly_core::repo::DiffTarget;

fn host(name: &str) -> Target {
    Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: name.into() }
}

fn panel() -> PrView {
    view(pr(None), Tips { target: None, source: None })
}

fn line(text: &str) -> LineDraft {
    LineDraft {
        target: DiffTarget::Between(oid(3), oid(2), "f.txt".into()),
        head: oid(2).to_string(),
        hunk: 0,
        first: 1,
        last: 1,
        anchor: LineAnchor { path: "f.txt".into(), line: 2, side: LineSide::New, start: None },
        lines: Vec::new(),
        text: text.into(),
        posting: None,
        error: None,
    }
}

/// A panel with a comment, a review summary and a reply typed.
fn typed() -> PrView {
    let mut v = panel();
    v.forms.comment = "Looks good".into();
    v.forms.review_body = "Two nits".into();
    v.forms.replies.insert("T1".into(), "Done".into());
    v
}

#[test]
fn only_text_worth_keeping_is_kept() {
    let mut kept = KeptPrDrafts::default();
    let mut v = panel();
    v.forms.comment = "  \n".into();
    v.forms.review_body = "\t".into();
    v.forms.replies.insert("T1".into(), " ".into());
    v.forms.merge_title = "Add search (#7)".into();
    v.line_draft = Some(line(" \n"));
    assert!(!unposted_text(&v.forms, v.line_draft.as_ref()));
    kept.keep(Some(&host("r")), v);
    assert!(kept.is_empty(), "blank text and filled-in merge fields aren't kept");

    let typings: [fn(&mut PrView); 4] = [
        |v| v.forms.comment = "a".into(),
        |v| v.forms.review_body = "a".into(),
        |v| _ = v.forms.replies.insert("T".into(), "a".into()),
        |v| v.line_draft = Some(line("a")),
    ];
    for f in typings {
        let mut v = panel();
        f(&mut v);
        kept.keep(Some(&host("r")), v);
        assert_eq!(kept.len(), 1);
        assert!(kept.any_typed());
        kept.clear();
    }
    kept.keep(None, typed());
    assert!(kept.is_empty(), "nothing is kept without a service");
}

#[test]
fn keeping_a_blank_panel_removes_the_earlier_entry() {
    let mut kept = KeptPrDrafts::default();
    kept.keep(Some(&host("r")), typed());
    assert_eq!(kept.len(), 1);
    kept.keep(Some(&host("r")), panel());
    assert!(kept.is_empty());
}

#[test]
fn restore_gives_everything_back_once() {
    let mut kept = KeptPrDrafts::default();
    let mut v = typed();
    v.line_draft = Some(line("Rename this"));
    v.busy = Some(PrAction::Comment);
    v.tab = PrTab::Files;
    kept.keep(Some(&host("r")), v);

    let mut other = panel();
    kept.restore(&host("fork"), &mut other);
    assert_eq!(other.forms, PrForms::default(), "another service's text isn't given");
    assert_eq!(kept.len(), 1, "and stays");

    let mut back = panel();
    kept.restore(&host("r"), &mut back);
    assert_eq!(back.forms.comment, "Looks good");
    assert_eq!(back.forms.review_body, "Two nits");
    assert_eq!(back.forms.replies.get("T1").map(String::as_str), Some("Done"));
    assert_eq!(back.line_draft.as_ref().map(|d| d.text.as_str()), Some("Rename this"));
    assert_eq!(back.busy, Some(PrAction::Comment));
    assert_eq!(back.tab, PrTab::Files);
    assert!(kept.is_empty());
}

#[test]
fn another_service_drops_the_entries() {
    let mut kept = KeptPrDrafts::default();
    kept.keep(Some(&host("r")), typed());
    kept.retain_host(Some(&host("r")));
    assert_eq!(kept.len(), 1);
    kept.retain_host(Some(&host("fork")));
    assert!(kept.is_empty());
    kept.keep(Some(&host("r")), typed());
    kept.retain_host(None);
    assert!(kept.is_empty());
}

#[test]
fn settle_takes_out_what_was_posted() {
    let r = host("r");
    let mut kept = KeptPrDrafts::default();
    let mut v = panel();
    v.forms.comment = "Looks good".into();
    v.busy = Some(PrAction::Comment);
    kept.keep(Some(&r), v);
    kept.settle(&r, 7, &PrAction::Comment, Ok(()));
    assert!(kept.is_empty(), "nothing else was typed");

    let mut v = typed();
    v.forms.replies.insert("T2".into(), "Also".into());
    v.busy = Some(PrAction::Reply("T1".into()));
    kept.keep(Some(&r), v);
    kept.settle(&r, 7, &PrAction::Reply("T1".into()), Ok(()));
    let (forms, _, busy) = kept.get(7).expect("still typed");
    assert_eq!(forms.replies.keys().collect::<Vec<_>>(), ["T2"]);
    assert_eq!((forms.comment.as_str(), busy), ("Looks good", None));
}

#[test]
fn a_failed_line_comment_keeps_its_text_with_the_reason() {
    let r = host("r");
    let mut kept = KeptPrDrafts::default();
    let mut v = panel();
    v.line_draft = Some(LineDraft { posting: Some(false), ..line("Rename this") });
    v.busy = Some(PrAction::LineComment);
    kept.keep(Some(&r), v);
    kept.settle(&r, 7, &PrAction::LineComment, Err("timed out".into()));
    let (_, d, busy) = kept.get(7).expect("kept");
    let d = d.expect("the line comment");
    assert_eq!((d.text.as_str(), d.posting, d.error.as_deref(), busy), ("Rename this", None, Some("timed out"), None));

    let mut v = panel();
    v.line_draft = Some(LineDraft { posting: Some(false), ..line("Rename this") });
    v.busy = Some(PrAction::LineComment);
    kept.keep(Some(&r), v);
    kept.settle(&r, 7, &PrAction::LineComment, Ok(()));
    assert!(kept.is_empty());
}

#[test]
fn settle_for_another_action_or_service_changes_nothing() {
    let r = host("r");
    let mut kept = KeptPrDrafts::default();
    let mut v = typed();
    v.busy = Some(PrAction::Review);
    kept.keep(Some(&r), v);
    kept.settle(&r, 7, &PrAction::Comment, Ok(()));
    kept.settle(&host("fork"), 7, &PrAction::Review, Ok(()));
    kept.settle(&r, 8, &PrAction::Review, Ok(()));
    let (forms, _, busy) = kept.get(7).unwrap();
    assert_eq!((forms.comment.as_str(), forms.review_body.as_str(), busy), ("Looks good", "Two nits", Some(&PrAction::Review)));

    kept.settle(&r, 7, &PrAction::Review, Err("refused".into()));
    let (forms, _, busy) = kept.get(7).unwrap();
    assert_eq!((forms.review_body.as_str(), busy), ("Two nits", None), "a failure only stops the wait");
}
