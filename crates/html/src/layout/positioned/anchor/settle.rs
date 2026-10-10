//! Статическое сведение `anchor()` до сборки дерева и якорь по умолчанию.

use super::{CHOSEN, REFRAMES};
use crate::dom::Node;
use crate::style::computed::{Computed, PositionAnchor};
use crate::style::values::value::{Len, anchor_get};
use std::collections::HashSet;

/// Статически неразрешимые `anchor()` решаются до отрисовки (§anchor-resolution:
/// неразрешимая функция «computes to its specified fallback value. If no
/// fallback value is specified, it makes the declaration … invalid at
/// computed-value time» — вставка становится `auto`, коробка остаётся на
/// статической позиции). Решать это на подготовке кадра поздно: слои и
/// щупы статической позиции выбираются при сборке дерева по `edge_set`.
/// Без этого шага две пары, зелёные сейчас, уходили бы в красное
/// (`position-anchor-none/normal-pseudo-element-implicit-002`).
pub fn settle_static(nodes: &mut [Node]) {
    fn names(nodes: &[Node], out: &mut HashSet<String>) {
        for n in nodes {
            let Node::Element(e) = n else { continue };
            if let Some(list) = &e.style.anchor_name {
                out.extend(list.iter().cloned());
            }
            names(&e.children, out);
        }
    }
    fn settle(nodes: &mut [Node], known: &HashSet<String>) {
        for n in nodes.iter_mut() {
            let Node::Element(e) = n else { continue };
            let has_default = match &e.style.position_anchor {
                Some(PositionAnchor::Named(n)) => known.contains(n),
                Some(PositionAnchor::Auto) => e.style.implicit_anchor.is_some(),
                // `normal` при непустой `position-area` ведёт себя как `auto`
                // (§position-anchor); без области — как `none`.
                Some(PositionAnchor::Normal) | None => {
                    e.style.position_area.is_some() && e.style.implicit_anchor.is_some()
                }
                _ => false,
            };
            let s = &mut e.style.inset;
            for slot in [&mut s.top, &mut s.right, &mut s.bottom, &mut s.left] {
                *slot = settle_len(*slot, has_default, known);
            }
            settle(&mut e.children, known);
        }
    }
    // Новый документ: выбор `position-try` прошлого документа с теми же
    // `node_id` не должен пережить сборку.
    CHOSEN.with(|m| m.borrow_mut().clear());
    REFRAMES.with(|r| r.set(0));
    let mut known = HashSet::new();
    names(nodes, &mut known);
    settle(nodes, &known);
}

/// Одна вставка: цепочка запасных значений раскручивается, пока не
/// найдётся разрешимая функция или обычная длина.
fn settle_len(l: Option<Len>, has_default: bool, known: &HashSet<String>) -> Option<Len> {
    let mut cur = l;
    for _ in 0..4 {
        let Some(Len::Anchor(i)) = cur else {
            return cur;
        };
        let f = anchor_get(i)?;
        // У `min()`/`max()` разрешимы должны быть ВСЕ доводы.
        let resolvable = std::iter::once(&f)
            .chain(f.alts.iter())
            .all(|g| match &g.name {
                Some(n) => known.contains(n),
                None => has_default,
            });
        if resolvable {
            return cur;
        }
        cur = f.fallback;
    }
    cur
}

/// Якорь по умолчанию коробки (§position-anchor).
#[derive(Clone, Debug)]
pub(super) enum DefaultAnchor {
    Named(String),
    Implicit(u64),
}

/// `position-anchor` → якорь по умолчанию; `normal`/не задано при непустой
/// `position-area` — как `auto` (§position-anchor).
pub(super) fn default_anchor_of(c: &Computed) -> Option<DefaultAnchor> {
    match &c.position_anchor {
        Some(PositionAnchor::Named(n)) => Some(DefaultAnchor::Named(n.clone())),
        Some(PositionAnchor::Auto) => c.implicit_anchor.map(DefaultAnchor::Implicit),
        Some(PositionAnchor::Normal) | None if c.position_area.is_some() => {
            c.implicit_anchor.map(DefaultAnchor::Implicit)
        }
        _ => None,
    }
}
