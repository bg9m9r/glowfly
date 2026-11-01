//! Which glowfly-core functions do I/O, worked out from core's own sources rather than a hand
//! list, so a new core I/O function is caught without anyone listing it. Every function body in
//! core's non-test code is read (`core_fns.rs`): a function does I/O when its body reaches outside
//! work directly (a `DENIED` family or the repository, in its signature too) or calls a core
//! function that does, until nothing changes. Calls are followed by path, bare name and
//! `Type::`/`Self::` associated function; a method call `.f(` counts when `f` does I/O on a core
//! trait, on the type the method is on, or on a core type the function names. A call from a
//! checked crate that resolves to no core function counts as I/O when its module has any, unless
//! `PURE_FUNCTIONS` (`rules.rs`) clears it.

#[path = "core_fns.rs"]
mod core_fns;

use super::direct_hits;
use super::paths::{call_paths, imports};
use super::rules::PURE_FUNCTIONS;
use crate::lexer::bare_lines;
use crate::views::crate_sources;
use core_fns::{Body, functions};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

type ModPath = Vec<String>;

/// What a call in core names: a free function by its full path, or an associated function of a
/// type (`Type::f`, `Self::f`).
enum Target {
    Path(ModPath),
    Method(String, String),
    /// `.f(`, with the types it could be on: the method's own type and the types the body names.
    Dot(String, BTreeSet<String>),
}

/// glowfly-core's functions and which of them do I/O.
pub struct Core {
    top: BTreeSet<String>,
    modules: BTreeSet<ModPath>,
    fns: BTreeMap<ModPath, bool>,
    methods: BTreeMap<(String, String), bool>,
    /// Methods by the core trait they implement or define.
    trait_methods: BTreeMap<(String, String), bool>,
    /// The names of inherent methods (in an `impl Type` block).
    inherent: BTreeSet<String>,
    /// Why each function that does I/O does: the outside work it reaches, or the call that does.
    reasons: BTreeMap<String, String>,
    /// What each module re-exports (`pub use`): (name bound, or `*` for a glob; target path).
    exports: BTreeMap<ModPath, Vec<(String, ModPath)>>,
}

/// A call or import in a checked crate that reaches a core function: the line (1-based), its path
/// under core (`repo::load_diff`) and whether it does I/O.
#[derive(Debug, PartialEq)]
pub struct CoreCall {
    pub line: usize,
    pub path: String,
    pub io: bool,
    /// Why it does I/O, as a chain of calls ending in the outside work.
    pub why: String,
}

/// The module path of the file at `rel` (under `src/`; `lib.rs` is the root), with the files a
/// `#[path]` moved mapped.
fn module_of(rel: &str, moved: &BTreeMap<String, ModPath>) -> ModPath {
    if let Some(m) = moved.get(rel) {
        return m.clone();
    }
    let stem = rel.trim_end_matches(".rs").trim_end_matches("/mod");
    if stem == "lib" { Vec::new() } else { stem.split('/').map(str::to_owned).collect() }
}

/// The files a `#[path = "…"]` brings in under another module's name, as (file, module path).
fn moved_files(sources: &[(String, String)]) -> BTreeMap<String, ModPath> {
    let mut moved = BTreeMap::new();
    for (rel, text) in sources {
        let raw: Vec<&str> = text.lines().collect();
        for (i, l) in raw.iter().enumerate() {
            let Some(file) = l.trim().strip_prefix("#[path = \"").and_then(|p| p.strip_suffix("\"]")) else { continue };
            let Some(name) = raw[i + 1..].iter().find_map(|l| l.trim().rsplit_once("mod ").and_then(|(_, n)| n.strip_suffix(';'))) else {
                continue;
            };
            let dir = rel.rsplit_once('/').map_or("", |(d, _)| d);
            let target = if dir.is_empty() { file.to_owned() } else { format!("{dir}/{file}") };
            moved.insert(target, module_of(rel, &BTreeMap::new()).into_iter().chain([name.trim().to_owned()]).collect());
        }
    }
    moved
}

