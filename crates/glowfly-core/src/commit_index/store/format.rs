//! The saved index's byte layout, written and read in one pass. All integers are little-endian.
//!
//! A 64-byte header (magic, `FORMAT`, oid length, flags, the cap the index was built with, the
//! body's length and the count of every section), then the body's sections in order: the
//! repository the file belongs to, the Glowfly version that wrote it, the starts, the shallow
//! boundary, one 64-byte record per entry, the walked parents, the authors and the text blocks.
//! A CRC-32 of every byte before it ends the file.
//!
//! Reading trusts nothing: the length, magic and counts are checked before anything is
//! allocated, every string and block must fit in what is left and be UTF-8, every entry's
//! author, parents and text must lie inside their sections (on character boundaries), oids must
//! be distinct and the checksum must match. Anything else is `Damaged`.

use super::super::{CommitIndex, Entry, IndexKey, MAX_INDEXED};
use super::{FORMAT, Stored};
use crate::repo::Author;
use flate2::Crc;
use git2::Oid;
use std::collections::HashMap;
use std::io::{self, BufReader, Read, Write};
use std::path::Path;
use std::sync::Arc;

const MAGIC: &[u8; 8] = b"GLOWIDX\0";
pub(super) const HEADER: usize = 64;
const OID: usize = 20;
const ENTRY: usize = 64;
const TRAILER: usize = 4;
/// How often (in entries, parents or authors) a read asks whether to stop.
const CHECK_EVERY: usize = 65_536;
/// Flag bit: the index holds the whole history.
const COMPLETE: u32 = 1;

/// This Glowfly's version, saved with the index: positions are trusted only from the same one.
pub(super) fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Writes `ix`, saved for the repository `repo`, to `w`.
pub(super) fn write<W: Write>(w: W, repo: &str, ix: &CommitIndex) -> io::Result<()> {
    let version = app_version();
    let strings = |s: &str| 4 + s.len() as u64;
    let body_len = strings(repo)
        + strings(version)
        + (OID * (ix.key.starts.len() + ix.key.shallow.len() + ix.parents.len())) as u64
        + (ENTRY * ix.entries.len()) as u64
        + ix.people.iter().map(|a| strings(a.name()) + strings(a.email())).sum::<u64>()
        + ix.texts.iter().map(|t| strings(t)).sum::<u64>();
    let mut w = CrcWrite { inner: w, crc: Crc::new() };
    let count = |n: usize| u32::try_from(n).map_err(|_| io::Error::other("too many items to save"));
    w.write_all(MAGIC)?;
    for v in [FORMAT, OID as u32, if ix.complete { COMPLETE } else { 0 }, MAX_INDEXED as u32] {
        w.write_all(&v.to_le_bytes())?;
    }
    w.write_all(&body_len.to_le_bytes())?;
    let counts = [ix.key.starts.len(), ix.key.shallow.len(), ix.entries.len(), ix.parents.len(), ix.people.len(), ix.texts.len()];
    for n in counts {
        w.write_all(&count(n)?.to_le_bytes())?;
    }
    w.write_all(&[0; 8])?;
    let string = |w: &mut CrcWrite<W>, s: &str| -> io::Result<()> {
        w.write_all(&count(s.len())?.to_le_bytes())?;
        w.write_all(s.as_bytes())
    };
    string(&mut w, repo)?;
    string(&mut w, version)?;
    for oid in ix.key.starts.iter().chain(&ix.key.shallow) {
        w.write_all(oid.as_bytes())?;
    }
    let mut rec = [0u8; ENTRY];
    for e in &ix.entries {
        rec[..OID].copy_from_slice(e.oid.as_bytes());
        rec[20..28].copy_from_slice(&e.time.to_le_bytes());
        rec[28..36].copy_from_slice(&e.author_time.to_le_bytes());
        let fields = [e.person, e.block, e.at, e.summary_len, e.body_len, e.parents_at, e.parents_len];
        for (i, v) in fields.into_iter().enumerate() {
            rec[36 + 4 * i..40 + 4 * i].copy_from_slice(&v.to_le_bytes());
        }
        w.write_all(&rec)?;
    }
    for p in &ix.parents {
        w.write_all(p.as_bytes())?;
    }
    for a in &ix.people {
        string(&mut w, a.name())?;
        string(&mut w, a.email())?;
    }
    for t in &ix.texts {
        string(&mut w, t)?;
    }
    let crc = w.crc.sum();
    let mut inner = w.inner;
    inner.write_all(&crc.to_le_bytes())?;
    inner.flush()
}

