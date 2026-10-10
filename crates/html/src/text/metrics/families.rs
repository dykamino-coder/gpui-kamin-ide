//! Families for metrics; split out to keep the owning module within 250 lines.

use super::CACHE;
use super::{FALLBACK, PROBE, PROBE_SIZE};
use std::cell::RefCell;

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
thread_local! {
    /// Семейство ДОКУМЕНТА (`RenderOpts::text.font_family`): им набирается
    /// текст без своего `font-family`. Ставит `render::render`.
    pub(super) static DOC_FAMILY: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Запомнить семейство документа для замеров без своего семейства.
pub fn set_doc_family(family: &str) {
    DOC_FAMILY.with(|d| {
        if d.borrow().as_str() != family {
            *d.borrow_mut() = family.to_string();
        }
    });
}

pub(super) fn fractions(family: &str) -> (f32, f32, f32, f32) {
    // Пустое семейство — это шрифт документа, а не родовой sans: щуп мерил
    // его как Segoe UI, и `1ch` выходил 0.56 кегля при наборе Times New Roman
    // с нулём в 0.5 (`white-space-wrap-after-nowrap-001`: «12345 67890»
    // влезало в `width: 10ch`). Та же развилка, что у `normal_fraction`.
    let doc;
    let family = if family.is_empty() {
        doc = DOC_FAMILY.with(|d| d.borrow().clone());
        if doc.is_empty() { family } else { doc.as_str() }
    } else {
        family
    };
    let real = crate::text::fonts::alias(family);
    let family = real.as_deref().unwrap_or(family);
    if let Some(hit) = CACHE.with(|c| c.borrow().get(family).copied()) {
        return hit;
    }
    let measured = PROBE.with(|p| {
        p.borrow()
            .as_ref()
            .map(|probe| probe(family, PROBE_SIZE))
            .map(|(ch, ex, line, ic)| {
                (
                    ch / PROBE_SIZE,
                    ex / PROBE_SIZE,
                    line / PROBE_SIZE,
                    ic / PROBE_SIZE,
                )
            })
    });
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
    CACHE.with(|c| c.borrow_mut().insert(family.to_string(), out));
    out
}

thread_local! {
    pub(super) static INSTALLED: std::cell::RefCell<std::collections::HashSet<String>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
}

/// Есть ли такое семейство в системе шрифтов. До `use_text_system` список пуст,
/// и ответ отрицательный для всех — разбор тогда работает как раньше.
pub fn font_installed(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    INSTALLED.with(|i| i.borrow().contains(&lower))
}

/// Снят ли список установленных семейств (`use_text_system`). До него
/// `font_installed` отрицателен для всех, и судить о «недоступности» имени
/// нельзя.
pub fn fonts_known() -> bool {
    INSTALLED.with(|i| !i.borrow().is_empty())
}
