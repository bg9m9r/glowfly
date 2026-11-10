//! The names of persist's temporary files, read back: which file a temporary file was written
//! for.

/// The name of the file a `temp_sibling` name (`.<name>.<process id>.<counter>.tmp`) was made
/// for; None when `candidate` isn't such a name. Names may hold dots themselves, so it is read
/// from the end.
pub(crate) fn temp_target(candidate: &str) -> Option<&str> {
    let rest = candidate.strip_prefix('.')?.strip_suffix(".tmp")?;
    let mut parts = rest.rsplitn(3, '.');
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let (counter, pid, name) = (parts.next()?, parts.next()?, parts.next()?);
    (digits(counter) && digits(pid) && !name.is_empty()).then_some(name)
}
