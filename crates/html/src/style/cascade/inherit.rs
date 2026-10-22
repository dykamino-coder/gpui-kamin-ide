//! Каскад: наследование стиля — слияние вычисленного стиля родителя с
//! собственным стилем узла (`inherit`). Вход — два `Computed`, выход — стиль
//! узла, у которого разрешены `em`/проценты кегля, цвет рамки по умолчанию,
//! абсолютный `tab-size`, flow-относительное выравнивание и декорации; этапы
//! разнесены по подмодулям (containing, sizes, explicit, fonts, text_layout,
//! paint, decorate). `inherit_unpainted` — то же без окраски фильтром.

use crate::document::state::ValueCx;

use crate::style::computed::box_predicates::{
    backdrop_root, establishes_cb, resolve_bidi_inheritance,
};
use crate::style::computed::{Computed, TextAlign};
use crate::style::values::value::{Color, Len};

mod background;
mod containing;
mod decorate;
mod explicit;
mod flow_align;
mod fonts;
mod paint;
mod sizes;
mod text_layout;
use containing::inherit_containing;
use decorate::decorate;
use explicit::inherit_explicit;
pub(crate) use flow_align::note as note_flow_align;
use fonts::inherit_fonts;
use paint::inherit_paint;
use sizes::inherit_sizes;
use text_layout::inherit_text_layout;

// `zoom` (css-viewport-1) не делать домножением использованных длин здесь,
// в `inherit` (с `zoom_eff`/`scale_px` в `Computed`): гейт `zoom_eff != 1.0`
// не удерживает — домножение задевает общий путь длин (CSS2 `background-*`,
// таблицы, абсолюты). Только отдельным проходом после каскада
// (`style::zoom::resolve`), а не через самую горячую функцию крейта.
pub fn inherit(parent: &Computed, own: &Computed, vcx: ValueCx<'_>) -> Computed {
    inherit_stage(parent, own, true, vcx)
}

/// Document preparation propagates style; Filter Effects 1 §5 applies colors
/// only when the renderer builds the element, never while removing wrappers.
pub(crate) fn inherit_unpainted(parent: &Computed, own: &Computed, vcx: ValueCx<'_>) -> Computed {
    inherit_stage(parent, own, false, vcx)
}

fn inherit_stage(
    parent: &Computed,
    own: &Computed,
    paint_filter: bool,
    vcx: ValueCx<'_>,
) -> Computed {
    let mut resolved;
    let own = if own.explicit_inherit.is_empty() {
        own
    } else {
        resolved = own.clone();
        super::inherited_value::apply_explicit_inheritance(&mut resolved, parent, vcx);
        &resolved
    };
    let mut c = own.clone();
    c.text_shadow_ink = own.text_shadow_ink.or(parent.text_shadow_ink);
    crate::style::computed::scroll::inherit(parent, &mut c);
    inherit_containing(parent, own, &mut c, vcx);
    inherit_sizes(parent, own, &mut c, vcx);
    inherit_explicit(parent, own, &mut c, vcx);
    background::inherit_background(parent, own, &mut c);
    inherit_fonts(parent, own, &mut c);
    crate::style::computed::math_style::inherit(parent, &mut c, vcx);
    inherit_text_layout(parent, own, &mut c);
    flow_align::resolve(parent, &mut c);
    c.resolve_gradient_current_color(vcx);
    inherit_paint(parent, own, &mut c, paint_filter, vcx);
    // `em` считается от размера шрифта — а он известен только здесь, когда
    // наследование уже произошло. Раньше длина переводилась в точки при
    // разборе, по постоянным 16 точкам, и вложенные кегли не перемножались.
    // База `em` у собственного кегля — ВЫЧИСЛЕННЫЙ кегль родителя:
    // `font-size-adjust` «does not affect the size of em units».
    let parent_px = match parent.font_adjust_base {
        Some((px, _)) => px,
        None => match parent.font_size {
            Some(Len::Px(px)) => px,
            _ => 16.0,
        },
    };
    // Кегль (в том числе процент, `larger`/`smaller` по таблице и единицы
    // шрифта) — доля РОДИТЕЛЬСКОГО: его считает `resolve_em` первым шагом.
    c.resolve_em(parent_px, vcx);
    // `tab-size` в длине наследуется АБСОЛЮТНОЙ величиной: `5em` при кегле
    // 10px — это 50px и у ребёнка с кеглем 20px, а не его собственные 5em
    // (`tab-size-inheritance-001`).
    let own_px = match c.font_size {
        Some(Len::Px(px)) => px,
        _ => parent_px,
    };
    c.tab_size_len = match own.tab_size_len {
        Some(len) => Some(Len::Px(crate::text::metrics::spacing_px(
            Some(len),
            &c.font_family.clone().unwrap_or_default(),
            own_px,
            vcx,
        ))),
        None if own.tab_size.is_some() => None,
        None => parent.tab_size_len,
    };
    decorate(parent, own, &mut c, own_px, vcx);
    c
}
