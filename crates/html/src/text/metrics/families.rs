//! Замер долей кегля по семейству (ширина нуля, высота строчной, `normal`, продвижение `水`):
//! один замер щупом на настоящее имя семейства (имя `@font-face` разворачивается через подмену,
//! пустое — шрифт документа), кэш документа, запасные значения при неудавшемся замере.
//! Здесь же семейство документа и список установленных семейств системы.

use super::{FALLBACK, PROBE_SIZE};
use crate::document::state::ValueCx;

// Доли кегля для семейства: замер идёт один раз и запоминается.
//
// Имя из разметки может быть ПРИДУМАННЫМ (`@font-face`): система шрифтов
// знает файл под его собственным именем из name-таблицы. Набор подмену уже
// делает (`inline::run_for`, `render::measure_font`), а замер — нет, и
// DirectWrite на неизвестное имя молча подставлял системный UI-шрифт
// (`direct_write.rs`, `select_font`): `line-height: normal` считался по
// ЧУЖИМ метрикам. Для `WOFF Test` это 1.33 вместо 1.0 — строка съезжала на
// полулидинг, 33 точки при кегле 200 (весь набор `css/WOFF2`).
//
// Замер запоминается по НАСТОЯЩЕМУ имени: придуманное на соседней странице
// значит другой файл, а имя семейства в системе одно на всех.
/// Set the default family used by this document's measurements.
pub fn set_doc_family(family: &str, vcx: ValueCx<'_>) {
    vcx.doc.metrics.family.replace(family.to_owned());
}

pub(super) fn fractions(family: &str, vcx: ValueCx<'_>) -> (f32, f32, f32, f32) {
    // Пустое семейство — это шрифт документа, а не родовой sans: щуп мерил
    // его как Segoe UI, и `1ch` выходил 0.56 кегля при наборе Times New Roman
    // с нулём в 0.5 (`white-space-wrap-after-nowrap-001`: «12345 67890»
    // влезало в `width: 10ch`). Та же развилка, что у `normal_fraction`.
    let doc;
    let family = if family.is_empty() {
        doc = vcx.doc.metrics.family.borrow().clone();
        if doc.is_empty() { family } else { doc.as_str() }
    } else {
        family
    };
    let real = crate::text::fonts::alias(family, vcx);
    let family = real.as_deref().unwrap_or(family);
    if let Some(hit) = vcx.doc.metrics.cache.borrow().get(family).copied() {
        return hit;
    }
    let measured = {
        vcx.doc
            .metrics
            .probe
            .borrow()
            .as_ref()
            .map(|probe| probe(family, PROBE_SIZE))
            .or_else(|| super::text_system::horizontal(family, PROBE_SIZE, vcx))
            .map(|(ch, ex, line, ic)| {
                (
                    ch / PROBE_SIZE,
                    ex / PROBE_SIZE,
                    line / PROBE_SIZE,
                    ic / PROBE_SIZE,
                )
            })
    };
    // Нулевая метрика — это не «шрифт шириной ноль», а неудавшийся замер:
    // такой ответ хуже запасного значения, потому что схлопывает коробку.
    let out = match measured {
        Some((ch, ex, line, ic)) if ch > 0.0 && ex > 0.0 && line > 0.0 => {
            (ch, ex, line, if ic > 0.0 { ic } else { FALLBACK.3 })
        }
        Some((ch, _, line, ic)) if ch > 0.0 => (
            ch,
            FALLBACK.1,
            if line > 0.0 { line } else { FALLBACK.2 },
            if ic > 0.0 { ic } else { FALLBACK.3 },
        ),
        _ => FALLBACK,
    };
    if let Some((_, _, line, _)) = measured
        && line > 0.0
        && line == out.2
    {
        let (a, d, _) = super::vmetrics_px(family, 1.0, vcx);
        super::normal_px::register(line, a, d, vcx);
    }
    vcx.doc
        .metrics
        .cache
        .borrow_mut()
        .insert(family.to_string(), out);
    out
}

/// Есть ли такое семейство в системе шрифтов. До `use_text_system` список пуст,
/// и ответ отрицательный для всех — разбор тогда работает как раньше.
pub fn font_installed(name: &str, vcx: ValueCx<'_>) -> bool {
    let lower = name.to_ascii_lowercase();
    vcx.doc.metrics.platform.borrow().installed.contains(&lower)
}

/// Снят ли список установленных семейств (`use_text_system`). До него
/// `font_installed` отрицателен для всех, и судить о «недоступности» имени
/// нельзя.
pub fn fonts_known(vcx: ValueCx<'_>) -> bool {
    !vcx.doc.metrics.platform.borrow().installed.is_empty()
}
