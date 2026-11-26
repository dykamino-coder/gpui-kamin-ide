use super::*;
use kamin_heap::retain::{Block, Graph};

#[test]
fn probe_metadata_cannot_root_a_block_but_adjacent_roots_survive() {
    let live = [0usize];
    let orphan = [0usize];
    let words = [
        live.as_ptr() as usize,
        orphan.as_ptr() as usize,
        live.as_ptr() as usize,
    ];
    let start = words.as_ptr() as usize;
    let roots = vec![region("test statics", start, start + size_of_val(&words)).unwrap()];
    let roots = excluding(roots, start + WORD..start + 2 * WORD);
    assert_eq!(roots.len(), 2);
    let blocks = [live.as_ptr(), orphan.as_ptr()]
        .map(|pointer| Block {
            address: pointer as usize,
            bytes: WORD,
        })
        .to_vec();
    let graph = Graph::build(blocks, roots, &super::super::Process);
    let orphan_id = graph
        .blocks
        .iter()
        .position(|block| block.address == orphan.as_ptr() as usize)
        .unwrap();
    let live_id = graph
        .blocks
        .iter()
        .position(|block| block.address == live.as_ptr() as usize)
        .unwrap();
    assert_eq!(graph.parent[orphan_id], graph.unrooted());
    assert_ne!(graph.parent[live_id], graph.unrooted());
}

#[test]
fn excludes_overlaps_without_expanding_or_losing_unrelated_regions() {
    for excluded in [0..64, 0..24, 24..64, 16..48, 0..16, 48..64] {
        let original = region("test", 16, 48).unwrap();
        let roots = excluding(vec![original], excluded.clone());
        let remaining: Vec<_> = roots
            .iter()
            .flat_map(|root| (root.start..root.start + root.bytes).step_by(WORD))
            .collect();
        let expected: Vec<_> = (16..48)
            .step_by(WORD)
            .filter(|word| !excluded.contains(word))
            .collect();
        assert_eq!(remaining, expected);
    }
}
