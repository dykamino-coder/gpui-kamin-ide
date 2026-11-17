//! Кратчайшие пути и доминаторы. Путь — обход в ширину от общего корня, сначала
//! через области корней, и только потом через «без корня»: блок, который держат
//! и стек, и недостижимый объект, получает путь от стека. Доминаторы —
//! итеративный алгоритм Cooper–Harvey–Kennedy («A Simple, Fast Dominance
//! Algorithm»; его же применяет анализ кучи Gecko) по обратному постпорядку;
//! удержанный размер — сумма собственных размеров поддерева доминаторов.

use super::{Graph, NONE};
use std::collections::VecDeque;

pub(super) fn shortest_paths(graph: &mut Graph) {
    let nodes = graph.nodes();
    let (root, unrooted) = (graph.root(), graph.unrooted());
    let mut parent = vec![NONE; nodes];
    let mut offset = vec![0u32; nodes];
    parent[root as usize] = root;
    let mut queue = VecDeque::from([root]);
    let mut late = true;
    loop {
        while let Some(node) = queue.pop_front() {
            for (target, at) in graph.edges(node) {
                if target == unrooted || parent[target as usize] != NONE {
                    continue;
                }
                parent[target as usize] = node;
                offset[target as usize] = at;
                queue.push_back(target);
            }
        }
        if !std::mem::take(&mut late) {
            break;
        }
        parent[unrooted as usize] = root;
        queue.push_back(unrooted);
    }
    graph.parent = parent;
    graph.parent_offset = offset;
}

/// Постпорядок обхода в глубину от корня: номер узла и порядок узлов.
fn postorder(graph: &Graph) -> (Vec<u32>, Vec<u32>) {
    let nodes = graph.nodes();
    let mut number = vec![NONE; nodes];
    let mut order = Vec::with_capacity(nodes);
    let mut seen = vec![false; nodes];
    let root = graph.root();
    seen[root as usize] = true;
    let mut calls = vec![(root, graph.edge_start[root as usize])];
    while let Some(top) = calls.len().checked_sub(1) {
        let (node, position) = calls[top];
        if position < graph.edge_start[node as usize + 1] {
            calls[top].1 += 1;
            let target = graph.edge_to[position as usize];
            if !seen[target as usize] {
                seen[target as usize] = true;
                calls.push((target, graph.edge_start[target as usize]));
            }
            continue;
        }
        calls.pop();
        number[node as usize] = order.len() as u32;
        order.push(node);
    }
    (number, order)
}

/// Предшественники в виде CSR.
fn predecessors(graph: &Graph) -> (Vec<u32>, Vec<u32>) {
    let nodes = graph.nodes();
    let mut start = vec![0u32; nodes + 1];
    for target in &graph.edge_to {
        start[*target as usize + 1] += 1;
    }
    for index in 0..nodes {
        start[index + 1] += start[index];
    }
    let mut fill = start.clone();
    let mut from = vec![0u32; graph.edge_to.len()];
    for source in 0..nodes as u32 {
        for (target, _) in graph.edges(source) {
            from[fill[target as usize] as usize] = source;
            fill[target as usize] += 1;
        }
    }
    (start, from)
}

pub(super) fn dominators(graph: &mut Graph) {
    let nodes = graph.nodes();
    let root = graph.root();
    let (number, order) = postorder(graph);
    let (start, from) = predecessors(graph);
    let mut idom = vec![NONE; nodes];
    idom[root as usize] = root;
    let intersect = |idom: &[u32], mut a: u32, mut b: u32| {
        while a != b {
            while number[a as usize] < number[b as usize] {
                a = idom[a as usize];
            }
            while number[b as usize] < number[a as usize] {
                b = idom[b as usize];
            }
        }
        a
    };
    let mut changed = true;
    while changed {
        changed = false;
        for &node in order.iter().rev().skip(1) {
            let mut next = NONE;
            for &pred in &from[start[node as usize] as usize..start[node as usize + 1] as usize] {
                if idom[pred as usize] == NONE {
                    continue;
                }
                next = if next == NONE {
                    pred
                } else {
                    intersect(&idom, pred, next)
                };
            }
            if idom[node as usize] != next {
                idom[node as usize] = next;
                changed = true;
            }
        }
    }
    let mut retained: Vec<u64> = (0..nodes as u32).map(|node| graph.bytes(node)).collect();
    for &node in &order {
        let dominator = idom[node as usize];
        if node != root && dominator != NONE {
            retained[dominator as usize] += retained[node as usize];
        }
    }
    graph.idom = idom;
    graph.retained = retained;
}
