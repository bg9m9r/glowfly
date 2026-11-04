//! The SVG preview's render: size, scale, transparency, text in the bundled fonts, broken and
//! empty files, and hrefs that name files or URLs left out while `data:` images draw.

use super::*;
use crate::testutil::scratch_repo;

fn svg(body: &str) -> Vec<u8> {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" {body}</svg>"#).into_bytes()
}

/// The pixel at `x`, `y` of the render.
fn at(r: &SvgRender, x: usize, y: usize) -> [u8; 4] {
    let i = (y * r.image.shown[0] + x) * 4;
    r.image.rgba[i..i + 4].try_into().unwrap()
}

#[test]
fn a_small_icon_is_drawn_with_its_longer_side_at_shown_max() {
    let r = render(&svg(r##"width="24" height="12" viewBox="0 0 24 12"><rect width="24" height="12" fill="#ff0000"/>"##)).unwrap();
    assert_eq!((r.image.width, r.image.height, r.image.format), (24, 12, "SVG"));
    assert_eq!(r.image.shown, [SHOWN_MAX as usize, SHOWN_MAX as usize / 2]);
    assert_eq!(r.image.rgba.len(), r.image.shown[0] * r.image.shown[1] * 4);
    assert_eq!(at(&r, 10, 10), [255, 0, 0, 255]);
    assert!(!r.linked);
}

#[test]
fn a_huge_size_is_reported_but_drawn_small() {
    let r = render(&svg(r#"width="100000" height="50000"><rect width="10" height="10"/>"#)).unwrap();
    assert_eq!((r.image.width, r.image.height), (100000, 50000));
    assert_eq!(r.image.shown, [1024, 512]);
}

#[test]
fn an_svg_without_a_size_takes_one_from_its_drawing() {
    // No width, height or viewBox: usvg sizes it to reach the drawing's far edges from the
    // origin (a circle out to 90, 90), within its 100 x 100 default canvas.
    let r = render(&svg(r#"><circle cx="50" cy="50" r="40"/>"#)).unwrap();
    assert_eq!((r.image.width, r.image.height), (90, 90));
    assert_eq!(r.image.shown, [1024, 1024]);
}

#[test]
fn transparency_is_kept_unmultiplied() {
    let r = render(&svg(r##"width="10" height="10"><rect width="5" height="10" fill="#0000ff" fill-opacity="0.5"/>"##)).unwrap();
    let [red, green, blue, alpha] = at(&r, 100, 500);
    assert_eq!((red, green), (0, 0));
    assert!(blue >= 250, "unmultiplied, the blue stays full: {blue}");
    assert!((126..=129).contains(&alpha), "{alpha}");
    assert_eq!(at(&r, 900, 500)[3], 0, "the right half is empty");
}

#[test]
fn text_is_set_in_the_bundled_font() {
    let text =
        render(&svg(r#"width="40" height="20"><text x="2" y="16" font-size="16" font-family="Some Font Nobody Has">Hi</text>"#)).unwrap();
    assert!(text.image.rgba.chunks(4).any(|p| p[3] > 0), "the text is drawn");
    let none = render(&svg(r#"width="40" height="20">"#)).unwrap();
    assert!(none.image.rgba.chunks(4).all(|p| p[3] == 0));
}

#[test]
fn broken_and_empty_files_are_errors() {
    assert_eq!(render(b"<svg").unwrap_err(), SvgError::Unreadable);
    assert_eq!(render(b"not xml at all").unwrap_err(), SvgError::Unreadable);
    assert_eq!(render(&[0xff, 0xfe, 0x00]).unwrap_err(), SvgError::Unreadable);
    assert_eq!(render(b"").unwrap_err(), SvgError::Unreadable);
    assert_eq!(render(&svg(r#"width="0" height="10">"#)).unwrap_err(), SvgError::Unreadable);
}

/// A 1 x 1 opaque green PNG.
fn green_png() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 255, 0, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

#[test]
fn linked_files_and_urls_are_never_read() {
    // A real PNG next to the test's working folder, named by absolute path and by URL: neither
    // is drawn, and the render says files were left out.
    let dir = scratch_repo("svg-preview-linked");
    let file = dir.join("green.png");
    std::fs::write(&file, green_png()).unwrap();
    let path = file.to_string_lossy().replace('\\', "/");
    for href in [path.as_str(), "green.png", "https://example.invalid/green.png", "file:///etc/passwd"] {
        let body = format!(
            r#"width="10" height="10"><image width="10" height="10" xlink:href="{href}"/><image width="10" height="10" href="{href}"/>"#
        );
        let r = render(&svg(&body)).unwrap();
        assert!(r.linked, "{href}");
        assert!(r.image.rgba.chunks(4).all(|p| p[3] == 0), "{href} isn't drawn");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn data_images_inside_the_file_are_drawn() {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(green_png());
    let body =
        format!(r#"width="10" height="10"><image width="10" height="10" preserveAspectRatio="none" href="data:image/png;base64,{b64}"/>"#);
    let r = render(&svg(&body)).unwrap();
    assert!(!r.linked);
    assert_eq!(at(&r, 512, 512), [0, 255, 0, 255]);
}

#[test]
fn svg_paths_by_extension() {
    for p in ["a.svg", "dir/b.SVG", "icons/x.Svg"] {
        assert!(is_svg_path(p), "{p}");
    }
    for p in ["a.svg.txt", "svg", ".svg", "dir/.svg", "a.svgz", "dir.svg/readme"] {
        assert!(!is_svg_path(p), "{p}");
    }
}
