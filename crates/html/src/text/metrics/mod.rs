//! Метрики шрифта для единиц `ch` и `ex`.
//!
//! `1ch` — ширина нуля выбранного шрифта, `1ex` — высота строчной буквы. Обе
//! зависят от ГЛИФОВ, а не от кегля: у служебного Ahem нуль занимает целый
//! кегль, у текстового шрифта — около половины. Разбор значений живёт далеко
//! от системы шрифтов, поэтому доли кегля берутся здесь через щуп, который
//! ставит вызывающий: у него есть `TextSystem`, у нас — нет.
//!
//! Пока щуп не поставлен, работает запасное значение спецификации (полкегля):
//! CSS сам разрешает его, когда нужного глифа в шрифте не нашлось.

mod families;
mod font_loader;
mod platform;
mod state;
use crate::document::state::ValueCx;
pub use platform::FontContext;
pub(crate) use state::Metrics;
#[cfg(test)]
mod tests;
mod text_system;
pub use crate::text::metrics::families::font_installed;
pub use crate::text::metrics::families::fonts_known;
use crate::text::metrics::families::fractions;
pub use crate::text::metrics::families::set_doc_family;
pub use crate::text::metrics::text_system::use_text_system;
pub(crate) use crate::text::metrics::text_system::{font_extents, math_metrics};
pub(crate) use text_system::{install_font_loader, load_font};

mod spacing;
pub use crate::text::metrics::spacing::spacing_px;
mod space;
pub use crate::text::metrics::space::{install_space_probe, space_advance};

/// Щуп: по имени семейства и кеглю отдаёт ширину нуля и высоту строчной.
type Probe = Box<dyn Fn(&str, f32) -> (f32, f32, f32, f32)>;

/// Щуп ВЕРТИКАЛЬНЫХ метрик: подъём, спуск и высота прописной в долях кегля.
///
/// Отдельно от `Probe`, потому что нужен только `text-box-trim`
/// (css-inline-3 §4.2 и §4.3): у краёв `text`, `cap` и `ex` разные метрики,
/// а сумма «подъём + спуск», которую уже отдаёт `Probe`, ни одну из них не
/// восстанавливает.
type VProbe = Box<dyn Fn(&str, f32) -> (f32, f32, f32)>;

/// Доли кегля, когда щуп вертикальных метрик не поставлен: подъём 0.8,
/// спуск 0.2, прописная 0.7 — обычные значения текстового шрифта.
const V_FALLBACK: (f32, f32, f32) = (0.8, 0.2, 0.7);

/// Кегль, на котором меряются метрики. Метрики линейны по кеглю, поэтому
/// хватает одного замера на семейство: остальное — умножение.
const PROBE_SIZE: f32 = 100.0;

/// Доли кегля по спецификации, когда глифа нет или щуп не поставлен.
// Продвижение иероглифа без замера — целый кегль: так его определяет
// спецификация для шрифта, в котором знака `水` нет (CSS Values §6.1.4).
const FALLBACK: (f32, f32, f32, f32) = (0.5, 0.5, 1.2, 1.0);

/// Моноширинные семейства в порядке предпочтения: своё встроенное, затем то,
/// что подставляет за `font-family: monospace` браузер на этой системе.
const MONO_FAMILIES: [&str; 3] = ["JetBrains Mono", "Consolas", "Courier New"];

mod normal_px;
pub use normal_px::{content_px, normal_px};

/// Щуп переноса строк: сколько строк займёт текст шрифтом `font` кеглем `size`
/// при ширине `width`. `\n` — принудительный разрыв.
type WrapProbe = Box<dyn Fn(&gpui::Font, f32, &str, f32) -> usize>;

/// Число строк текста при сборке дерева — для фрагментации ПО СТРОКАМ
/// (css-break-3 §4.3: «Between line boxes» — законная точка разрыва). Перенос
/// тот же, что у текстового пути колонок (`float.rs` `measure_columns`):
/// переносчик GPUI, аварийный разрыв внутри слова отбрасывается (строка его не
/// рвёт, `overflow-wrap: normal`). `None` — щуп не поставлен.
pub fn line_count(
    font: &gpui::Font,
    size: f32,
    text: &str,
    width: f32,
    vcx: ValueCx<'_>,
) -> Option<usize> {
    vcx.doc
        .metrics
        .wrap
        .borrow()
        .as_ref()
        .map(|f| f(font, size, text, width))
}

/// Поставить щуп вертикальных метрик. Зовётся оттуда же, откуда `install_probe`.
pub fn install_vprobe(probe: impl Fn(&str, f32) -> (f32, f32, f32) + 'static, vcx: ValueCx<'_>) {
    *vcx.doc.metrics.vprobe.borrow_mut() = Some(Box::new(probe));
    vcx.doc.metrics.vcache.borrow_mut().clear();
    vcx.doc.metrics.parts.borrow_mut().clear();
}

