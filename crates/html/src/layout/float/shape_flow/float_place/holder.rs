//! Float content construction and absolute holder placement at its flow edge.

use crate::dom::Element;
use crate::layout::list::list_item;
use crate::layout::replaced::image::image;
use crate::paint::effects::grouped::grouped;
use crate::render::{RenderOpts, blocks, styled_div_with};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

#[allow(clippy::too_many_arguments)]
pub(super) fn push_float_holder(
    inherited: &Computed,
    opts: &RenderOpts,
    vert_lr: bool,
    line_left_bottom: bool,
    floats: &mut Vec<AnyElement>,
    f: &Element,
    side: i32,
    ml: f32,
    mr: f32,
    mt: f32,
    mb: f32,
    fy: f32,
    off: f32,
) {
    let mut copy = f.clone();
    // у бандового хоста `float` доживает до сюда, а слитый стиль с
    // `float` заводит лишний контекст обрезки.
    copy.style.float = None;
    copy.style.floated_box = true;
    copy.style.clear = None;
    let mut merged = inherit(inherited, &copy.style);
    merged.self_node = copy.node_id;
    // Поля кладёт держатель (позиция absolute от края) — на самой
    // коробке они сдвигали бы её обратно (float: right с margin-left
    // вылезал за правый край контейнера). Снимать их надо И СО СЛИТОГО
    // стиля: коробку строит он, и через него поле возвращалось —
    // четвёрка флоатов с `margin: 10px` уезжала на поле целиком
    // (`floats-014`).
    copy.style.margin = crate::style::computed::Sides::default();
    merged.margin = crate::style::computed::Sides::default();
    // Маска и обрезка формой живут в буфере группы (`grouped`): у флоата
    // с `shape-outside` этот путь был не пройден вовсе, и `clip-path`
    // на нём не резал НИЧЕГО — коробка рисовалась целым прямоугольником,
    // тогда как эталон (тот же флоат без `shape-outside`) идёт обычным
    // путём и маску получает. Стиль берётся с самой коробки (`copy.style`,
    // поля уже сняты выше — их несёт держатель), как на пути замещаемых
    // и внепоточных (`:7718`, `:7722`).
    let built = if copy.tag == "img" {
        grouped(image(&copy), &copy.style)
    } else if copy.style.display == Some(Display::ListItem) {
        // CSS Lists 3 §2: a floated list item keeps its marker.
        grouped(
            list_item::render_with_style(&copy, inherited, &merged, opts),
            &copy.style,
        )
    } else {
        grouped(
            styled_div_with(&copy, &merged)
                .children(blocks(&copy.children, &merged, opts))
                .into_any_element(),
            &copy.style,
        )
    };
    let holder = if inherited.vertical_rl == Some(true) || vert_lr {
        // Вертикальное письмо: блок-старт — ПРАВЫЙ край, колонки
        // флоатов идут влево; инлайн-старт — верх, а у float:right
        // (line-right) — НИЗ (css-writing-modes §7,
        // shape-outside-circle-049 и родня). У `vertical-lr` блок-старт —
        // левый край.
        let col = if vert_lr {
            div().absolute().left(px(off + ml))
        } else {
            div().absolute().right(px(off + mr))
        };
        if (side < 0) != line_left_bottom {
            col.top(px(mt))
        } else {
            col.bottom(px(mb))
        }
    } else if side < 0 {
        div().absolute().left(px(off + ml)).top(px(mt + fy))
    } else {
        div().absolute().right(px(off + mr)).top(px(mt + fy))
    };
    floats.push(holder.child(built).into_any_element());
}
