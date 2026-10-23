//! Этап обработки позиционирования и слоя блочного ребёнка.

use super::place_element;
use crate::layout::options::RenderOpts;
use crate::render::blocks::flow::BelowRun;

use gpui::AnyElement;
mod build;
mod defer;

#[allow(clippy::too_many_arguments)]
pub(crate) fn position_element(
    e: &crate::dom::Element,
    inherited: &crate::style::computed::BoxStyle,
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
    // The canvas layer's measurement of this root box (`canvas_layer`).
    root_box: Option<crate::paint::background::RootMeasurement>,
) {
    if defer::defer(
        e,
        inherited,
        opts,
        frame,
        layer_ok,
        under_tf,
        ordered_context,
        geometry_layer_ok,
        paint_key,
        root_box.clone(),
        out,
        &nodes[idx + 1..],
    ) {
        return;
    }
    let built = build::build(e, inherited, opts, frame, layer_ok, root_box);
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
    // Без этого слагаемого абсолют внутри `position: relative`-РОДИТЕЛЯ
    // уходил к вьюпорту.
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
