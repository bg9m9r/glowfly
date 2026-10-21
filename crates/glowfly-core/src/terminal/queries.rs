//! Queries a program sends the terminal and expects an answer to, found in its output.

/// A query the terminal answers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Query {
    /// DSR 6: where is the cursor?
    CursorPosition,
    /// DSR 5: are you working?
    Status,
    /// Primary device attributes: what kind of terminal are you?
    Attributes,
}

/// A control sequence longer than this is never held back waiting for its end.
const MAX_HELD: usize = 32;

/// The complete control sequence (ESC [ params final) starting at `data[0]`, as (length,
/// params, final byte); None if `data` ends before it does.
fn csi(data: &[u8]) -> Option<(usize, &[u8], u8)> {
    let body = data.get(2..)?;
    let params = body.iter().take_while(|b| (0x20..=0x3f).contains(*b)).count();
    let fin = *body.get(params)?;
    Some((2 + params + 1, &body[..params], fin))
}

/// Each query in `data` as (the index just past it, query), in order.
pub(super) fn find(data: &[u8]) -> Vec<(usize, Query)> {
    let mut found = Vec::new();
    let mut i = 0;
    while let Some(off) = data[i..].iter().position(|&b| b == 0x1b) {
        let at = i + off;
        if data.get(at + 1) != Some(&b'[') {
            i = at + 1;
            continue;
        }
        let Some((len, params, fin)) = csi(&data[at..]) else { break };
        let query = match (params, fin) {
            (b"6", b'n') => Some(Query::CursorPosition),
            (b"5", b'n') => Some(Query::Status),
            (b"" | b"0", b'c') => Some(Query::Attributes),
            _ => None,
        };
        if let Some(q) = query {
            found.push((at + len, q));
        }
        i = at + len;
    }
    found
}

/// How many bytes at the end of `data` might be the start of a control sequence that the next
/// chunk of output completes.
pub(super) fn partial_tail(data: &[u8]) -> usize {
    let Some(at) = data.iter().rposition(|&b| b == 0x1b) else { return 0 };
    let tail = &data[at..];
    let incomplete = match tail.get(1) {
        None => true,
        Some(b'[') => csi(tail).is_none(),
        Some(_) => false,
    };
    if incomplete && tail.len() <= MAX_HELD { tail.len() } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_queries_and_partial_sequences() {
        let data = b"ab\x1b[6n\x1b[1;2H\x1b[5n\x1b[c\x1b[?6n";
        assert_eq!(find(data), vec![(6, Query::CursorPosition), (16, Query::Status), (19, Query::Attributes)]);
        assert_eq!(partial_tail(b"abc"), 0);
        assert_eq!(partial_tail(b"abc\x1b"), 1);
        assert_eq!(partial_tail(b"abc\x1b[6"), 3);
        assert_eq!(partial_tail(b"abc\x1b[6n"), 0);
        assert_eq!(partial_tail(b"abc\x1b]0;title"), 0);
        assert!(find(b"\x1b[6").is_empty());
    }
}
