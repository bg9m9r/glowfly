//! Parsed Markdown bodies (pull request and issue descriptions and comments, and the file
//! history File view's preview), kept so each body is parsed and its code blocks highlighted
//! once, not every frame. Keyed by the text's hash; the text is kept too, so a hash collision
//! parses again instead of showing the wrong body.

use glowfly_core::highlight::{self, Span, SyntaxHint};
use glowfly_core::markdown::{self, Block, MdDoc};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// More bodies than this and the cache starts over.
const MAX_BODIES: usize = 256;
/// More text than this, all kept bodies together, and the cache starts over (a File view
/// preview can be up to `markdown::MARKDOWN_MAX_BYTES` on its own).
const MAX_BYTES: usize = 16 << 20;
/// The longest code block highlighted, in lines.
const MAX_BLOCK_LINES: usize = 300;
/// Lines highlighted per body, all code blocks together; later blocks stay plain.
const MAX_BODY_LINES: usize = 1_000;

/// One body, ready to draw.
pub(crate) struct Rendered {
    /// `None` when the text is too long to parse (`markdown::MARKDOWN_MAX_BYTES`): drawn plain.
    pub doc: Option<MdDoc>,
    /// Each code block's text and colors, in the order the blocks are drawn.
    pub code: Vec<CodeBlock>,
}

/// A code block's text with tabs expanded, and token spans per line when its language is known.
pub(crate) struct CodeBlock {
    pub text: String,
    pub spans: Option<Vec<Vec<Span>>>,
}

#[derive(Default)]
pub struct MarkdownCache {
    bodies: HashMap<u64, (String, Rc<Rendered>)>,
    /// The kept texts' lengths, summed.
    bytes: usize,
}

impl MarkdownCache {
    /// `text` parsed, from the cache or now.
    pub(crate) fn get(&mut self, text: &str) -> Rc<Rendered> {
        let mut h = DefaultHasher::new();
        text.hash(&mut h);
        let key = h.finish();
        if let Some((kept, r)) = self.bodies.get(&key)
            && kept == text
        {
            return r.clone();
        }
        if self.bodies.len() >= MAX_BODIES || self.bytes + text.len() > MAX_BYTES {
            self.bodies.clear();
            self.bytes = 0;
        }
        let r = Rc::new(render(text));
        if let Some((old, _)) = self.bodies.insert(key, (text.to_owned(), r.clone())) {
            self.bytes -= old.len();
        }
        self.bytes += text.len();
        r
    }
}

fn render(text: &str) -> Rendered {
    let doc = markdown::parse_capped(text);
    let mut code = Vec::new();
    if let Some(d) = &doc {
        let mut budget = MAX_BODY_LINES;
        collect(&d.blocks, &mut code, &mut budget);
    }
    Rendered { doc, code }
}

/// The code blocks in `blocks`, depth first: the order `markdown::show` draws them in.
fn collect(blocks: &[Block], out: &mut Vec<CodeBlock>, budget: &mut usize) {
    for b in blocks {
        match b {
            Block::Code { lang, text } => out.push(code_block(lang.as_deref(), text, budget)),
            Block::List { items, .. } => items.iter().for_each(|i| collect(&i.blocks, out, budget)),
            Block::Quote(inner) => collect(inner, out, budget),
            Block::Heading { .. } | Block::Paragraph(_) | Block::Table { .. } | Block::Rule => {}
        }
    }
}

fn code_block(lang: Option<&str>, text: &str, budget: &mut usize) -> CodeBlock {
    let text = text.replace('\t', "    ");
    let lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let fits = lines.len() <= MAX_BLOCK_LINES && lines.len() <= *budget;
    let spans = match lang {
        Some(lang) if fits => {
            *budget -= lines.len();
            highlight::highlight_file(SyntaxHint { path: "", language: Some(lang) }, &lines)
        }
        _ => None,
    };
    CodeBlock { text, spans }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_is_parsed_once_and_its_code_highlighted() {
        let mut cache = MarkdownCache::default();
        let src = "# Hi\n\n```rust\nfn main() {}\n```\n\n- ```\n  plain\n  ```\n";
        let a = cache.get(src);
        let b = cache.get(src);
        assert!(Rc::ptr_eq(&a, &b), "cached");
        assert_eq!(a.code.len(), 2);
        assert!(a.code[0].spans.as_ref().is_some_and(|s| s.len() == 1), "rust is highlighted");
        assert!(a.code[1].spans.is_none(), "no language");
        assert!(!Rc::ptr_eq(&a, &cache.get("other")));
    }

    #[test]
    fn long_text_is_left_plain_and_long_code_uncolored() {
        let mut cache = MarkdownCache::default();
        assert!(cache.get(&"a".repeat(markdown::MARKDOWN_MAX_BYTES + 1)).doc.is_none());
        let long = format!("```rust\n{}```\n", "let x = 1;\n".repeat(MAX_BLOCK_LINES + 1));
        assert!(cache.get(&long).code[0].spans.is_none());
    }

    #[test]
    fn a_large_body_over_the_byte_budget_starts_the_cache_over() {
        let mut cache = MarkdownCache::default();
        let first = cache.get("# First");
        let big = "b".repeat(MAX_BYTES);
        let r = cache.get(&big);
        assert!(r.doc.is_none(), "over the parse limit, drawn plain");
        assert_eq!(cache.bytes, MAX_BYTES);
        let again = cache.get("# First");
        assert!(!Rc::ptr_eq(&first, &again), "parsed again after the cache started over");
        assert!(again.doc.as_ref().is_some_and(|d| d.blocks.len() == 1));
        assert!(Rc::ptr_eq(&again, &cache.get("# First")));
    }

    #[test]
    fn tabs_are_expanded() {
        let mut cache = MarkdownCache::default();
        assert_eq!(cache.get("```\n\tx\n```\n").code[0].text, "    x");
    }
}
