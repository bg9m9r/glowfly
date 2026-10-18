//! Azure DevOps keeps work item descriptions and comments as HTML. Trellis shows them as plain
//! text: paragraphs and breaks as new lines, list items as "- ", links as "text (url)", images
//! as "[image: alt]", entities decoded, scripts and styles dropped. And plain text typed in the
//! Create dialog goes back as escaped HTML. A small scanner, not a full HTML parser.

/// Tags that end the current line, opening or closing (an empty line stays as it is).
const BLOCKS: &[&str] = &["div", "tr", "pre", "blockquote", "ul", "ol", "table", "li"];
/// Tags that stand apart from what follows them: a blank line after them.
const PARAGRAPHS: &[&str] = &["p", "h1", "h2", "h3", "h4", "h5", "h6"];

/// One tag: its lowercase name, whether it closes, and its attribute text.
struct Tag<'a> {
    name: String,
    closing: bool,
    attrs: &'a str,
}

fn parse_tag(inner: &str) -> Tag<'_> {
    let inner = inner.trim();
    let (closing, rest) = match inner.strip_prefix('/') {
        Some(r) => (true, r.trim_start()),
        None => (false, inner),
    };
    let end = rest.find(|c: char| c.is_whitespace() || c == '/' || c == '>').unwrap_or(rest.len());
    Tag { name: rest[..end].to_ascii_lowercase(), closing, attrs: &rest[end..] }
}

/// The value of attribute `name` (quoted or not), entities decoded.
fn attr(attrs: &str, name: &str) -> Option<String> {
    let lower = attrs.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find(name) {
        let at = from + i;
        from = at + name.len();
        let before_ok = at == 0 || lower.as_bytes()[at - 1].is_ascii_whitespace();
        let rest = attrs[from..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let rest = rest[1..].trim_start();
        let value = match rest.chars().next() {
            Some(q @ ('"' | '\'')) => rest[1..].split(q).next().unwrap_or_default(),
            _ => rest.split(|c: char| c.is_whitespace() || c == '>').next().unwrap_or_default(),
        };
        return Some(decode(value));
    }
    None
}

/// Decodes the named entities HTML text commonly has and numeric references; an unknown
/// entity stays as it is.
fn decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest[1..].find(';').map(|e| e + 1).filter(|e| *e <= 12);
        let decoded = end.and_then(|e| entity(&rest[1..e]).map(|c| (c, e)));
        match decoded {
            Some((c, e)) => {
                out.push(c);
                rest = &rest[e + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    let c = match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" | "#39" => '\'',
        "nbsp" => ' ',
        _ => {
            let num = name.strip_prefix('#')?;
            let code = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => num.parse().ok()?,
            };
            return char::from_u32(code).filter(|c| *c != '\0');
        }
    };
    Some(c)
}

/// Collects text, collapsing whitespace outside `<pre>`.
struct Out {
    lines: Vec<String>,
    line: String,
    pre: usize,
}

impl Out {
    fn text(&mut self, t: &str) {
        if self.pre > 0 {
            let mut parts = t.split('\n');
            if let Some(first) = parts.next() {
                self.line.push_str(first);
            }
            for p in parts {
                self.hard();
                self.line.push_str(p);
            }
            return;
        }
        for (i, word) in t.split(|c: char| c.is_whitespace()).enumerate() {
            if i > 0 && !self.line.ends_with(' ') && !self.line.is_empty() {
                self.line.push(' ');
            }
            self.line.push_str(word);
        }
    }

    /// A line break (`<br>`): an empty line counts.
    fn hard(&mut self) {
        self.lines.push(std::mem::take(&mut self.line));
    }

    /// The end of a block: the line ends unless nothing is on it yet.
    fn soft(&mut self) {
        if !self.line.trim().is_empty() {
            self.hard();
        }
    }

    /// The end of a paragraph: a blank line after it.
    fn blank(&mut self) {
        self.soft();
        if self.lines.last().is_some_and(|l| !l.trim().is_empty()) {
            self.lines.push(String::new());
        }
    }

    fn finish(mut self) -> String {
        self.hard();
        let mut out: Vec<String> = Vec::new();
        for l in self.lines {
            let l = l.trim().to_owned();
            if l.is_empty() && out.last().is_none_or(|p| p.is_empty()) {
                continue;
            }
            out.push(l);
        }
        out.join("\n").trim().to_owned()
    }
}

/// `html` as plain text.
pub(super) fn to_text(html: &str) -> String {
    let mut out = Out { lines: Vec::new(), line: String::new(), pre: 0 };
    let mut link: Option<(String, usize)> = None;
    let mut rest = html;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            out.text(&decode(rest));
            break;
        };
        out.text(&decode(&rest[..lt]));
        rest = &rest[lt..];
        if let Some(after) = rest.strip_prefix("<!--") {
            rest = after.find("-->").map_or("", |e| &after[e + 3..]);
            continue;
        }
        let Some(gt) = rest.find('>') else {
            out.text(&decode(rest));
            break;
        };
        let tag = parse_tag(&rest[1..gt]);
        rest = &rest[gt + 1..];
        if !tag.closing && (tag.name == "script" || tag.name == "style") {
            let close = format!("</{}", tag.name);
            rest = rest.to_ascii_lowercase().find(&close).map_or("", |e| {
                let after = &rest[e..];
                after.find('>').map_or("", |g| &after[g + 1..])
            });
            continue;
        }
        let name = tag.name.as_str();
        if name == "br" {
            out.hard();
        } else if PARAGRAPHS.contains(&name) {
            if tag.closing { out.blank() } else { out.soft() }
        } else if BLOCKS.contains(&name) {
            out.soft();
        }
        match (tag.name.as_str(), tag.closing) {
            ("pre", false) => out.pre += 1,
            ("pre", true) => out.pre = out.pre.saturating_sub(1),
            ("li", false) => out.line.push_str("- "),
            ("a", false) => link = attr(tag.attrs, "href").map(|h| (h, out.line.len())),
            ("a", true) => {
                if let Some((href, start)) = link.take() {
                    let text = out.line.get(start..).unwrap_or_default().trim().to_owned();
                    let web = href.starts_with("https://") || href.starts_with("http://");
                    if web && text != href {
                        if text.is_empty() {
                            out.line.push_str(&href);
                        } else {
                            out.line.push_str(&format!(" ({href})"));
                        }
                    }
                }
            }
            ("img", _) => match attr(tag.attrs, "alt").filter(|a| !a.trim().is_empty()) {
                Some(alt) => out.text(&format!("[image: {}]", alt.trim())),
                None => out.text("[image]"),
            },
            _ => {}
        }
    }
    out.finish()
}

