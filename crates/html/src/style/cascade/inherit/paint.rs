//! inherit_stage, этап краски: filter и drop-shadow с цветом, заливка текста под background-clip: text, белые пробелы, курсор, таблицы, caret-color.

use super::*;

pub(super) fn inherit_paint(
    parent: &Computed,
    own: &Computed,
    c: &mut Computed,
    paint_filter: bool,
) {
    // Фильтр в CSS красит элемент И ВСЁ поддерево. Наследуем его сами и
    // применяем к собственным цветам потомка: раньше фильтр действовал только
    // на узел, где написан, и дети оставались цветными.
    if c.filter.is_none() {
        c.filter = parent.filter;
    }
    // ЕДИНСТВЕННАЯ точка окраски фильтром (каскад цвета не трогает).
    // Красится только ВОЗНИКШЕЕ на этом узле: унаследованный цвет уже
    // покрашен предком — повторная окраска давала f^N по поколениям.
    if paint_filter && let Some(f) = c.filter {
        if own.background.is_some() {
            c.background = c.background.map(|col| f.apply(col));
        }
        if own.color.is_some() || own.filter.is_some() && parent.color.is_none() {
            c.color = c.color.map(|col| f.apply(col));
        }
        if own.border_color.is_some() || own.border_color_is_current {
            c.border_color = c.border_color.map(|col| f.apply(col));
        }
        for (side, own_side) in c.border_colors.iter_mut().zip(own.border_colors.iter()) {
            if own_side.is_some() {
                *side = side.map(|col| f.apply(col));
            }
        }
        if own.gradient.is_some()
            && let Some(g) = c.gradient.as_mut()
        {
            g.from = f.apply(g.from);
            g.to = f.apply(g.to);
            for stop in g.stops.iter_mut() {
                stop.0 = f.apply(stop.0);
            }
        }
        // Растровый градиент (`conic`, `repeating-*`) живёт строкой в
        // `bg_image`: его стопы красятся в самой записи
        // (`filter-function-repeating-*-ref`: `filter: invert(1)` на фоне).
        if own.bg_image.is_some()
            && let Some(img) = c.bg_image.as_deref()
            && crate::style::computed::gradient_as_raster(img)
        {
            c.bg_image = Some(crate::style::computed::filter_gradient_text(img, &f));
        }
        if !own.shadows.is_empty() {
            for sh in c.shadows.iter_mut() {
                sh.color = f.apply(sh.color);
            }
        }
    }
    // `filter: drop-shadow()` у коробки со СПЛОШНЫМ фоном: силуэт такой
    // коробки — border-box со скруглением, и тень фильтра (filter-effects-1
    // §dropshadowEquivalent: размытая альфа входа, сдвиг, цвет — ПОД входом)
    // совпадает с внешней box-shadow без разлёта. Картинку поддерева так не
    // выразить — только коробку; повторное слияние тень не удваивает.
    // Длина размытия у `drop-shadow()` — это σ (filter-effects-1
    // §funcdef-filter-drop-shadow: «standard deviation»), а у `box-shadow`
    // радиус = 2σ (css-backgrounds-3 §box-shadow) — в список внешних теней
    // она идёт удвоенной, чтобы после деления в `apply::apply_paint` σ
    // осталась своей.
    if let Some(sh) = c.drop_shadow
        && c.background.is_some_and(|b| b.a >= 1.0)
    {
        let as_box = crate::style::computed::Shadow {
            blur: sh.blur * 2.0,
            ..sh
        };
        if !c.shadows.contains(&as_box) {
            c.shadows.push(as_box);
        }
    }
    // `background-clip: text` со СПЛОШНОЙ заливкой (css-backgrounds-4
    // §background-clip): фон виден только под глифами элемента и его
    // поточных и плавающих потомков, а сам текст красится ПОВЕРХ фона своим
    // цветом. Для одноцветного непрозрачного фона это ровно «цвет текста
    // поверх заливки» — маска глифов не нужна, а подчёркивания, многоточие и
    // знаки выделения цветом `currentColor` получают тот же цвет сами.
    // Смешивается только ВОЗНИКШЕЕ на узле (как у фильтра выше): унаследованный
    // цвет уже смешан предком. Внепоточные потомки в геометрию текста не входят
    // (`clip-text-out-of-flow-child`) и получают несмешанный цвет обратно.
    let black = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let out_of_flow = matches!(
        own.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    );
    if let Some(fill) = crate::paint::background::text_clip_fill(c) {
        c.text_clip_raw = c.color;
        c.text_clip_fill = Some(fill);
        c.color = Some(crate::paint::background::over(
            c.color.unwrap_or(black),
            fill,
        ));
    } else if parent.text_clip_fill.is_some() && out_of_flow {
        c.text_clip_fill = None;
        c.text_clip_raw = None;
        if own.color.is_none() {
            c.color = parent.text_clip_raw;
        }
    } else if let Some(fill) = parent.text_clip_fill {
        c.text_clip_fill = Some(fill);
        if own.color.is_some() {
            c.text_clip_raw = c.color;
            c.color = Some(crate::paint::background::over(
                c.color.unwrap_or(black),
                fill,
            ));
        } else {
            c.text_clip_raw = parent.text_clip_raw;
        }
    }
    // Наследуемые по CSS, но забытые прежде: без них `white-space: pre` на
    // контейнере не доходил до вложенного текста, а маркер, курсор и зазор
    // ячеек не доставались детям.
    c.preserve_newlines = own.preserve_newlines.or(parent.preserve_newlines);
    c.keep_spaces = own.keep_spaces.or(parent.keep_spaces);
    c.hidden = own.hidden.or(parent.hidden);
    c.cursor = own.cursor.clone().or(parent.cursor.clone());
    c.no_marker = own.no_marker.or(parent.no_marker);
    c.border_collapse = own.border_collapse.or(parent.border_collapse);
    c.empty_cells_hide = own.empty_cells_hide.or(parent.empty_cells_hide);
    c.border_spacing = own.border_spacing.or(parent.border_spacing);
    c.caret_color = own.caret_color.or(parent.caret_color);
}
