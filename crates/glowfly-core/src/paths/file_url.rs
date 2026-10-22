//! The local path a `file:` URL names. Only macOS hands Glowfly these (the folders an Open
//! Documents Apple event carries); the other systems pass paths on the command line, so there
//! the function only needs to compile and pass its tests.

use percent_encoding::percent_decode_str;
use std::path::PathBuf;

/// The local path `url` names: `file:` in any case, then `//`, an empty or `localhost` host and
/// an absolute path, percent-decoded. A folder's URL ends in `/`; that is dropped, so it reads
/// like a typed path. None for another scheme or host, a relative path, a query or fragment, a
/// NUL byte, and (outside Unix) a name that isn't UTF-8.
pub fn from_file_url(url: &str) -> Option<PathBuf> {
    let (scheme, rest) = url.split_once(':')?;
    if !scheme.eq_ignore_ascii_case("file") {
        return None;
    }
    let rest = rest.strip_prefix("//")?;
    let slash = rest.find('/')?;
    let (host, path) = rest.split_at(slash);
    if !(host.is_empty() || host.eq_ignore_ascii_case("localhost")) {
        return None;
    }
    if path.contains(['?', '#']) {
        return None;
    }
    let mut bytes: Vec<u8> = percent_decode_str(path).collect();
    if bytes.contains(&0) {
        return None;
    }
    while bytes.len() > 1 && bytes.last() == Some(&b'/') {
        bytes.pop();
    }
    path_of(bytes)
}

#[cfg(unix)]
fn path_of(bytes: Vec<u8>) -> Option<PathBuf> {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    Some(PathBuf::from(OsStr::from_bytes(&bytes)))
}

#[cfg(not(unix))]
fn path_of(bytes: Vec<u8>) -> Option<PathBuf> {
    String::from_utf8(bytes).ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_urls_name_local_paths() {
        let got = |url: &str| from_file_url(url).unwrap_or_else(|| panic!("{url} names a path"));
        assert_eq!(got("file:///Users/me/My%20Repo/"), PathBuf::from("/Users/me/My Repo"));
        assert_eq!(got("FILE://localhost/srv/r"), PathBuf::from("/srv/r"));
        assert_eq!(got("file://LocalHost/srv/r//"), PathBuf::from("/srv/r"));
        assert_eq!(got("file:///"), PathBuf::from("/"));
        assert_eq!(got("file:///a%23b"), PathBuf::from("/a#b"));
        assert_eq!(got("file:///caf%C3%A9"), PathBuf::from("/café"));
    }

    #[test]
    fn other_urls_are_refused() {
        for url in [
            "file://server/share/r",
            "file:relative",
            "file://",
            "https://example.com/r",
            "file:///a%00b",
            "file:///a?b=1",
            "file:///a#x",
            "no scheme",
        ] {
            assert_eq!(from_file_url(url), None, "{url}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_name_that_is_not_utf8_is_kept() {
        use std::os::unix::ffi::OsStrExt;
        let got = from_file_url("file:///caf%E9").expect("a path");
        assert_eq!(got.as_os_str().as_bytes(), b"/caf\xe9");
    }

    #[cfg(not(unix))]
    #[test]
    fn a_name_that_is_not_utf8_is_refused() {
        assert_eq!(from_file_url("file:///caf%E9"), None);
    }
}
