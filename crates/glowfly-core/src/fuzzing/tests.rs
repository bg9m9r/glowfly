//! Every fuzz check over its seeds in `fuzz/seeds/<target>/` and a few hundred mutations of
//! them (byte flips, inserted marker and URL tokens, cuts, splices), so the checks build and
//! hold on stable without libFuzzer. `GLOWFLY_FUZZ_ROUNDS=<n>` mutates each seed `n` times
//! instead, for a longer local run.

use super::CHECKS;
use std::path::PathBuf;

/// Pieces the parsers look for, spliced into seeds.
const TOKENS: [&[u8]; 24] = [
    b"\n",
    b"\r\n",
    b"\t",
    b"\0",
    b" ",
    b"://",
    b"@",
    b":",
    b"/",
    b"%2F",
    b"%0A",
    b"?",
    b"#",
    b"\\",
    b"<<<<<<< ours",
    b"|||||||",
    b"=======",
    b">>>>>>> theirs",
    b"pick ",
    b"merge -C ",
    b"label ",
    b"diff --git a/x b/",
    b"\"\\303\\251",
    b"~1",
];

fn seeds(target: &str) -> Vec<Vec<u8>> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/seeds").join(target);
    let mut files: Vec<PathBuf> =
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())).flatten().map(|e| e.path()).collect();
    files.sort();
    let seeds: Vec<Vec<u8>> = files.iter().map(|f| std::fs::read(f).unwrap()).collect();
    assert!(!seeds.is_empty(), "no seeds in {}", dir.display());
    seeds
}

/// A small xorshift generator: the same mutations on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn mutate(rng: &mut Rng, input: &mut Vec<u8>, other: &[u8]) {
    for _ in 0..1 + rng.below(4) {
        let at = rng.below(input.len() + 1);
        match rng.below(5) {
            0 if at < input.len() => input[at] ^= 1 << rng.below(8),
            1 => {
                let token = TOKENS[rng.below(TOKENS.len())];
                input.splice(at..at, token.iter().copied());
            }
            2 if at < input.len() => {
                let end = (at + 1 + rng.below(16)).min(input.len());
                input.drain(at..end);
            }
            3 => {
                let from = rng.below(other.len());
                let piece = &other[from..(from + 1 + rng.below(32)).min(other.len())];
                input.splice(at..at, piece.iter().copied());
            }
            _ => input.insert(at, rng.next() as u8),
        }
    }
}

#[test]
fn every_check_holds_on_its_seeds_and_their_mutations() {
    let rounds: usize = std::env::var("GLOWFLY_FUZZ_ROUNDS").ok().and_then(|r| r.parse().ok()).unwrap_or(300);
    for (target, check) in CHECKS {
        let seeds = seeds(target);
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        for (i, seed) in seeds.iter().enumerate() {
            check(seed);
            let other = &seeds[(i + 1) % seeds.len()];
            for _ in 0..rounds {
                let mut input = seed.clone();
                mutate(&mut rng, &mut input, other);
                let result = std::panic::catch_unwind(|| check(&input));
                assert!(result.is_ok(), "{target} failed on {:?}", String::from_utf8_lossy(&input));
            }
        }
    }
}

#[test]
fn every_fuzz_target_has_a_check_and_seeds() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/fuzz_targets");
    let mut targets: Vec<String> =
        std::fs::read_dir(&dir).unwrap().flatten().filter_map(|e| e.file_name().to_str()?.strip_suffix(".rs").map(str::to_owned)).collect();
    targets.sort();
    let checks: Vec<&str> = CHECKS.iter().map(|(name, _)| *name).collect();
    assert_eq!(targets, checks);
}
