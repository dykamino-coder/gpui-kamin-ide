//! Статическое сведение `anchor()` до сборки дерева и якорь по умолчанию.

use crate::dom::Node;
use crate::style::computed::{Computed, PositionAnchor};
use crate::style::values::value::{Len, anchor_get};
use crate::values::ValueCx;
use std::collections::HashSet;

/// Статически неразрешимые `anchor()` решаются до отрисовки (§anchor-resolution:
/// неразрешимая функция «computes to its specified fallback value. If no
/// fallback value is specified, it makes the declaration … invalid at
/// computed-value time» — вставка становится `auto`, коробка остаётся на
/// статической позиции). Решать это на подготовке кадра поздно: слои и
/// щупы статической позиции выбираются при сборке дерева по `edge_set`.
/// Без этого шага две пары, зелёные сейчас, уходили бы в красное
/// (`position-anchor-none/normal-pseudo-element-implicit-002`).
pub fn settle_static(nodes: &mut [Node], vcx: ValueCx<'_>) {
    fn names(nodes: &[Node], out: &mut HashSet<String>) {
        for n in nodes {
            let Node::Element(e) = n else { continue };
            if let Some(list) = &e.style.used().reset_positioned.anchor_name {
                out.extend(list.iter().cloned());
            }
            names(&e.children, out);
        }
    }
    fn settle(nodes: &mut [Node], known: &HashSet<String>, parent: &Parent, vcx: ValueCx<'_>) {
        for n in nodes.iter_mut() {
            let Node::Element(e) = n else { continue };
            if e.style.used().reset_positioned.position_anchor == Some(PositionAnchor::MatchParent)
            {
                let (pa, implicit) = match_parent(parent);
                e.style.used_mut().reset_positioned.position_anchor = Some(pa);
                if implicit.is_some() {
                    e.style.used_mut().reset_positioned.implicit_anchor = implicit;
                }
            }
            let has_default = match &e.style.used().reset_positioned.position_anchor {
                Some(PositionAnchor::Named(n)) => known.contains(n),
                Some(PositionAnchor::Auto) => {
                    e.style.used().reset_positioned.implicit_anchor.is_some()
                }
                // `normal` при непустой `position-area` ведёт себя как `auto`
                // (§position-anchor); без области — как `none`.
                Some(PositionAnchor::Normal) | None => {
                    e.style.used().reset_positioned.position_area.is_some()
                        && e.style.used().reset_positioned.implicit_anchor.is_some()
                }
                _ => false,
            };
            let inset = e.style.used().reset_positioned.inset;
            // §anchor-resolution меняет только anchor(): обычные вставки не
            // требуют отдельной копии стиля и сохраняют ключ materialization.
            if [inset.top, inset.right, inset.bottom, inset.left]
                .into_iter()
                .any(|l| matches!(l, Some(Len::Anchor(_))))
            {
                let s = &mut e.style.used_mut().reset_positioned.inset;
                for slot in [&mut s.top, &mut s.right, &mut s.bottom, &mut s.left] {
                    *slot = settle_len(*slot, has_default, known, vcx);
                }
            }
            let abs = matches!(
                e.style.used().reset_positioned.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            );
            let me: Parent = abs.then(|| {
                (
                    e.style.used().reset_positioned.position_anchor.clone(),
                    e.style.used().reset_positioned.position_area.is_some(),
                    e.style.used().reset_positioned.implicit_anchor,
                )
            });
            settle(&mut e.children, known, &me, vcx);
        }
    }
    let mut known = HashSet::new();
    names(nodes, &mut known);
    settle(nodes, &known, &None, vcx);
}

