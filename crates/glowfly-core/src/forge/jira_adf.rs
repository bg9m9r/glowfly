//! Jira Cloud's rich text (the Atlassian Document Format, a JSON tree) to and from plain text.
//! Pure functions: descriptions and comments are shown as text, and new descriptions are sent
//! as paragraphs.

use serde_json::{Value, json};

/// Nodes deeper than this are left out, so a hostile answer can't exhaust the stack.
const MAX_DEPTH: usize = 32;

/// Node types that hold text inside a block.
const INLINE: &[&str] = &["text", "hardBreak", "mention", "emoji", "inlineCard", "status", "date"];

/// The text of a document: blocks separated by blank lines, list items on their own lines.
/// A string (Jira Data Center's descriptions) is returned as it is.
pub(super) fn to_text(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        return s.to_owned();
    }
    blocks(v, 0).join("\n\n").trim().to_owned()
}

fn kind(v: &Value) -> &str {
    v["type"].as_str().unwrap_or_default()
}

fn content(v: &Value) -> &[Value] {
    v["content"].as_array().map_or(&[], Vec::as_slice)
}

/// The blocks `v` holds, as text.
fn blocks(v: &Value, depth: usize) -> Vec<String> {
    if depth > MAX_DEPTH {
        return Vec::new();
    }
    let d = depth + 1;
    match kind(v) {
        "paragraph" | "heading" | "codeBlock" => vec![inline_of(v, d)],
        "rule" => vec!["---".to_owned()],
        "blockCard" | "embedCard" => vec![v["attrs"]["url"].as_str().unwrap_or_default().to_owned()],
        "bulletList" | "orderedList" => vec![list(v, d)],
        "table" => content(v).iter().map(|row| row_text(row, d)).collect(),
        t if INLINE.contains(&t) => vec![inline_of(&json!({ "content": [v] }), d)],
        // doc, blockquote, panel, expand and unknown nodes: their children.
        _ => children(content(v), d),
    }
}

/// Blocks of `nodes`, with runs of inline nodes gathered into one block.
fn children(nodes: &[Value], depth: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut run = String::new();
    for n in nodes {
        if INLINE.contains(&kind(n)) {
            inline(n, depth, &mut run);
        } else {
            if !run.is_empty() {
                out.push(std::mem::take(&mut run));
            }
            out.extend(blocks(n, depth));
        }
    }
    if !run.is_empty() {
        out.push(run);
    }
    out.retain(|b| !b.trim().is_empty());
    out
}

fn inline_of(v: &Value, depth: usize) -> String {
    let mut out = String::new();
    for n in content(v) {
        inline(n, depth, &mut out);
    }
    out
}

/// Appends the text of an inline node.
fn inline(v: &Value, depth: usize, out: &mut String) {
    if depth > MAX_DEPTH {
        return;
    }
    let attr = |name: &str| v["attrs"][name].as_str().unwrap_or_default();
    match kind(v) {
        "text" => out.push_str(v["text"].as_str().unwrap_or_default()),
        "hardBreak" => out.push('\n'),
        "mention" | "status" => out.push_str(attr("text")),
        "emoji" => out.push_str(if attr("text").is_empty() { attr("shortName") } else { attr("text") }),
        "inlineCard" => out.push_str(attr("url")),
        _ => {
            for n in content(v) {
                inline(n, depth + 1, out);
            }
        }
    }
}

/// A list's items, one per line ("- " or "1. "), nested lists indented under their item.
fn list(v: &Value, depth: usize) -> String {
    let ordered = kind(v) == "orderedList";
    let start = v["attrs"]["order"].as_u64().unwrap_or(1);
    let mut lines = Vec::new();
    for (i, item) in content(v).iter().enumerate() {
        let marker = if ordered { format!("{}. ", start + i as u64) } else { "- ".to_owned() };
        let text = children(content(item), depth + 1).join("\n");
        for (j, line) in text.lines().enumerate() {
            lines.push(if j == 0 { format!("{marker}{line}") } else { format!("  {line}") });
        }
        if text.is_empty() {
            lines.push(marker.trim_end().to_owned());
        }
    }
    lines.join("\n")
}

/// A table row: its cells' text, separated by " | ".
fn row_text(row: &Value, depth: usize) -> String {
    content(row).iter().map(|cell| children(content(cell), depth + 1).join(" ")).collect::<Vec<_>>().join(" | ")
}

