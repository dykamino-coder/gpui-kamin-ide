//! Каскад, стадия вычисленных значений (css-cascade-5 §7.1–7.3): из
//! каскадного стиля элемента (`own`: свои значения, `None` у наследуемого —
//! «не задано») и ВЫЧИСЛЕННОГО стиля родителя — вычисленный стиль элемента.
//! Наследуемые длинные свойства без своего значения берут значение родителя,
//! каскадное `inherit` копирует его, относительные величины решаются в
//! порядке зависимостей (кегль и `em`/`ex`/`ch`/`lh`, функции цвета,
//! `tab-size`, `math-depth`, `currentcolor` фона).
//!
//! Стадия читает у родителя только вычисленные значения — ни контекста
//! коробки, ни used-значений (это `inherit::box_stage`); поэтому её итог
//! материализуется один раз на элемент и годится, пока унаследованные
//! данные родителя те же (`inherited_eq`).

use crate::document::state::ValueCx;
use crate::style::computed::box_predicates::resolve_bidi_inheritance;
use crate::style::computed::{Computed, TextAlign};
use crate::style::values::value::Len;

mod background;
mod explicit;
mod fonts;
mod longhands;
mod sizes;
mod text;
pub(in crate::style::cascade) use text::{decor_family, resolve_decor_len};

/// Вычисленный стиль элемента с каскадным стилем `own` под вычисленным
/// стилем родителя `parent`.
pub(crate) fn compute(parent: &Computed, own: &Computed, vcx: ValueCx<'_>) -> Computed {
    // Каскадное `inherit` ненаследуемых свойств — вычисленное значение
    // родителя вместо своего (css-cascade-5 §7.3.1), до всех шагов ниже:
    // они видят его как значение элемента.
    let resolved;
    let own = if own.explicit_inherit.is_empty() && own.inherit_longhands.is_empty() {
        own
    } else {
        let mut copy = own.clone();
        super::inherited_value::apply_explicit_inheritance(&mut copy, parent, vcx);
        longhands::inherit_longhands(&mut copy, parent, vcx);
        resolved = copy;
        &resolved
    };
    let mut c = own.clone();
    crate::style::computed::scroll::inherit(parent, &mut c);
    resolve_bidi_inheritance(parent, &mut c);
    sizes::compute_sizes(parent, own, &mut c, vcx);
    explicit::inherit_explicit(parent, own, &mut c);
    background::inherit_background(own, &mut c);
    fonts::inherit_fonts(parent, own, &mut c);
    crate::style::computed::math_style::inherit(parent, &mut c, vcx);
    text::inherit_text(parent, own, &mut c);
    c.resolve_gradient_current_color(vcx);
    text::inherit_ui(parent, own, &mut c);
    // `em` считается от размера шрифта — а он известен только здесь, когда
    // наследование уже произошло. База `em` у собственного кегля —
    // ВЫЧИСЛЕННЫЙ кегль родителя: `font-size-adjust` «does not affect the
    // size of em units».
    let parent_px = parent_font_px(parent);
    // Кегль (в том числе процент, `larger`/`smaller` по таблице и единицы
    // шрифта) — доля РОДИТЕЛЬСКОГО: его считает `resolve_em` первым шагом.
    c.resolve_em(parent_px, vcx);
    // `tab-size` в длине наследуется АБСОЛЮТНОЙ величиной: `5em` при кегле
    // 10px — это 50px и у ребёнка с кеглем 20px, а не его собственные 5em
    // (`tab-size-inheritance-001`).
    let own_px = own_font_px(&c, parent_px);
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
    text::inherit_decoration_values(parent, own, &mut c, own_px, vcx);
    // Вычисленный стиль несёт значения, а не каскадные слова.
    c.inherit_longhands = Default::default();
    c
}

/// Вычисленный кегль родителя — база `em` ребёнка (у подогнанного
/// `font-size-adjust` — его вычисленный, а не используемый кегль).
pub(crate) fn parent_font_px(parent: &Computed) -> f32 {
    match parent.font_adjust_base {
        Some((px, _)) => px,
        None => match parent.font_size {
            Some(Len::Px(px)) => px,
            _ => 16.0,
        },
    }
}

/// Собственный вычисленный кегль (после `resolve_em`); без своего — кегль
/// родителя.
pub(crate) fn own_font_px(c: &Computed, parent_px: f32) -> f32 {
    match c.font_size {
        Some(Len::Px(px)) => px,
        _ => parent_px,
    }
}
