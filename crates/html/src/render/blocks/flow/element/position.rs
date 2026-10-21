//! Этап обработки позиционирования и слоя блочного ребёнка.

use super::place_element;
use crate::animation::animation_live::animated;
use crate::interactive::sticky::sticky_wrap;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::predicates::edge_set;
use crate::layout::writing_mode::vertical_hug;
use crate::render::blocks::flow::BelowRun;
use crate::render::blocks::inset_holder_box;
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
    // The canvas layer's measurement of this root box (`canvas_layer`).
    root_box: Option<crate::paint::background::RootMeasurement>,
) {
    let vcx = opts.cx.vcx();
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
    // max-height: fit-content` — 100, а не 200). `paint/apply/` такой
    // предел умеет только при `height` в точках и иначе пропускает.
    // Содержимое выше растяжения держатель не зажмёт — приближение.
    let auto_len = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
    let holder_axis = if positioned_out
        && e.style.writing_mode.is_none()
        && auto_len(e.style.height)
        && kw_len(e.style.max_height)
        && edge_set(e.style.inset.top)
        && edge_set(e.style.inset.bottom)
    {
        Some(true)
    } else if positioned_out
        // Письмо коробки держателю не мешает: размеры
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
    // Липкая коробка сдвигает своё поддерево на подготовке: щупы внутри
    // вычитают сдвиг (`ShiftLink`), поэтому звено заводится ДО сборки.
    let sticky_link = (e.style.position == Some(crate::style::computed::Position::Sticky))
        .then(|| crate::interactive::scroll_extent::ShiftLink::inside(opts.cx.sticky.as_ref()));
    let sticky_opts = sticky_link
        .as_ref()
        .map(|link| opts.inner(|cx| cx.sticky = Some(link.clone())));
    let outer_opts = opts;
    let opts = sticky_opts.as_deref().unwrap_or(opts);
    let built = if let Some(block_axis) = holder_axis {
        inset_holder_box(e, block_axis, inherited, opts, &kw_len)
    } else {
        crate::paint::effects::grouped::block_group(animated(e, inherited, opts), e, inherited, vcx)
    };
    let built = match root_box {
        Some(area) if e.style.root_box && e.style.canvas_bg => {
            crate::paint::background::record_root_box(
                built,
                area,
                &e.style,
                opts.base_size(),
                opts.cx.vcx(),
            )
        }
        _ => built,
    };
    let built = vertical_hug(built, e, inherited);
    let built = sticky_wrap(built, &e.style, frame, layer_ok, sticky_link);
    let opts = outer_opts;
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
