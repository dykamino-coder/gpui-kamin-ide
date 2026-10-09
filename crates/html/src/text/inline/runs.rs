//! Текст и прогоны абзаца, строка из элементов, склейка атомов.

use crate::style::computed::Computed;
use crate::style::values::value::{Color, Len};
use crate::text::inline::*;
use gpui::{AnyElement, FontStyle, FontWeight, HighlightStyle, ParentElement, Styled, TextRun, TextStyle, UnderlineStyle};

/// Можно ли собрать всё в один текстовый блок: одинаковый размер шрифта и ни
/// одного не-текстового куска.
/// ПРОБОВАЛИ И ОТКАТИЛИ: пускать абзац с атомом нормальным путём строк.
/// Собран весь первый шаг разбора (`target/scout-atomstep1.md`): `Piece::Atom`
/// со своей коробкой полей, кусок-НОСИТЕЛЬ `U+FEFF` с трекингом рядом с ним,
/// `atom_spans`/`atom_marks`, рост строки в `line_padding`, подмена носителя
/// на `U+FFFC` в `linebreaks` (css-text-3 §5.1), накопленный шаг строк и
/// посадка атома нижним краем на базовую линию в `point_of`, узкие ворота
/// `atoms_ready`. Замерено по обоим сводам: CSS2 5234 -> 5210, CSS3
/// 2352 -> 2342; приобретено 3, потеряно 38.
///
/// Что вскрылось: (1) базовая линия `inline-block` — нижний край коробки полей
/// ТОЛЬКО при `overflow` не `visible`, иначе это базовая линия последней
/// строки содержимого (§10.8.1), а его высота на сборке неизвестна;
/// (2) доли подъёма и спуска струта 0.8/0.2 верны для Ahem, но не для
/// шрифтов корпуса; (3) накопленный шаг в `point_of` сам по себе роняет всё,
/// что кладётся слоем поверх строки (`text-combine-upright-*`,
/// `position-relative-table-*-absolute-child`) — эту часть надо мерить
/// отдельно и чинить вместе с `paint`.
pub fn single_block(pieces: &[Piece], _base_size: f32) -> bool {
    // Кегль куску больше не мешает: он едет в прогон (патч GPUI). Мешает
    // только не-текстовый кусок — картинку в прогон не положить. Кусок ВНЕ
    // потока не мешает: место в строке он не занимает.
    pieces
        .iter()
        .all(|p| matches!(p, Piece::Text { .. } | Piece::Overlay(..)))
}

/// Самый крупный кегль среди кусков — по нему считается высота строки.
/// `strut` — кегль самого абзаца: строка не бывает ниже его. `em_base` —
/// от чего считается доля `em` у куска: это кегль РОДИТЕЛЯ абзаца, а не его
/// собственный, иначе `font-size: 4em` на абзаце и на его куске умножились бы
/// дважды (`text-transform-shaping-001`: строка вышла в 256 точек вместо 64).
pub fn max_font_size(pieces: &[Piece], strut: f32, em_base: f32) -> f32 {
    pieces.iter().fold(strut, |acc, p| match p {
        Piece::Text { style, .. } => match style.font_size {
            Some(Len::Px(v)) => acc.max(v),
            Some(Len::Em(k)) => acc.max(k * em_base),
            _ => acc,
        },
        Piece::Atom(_) | Piece::Overlay(..) => acc,
    })
}

/// Высота строки абзаца: максимум по кускам, но с УЧЁТОМ объявленной на
/// куске `line-height`.
///
/// Прежде высота считалась как «самый крупный кегль × доля normal», и
/// объявленная на куске `line-height` доходила только каналом `lh_spans`,
/// который умеет строку растить и не умеет сжимать. У `font: 100px/1` строка
/// выходила 132 вместо 100, а глиф садился по полулидингу на 16 точек ниже
/// (§10.8: лидинг тут ноль — содержимое равно `line-height`).
pub fn max_line_height(pieces: &[Piece], strut: f32, em_base: f32, fraction: f32) -> f32 {
    pieces.iter().fold(strut * fraction, |acc, p| match p {
        Piece::Text { style, .. } => {
            let size = match style.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * em_base,
                _ => strut,
            };
            let own = match style.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
                _ => size * fraction,
            };
            acc.max(own)
        }
        Piece::Atom(_) | Piece::Overlay(..) => acc,
    })
}

