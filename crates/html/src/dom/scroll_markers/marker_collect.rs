//! Marker collect for scroll_markers; split out to keep the owning module within 250 lines.

use crate::dom::*;
use crate::style::computed::{Display, Position};

/// Убрать все `::scroll-marker` поддерева (их скроллер без группы), не
/// заходя в уже собранные группы.
pub(super) fn purge_scroll_markers(nodes: &mut Vec<Node>) {
    nodes.retain(|n| !matches!(n, Node::Element(e) if e.tag == "::scroll-marker"));
    for n in nodes.iter_mut() {
        if let Node::Element(e) = n
            && e.tag != "::scroll-marker-group"
        {
            purge_scroll_markers(&mut e.children);
        }
    }
}

/// Вынуть `::scroll-marker` потомков в порядке документа (§scroll-markers:
/// «tree order of their originating element»): сперва свой маркер хозяина
/// (он лежит последним среди его детей), потом маркеры его потомков. Во
/// вложенные скроллеры не заходим — их маркеры уже собраны или погашены
/// ими самими; в готовые группы и псевдокоробки — тоже. Хозяин с
/// `position: absolute|fixed` считается, только если его содержащий блок
/// внутри скроллера (`abs_ok`/`fixed_ok`): иначе его коробка раскладки
/// лежит снаружи (`scroll-marker-005/006`), и его поддерево гасится.
pub(super) fn collect_scroll_markers(
    nodes: &mut [Node],
    abs_ok: bool,
    fixed_ok: bool,
    out: &mut Vec<Element>,
) {
    use crate::style::computed::Overflow;
    for n in nodes.iter_mut() {
        let Node::Element(e) = n else { continue };
        if e.tag.starts_with("::") {
            continue;
        }
        let inside = match e.style.position {
            Some(Position::Absolute) => abs_ok,
            Some(Position::Fixed) => fixed_ok,
            _ => true,
        };
        let mut kept = Vec::with_capacity(e.children.len());
        for c in e.children.drain(..) {
            match c {
                Node::Element(m) if m.tag == "::scroll-marker" => {
                    if inside {
                        out.push(m);
                    }
                }
                other => kept.push(other),
            }
        }
        e.children = kept;
        if !inside {
            purge_scroll_markers(&mut e.children);
            continue;
        }
        let nested = matches!(
            e.style.overflow_x,
            Some(Overflow::Scroll) | Some(Overflow::Hidden)
        ) || matches!(
            e.style.overflow_y,
            Some(Overflow::Scroll) | Some(Overflow::Hidden)
        );
        if nested || e.style.display == Some(Display::None) {
            continue;
        }
        let abs2 = abs_ok || crate::text::inline::establishes_cb(&e.style);
        let fixed2 = fixed_ok
            || e.style.transform.is_some()
            || e.style.contain_layout == Some(true)
            || e.style.contain_paint == Some(true);
        collect_scroll_markers(&mut e.children, abs2, fixed2, out);
    }
}
