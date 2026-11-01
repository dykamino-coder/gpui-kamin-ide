//! CSS-операнды группы text_box; значение за общим COW-указателем.
use super::super::*;
use crate::style::values::value::{Color, Len, StyleColor};
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResetTextBox {
    /// Paint-only fragment ink for a text-shadow mask; decoration records stay intact.
    pub text_shadow_ink: Option<Color>,
    pub underline: Option<bool>,
    pub line_through: Option<bool>,
    /// `match-parent` (css-text-3 §text-align): бит 1 — у `text-align-all`,
    /// бит 2 — у `text-align-last`; решается при наследовании по письму
    /// РОДИТЕЛЯ (`inherit_fonts`).
    pub text_align_match_parent: u8,
    /// Internal ruby unit promoted to a technical block for layout.
    pub ruby_unit: bool,
    /// Blink-compatible forced break suppression within ruby inline content.
    pub ruby_suppress_breaks: bool,
    /// `hanging-punctuation` — какая пунктуация выходит за край строки.
    /// `text-box-trim` — срезать полулидинг первой/последней строки блока.
    pub text_box_trim_start: bool,
    pub text_box_trim_end: bool,
    pub ellipsis: Option<bool>,
    /// Маркер обрезки `text-overflow: <string>` (css-overflow-4 §5);
    /// None при ellipsis — многоточие по умолчанию.
    pub overflow_marker: Option<String>,
    /// `text-overflow: inherit` (css-cascade-4 §7.3): свойство не
    /// наследуемое, значение родителя переносит `compute::explicit`
    /// (`text-overflow-004`).
    pub text_overflow_inherit: bool,
    /// Межбуквенный интервал ПОСЛЕ последнего знака этого куска.
    ///
    /// На границе двух элементов зазор задаёт не сам кусок, а ближайший общий
    /// предок обоих знаков (css-text-3 §8.2: зазор «задаётся и рисуется внутри
    /// самого внутреннего элемента, который содержит эту границу»). Видно эту
    /// границу только сборке кусков, она поле и ставит; из стилей документа
    /// оно не приходит и по дереву не наследуется.
    pub letter_spacing_after: Option<Len>,
    /// `text-shadow` с длинами в единицах шрифта ждёт своего кегля, как
    /// `shadow_raw` у `box-shadow` (вычисленное значение — «three absolute
    /// lengths», css-text-decor-3 Overview.bs:868-869), и разбирается в
    /// `resolve_em`.
    pub text_shadow_raw: Option<String>,
    /// `text-shadow: none`, записанное самим элементом: гасит унаследованный
    /// список (без флага пустой разбор читался как «не задано»).
    pub text_shadow_none: bool,
    /// Сдвиг куска по вертикали в долях кегля: `vertical-align: super` и
    /// `sub`. Не наследуется — принадлежит самому куску.
    pub vertical_shift: Option<f32>,
    /// То же, но ПРОЦЕНТОМ: доля считается от `line-height` куска, а не от
    /// кегля (§10.8.1), и хранить её вместе с `em` нельзя.
    pub vertical_shift_pct: Option<f32>,
    /// Сдвиг от базовой линии, названный ДЛИНОЙ: хранится в точках, потому
    /// что доля кегля на момент разбора ещё неизвестна — у строчного своего
    /// кегля обычно нет, он приходит наследованием.
    pub vertical_shift_px: Option<f32>,
    /// Сдвиг, названный единицей ШРИФТА (`ex`, `ch`): хранится сырым —
    /// метрики гарнитуры и кегль известны только при наборе строки.
    pub vertical_shift_len: Option<Len>,
    /// `vertical-align: text-top` (`true`) и `text-bottom` (`false`): край
    /// куска равняется по краю ТЕКСТОВОЙ области родителя, а не строки, —
    /// величина зависит от кеглей обоих и считается при наборе.
    pub vertical_align_text: Option<bool>,
    /// Кегль РОДИТЕЛЯ строчного куска: `text-top`/`text-bottom` равняются по
    /// его текстовой области, а не по самой высокой в строке.
    pub vertical_align_base: Option<f32>,
    /// Стиль первой буквы абзаца (`::first-letter`).
    ///
    /// Живёт в стиле, а не в элементе, потому что абзац собирается из кусков
    /// уже без узла-родителя: до кусков доезжает только вычисленный стиль.
    pub first_letter: Option<Box<Computed>>,
    /// Renderer metadata: marker text is outside first-letter selection.
    pub first_letter_excluded: bool,
    /// Own declarations remain separate for fictitious inheritance in descendants.
    pub first_letter_own: Option<Box<Computed>>,
    /// Стиль первой строки абзаца (`::first-line`).
    pub first_line: Option<Box<Computed>>,
    /// Только объявления `::first-line` самого узла — их получает первый
    /// блок-потомок, несущий первую строку (`render/first_line_descendants`).
    pub first_line_own: Option<Box<Computed>>,
    /// `initial-letter` (css-inline-3 §initial-letter): размер буквицы в
    /// строках и её осадка (sink) — на базовой какой строки она стоит.
    /// `None` — `normal`, обычная буква. Живёт в слое `::first-letter`.
    pub initial_letter: Option<(f32, u32)>,
    /// `text-decoration-line` самой коробки (биты `DECOR_*`); пусто — не
    /// задано (`none`). Не наследуется: потомкам линии достаются через
    /// `decors` (css-text-decor-3 §2 «propagated»).
    pub td_lines: Option<u8>,
    /// `text-decoration-style` (не наследуется).
    pub td_style: Option<DecorStyle>,
    /// `text-decoration-color` (не наследуется; начальное — `currentcolor`).
    pub td_color: StyleColor,
    /// `text-decoration-thickness` (не наследуется).
    pub td_thickness: Option<DecorLen>,
    /// `text-decoration-inset` (css-text-decor-4 §4.1, не наследуется):
    /// `None` — 0, `Some(None)` — `auto`.
    pub td_inset: Option<Option<[DecorLen; 2]>>,
    /// Украшения, наложенные на текст коробки её предками и ею самой
    /// (css-text-decor-3 §2.1), от внешнего к внутреннему.
    pub decors: Vec<Decor>,
    /// Роль руби-коробки из `display: ruby*` (css-ruby-1 §2.1). Не
    /// наследуется. `display` при этом остаётся строчным (`InlineBlock` +
    /// `inline_display`), у `block ruby` — `Block`: все `match` по `Display`
    /// остаются как есть, роль читается отдельно.
    pub ruby_role: Option<RubyRole>,
    /// Rotation model: inside the horizontal clone of a turned vertical paragraph
    /// (`text-combine-upright` and atoms need a counter-rotation). Inherited.
    pub turned_line: bool,
}
