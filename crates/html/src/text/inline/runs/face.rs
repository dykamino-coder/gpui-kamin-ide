//! Шрифт прогона и его кегль: вывод из унаследованных групп стиля и базового шрифта абзаца.
//! Внутри сборки документа результат общий для кусков с теми же группами (`RunFaces`).

use crate::layout::cx::run_faces::{Role, RunFace, RunFaces};
use crate::style::computed::Computed;
use crate::values::ValueCx;
use gpui::{FontStyle, FontWeight, TextStyle};

/// Шрифт, которым набирается кусок со стилем `style` в абзаце с базой `base`.
pub(super) fn run_face(
    style: &Computed,
    base: &TextStyle,
    base_px: f32,
    vcx: ValueCx<'_>,
    faces: Option<&RunFaces>,
) -> RunFace {
    let font = base.font();
    match faces {
        Some(memo) => memo.face(Role::Run, style, &font, base_px, vcx, || {
            derive(style, font.clone(), base_px, vcx)
        }),
        None => derive(style, font, base_px, vcx),
    }
}

fn derive(style: &Computed, mut font: gpui::Font, base_px: f32, vcx: ValueCx<'_>) -> RunFace {
    let font_style = &style.inh.font;
    font.fallbacks = crate::text::fonts::fallbacks::fallbacks(style, font.fallbacks, vcx);
    // Названное семейство сильнее родового; пустое имя сохраняет базовый шрифт.
    let face = vcx.doc.used_face(style);
    if let Some(family) = face.family {
        // Имя из разметки может быть придуманным (`@font-face`) — система
        // шрифтов знает файл под его собственным именем. Лиц у имени бывает
        // несколько, и нужное выбирает ширина начертания (§font-matching).
        font.family = family.into();
    } else if font_style.monospace == Some(true) {
        font.family =
            crate::text::metrics::mono_family_for(style.inh.text_flow.lang.as_deref(), vcx).into();
    }
    // Вес и курсив — ВСЕГДА от стиля куска: `None` в слитом стиле — это
    // обычное начертание, а не «как у базы» (база абзаца строится по
    // первому куску, и `abc<b>def</b>ghi` набирался одним прогоном его
    // веса — сквозной дефект по всему корпусу).
    font.weight = FontWeight(font_style.font_weight.unwrap_or(400) as f32);
    font.style = if font_style.italic == Some(true) {
        FontStyle::Italic
    } else {
        FontStyle::Normal
    };
    if let Some(pct) = font_style.font_stretch {
        font.stretch = gpui::FontStretch::from_percent(pct);
    }
    if !face.features.is_empty() {
        font.features = gpui::FontFeatures(face.features);
    }
    // Кегль куска: без него разный размер в строке собрать в один блок было
    // нельзя (см. `single_block`).
    // TRANSITIONAL(cascade-7b-font-size-adjust): A03-backend после F2 должен подгонять каждый нативный fallback face; TextRun пока передаёт только размер first available face.
    let font_size = font_style
        .font_size
        .map(|_| gpui::px(style.used_font(base_px, vcx).size_px));
    (font, font_size)
}
