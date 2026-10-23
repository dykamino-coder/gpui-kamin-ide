//! Сборка дерева: разрыв строчных коробок вокруг блочного потомка
//! (CSS 2.1 §9.2.1.1). Строчный элемент с блоком внутри становится тройкой
//! «анонимный блок | блок | анонимный блок»; продолжения вложенных строчных
//! остаются в той же анонимной коробке вокруг настоящего блока, а не вокруг
//! каждого продолжения. Вынесенный блок получает сдвиг, прозрачность и слой
//! хозяина; боковые поля/рамки/отбивки хозяина делятся между первым и
//! последним фрагментом (`box-decoration-break: slice`).

mod containing;
pub(crate) use containing::restore_containing_block;
mod edges;
use edges::drop_inline_side;
mod flush;
#[cfg(test)]
mod tests;
pub(super) use flush::flush_inline;

use crate::dom::Node;
use crate::layout::box_kind::{breaks_inline, contains_block, out_of_flow, real_inline};
use crate::layout::table::anon::anon_element;
use crate::style::values::value::Len;

/// Разорвать строчные, внутри которых лежит блок (CSS 2.1 §9.2.1.1).
///
/// Строчный элемент с блочным потомком превращается в тройку «анонимный
/// блок | блок | анонимный блок»: строчное содержимое до и после блока
/// остаётся в своих анонимных коробках, сам блок встаёт между ними. Подряд
/// идущие блоки в отдельные анонимные коробки не заворачиваются.
pub(crate) fn split_block_in_inline(nodes: &[Node]) -> Vec<Node> {
    split_block_in_inline_dir(nodes, false)
}

