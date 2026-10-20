use super::*;
use git2::Oid;

#[test]
fn fnv_vectors() {
    assert_eq!(name_hash(b""), 0x811c_9dc5);
    assert_eq!(name_hash(b"a"), 0xe40c_292c);
    assert_eq!(name_hash(b"main"), 0xea90_e208);
    assert_eq!(name_hash(b"feature/login"), 0x28ac_1333);
    assert_eq!(slot_of(b"search", 14), 13);
    assert_eq!(slot_of(b"topic", 14), 4);
    assert_eq!(slot_of(b"fix-1", 14), 4);
    assert_eq!((slot_of(b"", 14), slot_of(b"a", 14), slot_of(b"feature/login", 14)), (9, 12, 7));
    assert_eq!(slot_of("café".as_bytes(), 14), (name_hash("café".as_bytes()) % 14) as u8, "hashed as UTF-8");
}

#[test]
fn branch_key_strips_the_ref_and_the_remote() {
    assert_eq!(branch_key("refs/heads/a/b", &[]), Some("a/b"));
    assert_eq!(branch_key("refs/remotes/origin/a/b", &["origin"]), Some("a/b"));
    assert_eq!(branch_key("refs/remotes/my/fork/x", &["my", "my/fork"]), Some("x"), "the longest remote");
    assert_eq!(branch_key("refs/remotes/zz/x", &["origin"]), Some("x"), "no matching remote: the first segment");
    assert_eq!(branch_key("refs/remotes/origin/HEAD", &["origin"]), None);
    assert_eq!(branch_key("refs/tags/v1", &["origin"]), None);
    assert_eq!(branch_key("refs/notes/commits", &[]), None);
    assert_ne!(branch_key("refs/heads/Feature", &[]), branch_key("refs/heads/feature", &[]), "case is kept");
    assert_eq!(branch_key("refs/remotes/origin/feature", &["origin"]), branch_key("refs/heads/feature", &[]));
}

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn owners(names: &[&str], neighbours: &[(u32, u32)], mainline: Option<&str>) -> Owners {
    Owners {
        list: names.iter().map(|n| Owner::Branch((*n).to_owned())).collect(),
        mainline: mainline.map(str::to_owned),
        neighbours: neighbours.to_vec(),
        ..Owners::default()
    }
}

#[test]
fn the_mainline_takes_the_accent() {
    let o = owners(&["main", "topic"], &[(0, 1)], Some("main"));
    assert_eq!(resolve(&o, &Colouring::default()), [Swatch::Main, Swatch::Slot(4)]);
}

#[test]
fn neighbours_never_share_a_slot() {
    let side_by_side = owners(&["topic", "fix-1"], &[(0, 1)], None);
    assert_eq!(resolve(&side_by_side, &Colouring::default()), [Swatch::Slot(4), Swatch::Slot(5)]);
    let apart = owners(&["topic", "fix-1"], &[], None);
    assert_eq!(resolve(&apart, &Colouring::default()), [Swatch::Slot(4), Swatch::Slot(4)]);
}

#[test]
fn a_pin_holds_and_its_neighbour_moves() {
    let o = owners(&["topic", "fix-1"], &[(0, 1)], None);
    let c = Colouring { pins: [("fix-1".to_owned(), 4)].into_iter().collect(), ..Colouring::default() };
    assert_eq!(resolve(&o, &c), [Swatch::Slot(5), Swatch::Slot(4)]);
    assert_eq!(c.slot_for("fix-1"), 4);
    assert_eq!(c.slot_for("search"), 13);
}

#[test]
fn an_owner_with_every_slot_taken_keeps_its_own() {
    let mut names: Vec<String> = (0..14).map(|i| format!("n{i}")).collect();
    names.push("search".to_owned());
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let neighbours: Vec<(u32, u32)> = (0..14).map(|i| (i, 14)).collect();
    let o = owners(&refs, &neighbours, None);
    let c = Colouring { pins: (0..14u8).map(|i| (format!("n{i}"), i)).collect(), ..Colouring::default() };
    let got = resolve(&o, &c);
    assert_eq!(got[..14], (0..14).map(Swatch::Slot).collect::<Vec<_>>()[..]);
    assert_eq!(got[14], Swatch::Slot(13), "search's own slot");
}

#[test]
fn unnamed_lines_hash_their_commit_and_resolving_is_stable() {
    let mut o = owners(&["topic"], &[], None);
    o.list.push(Owner::Unnamed(oid(7)));
    let c = Colouring::default();
    let first = resolve(&o, &c);
    assert_eq!(first[1], Swatch::Slot(slot_of(oid(7).as_bytes(), 14)));
    assert_eq!(resolve(&o, &c), first);
}

#[test]
fn a_person_is_coloured_by_their_email() {
    let want = slot_of(b"ada@example.com", 14);
    assert_eq!(person_slot("ada@example.com", "Ada", 14), want);
    assert_eq!(person_slot("  Ada@Example.COM ", "Someone else", 14), want, "case and spaces don't count");
    assert_eq!(person_slot("", " Ada Lovelace ", 14), slot_of(b"Ada Lovelace", 14), "no email: the name");
    assert_eq!(person_slot("   ", "Ada Lovelace", 14), slot_of(b"Ada Lovelace", 14));
    assert!(person_slot("grace@example.com", "Grace", COLOUR_BLIND_SLOTS) < COLOUR_BLIND_SLOTS);
}

#[test]
fn the_colour_blind_count_stays_below_eight_and_pins_wrap() {
    let names: Vec<String> = (0..30).map(|i| format!("b{i}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let pairs: Vec<(u32, u32)> = (0..29).map(|i| (i, i + 1)).collect();
    let o = owners(&refs, &pairs, Some("b0"));
    let c = Colouring { slots: COLOUR_BLIND_SLOTS, pins: [("b5".to_owned(), 11)].into_iter().collect() };
    let swatches = resolve(&o, &c);
    assert!(swatches.iter().all(|s| matches!(s, Swatch::Main) || matches!(s, Swatch::Slot(n) if *n < 8)), "{swatches:?}");
    assert_eq!(swatches[5], Swatch::Slot(3), "a pin of 11 shows as 11 % 8");
    assert_eq!(c.slot_for("b5"), 3);
}
