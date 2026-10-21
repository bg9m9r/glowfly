//! Markdown parsed into the few block and inline kinds Glowfly draws (pull request and issue
//! descriptions and comments): headings, paragraphs, code, lists (with task items), quotes,
//! tables and rules, with text runs that are strong, emphasised, struck, code, an image's alt
//! text or a link. Bare `https://` addresses in text become links. HTML is dropped and an
//! image becomes its alt text linked to its source, so nothing is fetched. Pure: no I/O.

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

/// The longest Markdown text that is parsed and laid out; longer text is shown plain.
pub const MARKDOWN_MAX_BYTES: usize = 512 * 1024;
/// The deepest quotes and lists nest; deeper ones are drawn at this depth.
const MAX_DEPTH: usize = 24;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MdDoc {
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    /// Level 1 to 6.
    Heading {
        level: u8,
        text: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    /// A fenced or indented code block (or front matter); `lang` is the info string's first
    /// word.
    Code {
        lang: Option<String>,
        text: String,
    },
    /// `start` is an ordered list's first number; `None` for bullets.
    List {
        start: Option<u64>,
        items: Vec<Item>,
    },
    Quote(Vec<Block>),
    /// The header row's cells, then each row's.
    Table {
        head: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    Rule,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Item {
    /// A task item's box: ticked or not.
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

/// A run of text in one style.
#[derive(Clone, Debug, PartialEq)]
pub struct Inline {
    pub text: String,
    pub style: InlineStyle,
    /// The link's target, as written.
    pub link: Option<String>,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct InlineStyle {
    pub strong: bool,
    pub emphasis: bool,
    pub strike: bool,
    pub code: bool,
    /// An image's alt text.
    pub image: bool,
}

/// A container being filled.
enum Frame {
    /// The document, a quote or a list item.
    Blocks {
        blocks: Vec<Block>,
        quote: bool,
        task: Option<bool>,
    },
    List {
        start: Option<u64>,
        items: Vec<Item>,
    },
    Table {
        head: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
        row: Vec<Vec<Inline>>,
    },
}

#[derive(Default)]
struct Builder {
    frames: Vec<Frame>,
    /// For each open quote, list or item: whether it got a frame (not past `MAX_DEPTH`).
    opened: Vec<bool>,
    inlines: Vec<Inline>,
    strong: u32,
    emphasis: u32,
    strike: u32,
    image: u32,
    /// Where the open image's alt text starts in `inlines`.
    image_from: usize,
    link: Option<String>,
    /// The open image (outside any link) set `link` to its source.
    image_link: bool,
    /// The open code block's language and text.
    code: Option<(Option<String>, String)>,
    heading: Option<u8>,
}

impl Builder {
    fn style(&self) -> InlineStyle {
        InlineStyle { strong: self.strong > 0, emphasis: self.emphasis > 0, strike: self.strike > 0, code: false, image: self.image > 0 }
    }

    /// Appends text, merged into the last run when that has the same style and link.
    fn text(&mut self, text: &str, style: InlineStyle) {
        if let Some(last) = self.inlines.last_mut().filter(|l| l.style == style && l.link == self.link) {
            last.text.push_str(text);
        } else {
            self.inlines.push(Inline { text: text.to_owned(), style, link: self.link.clone() });
        }
    }

    fn push(&mut self, block: Block) {
        if let Some(Frame::Blocks { blocks, .. }) = self.frames.iter_mut().rev().find(|f| matches!(f, Frame::Blocks { .. })) {
            blocks.push(block);
        }
    }

    /// The runs written so far, with bare addresses linked.
    fn take_inlines(&mut self) -> Vec<Inline> {
        autolink(std::mem::take(&mut self.inlines))
    }

    /// Text met outside a paragraph (a tight list item's) becomes one.
    fn flush(&mut self) {
        if !self.inlines.is_empty() {
            let text = self.take_inlines();
            self.push(Block::Paragraph(text));
        }
    }

    /// Opens a quote, list or item frame, unless nested too deep.
    fn open(&mut self, frame: Frame) {
        self.flush();
        let depth = self.frames.iter().filter(|f| !matches!(f, Frame::Table { .. })).count();
        let item = matches!(frame, Frame::Blocks { quote: false, .. });
        let ok = if item { matches!(self.frames.last(), Some(Frame::List { .. })) } else { depth < MAX_DEPTH };
        if ok {
            self.frames.push(frame);
        }
        self.opened.push(ok);
    }

    fn close(&mut self) {
        self.flush();
        if !self.opened.pop().unwrap_or(false) {
            return;
        }
        match self.frames.pop() {
            Some(Frame::Blocks { blocks, quote: true, .. }) => self.push(Block::Quote(blocks)),
            Some(Frame::Blocks { blocks, task, .. }) => {
                if let Some(Frame::List { items, .. }) = self.frames.last_mut() {
                    items.push(Item { task, blocks });
                }
            }
            Some(Frame::List { start, items }) => self.push(Block::List { start, items }),
            Some(Frame::Table { .. }) | None => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph | Tag::HtmlBlock => self.flush(),
            Tag::Heading { level, .. } => {
                self.flush();
                self.heading = Some(level as u8);
            }
            Tag::BlockQuote(_) => self.open(Frame::Blocks { blocks: Vec::new(), quote: true, task: None }),
            Tag::CodeBlock(kind) => {
                self.flush();
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => info.split_whitespace().next().map(str::to_owned),
                    CodeBlockKind::Indented => None,
                };
                self.code = Some((lang, String::new()));
            }
            Tag::MetadataBlock(_) => {
                self.flush();
                self.code = Some((Some("yaml".to_owned()), String::new()));
            }
            Tag::List(start) => self.open(Frame::List { start, items: Vec::new() }),
            Tag::Item => self.open(Frame::Blocks { blocks: Vec::new(), quote: false, task: None }),
            Tag::Table(_) => {
                self.flush();
                self.frames.push(Frame::Table { head: Vec::new(), rows: Vec::new(), row: Vec::new() });
            }
            Tag::Emphasis => self.emphasis += 1,
            Tag::Strong => self.strong += 1,
            Tag::Strikethrough => self.strike += 1,
            Tag::Link { dest_url, .. } => self.link = Some(dest_url.into_string()),
            Tag::Image { dest_url, .. } => {
                if self.image == 0 {
                    self.image_from = self.inlines.len();
                    self.image_link = self.link.is_none() && !dest_url.is_empty();
                    if self.image_link {
                        self.link = Some(dest_url.into_string());
                    }
                }
                self.image += 1;
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush(),
            TagEnd::Heading(_) => {
                let text = self.take_inlines();
                let level = self.heading.take().unwrap_or(1);
                self.push(Block::Heading { level, text });
            }
            TagEnd::CodeBlock | TagEnd::MetadataBlock(_) => {
                if let Some((lang, mut text)) = self.code.take() {
                    if text.ends_with('\n') {
                        text.pop();
                    }
                    self.push(Block::Code { lang, text });
                }
            }
            TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item => self.close(),
            TagEnd::TableCell => {
                let cell = self.take_inlines();
                if let Some(Frame::Table { row, .. }) = self.frames.last_mut() {
                    row.push(cell);
                }
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                if let Some(Frame::Table { head, rows, row }) = self.frames.last_mut() {
                    let cells = std::mem::take(row);
                    if tag == TagEnd::TableHead { *head = cells } else { rows.push(cells) }
                }
            }
            TagEnd::Table => {
                if let Some(Frame::Table { head, rows, .. }) = self.frames.pop() {
                    self.push(Block::Table { head, rows });
                }
            }
            TagEnd::Emphasis => self.emphasis = self.emphasis.saturating_sub(1),
            TagEnd::Strong => self.strong = self.strong.saturating_sub(1),
            TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
            TagEnd::Link => self.link = None,
            TagEnd::Image => {
                self.image = self.image.saturating_sub(1);
                if self.image == 0 && self.inlines.len() == self.image_from {
                    let style = InlineStyle { image: true, ..self.style() };
                    self.text("image", style);
                }
                if self.image == 0 && std::mem::take(&mut self.image_link) {
                    self.link = None;
                }
            }
            _ => {}
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(t) => match &mut self.code {
                Some((_, code)) => code.push_str(&t),
                None => self.text(&t, self.style()),
            },
            Event::Code(t) => self.text(&t, InlineStyle { code: true, ..self.style() }),
            Event::SoftBreak => self.text(" ", self.style()),
            Event::HardBreak => self.text("\n", self.style()),
            Event::Rule => {
                self.flush();
                self.push(Block::Rule);
            }
            Event::TaskListMarker(done) => {
                if let Some(Frame::Blocks { quote: false, task, .. }) = self.frames.last_mut() {
                    *task = Some(done);
                }
            }
            _ => {}
        }
    }
}

/// Parses `src` as CommonMark with tables, strikethrough, task lists and YAML front matter.
pub fn parse(src: &str) -> MdDoc {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
    let mut b = Builder { frames: vec![Frame::Blocks { blocks: Vec::new(), quote: false, task: None }], ..Builder::default() };
    for event in Parser::new_ext(src, options) {
        b.event(event);
    }
    b.flush();
    match b.frames.into_iter().next() {
        Some(Frame::Blocks { blocks, .. }) => MdDoc { blocks },
        _ => MdDoc::default(),
    }
}

/// `parse`, unless `src` is longer than `MARKDOWN_MAX_BYTES`.
pub fn parse_capped(src: &str) -> Option<MdDoc> {
    (src.len() <= MARKDOWN_MAX_BYTES).then(|| parse(src))
}

const SCHEME: &str = "https://";

/// Splits plain text runs (not code, not already a link) at bare `https://` addresses, each
/// address becoming a link to itself.
fn autolink(runs: Vec<Inline>) -> Vec<Inline> {
    if !runs.iter().any(|r| r.link.is_none() && !r.style.code && r.text.contains(SCHEME)) {
        return runs;
    }
    let mut out = Vec::with_capacity(runs.len() + 2);
    for run in runs {
        if run.link.is_some() || run.style.code {
            out.push(run);
            continue;
        }
        let mut rest = run.text.as_str();
        while let Some((at, len)) = find_url(rest) {
            if at > 0 {
                out.push(Inline { text: rest[..at].to_owned(), style: run.style, link: None });
            }
            let url = &rest[at..at + len];
            out.push(Inline { text: url.to_owned(), style: run.style, link: Some(url.to_owned()) });
            rest = &rest[at + len..];
        }
        if !rest.is_empty() {
            out.push(Inline { text: rest.to_owned(), style: run.style, link: None });
        }
    }
    out
}

/// The first bare address in `s`: its byte offset and length. It starts with `https://` at a
/// word boundary, runs to whitespace or `<`, and leaves off trailing punctuation and a closing
/// bracket that has no opening one inside it.
fn find_url(s: &str) -> Option<(usize, usize)> {
    let mut from = 0;
    while let Some(i) = s[from..].find(SCHEME) {
        let at = from + i;
        from = at + SCHEME.len();
        if s[..at].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '/') {
            continue;
        }
        let tail = &s[at..];
        let mut end = tail.find(|c: char| c.is_whitespace() || c == '<').unwrap_or(tail.len());
        loop {
            let url = &tail[..end];
            let Some(last) = url.chars().next_back() else { break };
            let unbalanced = |open: char| url.matches(open).count() < url.matches(last).count();
            let trim = matches!(last, '.' | ',' | ':' | ';' | '!' | '?' | '"' | '\'' | '*' | '_' | '~')
                || (last == ')' && unbalanced('('))
                || (last == ']' && unbalanced('['));
            if !trim {
                break;
            }
            end -= last.len_utf8();
        }
        if end > SCHEME.len() {
            return Some((at, end));
        }
    }
    None
}

#[cfg(test)]
#[path = "markdown_tests.rs"]
mod tests;