/// Why a saved index wasn't read.
#[derive(Debug)]
pub(super) enum ReadError {
    /// It isn't a valid index file (or not this repository's): delete it.
    Damaged(&'static str),
    /// Another layout version: leave it to be overwritten.
    OtherFormat,
    /// `stop` said to stop.
    Stopped,
    Io(io::Error),
}

impl From<io::Error> for ReadError {
    fn from(e: io::Error) -> Self {
        // The length was checked first, so running out of bytes means the file changed under us.
        if e.kind() == io::ErrorKind::UnexpectedEof { ReadError::Damaged("truncated") } else { ReadError::Io(e) }
    }
}

/// The header's fields after the magic.
struct Header {
    format: u32,
    oid_len: u32,
    flags: u32,
    cap: u32,
    body_len: u64,
    starts: usize,
    shallow: usize,
    entries: usize,
    parents: usize,
    people: usize,
    blocks: usize,
}

impl Header {
    fn parse(b: &[u8; HEADER]) -> Header {
        let u32_at = |at: usize| u32::from_le_bytes(b[at..at + 4].try_into().expect("4 bytes"));
        let n = |at: usize| u32_at(at) as usize;
        Header {
            format: u32_at(8),
            oid_len: u32_at(12),
            flags: u32_at(16),
            cap: u32_at(20),
            body_len: u64::from_le_bytes(b[24..32].try_into().expect("8 bytes")),
            starts: n(32),
            shallow: n(36),
            entries: n(40),
            parents: n(44),
            people: n(48),
            blocks: n(52),
        }
    }

    /// The least the body can hold with these counts (every string empty), or None on overflow.
    fn least_body(&self) -> Option<u64> {
        let oids = (self.starts as u64).checked_add(self.shallow as u64)?.checked_add(self.parents as u64)?;
        let fixed = oids.checked_mul(OID as u64)?.checked_add((self.entries as u64).checked_mul(ENTRY as u64)?)?;
        let strings = 2 + (self.people as u64).checked_mul(2)?.checked_add(self.blocks as u64)?;
        fixed.checked_add(strings.checked_mul(4)?)
    }
}

/// Reads the index saved at `path` for the repository `expect_repo`.
pub(super) fn read(path: &Path, expect_repo: &str, stop: &dyn Fn() -> bool) -> Result<(CommitIndex, Stored), ReadError> {
    let file = std::fs::File::open(path)?;
    let file_len = file.metadata()?.len();
    let mut r = Reader { inner: CrcRead { inner: BufReader::with_capacity(1 << 20, file), crc: Crc::new() }, left: 0 };
    if file_len < (HEADER + TRAILER) as u64 {
        return Err(ReadError::Damaged("shorter than a header"));
    }
    let mut head = [0u8; HEADER];
    r.inner.read_exact(&mut head)?;
    let h = Header::parse(&head);
    if Some(file_len) != h.body_len.checked_add((HEADER + TRAILER) as u64) {
        return Err(ReadError::Damaged("length"));
    }
    if &head[..8] != MAGIC {
        return Err(ReadError::Damaged("magic"));
    }
    if h.format != FORMAT || h.oid_len != OID as u32 {
        return Err(ReadError::OtherFormat);
    }
    if h.entries > MAX_INDEXED || h.least_body().is_none_or(|least| least > h.body_len) {
        return Err(ReadError::Damaged("counts"));
    }
    r.left = h.body_len;
    if r.string()? != expect_repo {
        return Err(ReadError::Damaged("another repository"));
    }
    let version = r.string()?;
    let starts = r.oids(h.starts)?;
    let shallow = r.oids(h.shallow)?;
    let mut entries = Vec::with_capacity(h.entries);
    let mut by_oid = HashMap::with_capacity(h.entries);
    let mut rec = [0u8; ENTRY];
    for i in 0..h.entries {
        if i % CHECK_EVERY == 0 && stop() {
            return Err(ReadError::Stopped);
        }
        r.bytes(&mut rec)?;
        let u32_at = |at: usize| u32::from_le_bytes(rec[at..at + 4].try_into().expect("4 bytes"));
        let i64_at = |at: usize| i64::from_le_bytes(rec[at..at + 8].try_into().expect("8 bytes"));
        let oid = Oid::from_bytes(&rec[..OID]).map_err(|_| ReadError::Damaged("oid"))?;
        if by_oid.insert(oid, i as u32).is_some() {
            return Err(ReadError::Damaged("duplicate oid"));
        }
        entries.push(Entry {
            oid,
            time: i64_at(20),
            author_time: i64_at(28),
            person: u32_at(36),
            block: u32_at(40),
            at: u32_at(44),
            summary_len: u32_at(48),
            body_len: u32_at(52),
            parents_at: u32_at(56),
            parents_len: u32_at(60),
        });
    }
    let parents = r.oids_stoppable(h.parents, stop)?;
    let mut people = Vec::with_capacity(h.people);
    for i in 0..h.people {
        if i % CHECK_EVERY == 0 && stop() {
            return Err(ReadError::Stopped);
        }
        let name = r.string()?;
        let email = r.string()?;
        people.push(Author::new(&name, &email));
    }
    let mut texts: Vec<Arc<str>> = Vec::with_capacity(h.blocks);
    for _ in 0..h.blocks {
        if stop() {
            return Err(ReadError::Stopped);
        }
        texts.push(Arc::from(r.string()?));
    }
    for e in &entries {
        check_entry(e, &people, parents.len(), &texts)?;
    }
    if r.left != 0 {
        return Err(ReadError::Damaged("body length"));
    }
    let sum = r.inner.crc.sum();
    let mut trailer = [0u8; TRAILER];
    r.inner.inner.read_exact(&mut trailer)?;
    if u32::from_le_bytes(trailer) != sum {
        return Err(ReadError::Damaged("checksum"));
    }
    let mut person_of = HashMap::with_capacity(people.len());
    for (i, a) in people.iter().enumerate() {
        person_of.entry(a.clone()).or_insert(i as u32);
    }
    let complete = h.flags & COMPLETE != 0;
    let current = version == app_version() && (complete || h.cap as usize == MAX_INDEXED);
    let key = IndexKey { starts, shallow };
    let stored = Stored { key: key.clone(), len: entries.len(), complete, current };
    let ix = CommitIndex { key, complete, entries, by_oid, parents, texts, people, person_of };
    Ok((ix, stored))
}

/// Whether `e`'s author, parents and texts lie inside their sections.
fn check_entry(e: &Entry, people: &[Author], parents: usize, texts: &[Arc<str>]) -> Result<(), ReadError> {
    if e.person as usize >= people.len() {
        return Err(ReadError::Damaged("author"));
    }
    if e.parents_at as u64 + e.parents_len as u64 > parents as u64 {
        return Err(ReadError::Damaged("parents"));
    }
    // An entry without text may name any block (the builder leaves the next one's number).
    if e.summary_len == 0 && e.body_len == 0 {
        return Ok(());
    }
    let block = texts.get(e.block as usize).ok_or(ReadError::Damaged("block"))?;
    let (at, mid) = (e.at as u64, e.at as u64 + e.summary_len as u64);
    let end = mid + e.body_len as u64;
    if end > block.len() as u64 {
        return Err(ReadError::Damaged("text range"));
    }
    if ![at, mid, end].iter().all(|&i| block.is_char_boundary(i as usize)) {
        return Err(ReadError::Damaged("text boundary"));
    }
    Ok(())
}

/// The body being read, with how many of its bytes are left.
struct Reader<R: Read> {
    inner: CrcRead<R>,
    left: u64,
}

impl<R: Read> Reader<R> {
    fn bytes(&mut self, buf: &mut [u8]) -> Result<(), ReadError> {
        let n = buf.len() as u64;
        if n > self.left {
            return Err(ReadError::Damaged("past the body"));
        }
        self.inner.read_exact(buf)?;
        self.left -= n;
        Ok(())
    }