/// Один `StyledText` с прогонами — честный перенос по словам сквозь границы
/// `<b>`/`<a>`/`<span>`.
/// Текст и прогоны абзаца — то же, что уходит в `StyledText`.
///
/// Отдаются отдельно, потому что выделение строит свой элемент из тех же
/// прогонов, добавляя подложку на выделенный кусок.
pub fn text_and_runs(pieces: &[Piece], base: &TextStyle) -> Option<(String, Vec<TextRun>)> {
    let mut text = String::new();
    let mut runs: Vec<TextRun> = vec![];
    for p in pieces {
        match p {
            Piece::Text { text: t, style } => {
                if t.is_empty() {
                    continue;
                }
                text.push_str(t);
                runs.push(run_for(t, style, base));
            }
            // Кусок вне потока в текст не входит: его место помечает нулевой
            // пробел, а сам он рисуется поверх (см. `overlays`).
            Piece::Overlay(..) => {}
            Piece::Atom(_) => return None,
        }
    }
    (!text.is_empty()).then_some((text, runs))
}

/// Шрифт струта абзаца — тот же, каким набирался бы текст самого блока.
pub fn strut_font(style: &Computed, base: &TextStyle) -> gpui::Font {
    run_for("x", style, base).font
}

pub(crate) fn run_for(text: &str, style: &Computed, base: &TextStyle) -> TextRun {
    let mut font = base.font();
    font.fallbacks = crate::style::computed::font_family::fallbacks(style, font.fallbacks);
    // Названное семейство сильнее родового: подстановкой занимается система.
    // Пустое имя — «шрифт документа» (разбор `font-family`): база как есть.
    if let Some(family) = style.font_family.as_ref().filter(|f| !f.is_empty()) {
        // Имя из разметки может быть придуманным (`@font-face`) — система
        // шрифтов знает файл под его собственным именем. Лиц у имени бывает
        // несколько, и нужное выбирает ширина начертания (§font-matching).
        font.family = crate::text::fonts::alias_stretch(family, style.font_stretch)
            .unwrap_or_else(|| family.clone())
            .into();
    } else if style.monospace == Some(true) {
        font.family = crate::text::metrics::mono_family_for(style.lang.as_deref()).into();
    }
    // Вес и курсив — ВСЕГДА от стиля куска: `None` в слитом стиле — это
    // обычное начертание, а не «как у базы» (база абзаца строится по
    // первому куску, и `abc<b>def</b>ghi` набирался одним прогоном его
    // веса — сквозной дефект по всему корпусу).
    font.weight = FontWeight(style.font_weight.unwrap_or(400) as f32);
    font.style = if style.italic == Some(true) {
        FontStyle::Italic
    } else {
        FontStyle::Normal
    };
    if let Some(pct) = style.font_stretch {
        font.stretch = gpui::FontStretch::from_percent(pct);
    }
    let features = style.used_features();
    if !features.is_empty() {
        font.features = gpui::FontFeatures(std::sync::Arc::new(features));
    }
    // `visibility: hidden` на самом куске: место в строке он держит, а чернил
    // не даёт (§11.2). Прозрачный цвет, а не пропуск куска, — иначе поехали бы
    // ширины и переносы.
    let color = if style.hidden == Some(true) {
        gpui::hsla(0.0, 0.0, 0.0, 0.0)
    } else {
        style.color.map(Color::to_hsla).unwrap_or(base.color)
    };
    // Кегль куска: без него разный размер в строке собрать в один блок было
    // нельзя (см. `single_block`).
    let base_px = f32::from(base.font_size.to_pixels(gpui::px(16.)));
    let font_size = match style.font_size {
        Some(Len::Px(v)) => Some(gpui::px(v)),
        Some(Len::Em(k)) => Some(gpui::px(k * base_px)),
        _ => None,
    };
    TextRun {
        len: text.len(),
        font,
        font_size,
        color,
        // A fully transparent band colour only identifies its inline box (see
        // `BORDER_BAND`): it is kept black (lightness 0) so nothing of it can
        // blend into the border edge, with the id in hue and saturation.
        background_color: style.inline_bg.map(|c| {
            if c.a == 0.0 {
                gpui::Hsla {
                    h: c.r,
                    s: c.g,
                    l: 0.0,
                    a: 0.0,
                }
            } else {
                c.to_hsla()
            }
        }),
        background_border: style
            .inline_border
            .map(|(c, w)| (c.to_hsla(), w.map(gpui::px))),
        background_pad: style.inline_pad.unwrap_or_default().map(gpui::px),
        background_radius: gpui::px(style.inline_radius.unwrap_or(0.0)),
        underline: style.underline.unwrap_or(false).then(|| UnderlineStyle {
            thickness: gpui::px(1.),
            color: Some(color),
            wavy: false,
        }),
        strikethrough: style
            .line_through
            .unwrap_or(false)
            .then(|| gpui::StrikethroughStyle {
                thickness: gpui::px(1.),
                color: Some(color),
            }),
    }
}

