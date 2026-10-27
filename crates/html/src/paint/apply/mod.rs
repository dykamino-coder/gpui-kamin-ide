//! Вычисленный стиль → элемент GPUI. Здесь и проходит граница охвата.
//!
//! Правило одно: если свойство выразимо примитивами GPUI — применяем; если нет
//! — не применяем НИЧЕГО вместо него. Приблизительная замена (нарисовать
//! `filter: blur` полупрозрачностью, `inset`-тень внешней) выглядит как рабочая
//! поддержка и стоит дороже честного пропуска: расхождение всплывает у
//! пользователя, а не в тесте.

use crate::document::state::ValueCx;
use crate::style::computed::{Align, BoxStyle, Computed, Gradient};
use crate::style::values::value::Len;
use gpui::{Div, InteractiveElement, ParentElement, Styled, px, relative};
mod contained_intrinsic;
pub(crate) mod intrinsic_size;
use crate::paint::apply::contained_intrinsic::empty_contained_size;
mod flex_cross_default;
pub(super) mod grid;
mod grid_flow_axes;
mod inset_percent;
mod size_percent;
pub(crate) use crate::paint::apply::grid::*;
pub(super) mod layout;
use crate::paint::apply::layout::*;
pub(super) mod box_model;
pub use crate::paint::apply::box_model::*;
pub(super) mod paint;
pub(crate) use crate::paint::apply::paint::*;
mod text;
pub use crate::paint::apply::text::*;
use crate::style::computed::FontCx;

/// Ширина/высота/отступ: доля родителя или пиксели.
pub(crate) fn len_to_gpui(l: Len, vcx: ValueCx<'_>) -> gpui::DefiniteLength {
    match l {
        Len::Px(v) => px(v).into(),
        Len::Pct(v) => relative(v),
        // Сюда шрифтовые единицы доходят только у узлов вне наследования
        // (элементы форм, корень) — запасные значения даёт единая точка.
        l @ (Len::Em(_)
        | Len::EmPx(..)
        | Len::Ch(_)
        | Len::Ic(_)
        | Len::Ex(_)
        | Len::Lh(_)
        | Len::LhPx(..)) => px(l.to_px(None, FontCx::FALLBACK, vcx).unwrap_or(0.0)).into(),
        // Единицы окна разрешает сборщик дерева; сюда они доходят только у
        // узлов вне его — доля родителя ближе всего по смыслу.
        Len::Vw(k) | Len::Vh(k) => relative(k),
        // Смешанный остаток calc (обычно px+%): честно ляжет только в
        // taffy-calc (фаза 2); пока — процентная часть, при её отсутствии
        // точечная (ближе, чем прежний сброс всего объявления).
        Len::Calc(i) => {
            let s = crate::style::values::value::calc_get(i, vcx);
            match s.pct_px() {
                // Доля с точками — настоящий calc раскладки (css-values-4
                // §10.9): KaminIDE patch `DefiniteLength::Calc` + решатель в
                // taffy: обе составляющие длины должны сохраниться до раскладки.
                Some((pct, add)) => gpui::DefiniteLength::Calc(add, pct),
                None if s.pct != 0.0 => relative(s.pct),
                None => px(s.px).into(),
            }
        }
        // `auto` в размере значит «пусть решает раскладка» — это отсутствие
        // ограничения, а не значение; вызывающий такие поля не применяет.
        // Размер по содержимому — то же самое: его ставит обёртка-сетка
        // (`render::content_sized`), а не длина.
        // `anchor()` во вставке: раскладке отдаётся НОЛЬ — коробка встаёт к
        // краю содержащего блока, а сдвиг до края якоря считает
        // `anchor::AnchorPlace` на подготовке кадра; там же от этого нуля
        // отсчитывается и запасное значение `anchor(left, 20px)`.
        Len::Anchor(_) => px(0.0).into(),
        Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent => relative(1.0),
    }
}

/// Стиль наведения: `.btn:hover { … }`.
///
/// Отдельная функция, потому что GPUI принимает состояние наведения не
/// цепочкой методов, а правкой стиля в замыкании. Поддержано подмножество,
/// которое и встречается в наведении: цвет, фон, рамка, прозрачность, вес и
/// начертание шрифта. Отступы и размеры в наведении менять нельзя — это
/// сдвинуло бы раскладку под курсором.
pub fn apply_hover(d: Div, hover: &Computed, own: &BoxStyle) -> Div {
    let background = own.background_paint.paints();
    let h = hover.clone();
    d.hover(move |mut s| {
        if background && let Some(bg) = h.background {
            s.background = Some(gpui::Fill::Color(bg.to_hsla().into()));
        }
        if background && let Some(g) = &h.gradient {
            s.background = Some(gpui::Fill::Color(fill(g)));
        }
        if let Some(bc) = h.used_uniform_border_color() {
            s.border_color = Some(bc.to_hsla());
        }
        if let Some(o) = h.opacity {
            s.opacity = Some(o);
        }
        if let Some(col) = h.color {
            s.text.color = Some(col.to_hsla());
        }
        if let Some(w) = h.font_weight {
            s.text.font_weight = Some(gpui::FontWeight(w as f32));
        }
        if h.italic == Some(true) {
            s.text.font_style = Some(gpui::FontStyle::Italic);
        }
        s
    })
}

