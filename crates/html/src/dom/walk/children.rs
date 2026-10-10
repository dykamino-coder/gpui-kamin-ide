//! Children for walk; split out to keep the owning module within 250 lines.

use super::{walk, walk_children};
use crate::dom::*;
use crate::style::computed::{Computed, Display};
use crate::style::css::{Decls, Keyframes, Rule};
use crate::style::select::{Ancestor, Sibs};
use markup5ever_rcdom::Handle;
use std::collections::HashMap;
use std::rc::Rc;

#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_borrow)]
pub(super) fn collect_children(
    handle: &Handle,
    rules: &[Rule],
    vars: &Decls,
    frames: &HashMap<String, Keyframes>,
    counter: &mut u64,
    counters: &mut crate::style::generated::counters::Counters,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    style: &Computed,
    shadow: &Option<Rc<crate::dom::shadow::Shadow>>,
    preserve: bool,
    tag: &String,
    attrs: &[(String, String)],
) -> (Vec<Node>, ScrollPseudos) {
    let mut path2 = path.to_vec();
    path2.push(me.clone());
    let mut children = vec![];
    // Псевдоэлементы: коробка появляется, только если у правила есть
    // `content`. Значками, стрелками и разделителями в вёрстке
    // занимаются именно они, и без них разметка теряет часть смысла.
    // `::before` строится ДО детей, `::after` — после: счётчики они
    // видят в том же порядке, что и браузер (css-lists §counters).
    if let Some(el) = pseudo_box(rules, vars, counters, &me, path, sibs, "before", &attrs) {
        children.push(Node::Element(el));
    }
    let keep = style.preserve_newlines.unwrap_or(preserve);
    if let Some(shadow) = &shadow {
        // Плоское дерево (css-shadow-1 §3.3): хост наполняется детьми
        // корня тени вместо светлых — в области стилей тени и с
        // цепочкой предков из одного безликого хоста. Светлые дети
        // попадут в вывод только через `<slot>`.
        walk_children(
            &shadow.root,
            &shadow.scope.rules,
            vars,
            &shadow.scope.frames,
            counter,
            counters,
            std::slice::from_ref(&shadow.marker),
            keep,
            &mut children,
        );
    } else if let Some(slot) = slot_of(handle).filter(|s| !s.assigned.is_empty()) {
        // Слот показывает распределённые узлы, fallback — только без
        // них (HTML §4.12.4). Распределённый ребёнок стилизуется
        // таблицами ОБЛАСТИ ХОСТА и сопоставляется в светлом контексте
        // (предки хоста + хост, светлые братья), а наследует — от
        // слота, своего родителя в плоском дереве (`vars`, `keep`).
        for s in &slot.assigned {
            let sibs = Sibs {
                all: &slot.light_all[..],
                pos: s.elem_pos,
                is_elem: s.anc.is_some(),
                rc: Some(&slot.light_all),
            };
            walk(
                &s.node,
                &slot.outer.rules,
                vars,
                &slot.outer.frames,
                counter,
                counters,
                &slot.host_path[..],
                slot.light_spots[s.light_idx],
                keep,
                sibs,
                &slot.light,
                &slot.light_spots,
                s.light_idx,
                &mut children,
            );
        }
    } else {
        walk_children(
            handle,
            rules,
            vars,
            frames,
            counter,
            counters,
            &path2,
            keep,
            &mut children,
        );
    }
    if let Some(el) = pseudo_box(rules, vars, counters, &me, path, sibs, "after", &attrs) {
        children.push(Node::Element(el));
    }
    // css-overflow-5: скроллер со `scroll-marker-group` собирает
    // `::scroll-marker` потомков в группу-соседа (у корня — ребёнка),
    // скроллер без группы их гасит; кнопки `::scroll-button()` — по
    // `content`. Свой маркер элемента кладётся ПОСЛЕ сбора: он идёт
    // в группу ВНЕШНЕГО скроллера, а не в собственную.
    let scroll_pseudos = scroll_marker_pass(
        rules,
        vars,
        counters,
        &me,
        path,
        sibs,
        &tag,
        &style,
        &attrs,
        &mut children,
    );
    if let Some(el) = pseudo_box(
        rules,
        vars,
        counters,
        &me,
        path,
        sibs,
        "scroll-marker",
        &attrs,
    ) {
        children.push(Node::Element(el));
    }
    // Колонка значит что-то ТОЛЬКО внутри таблицы или группы
    // колонок. У любого другого родителя она исчезает ровно так же,
    // как исчезала до сих пор: `empty-cells-applies-to-012` ставит
    // `display: table-column` с красным фоном ВНУТРИ ряда и требует
    // «no red». Без этой отсечки такой узел уехал бы в анонимную
    // ячейку и покрасился.
    let holds_columns = matches!(tag.as_str(), "table" | "colgroup")
        || style.col_role == Some(1)
        || matches!(
            style.display,
            Some(Display::Table) | Some(Display::InlineTable)
        );
    if !holds_columns {
        children.retain(|n| !matches!(n, Node::Element(c) if c.style.col_role.is_some()));
    }
    (children, scroll_pseudos)
}
