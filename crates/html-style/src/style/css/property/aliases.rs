//! Legacy property name aliases (css-cascade-5 section 2.1 "legacy name alias") and
//! the value parsing an old name keeps where browsers differ from the new name.
//!
//! The list is Blink's `css_properties.json5` `alias_for` (the Compat Standard
//! for `-webkit-`): the old name becomes the new one while parsing and both share
//! one cascade slot. Value grammars and rendering support remain the responsibility
//! of the corresponding property; an alias does not add rendering capabilities.

use std::borrow::Cow;

/// The new property name for a legacy one.
pub fn legacy_name_alias(name: &str) -> Option<&'static str> {
    Some(match name {
        "-webkit-appearance" => "appearance",
        "-webkit-background-clip" => "background-clip",
        "-webkit-background-origin" => "background-origin",
        "-webkit-background-size" => "background-size",
        "-webkit-user-select" => "user-select",
        "-webkit-mask" => "mask",
        "-webkit-mask-image" => "mask-image",
        "-webkit-mask-clip" => "mask-clip",
        "-webkit-mask-origin" => "mask-origin",
        "-webkit-mask-position" => "mask-position",
        "-webkit-mask-repeat" => "mask-repeat",
        "-webkit-mask-size" => "mask-size",
        "-webkit-mask-composite" => "mask-composite",
        "-webkit-mask-box-image" => "mask-border",
        "-webkit-mask-box-image-outset" => "mask-border-outset",
        "-webkit-mask-box-image-repeat" => "mask-border-repeat",
        "-webkit-mask-box-image-slice" => "mask-border-slice",
        "-webkit-mask-box-image-source" => "mask-border-source",
        "-webkit-mask-box-image-width" => "mask-border-width",
        // css-fonts-4 §font-width-prop: «font-stretch is a legacy name alias».
        "font-stretch" => "font-width",
        // css-text-3 §5.5: «word-wrap … legacy name alias of overflow-wrap».
        "word-wrap" => "overflow-wrap",
        "-webkit-text-decoration-line" => "text-decoration-line",
        "-webkit-text-decoration-style" => "text-decoration-style",
        "-webkit-text-decoration-color" => "text-decoration-color",
        // css-align-3 §8.4 / css-gaps-1: «legacy name aliases».
        "grid-gap" => "gap",
        "grid-row-gap" => "row-gap",
        "grid-column-gap" => "column-gap",
        "-webkit-flex" => "flex",
        "-webkit-flex-basis" => "flex-basis",
        "-webkit-flex-direction" => "flex-direction",
        "-webkit-flex-flow" => "flex-flow",
        "-webkit-flex-grow" => "flex-grow",
        "-webkit-flex-shrink" => "flex-shrink",
        "-webkit-flex-wrap" => "flex-wrap",
        "-webkit-align-content" => "align-content",
        "-webkit-align-items" => "align-items",
        "-webkit-align-self" => "align-self",
        "-webkit-justify-content" => "justify-content",
        "-webkit-order" => "order",
        "-webkit-column-count" => "column-count",
        "-webkit-column-gap" => "column-gap",
        "-webkit-column-rule" => "column-rule",
        "-webkit-column-rule-color" => "column-rule-color",
        "-webkit-column-rule-style" => "column-rule-style",
        "-webkit-column-rule-width" => "column-rule-width",
        "-webkit-column-span" => "column-span",
        "-webkit-column-width" => "column-width",
        "-webkit-columns" => "columns",
        // css-transforms-1/2: transform properties.
        "-webkit-transform" => "transform",
        "-webkit-transform-origin" => "transform-origin",
        "-webkit-transform-style" => "transform-style",
        "-webkit-perspective" => "perspective",
        "-webkit-perspective-origin" => "perspective-origin",
        "-webkit-backface-visibility" => "backface-visibility",
        // css-backgrounds-3: shadows and corner radii.
        "-webkit-box-shadow" => "box-shadow",
        "-webkit-border-radius" => "border-radius",
        "-webkit-border-top-left-radius" => "border-top-left-radius",
        "-webkit-border-top-right-radius" => "border-top-right-radius",
        "-webkit-border-bottom-left-radius" => "border-bottom-left-radius",
        "-webkit-border-bottom-right-radius" => "border-bottom-right-radius",
        "-webkit-box-sizing" => "box-sizing",
        "-webkit-clip-path" => "clip-path",
        "-webkit-filter" => "filter",
        "-webkit-opacity" => "opacity",
        "-webkit-animation" => "animation",
        "-webkit-animation-delay" => "animation-delay",
        "-webkit-animation-direction" => "animation-direction",
        "-webkit-animation-duration" => "animation-duration",
        "-webkit-animation-fill-mode" => "animation-fill-mode",
        "-webkit-animation-iteration-count" => "animation-iteration-count",
        "-webkit-animation-name" => "animation-name",
        "-webkit-animation-play-state" => "animation-play-state",
        "-webkit-animation-timing-function" => "animation-timing-function",
        "-webkit-transition" => "transition",
        "-webkit-transition-delay" => "transition-delay",
        "-webkit-transition-duration" => "transition-duration",
        "-webkit-transition-property" => "transition-property",
        "-webkit-transition-timing-function" => "transition-timing-function",
        "-webkit-font-feature-settings" => "font-feature-settings",
        "-webkit-hyphenate-character" => "hyphenate-character",
        "-webkit-print-color-adjust" => "print-color-adjust",
        "-webkit-text-size-adjust" => "text-size-adjust",
        "-webkit-text-emphasis" => "text-emphasis",
        "-webkit-text-emphasis-color" => "text-emphasis-color",
        "-webkit-text-emphasis-position" => "text-emphasis-position",
        "-webkit-text-emphasis-style" => "text-emphasis-style",
        "-webkit-shape-image-threshold" => "shape-image-threshold",
        "-webkit-shape-margin" => "shape-margin",
        "-webkit-shape-outside" => "shape-outside",
        // Old WebKit logical names (css-logical-1).
        "-webkit-margin-start" => "margin-inline-start",
        "-webkit-margin-end" => "margin-inline-end",
        "-webkit-margin-before" => "margin-block-start",
        "-webkit-margin-after" => "margin-block-end",
        "-webkit-padding-start" => "padding-inline-start",
        "-webkit-padding-end" => "padding-inline-end",
        "-webkit-padding-before" => "padding-block-start",
        "-webkit-padding-after" => "padding-block-end",
        "-webkit-border-start" => "border-inline-start",
        "-webkit-border-end" => "border-inline-end",
        "-webkit-border-before" => "border-block-start",
        "-webkit-border-after" => "border-block-end",
        "-webkit-border-start-color" => "border-inline-start-color",
        "-webkit-border-start-style" => "border-inline-start-style",
        "-webkit-border-start-width" => "border-inline-start-width",
        "-webkit-border-end-color" => "border-inline-end-color",
        "-webkit-border-end-style" => "border-inline-end-style",
        "-webkit-border-end-width" => "border-inline-end-width",
        "-webkit-border-before-color" => "border-block-start-color",
        "-webkit-border-before-style" => "border-block-start-style",
        "-webkit-border-before-width" => "border-block-start-width",
        "-webkit-border-after-color" => "border-block-end-color",
        "-webkit-border-after-style" => "border-block-end-style",
        "-webkit-border-after-width" => "border-block-end-width",
        "-webkit-logical-width" => "inline-size",
        "-webkit-logical-height" => "block-size",
        "-webkit-min-logical-width" => "min-inline-size",
        "-webkit-min-logical-height" => "min-block-size",
        "-webkit-max-logical-width" => "max-inline-size",
        "-webkit-max-logical-height" => "max-block-size",
        _ => return None,
    })
}