/// [`split_block_in_inline`] with the parent's `direction` known: an inline
/// that does not set its own inherits it, and its start edge follows it.
pub(crate) fn split_block_in_inline_dir(nodes: &[Node], parent_rtl: bool) -> Vec<Node> {
    let need = nodes.iter().any(|n| match n {
        Node::Element(e) => {
            real_inline(e) && !out_of_flow(&e.style.computed()) && contains_block(&e.children)
        }
        Node::Text(_) => false,
    });
    if !need {
        return nodes.to_vec();
    }
    let mut out: Vec<Node> = vec![];
    for node in nodes {
        let Node::Element(e) = node else {
            out.push(node.clone());
            continue;
        };
        if !real_inline(e) || out_of_flow(&e.style.computed()) || !contains_block(&e.children) {
            out.push(node.clone());
            continue;
        }
        // Куски строчного содержимого копят стиль хозяина: анонимная коробка
        // своего оформления не имеет, а спан внутри неё — имеет.
        let mut piece: Vec<Node> = vec![];
        // Край строчного со стороны `side` (1 — правый, 3 — левый): рамка,
        // отбивка или поле ненулевой толщины.
        let has_edge = |side: usize| {
            let nz = |l: Option<Len>| matches!(l, Some(Len::Px(v)) if v.abs() > 0.0);
            let st = &*e.style.computed();
            let (b, p, m) = if side == 1 {
                (st.borders().right, st.padding.right, st.margin.right)
            } else {
                (st.borders().left, st.padding.left, st.margin.left)
            };
            nz(b) || nz(p) || nz(m)
        };
        // Horizontal fragments keep their logical start/end edges in either direction.
        let own_rtl = e.style.cascaded().rtl.unwrap_or(parent_rtl);
        let edges = e
            .style
            .cascaded()
            .writing_mode
            .is_none()
            .then_some(if own_rtl { (1, 3) } else { (3, 1) });
        // `first`: кусок до первого блока, `last`: после последнего.
        // Positions in `out` of this element's inline pieces (for edge slicing below).
        let mut hosts: Vec<usize> = vec![];
        let flush = |piece: &mut Vec<Node>,
                     out: &mut Vec<Node>,
                     hosts: &mut Vec<usize>,
                     first: bool,
                     last: bool| {
            flush_inline(piece, out, hosts, first, last, e, edges, has_edge)
        };
        // Split inner ancestors first, then join their inline continuations
        // to this ancestor's fragments on either side of the actual block.
        let kids = split_block_in_inline_dir(&e.children, own_rtl);
        let mut first = true;
        for child in &kids {
            // CSS 2.1 section 9.2.1.1 breaks inline ancestors around the
            // actual block, not around each descendant's continuation.
            if let Node::Element(anon) = child
                && anon.tag == "anon-block"
            {
                piece.extend(anon.children.iter().cloned());
                continue;
            }
            if breaks_inline(child) {
                flush(&mut piece, &mut out, &mut hosts, first, false);
                first = false;
                // Относительный сдвиг строчного хозяина переносится на
                // вынесенный блок (§9.2.1.1: разрыв не отменяет смещения).
                let mut block = match child {
                    Node::Element(c) => c.clone(),
                    // breaks_inline возвращает false для Node::Text.
                    Node::Text(_) => unreachable!("блоком бывает только элемент"),
                };
                // Метка выноса: объёмный контекст и перспектива деда на блок
                // не действуют (`Computed::hoisted_block`, `transformed`).
                block.style.used_mut().hoisted_block = true;
                containing::hoist(&mut block, e);
                // Разрыв коробок не меняет наследование стилей (CSS 2 §9.2.1.1).
                // Сохраняем свойства маркера до выноса блока из родителя.
                let (own, host, decl) = (
                    block.style.computed(),
                    e.style.computed(),
                    e.style.cascaded(),
                );
                let list_type = block.style.cascaded().list_style_type.clone();
                let inside = block
                    .style
                    .cascaded()
                    .list_style_inside
                    .or(decl.list_style_inside);
                let used = block.style.used_mut();
                used.list_style_type = list_type.or_else(|| decl.list_style_type.clone());
                used.list_style_image = own
                    .list_style_image
                    .clone()
                    .or_else(|| host.list_style_image.clone());
                used.list_style_inside = inside;
                used.no_marker = own.no_marker.or(host.no_marker);
                used.marker_text = own.marker_text.clone().or_else(|| host.marker_text.clone());
                // Сам блок ПОЗИЦИОНИРОВАН: его собственные края нельзя ни
                // заменить, ни сложить с чужими (у хозяина они бывают в долях,
                // у блока — в точках). Сдвиг хозяина накладывается ОБЁРТКОЙ:
                // каждый слой решает свою долю от того же содержащего блока
                // (`position-relative-001/002`), а `fixed` едет вместе со
                // своей статической позицией (`-003`).
                let host_shift = e.style.computed().position
                    == Some(crate::style::computed::Position::Relative)
                    && (e.style.computed().inset.left.is_some()
                        || e.style.computed().inset.top.is_some());
                let wrap_shift = host_shift
                    && matches!(
                        block.style.computed().position,
                        Some(crate::style::computed::Position::Relative)
                            | Some(crate::style::computed::Position::Absolute)
                            | Some(crate::style::computed::Position::Fixed)
                    );
                if host_shift && !wrap_shift {
                    block.style.used_mut().position =
                        Some(crate::style::computed::Position::Relative);
                    if block.style.computed().inset.left.is_none() {
                        block.style.used_mut().inset.left = e.style.computed().inset.left;
                    }
                    if block.style.computed().inset.top.is_none() {
                        block.style.used_mut().inset.top = e.style.computed().inset.top;
                    }
                }
                // `inherit` на размере вынесенного блока брал бы значение уже
                // не у хозяина, а у его родителя: разрыв делает блок БРАТОМ
                // хозяина. Значение забирается здесь, пока связь ещё видна.
                use crate::style::css::LonghandId as L;
                if block.style.cascaded().inherit_longhands.contains(L::Width) {
                    block.style.used_mut().width = e.style.computed().width;
                    block
                        .style
                        .used_mut()
                        .inherit_longhands
                        .set(L::Width, false);
                }
                if block.style.cascaded().inherit_longhands.contains(L::Height) {
                    block.style.used_mut().height = e.style.computed().height;
                    block
                        .style
                        .used_mut()
                        .inherit_longhands
                        .set(L::Height, false);
                }
                // Прозрачность и слой хозяина действуют на ВЕСЬ разорванный
                // элемент, включая вынесенный блок: раньше блок был куском
                // строки и получал их заодно с ней.
                if e.style.computed().opacity.is_some() && block.style.computed().opacity.is_none()
                {
                    block.style.used_mut().opacity = e.style.computed().opacity;
                }
                if e.style.computed().z_index.is_some() && block.style.computed().z_index.is_none()
                {
                    block.style.used_mut().z_index = e.style.computed().z_index;
                }
                if wrap_shift {
                    let mut shifter = anon_element("anon-relshift", vec![Node::Element(block)]);
                    shifter.style.used_mut().position =
                        Some(crate::style::computed::Position::Relative);
                    shifter.style.used_mut().inset.left = e.style.computed().inset.left;
                    shifter.style.used_mut().inset.top = e.style.computed().inset.top;
                    out.push(Node::Element(shifter));
                    continue;
                }
                out.push(Node::Element(block));
                continue;
            }
            piece.push(child.clone());
        }
        flush(&mut piece, &mut out, &mut hosts, first, !first);
        // The pieces are fragments of ONE inline box: its start margin,
        // border and padding go on the first fragment only, its end ones on
        // the last (CSS 2.1 §9.2.1.1 with §8.6, css-break-3 §5.4
        // `box-decoration-break: slice`; `split-inline-borders`).
        let n = hosts.len();
        for (k, &at) in hosts.iter().enumerate() {
            let Some(Node::Element(anon)) = out.get_mut(at) else {
                continue;
            };
            let Some(Node::Element(host)) = anon.children.first_mut() else {
                continue;
            };
            // `box-decoration-break: clone` keeps every side on every fragment.
            if host.style.computed().bdb_clone {
                continue;
            }
            // Physical sides: 1 = right, 3 = left.
            let (start, end) = if own_rtl { (1, 3) } else { (3, 1) };
            if k > 0 {
                drop_inline_side(host.style.used_mut(), start);
            }
            if k + 1 < n {
                drop_inline_side(host.style.used_mut(), end);
            }
        }
    }
    // Проверено: сливать прогон между разрывами в ОДНУ анонимную коробку
    // (§9.2.1.1 обнимает всю строчную коробку, а не только куски разорванного
    // строчного; братья по бокам идут голыми) ничего не меняет — `blocks()`
    // и без склейки собирает такой прогон одним абзацем (`block-in-inline-*`,
    // `inline-box-001`, `abspos-029`, `text-indent-014`). Разница эталонов
    // лежит не в числе анонимных коробок.
    out
}
