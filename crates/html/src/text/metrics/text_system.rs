//! Привязка метрик к системе шрифтов gpui, явно заданной документу: загрузчик шрифтов страницы, список
//! установленных семейств и замеры без кэша — ширина нуля, x-height, `normal`, продвижение `水`
//! (`horizontal`), подъём/спуск/прописная (`vertical`), число строк при переносе. Здесь же
//! подъём и спуск выбранного лица для края содержимого руби (css-ruby-1 §3.4) с учётом
//! `*-override` и метрики математического лица. Кэш долей живёт в состоянии документа.

use super::space;

use crate::document::state::ValueCx;

pub(crate) fn install_font_loader(loader: crate::text::fonts::Loader, vcx: ValueCx<'_>) {
    vcx.doc.metrics.platform.borrow_mut().loader =
        Some(std::rc::Rc::from(super::font_loader::deduplicated(loader)));
}

pub(crate) fn load_font(bytes: Vec<u8>, vcx: ValueCx<'_>) -> Option<String> {
    let loader = vcx.doc.metrics.platform.borrow().loader.clone();
    loader.as_ref().and_then(|load| load(bytes))
}

/// Ruby's content edge uses the selected face, including generic family,
/// weight and style (CSS Ruby 1 §3.4; Blink ruby_utils.cc ComputeEmHeight).
pub(crate) fn font_extents(font: &gpui::Font, size: f32, vcx: ValueCx<'_>) -> (f32, f32) {
    let system = vcx.doc.metrics.platform.borrow();
    if let Some(system) = system.system.as_ref() {
        let id = system.resolve_font(font);
        crate::text::fonts::metric_overrides::extents(
            font,
            size,
            (
                f32::from(system.ascent(id, gpui::px(size))),
                f32::from(system.descent(id, gpui::px(size))).abs(),
            ),
        )
    } else {
        let (ascent, descent, _) = super::vmetrics_px(&font.family, size, vcx);
        (ascent, descent)
    }
}

/// MathML fallback constants use the math face's x-height and rule thickness.
pub(crate) fn math_metrics(font: &gpui::Font, size: f32, vcx: ValueCx<'_>) -> (f32, f32) {
    if let Some(system) = vcx.doc.metrics.platform.borrow().system.as_ref() {
        let id = system.resolve_font(font);
        (
            f32::from(system.x_height(id, gpui::px(size))),
            f32::from(system.underline_thickness(id, gpui::px(size))),
        )
    } else {
        (size / 2.0, 0.0)
    }
}

/// Поставить щуп поверх системы шрифтов GPUI.
///
/// Вызывается для документа после регистрации шрифтов приложения:
/// до неё `Ahem` ещё не найден и замер вернул бы метрики подмены.
pub fn use_text_system(text_system: std::sync::Arc<gpui::TextSystem>, vcx: ValueCx<'_>) {
    let loader = vcx.doc.metrics.platform.borrow().loader.clone();
    let mut platform = super::FontContext::new(text_system);
    platform.loader = loader;
    platform.install(vcx);
}

pub(super) fn bind_measurements(text_system: std::sync::Arc<gpui::TextSystem>, vcx: ValueCx<'_>) {
    space::use_text_system(text_system.clone(), vcx);
    let text_system3 = text_system;
    *vcx.doc.metrics.wrap.borrow_mut() = Some(Box::new(
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
}

pub(super) fn horizontal(
    family: &str,
    size: f32,
    vcx: ValueCx<'_>,
) -> Option<(f32, f32, f32, f32)> {
    let system = vcx.doc.metrics.platform.borrow();
    let text_system = system.system.as_ref()?;

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
    let line = f32::from(text_system.ascent(id, size)) - f32::from(text_system.descent(id, size))
        + f32::from(text_system.line_gap(id, size));
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
    Some((ch, f32::from(text_system.x_height(id, size)), line, ic))
}

pub(super) fn vertical(family: &str, size: f32, vcx: ValueCx<'_>) -> Option<(f32, f32, f32)> {
    let system = vcx.doc.metrics.platform.borrow();
    let text_system = system.system.as_ref()?;

    let name: gpui::SharedString = if family.is_empty() {
        crate::style::computed::GENERIC_SANS.into()
    } else {
        family.to_string().into()
    };
    let id = text_system.resolve_font(&gpui::font(name));
    let size = gpui::px(size);
    Some((
        f32::from(text_system.ascent(id, size)),
        -f32::from(text_system.descent(id, size)),
        f32::from(text_system.cap_height(id, size)),
    ))
}
