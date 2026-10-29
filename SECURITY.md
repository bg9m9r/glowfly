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

## Known advisories

Every dependency is checked against published advisories: cargo-deny checks the RustSec database
on every push and pull request (`deny.toml`, `.github/workflows/ci.yml`), and Scorecard's
osv-scanner checks `Cargo.lock` against OSV. An advisory that affects Glowfly is fixed by
updating the dependency. One that can't be fixed that way is listed here only when it doesn't
reach Glowfly, with the reason, until an update removes it.

The exceptions are kept in two files, which must be changed together: `deny.toml`'s
`[advisories] ignore` (with a comment on each explaining it) and `osv-scanner.toml`, with the
same advisories and the same reasons. The table below lists those advisories too, and a test
fails when the three differ. The osv-scanner entries lapse after a few months, so each exception
is reviewed again.

| Advisory | Crate | Why it doesn't reach Glowfly |
| --- | --- | --- |
| RUSTSEC-2026-0183 | git2 0.20 | `Remote::list()` is not called (networking goes through the git CLI) |
| RUSTSEC-2026-0184 | git2 0.20 | `Blame::blame_buffer()` is not called |
| RUSTSEC-2026-0194, RUSTSEC-2026-0195 | quick-xml 0.30 | Only parses the D-Bus interface XML bundled with atspi (zbus-lockstep, Linux accessibility), never outside input |
| RUSTSEC-2025-0141 | bincode 1 (unmaintained) | syntect's packed syntax and theme dumps, built into the program |
| RUSTSEC-2024-0320 | yaml-rust (unmaintained) | syntect's syntax loading, used by two-face at build time only |
| RUSTSEC-2024-0436 | paste (unmaintained) | A proc-macro of accesskit_windows (eframe 0.31); runs only at compile time |
| RUSTSEC-2026-0192 | ttf-parser (unmaintained) | ab_glyph, epaint's font rasterizer (egui 0.31); no flaw is known, only that it is unmaintained |
| RUSTSEC-2026-0370, RUSTSEC-2026-0371, RUSTSEC-2026-0372 | zvariant, zbus_names, zvariant_derive (merged into zbus 6) | Still required by zbus 4 and 5 (accesskit_unix, rfd, notify-rust) |
| GHSA-qwgh-2vcv-g2f7 | block-buffer 0.10 | Linux only, through digest 0.10 (keyring's dbus-secret-service via hkdf, hmac and sha2; zbus via sha1). The flaw needs a hash compress callback that panics, and RustCrypto's hashes never do; 0.10 also keeps its cursor in a field of its own, outside the block the advisory corrupts |

## Supported versions

Glowfly has no stable release yet; fixes go into the `main` branch.
