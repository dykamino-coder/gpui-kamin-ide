//! Шрифты страницы: правило `@font-face`.
//!
//! Разметка вправе принести СВОЙ шрифт и назвать его как угодно:
//!
//! ```css
//! @font-face { font-family: 'мой'; src: url('/fonts/mplus.woff') }
//! p { font-family: 'мой' }
//! ```
//!
//! Система шрифтов знает файл под его СОБСТВЕННЫМ именем, а разметка просит
//! по своему — поэтому здесь два дела: загрузить файл в систему и запомнить,
//! какое настоящее имя стоит за придуманным. Подмену делает [`alias`], её
//! зовут все места, где семейство уходит в набор.
//!
//! Упаковки `woff` и `woff2` — это тот же sfnt: у первой сжаты таблицы, у
//! второй сверх того перестроены глифы. Распаковку делает крейт `wuff`.

mod font_faces;
mod loading;
mod sfnt_metrics;
mod stretch;
#[cfg(test)]
mod tests;
mod windows_names;
use crate::text::fonts::font_faces::declaration;
use crate::text::fonts::font_faces::faces;
use crate::text::fonts::font_faces::is_quote;
use crate::text::fonts::font_faces::read_font;
use crate::text::fonts::font_faces::source;
pub use crate::text::fonts::loading::covers_space;
pub use crate::text::fonts::loading::load_faces;
pub use crate::text::fonts::loading::load_faces_additive;
pub use crate::text::fonts::loading::size_adjust;
pub use crate::text::fonts::sfnt_metrics::sfnt_family;
use crate::text::fonts::sfnt_metrics::sfnt_romn_baseline;
pub use crate::text::fonts::stretch::alias_stretch;
use crate::text::fonts::stretch::stretch_desc;
use crate::text::fonts::windows_names::ensure_windows_names;

use std::cell::RefCell;
use std::collections::HashMap;

pub mod alternates;

thread_local! {
    /// Придуманное разметкой имя → имя, под которым шрифт знает система.
    static ALIASES: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    /// Придуманное имя → дескриптор `font-feature-settings` его правила
    /// (css-fonts-4 §7.2, шаг 2). Ключ — ИМЯ ИЗ РАЗМЕТКИ: три правила с одним
    /// файлом (`lato-ffs-`, `lato-ffs-0`, `lato-ffs-1`) дают одно настоящее
    /// имя, а возможности у них разные.
    static FEATURES: RefCell<HashMap<String, Vec<(String, u32)>>> =
        RefCell::new(HashMap::new());
    /// ВСЕ лица семейства в порядке объявления: ширина из дескриптора
    /// `font-stretch` (отрезок процентов) и имя в системе. Одного слота мало:
    /// правил `@font-face` на одно имя бывает много, и выбор между ними ведёт
    /// css-fonts-4 §font-matching, а не «кто объявлен последним».
    static FACES: RefCell<HashMap<String, Vec<((f32, f32), String)>>> =
        RefCell::new(HashMap::new());
    /// Придуманное имя → есть ли в семействе знак U+0020. Семейство собирают
    /// из нескольких правил (подмножества знаков), и пробел у него есть,
    /// если его несёт ХОТЬ ОДНО.
    static HAS_SPACE: RefCell<HashMap<String, bool>> = RefCell::new(HashMap::new());
    /// Придуманное имя → лица семейства с их множителем `size-adjust`
    /// (css-fonts-5): `(наклон, множитель)` на каждое правило, наклон — из
    /// дескриптора `font-style` (0 normal, 1 italic, 2 oblique). Множитель —
    /// свойство ЛИЦА: какое из них возьмёт элемент, решает подбор по наклону
    /// (`size_adjust`), а не семейство целиком.
    static SIZE_ADJUST: RefCell<HashMap<String, Vec<(u8, f32)>>> =
        RefCell::new(HashMap::new());
    /// Уже загруженные файлы: одно и то же правило встречается на странице
    /// не по разу, а разбор шрифта дорог.
    static LOADED: RefCell<HashMap<String, Option<String>>> =
        RefCell::new(HashMap::new());
    /// Приёмник шрифта: отдаёт системе байты и возвращает её имя семейства.
    static LOADER: RefCell<Option<Loader>> = const { RefCell::new(None) };
    /// Алфавитная базовая линия файла (`BASE`, тег `romn`) в долях em над
    /// нулём глифа — по адресу файла: байты есть только при первой загрузке.
    static ROMN_BY_SRC: RefCell<HashMap<String, Option<f32>>> = RefCell::new(HashMap::new());
    /// То же по придуманному страницей имени семейства (живёт одну страницу).
    static ROMN: RefCell<HashMap<String, f32>> = RefCell::new(HashMap::new());
}

