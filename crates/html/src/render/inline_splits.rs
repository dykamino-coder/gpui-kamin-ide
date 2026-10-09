//! Keep nested inline continuations in the same anonymous block around a real block.

use super::{anon_element, breaks_inline, contains_block, out_of_flow, real_inline};
use crate::dom::Node;
use crate::value::Len;

/// Разорвать строчные, внутри которых лежит блок (CSS 2.1 §9.2.1.1).
///
/// Строчный элемент с блочным потомком превращается в тройку «анонимный
/// блок | блок | анонимный блок»: строчное содержимое до и после блока
/// остаётся в своих анонимных коробках, сам блок встаёт между ними. Подряд
/// идущие блоки в отдельные анонимные коробки не заворачиваются.
pub(crate) fn split_block_in_inline(nodes: &[Node]) -> Vec<Node> {
    let need = nodes.iter().any(|n| match n {
        Node::Element(e) => real_inline(e) && !out_of_flow(&e.style) && contains_block(&e.children),
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
        if !real_inline(e) || out_of_flow(&e.style) || !contains_block(&e.children) {
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
            let st = &e.style;
            let (b, p, m) = if side == 1 {
                (st.borders().right, st.padding.right, st.margin.right)
            } else {
                (st.borders().left, st.padding.left, st.margin.left)
            };
            nz(b) || nz(p) || nz(m)
        };
        // Сторона письма известна только своему стилю: при rtl/вертикали
        // прежнее поведение (пустой кусок пропадает).
        let ltr = e.style.rtl != Some(true) && e.style.vertical.is_none();
        // `first`: кусок до первого блока, `last`: после последнего.
        // Positions in `out` of this element's inline pieces (for edge slicing below).
        let mut hosts: Vec<usize> = vec![];
        let flush = |piece: &mut Vec<Node>,
                     out: &mut Vec<Node>,
                     hosts: &mut Vec<usize>,
                     first: bool,
                     last: bool| {
            // Кусок из одних схлопываемых пробелов коробки не создаёт —
            // иначе он рисовал бы фон и рамку строчного на пустом месте.
            let blank = piece.iter().all(|n| match n {
                Node::Text(t) => t.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n')),
                Node::Element(_) => false,
            });
            if piece.is_empty() || blank {
                piece.clear();
                // …кроме первого и последнего куска со своим краем: строка с
                // пустой строчной коробкой, у которой есть рамка, отбивка или
                // поле по строчной оси, не пуста (CSS 2.1 §9.4.2), а разрыв
                // §9.2.1.1 оставляет начальный край первому куску, конечный —
                // последнему (`block-in-inline-whitespace-001a`: синяя черта
                // `border-left` над первым блоком и `border-right` под вторым).
                let drop = if first && ltr && has_edge(3) {
                    Some(1)
                } else if last && ltr && has_edge(1) {
                    Some(3)
                } else {
                    None
                };
                if let Some(drop) = drop {
                    let mut host = e.clone();
                    host.children = Vec::new();
                    host.style.border_visible[drop] = Some(false);
                    if drop == 1 {
                        host.style.border_width.right = None;
                        host.style.padding.right = None;
                        host.style.margin.right = None;
                    } else {
                        host.style.border_width.left = None;
                        host.style.padding.left = None;
                        host.style.margin.left = None;
                    }
                    hosts.push(out.len());
                    out.push(Node::Element(anon_element(
                        "anon-block",
                        vec![Node::Element(host)],
                    )));
                }
                return;
            }
            let mut host = e.clone();
            host.children = std::mem::take(piece);
            hosts.push(out.len());
            out.push(Node::Element(anon_element(
                "anon-block",
                vec![Node::Element(host)],
            )));
        };
        // Split inner ancestors first, then join their inline continuations
        // to this ancestor's fragments on either side of the actual block.
        let kids = split_block_in_inline(&e.children);
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
                    Node::Text(_) => unreachable!("блоком бывает только элемент"),
                };
                // Метка выноса: объёмный контекст и перспектива деда на блок
                // не действуют (`Computed::hoisted_block`, `transformed`).
                block.style.hoisted_block = true;
                // Сам блок ПОЗИЦИОНИРОВАН: его собственные края нельзя ни
                // заменить, ни сложить с чужими (у хозяина они бывают в долях,
                // у блока — в точках). Сдвиг хозяина накладывается ОБЁРТКОЙ:
                // каждый слой решает свою долю от того же содержащего блока
                // (`position-relative-001/002`), а `fixed` едет вместе со
                // своей статической позицией (`-003`).
                let host_shift = e.style.position == Some(crate::computed::Position::Relative)
                    && (e.style.inset.left.is_some() || e.style.inset.top.is_some());
                let wrap_shift = host_shift
                    && matches!(
                        block.style.position,
                        Some(crate::computed::Position::Relative)
                            | Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                    );
                if host_shift && !wrap_shift {
                    block.style.position = Some(crate::computed::Position::Relative);
                    if block.style.inset.left.is_none() {
                        block.style.inset.left = e.style.inset.left;
                    }
                    if block.style.inset.top.is_none() {
                        block.style.inset.top = e.style.inset.top;
                    }
                }
                // `inherit` на размере вынесенного блока брал бы значение уже
                // не у хозяина, а у его родителя: разрыв делает блок БРАТОМ
                // хозяина. Значение забирается здесь, пока связь ещё видна.
                if block.style.width_inherit {
                    block.style.width = e.style.width;
                    block.style.width_inherit = false;
                }
                if block.style.height_inherit {
                    block.style.height = e.style.height;
                    block.style.height_inherit = false;
                }
                // Прозрачность и слой хозяина действуют на ВЕСЬ разорванный
                // элемент, включая вынесенный блок: раньше блок был куском
                // строки и получал их заодно с ней.
                if e.style.opacity.is_some() && block.style.opacity.is_none() {
                    block.style.opacity = e.style.opacity;
                }
                if e.style.z_index.is_some() && block.style.z_index.is_none() {
                    block.style.z_index = e.style.z_index;
                }
                if wrap_shift {
                    let mut shifter = anon_element("anon-relshift", vec![Node::Element(block)]);
                    shifter.style.position = Some(crate::computed::Position::Relative);
                    shifter.style.inset.left = e.style.inset.left;
                    shifter.style.inset.top = e.style.inset.top;
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
            if host.style.bdb_clone {
                continue;
            }
            let rtl = host.style.rtl == Some(true);
            // Physical sides: 1 = right, 3 = left.
            let (start, end) = if rtl { (1, 3) } else { (3, 1) };
            if k > 0 {
                drop_inline_side(&mut host.style, start);
            }
            if k + 1 < n {
                drop_inline_side(&mut host.style, end);
            }
        }
    }
    // ПРОБОВАЛИ И ОТКАТИЛИ: сливать прогон между разрывами в ОДНУ анонимную
    // коробку (§9.2.1.1 обнимает всю строчную коробку, а не только куски
    // разорванного строчного; братья по бокам идут голыми). Проход-склейка
    // поверх `out` при условии «в прогоне есть анонимная коробка и больше
    // одного непробельного узла» на 108 парах семей `block-in-inline-*`,
    // `inline-box-001`, `abspos-029`, `text-indent-014` не сдвинул НИ ОДНОЙ:
    // `blocks()` и без склейки собирает такой прогон одним абзацем. Разница
    // эталонов лежит не в числе анонимных коробок.
    out
}

/// Remove the margin, border and padding of one physical inline side
/// (1 = right, 3 = left) of a fragment of a split inline box.
fn drop_inline_side(style: &mut crate::computed::Computed, side: usize) {
    let zero = Some(crate::value::Len::Px(0.0));
    let pick = |s: &mut crate::computed::Sides| {
        if side == 1 {
            s.right = zero;
        } else {
            s.left = zero;
        }
    };
    pick(&mut style.margin);
    pick(&mut style.padding);
    pick(&mut style.border_width);
    style.border_visible[side] = Some(false);
}
