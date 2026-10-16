use super::*;
use std::ffi::{OsStr, OsString};

fn argv(first: &str) -> Vec<OsString> {
    vec![OsString::from("trellis"), OsString::from(first)]
}

#[test]
fn only_marked_option_calls_are_wrapper_calls() {
    let one = Some(OsStr::new("1"));
    assert!(is_wrapper_call(&argv("--status-fd=2"), one));
    assert!(is_wrapper_call(&argv("--keyid-format=long"), Some(OsStr::new("nested"))));
    assert!(!is_wrapper_call(&argv("--status-fd=2"), None), "no marker");
    assert!(!is_wrapper_call(&argv("--status-fd=2"), Some(OsStr::new("0"))));
    assert!(!is_wrapper_call(&argv("Username for 'https://x': "), one), "an askpass prompt");
    assert!(!is_wrapper_call(&argv("--askpass"), one));
    assert!(!is_wrapper_call(&argv("--write-todo"), one));
    assert!(!is_wrapper_call(&[OsString::from("trellis")], one));
}

#[test]
fn a_nested_call_is_a_loop() {
    assert!(is_nested(Some(OsStr::new("nested"))));
    assert!(!is_nested(Some(OsStr::new("1"))));
    assert!(!is_nested(None));
}
