//! Прогон атомов в хосте обтекания формой.

use crate::dom::{Element, Node};
use crate::layout::float::band_host::px_margin_box;
use crate::layout::float::float_atom::band_atom;
use crate::render::{RenderOpts, inline_level};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement};

#[allow(clippy::result_large_err)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn atoms_flow(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    vert_lr: bool,
    line_left_bottom: bool,
    rest: &Vec<Node>,
    shapes: std::sync::Arc<(
        Vec<super::super::super::shapes::FloatShape>,
        Vec<super::super::super::shapes::FloatShape>,
    )>,
    host: gpui::Div,
    mut atoms: Vec<crate::layout::fragment::types::FlowChild>,
    mut atoms_ok: bool,
) -> Result<
    AnyElement,
    (
        gpui::Div,
        std::sync::Arc<(
            Vec<super::super::super::shapes::FloatShape>,
            Vec<super::super::super::shapes::FloatShape>,
        )>,
    ),
> {
    for n in rest {
        match n {
            Node::Text(t) => {
                if !t.trim().is_empty() {
                    atoms_ok = false;
                    break;
                }
            }
            Node::Element(c) => {
                let inline_box = matches!(
                    c.style.display,
                    Some(Display::InlineBlock) | Some(Display::InlineFlex)
                ) || (c.tag == "img" && inline_level(c));
                // Размер атома — MARGIN-box: эталон
                // `floats-wrap-top-below-003l-ref` держится на
                // `margin-top: 25px; margin-right: 250px` у второй коробки, а
                // без полей она встаёт вплотную и уезжает на 25 точек вверх.
                let dims = px_margin_box(&c.style);
                // Пустая коробка без размеров — разделитель разметки
                // (незакрытый div в хвосте) — просто пропускается.
                let empty = !inline_box
                    && c.children
                        .iter()
                        .all(|n| matches!(n, Node::Text(t) if t.trim().is_empty()))
                    && dims == Some((0.0, 0.0))
                    && c.style.background.is_none();
                if empty {
                    continue;
                }
                match (inline_box, dims) {
                    (true, Some((w, h))) if w > 0.0 && h > 0.0 => {
                        atoms.push(band_atom(c, inherited, opts).unwrap());
                    }
                    _ => {
                        atoms_ok = false;
                        break;
                    }
                }
            }
        }
    }
    if atoms_ok && !atoms.is_empty() {
        let rtl = inherited.rtl == Some(true);
        // Вертикальное письмо (`vertical-rl`, `sideways-rl`): формы уже
        // построены в осях письма (`background::shape_profile_block`) —
        // индекс равен расстоянию от блок-старта (правого края), значение —
        // экстенту вдоль физической вертикали от своей line-стороны.
        // Транспонировать их второй раз нечего.
        //
        // Прежний `transpose` схлопывал `Profile` в полосу максимального
        // экстента (`w: max(ext)`) — то есть терял форму целиком, а её несут
        // ВСЕ произвольные фигуры: `inset` с `round`, `polygon`, `path()`,
        // `shape()`, слово-коробка с `border-radius`, картинка, градиент.
        // У `Band` он вдобавок не менял оси местами: `h` брался из высоты
        // margin-box, хотя по блок-оси лежит его ШИРИНА.
        //
        // Сторона сохраняется отдельными списками: `float: left` — line-left
        // = верх, `float: right` — line-right = низ (css-writing-modes-4
        // §6.3), и от `direction` это не зависит. А `direction: rtl`
        // разворачивает инлайн-ось, и коробки идут от НИЖНЕГО края — эталоны
        // семейства (`shape-outside-inset-023-ref` и родня) меряют свой
        // `inset-inline-start` именно снизу.
        if inherited.vertical_rl == Some(true) || vert_lr {
            let mut row =
                crate::layout::fragment::types::FlowRow::new(atoms, shapes, rtl).vertical_rl();
            if vert_lr {
                row = row.block_lr();
            }
            // `sideways-lr`: инлайн-ось идёт СНИЗУ вверх (line-left — низ,
            // css-writing-modes-4 §6.3), строки ряда кладутся от нижнего края.
            if line_left_bottom {
                row = row.inline_up();
            }
            if let Some(Len::Px(v)) = e.style.height {
                row = row.inline_limit(v);
            }
            return Ok(host.child(row).into_any_element());
        }
        return Ok(host
            .child(crate::layout::fragment::types::FlowRow::new(
                atoms, shapes, rtl,
            ))
            .into_any_element());
    }
    Err((host, shapes))
}
