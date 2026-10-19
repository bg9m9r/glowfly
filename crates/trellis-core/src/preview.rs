//! What the file history's File view can show rendered rather than as text, chosen by the
//! file's extension: Markdown files (parsed by `crate::markdown` when drawn) and images (decoded
//! by `crate::preview_image`, which the binary diff shares).

/// The kinds of file the File view can preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewKind {
    Markdown,
    Image,
}

const MARKDOWN_EXTS: [&str; 5] = ["md", "markdown", "mdown", "mkd", "mkdn"];

/// What `path` (repo-relative, '/'-separated) previews as, by its extension (ASCII case
/// ignored); `None` for text only. A name that is only an extension (".md") has none.
pub fn kind(path: &str) -> Option<PreviewKind> {
    if crate::preview_image::is_image_path(path) {
        return Some(PreviewKind::Image);
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    let (stem, ext) = name.rsplit_once('.')?;
    (!stem.is_empty() && MARKDOWN_EXTS.iter().any(|e| e.eq_ignore_ascii_case(ext))).then_some(PreviewKind::Markdown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_goes_by_the_extension_ignoring_ascii_case() {
        for p in ["README.md", "docs/Guide.MARKDOWN", "a.mkd", "b.mdown", "c.mkdn", "README.MD"] {
            assert_eq!(kind(p), Some(PreviewKind::Markdown), "{p}");
        }
        for p in ["logo.PNG", "p.jpeg", "q.JPG", "a.webp", "x.gif", "y.bmp"] {
            assert_eq!(kind(p), Some(PreviewKind::Image), "{p}");
        }
        for p in ["icon.svg", "Makefile", "notes.txt", "dir.md/file", ".md", "a/.png", "md"] {
            assert_eq!(kind(p), None, "{p}");
        }
    }
}
