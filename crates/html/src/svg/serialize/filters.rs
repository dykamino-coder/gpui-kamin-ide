//! Подготовка загрязнённых SVG-фильтров перед сериализацией.

use crate::dom::{Element, Node};

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): дописывать в `<defs>` недостающие
/// `<filter>` из соседних `<svg>` документа (реестр `render::mask_def`,
/// ключ `filter:<id>`) перед `</svg>` — срез filter-effects 184 -> 184,
/// `svg-filter-primitive-units-user-space` и родня не сдвинулись:
/// одной подстановки определения мало, единицы фильтра считаются от
/// чужого вьюпорта. Разбор: target/scout-masking-filters-2026-09.md, F5.
pub(super) fn subtree_has(e: &Element, tag: &str) -> bool {
    e.tag.eq_ignore_ascii_case(tag)
        || e.children.iter().any(|n| match n {
            Node::Element(c) => subtree_has(c, tag),
            _ => false,
        })
}

/// filter-effects-1 «Restrictions on filter primitives»: примитив, чей цвет
/// зависит от currentColor (`flood-color`/`lighting-color` в любой обёртке),
/// «загрязнён»; `feDisplacementMap` с загрязнённой картой (in2) обязан
/// работать сквозным проходом. resvg о загрязнении не знает — такой примитив
/// заменяется на `feOffset dx=0 dy=0` до сериализации (WPT tainting-*).
pub(super) fn untaint_filters(e: &mut Element) {
    if e.tag == "filter" {
        let mut tainted: std::collections::HashSet<String> = Default::default();
        let mut prev = false;
        for child in e.children.iter_mut() {
            let Node::Element(p) = child else { continue };
            if !p.tag.starts_with("fe") {
                continue;
            }
            let val =
                |p: &Element, k: &str| p.attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
            let dirty_ref = |name: &Option<String>, prev: bool| match name.as_deref() {
                None => prev,
                Some(n) => tainted.contains(n),
            };
            // Собственное загрязнение: цвет примитива от currentColor —
            // в атрибуте или в style, включая обёртки color-mix()/color(from).
            let self_dirty = p.attrs.iter().any(|(k, v)| {
                matches!(k.as_str(), "flood-color" | "lighting-color" | "style")
                    && v.to_ascii_lowercase().contains("currentcolor")
            });
            let in1 = val(p, "in");
            let in2 = val(p, "in2");
            let mut dirty = self_dirty || dirty_ref(&in1, prev);
            if p.tag == "feMerge" {
                for n in &p.children {
                    if let Node::Element(m) = n
                        && m.tag == "feMergeNode"
                        && dirty_ref(&val(m, "in"), prev)
                    {
                        dirty = true;
                    }
                }
            }
            if p.tag == "feDisplacementMap" && dirty_ref(&in2, prev) {
                // Сквозной проход: сохранить in/result, снять карту.
                p.tag = "feOffset".to_string();
                p.attrs.retain(|(k, _)| {
                    !matches!(
                        k.as_str(),
                        "in2" | "scale" | "xChannelSelector" | "yChannelSelector" | "dx" | "dy"
                    )
                });
                p.attrs.push(("dx".to_string(), "0".to_string()));
                p.attrs.push(("dy".to_string(), "0".to_string()));
                // Выход сквозного прохода загрязнён лишь настолько,
                // насколько его основной вход.
                dirty = dirty_ref(&in1, prev);
            } else if p.tag == "feDisplacementMap" {
                dirty = dirty || dirty_ref(&in2, prev);
            }
            if dirty && let Some(r) = val(p, "result") {
                tainted.insert(r);
            }
            prev = dirty;
        }
        return;
    }
    for child in e.children.iter_mut() {
        if let Node::Element(c) = child {
            untaint_filters(c);
        }
    }
}
