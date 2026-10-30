//! Markdown parsed for descriptions and comments (`markdown.rs`).

use super::*;

fn plain(text: &str) -> Inline {
    Inline { text: text.to_owned(), style: InlineStyle::default(), link: None }
}

fn styled(text: &str, style: InlineStyle) -> Inline {
    Inline { text: text.to_owned(), style, link: None }
}

fn linked(text: &str, url: &str) -> Inline {
    Inline { text: text.to_owned(), style: InlineStyle::default(), link: Some(url.to_owned()) }
}

const STRONG: InlineStyle = InlineStyle { strong: true, emphasis: false, strike: false, code: false, image: false };
const EM: InlineStyle = InlineStyle { strong: false, emphasis: true, strike: false, code: false, image: false };

fn blocks(src: &str) -> Vec<Block> {
    parse(src).blocks
}

fn para(src: &str) -> Vec<Inline> {
    match blocks(src).as_slice() {
        [Block::Paragraph(p)] => p.clone(),
        other => panic!("expected one paragraph, got {other:?}"),
    }
}

#[test]
fn headings_one_to_six_keep_their_styled_text() {
    let b = blocks("# One\n## Two **bold**\n### Three\n#### Four\n##### Five\n###### Six\n");
    let levels: Vec<u8> = b.iter().map(|b| if let Block::Heading { level, .. } = b { *level } else { 0 }).collect();
    assert_eq!(levels, [1, 2, 3, 4, 5, 6]);
    assert_eq!(b[1], Block::Heading { level: 2, text: vec![plain("Two "), styled("bold", STRONG)] });
    assert_eq!(blocks("Setext\n===\n"), [Block::Heading { level: 1, text: vec![plain("Setext")] }]);
}

#[test]
fn breaks_inside_a_paragraph() {
    assert_eq!(para("one\ntwo"), [plain("one two")]);
    assert_eq!(para("one  \ntwo"), [plain("one\ntwo")]);
    assert_eq!(para("one\\\ntwo"), [plain("one\ntwo")]);
}

#[test]
fn inline_styles_and_merging() {
    let code = InlineStyle { code: true, ..InlineStyle::default() };
    let strike = InlineStyle { strike: true, ..InlineStyle::default() };
    let both = InlineStyle { strong: true, emphasis: true, ..InlineStyle::default() };
    assert_eq!(
        para("a **b** *c* ~~d~~ `e` ***f***"),
        [
            plain("a "),
            styled("b", STRONG),
            plain(" "),
            styled("c", EM),
            plain(" "),
            styled("d", strike),
            plain(" "),
            styled("e", code),
            plain(" "),
            styled("f", both)
        ]
    );
    // Entities and escapes split the text into several events, merged back into one run.
    assert_eq!(para("fish &amp; chips \\* done"), [plain("fish & chips * done")]);
    assert_eq!(para("**a *b* c**"), [styled("a ", STRONG), styled("b", both), styled(" c", STRONG)]);
}

#[test]
fn the_space_or_punctuation_around_a_styled_run_stays_in_the_runs() {
    let code = InlineStyle { code: true, ..InlineStyle::default() };
    let strike = InlineStyle { strike: true, ..InlineStyle::default() };
    assert_eq!(
        para("Read *try* the **bold** and `code` then ~~old~~ [link](https://x.test) end"),
        [
            plain("Read "),
            styled("try", EM),
            plain(" the "),
            styled("bold", STRONG),
            plain(" and "),
            styled("code", code),
            plain(" then "),
            styled("old", strike),
            plain(" "),
            linked("link", "https://x.test"),
            plain(" end")
        ]
    );
    assert_eq!(
        para("*try*. **b**, `c`)"),
        [styled("try", EM), plain(". "), styled("b", STRONG), plain(", "), styled("c", code), plain(")")]
    );
    assert_eq!(para("a _try_ b"), [plain("a "), styled("try", EM), plain(" b")]);
    // A marker with a space on its inner side closes nothing: the text stays as written.
    assert_eq!(para("x **a **b"), [plain("x **a **b")]);
}

#[test]
fn links_carry_their_url_and_style() {
    assert_eq!(
        para("see [the docs](https://example.com/docs) now"),
        [plain("see "), linked("the docs", "https://example.com/docs"), plain(" now")]
    );
    assert_eq!(para("[ref][r]\n\n[r]: https://example.com/r"), [linked("ref", "https://example.com/r")]);
    assert_eq!(para("<https://example.com>"), [linked("https://example.com", "https://example.com")]);
    let strong_link = Inline { text: "bold".into(), style: STRONG, link: Some("a.md".into()) };
    assert_eq!(para("[**bold**](a.md) x"), [strong_link, plain(" x")]);
}

#[test]
fn code_blocks_and_front_matter() {
    assert_eq!(
        blocks("```rust ignore\nfn main() {}\n  x\n```\n"),
        [Block::Code { lang: Some("rust".into()), text: "fn main() {}\n  x".into() }]
    );
    assert_eq!(blocks("```\nplain\n```\n"), [Block::Code { lang: None, text: "plain".into() }]);
    assert_eq!(
        blocks("text\n\n    indented\n    more\n"),
        [Block::Paragraph(vec![plain("text")]), Block::Code { lang: None, text: "indented\nmore".into() }]
    );
    assert_eq!(
        blocks("---\ntitle: Hi\n---\n\n# Body\n"),
        [Block::Code { lang: Some("yaml".into()), text: "title: Hi".into() }, Block::Heading { level: 1, text: vec![plain("Body")] }]
    );
}

