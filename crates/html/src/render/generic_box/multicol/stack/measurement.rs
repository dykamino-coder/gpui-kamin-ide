//! Измерение блочных детей и вложенных многоколоночников.

use crate::dom::{Element, Node};
use crate::layout::block::struts::zero_len;
use crate::layout::float::block_like_float;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::line_shape::{
    inline_content, nested_rows_box, nested_rows_shape, resolved_lengths, transpose_tree,
};
use crate::layout::fragment::probe::forced_inside;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::render::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(crate) fn measure_children(
    ge: &Element,
    merged: &Computed,
    opts: &RenderOpts,
    first_flow: Option<*const Node>,
    last_flow: Option<*const Node>,
    outer_frag: Option<f32>,
    line_col_w: Option<f32>,
    nested_auto: &std::cell::RefCell<Vec<u64>>,
    whole_ok: bool,
    nested_whole: &std::cell::RefCell<Vec<u64>>,
    measured_kids: &std::cell::RefCell<Vec<(u64, f32)>>,
    col_vert: bool,
    col_rl: bool,
    measure_ok: bool,
) -> Option<Vec<(Element, Shape)>> {
    ge.children
        .iter()
        .filter(|n| !is_blank(n))
        .filter(|n| !matches!(n, Node::Element(c) if out_of_flow(&c.style)))
        .map(|n| match n {
            // Вложенный многоколоночник `height: auto` с верха
            // внешней колонки — рядами (`nested_rows_shape`).
            Node::Element(c)
                if first_flow == Some(n as *const Node)
                    && matches!(c.style.height, None | Some(Len::Auto))
                    && zero_len(c.style.margin.top)
                    && outer_frag.is_some()
                    && line_col_w.is_some()
                    && nested_rows_box(c) =>
            {
                let c = &resolved_lengths(c, merged);
                let (Some(hh), Some(cw)) = (outer_frag, line_col_w) else {
                    return None;
                };
                match nested_rows_shape(c, merged, hh, cw, opts) {
                    Some(h) => {
                        nested_auto.borrow_mut().push(c.node_id);
                        Some(((*c).clone(), h))
                    }
                    None => shape_full(c, 4, ShapeCx::COLUMNS).map(|h| ((*c).clone(), h)),
                }
            }
            // Вложенный многоколоночник `height: auto` во внешних колонках с
            // заданной высотой: его блочный размер — СБАЛАНСИРОВАННЫЕ колонки
            // (css-multicol-1 §7.1), а не сумма детей, которую даёт
            // `shape_full`. Высоту даёт раскладка копии (`StackChild::
            // measure`), разрез — краем внешней колонки (`multicol-nested-
            // 022…024`: внутренний 200 в двух колонках — коробка 100, а не 200,
            // и следующий за ним блок уезжал в третью внешнюю колонку).
            // Первый в потоке и внутренний `column-fill: auto` — прежними
            // путями (рядами; `multicol-fill-balance-003`, `multicol-nested-013`).
            Node::Element(c)
                if measure_ok
                    && first_flow != Some(n as *const Node)
                    && c.style.column_fill_auto != Some(true)
                    && matches!(c.style.height, None | Some(Len::Auto))
                    && line_col_w.is_some()
                    && nested_rows_box(c) =>
            {
                let c = resolved_lengths(c, merged);
                measured_nested(&c, line_col_w?, measured_kids, nested_whole)
            }
            Node::Element(c)
                if whole_ok
                    && matches!(c.style.height, None | Some(Len::Auto))
                    && line_col_w.is_some()
                    && nested_rows_box(c) =>
            {
                // Высоту даёт раскладка самой копии (`StackChild::measure`):
                // внутренний многоколоночник балансирует себя сам, и мера
                // `shape_full` его колонок не видит (сумма детей).
                let c = resolved_lengths(c, merged);
                let w = line_col_w?;
                let m = |l: &Option<Len>| match l {
                    Some(Len::Px(v)) => Some(*v),
                    None | Some(Len::Auto) => Some(0.0),
                    _ => None,
                };
                let (mt, mb) = (m(&c.style.margin.top)?, m(&c.style.margin.bottom)?);
                nested_whole.borrow_mut().push(c.node_id);
                measured_kids.borrow_mut().push((c.node_id, w));
                Some((c, (0.0, mt, mb, Vec::new(), Vec::new(), Vec::new())))
            }
            // `position: relative` укладке не мешает — сдвиг
            // накладывается на месте (корень A1).
            Node::Element(c)
                if !c.inline
                    && (c.style.position.is_none()
                        || c.style.position
                            == Some(crate::style::computed::Position::Relative))
                    && (c.style.float.unwrap_or(0) == 0 || block_like_float(&c.style)) =>
            {
                // В вертикальном письме мера — по БЛОЧНОЙ оси
                // (css-multicol-1 §2: «The column height is the
                // length of the column box in the block
                // direction»): поддерево меряется ПОВЁРНУТЫМ
                // клоном (`transpose_tree`), рисуется исходным.
                // Длины коробки — в точках (`resolved_lengths`):
                // и мере, и копиям, и распоркам роста — одно дерево.
                let mut c = resolved_lengths(c, merged);
                // `text-box-trim` многоколоночника режет его ПЕРВУЮ и
                // ПОСЛЕДНЮЮ отформатированную строку (css-inline-3
                // §4.2) — у строчного блока-ребёнка с края потока они
                // его же. Метка, а не флаг стиля: срез на разрывах
                // решает ближайшая коробка со своим флагом
                // (`brk_trim`), и флаг ребёнка отнял бы его у хоста
                // (`text-box-trim-multicol-005`).
                if inline_content(&c) {
                    if merged.text_box_trim_start && first_flow == Some(n as *const Node) {
                        c.attrs.push(("kamin-host-trim-start".into(), "1".into()));
                    }
                    if merged.text_box_trim_end && last_flow == Some(n as *const Node) {
                        c.attrs.push(("kamin-host-trim-end".into(), "1".into()));
                    }
                }
                let c = &c;
                if col_vert {
                    let t = transpose_tree(c, col_rl)?;
                    shape_full(&t, 4, ShapeCx::COLUMNS).map(|h| ((*c).clone(), h))
                } else {
                    shape_full(c, 4, ShapeCx::COLUMNS)
                        .map(|h| ((*c).clone(), h))
                        .or_else(|| {
                            // Мера `shape_full` не выразила ребёнка (флоаты,
                            // внепоточные потомки, таблица со сросшимися
                            // рамками …). Прежде отказ ОДНОГО ребёнка
                            // отправлял весь многоколоночник в сетку без
                            // фрагментации — содержимое вовсе не переходило
                            // в следующую колонку. css-break-3 §4: любая
                            // блочная коробка фрагментируема; её высоту даёт
                            // сама раскладка копии в колонку (`StackChild::
                            // measure`, Blink меряет ребёнка тем же
                            // алгоритмом, что и кладёт), а разрез — по краю
                            // колонки (`slice`, без точек класса A).
                            // Только колонки с заданной высотой
                            // (`column-fill: auto`): у баланса высота
                            // коробки сама зависит от меры, а раскладка
                            // копии флоаты в высоту не берёт (а коробка
                            // многоколоночника — корень контекста — берёт).
                            // Принудительного разрыва внутри мера без
                            // точек тоже не видит.
                            if !measure_ok || forced_inside(c, 6) {
                                return None;
                            }
                            let w = line_col_w?;
                            let m = |l: &Option<Len>| match l {
                                Some(Len::Px(v)) => Some(*v),
                                None | Some(Len::Auto) => Some(0.0),
                                _ => None,
                            };
                            let (mt, mb) = (m(&c.style.margin.top)?, m(&c.style.margin.bottom)?);
                            measured_kids.borrow_mut().push((c.node_id, w));
                            Some((
                                (*c).clone(),
                                (0.0, mt, mb, Vec::new(), Vec::new(), Vec::new()),
                            ))
                        })
                }
            }
            _ => None,
        })
        .collect()
}

/// Ребёнок-многоколоночник, чью высоту меряет раскладка копии.
fn measured_nested(
    c: &Element,
    w: f32,
    measured_kids: &std::cell::RefCell<Vec<(u64, f32)>>,
    nested_whole: &std::cell::RefCell<Vec<u64>>,
) -> Option<(Element, Shape)> {
    let m = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => Some(*v),
        None | Some(Len::Auto) => Some(0.0),
        _ => None,
    };
    let (mt, mb) = (m(&c.style.margin.top)?, m(&c.style.margin.bottom)?);
    nested_whole.borrow_mut().push(c.node_id);
    measured_kids.borrow_mut().push((c.node_id, w));
    Some((c.clone(), (0.0, mt, mb, Vec::new(), Vec::new(), Vec::new())))
}
