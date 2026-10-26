//! The pull request Overview with Markdown in the description and comments: a release notes
//! style description (headings, scoped items with commit links, a nested list, a quote, a code
//! block, tasks and a table) and a comment with inline code, in both themes; links open in the
//! browser.

use super::snapshot_tests::{harness_sized, remove_fixture};
use super::snapshot_tests_pull_request::with_pr;
use super::snapshot_tests_pull_request_review::{detail, entry, load};
use crate::msg::Msg;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{EntryKind, Verdict};
use glowfly_core::settings::ThemeChoice;

const RELEASE: &str = "\
:robot: I have created a release *beep* *boop*

## [0.2.0](https://github.com/acme/demo/compare/v0.1.0...v0.2.0) (2026-01-20)

### Features

* **search:** page the results ([1a2b3c4](https://github.com/acme/demo/commit/1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b))
* **search:** filter by date and author, and remember the last filter between sessions ([5d6e7f8](https://github.com/acme/demo/commit/5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e))
* add a settings page ([9a8b7c6](https://github.com/acme/demo/commit/9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f1a0b))
  * keyboard shortcuts for every control
  * a `--config` flag

### Bug Fixes

* keep the scroll position after a reload ([0f1e2d3](https://github.com/acme/demo/commit/0f1e2d3c4b5a69788796a5b4c3d2e1f0a9b8c7d6))

> **Note:** the search index is rebuilt on the first start.

```rust
fn main() {
    let hits = search(\"query\", 20);
    println!(\"{} results\", hits.len());
}
```

- [x] tests pass
- [ ] docs updated

| Package | Version |
|---------|---------|
| demo    | 0.2.0   |

---
This PR was generated with [Release Please](https://github.com/googleapis/release-please). See https://github.com/googleapis/release-please#readme for details.";

const COMMENT: &str = "Looks good. Run `cargo test -p demo` once more, then **merge**.";

fn render(theme: ThemeChoice, name: &str) {
    let dark = matches!(theme, ThemeChoice::Dark);
    let (mut st, dir) = with_pr(theme, name, true);
    st.session.pull_request.view.as_mut().unwrap().pr.description = RELEASE.into();
    load(&mut st, detail(), vec![entry("grace", 13, EntryKind::Review(Verdict::Approved), COMMENT)]);
    let mut h = harness_sized(st, dark, egui::vec2(1400.0, 1250.0));
    h.run();
    h.check_snapshot(name);
    assert!(h.query_by_label("Features").is_some(), "a heading");
    assert_eq!(h.get_all_by_label("search:").count(), 2, "the bold scopes");
    assert!(h.query_by_label("cargo test -p demo").is_some(), "inline code");
    assert!(h.query_by_label_contains("**").is_none(), "no raw Markdown");
    h.get_by_label("1a2b3c4").click();
    h.run();
    let url = "https://github.com/acme/demo/commit/1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b";
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::OpenUrl(u) if u == url)));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn pull_request_markdown_dark() {
    render(ThemeChoice::Dark, "pull_request_markdown_dark");
}

#[test]
fn pull_request_markdown_light() {
    render(ThemeChoice::Light, "pull_request_markdown_light");
}
