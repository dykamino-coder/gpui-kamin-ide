//! Тесты: проверка без записи отвечает ровно тем, что пишет `resolve_viewport`.

use crate::style::computed::{BoxStyle, Computed};

/// Стиль после наследования: шрифтовые длины уже вычислены, единицы окна — нет.
fn styled(declarations: &[(&str, &str)], vcx: crate::values::ValueCx<'_>) -> Computed {
    let mut own = Computed::default();
    for (property, value) in declarations {
        own.apply_one(property, value, vcx);
    }
    let inherited = crate::style::cascade::inherit::inherit(&BoxStyle::default(), &own, vcx);
    Computed::clone(&inherited)
}

/// Ответ проверки и сверка с записью: «нет» — запись не трогает ни значения, ни общие группы;
/// «да» — запись меняет стиль, а после неё решать больше нечего.
fn probe(declarations: &[(&str, &str)]) -> bool {
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    let style = styled(declarations, vcx);
    let has = style.has_viewport_lengths(vcx);
    let mut resolved = style.clone();
    resolved.resolve_viewport((200.0, 100.0), vcx);
    assert_eq!(has, resolved != style, "{declarations:?}");
    assert_eq!(has, !resolved.shares_storage(&style), "{declarations:?}");
    assert!(!resolved.has_viewport_lengths(vcx), "{declarations:?}");
    has
}

#[test]
fn values_without_viewport_units_have_nothing_to_resolve() {
    for declarations in [
        &[("width", "10px"), ("margin", "1em 5%")][..],
        &[("width", "calc(10px + 5%)")],
        &[("background-image", "linear-gradient(90deg, #def, #fff)")],
        &[("background-image", "linear-gradient(red 10px, blue 50%)")],
        &[(
            "background-image",
            "radial-gradient(circle 5px at 10px 20%, red, blue)",
        )],
        &[("background-image", "url(image.svg)")],
        &[(
            "background",
            "url(a.svg) 5px 10% / 20px auto, linear-gradient(red, blue)",
        )],
        &[("mask-image", "url(image.svg)")],
        &[("content", "url(image.svg)")],
        &[("border-image", "url(image.svg) 30")],
        &[("border-radius", "10px / 20px")],
        &[("text-decoration-inset", "2px 5px")],
        &[("gap", "4px 1em")],
    ] {
        assert!(!probe(declarations), "{declarations:?}");
    }
}

#[test]
fn viewport_units_in_every_walked_place_are_reported() {
    for declarations in [
        &[("width", "50vw")][..],
        &[("min-height", "calc(10px + 1vh)")],
        &[("flex-basis", "10vw")],
        &[("font-size", "2vw")],
        &[("padding", "0 1vw")],
        &[("scroll-margin", "1vh")],
        &[("margin-left", "calc(5% + 1vw)")],
        &[("border-bottom", "50vh solid")],
        &[("position", "absolute"), ("left", "1vw")],
        &[("border-radius", "1vw")],
        &[("border-radius", "10px / 2vh")],
        &[("background-image", "linear-gradient(red 10vw, blue)")],
        &[(
            "background-image",
            "radial-gradient(circle 5vw at 10px 20%, red, blue)",
        )],
        &[(
            "background",
            "url(a.svg) 5vw 10%, linear-gradient(red, blue)",
        )],
        &[("mask-image", "linear-gradient(red 10vw, blue)")],
        &[("gap", "1vw")],
        &[("text-decoration-inset", "1vw")],
    ] {
        assert!(probe(declarations), "{declarations:?}");
    }
}

#[test]
fn negative_radius_is_a_write_without_viewport_units() {
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut style = Computed::default();
    style.reset_border.radius.tl = Some(crate::style::values::value::Len::Px(-1.0));
    assert!(style.has_viewport_lengths(vcx));
    style.resolve_viewport((200.0, 100.0), vcx);
    assert!(!style.has_viewport_lengths(vcx));
}
