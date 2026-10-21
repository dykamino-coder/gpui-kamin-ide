//! Композиция source-over CSS-цветов с прямой (straight) альфой.

/// Краска `top` поверх `base` (source-over), цвета без премультипликации.
pub(crate) fn over(
    top: crate::style::values::value::Color,
    base: crate::style::values::value::Color,
) -> crate::style::values::value::Color {
    let a = top.a + base.a * (1.0 - top.a);
    if a <= 0.0 {
        return crate::style::values::value::Color::default();
    }
    let mix = |t: f32, b: f32| (t * top.a + b * base.a * (1.0 - top.a)) / a;
    crate::style::values::value::Color {
        r: mix(top.r, base.r),
        g: mix(top.g, base.g),
        b: mix(top.b, base.b),
        a,
    }
}
