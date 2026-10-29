//! Tests only: a crate's `license` field (an SPDX expression, with the old `/` for OR), which
//! licences Glowfly takes from it, and which licence a licence file's text is.

/// A parsed SPDX licence expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    /// One licence, with its exception (`Apache-2.0 WITH LLVM-exception`) as one id.
    Id(String),
    /// Every part applies.
    And(Vec<Expr>),
    /// Any one part may be chosen.
    Or(Vec<Expr>),
}

/// The licences Glowfly prefers when a crate offers a choice, most preferred first: the
/// shortest notice obligations first. An id not listed comes last.
const PREFERENCE: &[&str] = &[
    "MIT",
    "ISC",
    "Zlib",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "BSL-1.0",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "Unicode-3.0",
    "CC0-1.0",
    "0BSD",
    "Unlicense",
];

fn rank(id: &str) -> usize {
    PREFERENCE.iter().position(|p| *p == id).unwrap_or(PREFERENCE.len())
}

/// Parses `text` (`MIT OR Apache-2.0`, `MIT/Apache-2.0`, `(MIT OR Apache-2.0) AND Unicode-3.0`).
pub fn parse(text: &str) -> Result<Expr, String> {
    let spaced = text.replace('(', " ( ").replace(')', " ) ").replace('/', " OR ");
    let tokens: Vec<&str> = spaced.split_whitespace().collect();
    let mut at = 0;
    let expr = or_expr(&tokens, &mut at)?;
    if at != tokens.len() {
        return Err(format!("{text}: unexpected {:?}", tokens[at]));
    }
    Ok(expr)
}

fn or_expr(t: &[&str], at: &mut usize) -> Result<Expr, String> {
    let mut parts = vec![and_expr(t, at)?];
    while t.get(*at) == Some(&"OR") {
        *at += 1;
        parts.push(and_expr(t, at)?);
    }
    Ok(if parts.len() == 1 { parts.remove(0) } else { Expr::Or(parts) })
}

fn and_expr(t: &[&str], at: &mut usize) -> Result<Expr, String> {
    let mut parts = vec![atom(t, at)?];
    while t.get(*at) == Some(&"AND") {
        *at += 1;
        parts.push(atom(t, at)?);
    }
    Ok(if parts.len() == 1 { parts.remove(0) } else { Expr::And(parts) })
}

fn atom(t: &[&str], at: &mut usize) -> Result<Expr, String> {
    match t.get(*at) {
        Some(&"(") => {
            *at += 1;
            let e = or_expr(t, at)?;
            if t.get(*at) != Some(&")") {
                return Err("missing )".into());
            }
            *at += 1;
            Ok(e)
        }
        Some(id) if !matches!(*id, ")" | "AND" | "OR" | "WITH") => {
            *at += 1;
            let mut id = (*id).to_owned();
            if t.get(*at) == Some(&"WITH") {
                let exception = t.get(*at + 1).ok_or("WITH without an exception")?;
                id = format!("{id} WITH {exception}");
                *at += 2;
            }
            Ok(Expr::Id(id))
        }
        other => Err(format!("unexpected {other:?}")),
    }
}

/// The licences to follow: all parts of an AND, and of an OR the option whose licences all
/// have a text (`has_text`) and rank best, or the best-ranked option when none has.
pub fn choose(expr: &Expr, has_text: &dyn Fn(&str) -> bool) -> Vec<String> {
    match expr {
        Expr::Id(id) => vec![id.clone()],
        Expr::And(parts) => parts.iter().flat_map(|p| choose(p, has_text)).collect(),
        Expr::Or(parts) => {
            let options: Vec<Vec<String>> = parts.iter().map(|p| choose(p, has_text)).collect();
            let worst = |o: &Vec<String>| o.iter().map(|id| rank(id)).max().unwrap_or(0);
            let with_text = options.iter().filter(|o| o.iter().all(|id| has_text(id))).min_by_key(|o| worst(o));
            with_text.or_else(|| options.iter().min_by_key(|o| worst(o))).cloned().unwrap_or_default()
        }
    }
}

/// Which licence a licence file's text is, from its wording; None for a text that is none of
/// these (a pointer to other files, a NOTICE, a mixed file).
pub fn classify(text: &str) -> Option<&'static str> {
    let t = words(text);
    let has = |s: &str| t.contains(s);
    let (mit, apache) = mit_apache(&t);
    if mit && apache {
        return None;
    }
    Some(if has("boost software license") {
        "BSL-1.0"
    } else if apache && has("llvm exceptions") {
        "Apache-2.0 WITH LLVM-exception"
    } else if apache {
        "Apache-2.0"
    } else if mit {
        "MIT"
    } else if has("unicode license v3") {
        "Unicode-3.0"
    } else if has("community data license agreement") && has("permissive") {
        "CDLA-Permissive-2.0"
    } else if has("sil open font license") {
        "OFL-1.1"
    } else if has("ubuntu font licence") {
        "Ubuntu-font-1.0"
    } else if has("this software is provided 'as-is'") {
        "Zlib"
    } else if has("this is free and unencumbered software released into the public domain") {
        "Unlicense"
    } else if has("cc0 1.0 universal") {
        "CC0-1.0"
    } else if has("permission to use, copy, modify, and/or distribute this software for any purpose")
        && has("copyright notice and this permission notice appear in all copies")
    {
        "ISC"
    } else if has("permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted")
    {
        "0BSD"
    } else if has("redistribution and use in source and binary forms") {
        if has("endorse or promote products") { "BSD-3-Clause" } else { "BSD-2-Clause" }
    } else {
        return None;
    })
}

