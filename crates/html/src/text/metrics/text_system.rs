//! Text system for metrics; split out to keep the owning module within 250 lines.

use super::MONO;
use super::space;
use super::{INSTALLED, MONO_FAMILIES, WRAP, install_probe, install_vprobe};

/// Поставить щуп поверх системы шрифтов GPUI.
///
/// Вызывается один раз при старте приложения, ПОСЛЕ регистрации своих
/// шрифтов: до неё `Ahem` ещё не найден и замер вернул бы метрики подмены.
pub fn use_text_system(text_system: std::sync::Arc<gpui::TextSystem>) {
    space::use_text_system(text_system.clone());
    let names = text_system.all_font_names();
    // Список семейств из каскада разбирается ДО отрисовки, а выбирать из него
    // надо УСТАНОВЛЕННОЕ: `font-family: Courier New, Ahem` при отсутствующем
    // `Courier New` обязан взять `Ahem`, а не отдать системе имя, которого
    // нет (§15.3).
    INSTALLED.with(|i| {
        *i.borrow_mut() = names
            .iter()
            .map(|n| n.to_ascii_lowercase())
            .collect::<std::collections::HashSet<_>>();
    });
    if let Some(found) = MONO_FAMILIES
        .into_iter()
        .find(|want| names.iter().any(|have| have.eq_ignore_ascii_case(want)))
    {
        MONO.with(|m| *m.borrow_mut() = found);
    }
    // Второму щупу (вертикальные метрики) нужен свой владелец `Arc`:
    // первый забирает `text_system` в замыкание целиком.
    let text_system2 = text_system.clone();
    let text_system3 = text_system.clone();
    WRAP.with(|w| {
        *w.borrow_mut() = Some(Box::new(
            move |font: &gpui::Font, size: f32, text: &str, width: f32| {
                let mut wrapper = text_system3.line_wrapper(font.clone(), gpui::px(size));
                let mut lines = 0usize;
                for seg in text.split('\n') {
                    lines += 1;
                    lines += wrapper
                        .wrap_line_css(&[gpui::LineFragment::text(seg)], gpui::px(width.max(0.0)))
                        .filter(|b| seg.as_bytes().get(b.ix.wrapping_sub(1)) == Some(&b' '))
                        .count();
                }
                lines
            },
        ));
    });
    install_probe(move |family, size| {
        // Родовое имя системе шрифтов отдавать нельзя: `sans-serif` — это не
        // шрифт, а разряд, и поиск по нему кончается ничем. Подставляется то
        // же семейство, что и в каскаде за `sans-serif`.
        let name: gpui::SharedString = if family.is_empty() {
            crate::style::computed::GENERIC_SANS.into()
        } else {
            family.to_string().into()
        };
        let font = gpui::font(name);
        let id = text_system.resolve_font(&font);
        let size = gpui::px(size);
        let ch = text_system
            .ch_advance(id, size)
            .map(f32::from)
            .unwrap_or(0.0);
        // `line-height: normal` — это НЕ постоянная доля кегля, а метрика
        // шрифта: подъём плюс спуск (и зазор строк, если он есть). У Ahem она
        // ровно кегль, у текстовых шрифтов около 1.15–1.3 — из-за постоянной
        // 1.31 соседние коробки одной страницы расходились по высоте строк.
        let line =
            f32::from(text_system.ascent(id, size)) - f32::from(text_system.descent(id, size));
        // `ic` — продвижение знака `水`. Шрифт без него отдаёт запасной глиф,
        // и такой замер отбрасывается в пользу целого кегля.
        // A font without `水` must not answer with its .notdef advance
        // (DirectWrite maps a missing character to glyph 0; Times New Roman
        // gave 0.78em and `4ic` came out a quarter short,
        // `white-space-intrinsic-size-022`): the measure then falls back to
        // 1em (css-values-4 `ic`; Blink `SimpleFontData::
        // IdeographicInlineSize` returns nothing without the glyph).
        let ic = text_system
            .advance(id, size, '水')
            .map(|a| f32::from(a.width))
            .ok()
            .filter(|_| text_system.has_glyph(id, '水'))
            .unwrap_or(0.0);
        (ch, f32::from(text_system.x_height(id, size)), line, ic)
    });
    // Вертикальные метрики того же семейства: подъём и спуск ПОРОЗНЬ (в GPUI
    // спуск отрицателен) плюс высота прописной — по ним `text-box-trim`
    // считает срез (css-inline-3 §4.2).
    install_vprobe(move |family, size| {
        let name: gpui::SharedString = if family.is_empty() {
            crate::style::computed::GENERIC_SANS.into()
        } else {
            family.to_string().into()
        };
        let id = text_system2.resolve_font(&gpui::font(name));
        let size = gpui::px(size);
        (
            f32::from(text_system2.ascent(id, size)),
            -f32::from(text_system2.descent(id, size)),
            f32::from(text_system2.cap_height(id, size)),
        )
    });
}