impl Core {
    pub fn read(core_src: &Path) -> Core {
        let sources: Vec<(String, String)> = crate_sources(core_src).into_iter().filter(|s| !s.2).map(|(r, t, _)| (r, t)).collect();
        let lib = sources.iter().find(|(r, _)| r == "lib.rs").map(|(_, t)| bare_lines(t)).expect("glowfly-core's lib.rs");
        let top: BTreeSet<String> = lib
            .iter()
            .filter_map(|l| l.trim().trim_start_matches("pub ").strip_prefix("mod ")?.strip_suffix(';').map(str::to_owned))
            .collect();
        assert!(top.len() > 50, "found glowfly-core's modules");
        let moved = moved_files(&sources);
        let mut core = Core {
            top,
            modules: BTreeSet::new(),
            fns: BTreeMap::new(),
            methods: BTreeMap::new(),
            trait_methods: BTreeMap::new(),
            inherent: BTreeSet::new(),
            reasons: BTreeMap::new(),
            exports: BTreeMap::new(),
        };
        // First every module and function body, then what each body calls.
        let mut files = Vec::new();
        for (rel, text) in &sources {
            let module = module_of(rel, &moved);
            let bare = bare_lines(text).join("\n");
            core.modules.insert(module.clone());
            let bodies = functions(&bare, &module, &mut core.modules);
            files.push((module, bare, bodies));
        }
        let mut read: Vec<(Body, Option<String>, Vec<Target>)> = Vec::new();
        for (module, bare, bodies) in files {
            let mut aliases: BTreeMap<String, ModPath> = BTreeMap::new();
            for import in imports(&bare) {
                let Some(path) = core.absolute(&import.path, &module) else { continue };
                if import.public {
                    core.exports.entry(module.clone()).or_default().push((import.name.clone(), path.clone()));
                }
                aliases.insert(import.name, path);
            }
            for body in bodies {
                let types: BTreeSet<String> = body
                    .text
                    .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .filter(|w| w.starts_with(|c: char| c.is_ascii_uppercase()))
                    .map(str::to_owned)
                    .chain(body.owner.clone())
                    .collect();
                let mut targets: Vec<Target> =
                    body.text.lines().flat_map(call_paths).filter_map(|p| core.target(&p, &body, &aliases)).collect();
                targets.extend(method_calls(&body.text).into_iter().map(|m| Target::Dot(m, types.clone())));
                let direct = direct_hits(&body.text).into_iter().next().map(|f| format!("`{}` ({})", f.pattern, f.what));
                read.push((body, direct, targets));
            }
        }
        for (body, direct, _) in &read {
            core.set(body, direct.clone());
        }
        loop {
            let mut changed = false;
            for (body, _, targets) in &read {
                if core.get(body) {
                    continue;
                }
                if let Some(via) = targets.iter().find_map(|t| core.target_why(t)) {
                    core.set(body, Some(via));
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        core
    }

    /// Records `b`, doing I/O for the reason given.
    fn set(&mut self, b: &Body, why: Option<String>) {
        let io = why.is_some();
        match &b.owner {
            Some(owner) => *self.methods.entry((owner.clone(), b.name.clone())).or_default() |= io,
            None => *self.fns.entry(b.path()).or_default() |= io,
        }
        match &b.of_trait {
            Some(tr) => *self.trait_methods.entry((tr.clone(), b.name.clone())).or_default() |= io,
            None if b.owner.is_some() => {
                self.inherent.insert(b.name.clone());
            }
            None => {}
        }
        if let Some(why) = why {
            if let Some(tr) = &b.of_trait {
                self.reasons.entry(format!("{tr}::{}", b.name)).or_insert(why.clone());
            }
            self.reasons.entry(b.shown()).or_insert(why);
        }
    }

    fn get(&self, b: &Body) -> bool {
        match &b.owner {
            Some(owner) => self.methods.get(&(owner.clone(), b.name.clone())).copied().unwrap_or(false),
            None => self.fns.get(&b.path()).copied().unwrap_or(false),
        }
    }

    /// `path`, written in a file of `module`, as a path from core's root; none for a path
    /// outside core.
    fn absolute(&self, path: &[String], module: &[String]) -> Option<ModPath> {
        let first = path.first()?;
        let mut base: ModPath = module.to_vec();
        let mut rest = path;
        match first.as_str() {
            "crate" => {
                base.clear();
                rest = &path[1..];
            }
            "self" => rest = &path[1..],
            "super" => {
                while rest.first().is_some_and(|s| s == "super") {
                    base.pop();
                    rest = &rest[1..];
                }
            }
            _ if self.modules.contains(&base.iter().chain([first]).cloned().collect::<ModPath>()) => {}
            _ => return None,
        }
        Some(base.into_iter().chain(rest.iter().cloned()).collect())
    }

    /// What the call `path` in `body` names, through its file's `aliases`.
    fn target(&self, path: &[String], body: &Body, aliases: &BTreeMap<String, ModPath>) -> Option<Target> {
        let (name, _) = path.split_last()?;
        if path.len() == 2 && path[0] == "Self" {
            return body.owner.clone().map(|o| Target::Method(o, name.clone()));
        }
        if let Some(alias) = aliases.get(&path[0]) {
            return Some(Target::Path(alias.iter().chain(&path[1..]).cloned().collect()));
        }
        if path.len() == 1 {
            return Some(Target::Path(body.module.iter().chain(path).cloned().collect()));
        }
        if path.len() == 2 && path[0].starts_with(|c: char| c.is_ascii_uppercase()) {
            return Some(Target::Method(path[0].clone(), name.clone()));
        }
        self.absolute(path, &body.module).map(Target::Path)
    }

    /// Why calling `t` does I/O (`repo::load -> ...`), if it does.
    fn target_why(&self, t: &Target) -> Option<String> {
        let on = |map: &BTreeMap<(String, String), bool>, ty: &str, name: &str| {
            map.get(&(ty.to_owned(), name.to_owned())).copied().unwrap_or(false)
        };
        let shown = match t {
            Target::Method(owner, name) => on(&self.methods, owner, name).then(|| format!("{owner}::{name}")),
            Target::Path(p) => self.resolve_path(p, 0).filter(|(_, io)| *io).map(|(p, _)| p.join("::")),
            Target::Dot(name, types) => {
                let ty = types.iter().find(|t| on(&self.methods, t, name));
                // Through a core trait: one the body names, or any when no inherent method has
                // the name (`service(p).pull_requests(…)` on a `&dyn Forge`).
                let tr = self
                    .trait_methods
                    .keys()
                    .map(|(t, _)| t)
                    .filter(|t| types.contains(*t) || !self.inherent.contains(name))
                    .find(|t| on(&self.trait_methods, t, name));
                ty.or(tr).map(|t| format!("{t}::{name}"))
            }
        }?;
        let why = self.reasons.get(&shown).cloned().unwrap_or_default();
        Some(format!("{shown} -> {why}"))
    }

    /// Whether the free function at `path` does I/O, following re-exports; none when no core
    /// function is there.
    fn resolve(&self, path: &[String], depth: usize) -> Option<bool> {
        self.resolve_path(path, depth).map(|(_, io)| io)
    }

    /// The free function `path` names (where it is defined, following re-exports) and whether
    /// it does I/O.
    fn resolve_path(&self, path: &[String], depth: usize) -> Option<(ModPath, bool)> {
        if let Some(io) = self.fns.get(path) {
            return Some((path.to_vec(), *io));
        }
        let (name, module) = path.split_last()?;
        if depth > 8 {
            return None;
        }
        let exports = self.exports.get(module)?;
        let named = exports.iter().filter(|(n, _)| n == name).find_map(|(_, target)| self.resolve_path(target, depth + 1));
        named.or_else(|| {
            exports.iter().filter(|(n, _)| n == "*").find_map(|(_, glob)| {
                let module = &glob[..glob.len() - 1];
                self.resolve_path(&module.iter().chain([name]).cloned().collect::<ModPath>(), depth + 1)
            })
        })
    }

    /// The calls and imports in `src` (another crate's file) that reach a core function, through
    /// `glowfly_core::` paths or a core module imported from there.
    pub fn calls(&self, src: &str) -> Vec<CoreCall> {
        let code = bare_lines(src);
        let mut aliases: BTreeMap<String, ModPath> = BTreeMap::new();
        let mut out = Vec::new();
        for import in imports(&code.join("\n")) {
            let Some(rest) = import.path.strip_prefix(&["glowfly_core".to_owned()][..]).filter(|r| !r.is_empty()) else { continue };
            if self.modules.contains(rest) {
                aliases.insert(import.name, rest.to_vec());
            } else if let Some(call) = self.function(rest, import.line) {
                out.push(call);
            }
        }
        for (i, l) in code.iter().enumerate() {
            for path in call_paths(l).into_iter().filter(|p| p.len() > 1) {
                let rest = match aliases.get(&path[0]) {
                    _ if path[0] == "glowfly_core" => path[1..].to_vec(),
                    Some(module) => module.iter().chain(&path[1..]).cloned().collect(),
                    None => continue,
                };
                out.extend(self.function(&rest, i + 1));
            }
        }
        out
    }

    /// `rest` (a path under core) as a call, if it names a function: a lowercase last segment in
    /// a core module.
    fn function(&self, rest: &[String], line: usize) -> Option<CoreCall> {
        let (name, module) = rest.split_last()?;
        let in_core = self.top.contains(module.first()?) && self.modules.contains(module);
        if !in_core || !name.starts_with(|c: char| c.is_ascii_lowercase()) || self.modules.contains(rest) {
            return None;
        }
        let path = rest.join("::");
        // Not found (made by a macro, or re-exported in a way this reader doesn't follow): I/O
        // when its module has any.
        let (io, why) = match self.resolve_path(rest, 0) {
            Some((at, io)) => (io, self.reasons.get(&at.join("::")).cloned().unwrap_or_default()),
            None => (self.fns.iter().any(|(p, io)| *io && p.starts_with(module)), "not found in core; its module does I/O".to_owned()),
        };
        Some(CoreCall { line, io: io && !PURE_FUNCTIONS.contains(&path.as_str()), path, why })
    }

    /// Whether the free function at `path` (`repo::load_diff`) does I/O, if core has it.
    pub fn is_io(&self, path: &str) -> Option<bool> {
        self.resolve(&path.split("::").map(str::to_owned).collect::<ModPath>(), 0)
    }
}

/// The methods `text` calls (`.f(`, `.f::<T>(`).
fn method_calls(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (k, _) in text.match_indices('.') {
        let name: String = text[k + 1..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        let next = text[k + 1 + name.len()..].trim_start();
        if !name.is_empty() && (next.starts_with('(') || next.starts_with("::<")) && !name.starts_with(|c: char| c.is_ascii_digit()) {
            out.push(name);
        }
    }
    out
}
