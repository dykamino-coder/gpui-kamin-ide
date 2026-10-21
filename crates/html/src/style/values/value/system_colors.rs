//! Значения: системные цвета CSS (css-color-4 §6.2) для светлой и тёмной
//! схемы, общие для значений CSS и родного вида элементов форм; устаревшие
//! имена сводятся к современным ролям. Палитра — родной запас Blink
//! `LayoutTheme::DefaultSystemColor`.

use super::Color;

#[derive(Clone, Copy)]
#[repr(usize)]
pub enum SystemColor {
    Canvas,
    CanvasText,
    ButtonFace,
    ButtonText,
    ButtonBorder,
    Field,
    FieldText,
    GrayText,
    AccentColor,
    AccentColorText,
    Highlight,
    HighlightText,
    Mark,
    MarkText,
    LinkText,
    VisitedText,
    ActiveText,
}

// CSS Color 4: platform colours depend on the element's used colour scheme.
// Blink LayoutTheme::DefaultSystemColor supplies the native fallback palette.
const COLOURS: [[u32; 2]; 17] = [
    [0xffffff, 0x121212],
    [0x000000, 0xffffff],
    [0xefefef, 0x6b6b6b],
    [0x000000, 0xffffff],
    [0x767676, 0x6b6b6b],
    [0xffffff, 0x3b3b3b],
    [0x000000, 0xffffff],
    [0x6d6d6d, 0xaaaaaa],
    [0x0075ff, 0x99c8ff],
    [0xffffff, 0x3b3b3b],
    [0x0075ff, 0x99c8ff],
    [0xffffff, 0x3b3b3b],
    [0xffff00, 0xffff00],
    [0x000000, 0x000000],
    [0x0000ee, 0x9e9eff],
    [0x551a8b, 0xd0adf0],
    [0xff0000, 0xff0000],
];

pub fn system_color(role: SystemColor, dark: bool) -> Color {
    let c = COLOURS[role as usize][usize::from(dark)];
    Color {
        r: ((c >> 16) & 255) as f32 / 255.,
        g: ((c >> 8) & 255) as f32 / 255.,
        b: (c & 255) as f32 / 255.,
        a: 1.,
    }
}

pub(super) fn named(name: &str, dark: bool) -> Option<Color> {
    use SystemColor::*;
    let role = match name {
        "canvas" | "activecaption" | "appworkspace" | "background" | "inactivecaption"
        | "infobackground" | "menu" | "scrollbar" | "window" => Canvas,
        "canvastext" | "captiontext" | "infotext" | "menutext" | "windowtext" => CanvasText,
        "buttonface" | "buttonhighlight" | "buttonshadow" | "threedface" => ButtonFace,
        "buttontext" => ButtonText,
        "buttonborder" | "activeborder" | "inactiveborder" | "threeddarkshadow"
        | "threedhighlight" | "threedlightshadow" | "threedshadow" | "windowframe" => ButtonBorder,
        "field" => Field,
        "fieldtext" => FieldText,
        "graytext" | "inactivecaptiontext" => GrayText,
        "accentcolor" => AccentColor,
        "accentcolortext" => AccentColorText,
        "highlight" | "selecteditem" => Highlight,
        "highlighttext" | "selecteditemtext" => HighlightText,
        "mark" => Mark,
        "marktext" => MarkText,
        "linktext" => LinkText,
        "visitedtext" => VisitedText,
        "activetext" => ActiveText,
        _ => return None,
    };
    Some(system_color(role, dark))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_follows_page_scheme() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        assert_eq!(
            system_color(SystemColor::Field, false),
            Color::parse("#fff", vcx).unwrap()
        );
        assert_eq!(
            system_color(SystemColor::Field, true),
            Color::parse("#3b3b3b", vcx).unwrap()
        );
        assert_ne!(
            system_color(SystemColor::ButtonText, false),
            system_color(SystemColor::ButtonText, true)
        );
    }

    #[test]
    fn deprecated_names_follow_the_same_scheme() {
        for dark in [false, true] {
            for (old, new) in [
                ("window", "canvas"),
                ("threedface", "buttonface"),
                ("windowframe", "buttonborder"),
                ("inactivecaptiontext", "graytext"),
            ] {
                assert_eq!(named(old, dark), named(new, dark));
            }
        }
    }
}