/// Подсветка для куска текста — используется, когда прогоны накладываются на
/// готовый текст (например, при подсветке кода).
pub fn highlight_for(style: &Computed) -> HighlightStyle {
    HighlightStyle {
        color: style.color.map(Color::to_hsla),
        font_weight: style.font_weight.map(|w| FontWeight(w as f32)),
        font_style: style.italic.and_then(|i| i.then_some(FontStyle::Italic)),
        ..Default::default()
    }
}

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
            Piece::Text { text, .. } => text
                .chars()
                .all(|c| c.is_whitespace() || c == '\u{200b}'),
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

/// Знак классов GL/WJ/ZWJ (UAX #14), который НЕЛЬЗЯ отделять от соседней
/// атомарной строчной коробки.
///
/// css-text-3 §5.1, пункт «atomic-compat-wrap», дословно: «with the exception
/// of U+00A0 NO-BREAK SPACE, there must be no soft wrap opportunity between
/// atomic inlines and adjacent characters belonging to the Unicode GL, WJ, or
/// ZWJ line breaking classes». Неразрывный пробел из правила ИСКЛЮЧЁН: рядом
/// с атомом он точку переноса, наоборот, ДАЁТ — `line-breaking-atomic-001`
/// и `-002` на этом и построены.
pub(crate) fn atom_glue(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    ch != '\u{00A0}'
        && matches!(
            unicode_linebreak::break_property(ch as u32),
            NonBreakingGlue | WordJoiner | ZeroWidthJoiner
        )
}

/// Служебный кусок: метка границы атома (`U+200B`, ставится в `collect`) и
/// распорка полей строчной коробки (`SPACER`). Ширины у них нет, и поиску
/// настоящего соседа они мешать не должны: между текстом и атомом метка
/// стоит ВСЕГДА.
///
/// Сравнение идёт с куском ЦЕЛИКОМ: `U+FEFF` как знак документа
/// (`line-breaking-atomic-012/013`) приезжает внутри текста вместе с буквой и
/// служебным не считается.
pub(crate) fn glue_marker(p: &Piece) -> bool {
    matches!(p, Piece::Text { text, .. } if text == "\u{200b}" || text == SPACER || text.is_empty())
}