/// Заливка градиентом: радиальный — своим тегом (патч GPUI), линейный —
/// парой крайних стопов; промежуточные рисует сборщик дерева полосами.
pub fn fill(g: &Gradient) -> gpui::Background {
    let last = g.stops.len().saturating_sub(1);
    let (from, to) = (
        gpui::linear_color_stop(
            g.from.to_hsla(),
            g.stops.first().map(|s| s.1).unwrap_or(0.0),
        ),
        gpui::linear_color_stop(
            g.to.to_hsla(),
            g.stops.get(last).map(|s| s.1).unwrap_or(1.0),
        ),
    );
    let base = if g.radial {
        gpui::radial_gradient(from, to, g.circle)
    } else {
        gpui::linear_gradient(g.angle_deg, from, to)
    };
    // Пространство смешения (css-color-4 §12.2). GPU-путь выражает два:
    // гамма-sRGB и OKLab — шейдер переводит цвета в вершинном и обратно
    // после смешения. Прочие пространства сюда не доходят: `gradient_as_tile`
    // уводит их на растровый путь, где цвет считается на точку.
    let base = match g.space {
        crate::style::computed::GradSpace::Oklab => base.color_space(gpui::ColorSpace::Oklab),
        _ => base,
    };
    // Промежуточные цвета: до четырёх стопов заливка несёт сама (патч GPUI),
    // сверх того сборщик дерева по-прежнему кладёт полосы.
    if g.stops.len() > 2 {
        let stops: Vec<gpui::LinearColorStop> = g
            .stops
            .iter()
            .take(4)
            .map(|(c, p)| gpui::linear_color_stop(c.to_hsla(), *p))
            .collect();
        return base.with_stops(&stops);
    }
    base
}

/// `align-self` в раскладку. `last baseline` — отдельный вариант: группа
/// последних базовых прижимается к концу оси (css-align-3 §9.3); прежде
/// `last` сводился к первой базовой.
pub fn self_align(a: Align, last: bool) -> gpui::AlignItems {
    match a {
        Align::Center | Align::AnchorCenter => gpui::AlignItems::Center,
        Align::Start => gpui::AlignItems::FlexStart,
        Align::End => gpui::AlignItems::FlexEnd,
        Align::Baseline if last => gpui::AlignItems::LastBaseline,
        Align::Baseline => gpui::AlignItems::Baseline,
        Align::Stretch => gpui::AlignItems::Stretch,
    }
}

#[path = "alignment_axes.rs"]
mod alignment_axes;
mod grid_content_baseline;
pub(crate) mod item_metadata;

pub fn apply(d: Div, c: &BoxStyle, vcx: ValueCx<'_>) -> Div {
    let mut d = apply_for_decorations(d, c, vcx);
    if let Some(layer) = crate::paint::background::color_layer(c, vcx) {
        d = d.child(layer);
    }
    d
}

/// Полная сборка декораций вставляет общий цвет после backdrop и до картинок.
/// Остальные держатели получают тот же слой непосредственно через `apply`.
pub(crate) fn apply_for_decorations(d: Div, c: &BoxStyle, vcx: ValueCx<'_>) -> Div {
    apply_for_decorations_with_dimensions(d, c, None, vcx)
}

pub(crate) fn apply_for_decorations_with_dimensions(
    d: Div,
    c: &BoxStyle,
    dimensions: Option<crate::layout::block::sizing::BoxDimensions>,
    vcx: ValueCx<'_>,
) -> Div {
    let mut d = d;
    d = match dimensions {
        Some(sizes) => apply_layout_with_dimensions(d, c, sizes, vcx),
        None => apply_layout(d, c, vcx),
    };
    // CSS 2.1 §10.8.1: an inline-block's baseline is that of its last in-flow
    // line box. A float is out of flow, so it never gives the enclosing
    // inline-block its baseline (Blink `PropagateBaselineFromBlockChild`
    // ignores floats), whatever row or block wraps it.
    if c.floated_box || c.float.is_some_and(|f| f != 0) {
        d.style().no_inline_block_baseline = Some(true);
    }
    if c.baseline_orthogonal {
        d.style().baseline_unavailable = Some(true);
    }
    d = apply_box(d, c, vcx);
    d = apply_paint(d, c, vcx);
    apply_text(d, c, vcx)
}

#[cfg(test)]
#[path = "tests.rs"]
pub(crate) mod tests;
