//! Усечение строки под ширину с «…» ЧЕРЕЗ ИЗМЕРЕНИЕ шейпером.
//! `text_ellipsis()` в gpui ставит `TextOverflow::Truncate("…")`
//! (`vendor/gpui/src/styled.rs:83-88`), но для `.child(SharedString)` эллипсис
//! на экране не появлялся — текст просто обрезался родительским
//! `overflow_hidden` (скрин-сверка с оригиналом). Поэтому там, где многоточие
//! обязано быть видно, режем сами.

use gpui::{SharedString, Window, px};

/// Ширина строки UI-шрифтом (шейпер) — та же метрика, что у рендера.
pub fn measure(text: &str, size: f32, window: &mut Window) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let run = gpui::TextRun {
        len: text.len(),
        font: gpui::Font {
            family: crate::root::UI_FONT.into(),
            features: gpui::FontFeatures::default(),
            fallbacks: None,
            weight: gpui::FontWeight::MEDIUM,
            style: gpui::FontStyle::Normal,
            // Ширина начертания у шрифтов оболочки обычная.
            stretch: gpui::FontStretch::Normal,
        },
        color: gpui::black(),
        background_color: None,
        // Поля строчного бокса вокруг фона прогона: здесь их нет.
        font_size: None,
        background_pad: Default::default(),
        background_radius: Default::default(),
        background_border: None,
        underline: None,
        strikethrough: None,
    };
    f32::from(
        window
            .text_system()
            .shape_line(SharedString::from(text.to_string()), px(size), &[run], None)
            .width,
    )
}

/// Средняя ширина глифа UI-шрифта (Bricolage Grotesque medium) — фолбэк,
/// когда шейпер недоступен в этом контексте и возвращает 0.
const AVG_GLYPH_RATIO: f32 = 0.62;

/// Оценка ширины строки БЕЗ шейпера — та же метрика, по которой режет
/// `fit_approx`. Нужна, чтобы бюджет соседних колонок считался в той же
/// системе, что и усечение.
pub fn approx_width(text: &str, size: f32) -> f32 {
    text.chars().count() as f32 * size * AVG_GLYPH_RATIO
}

/// Строка, влезающая в `max_w` (иначе обрезок + «…»).
/// То же усечение с «…», но БЕЗ шейпера: ширина оценивается по числу
/// символов (`AVG_GLYPH_RATIO`). Нужна там, где до элемента не дотянут
/// `&mut Window` (дерево файлов, contributed-дерево): точность ±1 символ,
/// зато многоточие есть — без него строка просто обрывалась.
pub fn fit_approx(text: &str, max_w: f32, size: f32) -> SharedString {
    if max_w <= 0.0 {
        return SharedString::from(text.to_string());
    }
    let max_chars = (max_w / (size * AVG_GLYPH_RATIO)).floor() as usize;
    let count = text.chars().count();
    if count <= max_chars || max_chars == 0 {
        return SharedString::from(text.to_string());
    }
    let cut: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    SharedString::from(format!("{}…", cut.trim_end()))
}

pub fn fit(text: &str, max_w: f32, size: f32, window: &mut Window) -> SharedString {
    if max_w <= 0.0 {
        return SharedString::from(text.to_string());
    }
    let full = measure(text, size, window);
    if full <= 0.0 {
        // Шейпер не дал ширины → эвристика по числу символов
        let max_chars = (max_w / (size * AVG_GLYPH_RATIO)).floor() as usize;
        let count = text.chars().count();
        if count <= max_chars || max_chars == 0 {
            return SharedString::from(text.to_string());
        }
        let cut: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        return SharedString::from(format!("{}…", cut.trim_end()));
    }
    if full <= max_w {
        return SharedString::from(text.to_string());
    }
    let ell = "…";
    let ell_w = measure(ell, size, window);
    let budget = (max_w - ell_w).max(0.0);
    // Идём по символам, пока влезаем (строки короткие — линейно достаточно)
    let mut end = 0usize;
    for (i, _) in text.char_indices().skip(1) {
        if measure(&text[..i], size, window) > budget {
            break;
        }
        end = i;
    }
    if end == 0 {
        return SharedString::from(ell.to_string());
    }
    SharedString::from(format!("{}{}", text[..end].trim_end(), ell))
}
