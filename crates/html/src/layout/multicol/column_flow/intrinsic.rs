//! Внутренний строчный размер многоколоночного текстового потока.

use gpui::{Font, Pixels, Window, px};

/// Внутренний размер многоколоночного контейнера с ТЕКСТОВЫМ потоком.
///
/// css-sizing-4 `intrinsic-sizing-notes.bs` §multicol-intrinsic — единственное
/// письменное определение (css-multicol-1 §3.4 прямо отказывается его давать).
/// Порядок действий — как в Blink `ColumnLayoutAlgorithm::ComputeMinMaxSizes`.
/// Вклад содержимого у текста берут те же метрики, что и перенос
/// (`LineWrapper::min_content_width` / `max_content_width`), иначе замер и
/// перенос разойдутся между собой.
#[allow(clippy::too_many_arguments)]
pub(super) fn intrinsic_column_width(
    text: &str,
    count: Option<usize>,
    col_w: Option<f32>,
    gap: f32,
    font: &Font,
    font_size: f32,
    min: bool,
    window: &mut Window,
) -> Pixels {
    let mut wrapper = window
        .text_system()
        .line_wrapper(font.clone(), px(font_size));
    let (mut kid_min, mut kid_max) = (0.0f32, 0.0f32);
    // Жёсткие разрывы приходят переводом строки: каждый сегмент — свой абзац,
    // и вклад даёт самый широкий из них.
    for seg in text.split('\n') {
        kid_min = kid_min.max(f32::from(wrapper.min_content_width(seg)));
        kid_max = kid_max.max(f32::from(wrapper.max_content_width(seg)));
    }
    let n = count.unwrap_or(1).max(1) as f32;
    let gap_extra = gap * (n - 1.0);
    let (mut mn, mut mx) = (kid_min, kid_max);
    match col_w.filter(|w| *w > 0.0) {
        Some(w) => {
            mn = mn.min(w);
            mx = mx.max(w).max(mn);
        }
        None => mn = mn * n + gap_extra,
    }
    mx = mx * n + gap_extra;
    px(if min { mn } else { mx })
}
