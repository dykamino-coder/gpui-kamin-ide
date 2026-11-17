//! Граф на искусственной памяти: пути, доминаторы, вход недостижимого и циклы
//! `Rc`; плюс настоящий цикл `Rc` в куче теста.

use super::*;
use std::collections::BTreeMap;

/// Память теста: слова блоков и областей по их началу.
#[derive(Default)]
struct Fake(BTreeMap<usize, Vec<usize>>);

impl Memory for Fake {
    fn words(&self, address: usize, bytes: usize) -> &[usize] {
        let words = self.0.get(&address).map_or(&[][..], Vec::as_slice);
        &words[..words.len().min(bytes / size_of::<usize>())]
    }
}

const STACK: usize = 0x10;

/// Блок `index` по адресу `0x1000 * (index + 1)` со словами `words`.
fn at(index: usize) -> usize {
    0x1000 * (index + 1)
}

/// Граф из блоков (слова каждого) и одного стека со словами `stack`.
fn graph(blocks: &[Vec<usize>], stack: &[usize]) -> Graph {
    let mut memory = Fake::default();
    let mut list = Vec::new();
    for (index, words) in blocks.iter().enumerate() {
        let mut words = words.clone();
        words.resize(words.len().max(4), 0);
        list.push(Block {
            address: at(index),
            bytes: words.len() * size_of::<usize>(),
        });
        memory.0.insert(at(index), words);
    }
    memory.0.insert(STACK, stack.to_vec());
    let regions = vec![Region {
        name: "stack main".into(),
        start: STACK,
        bytes: stack.len() * size_of::<usize>(),
    }];
    Graph::build(list, regions, &memory)
}

/// Узлы пути от узла к корню по кратчайшим путям.
fn path(graph: &Graph, mut node: u32) -> Vec<u32> {
    let mut out = vec![node];
    while node != graph.root() {
        node = graph.parent[node as usize];
        out.push(node);
    }
    out
}

#[test]
fn a_chain_from_the_stack_is_dominated_link_by_link() {
    // stack -> 0 -> 1 -> 2
    let g = graph(&[vec![at(1)], vec![0, at(2)], vec![]], &[7, at(0)]);
    assert_eq!(path(&g, 2), vec![2, 1, 0, g.region(0), g.root()]);
    assert_eq!(g.parent_offset[1], 0);
    assert_eq!(g.parent_offset[2], 8);
    assert_eq!(g.parent_offset[0], 8);
    assert_eq!(g.idom[2], 1);
    assert_eq!(g.idom[1], 0);
    assert_eq!(g.idom[0], g.region(0));
    assert_eq!(g.retained[0], 3 * 32);
    assert_eq!(g.retained[g.root() as usize], 3 * 32);
    assert!(g.flags.iter().all(|flag| *flag == 0));
}

#[test]
fn a_block_held_twice_is_retained_by_their_common_dominator() {
    // stack -> 0, stack -> 1, 0 -> 2, 1 -> 2
    let g = graph(&[vec![at(2)], vec![at(2)], vec![]], &[at(0), at(1)]);
    assert_eq!(g.idom[2], g.region(0));
    assert_eq!(g.retained[0], 32);
    assert_eq!(g.retained[1], 32);
    assert_eq!(g.retained[g.region(0) as usize], 96);
}

#[test]
fn interior_words_are_edges_and_one_past_the_end_is_not() {
    // 0 points into the middle of 1 and one past the end of 2.
    let g = graph(&[vec![at(1) + 24, at(2) + 32], vec![], vec![]], &[at(0)]);
    let targets: Vec<u32> = g.edges(0).map(|(target, _)| target).collect();
    assert_eq!(targets, vec![1]);
    assert_eq!(g.block_at(at(1) + 31), Some(1));
    assert_eq!(g.block_at(at(1) + 32), None);
    assert_eq!(g.block_at(at(0) - 1), None);
}

#[test]
fn repeated_words_give_one_edge_with_the_first_offset() {
    let g = graph(&[vec![0, at(1), at(1), at(1) + 8], vec![]], &[at(0)]);
    let edges: Vec<(u32, u32)> = g.edges(0).collect();
    assert_eq!(edges, vec![(1, 8)]);
}

