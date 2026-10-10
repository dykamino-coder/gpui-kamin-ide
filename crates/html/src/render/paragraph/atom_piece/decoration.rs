//! Обёртки atomic inline для transform, выравнивания и порядка краски.

use crate::dom::Element;
use crate::layout::positioned::static_position::at_static_position;
use crate::paint::effects::grouped::grouped;
use crate::paint::effects::transform::transformed;
use crate::render::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::inline;
use gpui::{IntoElement, ParentElement, Styled, div};

#[allow(clippy::too_many_arguments)]
pub(crate) fn decorate_atom(
    el: AnyElement,
    e: &Element,
    inherited: &Computed,
    original_margin: crate::style::computed::Sides,
    abs_cb: bool,
) -> inline::Piece {
    // `mix-blend-mode` на ЗАМЕЩАЕМОМ атоме строки: блочный путь,
    // атом с коробкой и флоат смешивают через `grouped`, а `<svg>`,
    // `<iframe>`, `<img>` в строке шли мимо, и режим пропадал
    // (`mix-blend-mode-svg`, `-iframe-parent`, `-iframe-sibling`:
    // красный квадрат вместо зелёного). Носитель несёт ТОЛЬКО режим:
    // маска и обрезка у атомов живут своими путями, полный стиль
    // применил бы их второй раз.
    let el = if replaced_tag(e) && e.style.blend.is_some_and(|b| b != 0) {
        let mut only = Computed::default();
        only.blend = e.style.blend;
        grouped(el, &only)
    } else {
        el
    };
    // Атомарный строчный — transformable element (css-transforms-1
    // §transformable-element: всё по блочной модели, «except for
    // non-replaced inline boxes»): `img`, `iframe`, `inline-block`,
    // `inline-table`, строчный `<svg>`. Обёртку получали только поля
    // форм — они и сейчас заворачиваются в `atom_element`, здесь их
    // пропускаем. Абсолюты через пустышку статической позиции не
    // трогаем: обёртка на нулевой пустышке взяла бы origin от нуля.
    let el = if matches!(
        e.tag.as_str(),
        "input" | "textarea" | "select" | "progress" | "meter"
    ) || matches!(
        e.style.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    ) {
        el
    } else {
        transformed(el, &e.style, inherited)
    };
    // `vertical-align` НА САМОМ куске (`img { vertical-align: top }`):
    // ряд строит базовую линию, а кускам с top/middle/bottom нужен
    // собственный прижим (wm-propagation-body-033-ref: полоса-картинка
    // в строке с квадратом прижата к верху, у нас висела на базовой).
    // ПРОБОВАЛИ И ОТКАТИЛИ: выражать `text-top`/`text-bottom` у
    // атомарного куска прижимом к краю строки. Замерено по семьям
    // linebox/*, css1/*, *vertical*: 0 и 0 — этим парам нужен сдвиг
    // относительно ТЕКСТОВОЙ области родителя, а не край строки.
    use crate::style::computed::Align;
    let self_align = match e.style.vertical_align {
        Some(Align::Start) => Some(gpui::AlignItems::FlexStart),
        Some(Align::End) => Some(gpui::AlignItems::FlexEnd),
        Some(Align::Center) => Some(gpui::AlignItems::Center),
        _ => None,
    };
    let el = match self_align {
        Some(a) => {
            let mut w = crate::style::apply::margins(div().flex_shrink_0(), &original_margin);
            w.style().align_self = Some(a);
            // Доля куска считается от его КОНТЕЙНЕРА, а обёртка встаёт
            // между ним и рядом: без своей ширины она сжимается по
            // содержимому, и `width: 100%` внутри разрешался в ноль —
            // картинка пропадала целиком (`background-repeat-002-ref`:
            // `img{vertical-align:top}` + `width="100%"`). Долю
            // повторяем на обёртке, чтобы отсчёт остался прежним.
            if let Some(Len::Pct(k)) = e.style.width {
                w = w.w(gpui::relative(k));
            }
            if let Some(Len::Pct(k)) = e.style.height {
                w = w.h(gpui::relative(k));
            }
            w.child(el).into_any_element()
        }
        None => el,
    };
    // Отрицательный `z-index` строчного замещаемого: краска уходит
    // ПОД содержимое до него (CSS 2.1 §9.9 шаг 3) — как у блочного
    // (background-size-document-root-vrl-*: красный маркер обязан
    // лечь под зелёный фон iframe).
    let el = if e.style.z_index.is_some_and(|z| z < 0)
        && e.style.position == Some(crate::style::computed::Position::Relative)
    {
        crate::paint::effects::underlay::Underlay::new(el).into_any_element()
    } else {
        el
    };
    // Абсолютная коробка с заданными краями места в строке не
    // занимает — `atom_element` вернул пустышку нулевого размера.
    // Атомом её отдавать нельзя: атом уводит абзац с текстового пути
    // в ряд, и содержащим блоком абсолюта становится коробка ВСЕГО
    // абзаца, а §10.1 п.4 требует прямоугольник фрагментов строчного
    // предка. `Overlay` абзац с текстового пути не уводит.
    if matches!(
        e.style.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    ) && !at_static_position(&e.style)
        && !matches!(
            e.tag.as_str(),
            "svg" | "img" | "canvas" | "video" | "embed" | "object" | "iframe"
        )
    {
        return inline::Piece::Overlay(
            el,
            inline::OverlayAt {
                edges: abs_cb,
                ..Default::default()
            },
        );
    }
    inline::Piece::Atom(el)
}
