//! Этап обработки позиционирования и слоя блочного ребёнка.

use super::place_element;
use crate::animation::animation_live::animated;
use crate::interactive::sticky::sticky_wrap;
use crate::layout::positioned::predicates::edge_set;
use crate::layout::writing_mode::vertical_hug;
use crate::paint::effects::grouped::grouped;
use crate::paint::effects::transform::transformed;
use crate::paint::stacking::{blends_inside, stacking_context};
use crate::render::blocks::flow::BelowRun;
use crate::render::*;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

#[allow(clippy::too_many_arguments)]
pub(crate) fn position_element(
    e: &crate::dom::Element,
    inherited: &Computed,
    opts: &RenderOpts,
    frame: &crate::interactive::sticky::element::StickyCell,
    layer_ok: bool,
    under_tf: bool,
    ordered_context: bool,
    geometry_layer_ok: bool,
    nodes: &[crate::dom::Node],
    idx: usize,
    paint_key: u64,
    out: &mut Vec<AnyElement>,
    below: &mut BelowRun,
    hoist_margins: bool,
    placement: (Option<gpui::GridLocation>, Option<gpui::GridLineNames>),
    record_root: bool,
) {
    let kw_len = |l: Option<Len>| {
        matches!(
            l,
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
        )
    };
    let positioned_out = matches!(
        e.style.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    );
    // Предел ключевым словом содержимого при АВТОМАТИЧЕСКОМ размере —
    // тот же держатель: css-sizing-3 §fit-content в блочной оси даёт
    // высоту содержимого, и used = min(растяжение краями, содержимое)
    // (`position-absolute-fit-content`: `top: 0; bottom: 0;
    // max-height: fit-content` — 100, а не 200). `apply.rs` такой
    // предел умеет только при `height` в точках и иначе пропускает.
    // Содержимое выше растяжения держатель не зажмёт — приближение.
    let auto_len = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
    let holder_axis = if positioned_out
        && e.style.vertical.is_none()
        && auto_len(e.style.height)
        && kw_len(e.style.max_height)
        && edge_set(e.style.inset.top)
        && edge_set(e.style.inset.bottom)
    {
        Some(true)
    } else if positioned_out
        // Письмо коробки держателю не мешает: после `280d0d4` размеры
        // ортогонального узла ложатся по СВОЕМУ письму, и у
        // `vertical-rl` `block-size: min-content` — физическая ширина
        // (css-writing-modes-4, Abstract-Physical Mapping). Гейт
        // `vertical.is_none()` ставился, пока размеры
        // транспонировались; без держателя ширина тянулась краями
        // `left/right` во всё окно (`div-{min,max,fit}-content-
        // orthogonal-*`: 13.5 %). Ветка высоты выше гейт сохраняет: в
        // вертикальном письме это строчная ось, её пары не разбирались.
        && auto_len(e.style.width) && kw_len(e.style.max_width)
        && edge_set(e.style.inset.left)
        && edge_set(e.style.inset.right)
    {
        Some(false)
    } else {
        None
    };
    let built = if let Some(block_axis) = holder_axis {
        inset_holder_box(e, block_axis, inherited, opts, &kw_len)
    } else if stacking_context(&e.style)
        && e.style.isolate != Some(true)
        && blends_inside(&e.children, 0)
    {
        // css-compositing-1 §mix-blend-mode: смешиваемый потомок
        // смешивается только с содержимым СВОЕГО контекста наложения.
        // Контекст обязан сложиться отдельной группой, иначе подложкой
        // становится весь кадр: белая страница вокруг родителя давала
        // красное кольцо (`-blended-element-with-transparent-pixels`),
        // lime вместо fuchsia (`-blended-with-3D-transform`). Blink —
        // `PaintLayer::HasNonIsolatedDescendantWithBlendMode`.
        let mut iso = e.style.clone();
        iso.isolate = Some(true);
        grouped(
            transformed(animated(e, inherited, opts), &e.style, inherited),
            &iso,
        )
    } else {
        grouped(
            transformed(animated(e, inherited, opts), &e.style, inherited),
            &e.style,
        )
    };
    let built = vertical_hug(built, e, inherited);
    let built = sticky_wrap(built, &e.style, frame, layer_ok);
    // Таблица сжимается по содержимому (§17.5.2.2), и выражено это у
    // нас гибким рядом. В контейнере с БЛОЧНОЙ раскладкой гибкого
    // ряда нет, `align_self` мёртв, и таблица растягивалась на всю
    // ширину родителя — видно на `<span style="display:block">` с
    // табличными детьми.
    let table_child = e.tag == "table"
        || matches!(
            e.style.display,
            Some(Display::Table) | Some(Display::InlineTable)
        );
    let block_parent = matches!(
        inherited.display,
        Some(Display::Block) | Some(Display::ListItem) | Some(Display::TableCell)
    );
    let built = if table_child && block_parent && e.style.width.is_none() {
        div().flex().flex_row().child(built).into_any_element()
    } else {
        built
    };
    // Абсолютный блок без заданных краёв стоит на СТАТИЧЕСКОЙ позиции —
    // там, где он оказался бы в потоке, а не в углу содержащего блока.
    // Пустышка нулевой высоты держит это место в потоке, элемент висит
    // от её угла. Без неё такой блок уезжал к началу родителя и
    // накрывал собой всё, что стояло выше.
    // Только в обычном потоке: в сетке и гибком контейнере пустышка
    // стала бы ЯЧЕЙКОЙ и сдвинула соседей, а по CSS абсолютный
    // ребёнок из раскладки родителя выключен.
    // Внепоточный элемент, которому не нашлось позиционированного
    // предка: его содержащий блок — область просмотра (§10.1 п.4), а
    // не родитель. Элемент строится НА СВОЁМ МЕСТЕ — наследование,
    // шрифт, письмо и маски остаются верными, — а готовый уходит
    // последним ребёнком документа, где края решит уже вьюпорт.
    //
    // Пока только при заданных ОБЕИХ осях: при пустой оси элемент
    // стоит на статической позиции, а её знает лишь раскладка.
    // Отрицательный `z-index` рисуется ПОД потоком, слой же идёт
    // последним — такие остаются на месте.
    // Внепоточный элемент, которому не нашлось позиционированного
    // предка: его содержащий блок — область просмотра (§10.1 п.4), а
    // не родитель. Элемент строится НА СВОЁМ МЕСТЕ (наследование,
    // шрифт, письмо и маски остаются верными), а готовый уходит
    // последним ребёнком документа, где края решает уже вьюпорт.
    //
    // Предок считается ВКЛЮЧАЯ непосредственного родителя:
    // `inherited.cb_ancestor` отвечает за предков строго выше него.
    // ЗАМЕРЕНО без этого слагаемого: CSS2 4636 -> 4626, все двенадцать
    // потерь — абсолют внутри `position: relative`-РОДИТЕЛЯ.
    //
    // Пока только при заданных обеих осях: при пустой оси элемент
    // стоит на статической позиции, а её знает лишь раскладка.
    // Отрицательный `z-index` рисуется ПОД потоком, слой же идёт
    // последним — такие остаются на месте.
    // Достаточно ОДНОЙ заданной оси: по ней край считает раскладка от
    // области просмотра, по пустой элемент стоит на СТАТИЧЕСКОЙ
    // позиции (§10.3.7, §10.6.4), и её сообщает щуп, оставшийся на
    // месте элемента. Ось задана, если задана хотя бы одна сторона.
    //
    // Внутри отложенного поддерева щуп готовится ПОЗЖЕ слоя, и дырка
    // была бы пуста — такие остаются на месте.
    place_element(
        e,
        under_tf,
        inherited,
        ordered_context,
        geometry_layer_ok,
        nodes,
        idx,
        built,
        layer_ok,
        paint_key,
        out,
        below,
        hoist_margins,
        placement,
        record_root,
        opts,
    )
}
