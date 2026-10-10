//! Flow items for fixup_tree; split out to keep the owning module within 250 lines.

use crate::dom::*;
use crate::style::computed::{Computed, Display, Position};

/// Вбегание `display: run-in` (CSS 2.1 §9.2.3): элемент без блочного
/// содержимого, за которым (сквозь пробельный текст) идёт обычная блочная
/// коробка, становится её ПЕРВЫМ СТРОЧНЫМ ребёнком; во всех остальных
/// случаях он ведёт себя как блок (это уже так — разбор дал Block).
pub(crate) fn fold_run_ins(nodes: &mut Vec<Node>, parent: Option<&Computed>) {
    // Пробельный текст прозрачен для вбегания, только если он СХЛОПНЕТСЯ:
    // при `white-space: pre*` контейнера пробел — настоящий строчный кусок
    // (анонимная строка), и за run-in идёт уже не блок (css-display-3 §4.1:
    // «intervening white space» — схлопываемый). `run-in-basic-014`: эталон —
    // run-in блоком, строка сохранённого пробела, затем блок.
    let keep = parent.is_some_and(|p| p.keep_spaces == Some(true));
    let is_blank =
        |n: &Node| matches!(n, Node::Text(t) if t.is_empty() || (!keep && t.trim().is_empty()));
    let mut i = 0;
    while i < nodes.len() {
        // Сначала вглубь: вложенные run-in решаются в своём контейнере.
        if let Node::Element(e) = &mut nodes[i] {
            let own = e.style.clone();
            fold_run_ins(&mut e.children, Some(&own));
        }
        // Вне потока элемент вбеганию не мешает и сам не вбегает.
        fn out_of_flow(e: &Element) -> bool {
            e.style.float.is_some()
                || matches!(
                    e.style.position,
                    Some(crate::style::computed::Position::Absolute)
                        | Some(crate::style::computed::Position::Fixed)
                )
        }
        // Блочная коробка В ПОТОКЕ где угодно внутри (в том числе за
        // строчными обёртками) запрещает вбегание.
        fn holds_block(nodes: &[Node]) -> bool {
            nodes.iter().any(|c| match c {
                Node::Element(ch) => {
                    if out_of_flow(ch) || ch.style.display == Some(Display::None) {
                        return false;
                    }
                    if ch.inline
                        || matches!(
                            ch.style.display,
                            Some(Display::InlineBlock)
                                | Some(Display::InlineFlex)
                                | Some(Display::InlineGrid)
                                | Some(Display::InlineTable)
                        )
                    {
                        return holds_block(&ch.children);
                    }
                    true
                }
                _ => false,
            })
        }
        let runs_in = match &nodes[i] {
            Node::Element(e) => {
                e.style.run_in == Some(true) && !out_of_flow(e) && !holds_block(&e.children)
            }
            _ => false,
        };
        if !runs_in {
            i += 1;
            continue;
        }
        // Следующая непустая коробка: подходит только обычный блок — не
        // run-in, не строчный. Плавающие и позиционированные соседи
        // ПРОЗРАЧНЫ: они вне потока и вбеганию не мешают (§9.2.3).
        let Some(j) = (i + 1..nodes.len()).find(|&j| {
            !is_blank(&nodes[j]) && !matches!(&nodes[j], Node::Element(t) if out_of_flow(t))
        }) else {
            i += 1;
            continue;
        };
        let target_ok = matches!(&nodes[j], Node::Element(t)
        if !t.inline
            && t.style.run_in != Some(true)
            && !matches!(
                t.style.display,
                Some(Display::None)
                    | Some(Display::InlineBlock)
                    | Some(Display::InlineFlex)
                    | Some(Display::InlineGrid)
                    | Some(Display::InlineTable)
                    | Some(Display::Table)
                    | Some(Display::TableRow)
                    | Some(Display::TableRowGroup)
                    | Some(Display::TableCell)
            ));
        if !target_ok {
            i += 1;
            continue;
        }
        let Node::Element(mut run) = nodes.remove(i) else {
            unreachable!()
        };
        run.inline = true;
        run.style.display = None;
        run.style.run_in = None;
        // Наследование от ИСХОДНОГО родителя (§9.2.3) НЕ запекается:
        // inline::inherit сливает и рендерные поля, и замер показал минус
        // (run-in-inherit-001: 5.88 -> 7.35). Цвет нового блока вбёгнутый
        // перенимает неправильно — хвост запаркован.
        let _ = parent;
        let Node::Element(target) = &mut nodes[j - 1] else {
            unreachable!()
        };
        target.children.insert(0, Node::Element(run));
        // На месте i теперь стоит бывший j-1 — им и продолжаем.
    }
}

pub(crate) fn flex_items_lose_float(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        flex_items_lose_float(&mut el.children);
        if !matches!(
            el.style.display,
            Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::Flex)
                | Some(Display::InlineFlex)
        ) {
            continue;
        }
        for child in el.children.iter_mut() {
            let Node::Element(child) = child else {
                continue;
            };
            if matches!(
                child.style.position,
                Some(Position::Absolute) | Some(Position::Fixed)
            ) {
                continue;
            }
            child.style.float = None;
            child.style.clear = None;
        }
    }
}
