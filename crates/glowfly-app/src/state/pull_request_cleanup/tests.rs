//! The clean-up job's commands and its headline when the local part stops.

use super::*;

fn job(steps: Vec<Option<LocalStep>>) -> CleanupJob {
    let restore = strs(&["config", "branch.feature.merge", "refs/heads/feature"]);
    CleanupJob { label: "x".into(), branch: "feature".into(), remote: "origin".into(), target: "main".into(), steps, restore }
}

fn strs(cmd: &[&str]) -> Vec<String> {
    cmd.iter().map(|s| s.to_string()).collect()
}

#[test]
fn commands_check_the_target_first_and_delete_with_dash_d() {
    let d = LocalDelete { branch: "feature".into(), upstream: "feature".into(), checkout: None };
    let cmds: Vec<_> = local_delete_cmds(&d, "origin", "main");
    assert_eq!(
        cmds,
        [
            (LocalStep::Check, strs(&["merge-base", "--is-ancestor", "refs/heads/feature", "refs/remotes/origin/main"])),
            (LocalStep::Delete, strs(&["config", "branch.feature.merge", "refs/heads/main"])),
            (LocalStep::Delete, strs(&["branch", "-d", "feature"])),
        ]
    );
    assert_eq!(restore_upstream(&d), strs(&["config", "branch.feature.merge", "refs/heads/feature"]), "the upstream it had");
    let d = LocalDelete { branch: "feature".into(), upstream: "feature".into(), checkout: Some(strs(&["checkout", "main", "--"])) };
    let steps: Vec<_> = local_delete_cmds(&d, "origin", "main").into_iter().map(|(s, _)| s).collect();
    assert_eq!(steps, [LocalStep::Check, LocalStep::Checkout, LocalStep::Delete, LocalStep::Delete], "the checkout comes after the check");
}

#[test]
fn headline_names_the_local_step_that_stopped() {
    let t = Lang::En;
    let steps = vec![None, Some(LocalStep::Check), Some(LocalStep::Checkout), Some(LocalStep::Delete), Some(LocalStep::Delete)];
    let j = job(steps);
    let fetch = "$ git fetch --prune origin\n";
    assert_eq!(j.stopped(t, "$ git fetch --prune origin\nfatal: could not read"), None, "the fetch is git's to explain");
    let check = format!("{fetch}$ git merge-base --is-ancestor refs/heads/feature refs/remotes/origin/main\n");
    assert_eq!(j.stopped(t, &check).as_deref(), Some("Kept local branch feature: it has commits that aren't on origin/main."));
    let checkout = format!(
        "{check}$ git checkout main --\nerror: Your local changes would be overwritten\nAborting\n$ git config branch.feature.merge refs/heads/feature"
    );
    assert_eq!(
        j.stopped(t, &checkout).as_deref(),
        Some("Kept local branch feature: checking out main failed. error: Your local changes would be overwritten")
    );
    // The upstream's restore, which the runner adds after a failure, isn't a step.
    let restore = "\n$ git config branch.feature.merge refs/heads/feature";
    let delete = format!(
        "{check}$ git checkout main --\n$ git config branch.feature.merge refs/heads/main\n$ git branch -d feature\n\
         error: the branch 'feature' is not fully merged\nhint: If you are sure you want to delete it, run 'git branch -D feature'{restore}"
    );
    assert_eq!(
        j.stopped(t, &delete).as_deref(),
        Some("Kept local branch feature: git didn't delete it. error: the branch 'feature' is not fully merged")
    );
}
