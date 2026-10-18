//! Геометрия скроллбаров IDE из global.css, поверх цветов текущей темы.

use gpui::px;
use gpui_component::theme::ScrollbarStyles;

pub(super) fn styles(styles: ScrollbarStyles) -> ScrollbarStyles {
    if cfg!(any(target_os = "ios", target_os = "android")) {
        return styles;
    }
    // Дорожка и thumb — 8 px, радиус --radius-xs — 4 px, без inset
    // и без расширения thumb при hover/drag.
    styles
        .track(|style| style.width(px(8.)))
        .thumb(|style| style.width(px(8.)).inset(px(0.)).radius(px(4.)))
        .thumb_hover(|style| style.width(px(8.)).inset(px(0.)).radius(px(4.)))
        .thumb_active(|style| style.width(px(8.)).inset(px(0.)).radius(px(4.)))
}
