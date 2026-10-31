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
pub fn fold_run_ins(
    admission: &crate::budget::admission::Admission,
    nodes: &mut Vec<Node>,
    parent: Option<&Computed>,
) -> Result<(), crate::budget::admission::Error> {
    struct Frame {
        nodes: Vec<Node>,
        index: usize,
        keep: bool,
        descended: bool,
        owner: Option<Element>,
    }
    let mut stack = crate::budget::admission::Buffer::default();
    stack.push(
        Frame {
            nodes: std::mem::take(nodes),
            index: 0,
            keep: parent.is_some_and(|p| p.keep_spaces == Some(true)),
            descended: false,
            owner: None,
        },
        admission,
    )?;
    while let Some(frame) = stack.last_mut() {
        if frame.index == frame.nodes.len() {
            let frame = stack.pop().expect("active run-in frame");
            if let Some(mut owner) = frame.owner {
                owner.children = frame.nodes.into();
                let parent = stack.last_mut().expect("run-in parent");
                parent.nodes[parent.index] = Node::Element(owner);
            } else {
                *nodes = frame.nodes;
            }
            continue;
        }
        if !frame.descended {
            frame.descended = true;
            if matches!(frame.nodes[frame.index], Node::Element(_)) {
                let Node::Element(mut owner) =
                    std::mem::replace(&mut frame.nodes[frame.index], Node::Text(String::new()))
                else {
                    unreachable!()
                };
                let nodes = std::mem::take(&mut owner.children).into_vec();
                let keep = owner.style.cascaded().keep_spaces == Some(true);
                stack.push(
                    Frame {
                        nodes,
                        index: 0,
                        keep,
                        descended: false,
                        owner: Some(owner),
                    },
                    admission,
                )?;
                continue;
            }
        }
        admission.work(1)?;
        frame.index = fold_at(admission, &mut frame.nodes, frame.index, frame.keep)?;
        frame.descended = false;
    }
    Ok(())
}

fn fold_at(
    admission: &crate::budget::admission::Admission,
    nodes: &mut Vec<Node>,
    i: usize,
    keep: bool,
) -> Result<usize, crate::budget::admission::Error> {
    // Блочная коробка В ПОТОКЕ где угодно внутри (в том числе за
    // строчными обёртками) запрещает вбегание.
    let runs_in = match &nodes[i] {
        Node::Element(e) => {
            e.style.cascaded().run_in == Some(true)
                && !crate::style::flow::has_float_or_absolute_position(e.style.cascaded())
                && !holds_block(admission, &e.children)?
        }
        _ => false,
    };
    if !runs_in {
        return Ok(i + 1);
    }
    // Следующая непустая коробка: подходит только обычный блок — не
    // run-in, не строчный. Плавающие и позиционированные соседи
    // ПРОЗРАЧНЫ: они вне потока и вбеганию не мешают (§9.2.3).
    let Some(j) = (i + 1..nodes.len()).find(|&j| {
            !matches!(&nodes[j], Node::Text(t) if t.is_empty() || (!keep && t.trim().is_empty())) && !matches!(&nodes[j], Node::Element(t) if crate::style::flow::has_float_or_absolute_position(t.style.cascaded()))
        }) else {
            return Ok(i + 1);
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
        return Ok(i + 1);
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

    let Node::Element(target) = &mut nodes[j - 1] else {
        // target_ok проверил Element в j; remove(i), i < j, сдвинул его в j - 1.
        unreachable!()
    };
    target.children.insert(0, Node::Element(run));
    // Сдвинутый узел должен сначала пройти свой спуск.
    Ok(i)
}

fn holds_block(
    admission: &crate::budget::admission::Admission,
    nodes: &[Node],
) -> Result<bool, crate::budget::admission::Error> {
    let transparent = |ch: &Element| {
        ch.inline
            || matches!(
                ch.style.cascaded().display,
                Some(
                    Display::InlineBlock
                        | Display::InlineFlex
                        | Display::InlineGrid
                        | Display::InlineTable
                )
            )
    };
    let visible = |ch: &Element| {
        !crate::style::flow::has_float_or_absolute_position(ch.style.cascaded())
            && ch.style.cascaded().display != Some(Display::None)
    };
    for node in crate::dom::traversal::nodes(admission, nodes, |ch| visible(ch) && transparent(ch))
    {
        if matches!(node?, Node::Element(ch) if visible(ch) && !transparent(ch)) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn flex_items_lose_float(
    admission: &crate::budget::admission::Admission,
    nodes: &mut [Node],
) -> Result<(), crate::budget::admission::Error> {
    crate::dom::traversal::postorder(admission, nodes, |el| {
        if !matches!(
            el.style.cascaded().display,
            Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::Flex)
                | Some(Display::InlineFlex)
        ) {
            return;
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
    })
}
