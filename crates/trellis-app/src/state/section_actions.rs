//! What each left-panel section header's + does: its name, why it is disabled (the sentence its
//! hover shows) and the message it sends, which opens the same dialog as the header's menu and
//! the palette. Pure; shared by the sidebar and the update tests.

use super::{AppState, ForgeState, issue_form, push_pr, toolbar_tips};
use crate::msg::{BranchMsg, ForgeMsg, IssueMsg, Msg, StashMsg};
use crate::remote::RemoteMsg;
use crate::submodule::SubmoduleMsg;
use crate::tag::TagMsg;
use crate::worktree::WorktreeMsg;
use trellis_core::repo::Snapshot;
use trellis_core::stash_ops::StashScope;
use trellis_core::worktree_ops::WorktreeBase;

/// Why New tag is off: HEAD has no commit yet.
pub const NO_COMMIT_TAG: &str = "There is no commit to tag yet.";
/// Why Add worktree is off: HEAD has no commit yet.
pub const NO_COMMIT_WORKTREE: &str = "Make a first commit before adding a worktree.";

/// One create action: "New branch", disabled while `blocked` is set.
pub struct SectionAction {
    /// The action without an ellipsis: the +'s accessible name and hover text.
    pub name: String,
    /// Why it can't run, shown on hover.
    pub blocked: Option<String>,
    make: Box<dyn Fn() -> Msg>,
}

impl SectionAction {
    fn new(name: impl Into<String>, blocked: Option<String>, make: impl Fn() -> Msg + 'static) -> Self {
        Self { name: name.into(), blocked, make: Box::new(make) }
    }

    /// The message it sends (to open its dialog).
    pub fn msg(&self) -> Msg {
        (self.make)()
    }

    /// Its menu item's label: the name with an ellipsis, as it opens a dialog.
    pub fn label(&self) -> String {
        format!("{}…", self.name)
    }
}

/// Every header's actions, worked out once a frame.
pub struct SectionActions {
    pub local: SectionAction,
    pub remote: SectionAction,
    /// None without a code host (there is no PULL REQUESTS section then).
    pub pr: Option<SectionAction>,
    /// None without an issue tracker (no ISSUES section).
    pub issue: Option<SectionAction>,
    pub tag: SectionAction,
    pub annotated_tag: SectionAction,
    pub stash: SectionAction,
    pub stash_staged: SectionAction,
    pub worktree: SectionAction,
    pub submodule: SectionAction,
}

pub fn all(st: &AppState) -> SectionActions {
    let fs = st.forge.as_ref();
    SectionActions {
        local: new_branch(st),
        remote: add_remote(),
        pr: fs.and_then(|f| create_pr(st, f)),
        issue: fs.and_then(|f| create_issue(st, f)),
        tag: new_tag(st, false),
        annotated_tag: new_tag(st, true),
        stash: stash(st, StashScope::All),
        stash_staged: stash(st, StashScope::Staged),
        worktree: add_worktree(st),
        submodule: add_submodule(),
    }
}

/// A new branch from HEAD.
pub fn new_branch(st: &AppState) -> SectionAction {
    SectionAction::new("New branch", toolbar_tips::branch_blocked(st), || {
        Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() })
    })
}

pub fn add_remote() -> SectionAction {
    SectionAction::new("Add remote", None, || Msg::Remote(RemoteMsg::AddDialog))
}

/// A pull request from the checked-out branch, as the toolbar's PR button; None without a code
/// host.
pub fn create_pr(st: &AppState, fs: &ForgeState) -> Option<SectionAction> {
    fs.host.as_ref()?;
    let noun = fs.pr_noun();
    let head = st.snap.as_ref().and_then(|s| s.head_branch.clone());
    let open = || {
        let (snap, b) = (st.snap.as_ref()?, head.as_ref()?);
        let pr = push_pr::open_pr(snap, fs, b)?;
        Some(format!("{b} already has {noun} {}.", fs.pr_label(pr.id)))
    };
    let blocked = toolbar_tips::pr_blocked(st).or_else(open);
    let source = head.unwrap_or_default();
    Some(SectionAction::new(format!("Create {noun}"), blocked, move || {
        Msg::Forge(ForgeMsg::CreatePrDialog { source: source.clone(), target: None })
    }))
}

/// A new issue on the tracker; None without one.
pub fn create_issue(st: &AppState, fs: &ForgeState) -> Option<SectionAction> {
    fs.tracker.as_ref()?;
    Some(SectionAction::new(format!("Create {}", fs.issue_noun()), issue_form::create_blocked(st), || Msg::Issue(IssueMsg::CreateDialog)))
}

/// A tag (`annotated` or lightweight) on HEAD's commit.
pub fn new_tag(st: &AppState, annotated: bool) -> SectionAction {
    let at = st.snap.as_ref().and_then(|s| s.head_oid);
    let blocked = at.is_none().then(|| NO_COMMIT_TAG.to_owned());
    let at = at.map(|o| o.to_string()).unwrap_or_default();
    let name = if annotated { "New annotated tag" } else { "New tag" };
    SectionAction::new(name, blocked, move || Msg::Tag(TagMsg::NewTag { at: at.clone(), annotated }))
}

/// The Stash dialog for `scope`. Only a running job or a clean tree disable it here; the dialog
/// checks the rest (nothing staged, say).
pub fn stash(st: &AppState, scope: StashScope) -> SectionAction {
    let name = if scope == StashScope::Staged { "Stash staged changes" } else { "Stash changes" };
    SectionAction::new(name, toolbar_tips::stash_blocked(st), move || Msg::Stash(StashMsg::PushDialog { scope, path: None }))
}

/// The Create worktree dialog from HEAD: its branch (asking for a new branch name) or, detached,
/// its commit.
pub fn add_worktree(st: &AppState) -> SectionAction {
    let base = st.snap.as_ref().and_then(worktree_base);
    let blocked = base.is_none().then(|| NO_COMMIT_WORKTREE.to_owned());
    let base = base.unwrap_or(WorktreeBase::Branch(String::new()));
    SectionAction::new("Add worktree", blocked, move || Msg::Worktree(WorktreeMsg::CreateDialog(base.clone())))
}

pub fn add_submodule() -> SectionAction {
    SectionAction::new("Add submodule", None, || Msg::Submodule(SubmoduleMsg::AddDialog))
}

/// What a worktree added from the header starts from: HEAD's branch, else HEAD's commit; None
/// while HEAD has no commit.
pub fn worktree_base(snap: &Snapshot) -> Option<WorktreeBase> {
    let head = snap.head_oid?;
    Some(match &snap.head_branch {
        Some(b) => WorktreeBase::Branch(b.clone()),
        None => WorktreeBase::Commit(head),
    })
}
