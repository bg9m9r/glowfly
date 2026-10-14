//! Lists that span several pages. Every service caps a page (usually at 100 items), so lists
//! follow the service's "next page" pointer, up to a total cap, and only to the same host.

use super::http::get_paged;
use super::links::https_host;
use super::{Auth, enc};
use crate::error::Result;
use serde::de::DeserializeOwned;

/// The most items any one list collects.
pub(super) const MAX_ITEMS: usize = 1000;
/// The most pages any one list asks for, in case a server sends tiny pages.
const MAX_PAGES: usize = 50;

/// One page of a list, and where the next one is.
pub(super) struct Page<T> {
    pub items: Vec<T>,
    pub next: Option<String>,
}

/// Collects `first` and the pages after it. A next-page URL is followed only when it is https
/// on the same host as the first page and isn't the page just read.
pub(super) fn collect<T>(first: String, mut fetch: impl FnMut(&str) -> Result<Page<T>>) -> Result<Vec<T>> {
    let host = https_host(&first);
    let mut out = Vec::new();
    let mut url = Some(first);
    for _ in 0..MAX_PAGES {
        let Some(this) = url.take() else { break };
        let page = fetch(&this)?;
        let empty = page.items.is_empty();
        out.extend(page.items);
        if out.len() >= MAX_ITEMS {
            out.truncate(MAX_ITEMS);
            break;
        }
        if empty {
            break;
        }
        url = page.next.filter(|n| *n != this && https_host(n).is_some() && https_host(n) == host);
    }
    Ok(out)
}

/// A list paged by `Link: <…>; rel="next"` headers (GitHub, GitLab).
pub(super) fn by_link<T: DeserializeOwned>(auth: &Auth, first: String) -> Result<Vec<T>> {
    collect(first, |url| {
        let p = get_paged::<Vec<T>>(auth, url)?;
        Ok(Page { items: p.body, next: p.link_next })
    })
}

/// `base` with one more query parameter.
pub(super) fn with_param(base: &str, name: &str, value: &str) -> String {
    let sep = if base.contains('?') { '&' } else { '?' };
    format!("{base}{sep}{name}={}", enc(value))
}

/// The `rel="next"` target of a `Link` header.
pub(super) fn next_link(header: &str) -> Option<String> {
    let mut rest = header;
    while let Some(start) = rest.find('<') {
        let end = start + rest[start..].find('>')?;
        let url = &rest[start + 1..end];
        let params = &rest[end + 1..];
        let params = &params[..params.find('<').unwrap_or(params.len())];
        let is_next = params.split(';').any(|p| {
            let p = p.trim().trim_end_matches(',').trim();
            p.strip_prefix("rel=").is_some_and(|v| v.trim_matches('"').split_whitespace().any(|r| r.eq_ignore_ascii_case("next")))
        });
        if is_next {
            return Some(url.to_owned());
        }
        rest = &rest[end + 1..];
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_next_link() {
        let h = r#"<https://api.github.com/repositories/1/pulls?page=2>; rel="next", <https://api.github.com/repositories/1/pulls?page=5>; rel="last""#;
        assert_eq!(next_link(h).as_deref(), Some("https://api.github.com/repositories/1/pulls?page=2"));
        let h = r#"<https://gitlab.com/api/v4/x?page=1>; rel="first", <https://gitlab.com/api/v4/x?page=3,4>; rel="prev next""#;
        assert_eq!(next_link(h).as_deref(), Some("https://gitlab.com/api/v4/x?page=3,4"));
        assert_eq!(next_link(r#"<https://x/?page=1>; rel="prev""#), None);
        assert_eq!(next_link(""), None);
        assert_eq!(next_link("<unterminated; rel=next"), None);
    }

    fn pages(next: impl Fn(usize) -> Option<String>, per_page: usize) -> (Vec<usize>, Vec<String>) {
        let mut asked = Vec::new();
        let mut n = 0;
        let items = collect("https://h.example/list".into(), |url| {
            asked.push(url.to_owned());
            n += 1;
            Ok(Page { items: (0..per_page).collect(), next: next(n) })
        })
        .unwrap();
        (items, asked)
    }

    #[test]
    fn follows_pages_on_the_same_host_up_to_the_cap() {
        let (items, asked) = pages(|n| (n < 3).then(|| format!("https://H.example/list?page={}", n + 1)), 10);
        assert_eq!((items.len(), asked.len()), (30, 3));
        // An endless list stops at the item cap.
        let (items, asked) = pages(|n| Some(format!("https://h.example/list?page={}", n + 1)), 100);
        assert_eq!((items.len(), asked.len()), (MAX_ITEMS, 10));
        // ... or the page cap, with tiny pages.
        let (items, asked) = pages(|n| Some(format!("https://h.example/list?page={}", n + 1)), 1);
        assert_eq!((items.len(), asked.len()), (MAX_PAGES, MAX_PAGES));
        // An empty page ends the list.
        assert_eq!(pages(|_| Some("https://h.example/list?page=2".into()), 0).1.len(), 1);
    }

    #[test]
    fn never_follows_pages_elsewhere() {
        for bad in [
            "https://evil.example/list?page=2",
            "http://h.example/list?page=2",
            "https://h.example.evil.example/list",
            "https://user@h.example/list",
            "https://h.example/list",
        ] {
            let (items, asked) = pages(|_| Some(bad.to_owned()), 5);
            assert_eq!((items.len(), asked.len()), (5, 1), "{bad}");
        }
    }

    #[test]
    fn adds_query_parameters() {
        assert_eq!(with_param("https://h/x?a=1", "start", "25"), "https://h/x?a=1&start=25");
        assert_eq!(with_param("https://h/x", "token", "a b&c"), "https://h/x?token=a%20b%26c");
    }
}
