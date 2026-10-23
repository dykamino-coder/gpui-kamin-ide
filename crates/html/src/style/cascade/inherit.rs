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

use crate::style::computed::Computed;
use crate::style::computed::box_predicates::{backdrop_root, establishes_cb};
use crate::style::values::value::Color;

mod containing;
mod context;
mod decorate;
mod flow_align;
mod paint;
mod quirks;
mod self_align;
use containing::inherit_containing;
use decorate::decorate;
pub(crate) use flow_align::note as note_flow_align;
use paint::inherit_paint;

// `zoom` (css-viewport-1) не делать домножением использованных длин здесь,
// в `inherit` (с `zoom_eff`/`scale_px` в `Computed`): гейт `zoom_eff != 1.0`
// не удерживает — домножение задевает общий путь длин (CSS2 `background-*`,
// таблицы, абсолюты). Только отдельным проходом после каскада
// (`style::zoom::resolve`), а не через самую горячую функцию крейта.
pub fn inherit(parent: &Computed, own: &Computed, vcx: ValueCx<'_>) -> Computed {
    let computed = super::compute::compute(parent, own, vcx);
    box_stage(parent, own, own, computed, true, vcx)
}

/// Document preparation propagates style; Filter Effects 1 §5 applies colors
/// only when the renderer builds the element, never while removing wrappers.
pub(crate) fn inherit_unpainted(parent: &Computed, own: &Computed, vcx: ValueCx<'_>) -> Computed {
    let computed = super::compute::compute(parent, own, vcx);
    box_stage(parent, own, own, computed, false, vcx)
}

/// Стиль коробки из вычисленного стиля `c` элемента под коробкой-родителем
/// со стилем `parent`: `own` — вычисленный стиль элемента с used-значениями
/// раскладки, `declared` — его каскадный стиль (заданы ли свои цвета).
pub(crate) fn box_stage(
    parent: &Computed,
    declared: &Computed,
    own: &Computed,
    mut c: Computed,
    paint_filter: bool,
    vcx: ValueCx<'_>,
) -> Computed {
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