/// Plain `text` as HTML: escaped, each blank-line-separated paragraph in a `<div>`, with
/// `<br>` between its lines and an empty `<div><br></div>` line between paragraphs.
pub(super) fn from_text(text: &str) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    let text = text.replace("\r\n", "\n");
    let mut paras: Vec<Vec<&str>> = vec![Vec::new()];
    for line in text.lines() {
        if line.trim().is_empty() {
            if paras.last().is_some_and(|p| !p.is_empty()) {
                paras.push(Vec::new());
            }
        } else if let Some(p) = paras.last_mut() {
            p.push(line);
        }
    }
    paras
        .into_iter()
        .filter(|p| !p.is_empty())
        .map(|p| format!("<div>{}</div>", p.iter().map(|l| esc(l)).collect::<Vec<_>>().join("<br>")))
        .collect::<Vec<_>>()
        .join("<div><br></div>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_become_lines() {
        assert_eq!(to_text("<div>One</div><div>Two<br>Three</div><p>Four</p>"), "One\nTwo\nThree\nFour");
        assert_eq!(to_text("<ul><li>a</li><li>b <b>bold</b></li></ul>after"), "- a\n- b bold\nafter");
        assert_eq!(to_text("<h2>Title</h2>text"), "Title\n\ntext");
        assert_eq!(to_text("<p>One</p><p>Two</p>"), "One\n\nTwo");
        assert_eq!(to_text("<div>a<br></div><div><br></div><div>b</div>"), "a\n\nb");
    }

    #[test]
    fn links_and_images() {
        assert_eq!(to_text(r#"See <a href="https://x">t</a>."#), "See t (https://x).");
        assert_eq!(to_text(r#"<a href="https://x">https://x</a>"#), "https://x");
        assert_eq!(to_text(r#"<a href="mailto:a@b">mail</a>"#), "mail");
        assert_eq!(to_text(r#"<a href='https://x?a=1&amp;b=2'></a>"#), "https://x?a=1&b=2");
        assert_eq!(to_text(r#"<img src="x.png" alt="a">"#), "[image: a]");
        assert_eq!(to_text(r#"<img src="x.png">"#), "[image]");
    }

    #[test]
    fn entities_decode() {
        assert_eq!(to_text("a &amp; b &lt;c&gt; it&#39;s &#x263A; x&nbsp;y &quot;q&quot; &apos;"), "a & b <c> it's \u{263a} x y \"q\" '");
        assert_eq!(to_text("&foo; &#65;"), "&foo; A");
        assert_eq!(to_text("AT&T"), "AT&T");
    }

    #[test]
    fn scripts_styles_and_comments_are_dropped() {
        let html = "<style>p { color: red }</style>a<!-- hidden -->b<script>alert(1)</script>c<SCRIPT>x</SCRIPT>d";
        assert_eq!(to_text(html), "abcd");
    }

    #[test]
    fn whitespace_collapses_and_blank_lines_are_capped() {
        assert_eq!(to_text("  a \n\n  b\t c  "), "a b c");
        assert_eq!(to_text("<div>a</div><br><br><br><div>b</div>"), "a\n\nb");
        assert_eq!(to_text("<pre>x  y\n  z</pre>"), "x  y\nz");
        assert_eq!(to_text(""), "");
    }

    #[test]
    fn plain_text_round_trips() {
        let s = "First line\nsecond line\n\nNext paragraph";
        assert_eq!(from_text(s), "<div>First line<br>second line</div><div><br></div><div>Next paragraph</div>");
        assert_eq!(to_text(&from_text(s)), s);
        assert_eq!(from_text("a < b & \"c\" > d"), "<div>a &lt; b &amp; &quot;c&quot; &gt; d</div>");
        assert_eq!(to_text(&from_text("a < b & \"c\" > d")), "a < b & \"c\" > d");
        assert_eq!(from_text("\n\n"), "");
    }
}