/// Куски ряда, сгруппированные по правилу склейки с атомом (css-text-3 §5.1).
///
/// Клеится только ПРИЛЕГАЮЩЕЕ слово: точка переноса по пробелу перед ним
/// обязана остаться, иначе весь кусок текста стал бы неразрывным.
/// Две соседние коробки БЕЗ знака-склейки не склеиваются: между ними точка
/// переноса есть (`line-breaking-atomic-007`).
pub(crate) fn glue_atoms(pieces: Vec<Piece>) -> Vec<Vec<Piece>> {
    // Шаг 1: у каких текстовых кусков сосед — атом (сквозь служебные метки).
    let mut before_atom = vec![false; pieces.len()];
    let mut after_atom = vec![false; pieces.len()];
    {
        let real = |from: usize, back: bool| -> Option<usize> {
            let mut k = from;
            loop {
                k = if back { k.checked_sub(1)? } else { k + 1 };
                if k >= pieces.len() {
                    return None;
                }
                if !glue_marker(&pieces[k]) {
                    return Some(k);
                }
            }
        };
        for i in 0..pieces.len() {
            if !matches!(pieces[i], Piece::Atom(_)) {
                continue;
            }
            if let Some(j) = real(i, true) {
                before_atom[j] = true;
            }
            if let Some(j) = real(i, false) {
                after_atom[j] = true;
            }
        }
    }
    // Шаг 2: отрезать прилегающее слово в свой кусок и пометить его.
    let mut cut: Vec<(Piece, bool)> = Vec::with_capacity(pieces.len() + 2);
    for (i, p) in pieces.into_iter().enumerate() {
        match p {
            Piece::Text { text, style }
                if before_atom[i] && text.chars().next_back().is_some_and(atom_glue) =>
            {
                let at = text.rfind(' ').map(|k| k + 1).unwrap_or(0);
                if at > 0 {
                    cut.push((
                        Piece::Text {
                            text: text[..at].to_string(),
                            style: style.clone(),
                        },
                        false,
                    ));
                }
                cut.push((
                    Piece::Text {
                        text: text[at..].to_string(),
                        style,
                    },
                    true,
                ));
            }
            Piece::Text { text, style }
                if after_atom[i] && text.chars().next().is_some_and(atom_glue) =>
            {
                let at = text.find(' ').map(|k| k + 1).unwrap_or(text.len());
                cut.push((
                    Piece::Text {
                        text: text[..at].to_string(),
                        style: style.clone(),
                    },
                    true,
                ));
                if at < text.len() {
                    cut.push((
                        Piece::Text {
                            text: text[at..].to_string(),
                            style,
                        },
                        false,
                    ));
                }
            }
            other => cut.push((other, false)),
        }
    }
    // Шаг 3: где шва между детьми ряда быть НЕ должно. 0 — обычный кусок,
    // 1 — служебная метка, 2 — атом, 3 — приклеенное слово.
    let kind: Vec<u8> = cut
        .iter()
        .map(|(p, glued)| match p {
            Piece::Atom(_) => 2,
            _ if *glued => 3,
            p if glue_marker(p) => 1,
            _ => 0,
        })
        .collect();
    let mut bind = vec![false; kind.len()];
    for i in 0..kind.len() {
        if kind[i] != 2 {
            continue;
        }
        // Влево: сквозь служебные метки — и дальше ТОЛЬКО если там стоит
        // приклеенное слово. Цепочка из одних меток связывать не должна:
        // иначе две соседние коробки слиплись бы навсегда.
        let mut k = i;
        while k > 0 && kind[k - 1] == 1 {
            k -= 1;
        }
        if k > 0 && kind[k - 1] == 3 {
            for j in k..=i {
                bind[j] = true;
            }
            // Слово, к которому приклеен атом, часто лежит в НЕСКОЛЬКИХ
            // кусках: `<a>A</a>&#x2011;<span>B</span>` даёт куски `A` и
            // `\u{2011}`, приклеивался только второй, шов между ними
            // оставался, и ряд рвал слово пополам — коробка уезжала на
            // вторую строку (`line-breaking-atomic-020/022/024/026`).
            // Слово продолжается влево, пока слева стоит ТЕКСТ, не
            // кончающийся пробелом: по UAX-14 LB12a перед классом GL
            // переносить нельзя, кроме как после пробела, а точка переноса
            // по пробелу обязана остаться.
            let mut g = k - 1;
            loop {
                let mut m = g;
                while m > 0 && kind[m - 1] == 1 {
                    m -= 1;
                }
                if m == 0 {
                    break;
                }
                let Piece::Text { text, .. } = &cut[m - 1].0 else {
                    break;
                };
                if text.ends_with(' ') {
                    break;
                }
                for j in m..=g {
                    bind[j] = true;
                }
                g = m - 1;
            }
        }
        // Вправо тем же порядком.
        let mut k = i;
        while k + 1 < kind.len() && kind[k + 1] == 1 {
            k += 1;
        }
        if k + 1 < kind.len() && kind[k + 1] == 3 {
            for j in (i + 1)..=(k + 1) {
                bind[j] = true;
            }
        }
    }
    let mut out: Vec<Vec<Piece>> = Vec::with_capacity(cut.len());
    for (i, (p, _)) in cut.into_iter().enumerate() {
        if bind[i] && !out.is_empty() {
            out.last_mut().expect("группа уже открыта").push(p);
        } else {
            out.push(vec![p]);
        }
    }
    out
}
