//! Запасная ветка абзаца — гибкий ряд из отдельных слов и атомов с переносом, когда единым
//! текстовым блоком его не собрать (не-текстовый кусок в строке). Куски режутся на слова,
//! склеенные с атомом группы идут одним ребёнком, `<br>` и сохранённый перевод строки —
//! распоркой во всю ширину, `text-align` — выключкой ряда, `text-indent` — пустой коробкой,
//! внепоточные куски — нулевой распоркой статической позиции (`overlay_in_row`), ряд из
//! одних атомов при `rtl` разворачивается (css-writing-modes-4 §2.4, UAX #9 L2).

use super::glue_atoms;
use crate::style::computed::Computed;
use crate::text::inline::*;
use gpui::{AnyElement, ParentElement, Styled};

/// Запасная ветка: гибкая строка из отдельных СЛОВ.
///
/// Не зеркалить прижим развёрнутого ряда при `direction: rtl` (`Right` →
/// `justify_start()`): `justify_start()` это `JustifyContent::Start`, а taffy
/// считает `Start` от ФИЗИЧЕСКОГО начала и `row-reverse` его не разворачивает
/// (`taffy/.../alignment.rs`, разворот учтён только у `FlexStart`/`FlexEnd`) —
/// развёрнутый ряд прижимался к ЛЕВОМУ краю (`abs-pos-non-replaced-icb-vlr-
/// 005/-013`, `-vrl-004/-012`, `ruby-bidi-002`). Прижим ниже — физический.
///
/// Сюда абзац попадает, когда единым текстовым блоком его не собрать: разные
/// кегли в строке или не-текстовый кусок посреди неё. Кусок целиком гибкая
/// строка перенести не может — он неделим, и раскладка ужимает его до самого
/// узкого слова: текст вставал столбиком и налезал на следующий абзац.
/// Поэтому куски режутся на слова: перенос идёт по ним, как в строке.
// Inline construction consumes the independently cascaded text and box properties.
#[allow(clippy::too_many_arguments)]
pub fn as_wrapped_row(
    pieces: Vec<Piece>,
    align: Option<crate::style::computed::Align>,
    text_align: Option<crate::style::computed::TextAlign>,
    // Уровень абзаца по HL1 (css-writing-modes-4 §2.4): `direction`
    // содержащего блока. В `text_align` он уже растворён физической стороной,
    // а порядку коробок в строке нужен сам признак.
    rtl: bool,
    indent: f32,
    nowrap: bool,
    render_text: &mut dyn FnMut(String, &Computed) -> AnyElement,
    // Ряд прозрачен для долей высоты (см. `percent_basis_from_parent` ниже):
    // только в горизонтальной строке. Повёрнутый абзац (`VerticalText`)
    // меряется отдельным корнем, и его «родитель» — не блок строки
    // (`horizontal-rule-vlr-003`).
    pct_from_parent: bool,
    // Block flow of a turned vertical line (`Some(vertical-rl)`): an
    // out-of-flow box's static position hangs from the block-start side.
    turned: Option<bool>,
    // Слои содержащих блоков сборки (`PositioningContext::state`): куда уходят
    // внепоточные куски строки.
    cx: &crate::layout::cx::LayoutCx,
) -> AnyElement {
    use crate::style::computed::{Align, TextAlign};
    // `vertical-align` в строке: умолчание — базовая линия, но `middle`,
    // `top` и `bottom` встречаются и раньше не доезжали никуда, кроме
    // ячейки таблицы.
    // Не класть минимум высоты ряда в струт строки (§10.8), чтобы ряд из одних
    // картинок не садился на их высоту: ряд переносится, и минимум ложится на
    // ВСЮ пачку строк, а не на каждую. Вернуть можно только вместе с настоящей
    // строчной раскладкой атомов.
    // `white-space: nowrap` — строка НЕ переносится (css-text-3 §3): ряд
    // остаётся один и вылезает за край. До ряда значение не доходило вовсе,
    // оно доезжало только до текстового стиля, и раскладка переносила атомы.
    // Перенос ряда нужен и для ЖЁСТКИХ разрывов: `<br>` и сохранённый перевод
    // строки выражаются распоркой на всю ширину, а она работает только в
    // переносящемся ряду. `nowrap` запрещает лишь МЯГКИЙ перенос (css-text-3
    // §3), поэтому при жёстком разрыве перенос ряда остаётся.
    let hard_break = pieces.iter().any(|p| match p {
        Piece::Text { text, .. } => text.contains('\n'),
        _ => false,
    });
    // Правило L2 двунаправленного алгоритма для строки БЕЗ ЗНАКОВ.
    // css-writing-modes-4 §2.4: атомарные строчные коробки «are treated as
    // neutral characters»; нейтралы без сильных соседей получают уровень
    // абзаца (UAX #9 N1/N2), при `rtl` это 1, и L2 разворачивает строку
    // целиком. Уровней 2 (латиница внутри rtl) без знаков не бывает, поэтому
    // разворот ряда — точная перестановка. Blink:
    // `LogicalLineBuilder::BidiReorder` (атом = U+FFFC).
    // Ряд со знаками не трогаем: у прогонов текста свои уровни. Жёсткий
    // разрыв не трогаем: он переставил бы сами строки. Нулевой щуп абсолюта
    // отмечает нейтральный объект в гипотетической строке (CSS2 §10.3.7),
    // поэтому получает ту же перестановку, что и соседний строчный атом.
    let reversed = rtl
        && !hard_break
        && pieces.iter().any(|p| matches!(p, Piece::Atom(_)))
        && pieces.iter().all(|p| match p {
            Piece::Atom(_) => true,
            Piece::Overlay(_, how) => how.line_probe.is_some(),
            Piece::Text { text, .. } => text.chars().all(|c| c.is_whitespace() || c == '\u{200b}'),
        });
    let mut row = gpui::div().flex().max_w_full();
    // Ряд строки — не коробка CSS: доли высоты атомов считаются от блока,
    // которому принадлежит строка (CSS 2.1 §10.1 п.2; `vendor/taffy`
    // `percent_basis_from_parent`). Доля доходит сюда, только если блок
    // определён (`cb_height_def`, гейт `paint/apply`), — иначе она уже `auto`.
    if pct_from_parent {
        row.style().percent_basis_from_parent = Some(true);
    }
    // Развёрнутый ряд кладёт первого ребёнка у ПРАВОГО края; перенос строк
    // при этом идёт по-прежнему вниз, а в каждую строку попадают куски в
    // логическом порядке — ровно как у rtl-строк в CSS.
    if reversed {
        row = row.flex_row_reverse();
    }
    if !nowrap || hard_break {
        row = row.flex_wrap();
    }
    row = match align {
        Some(Align::Center) => row.items_center(),
        Some(Align::Start) => row.items_start(),
        Some(Align::End) => row.items_end(),
        _ => row.items_baseline(),
    };
    // `text-align` прижимает СТРОКУ целиком, включая строчные коробки. Строка
    // из элементов — гибкий ряд, и прижим у него называется `justify-content`;
    // раньше свойство доходило только до текстового блока, и ряд из
    // `inline-block` оставался слева при `text-align: right`.
    //
    // Прижим ФИЗИЧЕСКИЙ и у развёрнутого ряда: `Start`/`End` taffy считает от
    // физического начала (`taffy/src/compute/common/alignment.rs`), и
    // `End` кладёт развёрнутую строку к правому краю ровно так же, как
    // `FlexStart` (первым идёт физически левый ребёнок, `flexbox.rs`,
    // смещение `free_space` у обоих). Ломает не физический прижим, а ЗЕРКАЛО
    // (`Right` → `justify_start()`), см. доку функции.
    // `FlexStart`/`FlexEnd` здесь нельзя: абсолют-ребёнок ряда (держатель
    // замещаемого с долей ширины, `atom_element`) ставится taffy по
    // `justify_content` БЕЗ учёта разворота (`flexbox.rs`: пара
    // `(FlexStart, false)` → начало), и rtl-статика уезжала к ЛЕВОМУ краю
    // (`absolute-replaced-width-020`: синий квадрат слева). Blink
    // разворот учитывает (`flex_layout_algorithm.cc`
    // `MainAxisStaticPositionEdge`), css-flexbox-1 §4.1 — «as if it were the
    // sole flex item». Без значения taffy сам берёт `FlexStart`
    // (`flexbox.rs`), то есть правый край — начало rtl-строки.
    row = match text_align {
        Some(TextAlign::Center) => row.justify_center(),
        Some(TextAlign::Right) => row.justify_end(),
        Some(TextAlign::Left) => row.justify_start(),
        _ => row,
    };
    // Отступ первой строки (§16.1) в ряду выражает пустая коробка первым
    // куском: текстовый путь несёт его полем `Indent`, а сюда абзац попадает,
    // когда в нём есть атом, и отступ пропадал молча.
    if indent != 0.0 {
        row = row.child(
            gpui::div()
                .w(gpui::px(indent))
                .h_0()
                .flex_shrink_0()
                .into_any_element(),
        );
    }
    // Пуста ли текущая строка ряда: распорка отступа строку не наполняет,
    // слово и атом — наполняют (см. `blank_line_break`).
    let mut line_empty = true;
    // strut пустой строки — только у ПЕРВОЙ строки с ненулевым отступом
    // (`text-indent-on-blank-line-rtl-left-align`). CSS 2.1 §9.4.2
    // (css2/Overview.bs) даёт strut и прочим строкам `<br>`, но тогда его
    // обязана получить и строка из одного атома (Blink ставит
    // `should_create_line_box` обоим: `line_breaker.cc`), а у ряда
    // атомов strut сознательно нет. Пустой `<br>` со strut при
    // атоме без него разводил пару `<svg height=0>` ↔ `<br>`: эталон
    // `mask-image-3-ref` уезжал на 19.2px, `mask-image-3a…3e, 3h` — 0.61.
    let mut indent_line = indent != 0.0;
    for group in glue_atoms(split_glued_tail(drop_hanging_tail(pieces))) {
        // Склеенная группа — свой НЕПЕРЕНОСИМЫЙ ряд: шва внутри него нет, и
        // атом уходит на новую строку вместе с приклеенным знаком. Ряд из
        // слов рвётся только по швам между детьми, поэтому «точки переноса
        // тут нет» выражается ровно одним общим ребёнком
        // (css-text-3 §5.1, пункт «atomic-compat-wrap»).
        if group.len() > 1 {
            line_empty = false;
            let mut glued = gpui::div().flex().flex_shrink_0().items_baseline();
            // Склеенная группа — часть той же строки: доли атомов — от блока.
            if pct_from_parent {
                glued.style().percent_basis_from_parent = Some(true);
            }
            for p in group {
                glued = match p {
                    Piece::Atom(el) => glued.child(el),
                    Piece::Overlay(_, how)
                        if how.cb_marker.is_some() || how.box_marker.is_some() =>
                    {
                        glued
                    }
                    Piece::Overlay(el, how) => glued.child(overlay_in_row(el, how, turned, cx)),
                    Piece::Text { text, style } => glued.child(render_text(text, &style)),
                };
            }
            row = row.child(glued);
            continue;
        }
        for p in group {
            row = match p {
                Piece::Atom(el) => {
                    line_empty = false;
                    row.child(el)
                }
                Piece::Overlay(_, how) if how.cb_marker.is_some() || how.box_marker.is_some() => {
                    row
                }
                Piece::Overlay(el, how) => row.child(overlay_in_row(el, how, turned, cx)),
                Piece::Text { text, style } => {
                    // Пробел остаётся при слове: без него слова слиплись бы.
                    for (n, part) in text.split('\n').enumerate() {
                        if n > 0 {
                            // Разрыв после содержимого — нулевая распорка, как
                            // прежде; разрыв ПУСТОЙ строки с отступом держит
                            // её strut (см. `indent_line`).
                            row = row.child(if line_empty && indent_line {
                                blank_line_break(&style)
                            } else {
                                line_break()
                            });
                            line_empty = true;
                            indent_line = false;
                        }
                        for w in part.split_inclusive(' ') {
                            if w.chars().any(|c| !matches!(c, ' ' | '\u{200b}')) {
                                line_empty = false;
                            }
                            row = row.child(render_text(w.to_string(), &style));
                        }
                    }
                    row
                }
            };
        }
    }
    row.into_any_element()
}
