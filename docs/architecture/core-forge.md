# glowfly-core: hosting services (`crates/glowfly-core/src/forge/`)

Part of [ARCHITECTURE.md](../ARCHITECTURE.md). One line per file or topic: when you add or move a
source file, update its line here.

## The model

- Hosting services sit behind the `Forge` trait (GitHub, GitLab, Bitbucket Cloud and Data Center, Azure DevOps, Jira, Trello), each described once in a `ProviderInfo`.
- `model.rs`: the one common model they map to: pull requests with their reviewers and CI checks, issues.
- The UI never matches on provider-specific types (Rule 6).

## Pull requests and checks

- `checks.rs`, `*_checks.rs`: CI checks on pull requests and commits.
- `check_detail.rs`, `github_check_list.rs`, `azure_check_list.rs`, `gitlab_checks.rs`, `bitbucket_checks.rs`: each check on a pull request's head commit, for the pull request panel.
- `details.rs`: per-PR checks and votes read after the list.
- `review/`: pull request conversations, review threads, reviews and merging, routing in `route.rs`: GitHub's in `github.rs` and `github_threads.rs`, GitLab merge requests' notes, approvals and merging in `gitlab.rs`, Bitbucket Cloud pull requests' comments and votes in `bitbucket.rs`, Data Center's in `bitbucket_dc.rs`, and both services' merging in `bitbucket_merge.rs`.
- `pr_heads.rs`: each service's head on a fork.
- `head_repo.rs`: the repository a listed pull request's source is on (`PullRequest::head_repo`: the same one, a fork or a deleted fork, from each service's list, a GitLab fork's path read by its project id, a nameless entry as an unknown fork) and whether a remote's repository is it; fixtures for every service in `api_tests/fork_pr_list.rs`.
- `repo_size.rs`: the hosting service's reported repository size (see Partial clones in [core-ops.md](core-ops.md)).

## Issues

- `refs.rs`: issue key detection in text and branch names (a year-like leading number, `2024-cleanup`, names an issue only when the repository turns it on).
- `issue_details.rs`, `issue_comments.rs`: issue details, creating issues and posting comments.
- `github_issues.rs`, `gitlab_issues.rs`, `bitbucket_issues.rs`, `trello_issues.rs`: each service's issues.
- `jira_issues.rs`, with `jira_adf.rs` for Jira's rich text.
- `azure_issues.rs`, with `azure_work_items.rs` for Azure's work item lookups and queries and `html_text.rs` for Azure's HTML as plain text.

## Repositories and accounts

- `web.rs`: repository web pages.
- `forks/`: forks.
- `user_repos/`: the signed-in user's own repositories.
- `ssh_keys.rs`: SSH public keys on the user's account.
- `releases.rs`: Glowfly's own newest release and its public files (unauthenticated, https only).

## Filters and the dashboard

- `filters.rs`, `filters/`: PR and issue filters: conditions, the signed-in user with their teams or groups, matching loaded pull requests, issue searches per service.
- `dashboard.rs`: the dashboard's reading of local repositories' remotes with libgit2 and its bounded calls to several services at once.
