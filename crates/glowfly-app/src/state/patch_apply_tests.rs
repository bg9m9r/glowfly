//! The Apply patch dialog's form and the texts of the apply and git am jobs.

use super::*;
use glowfly_core::patch_ops::{AmOptions, ApplyOptions};

fn summary(commits: usize, files: &[&str]) -> PatchSummary {
    PatchSummary { commits, first_subject: None, files: files.iter().map(|f| (*f).to_owned()).collect(), crlf_content: false }
}

fn form(commits: usize, files: &[&str]) -> ApplyForm {
    ApplyForm::new(PathBuf::from("repo"), PathBuf::from("dir").join("x.patch"), summary(commits, files))
}

fn opts(on_fail: OnFail, index: bool, reverse: bool) -> ApplyOptions {
    ApplyOptions { index, on_fail, reverse }
}

#[test]
fn the_form_picks_its_mode_and_forces_the_index_for_three_way() {
    let f = form(3, &["a", "b"]);
    assert_eq!((f.mode, f.name.as_str(), f.am), (ApplyMode::Commits, "x.patch", AmOptions::default()));
    assert_eq!(f.summary_line().as_deref(), Some("3 commits · 2 files"));
    assert_eq!(f.request().how, ApplyHow::Commits(AmOptions::default()));
    // CRLF lines among LF ones are the files' own: keep their carriage returns.
    let crlf = ApplyForm::new(PathBuf::from("repo"), PathBuf::from("x.patch"), PatchSummary { crlf_content: true, ..summary(1, &["a"]) });
    assert_eq!(crlf.request().how, ApplyHow::Commits(AmOptions { keep_cr: true, ..AmOptions::default() }));
    assert_eq!(f.request().commits, 3);

    let mut f = form(0, &["a"]);
    assert_eq!(f.mode, ApplyMode::Changes);
    assert_eq!(f.summary_line().as_deref(), Some("Changes to 1 file"));
    assert_eq!(f.request().how, ApplyHow::Changes(ApplyOptions::default()));
    f.opts.on_fail = OnFail::ThreeWay;
    assert_eq!(f.request().how, ApplyHow::Changes(opts(OnFail::ThreeWay, true, false)));
    assert_eq!(form(1, &[]).summary_line(), None);
    assert_eq!(form(1, &["a"]).summary_line().as_deref(), Some("1 commit · 1 file"));
}

#[test]
fn changes_texts() {
    assert_eq!(changes_done("x.patch", &opts(OnFail::Refuse, false, false)), "Applied x.patch to the working tree.");
    assert_eq!(changes_done("x.patch", &opts(OnFail::Refuse, true, false)), "Applied x.patch and staged its changes.");
    assert_eq!(changes_done("x.patch", &opts(OnFail::ThreeWay, false, false)), "Applied x.patch and staged its changes.");
    assert_eq!(changes_done("x.patch", &opts(OnFail::Refuse, false, true)), "Took x.patch's changes out of the working tree.");
    assert_eq!(changes_done("x.patch", &opts(OnFail::Reject, true, true)), "Took x.patch's changes out of the working tree and the index.");

    let three =
        "$ git apply --3way x\nApplied patch to 'a.txt' with conflicts.\nApplied patch to 'b.txt' with conflicts.\nU a.txt\nU b.txt";
    assert_eq!(
        changes_failed("x.patch", &opts(OnFail::ThreeWay, true, false), three),
        "Applied x.patch with conflicts in 2 files. Resolve them in the conflict view."
    );
    let rej = "Applying patch a.txt with 2 rejects...\nApplying patch b.txt with 1 reject...\nApplying patch c with 1 reject...\n\
               Applying patch d with 1 reject...\nApplying patch e with 1 reject...";
    assert_eq!(
        changes_failed("x.patch", &opts(OnFail::Reject, false, false), rej),
        "Applied part of x.patch: 6 hunks didn't fit and were saved to a.txt.rej, b.txt.rej, c.rej and 2 more."
    );
    let one = "Applying patch a.txt with 1 reject...";
    assert_eq!(
        changes_failed("x.patch", &opts(OnFail::Reject, false, false), one),
        "Applied part of x.patch: 1 hunk didn't fit and was saved to a.txt.rej."
    );
    let refused = "$ git apply --check --index x\nerror: a.txt: does not match index";
    assert_eq!(
        changes_failed("x.patch", &opts(OnFail::Refuse, true, false), refused),
        "x.patch doesn't apply: a.txt has unstaged changes; stage them or apply to the working tree only. Nothing was changed."
    );
    assert_eq!(
        changes_failed("x.patch", &ApplyOptions::default(), "fatal: oops."),
        "x.patch doesn't apply: fatal: oops. Nothing was changed."
    );
}

#[test]
fn am_texts() {
    assert_eq!(am_done("x.patch", 3), "Applied x.patch as 3 commits.");
    assert_eq!(am_done("x.patch", 1), "Applied x.patch as a commit.");
    let conflict =
        "Applying: Add\nCONFLICT (content): Merge conflict in a.txt\nerror: Failed to merge in the changes.\nPatch failed at 0002 Fix it";
    assert_eq!(
        am_failed(Some("x.patch"), conflict),
        "Applying x.patch stopped at patch 2 (Fix it) with conflicts. Resolve them, then Continue; or Skip patch or Abort."
    );
    assert_eq!(
        am_failed(None, conflict),
        "Applying patches stopped at patch 2 (Fix it) with conflicts. Resolve them, then Continue; or Skip patch or Abort."
    );
    let stopped = "error: patch failed: a.txt:3\nerror: a.txt: patch does not apply\nPatch failed at 0002 Fix it\nhint: ...";
    assert_eq!(
        am_failed(Some("x.patch"), stopped),
        "Applying x.patch stopped at patch 2 (Fix it): a.txt has changed where the hunk at line 3 goes. Make the change by hand and \
         Continue; or Skip patch or Abort."
    );
    assert_eq!(
        am_failed(Some("x.patch"), "Patch format detection failed."),
        "x.patch has no commit messages, so it can't be applied as commits. Apply its changes instead."
    );
    assert_eq!(am_resume_done("Continue"), "Applied the remaining patches.");
    assert_eq!(am_resume_done("Skip patch"), "Applied the remaining patches.");
    assert_eq!(am_resume_done("Abort"), "Stopped applying patches. The branch is back where it was.");
}