fn list(src: &str) -> (Option<u64>, Vec<Item>) {
    match blocks(src).as_slice() {
        [Block::List { start, items }] => (*start, items.clone()),
        other => panic!("expected one list, got {other:?}"),
    }
}

fn item(text: &str) -> Item {
    Item { task: None, blocks: vec![Block::Paragraph(vec![plain(text)])] }
}

#[test]
fn tight_and_loose_lists_hold_paragraphs() {
    assert_eq!(list("- a\n- b\n"), (None, vec![item("a"), item("b")]));
    assert_eq!(list("- a\n\n- b\n"), (None, vec![item("a"), item("b")]));
    assert_eq!(list("3. three\n4. four\n"), (Some(3), vec![item("three"), item("four")]));
}

#[test]
fn nested_lists_and_tasks() {
    let (_, items) = list("- a\n  - b\n  - c\n- d\n");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].blocks, [Block::Paragraph(vec![plain("a")]), Block::List { start: None, items: vec![item("b"), item("c")] }]);
    let (_, tasks) = list("- [ ] todo\n- [x] done\n- plain\n");
    let marks: Vec<_> = tasks.iter().map(|i| i.task).collect();
    assert_eq!(marks, [Some(false), Some(true), None]);
    assert_eq!(tasks[1].blocks, [Block::Paragraph(vec![plain("done")])]);
}

#[test]
fn quotes_rules_and_tables() {
    assert_eq!(
        blocks("> - x\n>\n> > inner\n"),
        [Block::Quote(vec![
            Block::List { start: None, items: vec![item("x")] },
            Block::Quote(vec![Block::Paragraph(vec![plain("inner")])])
        ])]
    );
    assert_eq!(blocks("a\n\n---\n\nb\n"), [Block::Paragraph(vec![plain("a")]), Block::Rule, Block::Paragraph(vec![plain("b")])]);
    let t = blocks("| A | **B** |\n|---|:-:|\n| 1 | `2` |\n| 3 | 4 |\n");
    let code = InlineStyle { code: true, ..InlineStyle::default() };
    assert_eq!(
        t,
        [Block::Table {
            head: vec![vec![plain("A")], vec![styled("B", STRONG)]],
            rows: vec![vec![vec![plain("1")], vec![styled("2", code)]], vec![vec![plain("3")], vec![plain("4")]]],
        }]
    );
}

#[test]
fn html_is_dropped_and_images_become_alt_text_links() {
    assert_eq!(blocks("<div>\nhidden\n</div>\n\nshown\n"), [Block::Paragraph(vec![plain("shown")])]);
    assert_eq!(para("a<br>b"), [plain("ab")]);
    let image = InlineStyle { image: true, ..InlineStyle::default() };
    let alt = |text: &str, url: &str| Inline { text: text.into(), style: image, link: Some(url.into()) };
    assert_eq!(para("![a logo](https://x.test/a.png)"), [alt("a logo", "https://x.test/a.png")]);
    assert_eq!(para("![](a.png) after"), [alt("image", "a.png"), plain(" after")]);
    // An image inside a link keeps the link's target.
    assert_eq!(para("[![badge](b.svg)](https://ci.test)"), [alt("badge", "https://ci.test")]);
}

#[test]
fn bare_https_addresses_become_links() {
    assert_eq!(
        para("See https://example.com/a?b=1&amp;c=2. Then"),
        [plain("See "), linked("https://example.com/a?b=1&c=2", "https://example.com/a?b=1&c=2"), plain(". Then")]
    );
    assert_eq!(
        para("(https://example.com/wiki/A_(b)) end"),
        [plain("("), linked("https://example.com/wiki/A_(b)", "https://example.com/wiki/A_(b)"), plain(") end")]
    );
    let bold = Inline { text: "https://x.test".into(), style: STRONG, link: Some("https://x.test".into()) };
    assert_eq!(para("**https://x.test**"), [bold]);
    let code = InlineStyle { code: true, ..InlineStyle::default() };
    assert_eq!(para("`https://x.test`"), [styled("https://x.test", code)]);
    for text in ["http://x.test", "xhttps://x.test", "https:// x", "just https://"] {
        assert!(para(text).iter().all(|r| r.link.is_none()), "{text}");
    }
    let (_, items) = list(
        "- one https://a.test
- [two](https://b.test)
",
    );
    assert_eq!(items[0].blocks, [Block::Paragraph(vec![plain("one "), linked("https://a.test", "https://a.test")])]);
}

#[test]
fn empty_and_non_ascii_sources() {
    assert!(blocks("").is_empty());
    assert!(blocks("\n\n  \n").is_empty());
    assert_eq!(para("café — 日本語"), [plain("café — 日本語")]);
}

#[test]
fn deep_nesting_is_flattened_not_recursed() {
    let src = ">".repeat(5000) + " deep";
    let doc = parse(&src);
    let mut depth = 0;
    let mut b = &doc.blocks;
    while let [Block::Quote(inner)] = b.as_slice() {
        depth += 1;
        b = inner;
    }
    assert!(depth <= 24, "{depth}");
    assert_eq!(b.as_slice(), [Block::Paragraph(vec![plain("deep")])]);
}

#[test]
fn parse_capped_refuses_long_sources() {
    let at = "a".repeat(MARKDOWN_MAX_BYTES);
    assert!(parse_capped(&at).is_some());
    assert!(parse_capped(&format!("{at}b")).is_none());
}
