//! Обход html5ever-дерева: каскад и сборка узлов (walk).

mod ancestor;
use ancestor::element_ancestor;

mod sibling_survey;
pub(super) use sibling_survey::walk_children;

mod text_nodes;
use crate::dom::walk::text_nodes::walk_text;

mod hidden;
use crate::dom::walk::hidden::hidden_element;

mod children;
use crate::dom::walk::children::collect_children;

mod cascade;
use crate::dom::walk::cascade::cascade_element;

mod animation_frames;
mod finish_element;
mod pseudo_styles;
mod style_normalization;
use crate::dom::walk::animation_frames::animation_frames;
use crate::dom::walk::finish_element::finish_element;
use crate::dom::walk::pseudo_styles::pseudo_styles;
use crate::dom::walk::style_normalization::normalize_style;

use super::{content, initial_pseudos, language, presentational_hints};
use crate::dom::*;
use crate::style::computed::Display;
use crate::style::css::{Decls, Keyframes, Rule};
use crate::style::select::{Ancestor, Sibs, Spot};
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;

#[allow(clippy::too_many_arguments)]
fn walk(
    handle: &Handle,
    rules: &[Rule],
    vars: &Decls,
    frames: &HashMap<String, Keyframes>,
    counter: &mut u64,
    counters: &mut crate::style::generated::counters::Counters,
    path: &[Ancestor],
    spot: Spot,
    preserve: bool,
    sibs: Sibs,
    level: &[Handle],
    spots: &[Spot],
    level_pos: usize,
    out: &mut Vec<Node>,
) {
    match &handle.data {
        NodeData::Text { contents } => {
            walk_text(
                contents.borrow().to_string(),
                preserve,
                level,
                level_pos,
                out,
            );
        }
        NodeData::Element { name, attrs, .. } => {
            let tag = local_name(&name.local);
            if DROP_TAGS.contains(&tag.as_str()) {
                return;
            }
            let attrs: Vec<(String, String)> = attrs
                .borrow()
                .iter()
                .map(|a| (a.name.local.to_string(), a.value.to_string()))
                .collect();
            let mut me = element_ancestor(
                handle,
                &tag,
                &attrs,
                content::html_attributes(&name.ns),
                spot,
                sibs,
            );
            let (mut style, own_vars, _scheme, shadow) =
                cascade_element(&mut me, &attrs, &tag, handle, rules, vars, path, spot, sibs);
            let vars = &own_vars;
            normalize_style(&mut style, &tag, &attrs, path);
            // Правила с `:hover` собираются отдельным слоем: в базовый стиль
            // им нельзя, иначе элемент выглядел бы всегда наведённым.
            let (hover, first_letter, first_line, marker_layer) =
                pseudo_styles(&mut style, &tag, rules, vars, &me, path, sibs);
            // Обратный счётчик без числа: начальное значение — итог
            // предварительного обхода области (css-lists-3
            // §instantiating-counters). Считается ЗДЕСЬ, до применения
            // директив: запись создаётся уже готовым числом.
            let reversed_start =
                |nm: &str, _counters: &mut crate::style::generated::counters::Counters| {
                    crate::style::generated::counters_scan::reversed_initial(
                        rules, vars, nm, handle, &me, path, sibs, level, spots, level_pos,
                    )
                };

            if style.display == Some(Display::None) {
                hidden_element(
                    style,
                    tag,
                    attrs,
                    &me,
                    handle,
                    rules,
                    vars,
                    frames,
                    counter,
                    counters,
                    path,
                    preserve,
                    &reversed_start,
                    out,
                );
                return;
            }

            // Counters apply before children; display:contents has no box level.
            let box_level = style.display != Some(Display::Contents);
            if box_level {
                counters.enter();
                counters.set_quote_language(language::parent(&me, path).unwrap_or(""));
                if let Some(q) = &style.quotes {
                    counters.set_quotes(q.clone());
                }
            }
            let mut is_list_item = false;
            apply_counter_decls(
                &style,
                counters,
                &tag,
                &attrs,
                &mut is_list_item,
                &reversed_start,
            );
            // Номер пункта снимается СРАЗУ после своих директив — до
            // псевдоэлементов и детей, которые счётчик двигают дальше.
            let list_item = is_list_item.then(|| counters.value_of("list-item"));
            let style_scope = (box_level && style.contain_style == Some(true))
                .then(|| counters.enter_style_scope());
            // Содержимое маркера — по первому верному условию css-lists-3
            // §content-property: `content` на `::marker` не `normal` →
            // «exactly as for ::before»; `none` → коробки нет; иначе
            // прежний путь `list-style-*`. Blink делает ту же отсечку
            // первой строкой `ListMarker::MarkerText`:
            // `if (!marker.StyleRef().ContentBehavesAsNormal()) return
            // kNotText;` (`list_marker.cc:159`).
            //
            // Своей коробки у маркера в дереве нет — его рисует
            // `render::list` по `marker_text`/`no_marker`, поэтому
            // содержимое сворачивается в эти поля, а прочие свойства слоя
            // (цвет, шрифт, разрядка) едут в `marker_layer`.
            //
            // CSS Lists 3 §marker-pseudo: the marker is the item's FIRST child,
            // before ::before; counter-* declared on ::marker (with DOM
            // inheritance of counter directives) apply right here, before the
            // marker's own content is evaluated (`marker-counter`).
            if let Some(mut m) = marker_layer {
                inherit_counter_decls(&mut m, Some(&me.counter_style));
                counters.enter_marker();
                language::pseudo(counters, &m, &me, path);
                apply_counter_decls(&m, counters, "", &[], &mut false, &|_, _| 0);
                if let Some(items) = host_content(&m, &me) {
                    let quotes = m.quotes.as_ref();
                    style.marker_text = Some(content_text(
                        &items,
                        counters,
                        &attrs,
                        quotes,
                        me.html_attrs,
                    ));
                    style.no_marker = Some(false);
                } else if m.content_none == Some(true) {
                    style.no_marker = Some(true);
                }
                counters.leave();
                style.marker_layer = Some(Box::new(m));
            }

            let (mut children, scroll_pseudos) = collect_children(
                handle, rules, vars, frames, counter, counters, &me, path, sibs, &style, &shadow,
                preserve, &tag, &attrs,
            );
            // CSS Lists §12.4.1: ordinary counters survive into following siblings.
            // Restore only the isolated subtree; remove_stale handles ordinary scopes.
            if let Some(scope) = style_scope {
                counters.leave_style_scope(scope);
            }
            if box_level {
                counters.leave();
            }

            *counter += 1;
            // Неявный якорь псевдоэлемента — порождающий элемент
            // (css-anchor-position-1 §implicit); его `node_id` известен
            // только здесь, после обхода детей: `::before` собран до них,
            // `::after` — после, а номер хозяину даёт этот же счётчик.
            for kid in children.iter_mut() {
                if let Node::Element(c) = kid
                    && c.tag.starts_with("::")
                {
                    c.style.implicit_anchor = Some(*counter);
                }
            }
            let anim = animation_frames(&style, frames, vars);
            finish_element(
                style,
                children,
                tag,
                attrs,
                anim,
                list_item,
                *counter,
                hover,
                first_letter,
                first_line,
                scroll_pseudos,
                out,
            );
        }
        _ => walk_children(
            handle, rules, vars, frames, counter, counters, path, preserve, out,
        ),
    }
}
