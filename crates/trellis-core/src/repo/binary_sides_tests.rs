//! A binary diff's sides: sizes from blobs and working files, images decoded for image paths, and
//! the added, deleted, renamed, oversized and corrupt cases.

use super::*;
use crate::testutil::*;
use image::{ImageFormat, Rgba, RgbaImage};
use std::path::PathBuf;

fn png(w: u32, h: u32) -> Vec<u8> {
    let img = RgbaImage::from_pixel(w, h, Rgba([10, 120, 200, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png).unwrap();
    out.into_inner()
}

fn put(dir: &Path, file: &str, bytes: &[u8]) {
    std::fs::write(dir.join(file), bytes).unwrap();
}

fn head(dir: &Path) -> git2::Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

/// A repository with a 4x3 logo.png committed.
fn with_logo(name: &str) -> (PathBuf, Vec<u8>) {
    let dir = scratch_repo(name);
    let bytes = png(4, 3);
    put(&dir, "logo.png", &bytes);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "logo"]);
    (dir, bytes)
}

fn dims(s: &Option<BinarySide>) -> Option<(u32, u32)> {
    s.as_ref().and_then(|s| s.image.as_ref()).map(|i| (i.width, i.height))
}

fn size(s: &Option<BinarySide>) -> Option<u64> {
    s.as_ref().map(|s| s.size)
}

#[test]
fn a_changed_png_has_both_sides_decoded() {
    let (dir, old) = with_logo("binary-sides-changed");
    let new = png(6, 5);
    put(&dir, "logo.png", &new);
    let s = binary_sides(&dir, &DiffTarget::Unstaged("logo.png".into())).unwrap();
    assert_eq!((size(&s.old), size(&s.new)), (Some(old.len() as u64), Some(new.len() as u64)));
    assert_eq!((dims(&s.old), dims(&s.new)), (Some((4, 3)), Some((6, 5))));

    git(&dir, &["add", "logo.png"]);
    let s = binary_sides(&dir, &DiffTarget::Staged("logo.png".into())).unwrap();
    assert_eq!((size(&s.old), size(&s.new)), (Some(old.len() as u64), Some(new.len() as u64)));
    assert_eq!((dims(&s.old), dims(&s.new)), (Some((4, 3)), Some((6, 5))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn added_deleted_and_untracked_sides() {
    let (dir, bytes) = with_logo("binary-sides-added");
    let s = binary_sides(&dir, &DiffTarget::Commit(head(&dir), "logo.png".into())).unwrap();
    assert!(s.old.is_none(), "added in this commit");
    assert_eq!((size(&s.new), dims(&s.new)), (Some(bytes.len() as u64), Some((4, 3))));

    git(&dir, &["rm", "-q", "logo.png"]);
    git(&dir, &["commit", "-q", "-m", "gone"]);
    let s = binary_sides(&dir, &DiffTarget::Commit(head(&dir), "logo.png".into())).unwrap();
    assert!(s.new.is_none(), "deleted in this commit");
    assert_eq!(dims(&s.old), Some((4, 3)));

    let fresh = png(2, 2);
    put(&dir, "icon.png", &fresh);
    let s = binary_sides(&dir, &DiffTarget::Unstaged("icon.png".into())).unwrap();
    assert!(s.old.is_none(), "untracked");
    assert_eq!((size(&s.new), dims(&s.new)), (Some(fresh.len() as u64), Some((2, 2))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn other_binaries_and_corrupt_images_keep_their_sizes() {
    let dir = scratch_repo("binary-sides-other");
    put(&dir, "data.bin", b"\0\x01\x02 binary \0 data");
    put(&dir, "logo.png", b"\x89PNG\r\n\x1a\n\0\0\0 not really a png");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    let more: &[u8] = b"\0\x01\x02 more binary \0 data here";
    put(&dir, "data.bin", more);
    put(&dir, "logo.png", b"\x89PNG\r\n\x1a\n\0\0\0 still not a png, longer");
    let s = binary_sides(&dir, &DiffTarget::Unstaged("data.bin".into())).unwrap();
    assert_eq!((size(&s.old), size(&s.new)), (Some(17), Some(more.len() as u64)));
    assert!(dims(&s.old).is_none() && dims(&s.new).is_none());
    let s = binary_sides(&dir, &DiffTarget::Unstaged("logo.png".into())).unwrap();
    assert!(size(&s.old).is_some() && size(&s.new).is_some());
    assert!(dims(&s.old).is_none() && dims(&s.new).is_none(), "undecodable");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn large_files_are_not_decoded_and_renames_use_the_old_path() {
    let (dir, old) = with_logo("binary-sides-limit");
    let new = png(6, 5);
    put(&dir, "logo.png", &new);
    let s = binary_sides_with(&dir, &DiffTarget::Unstaged("logo.png".into()), 8).unwrap();
    assert_eq!((size(&s.old), size(&s.new)), (Some(old.len() as u64), Some(new.len() as u64)));
    assert!(dims(&s.old).is_none() && dims(&s.new).is_none(), "over the limit");

    git(&dir, &["checkout", "-q", "--", "logo.png"]);
    git(&dir, &["mv", "logo.png", "brand.png"]);
    git(&dir, &["commit", "-q", "-m", "rename"]);
    let s = binary_sides(&dir, &DiffTarget::Commit(head(&dir), "brand.png".into())).unwrap();
    assert_eq!((size(&s.old), size(&s.new)), (Some(old.len() as u64), Some(old.len() as u64)));
    assert_eq!((dims(&s.old), dims(&s.new)), (Some((4, 3)), Some((4, 3))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_gif_side_decodes_too() {
    let dir = scratch_repo("binary-sides-gif");
    let img = RgbaImage::from_pixel(5, 4, Rgba([200, 40, 40, 255]));
    let mut gif = std::io::Cursor::new(Vec::new());
    img.write_to(&mut gif, ImageFormat::Gif).unwrap();
    put(&dir, "anim.gif", gif.get_ref());
    let s = binary_sides(&dir, &DiffTarget::Unstaged("anim.gif".into())).unwrap();
    let image = s.new.as_ref().and_then(|s| s.image.as_ref()).expect("the GIF decodes");
    assert_eq!((image.width, image.height, image.format), (5, 4, "GIF"));
    let _ = std::fs::remove_dir_all(&dir);
}