/// Plain text as a document: paragraphs split on blank lines, the lines inside one joined by
/// hard breaks. Empty text is Null (the field is then left out).
pub(super) fn from_text(s: &str) -> Value {
    let text = s.replace("\r\n", "\n");
    let mut paragraphs: Vec<Vec<&str>> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in text.trim().lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                paragraphs.push(std::mem::take(&mut current));
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        paragraphs.push(current);
    }
    if paragraphs.is_empty() {
        return Value::Null;
    }
    let content: Vec<Value> = paragraphs
        .into_iter()
        .map(|lines| {
            let mut nodes = Vec::new();
            for (i, l) in lines.into_iter().enumerate() {
                if i > 0 {
                    nodes.push(json!({ "type": "hardBreak" }));
                }
                nodes.push(json!({ "type": "text", "text": l }));
            }
            json!({ "type": "paragraph", "content": nodes })
        })
        .collect();
    json!({ "type": "doc", "version": 1, "content": content })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(content: Value) -> Value {
        json!({ "type": "doc", "version": 1, "content": content })
    }

    fn p(text: &str) -> Value {
        json!({ "type": "paragraph", "content": [{ "type": "text", "text": text }] })
    }

    #[test]
    fn paragraphs_headings_and_breaks() {
        let v = doc(json!([
            { "type": "heading", "attrs": { "level": 2 }, "content": [{ "type": "text", "text": "Steps" }] },
            { "type": "paragraph", "content": [
                { "type": "text", "text": "Open the app" }, { "type": "hardBreak" }, { "type": "text", "text": "Click Login" }
            ] },
            { "type": "codeBlock", "content": [{ "type": "text", "text": "let x = 1;" }] },
            { "type": "rule" },
            p("Done")
        ]));
        assert_eq!(to_text(&v), "Steps\n\nOpen the app\nClick Login\n\nlet x = 1;\n\n---\n\nDone");
    }

    #[test]
    fn lists() {
        let item = |t: &str| json!({ "type": "listItem", "content": [p(t)] });
        let v = doc(json!([
            { "type": "bulletList", "content": [item("one"), item("two")] },
            { "type": "orderedList", "content": [item("first"), item("second")] }
        ]));
        assert_eq!(to_text(&v), "- one\n- two\n\n1. first\n2. second");
    }

    #[test]
    fn inline_nodes_and_unknown_nodes() {
        let v = doc(json!([
            { "type": "paragraph", "content": [
                { "type": "mention", "attrs": { "id": "1", "text": "@Ada" } },
                { "type": "text", "text": " " },
                { "type": "emoji", "attrs": { "shortName": ":smile:" } },
                { "type": "text", "text": " " },
                { "type": "emoji", "attrs": { "shortName": ":x:", "text": "X" } },
                { "type": "text", "text": " see " },
                { "type": "inlineCard", "attrs": { "url": "https://example.com/a" } },
                { "type": "text", "text": " " },
                { "type": "status", "attrs": { "text": "BLOCKED" } }
            ] },
            { "type": "panel", "content": [p("Note")] },
            { "type": "somethingNew", "content": [{ "type": "wrapper", "content": [{ "type": "text", "text": "deep" }] }] }
        ]));
        assert_eq!(to_text(&v), "@Ada :smile: X see https://example.com/a BLOCKED\n\nNote\n\ndeep");
    }

    #[test]
    fn strings_pass_through_and_depth_is_capped() {
        assert_eq!(to_text(&json!("Plain *wiki* text\n")), "Plain *wiki* text\n");
        assert_eq!(to_text(&Value::Null), "");
        let mut v = json!({ "type": "text", "text": "bottom" });
        for _ in 0..100 {
            v = json!({ "type": "blockquote", "content": [v] });
        }
        assert_eq!(to_text(&v), "", "nothing below the cap is read");
        let mut shallow = p("top");
        for _ in 0..5 {
            shallow = json!({ "type": "blockquote", "content": [shallow] });
        }
        assert_eq!(to_text(&shallow), "top");
    }

    #[test]
    fn plain_text_becomes_paragraphs() {
        assert_eq!(from_text("  \n "), Value::Null);
        let v = from_text("Line one\r\nLine two\n\n\nSecond");
        assert_eq!(v["type"], "doc");
        assert_eq!(v["version"], 1);
        let paras = v["content"].as_array().unwrap();
        assert_eq!(paras.len(), 2);
        assert_eq!(paras[0]["content"][1]["type"], "hardBreak");
        assert_eq!(paras[0]["content"][2]["text"], "Line two");
        let s = "First paragraph\nwith a second line\n\nAnother one";
        assert_eq!(to_text(&from_text(&format!("\n{s}\n"))), s);
    }
}
