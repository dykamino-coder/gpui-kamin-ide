//! Стадия коробки, контекст родителя-коробки: признаки руби, повёрнутой строки и ортогонального
//! потока, контейнер запросов, сетка/flex родителя, предел строк `line-clamp`, строчные украшения
//! предка, подавление маркера, чернила тени текста.

use super::*;
use crate::style::computed::{BoxStyle, Display as D};

pub(super) fn inherit_context(parent: &Computed, own: &Computed, c: &mut BoxStyle) {
    set!(
        c.text_shadow_ink,
        own.text_shadow_ink.or(parent.text_shadow_ink)
    );
    set!(c.ruby_unit, own.ruby_unit || parent.ruby_unit);
    set!(
        c.ruby_suppress_breaks,
        own.ruby_suppress_breaks || parent.ruby_suppress_breaks
    );
    set!(c.turned_line, own.turned_line || parent.turned_line);
    set!(c.ortho_limit, own.ortho_limit.or(parent.ortho_limit));
    set!(c.cq_box, parent.container_box_for_children());
    set!(c.orthogonal_scrollport, parent.orthogonal_scrollport);
    // An absolutely positioned box sizes its inline axis against its own
    // containing block (CSS 2.1 §10.3.7 / §10.6.4 shrink-to-fit), never with
    // the in-flow inline measure of the paragraph it was written in.
    set!(
        c.orthogonal_inline,
        if matches!(
            own.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        ) {
            None
        } else {
            parent.orthogonal_inline
        }
    );
    // Сетка-родитель и её письмо: у вертикальной сетки оси выравнивания
    // элемента переставляются (`paint/apply/`), а по оси x идут группы базовых.
    set!(
        c.parent_grid,
        match parent.display {
            Some(D::Grid) | Some(D::InlineGrid) => {
                match (parent.wm().vertical(), parent.wm().block_rl()) {
                    (false, _) => 1,
                    (true, false) => 2,
                    (true, true) => 3,
                }
            }
            _ => 0,
        }
    );
    set!(c.parent_lanes, parent.display == Some(D::GridLanes));
    set!(c.parent_grid_align_items, parent.align_items);
    set!(
        c.parent_subgrid,
        c.parent_grid != 0 && (parent.subgrid_cols || parent.subgrid_rows)
    );
    set!(
        c.parent_flex_grid,
        matches!(
            parent.display,
            Some(D::Flex)
                | Some(D::InlineFlex)
                | Some(D::Grid)
                | Some(D::InlineGrid)
                | Some(D::GridLanes)
        )
    );
    // A `-webkit-box` that is not a block container (no vertical orient)
    // is no line-clamp container and hands no clamp to its contents
    // (`line-clamp-029`).
    set!(
        c.line_clamp,
        c.line_clamp
            .or(parent.line_clamp.filter(|_| parent.clamp_block_container()))
    );
    // Фон строчного бокса идёт вниз как текстовое свойство: он принадлежит
    // строке, а не коробке, и вложенный `<b>` внутри подсветки обязан его
    // сохранить.
    set!(c.inline_bg, own.inline_bg.or(parent.inline_bg));
    set!(c.inline_border, own.inline_border.or(parent.inline_border));
    set!(c.inline_pad, own.inline_pad.or(parent.inline_pad));
    set!(c.inline_radius, own.inline_radius.or(parent.inline_radius));
    set!(c.no_marker, own.no_marker.or(parent.no_marker));
    // css-ui-4 §6.1: `user-select` не наследуется; используемое значение
    // `auto` — `none` под коробкой с используемым `none`, иначе выделяемое
    // (`all` родителя тоже выделяемо).
    use crate::style::computed::UserSelect as U;
    set!(
        c.no_select,
        match c.user_select {
            Some(U::None) => Some(true),
            Some(U::Text | U::Contain | U::All) => Some(false),
            Some(U::Auto) | None => own.no_select.or(parent.no_select),
        }
    );
}
