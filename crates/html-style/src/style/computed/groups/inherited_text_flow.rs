//! CSS-операнды группы text_flow; значение за общим COW-указателем.
use super::super::*;
use crate::style::values::value::Len;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InheritedTextFlow {
    /// `orphans`/`widows` (css-break-3 §4.4 «Breaks Between Lines»): сколько
    /// строк блока обязано остаться до/после разрыва внутри него. Наследуются,
    /// начальное значение 2 (`None` = 2).
    pub orphans: Option<u16>,
    pub widows: Option<u16>,
    pub text_align: Option<TextAlign>,
    /// `text-align-last` — выключка ПОСЛЕДНЕЙ строки абзаца. Отдельное
    /// свойство, потому что по умолчанию последняя строка не растягивается:
    /// иначе абзац из одного слова разъехался бы во всю ширину. `Some(None)`
    /// — `auto`, `None` — от родителя.
    pub text_align_last: Option<Option<TextAlign>>,
    /// `text-justify: none` — выключка запрещена, строка идёт как `start`.
    pub no_justify: Option<bool>,
    /// CSS Text 4: ruby annotation justification excludes word spaces.
    pub ruby_justify: Option<bool>,
    /// `text-justify` expansion opportunities (css-text-3 §7.3): `Some(0)`
    /// `inter-word` (word separators only), `Some(2)` `inter-character` /
    /// `distribute` (between typographic character units), `None`/`Some(1)`
    /// `auto` (word separators plus CJK ideographs, as Blink).
    pub justify_chars: Option<u8>,
    /// `text-box-edge` — метрики верхнего и нижнего краёв среза.
    pub text_box_over: TextEdge,
    pub text_box_under: TextEdge,
    /// `text-box-edge` задан явно (в т.ч. `auto`/`text`): свойство
    /// наследуемое, и явное значение перекрывает унаследованное, а начальное
    /// `Text` от него неотличимо без флага
    /// (`text-box-trim-not-ignore-nested-text-box-edge`).
    pub text_box_edge_set: bool,
    pub hanging: Option<Hanging>,
    pub nowrap: Option<bool>,
    /// Переводы строк значимы (`white-space: pre*`).
    pub preserve_newlines: Option<bool>,
    /// Пробелы значимы. У `pre-line` переводы строк значимы, а пробелы нет —
    /// без отдельного поля он схлопывал и то и другое.
    pub keep_spaces: Option<bool>,
    pub letter_spacing: Option<Len>,
    /// `block-ellipsis` (css-overflow-4 §block-ellipsis): `None` — от
    /// родителя (у корня — начальное `no-ellipsis`), `Some(None)` — `ellipsis`
    /// (U+2026), пустая строка — `no-ellipsis`, иначе строка-знак. Отдельно
    /// от `overflow_marker`: `text-overflow` относится к строчной оси и
    /// знака на строке обрыва `line-clamp` не задаёт.
    pub clamp_mark: Option<Option<String>>,
    pub word_spacing: Option<Len>,
    pub text_transform: Option<TextTransform>,
    /// Добавки `text-transform` к регистру (css-text-3 §2.1: значение —
    /// `[ case ] || full-width || full-size-kana`, плюс `math-auto`
    /// MathML Core §2.1.5): `TT_FULL_WIDTH` | `TT_KANA` | `TT_MATH`. Живут
    /// вместе с `text_transform` и наследуются вместе с ним.
    pub text_transform_flags: u8,
    pub text_indent: Option<Len>,
    /// `text-indent: … each-line` — отступ повторяется после жёстких разрывов.
    pub text_indent_each_line: Option<bool>,
    /// `text-indent: … hanging` — отступ получают все строки, КРОМЕ первой.
    pub text_indent_hanging: Option<bool>,
    /// `word-break: break-all` и родня — рвать слово, а не переносить целиком.
    pub break_anywhere: Option<bool>,
    /// `overflow-wrap: break-word|anywhere` — рвать слово, ТОЛЬКО если иначе
    /// оно не влезает в строку целиком.
    pub break_word: Option<bool>,
    /// `text-wrap: balance` — строки абзаца одной длины.
    pub balance_lines: Option<bool>,
    /// `line-break: anywhere` — разрыв разрешён В ЛЮБОМ месте, включая
    /// соседство с пробелом. Это НЕ то же самое, что `word-break: break-all`:
    /// тот рвёт только внутри слова.
    pub break_anywhere_strict: Option<bool>,
    /// `line-break: normal` (1) / `loose` (2): уровень строгости переноса
    /// CJK (css-text-3 §5.2); `auto`/`strict`/`anywhere` — `None`.
    pub line_break_loose: Option<u8>,
    /// `word-break: keep-all` — иероглифическое письмо переносится ТОЛЬКО по
    /// пробелам, между знаками разрыв запрещён.
    pub keep_all: Option<bool>,
    /// `word-break: auto-phrase`: normal fallback with conditional hyphenation.
    pub auto_phrase: Option<bool>,
    /// `white-space: break-spaces` — сохранённый пробел НЕ свисает за край:
    /// он занимает место в строке, и после каждого такого пробела разрешён
    /// перенос. Отличие от `pre-wrap`, где хвостовые пробелы висят снаружи.
    pub break_after_spaces: Option<bool>,
    /// `-webkit-line-clamp`: действует ТОЛЬКО в паре с
    /// `display: -webkit-box` и `-webkit-box-orient: vertical`
    /// (css-overflow-3 §webkit-line-clamp) — поэтому своё поле и гейт.
    /// Какое сокращение записало `line_clamp` последним: `-webkit-line-clamp`
    /// ставит `continue: -webkit-legacy`, который действует только при
    /// `display: -webkit-box` с вертикальной ориентацией (css-overflow-4
    /// §5.1); оба сокращения — одни лонгхенды, побеждает последнее.
    pub clamp_legacy: Option<bool>,
    pub webkit_box_vertical: Option<bool>,
    /// `text-fit` — подбор кегля под ширину коробки; `Some(None)` — `none`.
    pub text_fit: Option<Option<TextFit>>,
    /// `hyphenate-character` — чем показывать перенос слова. Пусто — ничем.
    pub hyphen_char: Option<String>,
    /// `hyphens: auto` — слогораздел ставит сам движок, а не разметка.
    pub hyphens_auto: Option<bool>,
    /// Minimum word, prefix and suffix lengths; None components mean auto.
    pub hyphen_limit_chars: Option<[Option<usize>; 3]>,
    /// Maximum consecutive hyphenated lines; usize::MAX means no-limit.
    pub hyphen_limit_lines: Option<usize>,
    /// Язык узла (атрибут `lang`): по нему выбираются образцы слогораздела.
    pub lang: Option<String>,
    /// `vertical-align` внутри строки и ячейки таблицы.
    pub vertical_align: Option<Align>,
    /// `overflow-wrap: anywhere` — в отличие от `break-word`, меняет размер
    /// по минимальному содержимому.
    pub wrap_anywhere: Option<bool>,
    /// `word-space-transform` — чем ПОКАЗЫВАТЬ точку переноса (`<wbr>`,
    /// нулевой пробел): обычным пробелом или идеографическим. Точкой переноса
    /// она при этом быть не перестаёт.
    pub word_space_char: Option<char>,
    /// `text-autospace` — зазор в 1/8 кегля между иероглифом и соседней
    /// буквой или цифрой (css-text-4 §7). Наследуется, поэтому живёт здесь;
    /// сами зазоры расставляет `inline::autospace_pieces` по кускам абзаца.
    pub autospace_alpha: Option<bool>,
    pub autospace_numeric: Option<bool>,
    /// `hyphens`: разрешён ли перенос по мягкому переносу.
    pub hyphenate: Option<bool>,
    /// `tab-size` — во сколько пробелов раскрывается табуляция.
    pub tab_size: Option<f32>,
    /// `tab-size` в ДЛИНЕ: шаг табуляции задан не числом знаков, а величиной.
    /// Наследуется абсолютным (`tab-size-inheritance-001`), поэтому к детям
    /// уходит уже в точках — перевод делает `style::cascade::inherit`.
    pub tab_size_len: Option<Len>,
    /// `ruby-position` (css-ruby-1 §4.1): `Some(true)` — аннотация ПОД базой
    /// (`under`), `Some(false)` — над (`over`/`alternate`/`inter-character`),
    /// `None` — не задано. Наследуется (`style::cascade::inherit`). Прежде делил флаг
    /// с акцентом, и `text-emphasis-position: under` переворачивал руби.
    pub ruby_under: Option<bool>,
    /// `ruby-align` (css-ruby-1 §4.3); `None` — начальное `space-around`.
    pub ruby_align: Option<RubyAlign>,
    /// `ruby-overhang` (css-ruby-1 §4.4); `None` — начальное `auto`. Наследуется.
    pub ruby_overhang: Option<RubyOverhang>,
    /// CSS Ruby §ruby-merge: 0 separate, 1 merge, 2 auto.
    pub ruby_merge: Option<u8>,
    /// `word-break: break-word` (css-text-4 §5.2, устаревшее): своё поле
    /// `word-break`; используемое значение складывается с `overflow-wrap:
    /// anywhere` у читателя (`text::paragraph::breaking`).
    pub word_break_word: Option<bool>,
}
