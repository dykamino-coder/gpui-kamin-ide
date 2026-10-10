//! Line types for paragraph; split out to keep the owning module within 250 lines.

use gpui::Pixels;

/// Правила переноса, собранные из CSS.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wrap {
    /// `white-space: nowrap`/`pre` — мягких переносов нет вовсе.
    pub nowrap: bool,
    /// `white-space: break-spaces` — пробел занимает место и даёт разрыв.
    pub break_spaces: bool,
    /// `word-break: break-all` — разрыв между любыми знаками слова.
    pub break_all: bool,
    /// `line-break: anywhere` — разрыв где угодно, поверх запретов.
    pub anywhere: bool,
    /// `word-break: keep-all` — иероглифы не рвутся, только по пробелам.
    pub keep_all: bool,
    /// `overflow-wrap: break-word` — рвать слово, только если иначе не влезает.
    pub break_word: bool,
    /// `overflow-wrap: anywhere` — рвёт слово И при подсчёте размера по
    /// минимальному содержимому, в отличие от `break-word`.
    pub wrap_anywhere: bool,
    /// `direction: rtl` — основное направление абзаца.
    pub rtl: bool,
    /// `text-wrap: balance` — строки абзаца выравниваются по длине.
    pub balance: bool,
    /// `white-space: pre*` — пробелы сохраняются, в начале строки не срезаются.
    pub keep_spaces: bool,
    /// `line-break: normal` (1) / `loose` (2) — послабления UAX-14 для CJK
    /// (css-text-3 §5.2): разрыв перед малой каной и знаком долготы (класс
    /// CJ), в `loose` ещё и перед знаками повтора и между `…`.
    pub loose: u8,
    /// Язык куска — китайский или японский: часть послаблений §5.2 действует
    /// только «if the writing system is Chinese or Japanese».
    pub cjk_lang: bool,
}

/// Куда прижимать строку.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// Мягкий перенос: разрешение разорвать слово, своей ширины он не имеет.
pub(super) const SOFT_HYPHEN: char = '\u{00ad}';

/// Знак обрыва строки по `line-clamp`.
pub(super) const ELLIPSIS: &str = "…";

/// Строка: что рисовать и сколько она занимает.
#[derive(Clone, Debug)]
pub(crate) struct Line {
    /// Байтовый отрезок исходного текста.
    pub(crate) range: std::ops::Range<usize>,
    /// Ширина без хвостовых пробелов — по ней идёт выключка.
    pub(crate) width: Pixels,
    /// Строка оборвана `line-clamp`: за её текстом рисуется многоточие.
    pub(crate) ellipsis: bool,
    /// Знак обрыва поставил `line-clamp` (block-ellipsis), а не
    /// `text-overflow`: у него свой знак (см. `line_mark`).
    pub(crate) clamped: bool,
    /// Усечение `text-overflow` по ВИДИМОМУ порядку (строка со смешанным
    /// направлением): ширина видимой части от начального края строки.
    /// Отрезок строки при этом целый — скрытые знаки отсекает маска.
    pub(crate) vis_cut: Option<Pixels>,
    /// Строка кончилась мягким переносом: за ней рисуется знак переноса.
    pub(crate) hyphen: bool,
    /// Отступ строки (`text-indent`). Отрицательный выводит строку за край.
    pub(crate) indent: Pixels,
}

/// `text-indent`: отступ первой строки блока.
///
/// Значение хранится ДВУМЯ частями. Абсолютную (`px`) считает разбор стилей,
/// а доля (`pct`) берётся от ширины содержащего блока и известна только там,
/// где ширина уже решена, — в раскладке строк.
#[derive(Default, Clone, Copy, PartialEq)]
pub struct Indent {
    /// Абсолютная часть в точках; отрицательная выводит строку за край.
    pub px: f32,
    /// Доля ширины строки: `10%` — это `0.1`.
    pub pct: f32,
    /// `each-line`: отступ повторяется после каждого жёсткого разрыва.
    pub each_line: bool,
    /// `hanging`: отступ получают все строки, КРОМЕ той, что получила бы его.
    pub hanging: bool,
}
