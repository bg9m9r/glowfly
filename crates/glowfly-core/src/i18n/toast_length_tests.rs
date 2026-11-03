//! Toasts stay short: every English text under a `toast` key segment (`clone.toast.retry`,
//! `watch.silent.toast`) is at most `LIMIT` characters, so the toast is one short sentence and
//! the longer story goes to the activity log behind Details. `LONGER` names the texts still over
//! it; it only shrinks.

use super::catalog::source;
use super::*;
use std::collections::BTreeMap;

/// The most characters a toast's English text may have, placeholders as written.
const LIMIT: usize = 140;

/// Toast texts over `LIMIT` today. Shorten one and take it off; never add one.
const LONGER: &[&str] = &[];

#[test]
fn toast_texts_are_short() {
    let en: BTreeMap<String, String> = serde_json::from_str(source(Lang::En)).expect("en.json parses");
    let toast = |k: &str| k.split('.').any(|seg| seg == "toast");
    let long: Vec<(&String, usize)> =
        en.iter().filter(|(k, _)| toast(k)).map(|(k, v)| (k, v.chars().count())).filter(|(_, n)| *n > LIMIT).collect();
    let new: Vec<_> = long.iter().filter(|(k, _)| !LONGER.contains(&k.as_str())).collect();
    assert!(new.is_empty(), "toast texts over {LIMIT} characters (move the detail to the activity log): {new:?}");
    let fixed: Vec<_> = LONGER.iter().filter(|k| !long.iter().any(|(l, _)| l == *k)).collect();
    assert!(fixed.is_empty(), "these toasts are short now; take them off LONGER: {fixed:?}");
    assert!(en.keys().filter(|k| toast(k)).count() > 30, "the toast keys are found");
}
