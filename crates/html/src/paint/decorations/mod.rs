//! Украшения коробки: тени, рамки по форме, слои градиентов.
mod borders;
pub(super) use borders::box_borders;
pub(super) use borders::coloured_borders;

// owner: A

use crate::paint::decorations::backdrop::backdrop_matrix;
use crate::paint::decorations::border_shape::border_shape_layer;
use crate::paint::decorations::gradient_stripes::gradient_stripes;
use crate::paint::decorations::shadows::{inset_shadows, outer_shadows};
use crate::paint::effects::mask::mask_def;
use crate::style::computed::Computed;
use gpui::{AnyElement, IntoElement};

mod backdrop;
mod border_shape;
mod gradient_stripes;
mod outline;
mod shadows;
pub(crate) mod text_shadows;

/// Слои, которые в GPUI выражаются только отдельным элементом.
///
/// Все — абсолютные и вне потока, поэтому на раскладку не влияют и могут
/// идти первыми детьми.
pub(crate) fn decorations(c: &Computed, empty: bool) -> Vec<AnyElement> {
    let mut out: Vec<AnyElement> = vec![];

    outer_shadows(c, &mut out);

    // `filter: url(#id)` на HTML-элементе (filter-effects-1 §filter
    // region): дешёвый путь для коробки без содержимого — SVG с `<rect>`
    // цвета фона и этим фильтром растрируется resvg и ложится слоем поверх
    // коробки; область — по умолчанию −10 %/120 % от border-box, поэтому
    // холст вдвое шире, а слой сдвинут на половину коробки. Фильтр над
    // готовым буфером группы — отдельная задача.
    // Только у коробки БЕЗ содержимого: над содержимым слой лёг бы поверх
    // детей (filter-region-transformed-composited-child-001).
    if empty
        && let Some(id) = c.filter_ref.as_deref()
        && let Some(def) = mask_def(&format!("filter:{id}"))
    {
        let fill = match c.background {
            Some(col) if col.a > 0.0 => format!(
                "rgba({},{},{},{})",
                (col.r * 255.0).round(),
                (col.g * 255.0).round(),
                (col.b * 255.0).round(),
                col.a
            ),
            _ => "none".to_string(),
        };
        out.push(
            crate::paint::effects::filter::FilterLayer {
                def,
                id: id.to_string(),
                fill,
            }
            .into_any_element(),
        );
    }
    // Фоновая картинка идёт первой: она поверх цвета фона и под всем
    // остальным — тот же порядок, что в браузере.
    // Несколько слоёв (css-backgrounds-3 §2.1): плитки каждого слоя своей
    // механикой, снизу вверх — первый в списке рисуется последним, поверх.
    if let Some(layers) = c.bg_layers() {
        for l in layers.iter().rev() {
            if let Some(layer) = crate::paint::background::layer(l) {
                out.push(layer);
            }
        }
    } else if let Some(layer) = crate::paint::background::layer(c) {
        out.push(layer);
    } else if c.gradient_as_tile() {
        // Градиент с размером/повтором/позицией — той же механикой плитки:
        // источник понимает записи `linear-gradient(...)`.
        let mut tiled = c.clone();
        tiled.bg_image = tiled.gradient_raw.clone();
        if let Some(layer) = crate::paint::background::layer(&tiled) {
            out.push(layer);
        }
    }

    inset_shadows(c, &mut out);

    // Рамка ПОВЕРХ слоя картинки (css-backgrounds-3 §3.7, прим.: «The
    // background is always drawn behind the border»; CSS 2.2 Прил. E; Blink
    // `PaintFillLayers` → `PaintBorder`). Квад рисует рамку ДО детей, а слой
    // плиток — ребёнок, и полупрозрачная/пунктирная рамка оказывалась ПОД
    // картинкой (`origin-border-box` 6.15, `css3-background-origin-*` 0.83).
    // Квад цвета не получает (`apply::apply_paint`); слой повторяет толщины,
    // стиль и скругление рамки и вынесен на толщину сторон — абсолютный
    // ребёнок отсчитывается от padding-box (как полосы сторон ниже).
    box_borders(c, &mut out);

    border_shape_layer(c, &mut out);

    // Рамка-картинка рисуется ПОВЕРХ фона и заменяет обычную рамку.
    if let Some(layer) = crate::paint::border_image::layer(c) {
        out.push(layer);
    }

    backdrop_matrix(c, &mut out);

    gradient_stripes(c, &mut out);

    out.extend(outline::decorations(c));

    // Разные цвета сторон рамки: у GPUI цвет рамки один на элемент, поэтому
    // несовпадающие стороны дорисовываются полосами поверх.
    coloured_borders(c, &mut out);
    out
}