/// The value written under an old name, as a value of the new property. The old
/// parsing is wider than the new one in two places (Blink `UseAliasParsing`):
/// - `-webkit-border-radius: a b` means `a / b` (`ConsumeRadii`);
/// - `-webkit-background-size: a` means `a a` (`ParsingStyle::kLegacy`).
pub fn legacy_alias_value<'a>(name: &str, value: &'a str) -> Cow<'a, str> {
    // CSS-wide keywords also apply to the complete legacy declaration.
    // Doubling `inherit` as a background size would make it invalid.
    if super::CssWide::parse(value).is_some() {
        return Cow::Borrowed(value);
    }
    let words = |v: &str| v.split_whitespace().count();
    match name {
        "-webkit-border-radius" if !value.contains('/') && words(value) == 2 => {
            let mut it = value.split_whitespace();
            match (it.next(), it.next()) {
                (Some(h), Some(v)) => Cow::Owned(format!("{h} / {v}")),
                _ => Cow::Borrowed(value),
            }
        }
        "-webkit-background-size" if !value.contains('(') => {
            let layers: Vec<String> = value
                .split(',')
                .map(|layer| {
                    let t = layer.trim();
                    let keyword = t.eq_ignore_ascii_case("cover")
                        || t.eq_ignore_ascii_case("contain")
                        || t.eq_ignore_ascii_case("auto");
                    if words(t) == 1 && !keyword {
                        format!("{t} {t}")
                    } else {
                        t.to_string()
                    }
                })
                .collect();
            Cow::Owned(layers.join(", "))
        }
        _ => Cow::Borrowed(value),
    }
}

#[cfg(test)]
mod tests;
