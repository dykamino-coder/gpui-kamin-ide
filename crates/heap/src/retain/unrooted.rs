//! Вход в граф для блоков, недостижимых из областей корней. Их держит то, что
//! скан не видит (стек другого потока, память ОС — например указатель окна в
//! данных окна Windows), или никто: утечка. Узел «без корня» получает ребро на
//! каждый исток — блок без входящих рёбер — и на представителя каждого цикла,
//! который никто снаружи не держит (компоненты сильной связности Тарьяна без
//! входа из других компонент; представитель — самый крупный блок). Тогда всё
//! недостижимое достижимо ровно через свои истоки, и доминаторы делят его по
//! настоящим держателям, а не по порядку обхода.

use super::{FLAG_CYCLE, FLAG_UNHELD, Graph, NONE};

/// Отмечает достижимое из `start` по рёбрам блоков.
fn mark_from(graph: &Graph, start: u32, reached: &mut [bool], stack: &mut Vec<u32>) {
    stack.push(start);
    while let Some(node) = stack.pop() {
        for (target, _) in graph.edges(node) {
            if !reached[target as usize] {
                reached[target as usize] = true;
                stack.push(target);
            }
        }
    }
}

pub(super) fn connect(graph: &mut Graph) {
    let count = graph.blocks.len();
    let mut reached = vec![false; graph.nodes()];
    let mut stack = Vec::new();
    mark_from(graph, graph.root(), &mut reached, &mut stack);
    // Входящие рёбра от недостижимых блоков.
    let mut incoming = vec![0u32; count];
    for source in 0..count as u32 {
        if !reached[source as usize] {
            for (target, _) in graph.edges(source) {
                incoming[target as usize] = incoming[target as usize].saturating_add(1);
            }
        }
    }
    let mut entries: Vec<(u32, u8)> = Vec::new();
    for block in 0..count as u32 {
        if !reached[block as usize] && incoming[block as usize] == 0 {
            reached[block as usize] = true;
            entries.push((block, FLAG_UNHELD));
            mark_from(graph, block, &mut reached, &mut stack);
        }
    }
    drop(incoming);
    for representative in unheld_cycles(graph, &reached) {
        entries.push((representative, FLAG_CYCLE));
    }
    graph.flags = vec![0; graph.nodes()];
    for (block, flag) in entries {
        graph.flags[block as usize] = flag;
        graph.edge_to.push(block);
        graph.edge_offset.push(0);
    }
    let end = graph.edge_to.len() as u32;
    if let Some(last) = graph.edge_start.last_mut() {
        *last = end;
    }
}

/// Представители компонент сильной связности среди ещё не достигнутых блоков,
/// в которые нет рёбер из других таких компонент.
fn unheld_cycles(graph: &Graph, reached: &[bool]) -> Vec<u32> {
    let count = graph.blocks.len();
    let mut component = vec![NONE; count];
    let components = tarjan(graph, reached, &mut component);
    if components == 0 {
        return Vec::new();
    }
    let mut entered = vec![false; components];
    let mut largest = vec![NONE; components];
    for block in 0..count as u32 {
        let own = component[block as usize];
        if own == NONE {
            continue;
        }
        let best = &mut largest[own as usize];
        if *best == NONE || graph.bytes(block) > graph.bytes(*best) {
            *best = block;
        }
        for (target, _) in graph.edges(block) {
            let other = component[target as usize];
            if other != NONE && other != own {
                entered[other as usize] = true;
            }
        }
    }
    (0..components)
        .filter(|&index| !entered[index])
        .map(|index| largest[index])
        .collect()
}

/// Итеративный алгоритм Тарьяна на недостигнутых блоках; возвращает число компонент.
fn tarjan(graph: &Graph, reached: &[bool], component: &mut [u32]) -> usize {
    let count = graph.blocks.len();
    let mut order = vec![NONE; count];
    let mut low = vec![0u32; count];
    let mut on_stack = vec![false; count];
    let mut stack: Vec<u32> = Vec::new();
    // Кадр обхода: узел и позиция следующего ребра.
    let mut calls: Vec<(u32, u32)> = Vec::new();
    let (mut next, mut components) = (0u32, 0usize);
    for start in 0..count as u32 {
        if reached[start as usize] || order[start as usize] != NONE {
            continue;
        }
        calls.push((start, graph.edge_start[start as usize]));
        order[start as usize] = next;
        low[start as usize] = next;
        next += 1;
        stack.push(start);
        on_stack[start as usize] = true;
        while let Some(top) = calls.len().checked_sub(1) {
            let (node, position) = calls[top];
            if position < graph.edge_start[node as usize + 1] {
                let target = graph.edge_to[position as usize];
                calls[top].1 += 1;
                let t = target as usize;
                if reached[t] {
                    continue;
                }
                if order[t] == NONE {
                    order[t] = next;
                    low[t] = next;
                    next += 1;
                    stack.push(target);
                    on_stack[t] = true;
                    calls.push((target, graph.edge_start[t]));
                } else if on_stack[t] {
                    low[node as usize] = low[node as usize].min(order[t]);
                }
                continue;
            }
            calls.pop();
            if let Some(&(caller, _)) = calls.last() {
                low[caller as usize] = low[caller as usize].min(low[node as usize]);
            }
            if low[node as usize] == order[node as usize] {
                while let Some(member) = stack.pop() {
                    on_stack[member as usize] = false;
                    component[member as usize] = components as u32;
                    if member == node {
                        break;
                    }
                }
                components += 1;
            }
        }
    }
    components
}
