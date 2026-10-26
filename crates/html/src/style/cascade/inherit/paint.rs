//! Стадия коробки, краска: цветовой `filter` поддерева и drop-shadow, заливка текста под
//! background-clip: text.

use super::*;
use crate::document::state::ValueCx;
use crate::style::computed::BoxStyle;

mod gradient_filter;

pub(super) fn inherit_paint(
    parent: &Computed,
    declared: &Computed,
    own: &Computed,
    c: &mut BoxStyle,
    paint_filter: bool,
    vcx: ValueCx<'_>,
) {
    // Фильтр в CSS красит элемент И ВСЁ поддерево. Наследуем его сами и
    // применяем к собственным цветам потомка: раньше фильтр действовал только
    // на узел, где написан, и дети оставались цветными.
    if c.filter.is_none() {
        set!(c.filter, parent.filter);
        set!(c.scrollbar_filter_ops, parent.scrollbar_filter_ops.clone());
    }
    // ЕДИНСТВЕННАЯ точка окраски фильтром (каскад цвета не трогает).
    // Computed-цвет элемента ещё не окрашен, в том числе унаследованный.
    // Анонимная коробка получает уже окрашенный цвет от коробки-родителя:
    // у неё фильтруются только собственные объявления.
    if paint_filter && let Some(f) = c.filter {
        if own.background.is_some() {
            set!(c.background, c.background.map(|col| f.apply(col)));
        }
        if c.element_style.is_some()
            || declared.declares_color()
            || own.filter.is_some() && parent.color.is_none()
        {
            set!(c.color, c.color.map(|col| f.apply(col)));
        }
        // `currentcolor` сторон решится уже окрашенным цветом текста.
        for side in c.border_colors.iter_mut() {
            if let crate::style::values::value::StyleColor::Absolute(col) = side {
                *col = f.apply(*col);
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
        gradient_filter::apply(c, own, f, vcx);
        if own.bg_image.is_some()
            && let Some(image) = &mut c.bg_image
        {
            image.filter_stops(&f);
        }
        if !own.shadows.is_empty() {
            for sh in c.shadows.iter_mut() {
                if let crate::style::values::value::StyleColor::Absolute(col) = &mut sh.color {
                    *col = f.apply(*col);
                }
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
    if let Some(sh) = c.drop_shadow.clone()
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
    // Computed-цвет элемента смешивается на его коробке; анонимная коробка
    // получает уже смешанный цвет. Внепоточные потомки в геометрию текста
    // не входят (`clip-text-out-of-flow-child`) и сохраняют исходный цвет.
    let images = vcx.background_images();
    if paint_filter {
        crate::style::computed::flat_background::collapse_flat_stack(c, vcx, &images);
    }
    let colors = super::super::color_inputs::text_clip_colors(
        parent,
        declared,
        own,
        c,
        c.element_style.is_some(),
        &images,
    );
    set!(c.color, colors.color);
    set!(c.text_clip_raw, colors.raw);
    set!(c.text_clip_fill, colors.fill);
}
