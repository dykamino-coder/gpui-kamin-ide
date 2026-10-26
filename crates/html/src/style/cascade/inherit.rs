//! Каскад: стиль коробки. Вычисленный стиль элемента (`compute::compute`)
//! дополняется контекстом родителя-КОРОБКИ (`box_stage`): признаки
//! содержащего блока и наложения, quirks-опора доли высоты, сетка/flex
//! родителя и самовыравнивание по нему, распространение линий оформления
//! (css-text-decor-3 §2.1 — по дереву коробок, не наследование), окраска
//! поддерева цветовым фильтром и заливка `background-clip: text`. Стадия
//! кладётся на свежую копию вычисленного стиля у каждой коробки.
//!
//! `inherit` — обе стадии для коробки без материализованного стиля:
//! анонимные коробки, коробки страниц и маркеров, слои псевдоэлементов
//! (CSS 2.1 §9.2.1.1: анонимная коробка наследует у своей коробки-родителя).

use crate::document::state::ValueCx;

use crate::style::computed::box_predicates::{backdrop_root, establishes_cb};
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Color;

/// Запись стадии коробки в общий вычисленный стиль — только изменившегося
/// значения: равное не копирует стиль коробки (P-18, копия при записи).
macro_rules! set {
    ($c:ident . $($f:ident).+, $v:expr) => {{
        let v = $v;
        if $c.$($f).+ != v {
            $c.$($f).+ = v;
        }
    }};
}

mod containing;
mod context;
mod decorate;
mod flow_align;
mod fragment;
mod marker;
mod paint;
mod quirks;
mod self_align;
use containing::inherit_containing;
use decorate::decorate;
pub(crate) use flow_align::note as note_flow_align;
pub(crate) use fragment::group as fragment_group_style;
pub(crate) use marker::content as marker_content;
pub(crate) use marker::symbol as marker_symbol;
use paint::inherit_paint;

// `zoom` (css-viewport-1) не делать домножением использованных длин здесь,
// в `inherit` (с `zoom_eff`/`scale_px` в `Computed`): гейт `zoom_eff != 1.0`
// не удерживает — домножение задевает общий путь длин (CSS2 `background-*`,
// таблицы, абсолюты). Только отдельным проходом после каскада
// (`style::zoom::resolve`), а не через самую горячую функцию крейта.
pub fn inherit(parent: &BoxStyle, own: &Computed, vcx: ValueCx<'_>) -> BoxStyle {
    let computed = super::compute::compute(parent, own, vcx);
    box_stage(parent, own, own, computed, None, true, vcx)
}

/// Document preparation propagates style; Filter Effects 1 §5 applies colors
/// only when the renderer builds the element, never while removing wrappers.
pub(crate) fn inherit_unpainted(parent: &BoxStyle, own: &Computed, vcx: ValueCx<'_>) -> BoxStyle {
    let computed = super::compute::compute(parent, own, vcx);
    box_stage(parent, own, own, computed, None, false, vcx)
}

/// Стиль коробки из вычисленного стиля `c` элемента под коробкой-родителем
/// со стилем `parent`: `own` — вычисленный стиль элемента с used-значениями
/// раскладки, `declared` — его каскадный стиль (заданы ли свои цвета).
pub(crate) fn box_stage(
    parent: &BoxStyle,
    declared: &Computed,
    own: &Computed,
    computed: std::rc::Rc<Computed>,
    element: Option<crate::style::computed::ElementStyle>,
    paint_filter: bool,
    vcx: ValueCx<'_>,
) -> BoxStyle {
    let mut c = BoxStyle::shared(computed);
    c.element_style = element;
    context::inherit_context(parent, own, &mut c);
    inherit_containing(parent, own, &mut c, vcx);
    quirks::percentage_height(parent, own, &mut c, vcx);
    self_align::self_alignment(parent, own, &mut c);
    flow_align::resolve(parent, &mut c);
    inherit_paint(parent, declared, own, &mut c, paint_filter, vcx);
    let parent_px = super::compute::parent_font_px(parent);
    let own_px = super::compute::own_font_px(&c, parent_px);
    decorate(parent, own, &mut c, own_px, vcx);
    c
}