/// Одна вставка: цепочка запасных значений раскручивается, пока не
/// найдётся разрешимая функция или обычная длина.
fn settle_len(
    l: Option<Len>,
    has_default: bool,
    known: &HashSet<String>,
    vcx: ValueCx<'_>,
) -> Option<Len> {
    let mut cur = l;
    for _ in 0..4 {
        let Some(Len::Anchor(i)) = cur else {
            return cur;
        };
        let f = anchor_get(i, vcx)?;
        // У `min()`/`max()` разрешимы должны быть ВСЕ доводы.
        let resolvable = std::iter::once(&*f)
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
    match &c.reset_positioned.position_anchor {
        Some(PositionAnchor::Named(n)) => Some(DefaultAnchor::Named(n.clone())),
        Some(PositionAnchor::Auto) => c
            .reset_positioned
            .implicit_anchor
            .map(DefaultAnchor::Implicit),
        Some(PositionAnchor::Normal) | None if c.reset_positioned.position_area.is_some() => c
            .reset_positioned
            .implicit_anchor
            .map(DefaultAnchor::Implicit),
        _ => None,
    }
}

/// Якорь по умолчанию родителя (или порождающего элемента псевдоэлемента)
/// для `match-parent`: `position-anchor`, есть ли `position-area` и неявный
/// якорь; `None` — родитель не абсолютный, свойство к нему не применяется.
pub(super) type Parent = Option<(Option<PositionAnchor>, bool, Option<u64>)>;

/// `position-anchor: match-parent` (§position-anchor): тот же якорь по
/// умолчанию, что у родителя — для псевдоэлемента у порождающего элемента;
/// нет его — якоря нет (`none`). Родитель сам с `match-parent` уже сведён
/// (обход сверху вниз). Неявный якорь родителя переходит вторым полем.
pub(super) fn match_parent(parent: &Parent) -> (PositionAnchor, Option<u64>) {
    let Some((pa, area, implicit)) = parent else {
        return (PositionAnchor::None, None);
    };
    match pa {
        Some(PositionAnchor::Named(n)) => (PositionAnchor::Named(n.clone()), None),
        Some(PositionAnchor::Auto) if implicit.is_some() => (PositionAnchor::Auto, *implicit),
        Some(PositionAnchor::Normal) | None if *area && implicit.is_some() => {
            (PositionAnchor::Auto, *implicit)
        }
        _ => (PositionAnchor::None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::{Element, Marks};
    use crate::style::computed::{ResetPositioned, Sides, StyleGroup};
    use std::rc::Rc;

    #[test]
    fn ordinary_insets_keep_cascaded_and_materialized_storage() {
        let doc = crate::values::DocumentValues::detached(Default::default());
        let seed = Computed {
            reset_positioned: StyleGroup::new(ResetPositioned {
                inset: Sides {
                    top: Some(Len::Px(7.)),
                    right: Some(Len::Pct(25.)),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..doc.initial_style()
        };
        for materialized in [false, true] {
            let element = Element::synthetic("div", seed.clone(), Marks::default(), Vec::new());
            let mut nodes = vec![Node::Element(element)];
            if materialized {
                crate::style::cascade::materialize::materialize(
                    &nodes,
                    &Rc::new(doc.initial_style()),
                    doc.values(),
                );
            }
            let Node::Element(element) = &nodes[0] else {
                panic!("element")
            };
            let before = element.style.computed();
            settle_static(&mut nodes, doc.values());
            let Node::Element(element) = &nodes[0] else {
                panic!("element")
            };
            assert!(Rc::ptr_eq(&before, &element.style.computed()));
            assert!(element.style.used_rc().is_none());
            assert!(
                seed.reset_positioned
                    .ptr_eq(&element.style.cascaded().reset_positioned)
            );
        }
    }

    #[test]
    fn missing_anchor_still_resolves_fallback_without_mutating_source() {
        let doc = crate::values::DocumentValues::detached(Default::default());
        let seed = Computed {
            reset_positioned: StyleGroup::new(ResetPositioned {
                inset: Sides {
                    top: Len::parse("anchor(--missing top, 7px)", doc.values()),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..doc.initial_style()
        };
        assert!(matches!(
            seed.reset_positioned.inset.top,
            Some(Len::Anchor(_))
        ));
        let mut nodes = vec![Node::Element(Element::synthetic(
            "div",
            seed.clone(),
            Marks::default(),
            Vec::new(),
        ))];
        settle_static(&mut nodes, doc.values());
        let Node::Element(element) = &nodes[0] else {
            panic!("element")
        };
        assert_eq!(
            element.style.used().reset_positioned.inset.top,
            Some(Len::Px(7.))
        );
        assert!(matches!(
            seed.reset_positioned.inset.top,
            Some(Len::Anchor(_))
        ));
    }
}
