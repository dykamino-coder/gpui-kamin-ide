//! Типографские хелперы: OpenType-фичи заголовков.
//!
//! Оригинал (`SessionsMode.module.css`, `FileTreeHeader`, `CustomizeMode`):
//! секционные заголовки идут `font-feature-settings: "ss01"` +
//! `letter-spacing: 0.08em` + `text-transform: uppercase`.
//! `ss01` включаем через `Font.features`; **letter-spacing gpui не
//! поддерживает вовсе** (в `TextStyle` поля нет) — это отмеченное
//! отклонение, а не забытая правка: заголовок выходит ~6px уже
//! оригинального на 8 символах.

use std::sync::Arc;

use gpui::{Font, FontFeatures, FontStyle, FontWeight};

/// Шрифт UI с моноширинными цифрами (`font-variant-numeric: tabular-nums`).
/// В OpenType это фича `tnum` — её просит пилюля версии на Welcome
/// (`WelcomePlaceholder.module.css:65`, ревью ц.26).
pub fn tnum(weight: FontWeight) -> Font {
    Font {
        family: crate::root::UI_FONT.into(),
        features: FontFeatures(Arc::new(vec![("tnum".to_string(), 1)])),
        fallbacks: None,
        weight,
        style: FontStyle::Normal,
    }
}

/// Шрифт UI с включённой стилистикой `ss01` (заголовки секций).
pub fn ss01(weight: FontWeight) -> Font {
    Font {
        family: crate::root::UI_FONT.into(),
        features: FontFeatures(Arc::new(vec![("ss01".to_string(), 1)])),
        fallbacks: None,
        weight,
        style: FontStyle::Normal,
    }
}
