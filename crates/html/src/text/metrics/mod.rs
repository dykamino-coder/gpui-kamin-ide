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
#[cfg(test)]
mod tests;
mod text_system;
pub use crate::text::metrics::families::font_installed;
pub use crate::text::metrics::families::fonts_known;
use crate::text::metrics::families::fractions;
pub use crate::text::metrics::families::set_doc_family;
pub use crate::text::metrics::text_system::use_text_system;

mod spacing;
pub use crate::text::metrics::spacing::spacing_px;
mod space;
pub use crate::text::metrics::space::{install_space_probe, space_advance};

use std::cell::RefCell;
use std::collections::HashMap;

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

thread_local! {
    static PROBE: RefCell<Option<Probe>> = const { RefCell::new(None) };
    static CACHE: RefCell<HashMap<String, (f32, f32, f32, f32)>> = RefCell::new(HashMap::new());
    static MONO: RefCell<&'static str> = const { RefCell::new(MONO_FAMILIES[0]) };
    static VPROBE: RefCell<Option<VProbe>> = const { RefCell::new(None) };
    static VCACHE: RefCell<HashMap<String, (f32, f32, f32)>> = RefCell::new(HashMap::new());
}

/// Щуп переноса строк: сколько строк займёт текст шрифтом `font` кеглем `size`
/// при ширине `width`. `\n` — принудительный разрыв.
type WrapProbe = Box<dyn Fn(&gpui::Font, f32, &str, f32) -> usize>;

thread_local! {
    static WRAP: RefCell<Option<WrapProbe>> = const { RefCell::new(None) };
}

/// Число строк текста при сборке дерева — для фрагментации ПО СТРОКАМ
/// (css-break-3 §4.3: «Between line boxes» — законная точка разрыва). Перенос
/// тот же, что у текстового пути колонок (`float.rs` `measure_columns`):
/// переносчик GPUI, аварийный разрыв внутри слова отбрасывается (строка его не
/// рвёт, `overflow-wrap: normal`). `None` — щуп не поставлен.
pub fn line_count(font: &gpui::Font, size: f32, text: &str, width: f32) -> Option<usize> {
    WRAP.with(|w| w.borrow().as_ref().map(|f| f(font, size, text, width)))
}

/// Поставить щуп вертикальных метрик. Зовётся оттуда же, откуда `install_probe`.
pub fn install_vprobe(probe: impl Fn(&str, f32) -> (f32, f32, f32) + 'static) {
    VPROBE.with(|p| *p.borrow_mut() = Some(Box::new(probe)));
    VCACHE.with(|c| c.borrow_mut().clear());
}

/// Подъём, спуск и высота прописной в ТОЧКАХ для семейства и кегля.
///
/// Метрики линейны по кеглю, поэтому замер идёт один раз на семейство
/// (`PROBE_SIZE`), а дальше — умножение.
pub fn vmetrics_px(family: &str, size_px: f32) -> (f32, f32, f32) {
    // Имя из `@font-face` — придумка страницы: `font-family: CSSTest` при
    // файле, чьё семейство `CSSTest Basic`. Щуп по придуманному имени
    // получал ПОДСТАНОВОЧНЫЙ шрифт, и полулидинг среза расходился со
    // строкой, которую рисует настоящее лицо (`text-box-trim-start-002`,
    // `-end-003`). `fractions()` разворачивает имя так же.
    let real = crate::text::fonts::alias(family);
    let family = real.as_deref().unwrap_or(family);
    let key = family.to_ascii_lowercase();
    if let Some(hit) = VCACHE.with(|c| c.borrow().get(&key).copied()) {
        return (hit.0 * size_px, hit.1 * size_px, hit.2 * size_px);
    }
    let got = VPROBE.with(|p| {
        p.borrow()
            .as_ref()
            .map(|probe| probe(family, PROBE_SIZE))
            .map(|(a, d, c)| (a / PROBE_SIZE, d / PROBE_SIZE, c / PROBE_SIZE))
    });
    let f = got.unwrap_or(V_FALLBACK);
    VCACHE.with(|c| c.borrow_mut().insert(key, f));
    (f.0 * size_px, f.1 * size_px, f.2 * size_px)
}