/// Подъём, спуск и высота прописной в ТОЧКАХ для семейства и кегля.
///
/// Метрики линейны по кеглю, поэтому замер идёт один раз на семейство
/// (`PROBE_SIZE`), а дальше — умножение.
pub fn vmetrics_px(family: &str, size_px: f32, vcx: ValueCx<'_>) -> (f32, f32, f32) {
    // Имя из `@font-face` — придумка страницы: `font-family: CSSTest` при
    // файле, чьё семейство `CSSTest Basic`. Щуп по придуманному имени
    // получал ПОДСТАНОВОЧНЫЙ шрифт, и полулидинг среза расходился со
    // строкой, которую рисует настоящее лицо (`text-box-trim-start-002`,
    // `-end-003`). `fractions()` разворачивает имя так же.
    let real = crate::text::fonts::alias(family, vcx);
    let family = real.as_deref().unwrap_or(family);
    let key = family.to_ascii_lowercase();
    if let Some(hit) = vcx.doc.metrics.vcache.borrow().get(&key).copied() {
        return (hit.0 * size_px, hit.1 * size_px, hit.2 * size_px);
    }
    let got = {
        vcx.doc
            .metrics
            .vprobe
            .borrow()
            .as_ref()
            .map(|probe| probe(family, PROBE_SIZE))
            .or_else(|| text_system::vertical(family, PROBE_SIZE, vcx))
            .map(|(a, d, c)| (a / PROBE_SIZE, d / PROBE_SIZE, c / PROBE_SIZE))
    };
    let f = got.unwrap_or(V_FALLBACK);
    vcx.doc.metrics.vcache.borrow_mut().insert(key, f);
    (f.0 * size_px, f.1 * size_px, f.2 * size_px)
}

/// Семейство за родовое `monospace`.
///
/// Приложение встраивает JetBrains Mono, поэтому в нём ответ всегда один. А
/// вот стенды и чужие машины его не несут: подстановка системой даёт
/// пропорциональный шрифт, и всё, что держится на равной ширине знака,
/// разъезжается. Поэтому семейство выбирается из НАЛИЧНЫХ.
pub fn mono_family(vcx: ValueCx<'_>) -> &'static str {
    MONO_FAMILIES
        .into_iter()
        .find(|name| font_installed(name, vcx))
        .unwrap_or(MONO_FAMILIES[0])
}

/// Семейство за родовое `monospace` с учётом языка текста. Хром на Windows
/// берёт моноширинный шрифт ПО ПИСЬМЕННОСТИ (настройка «fixed font» для
/// японского — MS Gothic, `chrome/app/resources/locale_settings_win.grd`):
/// кана и иероглифы в нём полноширинные, а системная подстановка за
/// Consolas давала пропорциональную кану в 0.82 кегля
/// (`hanging-punctuation-block-bound-001`: пять знаков в строке вместо
/// четырёх).
pub fn mono_family_for(lang: Option<&str>, vcx: ValueCx<'_>) -> &'static str {
    let ja = lang.is_some_and(|l| {
        let l = l.trim().to_ascii_lowercase();
        l == "ja" || l.starts_with("ja-")
    });
    if ja && font_installed("MS Gothic", vcx) {
        return "MS Gothic";
    }
    mono_family(vcx)
}

/// Override this document's measurements and invalidate its horizontal cache.
pub fn install_probe(
    probe: impl Fn(&str, f32) -> (f32, f32, f32, f32) + 'static,
    vcx: ValueCx<'_>,
) {
    *vcx.doc.metrics.probe.borrow_mut() = Some(Box::new(probe));
    vcx.doc.metrics.cache.borrow_mut().clear();
    vcx.doc.metrics.parts.borrow_mut().clear();
}

/// Длина `1ch` и `1ex` в точках для семейства и кегля.
pub fn ch_ex_px(family: &str, size_px: f32, vcx: ValueCx<'_>) -> (f32, f32) {
    let (ch, ex, _, _) = fractions(family, vcx);
    (ch * size_px, ex * size_px)
}

/// Продвижение знака `水` — единица `ic` (CSS Values §6.1.4).
///
/// Своя метрика, а не синоним `em`: у текстового шрифта иероглиф либо шире
/// кегля, либо его нет вовсе, и подмена кеглем врала на четверть
/// (`ic-unit-*`).
pub fn ic_px(family: &str, size_px: f32, vcx: ValueCx<'_>) -> f32 {
    fractions(family, vcx).3 * size_px
}

/// Высота строки при `line-height: normal` — доля кегля по метрикам шрифта.
pub fn normal_line(family: &str, vcx: crate::document::state::ValueCx<'_>) -> f32 {
    crate::text::fonts::metric_overrides::normal_line(family, fractions(family, vcx).2, vcx)
}

/// Метрика шрифта в ДОЛЯХ КЕГЛЯ для `font-size-adjust` (css-fonts-5): 0 —
/// `ex-height`, 1 — `cap-height`, 2 — `ch-width`, 3 — `ic-width`,
/// 4 — `ic-height`.
///
/// Вертикального продвижения щуп не меряет. У полноширинного иероглифа оно
/// равно горизонтальному, а без знака `水` обе метрики по css-values-4 §6.1.4
/// считаются целым кеглем (`FALLBACK.3`).
pub fn adjust_aspect(family: &str, metric: u8, vcx: ValueCx<'_>) -> Option<f32> {
    let (ch, ex, _, ic) = fractions(family, vcx);
    Some(match metric {
        0 => ex,
        1 => vmetrics_px(family, 1.0, vcx).2,
        2 => ch,
        3 | 4 => ic,
        _ => return None,
    })
}