    fn string(&mut self) -> Result<String, ReadError> {
        let mut len = [0u8; 4];
        self.bytes(&mut len)?;
        let len = u32::from_le_bytes(len) as u64;
        if len > self.left {
            return Err(ReadError::Damaged("string length"));
        }
        let mut buf = vec![0u8; len as usize];
        self.bytes(&mut buf)?;
        String::from_utf8(buf).map_err(|_| ReadError::Damaged("not UTF-8"))
    }

    fn oids(&mut self, n: usize) -> Result<Vec<Oid>, ReadError> {
        self.oids_stoppable(n, &|| false)
    }

    fn oids_stoppable(&mut self, n: usize, stop: &dyn Fn() -> bool) -> Result<Vec<Oid>, ReadError> {
        let mut out = Vec::with_capacity(n);
        let mut b = [0u8; OID];
        for i in 0..n {
            if i % CHECK_EVERY == 0 && stop() {
                return Err(ReadError::Stopped);
            }
            self.bytes(&mut b)?;
            out.push(Oid::from_bytes(&b).map_err(|_| ReadError::Damaged("oid"))?);
        }
        Ok(out)
    }
}

/// A writer that keeps a CRC-32 of what went through it.
struct CrcWrite<W: Write> {
    inner: W,
    crc: Crc,
}

impl<W: Write> Write for CrcWrite<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.crc.update(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// A reader that keeps a CRC-32 of what came through it.
struct CrcRead<R: Read> {
    inner: R,
    crc: Crc,
}

impl<R: Read> Read for CrcRead<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.crc.update(&buf[..n]);
        Ok(n)
    }
}
