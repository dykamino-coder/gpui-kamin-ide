//! Вычисленный стиль: какие свойства входят в сокращение `font` (CSS Fonts 4
//! #font-prop — longhand и сбрасываемые подсвойства) и потому делят с ним
//! порядок объявлений в каскаде; независимые `font-*` сюда не входят.

pub(crate) fn contains(property: &str) -> bool {
    // CSS Fonts 4 #font-prop: longhands and reset-only subproperties
    // participate in the shorthand; independently cascaded font-* do not.
    matches!(
        property,
        "font-family"
            | "font-size"
            | "font-style"
            | "font-weight"
            | "font-stretch"
            | "font-width"
            | "font-variant"
            | "font-variant-caps"
            | "font-kerning"
            | "font-variant-alternates"
            | "line-height"
    )
}
