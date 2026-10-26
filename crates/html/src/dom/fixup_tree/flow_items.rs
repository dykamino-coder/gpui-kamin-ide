//! Правки потока по собранному дереву: вбегание `run-in` и флоаты у детей
//! flex/grid.
//!
//! `fold_run_ins` (CSS 2.1 §9.2.3): элемент `display: run-in` без блочного
//! содержимого, за которым сквозь схлопываемый пробел идёт обычный блок,
//! становится его первым строчным ребёнком; внепоточные соседи прозрачны.
//! `flex_items_lose_float` (css-flexbox-1 §4, css-grid-2 §6): у потокового
//! ребёнка flex/grid `float` и `clear` вычисляются в `none`.

use crate::dom::*;
use crate::style::computed::{Computed, Display};

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
            fold_run_ins(&mut e.children, Some(own.cascaded()));
        }
        // Блочная коробка В ПОТОКЕ где угодно внутри (в том числе за
        // строчными обёртками) запрещает вбегание.
        fn holds_block(nodes: &[Node]) -> bool {
            nodes.iter().any(|c| match c {
                Node::Element(ch) => {
                    if crate::style::flow::has_float_or_absolute_position(ch.style.cascaded())
                        || ch.style.cascaded().display == Some(Display::None)
                    {
                        return false;
                    }
                    if ch.inline
                        || matches!(
                            ch.style.cascaded().display,
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
                e.style.cascaded().run_in == Some(true)
                    && !crate::style::flow::has_float_or_absolute_position(e.style.cascaded())
                    && !holds_block(&e.children)
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
            !is_blank(&nodes[j]) && !matches!(&nodes[j], Node::Element(t) if crate::style::flow::has_float_or_absolute_position(t.style.cascaded()))
        }) else {
            i += 1;
            continue;
        };
        let target_ok = matches!(&nodes[j], Node::Element(t)
        if !t.inline
            && t.style.cascaded().run_in != Some(true)
            && !matches!(
                t.style.cascaded().display,
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
            // runs_in выше истинно только для Element; до remove дерево не менялось.
            unreachable!()
        };
        run.inline = true;
        run.style.cascaded_mut().display = None;
        run.style.cascaded_mut().run_in = None;
        // Наследование от ИСХОДНОГО родителя (§9.2.3) НЕ запекается:
        // `style::cascade::inherit` сливает и рендерные поля, и `run-in-inherit-001`
        // от этого ломается. Цвет нового блока вбёгнутый перенимает
        // неправильно — хвост не решён.
        let _ = parent;
        let Node::Element(target) = &mut nodes[j - 1] else {
            // target_ok проверил Element в j; remove(i), i < j, сдвинул его в j - 1.
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
            el.style.cascaded().display,
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
            if crate::style::flow::absolutely_positioned(child.style.cascaded().position) {
                continue;
            }
            child.style.cascaded_mut().float = None;
            child.style.cascaded_mut().clear = None;
        }
    }
}
