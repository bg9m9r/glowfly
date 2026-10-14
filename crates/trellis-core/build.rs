//! Builds the syntax set used for diff highlighting and dumps it to `OUT_DIR/syntaxes.packdump`,
//! which `highlight::syntaxes` embeds with `include_bytes!`.
//!
//! The set is the curated two-face set (a superset of syntect's defaults) plus Trellis's own
//! grammars in `syntaxes/`. It is merged and dumped here, at build time, because building a
//! merged set at runtime takes about 0.2 s in a release build and over 1 s in a debug build,
//! while loading the finished dump takes under 1 ms.

use std::path::{Path, PathBuf};
use std::{env, fs};
use syntect::parsing::SyntaxDefinition;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=syntaxes");

    let mut builder = two_face::syntax::extra_newlines().into_builder();
    let mut files: Vec<PathBuf> = fs::read_dir("syntaxes")
        .expect("read the syntaxes folder")
        .map(|e| e.expect("list the syntaxes folder").path())
        .filter(|p| p.extension().is_some_and(|e| e == "sublime-syntax"))
        .collect();
    files.sort();
    for path in files {
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let stem = path.file_stem().and_then(|s| s.to_str());
        let def = SyntaxDefinition::load_from_str(&text, true, stem).unwrap_or_else(|e| panic!("load {}: {e}", path.display()));
        // A newer two-face could ship a grammar of the same name, which would silently shadow
        // ours (or be shadowed by it): make that a build error instead.
        if builder.syntaxes().iter().any(|s| s.name == def.name) {
            panic!("{}: a syntax named {:?} is already in the bundled set", path.display(), def.name);
        }
        builder.add(def);
    }
    let set = builder.build();
    let out = Path::new(&env::var_os("OUT_DIR").expect("cargo sets OUT_DIR")).join("syntaxes.packdump");
    syntect::dumps::dump_to_uncompressed_file(&set, &out).unwrap_or_else(|e| panic!("write {}: {e}", out.display()));
}