/// Семейство за родовое `monospace`.
///
/// Приложение встраивает JetBrains Mono, поэтому в нём ответ всегда один. А
/// вот стенды и чужие машины его не несут: подстановка системой даёт
/// пропорциональный шрифт, и всё, что держится на равной ширине знака,
/// разъезжается. Поэтому семейство выбирается из НАЛИЧНЫХ.
pub fn mono_family() -> &'static str {
    MONO.with(|m| *m.borrow())
}

/// Семейство за родовое `monospace` с учётом языка текста. Хром на Windows
/// берёт моноширинный шрифт ПО ПИСЬМЕННОСТИ (настройка «fixed font» для
/// японского — MS Gothic, `chrome/app/resources/locale_settings_win.grd`):
/// кана и иероглифы в нём полноширинные, а системная подстановка за
/// Consolas давала пропорциональную кану в 0.82 кегля
/// (`hanging-punctuation-block-bound-001`: пять знаков в строке вместо
/// четырёх).
pub fn mono_family_for(lang: Option<&str>) -> &'static str {
    let ja = lang.is_some_and(|l| {
        let l = l.trim().to_ascii_lowercase();
        l == "ja" || l.starts_with("ja-")
    });
    if ja && font_installed("MS Gothic") {
        return "MS Gothic";
    }
    mono_family()
}

/// Поставить щуп метрик. Вызывается один раз при старте: замер шрифта
/// возможен только там, где живёт система шрифтов.
pub fn install_probe(probe: impl Fn(&str, f32) -> (f32, f32, f32, f32) + 'static) {
    PROBE.with(|p| *p.borrow_mut() = Some(Box::new(probe)));
    CACHE.with(|c| c.borrow_mut().clear());
}

/// Длина `1ch` и `1ex` в точках для семейства и кегля.
pub fn ch_ex_px(family: &str, size_px: f32) -> (f32, f32) {
    let (ch, ex, _, _) = fractions(family);
    (ch * size_px, ex * size_px)
}

/// Продвижение знака `水` — единица `ic` (CSS Values §6.1.4).
///
/// Своя метрика, а не синоним `em`: у текстового шрифта иероглиф либо шире
/// кегля, либо его нет вовсе, и подмена кеглем врала на четверть
/// (`ic-unit-*`).
pub fn ic_px(family: &str, size_px: f32) -> f32 {
    fractions(family).3 * size_px
}

/// Длина в точках при НЕИЗВЕСТНОМ контексте (узел вне наследования, фон,
/// скругление): кегль — запасной, метрики — по семейству (пустое — шрифт-
/// подмена). Доля родителя, единицы окна и размеры по содержимому здесь не
/// решаются — их значение знает только вызывающий.
///
/// Единая точка: прежде этот match был дословно повторён в пяти местах
/// (apply/background/computed), и правка запасных значений расходилась.
pub fn fallback_len_px(
    l: crate::style::values::value::Len,
    family: &str,
    font_px: f32,
) -> Option<f32> {
    use crate::style::values::value::Len;
    Some(match l {
        Len::Px(v) => v,
        Len::Em(k) => k * font_px,
        Len::EmPx(k, add) => k * font_px + add,
        Len::Ch(k) => k * ch_ex_px(family, font_px).0,
        Len::Ic(k) => k * ic_px(family, font_px),
        Len::Ex(k) => k * ch_ex_px(family, font_px).1,
        Len::Lh(k) => k * 1.2 * font_px,
        Len::LhPx(k, add) => k * 1.2 * font_px + add,
        _ => return None,
    })
}

/// Высота строки при `line-height: normal` — доля кегля по метрикам шрифта.
pub fn normal_line(family: &str) -> f32 {
    fractions(family).2
}

/// Метрика шрифта в ДОЛЯХ КЕГЛЯ для `font-size-adjust` (css-fonts-5): 0 —
/// `ex-height`, 1 — `cap-height`, 2 — `ch-width`, 3 — `ic-width`,
/// 4 — `ic-height`.
///
/// Вертикального продвижения щуп не меряет. У полноширинного иероглифа оно
/// равно горизонтальному, а без знака `水` обе метрики по css-values-4 §6.1.4
/// считаются целым кеглем (`FALLBACK.3`).
pub fn adjust_aspect(family: &str, metric: u8) -> Option<f32> {
    let (ch, ex, _, ic) = fractions(family);
    Some(match metric {
        0 => ex,
        1 => vmetrics_px(family, 1.0).2,
        2 => ch,
        3 | 4 => ic,
        _ => return None,
    })
}

use crate::text::metrics::families::INSTALLED;
