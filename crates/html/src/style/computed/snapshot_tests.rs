//! Temporary Debug snapshot of `Computed` for the monolith split (refactor plan §2.2).
//!
//! The corpus and the recorded digests live outside the repository, in
//! `<workspace>/target/computed-snapshot/` (`corpus.txt`: one `key<TAB>value`
//! declaration or `*<TAB>style block` per line; `expected.txt`: one digest per
//! corpus line). The first run records `expected.txt`, later runs compare.
//! Run: `cargo test -p kamin-html --lib computed_debug_snapshot -- --ignored`.
//! Delete together with the other temporary refactor checks (phase 4).

use std::hash::{Hash, Hasher};

use super::Computed;
use crate::style::css::parse_decls;

fn digest(c: &Computed) -> u64 {
    let mut h = std::hash::DefaultHasher::new();
    format!("{c:?}").hash(&mut h);
    h.finish()
}

#[test]
#[ignore = "refactor snapshot: needs target/computed-snapshot/corpus.txt"]
fn computed_debug_snapshot() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/computed-snapshot");
    let corpus = std::fs::read_to_string(dir.join("corpus.txt")).expect("corpus.txt");
    let mut got = Vec::new();
    for line in corpus.lines() {
        let Some((key, value)) = line.split_once('\t') else {
            continue;
        };
        let mut c = Computed::default();
        if key == "*" {
            c.apply_decls_with_vars(&parse_decls(value), &Default::default());
        } else {
            c.apply_one(key, value);
        }
        got.push((line, digest(&c)));
    }
    let expected = dir.join("expected.txt");
    let Ok(old) = std::fs::read_to_string(&expected) else {
        let text: String = got.iter().map(|(_, d)| format!("{d:016x}\n")).collect();
        std::fs::write(&expected, text).expect("write expected.txt");
        eprintln!("recorded {} digests", got.len());
        return;
    };
    let old: Vec<&str> = old.lines().collect();
    assert_eq!(old.len(), got.len(), "corpus changed");
    let bad: Vec<&str> = got
        .iter()
        .zip(old)
        .filter(|((_, d), o)| format!("{d:016x}") != *o)
        .map(|((l, _), _)| *l)
        .collect();
    assert!(bad.is_empty(), "{} declarations differ, first: {:?}", bad.len(), &bad[..bad.len().min(30)]);
}
