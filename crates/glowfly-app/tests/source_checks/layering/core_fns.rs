//! Finding the function bodies of a core file for `core_io.rs`: each top-level `fn` or method
//! (`impl` or `trait`), with its module (inline `mod` blocks included), the type it is on and its
//! text. Braces are counted in code whose comments and literal text are blanked.

use std::collections::BTreeSet;

type ModPath = Vec<String>;

/// One function body: where it is, the type it is on (for a method) and the core trait it
/// implements (`impl Trait for Type`, or a trait's own default method), its name and its text
/// (the signature included).
pub struct Body {
    pub module: ModPath,
    pub owner: Option<String>,
    pub of_trait: Option<String>,
    pub name: String,
    pub text: String,
}

impl Body {
    /// The path of a free function: its module and name.
    pub fn path(&self) -> ModPath {
        self.module.iter().cloned().chain([self.name.clone()]).collect()
    }

    /// How a report names it: `repo::load` or `Type::method`.
    pub fn shown(&self) -> String {
        match &self.owner {
            Some(owner) => format!("{owner}::{}", self.name),
            None => self.path().join("::"),
        }
    }
}

/// What a `{` opens, from the text since the last `;`, `{` or `}`.
enum Block {
    Fn(String),
    /// An `impl` (its type, and the trait for `impl Trait for Type`) or a `trait` (its name
    /// twice).
    Impl(String, Option<String>),
    Mod(String),
    Other,
}

/// The last segment of the type path at the start of `text`, generics and `&` skipped.
fn type_name(text: &str) -> String {
    let text = text.trim_start();
    let text = if text.starts_with('<') { text.split_once('>').map_or("", |(_, t)| t) } else { text };
    let path: String =
        text.trim_start().trim_start_matches(['&', ' ']).chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':').collect();
    path.rsplit("::").next().unwrap_or("").to_owned()
}

fn block(header: &str) -> Block {
    let words: Vec<&str> = header.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).collect();
    let after = |kw: &str| words.iter().position(|w| *w == kw).and_then(|p| words.get(p + 1)).map(|w| (*w).to_owned());
    if let Some(name) = after("fn") {
        return Block::Fn(name);
    }
    if words.contains(&"impl") {
        let after_impl = header.split_once("impl").map_or("", |(_, t)| t);
        return match header.rsplit_once(" for ") {
            Some((_, ty)) => Block::Impl(type_name(ty), Some(type_name(after_impl))),
            None => Block::Impl(type_name(after_impl), None),
        };
    }
    if let Some(name) = after("trait") {
        return Block::Impl(name.clone(), Some(name));
    }
    match after("mod") {
        Some(name) => Block::Mod(name),
        None => Block::Other,
    }
}

/// The function bodies in `bare` (a file of `module`, comments and literal text blanked), with
/// nested functions and closures read as part of the function around them. Inline modules go
/// into `modules`.
pub fn functions(bare: &str, module: &[String], modules: &mut BTreeSet<ModPath>) -> Vec<Body> {
    let mut out: Vec<Body> = Vec::new();
    let mut stack: Vec<Block> = Vec::new();
    let mut header = String::new();
    // The body being read, and the depth its block opened at.
    let mut current: Option<(usize, usize)> = None;
    for c in bare.chars() {
        if let Some((k, _)) = current {
            out[k].text.push(c);
        }
        match c {
            '{' => {
                let b = block(&header);
                if current.is_none() {
                    let mods = stack.iter().filter_map(|b| if let Block::Mod(m) = b { Some(m.clone()) } else { None });
                    let here: ModPath = module.iter().cloned().chain(mods).collect();
                    match &b {
                        Block::Fn(name) => {
                            let on = stack
                                .iter()
                                .rev()
                                .find_map(|b| if let Block::Impl(t, tr) = b { Some((t.clone(), tr.clone())) } else { None });
                            let (owner, of_trait) = on.map_or((None, None), |(t, tr)| (Some(t), tr));
                            out.push(Body { module: here, owner, of_trait, name: name.clone(), text: format!("{header}{{") });
                            current = Some((out.len() - 1, stack.len()));
                        }
                        Block::Mod(name) => {
                            modules.insert(here.into_iter().chain([name.clone()]).collect());
                        }
                        _ => {}
                    }
                }
                header.clear();
                stack.push(b);
            }
            '}' => {
                header.clear();
                stack.pop();
                if current.is_some_and(|(_, depth)| depth == stack.len()) {
                    current = None;
                }
            }
            ';' => header.clear(),
            _ => header.push(c),
        }
    }
    out
}

#[test]
fn reads_function_bodies() {
    let src = "mod imp {\n    pub fn a() { b(); }\n}\nimpl<T: X> Trait for Foo<T> {\n    fn m(&self) { Self::n(); }\n}\n\
               fn top() -> u8 { let f = |x| { x }; 1 }\n";
    let mut modules = BTreeSet::new();
    let found: Vec<String> = functions(src, &["repo".to_owned()], &mut modules)
        .iter()
        .map(|b| format!("{}:{}:{}:{}", b.module.join("::"), b.owner.clone().unwrap_or_default(), b.name, b.text.trim()))
        .collect();
    assert_eq!(
        found,
        ["repo::imp::a:pub fn a() { b(); }", "repo:Foo:m:fn m(&self) { Self::n(); }", "repo::top:fn top() -> u8 { let f = |x| { x }; 1 }"]
    );
    assert!(modules.contains(&vec!["repo".to_owned(), "imp".to_owned()]));
}
