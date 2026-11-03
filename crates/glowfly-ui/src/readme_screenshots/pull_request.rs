//! The README's pull request screenshot: the demo's moon-phase pull request on Files changed,
//! with a new comment on a line of src/moon.rs, in Graphite and Porcelain.
//! Everything the hosting service would send is made up here; nothing goes online.

use super::demo::{self, ORIGIN};
use super::shots::state;
use super::{harness, shoot};
use crate::state::pull_request::{Page, PrTab, PrView};
use crate::state::pull_request_checks::ChecksPane;
use crate::state::pull_request_line_comment::draft_at;
use crate::state::{ForgeState, Status};
use eframe::egui;
use glowfly_core::diff_rows::{LineSide, line_index};
use glowfly_core::forge::{
    CheckList, Entry, EntryKind, MergeMethod, MergeState, PrDetail, PrPage, PrState, Provider, PullRequest, ReviewStyle, Reviewer, Target,
    Verdict,
};
use glowfly_core::repo::{self, PrCompare};
use std::sync::Arc;

/// The window this screenshot draws: wide enough for the file list, the diff and the comment box.
const WINDOW: egui::Vec2 = egui::vec2(1600.0, 940.0);

/// Unix seconds on day `day` of January 2026, at 10:00 UTC.
fn on(day: i64) -> i64 {
    1_767_225_600 + (day - 1) * 86_400 + 36_000
}

/// The two open pull requests: #14 from moon-phase (approved once) and #13, a draft.
fn pull_requests(head: String) -> Vec<PullRequest> {
    let pr = |id: u64, title: &str, source: &str, author: &str, draft: bool| PullRequest {
        id,
        title: title.into(),
        description: String::new(),
        source: source.into(),
        target: "main".into(),
        author: author.into(),
        draft,
        reviewers: Vec::new(),
        head: None,
        head_repo: Default::default(),
        merge_status: None,
        checks: None,
        url: format!("https://github.com/tidewatch-example/tidewatch/pull/{id}"),
        facets: Default::default(),
    };
    let mut moon = pr(14, "Add moon phases", "moon-phase", "jonah", false);
    moon.description =
        "Shows the moon's phase beside the next high water.\n\nThe phase is worked out locally; the tides API doesn't send it.".into();
    moon.reviewers = vec![
        Reviewer { name: "mira".into(), state: "approved".into(), required: true },
        Reviewer { name: "tomas".into(), state: "pending".into(), required: false },
    ];
    moon.head = Some(head);
    moon.merge_status = Some("mergeable".into());
    vec![moon, pr(13, "Metric units flag", "metric-units", "tomas", true)]
}

/// What the service says about #14: open, mergeable, one comment and an approval.
fn page(head: String) -> PrPage {
    let detail = PrDetail {
        state: PrState::Open,
        draft: false,
        locked: false,
        mergeable: Some(true),
        merge_state: MergeState::Clean,
        head_sha: head,
        base_sha: "b1".into(),
        same_repo: true,
        author: "jonah".into(),
        me: "priya".into(),
        can_merge: Some(true),
        methods: vec![MergeMethod::Merge, MergeMethod::Squash],
        deletes_merged_branches: false,
        pending_review: None,
        style: ReviewStyle::Verdicts,
    };
    let entries = vec![Entry {
        author: "mira".into(),
        time: on(28),
        kind: EntryKind::Review(Verdict::Approved),
        body: "Nice touch.".into(),
        url: String::new(),
    }];
    PrPage { detail, entries, threads: Vec::new(), threads_note: None }
}

/// Drags the top of the comments pane under the diff as high as it goes (inside the harness),
/// so the comment box has room around it.
fn raise_comments(h: &mut super::H) {
    use egui::{Event, Modifiers, PointerButton, pos2};
    let pane = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("pr-file-threads")).expect("the comments pane").rect;
    let from = pos2(pane.center().x, pane.top());
    let to = pos2(from.x, 0.0);
    let button = |pos, pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    for event in
        [Event::PointerMoved(from), button(from, true), Event::PointerMoved(to), button(to, false), Event::PointerMoved(pos2(300.0, 700.0))]
    {
        h.input_mut().events.push(event);
        h.run();
    }
}

#[test]
fn readme_screenshots_pull_request() {
    for dark in [true, false] {
        let name = format!("pull-request-{}", if dark { "dark" } else { "light" });
        let d = demo::build(&name);
        let mut st = state(&d, dark);
        let snap = st.snap.as_ref().unwrap();
        let tip = snap.locals.iter().find(|b| b.name == "moon-phase").and_then(|b| b.oid).unwrap();
        let target = Target {
            provider: Provider::GitHub,
            host: "github.com".into(),
            owner: "tidewatch-example".into(),
            project: String::new(),
            repo: "tidewatch".into(),
        };
        assert!(ORIGIN.contains("tidewatch-example/tidewatch"));
        let mut fs = ForgeState::new(Some(target), "origin".into(), None);
        fs.host_status = Status::Ok;
        fs.prs = pull_requests(tip.to_string());
        let mut v = PrView::new(fs.prs[0].clone(), &fs, snap);
        v.checks.pane = ChecksPane::Loaded(CheckList::empty(tip.to_string()));
        let key = v.files_key();
        match repo::pr_compare(&d.dir, key.target.expect("origin/main"), key.branch, key.head.as_deref()).unwrap() {
            PrCompare::Loaded { source, changes, commits } => v.set_loaded(changes, source, commits),
            other => panic!("not loaded: {other:?}"),
        }
        let file = v.diff_target("src/moon.rs").expect("moon.rs is in the pull request");
        let mut diff = repo::load_diff(&d.dir, &file).unwrap();
        diff.highlight = glowfly_core::highlight::highlight_diff(file.path(), &diff, None);
        diff.intraline = glowfly_core::intraline::intraline(&diff);
        let line_of =
            |text: &str| diff.hunks.iter().flat_map(|h| &h.lines).find(|l| l.text.contains(text)).and_then(|l| l.new).expect("the line");
        let named = line_of("pub fn phase(");
        crate::state::pull_request_actions::prefill(&mut v.forms, &page(tip.to_string()).detail, None);
        v.page = Page::Loaded(Box::new(page(tip.to_string())));
        v.tab = PrTab::Files;
        let (hunk, i) = line_index(&diff, LineSide::New, named).expect("the phase line");
        let mut draft = draft_at(&v, &file, &diff, hunk, i, i).unwrap();
        draft.text = "Could we name this `phase_fraction`?".into();
        v.line_draft = Some(draft);
        st.session.forge = Some(fs);
        st.session.pull_request.view = Some(v);
        st.session.diff.view = Some((file, Arc::new(diff)));
        st.view.sections.prs = true;
        let mut h = harness(st, dark, WINDOW);
        h.run();
        raise_comments(&mut h);
        // Up to the commit panel on the right.
        let panel = egui::Id::new(crate::ui::panel_size::DETAILS_ID);
        let right = egui::containers::panel::PanelState::load(&h.ctx, panel).expect("the commit panel").rect.min.x;
        shoot(&mut h, &name, Some(egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(right, WINDOW.y))));
        drop(h);
        d.remove();
    }
}
