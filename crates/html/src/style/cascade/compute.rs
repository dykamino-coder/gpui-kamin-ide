//! Каскад, стадия вычисленных значений (css-cascade-5 §7.1–7.3): из
//! каскадного стиля элемента (`own`: свои значения, `None` у наследуемого —
//! «не задано») и ВЫЧИСЛЕННОГО стиля родителя — вычисленный стиль элемента.
//! Наследуемые длинные свойства без своего значения берут значение родителя,
//! каскадное `inherit` копирует его, относительные величины решаются в
//! порядке зависимостей (кегль и `em`/`ex`/`ch`/`lh`, функции цвета,
//! `tab-size`, `math-depth`, `currentcolor` фона).
//!
//! Для cq-единиц входом служит размер query container из завершённой
//! раскладки, переданный отдельно через слот элемента. Изменение этого
//! входа пересчитывает стиль потомков до paint. Контекст коробки остаётся
//! в `inherit::box_stage`; критерий кеша учитывает контейнерные оси.

use crate::document::state::ValueCx;
use crate::style::computed::box_predicates::resolve_bidi_inheritance;
use crate::style::computed::{Computed, TextAlign};
use crate::style::publication::{ComputedBuilder, Published};
use crate::style::values::value::Len;

mod background;
mod explicit;
mod fonts;
mod longhands;
mod sizes;
mod text;
mod zoom;
pub(in crate::style::cascade) use text::{decor_family, resolve_decor_len};

/// Вычисленный стиль элемента с каскадным стилем `own` под вычисленным
/// стилем родителя `parent`.
pub(in crate::style) fn compute(parent: &Computed, own: &Computed, vcx: ValueCx<'_>) -> Published {
    build(parent, own, vcx).finish()
}

/// Входы materialize добавляются в builder до публикации закрытого handle.
pub(super) fn build(parent: &Computed, own: &Computed, vcx: ValueCx<'_>) -> ComputedBuilder {
    // Зум — первым (css-viewport-1 §zoom-property): свои длины каскадного
    // стиля — в зуме элемента, до наследования и `em` ниже.
    let zoomed;
    let mut own_zoom = 1.0;
    let own = match own
        .zoom_pending
        .then(|| zoom::factors(parent, own))
        .flatten()
    {
        Some((k, eff)) => {
            let mut copy = own.clone();
            zoom::zoom_own(parent, &mut copy, k, eff, vcx);
            own_zoom = k;
            zoomed = copy;
            &zoomed
        }
        None => own,
    };
    // Каскадное `inherit` ненаследуемых свойств — вычисленное значение
    // родителя вместо своего (css-cascade-5 §7.3.1), до всех шагов ниже:
    // они видят его как значение элемента. Длины родителя — в зуме элемента.
    let resolved;
    let own = if own.explicit_inherit.is_empty() && own.inherit_longhands.is_empty() {
        own
    } else {
        let scaled = zoom::parent_in_own_zoom(parent, own_zoom, vcx);
        let from = scaled.as_ref().unwrap_or(parent);
        let mut copy = own.clone();
        super::inherited_value::apply_explicit_inheritance(&mut copy, from, vcx);
        longhands::inherit_longhands(&mut copy, from, vcx);
        resolved = copy;
        &resolved
    };
    let mut c = ComputedBuilder::from_cascaded(own);
    c.cq_box = parent.container_box_for_children();
    // Коробка раскладки своего зума не задаёт: она в зуме родителя.
    if !own.zoom_pending && c.zoom_eff.is_none() {
        let eff = zoom::parent_eff(parent);
        c.zoom_eff = ((eff - 1.0).abs() > f32::EPSILON).then_some(eff);
    }
    c.zoom_pending = false;
    crate::style::computed::scroll::inherit(parent, &mut c);
    resolve_bidi_inheritance(parent, &mut c);
    fonts::inherit_writing_mode(parent, own, &mut c);
    // Эти две длины нужны уже вычислению lh в compute_sizes.
    c.resolve_container_font_lengths(vcx);
    sizes::compute_sizes(parent, own, &mut c, vcx);
    explicit::inherit_explicit(parent, own, &mut c);
    background::inherit_background(own, &mut c);
    fonts::inherit_fonts(parent, own, &mut c);
    crate::style::computed::math_style::inherit(parent, &mut c, vcx);
    text::inherit_text(parent, own, &mut c);
    c.resolve_gradient_current_color(vcx);
    text::inherit_ui(parent, own, &mut c);
    c.resolve_container_units(vcx);
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
    c.tab_size_len = match own.tab_size_len.and(c.tab_size_len) {
        Some(len) => Some(Len::Px(crate::style::font_metrics::spacing_px(
            Some(len),
            &c.font_family.clone().unwrap_or_default(),
            own_px,
            vcx,
        ))),
        None if own.tab_size.is_some() => None,
        None => parent.tab_size_len,
    };
    text::inherit_decoration_values(parent, own, &mut c, own_px, vcx);
    // `path-length` вычисляется в абсолютную длину (SVG 2 §pathLength).
    if let Some(mut length) = c
        .svg_path_length
        .filter(|l| !matches!(l, Len::Px(_) | Len::Auto))
    {
        let font = c.font_cx(vcx);
        length.resolve_line_height(font.line_px.unwrap_or(1.2 * font.font_px), vcx);
        if let Some(value) = length.to_px(None, font, vcx) {
            c.svg_path_length = Some(Len::Px(value.max(0.0)));
        }
    }
    // Вычисленный стиль несёт значения, а не каскадные слова.
    c.inherit_longhands = Default::default();
    // Те же наследуемые значения, что у родителя, — его же группа: дети без
    // своих наследуемых значений делят её, и сравнение по указателю решает
    // повторное использование (Blink: общие группы `ComputedStyle`).
    if c.inh == parent.inh {
        c.inh = parent.inh.clone();
    }
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
/// родителя. Под `font-size-adjust` `font_size` несёт подогнанный используемый
/// кегль, а `em` считается от вычисленного (css-fonts-5 §font-size-adjust) — он в
/// `font_adjust_base`.
pub(crate) fn own_font_px(c: &Computed, parent_px: f32) -> f32 {
    match (c.font_adjust_base, c.font_size) {
        (Some((px, _)), _) => px,
        (None, Some(Len::Px(px))) => px,
        _ => parent_px,
    }
}

#[cfg(test)]
mod font_px_tests {
    use super::*;

    #[test]
    fn em_lengths_use_the_computed_size_under_font_size_adjust() {
        // Computed 16px adjusted to a used 32px: `tab-size: 1em` and decoration
        // lengths resolve against 16px.
        let mut c = Computed::default();
        c.font_size = Some(Len::Px(32.0));
        c.font_adjust_base = Some((16.0, None));
        assert_eq!(own_font_px(&c, 10.0), 16.0);
        assert_eq!(parent_font_px(&c), 16.0);
    }
}
