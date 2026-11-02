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

use crate::values::ValueCx;

use crate::style::computed::box_predicates::{backdrop_root, establishes_cb};
use crate::style::computed::{BoxStyle, Computed};

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
pub use flow_align::note as note_flow_align;
pub use fragment::group as fragment_group_style;
pub use marker::content as marker_content;
pub use marker::symbol as marker_symbol;
use paint::inherit_paint;

// `zoom` (css-viewport-1) не делать домножением использованных длин здесь,
// в `inherit` (с `zoom_eff`/`scale_px` в `Computed`): гейт `zoom_eff != 1.0`
// не удерживает — домножение задевает общий путь длин (CSS2 `background-*`,
// таблицы, абсолюты). Только отдельным проходом после каскада
// (`style::zoom::resolve`), а не через самую горячую функцию крейта.
pub fn inherit(parent: &BoxStyle, own: &Computed, vcx: ValueCx<'_>) -> BoxStyle {
    let computed = super::compute::compute(parent, own, vcx).legacy;
    box_stage(
        BoxParents::same(parent),
        own,
        own,
        computed,
        None,
        true,
        vcx,
    )
}

/// Document preparation propagates style; Filter Effects 1 §5 applies colors
/// only when the renderer builds the element, never while removing wrappers.
pub fn inherit_unpainted(parent: &BoxStyle, own: &Computed, vcx: ValueCx<'_>) -> BoxStyle {
    let computed = super::compute::compute(parent, own, vcx).legacy;
    box_stage(
        BoxParents::same(parent),
        own,
        own,
        computed,
        None,
        false,
        vcx,
    )
}

/// Каскад ребёнка снятой обёртки `<html>`/`<body>` (подготовка документа):
/// значения обёртки наследуются, а её `filter`, которому больше негде
/// рисоваться (Filter Effects 1 §5: фильтр обрабатывает отрисовку элемента
/// с потомками), переходит на содержимое; собственный фильтр ребёнка сильнее.
pub fn inherit_unwrapped(wrapper: &BoxStyle, own: &Computed, vcx: ValueCx<'_>) -> Computed {
    let mut merged = inherit_unpainted(wrapper, own, vcx).into_computed();
    let neutral = crate::style::computed::Filter::neutral();
    if merged.reset_effects.filter.is_none_or(|f| f == neutral) {
        merged.reset_effects.filter = wrapper.reset_effects.filter;
        merged.reset_effects.scrollbar_filter_ops =
            wrapper.reset_effects.scrollbar_filter_ops.clone();
    }
    merged
}

/// Родители коробки на стадии коробки. У коробки, которую раскладка перенесла
/// под другую коробку (флоат из строчного, поднятый абсолют), они различаются:
/// `layout` — новая коробка-родитель, её содержащий блок и контекст
/// форматирования и выравнивания (CSS 2 §9.5, §10.1); `tree` — коробка
/// родителя элемента по дереву элементов, от которой идёт распространяемое по
/// этому дереву: окраска поддерева фильтром (Filter Effects 1 §5), используемый
/// `user-select` (css-ui-4 §6.1), руби, повёрнутая строка, чернила тени и линии
/// оформления (css-text-decor-3 §2.1).
#[derive(Clone, Copy)]
pub struct BoxParents<'a> {
    pub layout: &'a BoxStyle,
    pub tree: &'a BoxStyle,
}

impl<'a> BoxParents<'a> {
    /// Коробка на своём месте: родитель по дереву коробок и по дереву элементов один.
    pub fn same(parent: &'a BoxStyle) -> Self {
        Self {
            layout: parent,
            tree: parent,
        }
    }
}

/// Стиль коробки из вычисленного стиля `c` элемента под родителями `parents`:
/// `own` — вычисленный стиль элемента с used-значениями раскладки,
/// `declared` — его каскадный стиль (заданы ли свои цвета).
pub fn box_stage(
    parents: BoxParents<'_>,
    declared: &Computed,
    own: &Computed,
    computed: impl Into<crate::style::box_input::BoxStyleInput>,
    element: Option<crate::style::computed::ElementStyle>,
    paint_filter: bool,
    vcx: ValueCx<'_>,
) -> BoxStyle {
    let BoxParents { layout, tree } = parents;
    let mut c = BoxStyle::shared(computed);
    c.element_style = element;
    c.set_parent_context(layout);
    context::inherit_context(parents, own, &mut c);
    inherit_containing(parents, own, &mut c, vcx);
    quirks::percentage_height(layout, own, &mut c, vcx);
    let applicability = c.context.applicability;
    self_align::self_alignment(applicability, own, &mut c);
    flow_align::resolve(layout, &mut c);
    inherit_paint(tree, declared, own, &mut c, paint_filter, vcx);
    let parent_px = super::compute::parent_font_px(tree);
    let own_px = super::compute::own_font_px(&c, parent_px);
    let decoration = tree.decoration_inputs(own);
    decorate(decoration, own, &mut c, own_px, vcx);
    c
}
