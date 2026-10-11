//! Этап обработки позиционирования и слоя блочного ребёнка.

use crate::layout::positioned::static_position::at_static_position;
use crate::paint::stacking::{layered, stacking_context, z_index_applies};
use crate::render::blocks::flow::BelowRun;
use crate::render::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

#[allow(clippy::too_many_arguments)]
pub(crate) fn finish_element(
    e: &crate::dom::Element,
    y_set: bool,
    x_set: bool,
    inherited: &Computed,
    ordered_context: bool,
    below: &mut BelowRun,
    built: AnyElement,
    layer_ok: bool,
    under_tf: bool,
    nodes: &[crate::dom::Node],
    idx: usize,
    paint_key: u64,
    out: &mut Vec<AnyElement>,
    hoist_margins: bool,
    placement: (Option<gpui::GridLocation>, Option<gpui::GridLineNames>),
    record_root: bool,
    opts: &RenderOpts,
) {
    let below_cb_axis = e.style.position == Some(crate::style::computed::Position::Absolute)
        && e.style.z_index.is_some_and(|z| z < 0)
        && y_set
        && !x_set
        && !e.inline
        && inherited.rtl != Some(true)
        && inherited.vertical != Some(true)
        && e.style.vertical != Some(true)
        && matches!(
            inherited.position,
            Some(crate::style::computed::Position::Relative)
                | Some(crate::style::computed::Position::Absolute)
        )
        && crate::text::inline::establishes_cb(inherited)
        && !inherited.cb_ancestor
        && !stacking_context(inherited)
        && !ordered_context;
    let below_free_axis = e.style.position == Some(crate::style::computed::Position::Absolute)
        && e.style.z_index.is_some_and(|z| z < 0)
        && !(x_set && y_set)
        && !below_cb_axis;
    // ЗАМЕРЕНО И ОТКАЧЕНО: уводить в верхний слой ВСЯКУЮ абсолютную
    // коробку с одной свободной осью (§9.9 шаг 8) — по симметрии с
    // `below_free_axis`. Полный свод CSS2: приобретено 3, ПОТЕРЯНО
    // 165 (вся семья `vertical-align-0NN` уходит в «красное видно»,
    // `floats-wrap-bfc-outside-001` 0.08 -> 7.28). Распорка держит
    // место свободной оси только там, где элемент и так вне строки;
    // в абзаце она рвёт строку. Возвращаться только с настоящей
    // статической позицией внутри строки.
    if !ordered_context && (at_static_position(&e.style) || below_free_axis) {
        (below.start, below.end) = static_position_layer(
            e,
            inherited,
            built,
            layer_ok,
            under_tf,
            nodes,
            idx,
            paint_key,
            out,
            &mut below.zs,
            below.start,
            below.end,
        );
        return;
    }
    let _ = hoist_margins;
    // Замещаемому дорожка по содержимому не нужна: его размер по
    // ключевому слову — природный, считается в `image_with`.
    let layered_built = layered(built, &e.style, inherited, layer_ok, under_tf);
    let mut done = content_wrapper::for_element(layered_built, e, inherited, placement);
    // Тело под корнем-донором фона холста при `vertical-rl`: записать
    // левый край коробки корня для отрисовки холста (см. `canvas_paint`).
    // Корень по содержимому = margin-box тела плюс рамка и отбивка
    // корня слева.
    if record_root {
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let root_b = inherited.borders();
        let offset = side(e.style.margin.left) + side(inherited.padding.left) + side(root_b.left);
        done = crate::text::vertical::record_root_left(done, opts.doc_salt, offset);
    }
    // Корень vertical-rl прижат к ПРАВОМУ краю окна (§8.2 principal
    // flow): свой анкор-ряд вокруг ОДНОГО узла — соседей не трогает.
    // Корню с фоном-картинкой не ставится (гасил canvas-слой).
    // Прижим — свойство ГЛАВНОГО потока, а он на документ один. Если
    // обособление на `html` или на `body` погасило распространение
    // письма тела в область просмотра (css-contain-2
    // §containment-types), главным потоком тело не стало: оно
    // остаётся обычным блоком в потоке горизонтального корня и к
    // правому краю окна не жмётся
    // (contain-body-w-m-001..004, contain-html-w-m-001..004).
    if matches!(e.tag.as_str(), "html" | "body")
        && e.style.vertical_rl == Some(true)
        && !e.style.wm_contained
    {
        if e.style.bg_image.is_none() {
            done = div()
                .w_full()
                .flex()
                .justify_end()
                .child(done)
                .into_any_element();
        } else if let Some(Len::Px(w)) = e.style.width {
            // Корню с фоном-картинкой флекс-обёртка гасила слой
            // краски — прижим вправо считается сдвигом по известной
            // ширине (background-size-document-root-vrl-*).
            let shift = (opts.viewport.0 - w).max(0.0);
            if shift > 0.0 {
                done = div().ml(px(shift)).child(done).into_any_element();
            }
        }
    }
    // Релятивный элемент с отрицательным `z-index`: место в потоке —
    // своё, краска — под содержимым до него (CSS 2.1 §9.9, шаг 3).
    // Расширение на элементы сетки без `position` ЗАМЕРЕНО В МИНУС
    // (display-inline-grid 0.08 -> 8.20, inline-z-axis-002/004) —
    // подложка в строчной сетке рвёт свою же краску.
    // `<body>` исключён: его родитель — корневой элемент, а тот
    // всегда образует КОРНЕВОЙ контекст наложения. Шаги 1-2
    // приложения E — собственные фон и рамка корня, шаг 3 —
    // отрицательный `z-index` ПОВЕРХ них, а не под всем окном; братьев
    // у `<body>` нет, уходить не подо что
    // (`root-element-creates-stacking-context`).
    // Родитель — СВОЙ контекст наложения (transform, opacity < 1,
    // позиционированный с z-index, isolation, filter; CSS 2.1 прил. E,
    // css-transforms-1 §transform-rendering): отрицательный z-index
    // ребёнка ложится под его содержимое, но ПОВЕРХ его фона — не под
    // весь документ (`individual-transform/stacking-context-00*`,
    // `transform-stacking-001`).
    if e.style.z_index.is_some_and(|z| z < 0)
        && e.style.position == Some(crate::style::computed::Position::Relative)
        && e.tag != "body"
        && !stacking_context(inherited)
    {
        done = crate::paint::effects::underlay::Underlay::new(done).into_any_element();
    }
    // Абсолют с `z-index < 0`, оставленный на месте (`below_cb_axis`):
    // краска — шаг 3 корневого контекста, под потоком родителя, как и
    // у держателя на распорке выше.
    if below_cb_axis {
        done = crate::paint::effects::underlay::Underlay::new(done).into_any_element();
    }
    // Позиционированный блок с `z-index: auto | 0` рисуется на шаге 8
    // приложения E CSS 2.1 — ПОСЛЕ блоков и строк потока, — а у нас
    // порядок краски был порядком детей: следующий блок закрашивал
    // сдвинутый `relative` (`position-relative-035`) и абсолют с одной
    // свободной осью (`right-offset-003`). `PaintLast` меняет только
    // краску — раскладка и место в потоке те же, поэтому строки он не
    // рвёт (запись про распорку выше): внутри строки элементы идут
    // через `pending`, а не сюда. Положительный `z-index` и `fixed`
    // уже отложены `layered`, отрицательный — подложка выше.
    let step8 = matches!(
        e.style.position,
        Some(crate::style::computed::Position::Relative)
            | Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Sticky)
    ) && e.style.z_index.unwrap_or(0) == 0
        && !matches!(e.tag.as_str(), "html" | "body")
        && paint_last_ok(e, &nodes[idx + 1..]);
    // Непозиционированный элемент с `opacity` < 1 красится на том же слое, что
    // позиционированные с `z-index: 0` (css-color-4 §opacity: «painted
    // on the same layer … as positioned elements with stacking order
    // 0»; Blink кладёт такой слой в список z-порядка с нулём): после
    // блоков и строк потока, в порядке разметки (`t32-opacity-zorder-c`).
    let step8 = step8
        || (e.style.position.is_none_or(|p| p == crate::style::computed::Position::Static)
            // `z-index` у непозиционированного не действует
            // (CSS 2.1 §9.9.1 «Applies to: positioned elements»).
            && (e.style.z_index.unwrap_or(0) == 0
                || !z_index_applies(&e.style, inherited))
            // Только прозрачность: у `contain`/`will-change`/
            // `transform` положительный `z-index` потомков держится
            // на краске на месте (`contain-paint-stacking-context-*`).
            && (e.style.opacity.is_some_and(|o| o < 1.0)
                // css-transforms-1 §transform-rendering: a transformed
                // box establishes a stacking context and is painted
                // as a positioned `z-index: 0` layer (Blink puts it in
                // the z-order list with 0): `perspective-zero` — a
                // static transformed box after a `relative` one.
                || e.style.transform.is_some()
                || e.style.translate.is_some())
            && !e.style.z_index.is_some_and(|z| z > 0 && z_index_applies(&e.style, inherited))
            && !matches!(e.tag.as_str(), "html" | "body")
            && paint_last_ok(e, &nodes[idx + 1..]));
    if step8 {
        done = gpui::PaintLast::new(done).key(paint_key).into_any_element();
    }
    out.push(done);
}
