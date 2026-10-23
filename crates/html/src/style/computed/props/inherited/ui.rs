//! Цвет, взаимодействие, изображение и краска SVG: `color` (css-color-4
//! §4.1), `cursor` (css-ui-4 §6.1), `pointer-events` (SVG 2 §pointer-events),
//! `visibility` (css-display-4 §4), `accent-color`, `caret-color` (css-ui-4
//! §6.2), `scrollbar-color` (css-scrollbars-1 §2), `image-rendering`
//! (css-images-3 §6.2), `image-orientation` (§5.4).

use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::css::LonghandId;
use crate::style::values::value::{ColorOrAuto, StyleColor};

/// `cursor: [<url> [<x> <y>]?,]* <keyword>` (css-ui-4 §6.1.1).
pub(crate) fn cursor(v: &str) -> Option<String> {
    let words: Vec<_> = crate::style::css::split_args(v);
    let keyword = words.last()?.trim().to_ascii_lowercase();
    let known = matches!(
        keyword.as_str(),
        "auto"
            | "default"
            | "none"
            | "context-menu"
            | "help"
            | "pointer"
            | "progress"
            | "wait"
            | "cell"
            | "crosshair"
            | "text"
            | "vertical-text"
            | "alias"
            | "copy"
            | "move"
            | "no-drop"
            | "not-allowed"
            | "grab"
            | "grabbing"
            | "all-scroll"
            | "col-resize"
            | "row-resize"
            | "n-resize"
            | "e-resize"
            | "s-resize"
            | "w-resize"
            | "ne-resize"
            | "nw-resize"
            | "se-resize"
            | "sw-resize"
            | "ew-resize"
            | "ns-resize"
            | "nesw-resize"
            | "nwse-resize"
            | "zoom-in"
            | "zoom-out"
    );
    let images = words[..words.len() - 1]
        .iter()
        .all(|w| w.trim().starts_with("url(") && w.contains(')'));
    (known && images).then(|| v.to_string())
}

/// `pointer-events` (SVG 2 §pointer-events): `true` — `none`.
pub(crate) fn pointer_events(v: &str) -> Option<bool> {
    let lower = v.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "auto"
            | "none"
            | "visiblepainted"
            | "visiblefill"
            | "visiblestroke"
            | "visible"
            | "painted"
            | "fill"
            | "stroke"
            | "all"
            | "bounding-box"
    )
    .then_some(lower == "none")
}

/// `visibility: visible | hidden | collapse`: (скрыто, схлопнуто).
pub(crate) fn visibility(v: &str) -> Option<(bool, bool)> {
    match v.to_ascii_lowercase().as_str() {
        "visible" => Some((false, false)),
        "hidden" => Some((true, false)),
        "collapse" => Some((true, true)),
        _ => None,
    }
}

/// `image-rendering: auto | smooth | high-quality | pixelated |
/// crisp-edges`: `true` — без сглаживания.
pub(crate) fn image_rendering(v: &str) -> Option<bool> {
    match v.to_ascii_lowercase().as_str() {
        "auto" | "smooth" | "high-quality" => Some(false),
        "pixelated" | "crisp-edges" => Some(true),
        _ => None,
    }
}

/// `image-orientation: from-image | none` (углы и `flip` css-images-3 §5.4
/// не поддержаны — как у Blink): `true` — `none`.
pub(crate) fn image_orientation(v: &str) -> Option<bool> {
    match v.to_ascii_lowercase().as_str() {
        "from-image" => Some(false),
        "none" => Some(true),
        _ => None,
    }
}

/// `scrollbar-color: auto | <color>{2}`.
pub(crate) fn scrollbar_color(v: &str, vcx: ValueCx<'_>) -> Option<Option<[StyleColor; 2]>> {
    if v.eq_ignore_ascii_case("auto") {
        return Some(None);
    }
    match crate::style::computed::split_outside_parens(v).as_slice() {
        [a, b] => Some(Some([
            StyleColor::parse_value(a, vcx)?,
            StyleColor::parse_value(b, vcx)?,
        ])),
        _ => None,
    }
}

pub(super) fn accepts(id: LonghandId, v: &str, vcx: ValueCx<'_>) -> bool {
    use LonghandId::*;
    match id {
        Color => StyleColor::parse_value(v, vcx).is_some(),
        Cursor => cursor(v).is_some(),
        PointerEvents => pointer_events(v).is_some(),
        Visibility => visibility(v).is_some(),
        AccentColor | CaretColor => ColorOrAuto::parse_value(v, vcx).is_some(),
        ScrollbarColor => scrollbar_color(v, vcx).is_some(),
        ImageRendering => image_rendering(v).is_some(),
        ImageOrientation => image_orientation(v).is_some(),
        _ => false,
    }
}

impl Computed {
    pub(super) fn apply_ui(&mut self, id: LonghandId, v: &str, vcx: ValueCx<'_>) {
        use LonghandId::*;
        match id {
            // `currentcolor` в `color` — цвет родителя (css-color-4 §6.4);
            // функция с ним (`color-mix()`, относительный цвет) вычисляется
            // от цвета РОДИТЕЛЯ при слиянии и наследуется абсолютным цветом.
            Color => {
                let Some(parsed) = StyleColor::parse_value(v, vcx) else {
                    return;
                };
                self.color_initial = false;
                (self.color, self.color_function) = match parsed {
                    StyleColor::Current => (None, None),
                    StyleColor::Absolute(color) => (Some(color), None),
                    function => (None, Some(function)),
                };
            }
            Cursor => self.cursor = cursor(v).or(self.cursor.take()),
            PointerEvents => {
                self.pointer_events_none = pointer_events(v).or(self.pointer_events_none)
            }
            Visibility => {
                if let Some((hidden, collapsed)) = visibility(v) {
                    self.hidden = Some(hidden);
                    self.collapsed = Some(collapsed);
                }
            }
            AccentColor => {
                if let Some(c) = ColorOrAuto::parse_value(v, vcx) {
                    self.accent_color = Some(c);
                }
            }
            CaretColor => {
                if let Some(c) = ColorOrAuto::parse_value(v, vcx) {
                    self.caret_color = Some(c);
                }
            }
            ScrollbarColor => {
                if let Some(colors) = scrollbar_color(v, vcx) {
                    self.scroll.colors = Some(colors);
                }
            }
            ImageRendering => self.image_pixelated = image_rendering(v).or(self.image_pixelated),
            ImageOrientation => {
                self.image_orient_none = image_orientation(v).or(self.image_orient_none)
            }
            _ => {}
        }
    }

    pub(super) fn inherit_ui(&mut self, id: LonghandId) {
        use LonghandId::*;
        match id {
            Color => {
                self.color = None;
                self.color_function = None;
                self.color_initial = false;
            }
            Cursor => self.cursor = None,
            PointerEvents => self.pointer_events_none = None,
            Visibility => {
                self.hidden = None;
                self.collapsed = None;
            }
            AccentColor => self.accent_color = None,
            CaretColor => self.caret_color = None,
            ScrollbarColor => self.scroll.colors = None,
            ImageRendering => self.image_pixelated = None,
            ImageOrientation => self.image_orient_none = None,
            _ => {}
        }
    }
}
