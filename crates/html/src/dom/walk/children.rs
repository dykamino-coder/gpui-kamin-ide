//! Сборка детей элемента при обходе: псевдоэлементы, тень или слот, маркеры.
//!
//! `collect_children` строит `::before` до детей и `::after` после (счётчики
//! в порядке браузера); дети берутся из корня тени (плоское дерево,
//! css-shadow-1 §3.3), из распределённых узлов слота (стили области хоста,
//! наследование от слота) или из светлых детей. Затем `scroll_marker_pass`
//! и собственный `::scroll-marker` элемента (он идёт во внешнюю группу), а
//! колонки вне таблицы/группы колонок отбрасываются (CSS 2.1 §17.2.1).

use super::{build_dom_node, walk_children};
use crate::dom::tree_index::TreeIndex;
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
    tree: &TreeIndex,
    rules: &[Rule],
    vars: &Decls,
    frames: &HashMap<String, Keyframes>,
    counter: &mut u64,
    counters: &mut crate::style::generated::counters::Counters,
    me: &Ancestor,
    path: &mut Vec<Ancestor>,
    sibs: Sibs,
    style: &Computed,
    shadow: &Option<Rc<crate::dom::shadow::Shadow>>,
    preserve: bool,
    tag: &str,
    attrs: &[(String, String)],
    vcx: crate::document::state::ValueCx<'_>,
) -> (Vec<Node>, ScrollPseudos) {
    let mut children = vec![];
    // Псевдоэлементы: коробка появляется, только если у правила есть
    // `content`. Значками, стрелками и разделителями в вёрстке
    // занимаются именно они, и без них разметка теряет часть смысла.
    // `::before` строится ДО детей, `::after` — после: счётчики они
    // видят в том же порядке, что и браузер (css-lists §counters).
    if let Some(el) = pseudo_box(
        rules, vars, counters, &me, path, sibs, "before", &attrs, vcx,
    ) {
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
            tree,
            &shadow.scope.rules,
            vars,
            &shadow.scope.frames,
            counter,
            counters,
            &mut vec![shadow.marker.clone()],
            keep,
            &mut children,
            vcx,
        );
    } else if let Some(slot) = tree.slot_of(handle).filter(|s| !s.assigned.is_empty()) {
        // Слот показывает распределённые узлы, fallback — только без
        // них (HTML §4.12.4). Распределённый ребёнок стилизуется
        // таблицами ОБЛАСТИ ХОСТА и сопоставляется в светлом контексте
        // (предки хоста + хост, светлые братья), а наследует — от
        // слота, своего родителя в плоском дереве (`vars`, `keep`).
        let mut host_path = slot.host_path.as_ref().clone();
        for s in &slot.assigned {
            let sibs = Sibs {
                all: &slot.light_all[..],
                pos: s.elem_pos,
                is_elem: s.anc.is_some(),
                rc: Some(&slot.light_all),
            };
            build_dom_node(
                &s.node,
                tree,
                &slot.outer.rules,
                vars,
                &slot.outer.frames,
                counter,
                counters,
                &mut host_path,
                slot.light_spots[s.light_idx],
                keep,
                sibs,
                &slot.light,
                &slot.light_spots,
                s.light_idx,
                &mut children,
                vcx,
            );
        }
    } else {
        path.push(me.clone());
        walk_children(
            handle,
            tree,
            rules,
            vars,
            frames,
            counter,
            counters,
            path,
            keep,
            &mut children,
            vcx,
        );
        path.pop();
    }
    if let Some(el) = pseudo_box(rules, vars, counters, &me, path, sibs, "after", &attrs, vcx) {
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
        vcx,
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
        vcx,
    ) {
        children.push(Node::Element(el));
    }
    // Колонка значит что-то ТОЛЬКО внутри таблицы или группы
    // колонок. У любого другого родителя она исчезает ровно так же,
    // как исчезала до сих пор: `empty-cells-applies-to-012` ставит
    // `display: table-column` с красным фоном ВНУТРИ ряда и требует
    // «no red». Без этой отсечки такой узел уехал бы в анонимную
    // ячейку и покрасился.
    let holds_columns = matches!(tag, "table" | "colgroup")
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