/// A text's words, lower case, without comment leaders (a licence kept as a `//` or `#` block).
fn words(text: &str) -> String {
    let lines = text.lines().map(|l| l.trim_start().trim_start_matches(['/', '#', '*', ';']));
    lines.flat_map(str::split_whitespace).collect::<Vec<_>>().join(" ").to_lowercase()
}

fn mit_apache(words: &str) -> (bool, bool) {
    let mit = words.contains("permission is hereby granted, free of charge, to any person obtaining a copy of this software");
    (mit, words.contains("apache license") && words.contains("version 2.0, january 2004"))
}

/// Whether a file that `classify` can't name (one holding both texts) still has `id`'s full
/// text: MIT or Apache-2.0 in a combined licence file.
pub fn holds(text: &str, id: &str) -> bool {
    let (mit, apache) = mit_apache(&words(text));
    (id == "MIT" && mit) || (id == "Apache-2.0" && apache)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> Expr {
        Expr::Id(s.into())
    }

    #[test]
    fn parses_old_and_new_forms() {
        assert_eq!(parse("MIT").unwrap(), id("MIT"));
        assert_eq!(parse("MIT/Apache-2.0").unwrap(), Expr::Or(vec![id("MIT"), id("Apache-2.0")]));
        assert_eq!(parse("MIT / Apache-2.0").unwrap(), parse("MIT OR Apache-2.0").unwrap());
        assert_eq!(
            parse("(MIT OR Apache-2.0) AND Unicode-3.0").unwrap(),
            Expr::And(vec![Expr::Or(vec![id("MIT"), id("Apache-2.0")]), id("Unicode-3.0")])
        );
        assert_eq!(
            parse("Apache-2.0 WITH LLVM-exception OR MIT").unwrap(),
            Expr::Or(vec![id("Apache-2.0 WITH LLVM-exception"), id("MIT")])
        );
        assert!(parse("MIT OR").is_err());
        assert!(parse("(MIT").is_err());
    }

    #[test]
    fn chooses_the_preferred_option_that_has_a_text() {
        let all = |_: &str| true;
        let pick = |s: &str, f: &dyn Fn(&str) -> bool| choose(&parse(s).unwrap(), f);
        assert_eq!(pick("Apache-2.0 OR MIT", &all), ["MIT"]);
        assert_eq!(pick("Zlib OR Apache-2.0 OR MIT", &all), ["MIT"]);
        assert_eq!(pick("(MIT OR Apache-2.0) AND BSD-3-Clause", &all), ["MIT", "BSD-3-Clause"]);
        assert_eq!(pick("MIT OR Apache-2.0", &|id: &str| id == "Apache-2.0"), ["Apache-2.0"]);
        assert_eq!(pick("MIT OR Apache-2.0", &|_: &str| false), ["MIT"]);
        assert_eq!(pick("Apache-2.0 AND ISC", &all), ["Apache-2.0", "ISC"]);
    }

    #[test]
    fn classifies_licence_texts() {
        let mit = "Copyright (c) 2020 A\n\nPermission is hereby granted, free of charge, to any person obtaining a copy\nof this software";
        assert_eq!(classify(mit), Some("MIT"));
        assert_eq!(classify("Apache License\n Version 2.0, January 2004\n"), Some("Apache-2.0"));
        assert_eq!(
            classify("Boost Software License - Version 1.0\nPermission is hereby granted, free of charge, to any person"),
            Some("BSL-1.0")
        );
        let bsd3 = "Redistribution and use in source and binary forms ... used to endorse or promote products";
        assert_eq!(classify(bsd3), Some("BSD-3-Clause"));
        assert_eq!(classify("Redistribution and use in source and binary forms, with or without"), Some("BSD-2-Clause"));
        assert_eq!(classify("This software is provided 'as-is', without"), Some("Zlib"));
        let commented = "// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.";
        assert_eq!(classify(commented), Some("ISC"));
        assert_eq!(classify("See LICENSE-MIT and LICENSE-APACHE."), None);
        let both = format!(
            "{mit}

Apache License
 Version 2.0, January 2004"
        );
        assert_eq!(classify(&both), None);
        assert!(holds(&both, "MIT") && holds(&both, "Apache-2.0") && !holds(mit, "Apache-2.0"));
    }
}
