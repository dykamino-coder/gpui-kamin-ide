//! Wrapped row for runs; split out to keep the owning module within 250 lines.

use super::glue_atoms;
use crate::style::computed::Computed;
use crate::text::inline::*;
use gpui::{AnyElement, ParentElement, Styled};

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (08.09, v158, `scout-wm-2026-09f.md` K1): переворот
/// гибкого ряда из атомов при `direction: rtl` (`flex_row_reverse` + зеркало
/// `justify_start/end`, гейт «только атомы, без знаков и `<br>`»). Обещание
/// +24/−0 на `abs-pos-non-replaced-vrl-*`. Полный свод против v35: +0/−4
/// (`abs-pos-non-replaced-icb-vlr-005/-013`, `-vrl-004/-012` 0.00 → 1.16),
/// рядом `ruby-bidi-002` 0.16 → 0.72.
/// Причина отката — ПРИЖИМ, а не разворот: `justify_start()` это
/// `JustifyContent::Start`, а taffy считает `Start` от ФИЗИЧЕСКОГО начала и
/// `row-reverse` его не разворачивает (`taffy/.../alignment.rs:50-67`, учёт
/// разворота есть только у `FlexStart`/`FlexEnd`). Развёрнутый ряд
/// прижимался к ЛЕВОМУ краю: однокартинная строка `icb-*` уезжала на 530
/// влево (ровно 1.16 — текст подписи в двух местах), квадрат эталона
/// `vrl-006-ref` — за левый край окна (3.55 → 2.49). Разворот ниже прижимает
/// `FlexStart`/`FlexEnd`. Разбор: `target/scout-abspos-vert-2026-09-30.md` §2.
/// Запасная ветка: гибкая строка из отдельных СЛОВ.
///
/// Сюда абзац попадает, когда единым текстовым блоком его не собрать: разные
/// кегли в строке или не-текстовый кусок посреди неё. Кусок целиком гибкая
/// строка перенести не может — он неделим, и раскладка ужимает его до самого
/// узкого слова: текст вставал столбиком и налезал на следующий абзац.
/// Поэтому куски режутся на слова: перенос идёт по ним, как в строке.
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
    // меряется отдельным корнем, и его «родитель» — не блок строки:
    // замерено, `horizontal-rule-vlr-003` 0.08 → 1.22.
    pct_from_parent: bool,
) -> AnyElement {
    use crate::style::computed::{Align, TextAlign};
    // `vertical-align` в строке: умолчание — базовая линия, но `middle`,
    // `top` и `bottom` встречаются и раньше не доезжали никуда, кроме
    // ячейки таблицы.
    // ПРОБОВАЛИ И ОТКАТИЛИ: минимум высоты ряда в струт строки (§10.8), чтобы
    // ряд из одних картинок не садился на их высоту. Замерено: приобретено 3,
    // потеряно 37 — ряд переносится, и минимум ложится на ВСЮ пачку строк, а
    // не на каждую. Возвращать вместе с настоящей строчной раскладкой атомов.
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
    // разрыв и куски вне потока (`Overlay`, щуп статической позиции) — тоже:
    // первый переставил бы сами строки, второй сдвинул бы щуп, чьи rtl-рукава
    // в `LatePlace` настроены на прежний порядок.
    let reversed = rtl
        && !hard_break
        && pieces.iter().any(|p| matches!(p, Piece::Atom(_)))
        && pieces.iter().all(|p| match p {
            Piece::Atom(_) => true,
            Piece::Overlay(..) => false,
            Piece::Text { text, .. } => text.chars().all(|c| c.is_whitespace() || c == '\u{200b}'),
        });
    let mut row = gpui::div().flex().max_w_full();
    // Ряд строки — не коробка CSS: доли высоты атомов считаются от блока,
    // которому принадлежит строка (CSS 2.1 §10.1 п.2; `vendor/taffy`
    // `percent_basis_from_parent`). Доля доходит сюда, только если блок
    // определён (`cb_height_def`, гейт `apply.rs`), — иначе она уже `auto`.
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
    // физического начала (`taffy/src/compute/common/alignment.rs:50-66`), и
    // `End` кладёт развёрнутую строку к правому краю ровно так же, как
    // `FlexStart` (первым идёт физически левый ребёнок, `flexbox.rs:2499`,
    // смещение `free_space` у обоих). Откат K1 (08.09) падал на ЗЕРКАЛЕ
    // (`Right` → `justify_start()`), а не на физическом прижиме.
    // `FlexStart`/`FlexEnd` здесь нельзя: абсолют-ребёнок ряда (держатель
    // замещаемого с долей ширины, `atom_element`) ставится taffy по
    // `justify_content` БЕЗ учёта разворота (`flexbox.rs:2782-2799`: пара
    // `(FlexStart, false)` → начало), и rtl-статика уезжала к ЛЕВОМУ краю
    // (`absolute-replaced-width-020`: 3.84 — синий 96×96 слева). Blink
    // разворот учитывает (`flex_layout_algorithm.cc:457-478`
    // `MainAxisStaticPositionEdge`), css-flexbox-1 §4.1 — «as if it were the
    // sole flex item». Без значения taffy сам берёт `FlexStart`
    // (`flexbox.rs:1923`), то есть правый край — начало rtl-строки.
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
    // (css2/Overview.bs:6776) даёт strut и прочим строкам `<br>`, но тогда его
    // обязана получить и строка из одного атома (Blink ставит
    // `should_create_line_box` обоим: line_breaker.cc:2906 и :3158), а у ряда
    // атомов strut откачен (+3/−37, см. выше). Пустой `<br>` со strut при
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
                    Piece::Overlay(_, how) if how.cb_marker.is_some() => glued,
                    Piece::Overlay(el, _) => glued.child(overlay_in_row(el)),
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
                Piece::Overlay(_, how) if how.cb_marker.is_some() => row,
                Piece::Overlay(el, _) => row.child(overlay_in_row(el)),
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
