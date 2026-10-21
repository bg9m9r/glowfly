//! Semantic versions (`MAJOR.MINOR.PATCH[-pre][+build]`) and their precedence.

use std::cmp::Ordering;
use std::fmt;

/// A version, compared by semantic-versioning precedence. Build metadata is dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// Pre-release identifiers (`rc.1` is `[Alpha("rc"), Num(1)]`); empty for a release.
    pub pre: Vec<PreId>,
}

/// One pre-release identifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreId {
    Num(u64),
    Alpha(String),
}

/// A numeric part: digits only, without a leading zero (except `0` itself).
fn number(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) || (s.len() > 1 && s.starts_with('0')) {
        return None;
    }
    s.parse().ok()
}

fn ident_ok(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

impl Version {
    /// Parses `1.2.3`, `v1.2.3`, `V1.2.3-rc.1` or `1.2.3+build.5`; anything else is None.
    pub fn parse(text: &str) -> Option<Version> {
        let s = text.strip_prefix(['v', 'V']).unwrap_or(text);
        let (s, build) = match s.split_once('+') {
            Some((v, b)) => (v, Some(b)),
            None => (s, None),
        };
        if build.is_some_and(|b| !b.split('.').all(ident_ok)) {
            return None;
        }
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) => (c, Some(p)),
            None => (s, None),
        };
        let mut parts = core.split('.');
        let (major, minor, patch) = (number(parts.next()?)?, number(parts.next()?)?, number(parts.next()?)?);
        if parts.next().is_some() {
            return None;
        }
        let pre = match pre {
            None => Vec::new(),
            Some(p) => p
                .split('.')
                .map(|id| {
                    if !ident_ok(id) {
                        None
                    } else if id.bytes().all(|b| b.is_ascii_digit()) {
                        number(id).map(PreId::Num)
                    } else {
                        Some(PreId::Alpha(id.to_owned()))
                    }
                })
                .collect::<Option<Vec<_>>>()?,
        };
        Some(Version { major, minor, patch, pre })
    }

    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl Ord for PreId {
    /// Numeric identifiers sort below alphanumeric ones; each kind in its own order.
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (PreId::Num(a), PreId::Num(b)) => a.cmp(b),
            (PreId::Num(_), PreId::Alpha(_)) => Ordering::Less,
            (PreId::Alpha(_), PreId::Num(_)) => Ordering::Greater,
            (PreId::Alpha(a), PreId::Alpha(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for PreId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch)).then_with(|| {
            match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                // A pre-release sorts below its release.
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                // Identifier by identifier; a shorter list with the same prefix sorts first.
                (false, false) => self.pre.cmp(&other.pre),
            }
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for PreId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PreId::Num(n) => write!(f, "{n}"),
            PreId::Alpha(s) => f.write_str(s),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        for (i, id) in self.pre.iter().enumerate() {
            write!(f, "{}{id}", if i == 0 { '-' } else { '.' })?;
        }
        Ok(())
    }
}
