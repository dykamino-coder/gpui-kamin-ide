//! Identify font shorthand members whose declarations share source order.

pub(super) fn contains(property: &str) -> bool {
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