/// Загрузчик: получает содержимое файла, отдаёт имя семейства в системе.
type Loader = Box<dyn Fn(Vec<u8>) -> Option<String>>;

/// Поставить загрузчик шрифтов. Зовут там, где живёт система шрифтов.
pub fn install_loader(loader: impl Fn(Vec<u8>) -> Option<String> + 'static) {
    LOADER.with(|l| *l.borrow_mut() = Some(Box::new(loader)));
}

/// Настоящее имя семейства за именем из разметки.
pub fn alias(family: &str) -> Option<String> {
    ALIASES.with(|a| a.borrow().get(&family.to_ascii_lowercase()).cloned())
}

/// Возможности из дескриптора `font-feature-settings` правила `@font-face`.
pub fn face_features(family: &str) -> Vec<(String, u32)> {
    FEATURES.with(|f| {
        f.borrow()
            .get(&family.to_ascii_lowercase())
            .cloned()
            .unwrap_or_default()
    })
}

/// Алфавитная базовая линия семейства в долях em над нулём глифа: у
/// `BaselineDiagnostic` она на 50/1000 выше нуля, у обычного шрифта — ноль
/// (css-inline-3 §4.3 `alphabetic`: «Use the alphabetic baseline»).
pub fn alphabetic_em(family: &str) -> f32 {
    let key = family.trim().trim_matches(is_quote).to_ascii_lowercase();
    ROMN.with(|r| r.borrow().get(&key).copied()).unwrap_or(0.0)
}

/// Подстановка шрифта под японскую кану в тексте документа.
///
/// Глиф, которого нет в первом доступном шрифте, ищется подстановкой
/// (css-fonts-4 §5.1 «system font fallback» — выбор за UA). Системная
/// подстановка DirectWrite отдаёт хирагану/катакану «Yu Gothic UI», где кана
/// ПРОПОРЦИОНАЛЬНАЯ: `あ` — 0.816em при `U+3000` в 1em (hmtx `YuGothR.ttc`).
/// Браузер подбирает шрифт по письменности (Blink: карта «письменность →
/// семейства» в `font_fallback_win.cc`, японская — текстовые «Yu Gothic»/
/// «Meiryo», у которых кана моноширинная в 1em, как и у Noto CJK на Linux),
/// и страницы на это рассчитывают: `ああ&#x3000;` должно совпасть с `あああ`
/// (`trailing-ideographic-space-*`), а `あ&#x2004;あ` не помещаться в 2em
/// (`trailing-other-space-separators-break-spaces-*`). Поэтому для каны
/// документ просит «Yu Gothic» (затем «Meiryo», «MS Gothic») — только в её
/// диапазонах; остальные знаки идут прежней системной подстановкой.
/// Интерфейс IDE подстановок не задаёт и этого не касается.
pub fn document_fallbacks() -> Option<gpui::FontFallbacks> {
    if !cfg!(windows) {
        return None;
    }
    static LIST: std::sync::OnceLock<gpui::FontFallbacks> = std::sync::OnceLock::new();
    // CJK Symbols and Punctuation, хирагана, катакана; фонетические
    // расширения катаканы.
    const KANA: &[(u32, u32)] = &[(0x3000, 0x30FF), (0x31F0, 0x31FF)];
    Some(
        LIST.get_or_init(|| {
            gpui::FontFallbacks::from_fonts(
                ["Yu Gothic", "Meiryo", "MS Gothic"]
                    .iter()
                    .map(|f| gpui::FontFallbacks::restricted(f, KANA))
                    .collect(),
            )
        })
        .clone(),
    )
}