#[test]
fn an_rc_cycle_nobody_holds_enters_through_its_largest_block() {
    // 0 <-> 1 (cycle, 1 is larger), 2 holds 3 and nobody holds 2; stack holds 4.
    let g = graph(
        &[vec![1, 1, at(1)], vec![1, 1, at(0), 0, 0, 0], vec![at(3)], vec![], vec![]],
        &[at(4)],
    );
    assert_eq!(g.flags[1], FLAG_CYCLE);
    assert_eq!(g.flags[0], 0);
    assert_eq!(g.flags[2], FLAG_UNHELD);
    assert_eq!(g.idom[1], g.unrooted());
    assert_eq!(g.idom[0], 1);
    assert_eq!(g.retained[1], 48 + 32);
    assert_eq!(g.idom[3], 2);
    assert_eq!(g.idom[4], g.region(0));
    assert_eq!(path(&g, 0), vec![0, 1, g.unrooted(), g.root()]);
}

#[test]
fn a_cycle_held_by_an_unheld_block_is_not_a_cycle_entry() {
    // 2 -> 0 <-> 1, nobody holds 2.
    let g = graph(&[vec![at(1)], vec![at(0)], vec![at(0)]], &[]);
    assert_eq!(g.flags[2], FLAG_UNHELD);
    assert_eq!(g.flags[0], 0);
    assert_eq!(g.flags[1], 0);
    assert_eq!(g.retained[2], 96);
}

#[test]
fn a_block_held_by_the_stack_and_an_unheld_block_takes_the_stack_path() {
    // stack -> 0, 1 -> 0, nobody holds 1.
    let g = graph(&[vec![], vec![at(0)]], &[at(0)]);
    assert_eq!(g.parent[0], g.region(0));
    assert_eq!(g.idom[0], g.root());
    assert_eq!(g.flags[1], FLAG_UNHELD);
}

#[test]
fn overlapping_and_empty_blocks_are_dropped() {
    let memory = Fake::default();
    let blocks = vec![
        Block { address: 0x2000, bytes: 64 },
        Block { address: 0x2010, bytes: 8 },
        Block { address: 0x3000, bytes: 0 },
        Block { address: 0x1000, bytes: 8 },
    ];
    let g = Graph::build(blocks, Vec::new(), &memory);
    assert_eq!(
        g.blocks,
        vec![
            Block { address: 0x1000, bytes: 8 },
            Block { address: 0x2000, bytes: 64 }
        ]
    );
    assert_eq!(g.flags[0], FLAG_UNHELD);
    assert_eq!(g.retained[g.root() as usize], 72);
}

#[test]
fn the_file_starts_with_the_magic_and_the_header() {
    let g = graph(&[vec![at(1)], vec![]], &[at(0)]);
    let mut out = Vec::new();
    g.write(&mut out, "{\"x\":1}").unwrap();
    assert!(out.starts_with(write::MAGIC));
    let rest = &out[write::MAGIC.len()..];
    assert_eq!(u32::from_le_bytes(rest[..4].try_into().unwrap()), 7);
    assert_eq!(&rest[4..11], b"{\"x\":1}");
    let counts = &rest[11..43];
    let blocks = u64::from_le_bytes(counts[..8].try_into().unwrap());
    let nodes = u64::from_le_bytes(counts[8..16].try_into().unwrap());
    assert_eq!((blocks, nodes), (2, 5));
}

/// Настоящая память процесса теста: блоки живы, пока тест держит их `Rc`.
struct Process;

impl Memory for Process {
    fn words(&self, address: usize, bytes: usize) -> &[usize] {
        // SAFETY: тест передаёт только блоки живых `Rc`, выровненные по слову.
        unsafe { std::slice::from_raw_parts(address as *const usize, bytes / size_of::<usize>()) }
    }
}

#[test]
fn a_real_rc_cycle_is_found_in_the_heap() {
    use std::{cell::RefCell, rc::Rc};
    struct Link(RefCell<Option<Rc<Link>>>, [u64; 4]);
    let first = Rc::new(Link(RefCell::new(None), [0; 4]));
    let second = Rc::new(Link(RefCell::new(Some(first.clone())), [0; 4]));
    *first.0.borrow_mut() = Some(second.clone());
    // Блок Rc — RcBox: два счётчика и значение.
    let header = 2 * size_of::<usize>();
    let block = |rc: &Rc<Link>| Block {
        address: Rc::as_ptr(rc) as usize - header,
        bytes: header + size_of::<Link>(),
    };
    let g = Graph::build(vec![block(&first), block(&second)], Vec::new(), &Process);
    let cycle = g.flags.iter().filter(|flag| **flag == FLAG_CYCLE).count();
    assert_eq!(cycle, 1);
    assert_eq!(g.retained[g.unrooted() as usize], 2 * block(&first).bytes as u64);
    assert_eq!(second.1[0], 0);
    *first.0.borrow_mut() = None;
}
