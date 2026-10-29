# Security policy

Glowfly handles repository contents and access tokens for hosting services, so security reports
are taken seriously.

## Reporting a vulnerability

Please report it privately through GitHub: open the repository's **Security** tab and choose
**Report a vulnerability**. Don't open a public issue.

Include what an attacker could do, the steps to reproduce, and the Glowfly version or commit.
You should get a reply within a week. Once a fix is released, the report can be credited to you
if you'd like.

## Code scanning

CodeQL scans the Rust code and the GitHub workflows on every push and pull request to `main`
and weekly (`.github/workflows/codeql.yml`). Test code, which holds made-up tokens and keys, is
left out (`.github/codeql/codeql-config.yml`); every query stays on for the shipped code.

[zizmor](https://docs.zizmor.sh) lints the workflows themselves on every push and pull request
(`.github/workflows/zizmor.yml`) and fails on any finding of medium severity or higher, such as
template injection, credentials left in a checkout or overly broad token permissions.
[OpenSSF Scorecard](https://scorecard.dev/viewer/?uri=github.com/bg9m9r/glowfly) scores the
repository's supply-chain practices weekly and on pushes to `main`
(`.github/workflows/scorecard.yml`); its results go to the Security tab too.

Every action is pinned to a commit SHA, checkouts don't keep the token, each job asks only for
the token permissions it needs, release binaries are built without a cache, and publishing a
release waits for the owner's approval in the `release` environment (`docs/RELEASING.md`).

## Supported versions

Glowfly has no stable release yet; fixes go into the `main` branch.
