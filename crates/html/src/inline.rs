//! Инлайн-поток: текст с вкраплениями `<b>`, `<a>`, `<code>` внутри абзаца.
//!
//! Здесь лежит главное расхождение GPUI с HTML. В браузере абзац — это поток
//! строк, куда встраиваются куски любого размера, и строка растёт под самый
//! высокий из них. В GPUI текстовый блок — лист раскладки: размер шрифта у него
//! ОДИН на весь блок (`shape_text` принимает `font_size` скаляром), и метрики
//! строки тоже одни.
//!
//! Отсюда две стратегии, и выбор между ними делается по факту содержимого:
//!
//! * **Один блок текста.** Если все куски абзаца одного размера и ни один не
//!   несёт собственного бокса (фон, рамка, отступы), они собираются в один
//!   `StyledText` с прогонами. Перенос строк тогда честный — как в браузере,
//!   по словам, сквозь границы `<b>` и `<a>`.
//! * **Строка из элементов.** Иначе куски становятся отдельными элементами в
//!   гибкой строке с переносом. Перенос идёт по кускам, а не по словам внутри
//!   них — это заметно на длинном `<code>`, но зато размеры и боксы честные.
//!
//! Первая ветка покрывает подавляющее большинство: жирный, курсив, ссылка,
//! цвет. Вторая включается там, где без неё пришлось бы врать про размер.

mod first_letter;
mod first_line_background;
mod empty_inline;
pub use first_letter::split_first_letter;

mod tabs;
pub(crate) mod physical_sides;
mod physical_projection;
mod inline_spacing;
mod bidi_controls;
pub use bidi_controls::bidi_marks;
pub use tabs::tab_stops;

use crate::computed::{Computed, TextAlign, TextTransform};
use crate::dom::{Element, Node};
use crate::value::{Color, Len};
use gpui::{
    AnyElement, FontStyle, FontWeight, HighlightStyle, IntoElement, ParentElement, Styled, TextRun,
    TextStyle, UnderlineStyle,
};

/// Кусок инлайн-содержимого: либо текст со своим стилем, либо готовый элемент
/// (картинка, кнопка — то, что текстом не является).
pub enum Piece {
    Text {
        text: String,
        style: Computed,
    },
    Atom(AnyElement),
    /// Элемент ВНЕ потока строки: места не занимает, но рисуется там, где
    /// стоит в тексте (абсолютный элемент на статической позиции). В отличие
    /// от `Atom` не выводит абзац из текстового пути — иначе строка теряет
    /// пробелы и перенос по словам (`line-breaking-018`).
    ///
    /// Второе поле — БЛОЧНЫЙ уровень гипотетической коробки: её статическая
    /// позиция — начало СЛЕДУЮЩЕЙ строки, а не точка в текущей (CSS 2.1
    /// §10.6.4/§10.3.7 «if position had been static»; Blink
    /// `LogicalStaticPosition` блочного OOF в строчном контексте — блок-
    /// начало после текущей строки, строчное начало — край содержимого).
    Overlay(AnyElement, OverlayAt),
}

/// Как кусок вне потока садится на своё место в тексте (`lines.rs`).
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct OverlayAt {
    /// Гипотетическая коробка БЛОЧНАЯ: место — начало следующей строки.
    pub next_line: bool,
    /// Относительный сдвиг строчных предков по ДО-ПОВОРОТНОЙ оси y в
    /// повёрнутом абзаце. Прикладывается в `lines.rs` вместе с округлением
    /// до целой физической точки ВНИЗ — так же, как глиф соседнего текста
    /// (`window.rs: paint_glyph` берёт `floor`, раскладка — `round`).
    pub rot_dy: f32,
    /// То же по до-поворотной x (строчная ось). Отбивкой его не задать:
    /// при `direction: rtl` `inset-inline-start` даёт ОТРИЦАТЕЛЬНЫЙ сдвиг, а
    /// отрицательная отбивка обнуляется — коробка оставалась на месте, текст
    /// уезжал на 2px, и полоса красного проступала (`static-position/
    /// v{lr,rl}-rtl-*`, `cb`-случаи).
    pub rot_dx: f32,
    /// Строчный абсолют на статической точке повёрнутого rtl-абзаца: сторону,
    /// которой коробка висит на точке, решает УРОВЕНЬ bidi в точке
    /// (`lines.rs: rtl_level_at`), а не направление блока. Гипотетическая
    /// коробка стоит В ПРОГОНЕ: в ltr-прогоне (латиница, Ahem) она уходит от
    /// точки вправо, и строчное начало rtl-блока — её правый край — лежит на
    /// ширину правее точки (CSS 2.1 §10.3.7 «set 'right' to the static
    /// position»: статическая позиция — край гипотетической коробки).
    pub bidi_hang: bool,
}

/// Схлопывание пробелов ЧЕРЕЗ границу кусков (CSS 2.1 §16.6.1,
/// css-text-3 §4.1.1).
///
/// Схлопывание у нас поузловое, а ряд пробелов сплошь и рядом лежит в РАЗНЫХ
/// кусках: спека прямо оговаривает пробел «even one outside the boundary of
/// the inline containing that space». `<span>Row 1, </span>` плюс перевод
/// строки перед следующим тегом набирались как два пробела — на знак шире
/// эталона, и так на каждой границе.
///
/// Кусок из ОДНИХ пробелов ряд не разрывает: он схлопывается в предыдущий
/// пробел целиком и остаётся пустым.
///
/// ПРОБОВАЛИ И ОТКАТИЛИ: пропускать здесь и СЛУЖЕБНЫЕ куски — распорку полей
/// строчной коробки (U+FEFF) и метку границы атома (U+200B), — не сбрасывая
/// признак пробела. Замерено по семьям text/*, linebox/*, bidi-*: 0 и 0.
/// Семью `white-space-normal-*` держит не это.
pub fn collapse_across_pieces(pieces: &mut [Piece]) {
    let mut prev_space = false;
    for piece in pieces.iter_mut() {
        match piece {
            // Кусок вне потока места не занимает и ряд пробелов не рвёт.
            Piece::Overlay(..) => {}
            // Замещаемая коробка — не пробел: ряд на ней кончается.
            Piece::Atom(_) => prev_space = false,
            Piece::Text { text, style } => {
                // A zero-advance empty-box metric marker is not document
                // content and must not interrupt adjoining collapsible spaces.
                if text == SPACER && style.letter_spacing == Some(Len::Px(0.0)) {
                    continue;
                }
                // `white-space: pre*` пробелы бережёт — там схлопывать нечего.
                // `pre-line` переводы строк бережёт, а ПРОБЕЛЫ схлопывает
                // (§16.6): освобождать его от схлопки нельзя.
                if style.keep_spaces == Some(true) {
                    prev_space = false;
                    continue;
                }
                // Кусок-метка направления (`bidi_marks`: RLE/LRE/PDF вокруг
                // `<span dir>`) ряд пробелов НЕ рвёт и сам его не начинает —
                // css-text-3 §4.1 велит обрабатывать пробелы, не видя этих
                // знаков. Иначе `x <span dir=rtl> x </span> x` набирался
                // семью знаками вместо пяти (`white-space-collapsing-bidi-002`).
                if !text.is_empty() && text.chars().all(bidi_format) {
                    continue;
                }
                if prev_space {
                    let rest = text.trim_start_matches(' ');
                    if rest.len() != text.len() {
                        *text = rest.to_string();
                    }
                }
                match text.chars().next_back() {
                    Some(' ') => prev_space = true,
                    // Пустой кусок ряда не меняет: он и есть схлопнутый пробел.
                    None => {}
                    Some(_) => prev_space = false,
                }
            }
        }
    }
}

/// Собрать инлайн-куски из детей узла.
pub fn collect(
    children: &[Node],
    inherited: &Computed,
    atom: &mut dyn FnMut(&Element) -> Option<Piece>,
) -> Vec<Piece> {
    collect_with_empty_metrics(
        children,
        inherited,
        atom,
        empty_inline::has_text(children),
    )
}

fn collect_with_empty_metrics(
    children: &[Node],
    inherited: &Computed,
    atom: &mut dyn FnMut(&Element) -> Option<Piece>,
    has_text: bool,
) -> Vec<Piece> {
    let mut out = vec![];
    // Место последней распорки зазора за коробкой (см. ниже).
    let mut gap_at: Option<usize> = None;
    for child in children {
        // Куски ЭТОГО ребёнка: по ним ставится зазор на границе с тем, что
        // идёт следом (см. `set_boundary_spacing` ниже).
        let from = out.len();
        match child {
            Node::Text(t) => {
                // Возврат каретки (U+000D) — ПРОБЕЛ при любом `white-space`
                // (css-text-3 §4.1: «carriage returns … are treated
                // identically to spaces»): разбор HTML сводит CR к LF только в
                // разметке, а `&#x0D;` доезжает знаком и при `pre*` рвал
                // строку (`control-chars-00D`).
                let cr_free;
                let t: &str = if t.contains('\r') {
                    cr_free = t.replace('\r', " ");
                    &cr_free
                } else {
                    t
                };
                // `white-space: pre*` сохраняет пробелы как есть: отступы кода
                // иначе схлопывались в один пробел и текст терял форму.
                let raw = if inherited.preserve_newlines == Some(true)
                    && inherited.keep_spaces != Some(false)
                {
                    // Табуляция остаётся СВОИМ знаком: её ширину считает
                    // раскладка строк по позициям табуляции. Разворот в
                    // пробелы менял и число точек переноса, и вид хвоста
                    // строки (`break-spaces-tab`).
                    t.to_string()
                } else if inherited.preserve_newlines == Some(true) {
                    // `pre-line`: пробелы схлопываются, переводы строк живут.
                    t.split('\n')
                        .map(normalize_spaces)
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    normalize_spaces(t)
                };
                // Замена нулевого пробеля идеографическим идёт ПО КУСКАМ
                // абзаца (`space_transform_pieces`): соседи точки переноса
                // сплошь и рядом лежат в других кусках, и проход по одному
                // узлу их не видит.
                let mut text = breakable(&transform_case(&raw, inherited), inherited);
                // То же правило нулевого пробела СКВОЗЬ границу строчной
                // коробки (css-text-4 §4.1.3; границ коробок для него нет —
                // `seg-break-transformation-018`): перевод строки в начале
                // узла, а нулевой пробел — последний знак предыдущего куска.
                if inherited.preserve_newlines != Some(true)
                    && leading_segment_break(t)
                    && ends_with_zwsp(&out)
                    && text.starts_with(' ')
                {
                    text.remove(0);
                }
                if !text.is_empty() {
                    out.push(Piece::Text {
                        text,
                        style: inherited.clone(),
                    });
                }
            }
            Node::Element(e) => {
                if e.tag == "br" {
                    out.push(Piece::Text {
                        text: "\n".into(),
                        style: inherited.clone(),
                    });
                    continue;
                }
                // `<wbr>` — точка переноса без знака, то есть ровно нулевой
                // пробел (HTML §4.5.28). Раньше тег не давал НИЧЕГО, и
                // разрешённого переноса в этом месте не было.
                if e.tag == "wbr" {
                    // Замена точки переноса пробелом — по стилю САМОГО `<wbr>`
                    // (css-text-4 §word-space-transform: свойство наследуемое,
                    // и у элемента своё значение).
                    let mut style = inherited.clone();
                    style.word_space_char = e.style.word_space_char.or(style.word_space_char);
                    out.push(Piece::Text {
                        text: "\u{200b}".into(),
                        style,
                    });
                    continue;
                }
                // `display: contents` — коробки нет (css-display-3 §2.5 «does
                // not generate any boxes, but its children … still generate
                // boxes and text runs as normal»): ни рамки, ни полей, ни
                // отступов, ни знаков направления — детям уходят только
                // текстовые свойства наследованием. Блочный путь это знает
                // (`render.rs:4358`), а строчный сбор вёл такой элемент обычным
                // `<span>`, и рамка рисовалась прогоном
                // (`display-contents-inline-001` «красное видно»).
                if e.style.display == Some(crate::computed::Display::Contents) {
                    let merged = inherit(inherited, &e.style);
                    out.extend(collect_with_empty_metrics(
                        &e.children, &merged, atom, has_text,
                    ));
                    continue;
                }
                if let Some(piece) = atom(e) {
                    out.push(piece);
                    // Строчный `<span>`, ушедший в свою коробку (узорный фон,
                    // `has_own_box`), атомом в CSS не является: зазор между его
                    // последним знаком и следующим — ПРЕДКА (css-text-3 §8.2),
                    // а внутренний абзац коробки последний знак не трекует.
                    // Без распорки следующее слово вставало вплотную
                    // (`letter-spacing-nesting-003`). Хвостовая распорка у
                    // конца узла снимается ниже — за ней границы нет.
                    if let Some(gap) = boundary_gap_after_box(e, inherited) {
                        gap_at = Some(out.len());
                        out.push(Piece::Text {
                            text: SPACER.into(),
                            style: spacer_style(inherited, gap),
                        });
                    }
                    continue;
                }
                let mut merged = inherit(inherited, &e.style);
                // Относительный сдвиг строчного КОПИТСЯ вниз: вложенные
                // куски двигаются на сумму сдвигов предков.
                let own_rel = relative_inset(&e.style);
                if own_rel != (0.0, 0.0) {
                    let base = merged.rel_shift.unwrap_or((0.0, 0.0));
                    merged.rel_shift = Some((base.0 + own_rel.0, base.1 + own_rel.1));
                }
                // Фон строчного бокса рисует прогон текста: коробки у него
                // нет, а фон обязан рваться на переносах вместе со строкой.
                // Берётся из СЛИТОГО стиля: отложенный цвет (`currentColor`,
                // относительная функция) решён только там.
                // `background-clip: text`: бокс фоном не красится — заливку
                // несёт цвет глифов (`inherit`), узорную показать нечем.
                if let Some(bg) = merged
                    .background
                    .filter(|_| merged.bg_clip != Some(crate::computed::BgClip::Text))
                {
                    merged.inline_bg = Some(bg);
                    // Единицы шрифта разрешаются так же, как в `inline_sides`:
                    // разбор только по точкам ронял `padding: 1em` в ноль, и
                    // подсветка строчной коробки шла впритык к тексту.
                    // Замерено по строчным семьям: приобретено 0, потеряно 0 —
                    // правка держится на своей правоте, а не на счёте.
                    //
                    // ПРОБОВАЛИ И ОТКАТИЛИ вместе с ней: раздувать `line_bounds`
                    // в `vendor/gpui/.../line.rs` на перелив полосы прогона,
                    // чтобы фон строки не попадал в один порядок с глифами
                    // соседней. Замерено там же: 0 и 0.
                    let size = match merged.font_size {
                        Some(crate::value::Len::Px(v)) => v,
                        _ => 16.0,
                    };
                    let family = merged.font_family.clone().unwrap_or_default();
                    let px_of = |l: Option<crate::value::Len>| match l {
                        Some(
                            crate::value::Len::Px(_)
                            | crate::value::Len::Em(_)
                            | crate::value::Len::Ch(_)
                            | crate::value::Len::Ex(_),
                        ) => crate::metrics::spacing_px(l, &family, size),
                        _ => 0.0,
                    };
                    merged.inline_pad = Some(physical_sides::project(inherited, [
                        px_of(e.style.padding.top),
                        px_of(e.style.padding.right),
                        px_of(e.style.padding.bottom),
                        px_of(e.style.padding.left),
                    ]));
                    merged.inline_radius = Some(px_of(e.style.radius.tl));
                }
                // Рамка строчной коробки рисуется прогоном: и ровная, и
                // с РАЗНЫМИ гранями (у прогона теперь пооосевые ширины) —
                // коробка рвала перенос, и span с рамкой уезжал столбиком.
                let font_px = match merged.font_size {
                    Some(Len::Px(v)) => v,
                    _ => 16.0,
                };
                if let Some((color, width)) = uniform_border(&e.style, font_px) {
                    merged.inline_border = Some((color, [width; 4]));
                } else if let Some(sided) = sided_border(&e.style, font_px) {
                    merged.inline_border = Some((sided.0, physical_sides::project(inherited, sided.1)));
                }
                // Контур строчного куска рисует тот же прогон: коробки у
                // куска нет, а место контур и не занимает. Рисуется только
                // ЗАДАННЫЙ видимый стиль (начальное `outline-style` — `none`),
                // толщина без записи — `medium`, цвет без своего — цвет текста
                // куска С НАСЛЕДОВАНИЕМ (`merged`, а не свой `e.style`), без
                // него — чёрный. Полосу прогона `paint_line_background`
                // заводит ТОЛЬКО от фона (`line.rs`, `current_background`),
                // поэтому контуру без фона даётся прозрачная подложка: иначе
                // кольцо не рисовалось вовсе (`outline-004` «красное видно»,
                // `outline-022` 0.83 = 100²−80²). ★ Не путать с откатом K2
                // (07.09, `line.rs`): там полоса заводилась от ЛЮБОЙ рамки, и
                // обычная рамка рисовалась дважды (`bidi-00*`); здесь — только
                // контур, второго рисовальщика у которого нет.
                if merged.inline_border.is_none()
                    && e.style.display.is_none()
                    && let Some(o) = e.style.outline
                    && o.style.is_some_and(|s| s != 0)
                    && let Some(width) = match o.width {
                        Some(Len::Px(w)) => Some(w),
                        None => Some(3.0),
                        _ => None,
                    }
                    && width > 0.0
                {
                    let color = o.color.or(merged.color).unwrap_or(Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    });
                    merged.inline_border = Some((color, [width; 4]));
                    if merged.inline_bg.is_none() {
                        merged.inline_bg = Some(Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        });
                    }
                }
                // Атомарная строчная коробка — ГРАНИЦА переноса, даже когда
                // своей коробки в раскладке ей не завели (размер не задан, и
                // она осталась куском текста). По CSS строку можно рвать
                // между текстом и такой коробкой; у нас это выражается
                // нулевым пробелом по краям (`line-breaking-atomic-007`).
                // Настоящий `display: inline` сюда НЕ входит: разбор держит
                // его как `InlineBlock` с пометкой `inline_display`, и без
                // отсечки каждый `<span>` обрамлялся служебными нулевыми
                // пробелами — они рвали ряд схлопываемых пробелов, возвращали
                // краевой проход и добавляли лишнюю точку переноса. Та же
                // отсечка стоит в `has_own_box`.
                let atomic = matches!(
                    e.style.display,
                    Some(crate::computed::Display::InlineBlock)
                        | Some(crate::computed::Display::InlineFlex)
                        | Some(crate::computed::Display::InlineGrid)
                        | Some(crate::computed::Display::InlineTable)
                ) && e.style.inline_display != Some(true);
                // Ограничитель атомарной коробки — служебный знак, а не текст
                // документа: замена нулевого пробела идеографическим его
                // касаться не должна, иначе вокруг `inline-block` появляется
                // полноширинный пробел, которого в разметке нет.
                let mut marker = merged.clone();
                marker.word_space_char = None;
                if atomic {
                    out.push(Piece::Text {
                        text: "\u{200b}".into(),
                        style: marker.clone(),
                    });
                }
                // Боковые поля, рамки и отступы СТРОЧНОЙ коробки: своей
                // коробки в раскладке у неё нет, поэтому место занимает
                // невидимый знак-распорка. Соединитель слов (U+FEFF) выбран
                // не случайно: точкой переноса он не является, а нулевой
                // пробел ею был бы — строка рвалась бы по краю `<span>`.
                // Физические стороны НЕ переставляются направлением
                // (CSS 2.1 §9.10): распорка левых полей выпускается в
                // логическом НАЧАЛЕ при ltr и в логическом КОНЦЕ при rtl —
                // иначе перестановка UAX#9 уводила `margin-left` вправо
                // (bidi-box-model-013 и вся семья).
                //
                // ПРОБОВАЛИ И ОТКАТИЛИ: выбирать сторону по РАЗРЕШЁННОМУ
                // уровню двунаправленности куска (UAX#9) вместо
                // унаследованного `direction`. Замерено: CSS2 4765 -> 4765,
                // приобретено 0 / потеряно 0. Уровень куска здесь ещё не
                // посчитан, и подмена сводилась к тому же `direction`.
                let ((mut mlead, mut mtrail), (mut lead, mut trail)) = inline_spacing::inline_sides(e, &merged, inherited);
                if merged.rtl == Some(true) {
                    std::mem::swap(&mut lead, &mut trail);
                    std::mem::swap(&mut mlead, &mut mtrail);
                }
                if mlead != 0.0 {
                    out.push(Piece::Text {
                        text: SPACER.into(),
                        style: margin_spacer_style(&merged, inherited, mlead),
                    });
                }
                // ПУСТАЯ строчная коробка: прогона текста у неё нет, а
                // отступ, рамку и фон рисовать надо (§8.4). Распорка их не
                // красит (см. запись у `spacer_style`), поэтому коробка идёт
                // отдельным слоем — он места в строке не занимает (место
                // держат распорки) и высоту строки не меняет (§10.8: поля,
                // отступы и рамки строчного в неё не входят).
                let mut inner_text = String::new();
                crate::render::gather_text_public(&e.children, &mut inner_text);
                // Из одних СХЛОПЫВАЕМЫХ пробелов — тоже пустая: пробелы
                // схлопнутся в соседний или срежутся у края строки (§4.1.1,
                // §4.1.3), а рамка без фона прогоном не рисуется
                // (`line-edge-white-space-collapse-001/002`: зелёная рамка
                // `<span>  </span>` пропадала).
                // Коробка с элементами внутри (атомы, `<br>`) не пустая, даже
                // без текста: слой нарисовал бы её одной строкой поверх
                // настоящих фрагментов (`border-radius-012`).
                let only_text = e.children.iter().all(|n| matches!(n, Node::Text(_)));
                let blank = only_text
                    && (inner_text.is_empty()
                        || (merged.keep_spaces != Some(true)
                            && merged.inline_bg.is_none()
                            && inner_text.chars().all(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))));
                if blank && (lead != 0.0 || trail != 0.0) {
                    let px_of = |l: Option<Len>| match l {
                        Some(Len::Px(v)) => v,
                        _ => 0.0,
                    };
                    let size = match merged.font_size {
                        Some(Len::Px(v)) => v,
                        _ => 16.0,
                    };
                    let bs = e.style.borders();
                    let padding = e.style.padding;
                    let flat = physical_sides::project(inherited, [
                        px_of(padding.top) + px_of(bs.top),
                        px_of(padding.right) + px_of(bs.right),
                        px_of(padding.bottom) + px_of(bs.bottom),
                        px_of(padding.left) + px_of(bs.left),
                    ]);
                    let top = flat[0];
                    // Высота области содержимого — подъём плюс спуск шрифта, как
                    // у полосы непустого куска (`run_background_quad`); кегль
                    // вместо неё оставлял под пустой коробкой светлую черту
                    // рядом с полосой соседа (`word-spacing-characters-001`).
                    let family = merged.font_family.clone().unwrap_or_else(|| {
                        if merged.monospace == Some(true) {
                            crate::metrics::mono_family_for(merged.lang.as_deref()).to_string()
                        } else {
                            String::new()
                        }
                    });
                    let (asc, desc, _) = crate::metrics::vmetrics_px(&family, size);
                    let content = if asc + desc > 0.0 { asc + desc } else { size };
                    let line = match merged.line_height {
                        Some(Len::Px(v)) => v,
                        Some(Len::Em(k)) => k * size,
                        _ => size * crate::metrics::normal_line(&family),
                    };
                    // Коробка стоит на области содержимого: она в середине
                    // строки, а полулидинг делит остаток поровну (§10.8).
                    let dy = ((line - content) / 2.0 - top).max(-line);
                    let mut copy = e.clone();
                    copy.style.width = Some(Len::Px(0.0));
                    copy.style.height = Some(Len::Px(content));
                    copy.style.margin = Default::default();
                    copy.style.position = None;
                    copy.style.display = None;
                    // Размер коробки читается из СЛИТОГО стиля: без этого
                    // заданные выше ширина и высота терялись, и коробка
                    // выходила нулевой высоты — рамка `border-left` и фон
                    // под отступом не рисовались вовсе.
                    let mut sized = merged.clone();
                    sized.width = copy.style.width;
                    sized.height = copy.style.height;
                    sized.margin = Default::default();
                    sized.position = None;
                    physical_sides::project_box(inherited, &mut copy.style);
                    physical_sides::project_box(inherited, &mut sized);
                    let boxel = crate::render::styled_div_with(&copy, &sized)
                        .absolute()
                        .top(gpui::px(dy));
                    out.push(Piece::Overlay(boxel.into_any_element(), OverlayAt::default()));
                }
                if lead != 0.0 {
                    out.push(Piece::Text {
                        text: SPACER.into(),
                        style: spacer_style(&merged, lead),
                    });
                }
                if blank
                    && has_text
                    && !atomic
                    && lead == 0.0
                    && trail == 0.0
                    && empty_inline::different_metrics(&merged, inherited)
                {
                    // CSS 2.1 section 10.8: an empty inline box contributes
                    // its line height and font metrics even without glyphs.
                    out.push(Piece::Text {
                        text: SPACER.into(),
                        style: spacer_style(&merged, 0.0),
                    });
                }
                // Своя сторона письма у куска — это знаки управления по
                // Юникоду: разбор двунаправленности их и ждёт, а рисовать их
                // не надо, ширины у них нет.
                let (open, close) = bidi_marks(&e.style, &merged);
                if let Some(mark) = open {
                    out.push(Piece::Text {
                        text: mark.to_string(),
                        style: merged.clone(),
                    });
                }
                // Относительный сдвиг строчного куска несёт и его потомков вне
                // потока: абсолютный элемент внутри `position: relative`
                // спана стоит от СДВИНУТОГО места (`static-position/htb-*`).
                out.extend(shift_overlays(
                    collect_with_empty_metrics(&e.children, &merged, atom, has_text),
                    &e.style,
                    merged.rotated_line == Some(true),
                ));
                if let Some(mark) = close {
                    out.push(Piece::Text {
                        text: mark.to_string(),
                        style: merged.clone(),
                    });
                }
                if trail != 0.0 {
                    out.push(Piece::Text {
                        text: SPACER.into(),
                        style: spacer_style(&merged, trail),
                    });
                }
                if mtrail != 0.0 {
                    out.push(Piece::Text {
                        text: SPACER.into(),
                        style: margin_spacer_style(&merged, inherited, mtrail),
                    });
                }
                if atomic {
                    out.push(Piece::Text {
                        text: "\u{200b}".into(),
                        style: marker,
                    });
                }
                // Зазор между последним знаком этого элемента и первым знаком
                // того, что идёт следом, лежит ВНУТРИ текущего узла — значит,
                // и величина его отсюда (css-text-3 §8.2). Ставится всегда, а
                // снимается ниже с последнего куска: за ним границы уже нет.
                set_boundary_spacing(&mut out[from..], inherited.letter_spacing);
            }
        }
    }
    // За последним куском границы внутри ЭТОГО узла нет: зазор там задаст тот
    // предок, у которого дальше идёт своё содержимое. Он и поставит его на
    // этот же кусок, когда сборка вернётся к нему.
    set_boundary_spacing(&mut out, None);
    if gap_at.is_some_and(|at| at + 1 == out.len()) {
        out.pop();
    }
    out
}

/// Начинается ли текст узла пробельным рядом с переводом строки.
fn leading_segment_break(raw: &str) -> bool {
    raw.chars()
        .take_while(|c| is_collapsible(*c))
        .any(|c| matches!(c, '\n' | '\r'))
}

/// Последний ЗНАЧАЩИЙ знак собранных кусков — нулевой пробел. Распорки полей
/// и рамок, метки направления и куски вне потока — это границы коробок, а
/// для преобразования перевода строки их нет.
fn ends_with_zwsp(out: &[Piece]) -> bool {
    for p in out.iter().rev() {
        match p {
            Piece::Overlay(..) => continue,
            Piece::Atom(_) => return false,
            Piece::Text { text, .. } => {
                if text.is_empty() || text == SPACER || text.chars().all(bidi_format) {
                    continue;
                }
                return text.ends_with('\u{200b}');
            }
        }
    }
    false
}

/// Зазор предка за строчной коробкой, ушедшей в раскладку (см. `collect`).
/// Только НЕатомарный строчный элемент без замещения: у атома и картинки
/// межбуквенного интервала по краям Blink не ставит (интервал живёт в наборе
/// текста, `shape_result.cc` `ApplySpacing`).
fn boundary_gap_after_box(e: &Element, inherited: &Computed) -> Option<f32> {
    let inline_level = e.style.display.is_none() || e.style.inline_display == Some(true);
    let replaced = matches!(
        e.tag.as_str(),
        "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe" | "input" | "button"
            | "select" | "textarea"
    );
    if !inline_level || replaced || e.style.ruby_role.is_some() {
        return None;
    }
    let size = match inherited.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    let gap = crate::metrics::spacing_px(
        inherited.letter_spacing,
        &inherited.font_family.clone().unwrap_or_default(),
        size,
    );
    (gap != 0.0).then_some(gap)
}

/// Зазор на границе элементов — на последний ЗНАЧАЩИЙ кусок набора.
///
/// Распорка строчной коробки пропускается: в её трекинге лежит поле коробки, а
/// не межбуквенный интервал, и перебивать его нельзя.
fn set_boundary_spacing(pieces: &mut [Piece], spacing: Option<Len>) {
    let last = pieces.iter_mut().rev().find_map(|p| match p {
        Piece::Text { text, style } if !text.is_empty() && text != SPACER => Some(style),
        _ => None,
    });
    if let Some(style) = last {
        style.letter_spacing_after = spacing;
    }
}

/// Кусок вне потока в РЯДУ: сам абзац его разместить не может (ряд собирает
/// раскладка), поэтому работает прежний обход — нулевая распорка на месте
/// куска и сам элемент в позднем слое, который рисуется от её угла.
fn overlay_in_row(el: AnyElement) -> AnyElement {
    let spot: crate::interact::SpotCell = Default::default();
    let probe = crate::interact::spot_probe(spot.clone(), false);
    match crate::interact::late_push(spot, el) {
        None => probe,
        Some(kept) => {
            let mut hole = gpui::div().relative().w_0().h_0().flex_shrink_0();
            hole.style().align_self = Some(gpui::AlignItems::FlexStart);
            hole.child(kept).into_any_element()
        }
    }
}

/// Относительный сдвиг коробки в точках: `left - right`, `top - bottom`
/// (CSS 2.1 §9.4.3). Нулевой, если элемент не относительный.
pub(crate) fn relative_inset(style: &Computed) -> (f32, f32) {
    if style.position != Some(crate::computed::Position::Relative) {
        return (0.0, 0.0);
    }
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    (
        px_of(style.inset.left) - px_of(style.inset.right),
        px_of(style.inset.top) - px_of(style.inset.bottom),
    )
}

/// Сдвинуть куски вне потока на относительный сдвиг их строчного предка.
///
/// `rotated` — абзац повёрнутого вертикального письма. Там до-поворотная
/// ось y — БЛОЧНАЯ ось экрана, и сдвиг по ней обязан округляться так же, как
/// глифы соседнего текста: `paint_glyph` кладёт глиф `floor` от физической
/// точки, а раскладка ставит край отбивки `round` (`taffy.rs: layout_bounds`).
/// На 2px при масштабе 1.25 (2.5 точки) коробка вставала на точку дальше
/// текста, и столбец красного в одну точку проступал у всех `cb`-случаев
/// `css-position/static-position/v{lr,rl}-*`. Поэтому сдвиг по y не идёт
/// отбивкой, а копится в `OverlayAt::rot_dy` и прикладывается `lines.rs`
/// одним округлением вместе с местом в строке.
fn shift_overlays(pieces: Vec<Piece>, style: &Computed, rotated: bool) -> Vec<Piece> {
    if style.position != Some(crate::computed::Position::Relative) {
        return pieces;
    }
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let (dx, dy) = (
        px_of(style.inset.left) - px_of(style.inset.right),
        px_of(style.inset.top) - px_of(style.inset.bottom),
    );
    if dx == 0.0 && dy == 0.0 {
        return pieces;
    }
    pieces
        .into_iter()
        .map(|p| match p {
            Piece::Overlay(el, at) if rotated => Piece::Overlay(
                el,
                OverlayAt {
                    rot_dx: at.rot_dx + dx,
                    rot_dy: at.rot_dy + dy,
                    ..at
                },
            ),
            Piece::Overlay(el, at) => Piece::Overlay(
                gpui::div()
                    .pl(gpui::px(dx))
                    .pt(gpui::px(dy))
                    .child(el)
                    .into_any_element(),
                at,
            ),
            other => other,
        })
        .collect()
}

/// Одеть первые `at` байт абзаца в стиль первой строки (`::first-line`).
///
/// Длину первой строки считает замер: до переноса она неизвестна.
pub fn style_first_line(pieces: Vec<Piece>, at: usize, style: &Computed) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::with_capacity(pieces.len() + 1);
    let mut seen = 0usize;
    for p in pieces {
        match p {
            Piece::Text { text, style: own } if seen < at => {
                let dress = |base: &Computed| {
                    let mut c = base.clone();
                    c.font_size = style.font_size.or(base.font_size);
                    c.color = style.color.or(base.color);
                    c.font_weight = style.font_weight.or(base.font_weight);
                    c.italic = style.italic.or(base.italic);
                    // Возможности шрифта первой строки, в том числе запрет
                    // подмены начертания (`nsyw`/`nsys`, css-fonts-4 §6.5):
                    // без них полужирный и курсив `::first-line` синтезировались
                    // вопреки `font-synthesis-*: none`. Дописываются ПОСЛЕ
                    // своих: при повторе тега побеждает первая строка.
                    c.font_features.extend(style.font_features.iter().cloned());
                    c.font_kerning = style.font_kerning.or(base.font_kerning);
                    c.font_alternates = style
                        .font_alternates
                        .clone()
                        .or_else(|| base.font_alternates.clone());
                    c.font_family = style.font_family.clone().or(base.font_family.clone());
                    c.font_settings = style
                        .font_settings
                        .clone()
                        .or_else(|| base.font_settings.clone());
                    // Коробочная часть первой строки: интерлиньяж и подложка
                    // (css-pseudo-4 §4.1; first-line-line-height-001/002).
                    c.background = style.background.or(base.background);
                    // Красит подложку прогон текста, и берёт он её из
                    // `inline_bg`: слой первой строки накладывается уже ПОСЛЕ
                    // сборки кусков, когда `inline_bg` посчитан по своему
                    // стилю (`c25-pseudo-elmnt-000`: зелёной полосы не было).
                    c.inline_bg =
                        first_line_background::paint_color(base.inline_bg, style.background);
                    c.line_height = style.line_height.or(base.line_height);
                    // ПРОБОВАЛИ И ОТКАТИЛИ: переносить сюда и сдвиг по
                    // вертикали (§5.12.1 относит `vertical-align` к свойствам
                    // `::first-line`). Проба по 372 парам семей `first-line-*`
                    // и `first-letter-*`: `first-line-pseudo-012` 4.66 -> 4.94,
                    // флипов ноль. Краска поднимается, а коробка нет: высоту
                    // абзаца заявляет `float::FirstLine` (`measure_first_line`),
                    // и подъёма она не знает. Возвращаться вместе с ней.
                    c
                };
                let len = text.len();
                if seen + len <= at {
                    let dressed = dress(&own);
                    out.push(Piece::Text {
                        text,
                        style: dressed,
                    });
                } else {
                    let mut cut = at - seen;
                    while cut < text.len() && !text.is_char_boundary(cut) {
                        cut += 1;
                    }
                    out.push(Piece::Text {
                        text: text[..cut].to_string(),
                        style: dress(&own),
                    });
                    if cut < text.len() {
                        out.push(Piece::Text {
                            text: text[cut..].to_string(),
                            style: own,
                        });
                    }
                }
                seen += len;
            }
            other => {
                if let Piece::Text { text, .. } = &other {
                    seen += text.len();
                }
                out.push(other);
            }
        }
    }
    out
}

/// Наследование: в CSS вниз идут только текстовые свойства. Бокс-свойства
/// (отступы, фон) принадлежат самому элементу и вниз не передаются.
/// Устанавливает ли коробка содержащий блок для внепоточных потомков
/// (§10.1 п.4 плюс барьеры css-transforms и css-contain).
///
/// Список ШИРЕ, чем `position` не `static`: лишний барьер значит невынесенный
/// элемент, то есть прежнее поведение, а пропущенный — вынос из коробки,
/// которая обязана была его удержать.
pub(crate) fn establishes_cb(c: &Computed) -> bool {
    // Растворённый элемент коробки не даёт и содержащим блоком быть не может
    // (css-display-3 §3.2).
    if matches!(
        c.display,
        Some(crate::computed::Display::Contents) | Some(crate::computed::Display::None)
    ) {
        return false;
    }
    matches!(
        c.position,
        Some(crate::computed::Position::Relative)
            | Some(crate::computed::Position::Absolute)
            | Some(crate::computed::Position::Fixed)
            | Some(crate::computed::Position::Sticky)
    ) || c.transform.is_some()
        || c.filter.is_some()
        || c.contain_paint == Some(true)
        || c.contain_layout == Some(true)
        // css-will-change-1 §2.1: обещание свойства, которое дало бы блок,
        // даёт его уже сейчас (`will-change-abspos-cb-002/003`).
        || c.will_change & (crate::computed::wc::CB_ABS | crate::computed::wc::CB_FIXED) != 0
}

/// Корень подложки (filter-effects-2 §BackdropRoot) — без корня документа:
/// его «Backdrop Root Image» и есть весь кадр. Фильтр у потомков
/// наследуется (`inherit`), но они и так под корнем.
fn backdrop_root(c: &Computed) -> bool {
    c.opacity.is_some_and(|o| o < 1.0)
        || c.filter.is_some_and(|f| f != crate::computed::Filter::neutral())
        || c.filter_ref.is_some()
        || c.mask_image.is_some()
        || c.clip_ref.is_some()
        || c.clip_polygon.is_some()
        || c.clip_shape.is_some()
        || c.blend.is_some_and(|b| b != 0)
        || c.backdrop_blur.is_some()
        || c.backdrop_color.is_some()
        // Любой `backdrop-filter`, кроме `none` (Overview.bs:119; Blink
        // paint_property_tree_builder.cc:1846), и `view-transition-name`
        // (css-view-transitions-1 Overview.bs:582).
        || c.backdrop_filter_set
        || c.vt_name
        // `clip-path` ЛЮБОЙ формой (Overview.bs:118; Blink
        // paint_property_tree_builder.cc:1887 `ClipPathClip()`): `inset()`,
        // `rect()`, `xywh()` и голая коробка разбором лежат не в
        // `clip_shape`/`clip_polygon` (`backdrop-filter-backdrop-root-clip-path-2`).
        || c.clip_inset.is_some()
        || c.clip_edges.is_some()
        || c.clip_xywh.is_some()
        || c.clip_bare_box
        || c.will_change_root
}

// ★ ЗАМЕРЕНО И ОТКАЧЕНО (06.09): `zoom` (css-viewport-1) как домножение
// использованных длин здесь, в `inherit`, плюс `zoom`/`zoom_eff` и `scale_px`
// в `Computed`. Срез 6315 пар (css-viewport + все пары с `zoom:` + весь
// CSS2): 5722 -> 4918, то есть **+354/-1158**. Потери — сплошь CSS2
// `background-*` (0.00 -> 8.3), таблицы, абсолюты; «приобретения» ложные:
// позеленели JS-тесты `insert-block-in-inlines-*`, у которых обе стороны
// сломались одинаково. Гейт `zoom_eff != 1.0` не удержал: домножение
// задело общий путь длин. Возвращаться только через отдельный проход после
// каскада, а не через самую горячую функцию крейта.
pub fn inherit(parent: &Computed, own: &Computed) -> Computed {
    let mut c = own.clone();
    c.cb_ancestor = parent.cb_ancestor || establishes_cb(parent);
    c.in_multicol = parent.in_multicol
        || ((parent.column_count.is_some()
            || parent.column_width.is_some()
            || parent.column_height.is_some())
            && !matches!(
                parent.display,
                Some(crate::computed::Display::Grid)
                    | Some(crate::computed::Display::InlineGrid)
                    | Some(crate::computed::Display::GridLanes)
                    | Some(crate::computed::Display::Flex)
                    | Some(crate::computed::Display::InlineFlex)
            ));
    c.backdrop_root_above = parent.backdrop_root_above || backdrop_root(parent);
    // filter-effects-2 §3 шаг 4: содержимое B — и его СОБСТВЕННЫЙ фон —
    // рисуется поверх уже отфильтрованной подложки. Фон коробки красит сама
    // коробка раньше детей, а канвас подложки — ребёнок; явный
    // `background-clip: border-box` уводит фон в слой-ребёнка
    // `render::clip_layer`, который идёт после декораций.
    if c.backdrop_color.is_some()
        && !c.backdrop_root_above
        && c.bg_clip.is_none()
        && (c.background.is_some() || c.gradient.is_some())
        && matches!(c.display, None | Some(crate::computed::Display::Block))
    {
        c.bg_clip = Some(crate::computed::BgClip::BorderBox);
    }
    // Ближайший содержащий блок абсолюта по `node_id` — ключ реестра рамок
    // `anchor::CB` (нужен `position-area`); корень даёт 0 = начальный
    // содержащий блок, окно.
    c.cb_node = if establishes_cb(parent) {
        parent.self_node
    } else {
        parent.cb_node
    };
    // Барьер содержащего блока для `position: fixed` — не только трансформ.
    // css-contain-1 §containment-layout п.1: обособление раскладки делает
    // элемент содержащим блоком для потомков и с `absolute`, И С `fixed`;
    // §containment-paint говорит то же про `contain: paint`. Для абсолюта
    // этот список уже полон (`establishes_cb`), а `fixed` признавал барьером
    // только `transform` и висел от окна вместо предка
    // (`contain-layout-containing-block-fixed-001`,
    // `contain-paint-containing-block-fixed-001`).
    c.transform_ancestor = parent.transform_ancestor
        || parent.transform.is_some()
        || parent.contain_layout == Some(true)
        || parent.contain_paint == Some(true)
        // css-will-change-1 §2.1: блок для `fixed` — и от обещанного свойства
        // (`will-change-fixpos-cb-*`, `-fixedpos-cb-*`).
        || parent.will_change & crate::computed::wc::CB_FIXED != 0;
    // `will-change: z-index` — контекст наложения ровно там, где `z-index`
    // действует: у позиционированной коробки и у элемента flex/grid
    // (css-flexbox-1 §5.4, css-grid-2 §6.2) — `-stacking-context-z-index-2/3`;
    // у простого блока — нет (`-z-index-4` зелёный и обязан остаться). Это не
    // домножение длин (★ выше): один битовый тест, страница без
    // `will-change` в ветку не заходит.
    if own.will_change & crate::computed::wc::STACK_Z != 0
        && (matches!(
            own.position,
            Some(crate::computed::Position::Relative)
                | Some(crate::computed::Position::Absolute)
                | Some(crate::computed::Position::Fixed)
                | Some(crate::computed::Position::Sticky)
        ) || matches!(
            parent.display,
            Some(crate::computed::Display::Flex)
                | Some(crate::computed::Display::InlineFlex)
                | Some(crate::computed::Display::Grid)
                | Some(crate::computed::Display::InlineGrid)
        ))
    {
        c.will_change |= crate::computed::wc::STACK;
    }
    c.cb_rtl = parent.rtl == Some(true);
    // Внутри повёрнутого абзаца родитель — горизонтальный клон
    // (`render.rs: paragraph`, `horizontal.vertical = None`), и о вертикальном
    // письме содержащего блока говорит только `rotated_line`.
    c.cb_vertical = parent.vertical == Some(true) || parent.rotated_line == Some(true);
    c.cb_vertical_rl = parent.vertical_rl == Some(true);
    c.cb_sideways = parent.sideways == Some(true);
    // Относительный сдвиг строчного предка КОПИТСЯ вниз (§9.4.3: сдвиг несёт
    // с собой всё содержимое коробки). Куски вне потока его получали
    // (`shift_overlays`), а вложенные куски самой строки — нет: сдвиг
    // родителя терялся на первом же слиянии стилей.
    if c.rel_shift.is_none() {
        c.rel_shift = parent.rel_shift;
    }
    // §10.5: доля высоты считается только от ОПРЕДЕЛЁННОЙ высоты содержащего
    // блока. Определена она у корня (его блок — начальный), при высоте
    // родителя в точках, при доле от определённого деда и у абсолютной
    // коробки — та задаёт отсчёт сама.
    // Элемент гибкого контейнера, сетки и содержимое ячейки получают
    // определённую высоту от РАСКЛАДКИ (css-flexbox-1 §9.8 растяжение,
    // css-grid-2 дорожки, §17.5.3 ячейка) — признака у нас на это нет, и
    // считать их блок неопределённым нельзя: замерено CSS3 2355 -> 2348,
    // потери во `flexbox-definite-sizes-*` и `*-subgrid-*`.
    let laid_out_parent = matches!(
        parent.display,
        Some(crate::computed::Display::Flex)
            | Some(crate::computed::Display::InlineFlex)
            | Some(crate::computed::Display::Grid)
            | Some(crate::computed::Display::InlineGrid)
            | Some(crate::computed::Display::GridLanes)
            | Some(crate::computed::Display::TableCell)
    );
    // Растяжение передаётся дальше: у растянутой коробки высота от полосы, и
    // для её потомков блок определён (`column-align-items-005`).
    c.stretched = laid_out_parent && !matches!(own.height, Some(crate::value::Len::Px(_)));
    // Высота, выведенная из `aspect-ratio` при определённой ширине, —
    // определённая (css-sizing-4 §5.1: «the resulting size is definite if
    // its input sizes are also definite»): проценты детей решаются от неё
    // (`percentage-resolution-001/002`, `flex-aspect-ratio-047/048`).
    let ratio_height = parent.aspect_ratio.is_some()
        && matches!(parent.height, None | Some(crate::value::Len::Auto))
        && matches!(parent.width, Some(crate::value::Len::Px(_)));
    // Ребёнок элемента КОЛОНКИ, у которой главный размер неопределён:
    // блок неопределён, и доля высоты ведёт себя как `auto`
    // (css-flexbox-1 §9.8 п.1-2; `percentage-heights-016/020`), кроме
    // случая, когда сам элемент имеет высоту в точках или основу в точках.
    let indefinite_column_item = parent.flex_main_def == Some(false)
        && !matches!(parent.height, Some(crate::value::Len::Px(_)))
        && !matches!(parent.flex_basis, Some(crate::value::Len::Px(_)))
        // Соотношение сторон САМО даёт главный размер: строчную ось
        // элемента колонки решает контейнер, и высота выводится из неё
        // (Blink `AspectRatioProvidesBlockMainSize`;
        // `flex-aspect-ratio-032/033` — ширина у элемента даже не задана).
        && parent.aspect_ratio.is_none();
    c.cb_height_def = !indefinite_column_item
        && (parent.root_box
        || laid_out_parent
        || parent.stretched
        || ratio_height
        || match parent.height {
            Some(crate::value::Len::Px(_)) => true,
            // Доля высоты у абсолютной/фиксированной коробки решается ВСЕГДА
            // (§10.5: оговорка «not absolutely positioned»; её же держит гейт
            // `apply.rs`), значит её высота для детей определена — как у
            // Blink, где внепоточная коробка отдаёт детям свой блочный размер.
            // Прежде `.modal {position: fixed; height: stretch}` передавал
            // ложный признак от `body`, и `height: 100%` цепочки гас
            // (`intrinsic-height-abspos-stretch-percentage-child`: 25 %).
            Some(crate::value::Len::Pct(_)) => {
                parent.cb_height_def
                    || matches!(
                        parent.position,
                        Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                    )
            }
            _ => matches!(
                parent.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            ),
        });
    // Quirks Mode §3.5 «The percentage height calculation quirk»: в режиме
    // quirks содержащий блок для ДОЛИ высоты ищется циклом — предки с
    // `height: auto` пропускаются, пока не найдётся предок с заданной
    // высотой, абсолютный или табличный (тогда он и есть опора). Сама
    // коробка с долей и табличный `display` квирку не подлежат. Без него
    // `<canvas style="height:100%">` в `div` без высоты внутри флоата
    // высотой 100 схлопывался в ноль (`intrinsic-percent-replaced-002/006`,
    // `float-percentage-resolution-quirks-mode`). Blink:
    // `LayoutBox::ContainingBlockLogicalHeightForPercentageResolution`
    // (`SkipContainingBlockForPercentHeightCalculation`).
    if crate::dom::quirks() {
        use crate::computed::{Display as D, Position as P};
        use crate::value::Len as L;
        let out_of_flow = matches!(own.position, Some(P::Absolute) | Some(P::Fixed));
        let tabular = matches!(
            own.display,
            Some(D::Table | D::InlineTable | D::TableCell | D::TableRow | D::TableRowGroup)
        );
        if let (Some(L::Pct(k)), false, false, Some(base)) =
            (own.height, c.cb_height_def, tabular, parent.quirk_pct_base)
        {
            c.height = Some(L::Px(k * base));
        }
        c.quirk_pct_base = match c.height {
            Some(L::Px(h)) => Some(h),
            // Доля от определённого блока — тоже опора: флоат `height: 50%`
            // в контейнере 200 даёт потомкам 100 (`intrinsic-percent-
            // replaced-003/004`).
            Some(L::Pct(k)) if !tabular => match parent.height {
                Some(L::Px(h)) => Some(k * h),
                _ => parent.quirk_pct_base.map(|b| k * b),
            },
            None | Some(L::Auto) if !out_of_flow && !tabular && !c.root_box => parent.quirk_pct_base,
            _ => None,
        };
    }
    c.color = own.color.or(parent.color);
    // `background-color: inherit` переносит вычисленное значение родителя —
    // вместе с нерешённой относительной функцией (css-color-5 §4.1).
    // Единица `lh` разрешается ЗДЕСЬ: высота строки известна после каскада.
    {
        // css-values-4 §6.1.4, оговорка про font-affecting properties:
        // «when ''lh'' or ''rlh'' units are used in the value of the
        // 'line-height' property or font-affecting properties on the element
        // they refer to, they resolve against the computed 'line-height' and
        // font metrics of the PARENT element». То есть `line-height: 2lh` и
        // `font-size: 2lh` меряются строкой РОДИТЕЛЯ, а не своей: иначе
        // выходит круговая зависимость. Для всего остального (`height: 1lh`)
        // базой остаётся своя строка — та же оговорка, скобка в конце абзаца.
        let parent_font = match parent.font_size {
            Some(crate::value::Len::Px(v)) => v,
            _ => 16.0,
        };
        let parent_family = parent.font_family.clone().unwrap_or_else(|| {
            if parent.monospace == Some(true) {
                crate::metrics::mono_family_for(parent.lang.as_deref()).to_string()
            } else {
                String::new()
            }
        });
        let parent_line = match parent.line_height {
            Some(crate::value::Len::Px(v)) => v,
            Some(crate::value::Len::Em(k)) | Some(crate::value::Len::Pct(k)) => k * parent_font,
            _ => {
                let f = crate::metrics::normal_line(&parent_family);
                if f > 0.0 {
                    f * parent_font
                } else {
                    1.2 * parent_font
                }
            }
        };
        // `c` — это `own.clone()`: слияние с родителем ещё впереди, значит в
        // `c.line_height`/`c.font_size` лежит ровно то, что задано НА ЭТОМ
        // элементе. Незаданное поле — `None`, и ветка молчит.
        let from_parent = |l: &mut Option<crate::value::Len>| match *l {
            Some(crate::value::Len::Lh(k)) => {
                *l = Some(crate::value::Len::Px(k * parent_line))
            }
            Some(crate::value::Len::LhPx(k, add)) => {
                *l = Some(crate::value::Len::Px(k * parent_line + add))
            }
            _ => {}
        };
        from_parent(&mut c.line_height);
        from_parent(&mut c.font_size);
        // `lh` — вычисленный `line-height` САМОГО элемента, а `line-height`
        // (как кегль и гарнитура) НАСЛЕДУЕТСЯ: незаданное на элементе берётся у
        // родителя. Ребёнок `height: 3lh` внутри `font: 16px / 32px` обязан
        // выйти 96, а не 3 × normal(16) ≈ 55 (`line-clamp-auto-035`: блок
        // с `height: 3lh` не доставал до потолка и не прятался).
        let font = match c.font_size {
            Some(crate::value::Len::Px(v)) => v,
            None => parent_font,
            _ => 16.0,
        };
        // `line-height: normal` — доля кегля ПО МЕТРИКАМ шрифта, а не
        // постоянные 1.2: у `lh`-единицы иначе выходила чужая высота строки
        // (`line-clamp-auto-*` меряют высоту в `lh`).
        let family = c
            .font_family
            .clone()
            .or_else(|| parent.font_family.clone())
            .unwrap_or_else(|| {
                if c.monospace.or(parent.monospace) == Some(true) {
                    crate::metrics::mono_family_for(c.lang.as_deref()).to_string()
                } else {
                    String::new()
                }
            });
        let line = match c.line_height.or(parent.line_height) {
            Some(crate::value::Len::Px(v)) => v,
            Some(crate::value::Len::Em(k)) | Some(crate::value::Len::Pct(k)) => k * font,
            _ => {
                let f = crate::metrics::normal_line(&family);
                if f > 0.0 { f * font } else { 1.2 * font }
            }
        };
        let fix = |l: &mut Option<crate::value::Len>| match *l {
            Some(crate::value::Len::Lh(k)) => *l = Some(crate::value::Len::Px(k * line)),
            Some(crate::value::Len::LhPx(k, add)) => {
                *l = Some(crate::value::Len::Px(k * line + add))
            }
            _ => {}
        };
        fix(&mut c.width);
        fix(&mut c.height);
        fix(&mut c.min_width);
        fix(&mut c.min_height);
        fix(&mut c.max_width);
        fix(&mut c.max_height);
        // `background-size: 100px 1lh` — та же единица и та же база
        // (`lh-unit-same-element-*`). Чинится ЗДЕСЬ, а не в `background.rs`:
        // там доступен только запасной кегль 16 (`fallback_len_px` даёт
        // 1.2 × 16 = 19.2 — ровно то, что видно на снимке), а сплошной
        // перевод единиц в самом `background.rs` уже ЗАМЕРЕН в минус
        // (★ `background.rs:2577`, CSS2 4614 → 4610).
        if let crate::computed::BgSize::Fixed(w, h) = &mut c.bg_size {
            fix(w);
            fix(h);
        }
    }
    // `box-shadow: inherit` копирует ВЫЧИСЛЕННУЮ тень родителя — вместе с
    // нерешённым `currentColor` (метка отрицательной альфы): решает её
    // отрисовка цветом СВОЕГО элемента.
    if own.shadow_inherit {
        c.shadows = parent.shadows.clone();
        c.inset_shadows = parent.inset_shadows.clone();
    }
    if own.border_inherit {
        c.border_width = parent.border_width;
        c.border_color = parent.border_color.or(parent.color);
        c.border_colors = parent.border_colors;
    }
    // `inherit` по сторонам и по частям рамки. Толщина без рисунка ничего не
    // рисует, поэтому вместе с ней переносится и он: у наследующей стороны
    // своего `border-style` обычно нет.
    for i in 0..4 {
        if own.border_inherit_w[i] {
            match i {
                0 => c.border_width.top = parent.border_width.top,
                1 => c.border_width.right = parent.border_width.right,
                2 => c.border_width.bottom = parent.border_width.bottom,
                _ => c.border_width.left = parent.border_width.left,
            }
        }
        if own.border_inherit_s[i] {
            c.border_visible[i] = parent.border_visible[i];
            c.border_side_styles[i] = parent.border_side_styles[i];
            c.border_dashed = parent.border_dashed;
            c.border_dotted = parent.border_dotted;
        }
        if own.border_inherit_c[i] {
            // Начальное значение `border-color` — `currentColor`, и по
            // css-color-3 наследуется оно КЛЮЧЕВЫМ СЛОВОМ: у родителя, своего
            // цвета рамки не задавшего, наследуется само слово, а решает его
            // цвет РЕБЁНКА (`border-color-011`).
            c.border_colors[i] = parent.border_colors[i].or(parent.border_color).or(c.color);
        }
    }
    if own.padding_inherit {
        c.padding = parent.padding;
    }
    // Ненаследуемые свойства со словом `inherit`: значение родителя берётся
    // целиком (§6.2.1). Разбор их слотов слово не выражает — там оно давало
    // умолчание или роняло объявление.
    if own.inherit_bits != 0 {
        use crate::computed::inh;
        let on = |b: u16| own.inherit_bits & b != 0;
        if on(inh::BG_REPEAT) {
            c.bg_repeat = parent.bg_repeat;
        }
        if on(inh::Z_INDEX) {
            c.z_index = parent.z_index;
        }
        if own.inherit_bits
            & (inh::OUTLINE_W | inh::OUTLINE_C | inh::OUTLINE_S | inh::OUTLINE_O)
            != 0
        {
            let from = parent.outline.unwrap_or_default();
            let mut o = c.outline.unwrap_or_default();
            if on(inh::OUTLINE_W) {
                o.width = from.width;
            }
            if on(inh::OUTLINE_C) {
                // `currentColor` вычисляется В СЕБЯ (css-color-4 §resolving):
                // у родителя он хранится ПУСТЫМ слотом, и наследовать надо
                // пустоту — цвет возьмётся от СВОЕГО текста, а не от чужого
                // (`outline-019`: родитель красный, ребёнок зелёный).
                o.color = from.color;
            }
            if on(inh::OUTLINE_S) {
                o.style = from.style;
            }
            if on(inh::OUTLINE_O) {
                o.offset = from.offset;
                // Метка `inset` — часть значения сдвига и наследуется с ним.
                o.inset = from.inset;
            }
            c.outline = Some(o);
        }
        if on(inh::DISPLAY) {
            c.display = parent.display;
            c.inline_display = parent.inline_display;
        }
        if on(inh::BG_IMAGE) {
            c.bg_image = parent.bg_image.clone();
            c.gradient = parent.gradient.clone();
            c.gradient_raw = parent.gradient_raw.clone();
        }
        if on(inh::BG_POS) {
            c.bg_pos = parent.bg_pos;
        }
        if on(inh::CLIP) {
            c.clip_rect = parent.clip_rect;
        }
        if on(inh::BG_ORIGIN) {
            c.bg_origin = parent.bg_origin;
        }
        if on(inh::BG_CLIP) {
            c.bg_clip = parent.bg_clip;
        }
        if on(inh::BG_SIZE) {
            c.bg_size = parent.bg_size;
        }
        if on(inh::TRANSFORM) {
            c.transform = parent.transform;
        }
        if on(inh::TRANSFORM_ORIGIN) {
            c.transform_origin = parent.transform_origin;
            c.transform_origin_px = parent.transform_origin_px;
            c.transform_origin_z = parent.transform_origin_z;
        }
        if on(inh::CLIP_MARGIN) {
            c.clip_margin = parent.clip_margin;
            c.clip_margin_box = parent.clip_margin_box;
        }
    }
    for (i, on) in own.margin_inherit.iter().enumerate() {
        if !on {
            continue;
        }
        match i {
            0 => c.margin.top = parent.margin.top,
            1 => c.margin.right = parent.margin.right,
            2 => c.margin.bottom = parent.margin.bottom,
            _ => c.margin.left = parent.margin.left,
        }
    }
    for (i, on) in own.padding_inherit_side.iter().enumerate() {
        if !on {
            continue;
        }
        match i {
            0 => c.padding.top = parent.padding.top,
            1 => c.padding.right = parent.padding.right,
            2 => c.padding.bottom = parent.padding.bottom,
            _ => c.padding.left = parent.padding.left,
        }
    }
    if own.width_inherit {
        c.width = parent.width;
    }
    if own.height_inherit {
        c.height = parent.height;
    }
    for (i, on) in own.minmax_inherit.iter().enumerate() {
        if !on {
            continue;
        }
        match i {
            0 => c.min_width = parent.min_width,
            1 => c.min_height = parent.min_height,
            2 => c.max_width = parent.max_width,
            _ => c.max_height = parent.max_height,
        }
    }
    for (i, on) in own.inset_inherit.iter().enumerate() {
        if !on {
            continue;
        }
        match i {
            0 => c.inset.top = parent.inset.top,
            1 => c.inset.right = parent.inset.right,
            2 => c.inset.bottom = parent.inset.bottom,
            _ => c.inset.left = parent.inset.left,
        }
    }
    if own.background_inherit {
        c.background = parent.background;
        c.background_rcs = own.background_rcs.clone().or(parent.background_rcs.clone());
        if own.background_all_inherit {
            c.bg_image = parent.bg_image.clone();
            c.bg_repeat = parent.bg_repeat;
            c.bg_pos = parent.bg_pos;
            c.bg_size = parent.bg_size;
            c.bg_fixed = parent.bg_fixed;
            c.gradient = parent.gradient.clone();
            c.gradient_raw = parent.gradient_raw.clone();
        }
    }
    // Относительный цвет решается ЗДЕСЬ: только теперь известен цвет самого
    // элемента. Функция остаётся в поле — её унаследуют дети и решат своим
    // цветом заново.
    if let Some(expr) = c.background_rcs.clone() {
        let current = c.color.unwrap_or(crate::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });
        if let Some(resolved) = crate::color_space::resolve_relative(&expr, current) {
            c.background = Some(resolved);
        }
    }
    // `c` — клон `own`, но `lh` в собственном кегле уже решён выше от строки
    // РОДИТЕЛЯ (css-values-4 §6.1.1, `from_parent`). Брать `own` здесь значило
    // вернуть сырое `Len::Lh` — кегль терялся (`lh-unit-002`).
    // Детям уходит ВЫЧИСЛЕННЫЙ кегль, а не подогнанный `font-size-adjust`:
    // «child elements inherit the computed font-size value (otherwise, the
    // effect of font-size-adjust would compound)» (css-fonts-4 §2.5).
    c.font_size = c.font_size.or(match parent.font_adjust_base {
        Some((px, _)) => Some(Len::Px(px)),
        None => parent.font_size,
    });
    // Кегль НОЛЬ вешает набор намертво (DirectWrite-цикл: `font: 0 Ahem` из
    // vars-font-shorthand-001 замораживал страницу навсегда) — клэмп к
    // микроскопическому: визуально то же «ничего», формулы живы.
    if let Some(Len::Px(v)) = c.font_size {
        if v <= 0.0 {
            c.font_size = Some(Len::Px(0.01));
        }
    }
    c.font_weight = own.font_weight.or(parent.font_weight);
    c.italic = own.italic.or(parent.italic);
    c.oblique = own.oblique.or(parent.oblique);
    c.underline = own.underline.or(parent.underline);
    c.line_through = own.line_through.or(parent.line_through);
    // То же для высоты строки: `line-height: 2lh` уже переведён в точки от
    // строки родителя; сырое `Lh` уходило в `apply.rs` как `relative(2)` от
    // СВОЕГО кегля (`lh-unit-001`: 84 вместо 100).
    // У подогнанного родителя числовой `line-height` переведён в точки ЕГО
    // вычисленным кеглем; ребёнок наследует сам множитель.
    c.line_height = c.line_height.or(match parent.font_adjust_base {
        Some((_, lh)) => lh,
        None => parent.line_height,
    });
    c.text_align = own.text_align.or(parent.text_align);
    c.no_justify = own.no_justify.or(parent.no_justify);
    c.ruby_justify = own.ruby_justify.or(parent.ruby_justify);
    c.ruby_unit = own.ruby_unit || parent.ruby_unit;
    c.text_align_last = own.text_align_last.or(parent.text_align_last);
    c.hanging = own.hanging.or(parent.hanging);
    c.monospace = own.monospace.or(parent.monospace);
    c.font_family = own.font_family.clone().or(parent.font_family.clone());
    c.nowrap = own.nowrap.or(parent.nowrap);
    c.orphans = own.orphans.or(parent.orphans);
    c.widows = own.widows.or(parent.widows);
    // Направление письма наследуется: `writing-mode` ставят на `body`, а ось
    // потока обязана смениться у КАЖДОГО вложенного блока — иначе вертикально
    // становится только сам `body`, а его дети снова текут вниз.
    c.vertical = own.vertical.or(parent.vertical);
    c.vertical_rl = own.vertical_rl.or(parent.vertical_rl);
    c.sideways = own.sideways.or(parent.sideways);
    c.combine_upright = own.combine_upright.or(parent.combine_upright);
    c.rotated_line = own.rotated_line.or(parent.rotated_line);
    c.ortho_limit = own.ortho_limit.or(parent.ortho_limit);
    c.orthogonal_scrollport = parent.orthogonal_scrollport;
    c.orthogonal_inline = parent.orthogonal_inline;
    c.wrap_anywhere = own.wrap_anywhere.or(parent.wrap_anywhere);
    c.word_space_char = own.word_space_char.or(parent.word_space_char);
    c.autospace_alpha = own.autospace_alpha.or(parent.autospace_alpha);
    c.autospace_numeric = own.autospace_numeric.or(parent.autospace_numeric);
    // Сдвиг НЕ наследуется: он принадлежит своему куску, иначе надстрочный
    // знак поднимал бы весь текст после себя.
    c.vertical_shift = own.vertical_shift;
    c.vertical_shift_px = own.vertical_shift_px;
    c.vertical_shift_len = own.vertical_shift_len;
    c.vertical_align_text = own.vertical_align_text;
    // Кегль родителя нужен `text-top`/`text-bottom`: край куска равняется по
    // ЕГО текстовой области.
    c.vertical_align_base = match parent.font_size {
        Some(crate::value::Len::Px(v)) => Some(v),
        _ => own.vertical_align_base,
    };
    c.upright = own.upright.or(parent.upright);
    c.text_sideways = own.text_sideways.or(parent.text_sideways);
    // Сетка-родитель и её письмо: у вертикальной сетки оси выравнивания
    // элемента переставляются (`apply.rs`), а по оси x идут группы базовых.
    c.parent_grid = match parent.display {
        Some(crate::computed::Display::Grid) | Some(crate::computed::Display::InlineGrid) => {
            match (parent.vertical == Some(true), parent.vertical_rl == Some(true)) {
                (false, _) => 1,
                (true, false) => 2,
                (true, true) => 3,
            }
        }
        _ => 0,
    };
    c.parent_lanes = parent.display == Some(crate::computed::Display::GridLanes);
    c.parent_subgrid = c.parent_grid != 0 && (parent.subgrid_cols || parent.subgrid_rows);
    c.parent_flex_grid = matches!(
        parent.display,
        Some(crate::computed::Display::Flex)
            | Some(crate::computed::Display::InlineFlex)
            | Some(crate::computed::Display::Grid)
            | Some(crate::computed::Display::InlineGrid)
            | Some(crate::computed::Display::GridLanes)
    );
    // Наследуемые текстовые свойства из второй волны разбора. Без них
    // `text-transform` на контейнере не доходил до вложенного текста —
    // а в разметке его ставят именно на контейнер.
    c.letter_spacing = own.letter_spacing.or(parent.letter_spacing);
    c.word_spacing = own.word_spacing.or(parent.word_spacing);
    // Сторона подписи таблицы наследуется (CSS 2.1: caption-side inherited) —
    // читается потом С САМОГО заголовка (caption-side-applies-to-012..015:
    // значение на ряде до заголовка не доходит).
    c.caption_bottom = own.caption_bottom.or(parent.caption_bottom);
    c.text_transform = own.text_transform.or(parent.text_transform);
    // Добавки — часть того же значения: своё объявление заменяет их целиком.
    c.text_transform_flags = if own.text_transform.is_some() {
        own.text_transform_flags
    } else {
        parent.text_transform_flags
    };
    c.text_indent = own.text_indent.or(parent.text_indent);
    // `text-box-edge` наследуется (css-inline-3 §text-box-edge, Inherited:
    // yes); срез берёт край у корневой строчной коробки СТРОКИ, то есть у
    // блока, которому она принадлежит (`text-box-trim-accumulation-001…003`).
    if !own.text_box_edge_set {
        c.text_box_over = parent.text_box_over;
        c.text_box_under = parent.text_box_under;
        c.text_box_edge_set = parent.text_box_edge_set;
    }
    c.text_indent_each_line = own.text_indent_each_line.or(parent.text_indent_each_line);
    c.text_indent_hanging = own.text_indent_hanging.or(parent.text_indent_hanging);
    c.break_anywhere = own.break_anywhere.or(parent.break_anywhere);
    c.break_word = own.break_word.or(parent.break_word);
    c.balance_lines = own.balance_lines.or(parent.balance_lines);
    c.break_anywhere_strict = own.break_anywhere_strict.or(parent.break_anywhere_strict);
    c.line_break_loose = own.line_break_loose.or(parent.line_break_loose);
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): снимать `align-self` у коробки, чей
    // родитель не гибкий контейнер и не сетка (css-align-3 §6.2 «does not
    // apply to block-level boxes»). По спеке верно, но наш блок собран
    // колонкой flex, и на `align_self` держатся собственные приёмы сборки:
    // соотношение сторон блока (`blocks()` ставит `Align::Start`), обтекание,
    // сжатие стола. Узкий срез выравнивания (2661 пара) дал +8/−1, а ПОЛНЫЙ
    // свод v18 -> v19 — минус ~50: вся семья `float-applies-to-*` (0.00 ->
    // 3.84), `floats-002/025/147`, `clear-float-001/003`,
    // `block-aspect-ratio-002/015/016/018/043/047`, девять
    // `shape-outside-*-border-radius-*`, `absolute-replaced-width-020/034`.
    // Возвращать вместе с признаком «значение авторское», чтобы приёмы сборки
    // гейт не задевал (`self-align-start-end-flex-001` — цель правки).
    // `normal` у элемента ГИБКОГО контейнера = `stretch` (§6.2), а не
    // «пусто»: пустое значение брало `align-items` родителя
    // (`self-align-normal-flex`).
    if own.align_self_normal
        && matches!(
            parent.display,
            Some(crate::computed::Display::Flex) | Some(crate::computed::Display::InlineFlex)
        )
    {
        c.align_self = Some(crate::computed::Align::Stretch);
    }
    // `self-start`/`self-end` меряются по письму САМОГО элемента (css-align-3
    // §6.2). Значение уже физическое (начало = левый край при ltr), поэтому
    // зеркалим ровно тогда, когда строчная ось элемента смотрит в другую
    // сторону, чем у родителя: `flexbox-align-self-vert-002` даёт элементам
    // `direction: rtl` внутри ltr-колонки и ждёт `self-start` СПРАВА.
    // Вертикальное письмо здесь НЕ зеркалим намеренно: там ось строки уже
    // переставлена поворотом — это территория wm-скаута.
    // Письмо элемента — ДЕЙСТВУЮЩЕЕ (своё или унаследованное): незаданное
    // `direction` у элемента в rtl-колонке — тоже rtl, и зеркалить нечего
    // (поперечную ось rtl-колонки разворачивает сама раскладка,
    // `apply.rs`: `flex_cross_reverse` / `flip`).
    if c.parent_grid == 0 && own.align_self_own_axis && own.rtl.or(parent.rtl).unwrap_or(false) != parent.rtl.unwrap_or(false) {
        c.align_self = match c.align_self {
            Some(crate::computed::Align::Start) => Some(crate::computed::Align::End),
            Some(crate::computed::Align::End) => Some(crate::computed::Align::Start),
            other => other,
        };
    }
    // `start`/`end` (и `self-*`) меряются по ПИСЬМУ, а раскладка знает только
    // гибкие концы (`apply::to_items` → `FlexStart`/`FlexEnd`), которые
    // АВТОРСКИЙ `wrap-reverse` родителя переворачивает (css-align-3 §6.1,
    // css-flexbox-1 §5.2). Концы письма меняются местами ровно тогда: taffy
    // развернёт их обратно. Переворот поперёк письма (`apply.rs`: `flip`
    // через тот же `WrapReverse`) здесь не участвует — он виден раскладке и
    // для `flex-*`, и для `start` одинаково (`self-align-start-end-flex-001`).
    if !own.align_self_flex_kw
        && parent.flex_wrap_reverse == Some(true)
        && matches!(
            parent.display,
            Some(crate::computed::Display::Flex) | Some(crate::computed::Display::InlineFlex)
        )
    {
        c.align_self = match c.align_self {
            Some(crate::computed::Align::Start) => Some(crate::computed::Align::End),
            Some(crate::computed::Align::End) => Some(crate::computed::Align::Start),
            other => other,
        };
    }
    c.text_emphasis = own.text_emphasis.clone().or(parent.text_emphasis.clone());
    c.emphasis_under = own.emphasis_under || parent.emphasis_under;
    c.emphasis_color = own.emphasis_color.or(parent.emphasis_color);
    // css-ruby-1 §4.1/§4.3: оба свойства наследуемые.
    c.ruby_under = own.ruby_under.or(parent.ruby_under);
    c.ruby_align = own.ruby_align.or(parent.ruby_align);
    c.ruby_merge = own.ruby_merge.or(parent.ruby_merge);
    // `image-orientation` наследуется (css-images-3 §5.4, «Inherited: yes»):
    // в наборе его ставят на `body`, а действует он на каждой картинке.
    c.image_orient_none = own.image_orient_none.or(parent.image_orient_none);
    c.font_synth = (
        own.font_synth.0.or(parent.font_synth.0),
        own.font_synth.1.or(parent.font_synth.1),
        own.font_synth.2.or(parent.font_synth.2),
    );
    c.keep_all = own.keep_all.or(parent.keep_all);
    c.hyphens_auto = own.hyphens_auto.or(parent.hyphens_auto);
    c.lang = own.lang.clone().or(parent.lang.clone());
    c.break_after_spaces = own.break_after_spaces.or(parent.break_after_spaces);
    c.hyphenate = own.hyphenate.or(parent.hyphenate);
    c.tab_size = if own.tab_size_len.is_some() {
        None
    } else {
        own.tab_size.or(parent.tab_size)
    };
    c.list_style_type = own
        .list_style_type
        .clone()
        .or_else(|| parent.list_style_type.clone());
    c.list_style_inside = own.list_style_inside.or(parent.list_style_inside);
    c.vertical_align = own.vertical_align.or(parent.vertical_align);
    c.font_stretch = own.font_stretch.or(parent.font_stretch);
    // `font-size-adjust` наследуется значением; подгонку каждый элемент
    // считает сам, по СВОЕМУ шрифту (`Computed::resolve_em`).
    c.font_size_adjust = own.font_size_adjust.or(parent.font_size_adjust);
    c.no_select = own.no_select.or(parent.no_select);
    c.pointer_events_none = own.pointer_events_none.or(parent.pointer_events_none);
    c.line_clamp = own.line_clamp.or(parent.line_clamp);
    c.clamp_legacy = own.clamp_legacy.or(parent.clamp_legacy);
    // Гейтовые флаги -webkit-box НЕ наследуются: пара display+orient
    // обязана стоять на самом элементе.
    c.webkit_box = own.webkit_box;
    c.webkit_box_vertical = own.webkit_box_vertical.or(parent.webkit_box_vertical);
    // Фон строчного бокса идёт вниз как текстовое свойство: он принадлежит
    // строке, а не коробке, и вложенный `<b>` внутри подсветки обязан его
    // сохранить.
    c.inline_bg = own.inline_bg.or(parent.inline_bg);
    c.inline_border = own.inline_border.or(parent.inline_border);
    c.inline_pad = own.inline_pad.or(parent.inline_pad);
    c.inline_radius = own.inline_radius.or(parent.inline_radius);
    if c.font_features.is_empty() {
        c.font_features = parent.font_features.clone();
    }
    c.font_kerning = own.font_kerning.or(parent.font_kerning);
    c.font_alternates = own
        .font_alternates
        .clone()
        .or_else(|| parent.font_alternates.clone());
    // `font-feature-settings` наследуется своим значением независимо от
    // `font-variant-*` ребёнка (css-fonts-4 §6.12: `font-variant: none` «does
    // not reset … font-feature-settings»).
    c.font_settings = own
        .font_settings
        .clone()
        .or_else(|| parent.font_settings.clone());
    c.text_shadow = own.text_shadow.or(parent.text_shadow);
    // Хвост списка идёт вместе с первой тенью: своя запись — свой хвост,
    // унаследованная — хвост родителя (css-text-decor-3: `text-shadow`
    // наследуется списком целиком).
    if own.text_shadow.is_none() {
        c.text_shadow_rest = parent.text_shadow_rest.clone();
    }
    c.rtl = own.rtl.or(parent.rtl);
    // `text-align: start|end` — края СТРОКИ, и разворачиваются они в момент
    // ОТРИСОВКИ, а не здесь: иначе левый край, вычисленный для тела страницы,
    // достаётся по наследству и вложенному блоку справа налево (`physical`
    // зовёт `lines::align_for`). Умолчание CSS — `start`.
    c.text_align = Some(c.text_align.unwrap_or(TextAlign::Start));
    // Фильтр в CSS красит элемент И ВСЁ поддерево. Наследуем его сами и
    // применяем к собственным цветам потомка: раньше фильтр действовал только
    // на узел, где написан, и дети оставались цветными.
    if c.filter.is_none() {
        c.filter = parent.filter;
    }
    // ЕДИНСТВЕННАЯ точка окраски фильтром (каскад цвета не трогает).
    // Красится только ВОЗНИКШЕЕ на этом узле: унаследованный цвет уже
    // покрашен предком — повторная окраска давала f^N по поколениям.
    if let Some(f) = c.filter {
        if own.background.is_some() {
            c.background = c.background.map(|col| f.apply(col));
        }
        if own.color.is_some() || own.filter.is_some() && parent.color.is_none() {
            c.color = c.color.map(|col| f.apply(col));
        }
        if own.border_color.is_some() || own.border_color_is_current {
            c.border_color = c.border_color.map(|col| f.apply(col));
        }
        for (side, own_side) in c.border_colors.iter_mut().zip(own.border_colors.iter()) {
            if own_side.is_some() {
                *side = side.map(|col| f.apply(col));
            }
        }
        if own.gradient.is_some()
            && let Some(g) = c.gradient.as_mut()
        {
            g.from = f.apply(g.from);
            g.to = f.apply(g.to);
            for stop in g.stops.iter_mut() {
                stop.0 = f.apply(stop.0);
            }
        }
        // Растровый градиент (`conic`, `repeating-*`) живёт строкой в
        // `bg_image`: его стопы красятся в самой записи
        // (`filter-function-repeating-*-ref`: `filter: invert(1)` на фоне).
        if own.bg_image.is_some()
            && let Some(img) = c.bg_image.as_deref()
            && crate::computed::gradient_as_raster(img)
        {
            c.bg_image = Some(crate::computed::filter_gradient_text(img, &f));
        }
        if !own.shadows.is_empty() {
            for sh in c.shadows.iter_mut() {
                sh.color = f.apply(sh.color);
            }
        }
    }
    // `filter: drop-shadow()` у коробки со СПЛОШНЫМ фоном: силуэт такой
    // коробки — border-box со скруглением, и тень фильтра (filter-effects-1
    // §dropshadowEquivalent: размытая альфа входа, сдвиг, цвет — ПОД входом)
    // совпадает с внешней box-shadow без разлёта. Картинку поддерева так не
    // выразить — только коробку; повторное слияние тень не удваивает.
    // Длина размытия у `drop-shadow()` — это σ (filter-effects-1
    // §funcdef-filter-drop-shadow: «standard deviation»), а у `box-shadow`
    // радиус = 2σ (css-backgrounds-3 §box-shadow) — в список внешних теней
    // она идёт удвоенной, чтобы после деления в `apply::apply_paint` σ
    // осталась своей.
    if let Some(sh) = c.drop_shadow
        && c.background.is_some_and(|b| b.a >= 1.0)
    {
        let as_box = crate::computed::Shadow {
            blur: sh.blur * 2.0,
            ..sh
        };
        if !c.shadows.contains(&as_box) {
            c.shadows.push(as_box);
        }
    }
    // `background-clip: text` со СПЛОШНОЙ заливкой (css-backgrounds-4
    // §background-clip): фон виден только под глифами элемента и его
    // поточных и плавающих потомков, а сам текст красится ПОВЕРХ фона своим
    // цветом. Для одноцветного непрозрачного фона это ровно «цвет текста
    // поверх заливки» — маска глифов не нужна, а подчёркивания, многоточие и
    // знаки выделения цветом `currentColor` получают тот же цвет сами.
    // Смешивается только ВОЗНИКШЕЕ на узле (как у фильтра выше): унаследованный
    // цвет уже смешан предком. Внепоточные потомки в геометрию текста не входят
    // (`clip-text-out-of-flow-child`) и получают несмешанный цвет обратно.
    let black = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let out_of_flow = matches!(
        own.position,
        Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
    );
    if let Some(fill) = crate::background::text_clip_fill(&c) {
        c.text_clip_raw = c.color;
        c.text_clip_fill = Some(fill);
        c.color = Some(crate::background::over(c.color.unwrap_or(black), fill));
    } else if parent.text_clip_fill.is_some() && out_of_flow {
        c.text_clip_fill = None;
        c.text_clip_raw = None;
        if own.color.is_none() {
            c.color = parent.text_clip_raw;
        }
    } else if let Some(fill) = parent.text_clip_fill {
        c.text_clip_fill = Some(fill);
        if own.color.is_some() {
            c.text_clip_raw = c.color;
            c.color = Some(crate::background::over(c.color.unwrap_or(black), fill));
        } else {
            c.text_clip_raw = parent.text_clip_raw;
        }
    }
    // Наследуемые по CSS, но забытые прежде: без них `white-space: pre` на
    // контейнере не доходил до вложенного текста, а маркер, курсор и зазор
    // ячеек не доставались детям.
    c.preserve_newlines = own.preserve_newlines.or(parent.preserve_newlines);
    c.keep_spaces = own.keep_spaces.or(parent.keep_spaces);
    c.hidden = own.hidden.or(parent.hidden);
    c.cursor = own.cursor.clone().or(parent.cursor.clone());
    c.no_marker = own.no_marker.or(parent.no_marker);
    c.border_collapse = own.border_collapse.or(parent.border_collapse);
    c.empty_cells_hide = own.empty_cells_hide.or(parent.empty_cells_hide);
    c.border_spacing = own.border_spacing.or(parent.border_spacing);
    c.caret_color = own.caret_color.or(parent.caret_color);
    // `em` считается от размера шрифта — а он известен только здесь, когда
    // наследование уже произошло. Раньше длина переводилась в точки при
    // разборе, по постоянным 16 точкам, и вложенные кегли не перемножались.
    // База `em` у собственного кегля — ВЫЧИСЛЕННЫЙ кегль родителя:
    // `font-size-adjust` «does not affect the size of em units».
    let parent_px = match parent.font_adjust_base {
        Some((px, _)) => px,
        None => match parent.font_size {
            Some(Len::Px(px)) => px,
            _ => 16.0,
        },
    };
    // Процент у размера шрифта — доля родительского кегля; в точках его надо
    // получить здесь, иначе абзац уходил в запасную ветку переноса (размер
    // «не такой, как у базового») и терял перенос по словам.
    // `larger`/`smaller`: шаг по таблице ключевых кеглей, если кегль
    // родителя в ней стоит (§15.7; таблица та же, что у слов в
    // `computed.rs`). Вне таблицы работает запасной `Len::Em` (1.2 и 5/6).
    if c.font_size_step != 0 {
        const TABLE: [f32; 8] = [9.0, 10.0, 13.0, 16.0, 18.0, 24.0, 32.0, 48.0];
        if let Some(i) = TABLE.iter().position(|t| (t - parent_px).abs() < 0.01) {
            let j = i as i32 + i32::from(c.font_size_step);
            if (0..TABLE.len() as i32).contains(&j) {
                c.font_size = Some(Len::Px(TABLE[j as usize]));
            }
        }
    }
    if let Some(Len::Pct(k)) = c.font_size {
        c.font_size = Some(Len::Px(k * parent_px));
    }
    c.resolve_em(parent_px);
    // Цвет рамки по умолчанию — ЦВЕТ ТЕКСТА: `border: solid 1px` без цвета
    // рисуется в браузере чёрной рамкой, а у нас не рисовалась вовсе —
    // коробка выходила без рамки, и эталоны переносов выглядели сломанными.
    // Известен цвет только здесь: он наследуемый, а рамка нет.
    let has_border = {
        let b = c.borders();
        [b.top, b.right, b.bottom, b.left]
            .iter()
            .any(|w| !matches!(w, None | Some(Len::Px(0.0))))
    };
    // Сторона с `currentColor` из бокового сокращения при ОБЩЕМ цвете рамки:
    // ей положен цвет текста, а не общий (§8.5.4; `border-shorthands-003`).
    // Прочие стороны получают общий цвет ЯВНО: единый цвет квада
    // (`apply::apply_paint`) смотрит только на заданные стороны и иначе
    // выкрасил бы их цветом помеченной. Без общего цвета пустой слот и так
    // даёт цвет текста.
    if c.border_color.is_some() && c.border_side_current.iter().any(|f| *f) {
        let current = c.color.unwrap_or(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });
        for i in 0..4 {
            if c.border_colors[i].is_none() {
                c.border_colors[i] = if c.border_side_current[i] {
                    Some(current)
                } else {
                    c.border_color
                };
            }
        }
    }
    if has_border && c.border_color.is_none() && c.border_colors.iter().all(Option::is_none) {
        c.border_color = c.color.or(Some(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }));
    }
    // `tab-size` в длине наследуется АБСОЛЮТНОЙ величиной: `5em` при кегле
    // 10px — это 50px и у ребёнка с кеглем 20px, а не его собственные 5em
    // (`tab-size-inheritance-001`).
    let own_px = match c.font_size {
        Some(Len::Px(px)) => px,
        _ => parent_px,
    };
    c.tab_size_len = match own.tab_size_len {
        Some(len) => Some(Len::Px(crate::metrics::spacing_px(
            Some(len),
            &c.font_family.clone().unwrap_or_default(),
            own_px,
        ))),
        None if own.tab_size.is_some() => None,
        None => parent.tab_size_len,
    };
    c
}

/// `hyphens: auto` — расставить знаки мягкого переноса по слогоразделу.
///
/// Ставится ИМЕННО мягкий перенос (U+00AD): вся машинерия под него уже есть —
/// он даёт точку разрыва, сам ширины не имеет и на конце строки показывается
/// знаком из `hyphenate-character`. Образцы Лианга вшиты в `hypher` по языкам;
/// язык берётся из атрибута `lang`, без него — английский.
///
/// Слово ищется по ТЕКСТУ ВСЕГО АБЗАЦА, а не по одному куску: разметка режет
/// слова где угодно (`<span>high</span>way`), и по кускам порознь слогораздел
/// давал бы другие точки, чем у целого слова — а тест требует ровно тех же
/// (`hyphens-span-002`). Найденные точки раскладываются обратно в тот кусок,
/// которому принадлежат.
///
/// Слово с уже расставленными вручную знаками не трогаем: разметка знает
/// лучше.
pub fn hyphenate_pieces(pieces: &mut [Piece]) {
    // Текст абзаца и карта «глобальное смещение → кусок». Не-текстовый кусок
    // слово РАЗРЫВАЕТ: картинка посреди букв словом их не делает.
    let mut whole = String::new();
    let mut map: Vec<(usize, usize)> = Vec::new(); // (начало в тексте, кусок)
    let mut any = false;
    for (i, p) in pieces.iter().enumerate() {
        match p {
            Piece::Text { text, style } => {
                if style.hyphens_auto == Some(true) {
                    any = true;
                }
                map.push((whole.len(), i));
                whole.push_str(text);
            }
            // Кусок ВНЕ потока места в строке не занимает и слова не рвёт:
            // текста абзаца он не составляет вовсе
            // (`hyphens-out-of-flow-002`: `high<span abspos>…</span>way` —
            // это по-прежнему одно слово `highway`).
            Piece::Overlay(..) => {}
            // А вот атомарная коробка (картинка, `inline-block`) в строке
            // стоит и соседство букв разрывает.
            Piece::Atom(_) => {
                map.push((whole.len(), usize::MAX));
                whole.push('\u{0}');
            }
        }
    }
    if !any || whole.contains('\u{00ad}') {
        return;
    }
    let owner = |at: usize| -> usize {
        map.iter()
            .rev()
            .find(|(start, _)| *start <= at)
            .map_or(usize::MAX, |(_, i)| *i)
    };
    // Точки переноса: глобальное смещение. Собираем по всему тексту, вставляем
    // с конца — иначе ранние вставки сдвигают поздние смещения.
    let mut cuts: Vec<usize> = Vec::new();
    let mut at = 0usize;
    while at < whole.len() {
        let rest = &whole[at..];
        let Some(off) = rest.find(|c: char| c.is_alphabetic()) else {
            break;
        };
        let from = at + off;
        let len = whole[from..]
            .find(|c: char| !c.is_alphabetic())
            .unwrap_or(whole.len() - from);
        let word = &whole[from..from + len];
        at = from + len;
        // Правила переноса берём у куска, где слово началось: своего стиля у
        // слова нет, а разметка могла разрезать его посередине.
        let piece = owner(from);
        let Some(Piece::Text { style, .. }) = pieces.get(piece) else {
            continue;
        };
        if style.hyphens_auto != Some(true) {
            continue;
        }
        // Без объявленного языка не переносим ВОВСЕ: образцы слогораздела
        // у каждого языка свои, и угаданный язык рвал бы слова не там.
        // Так же решает и спецификация (`hyphens-auto-001`: без `lang`
        // ни одно слово не разрывается).
        let Some(lang) = style
            .lang
            .as_deref()
            .and_then(|l| {
                let code = l.as_bytes();
                (code.len() >= 2)
                    .then(|| [code[0].to_ascii_lowercase(), code[1].to_ascii_lowercase()])
            })
            .and_then(hypher::Lang::from_iso)
        else {
            continue;
        };
        let mut pos = from;
        let mut first = true;
        for part in hypher::hyphenate(word, lang) {
            if !first {
                cuts.push(pos);
            }
            pos += part.len();
            first = false;
        }
    }
    for cut in cuts.into_iter().rev() {
        let piece = owner(cut);
        let local = cut - map.iter().find(|(_, i)| *i == piece).map_or(0, |(s, _)| *s);
        if let Some(Piece::Text { text, .. }) = pieces.get_mut(piece)
            && local <= text.len()
            && text.is_char_boundary(local)
        {
            text.insert(local, '\u{00ad}');
        }
    }
}

/// `word-break: break-all` и родня: перенос разрешён внутри слова.
///
/// Перенос в GPUI идёт по границам слов, и другого рычага нет. Обходной путь —
/// вставить между символами нулевой пробел: он не рисуется и ширины не имеет,
/// но переносчик считает его законной точкой разрыва. Так длинный
/// нечленораздельный токен (хеш, путь, ссылка) перестаёт распирать колонку.
fn breakable(text: &str, style: &Computed) -> String {
    // `hyphens`: мягкий перенос (U+00AD) — законная точка разрыва, но
    // переносчик строк его таковой не считает. Меняем на нулевой пробел:
    // он и есть разрешение разорвать слово. Оговорка: дефис на месте
    // разрыва не рисуется — своего глифа у переноса в конвейере нет.
    // Умолчание CSS — `manual`: по мягкому переносу рвать МОЖНО. Убирает
    // его только явное `hyphens: none`.
    // Соединитель гроздей (U+034F) невидим и ширины не имеет: его дело —
    // запретить разрыв между соседями. До шейпера он доезжать не должен —
    // портит метрику последней строки
    // (`line-break-anywhere-overrides-uax-behavior-011`: коробка ниже на шесть
    // точек). Но СТИРАТЬ его нельзя: класс знака (GL по UAX-14) читается
    // позже — `glue_atoms` смотрит на последний знак куска и по нему решает,
    // клеить ли кусок с атомарной коробкой (css-text-3 §5.1). После стирания
    // куску `A\u{034f}` оставалось `A`, склейки не было, и ряд рвал слово
    // (`line-breaking-atomic-016/017`). Словосоединитель U+2060 такой же
    // невидимый и нулевой, класс WJ запрещает перенос с обеих сторон, а
    // метрику не портит: замерено на всех трёх зелёных парах корпуса с U+034F
    // (`-011` 0.04 → 0.04, `-012` 0.05 → 0.05, `zwnj-renders-invisible`
    // 0.00 → 0.00).
    let text = &text.replace('\u{034f}', "\u{2060}");
    // Принудительные разрывы Юникода — подача страницы, вертикальная
    // табуляция, NEL, разделители строки и абзаца. Набору их отдавать нельзя:
    // строка с ними шейпится в НУЛЕВУЮ ширину, и весь абзац пропадает
    // (`line-breaking-022`). Заменяются переводом строки — он и значит
    // «строка кончилась».
    let text = &text.replace(
        ['\u{000b}', '\u{000c}', '\u{0085}', '\u{2028}', '\u{2029}'],
        "\n",
    );
    // Мягкий перенос остаётся СВОИМ знаком, когда абзац считает раскладку сам:
    // переносчик UAX-14 знает его как точку разрыва, а на месте разрыва
    // рисуется знак переноса (`hyphenate-character`). Замена нулевым пробелом
    // нужна только ЧУЖОМУ набору — он о мягком переносе не знает.
    let owned = if style.hyphenate == Some(false) {
        text.replace('\u{00ad}', "")
    } else if crate::lines::rules(style).is_some() {
        text.to_string()
    } else {
        text.replace('\u{00ad}', "\u{200b}")
    };
    // Дальше идут подсказки переносчику GPUI. Своя строчная раскладка те же
    // правила считает сама и по НАСТОЯЩЕМУ тексту, поэтому подсказки ей
    // только мешают: невидимый знак становится лишней точкой разрыва и
    // сдвигает границы прогонов.
    if crate::lines::rules(style).is_some() {
        return owned;
    }
    // `white-space: break-spaces`: после КАЖДОГО сохранённого пробела есть
    // точка разрыва, а сам пробел остаётся в строке и занимает место. Нулевой
    // пробел сразу за ним и означает ровно это: разрыв разрешён здесь, и
    // предыдущий пробел уходит в измеряемую часть строки, а не свисает.
    let owned = if style.break_after_spaces == Some(true) {
        owned.replace(' ', " \u{200b}")
    } else {
        owned
    };
    // `word-break: keep-all`: иероглифы переносятся только по пробелам. Между
    // знаками ставится словосоединитель — он и означает «здесь не рвать», и
    // переносчик его уважает (класс WJ по UAX-14).
    if style.keep_all == Some(true) {
        let mut out = String::with_capacity(owned.len() * 2);
        let mut prev: Option<char> = None;
        for ch in owned.chars() {
            if let Some(p) = prev
                && !p.is_whitespace()
                && !ch.is_whitespace()
            {
                out.push('\u{2060}');
            }
            out.push(ch);
            prev = Some(ch);
        }
        return out;
    }
    if style.break_anywhere != Some(true) {
        return owned;
    }
    let text = owned.as_str();
    // `line-break: anywhere` рвёт где угодно, в том числе рядом с пробелом;
    // `word-break: break-all` — только внутри слова, поэтому перед пробелом
    // точку разрыва не ставит.
    let anywhere = style.break_anywhere_strict == Some(true);
    let mut out = String::with_capacity(text.len() * 2);
    for (i, ch) in text.chars().enumerate() {
        if i > 0 && (anywhere || !ch.is_whitespace()) {
            out.push('\u{200b}');
        }
        out.push(ch);
    }
    out
}

/// `word-space-transform: ideographic-space`.
///
/// Пробел нулевой ширины между двумя иероглифами становится идеографическим:
/// у него появляется ширина, и разметка без настоящих пробелов набирается так
/// же, как с ними. Только МЕЖДУ иероглифами — у края куска преобразования нет.
/// Правила переноса ПО КУСКАМ: отрезок байт готового текста → своё правило.
///
/// `word-break` и `overflow-wrap`, заданные на вложенном `<span>`, действуют
/// только на его знаки. Пока правило собиралось одно на абзац, всё заданное
/// внутри пропадало целиком.
pub fn wrap_spans(
    pieces: &[Piece],
    base: &Computed,
) -> Vec<(std::ops::Range<usize>, crate::lines::Wrap)> {
    // Правила абзаца целиком — с ними сравнивается каждый кусок: в список
    // попадает только тот, у кого они ДРУГИЕ. Без сравнения `<span>` со своим
    // `white-space` внутри `pre` неотличим от родителя, и перенос ему
    // запрещён вместе со всем абзацем.
    let whole = crate::lines::wrap_of(base);
    let mut out = Vec::new();
    let mut at = 0usize;
    for p in pieces {
        let Piece::Text { text, style } = p else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        let end = at + text.len();
        let w = crate::lines::wrap_of(style);
        if w != whole {
            out.push((at..end, w));
        }
        at = end;
    }
    out
}

/// Относительный сдвиг ПО КУСКАМ: отрезок байт → смещение отрисовки.
///
/// Отдельно от `shift_spans`: тот растит строчную коробку, а относительный
/// сдвиг на поток не влияет вовсе (CSS 2.1 §9.4.3).
pub fn rel_spans(pieces: &[Piece]) -> Vec<(std::ops::Range<usize>, (f32, f32))> {
    let mut out = Vec::new();
    let mut at = 0usize;
    for p in pieces {
        let Piece::Text { text, style } = p else {
            continue;
        };
        let end = at + text.len();
        if let Some(d) = style.rel_shift
            && d != (0.0, 0.0)
        {
            out.push((at..end, d));
        }
        at = end;
    }
    out
}

/// Сдвиг ПО КУСКАМ: отрезок байт → смещение базовой линии в точках.
///
/// `vertical-align: super`/`sub` поднимает и опускает кусок внутри строки.
/// Доля кегля взята браузерная: треть вверх и пятая часть вниз.
// ★ ЗАМЕРЕНО И ОТКАЧЕНО (11.09, `scout-centralbaseline-2026-09.md`,
// 4 хунка): центральная доминантная базовая линия у повёрнутой строки
// (css-writing-modes-4 §4.2: «the central baseline is used as the
// dominant baseline when text-orientation is mixed or upright»,
// величина по css-inline-3 A.2 = (ascent − descent)/2; Blink —
// `computed_style.cc:2154`). Патч включал разделение `mixed` и
// `sideways`, которые у нас хранятся одним булем.
// Срез 1476 пар, база тем же списком: **+0 / −3**. Обещанные
// `text-baseline-vrl-002` и `-vlr-003` НЕ позеленели, а ушли
// `vertical-alignment-003` 0.39 → 0.61, `-009` 0.39 → 0.55,
// `-vlr-025` 0.17 → 1.17. Возвращать только вместе с разбором того,
// почему поправка не даёт нуля там, где арифметика скаута его даёт.
pub fn shift_spans(
    pieces: &[Piece],
    base_size: f32,
    line_px: f32,
) -> Vec<(std::ops::Range<usize>, gpui::Pixels)> {
    let mut out = Vec::new();
    let mut at = 0usize;
    for p in pieces {
        let Piece::Text { text, style } = p else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        let end = at + text.len();
        let size = match style.font_size {
            Some(Len::Px(v)) => v,
            _ => base_size,
        };
        // К вне-поточной коробке выравнивание в строке не применяется
        // (CSS 2.1 §9.5, §10.8): она из строки вынута.
        let out_of_flow = style.float.is_some_and(|f| f != 0)
            || matches!(
                style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            );
        let dy = (!out_of_flow)
            .then(|| style.vertical_shift_px)
            .flatten()
            .or_else(|| {
                // Единица шрифта разрешается ЗДЕСЬ: кегль и гарнитура куска
                // уже известны.
                style.vertical_shift_len.map(|l| {
                    let family = style.font_family.clone().unwrap_or_default();
                    -crate::metrics::spacing_px(Some(l), &family, size)
                })
            })
            .or_else(|| style.vertical_shift.map(|k| k * size))
            // Процент считается от `line-height` САМОГО куска (§10.8.1), а не
            // от кегля: `vertical-align: 50%` при `line-height: 2` — это кегль
            // целиком, а не половина. Замерено: 0 и 0 — правка по спеке, в
            // своде такой записи почти нет.
            .or_else(|| {
                style.vertical_shift_pct.map(|k| {
                    let own = match style.line_height {
                        Some(Len::Px(v)) => v,
                        Some(Len::Pct(f)) | Some(Len::Em(f)) => f * size,
                        _ => line_px,
                    };
                    k * own
                })
            })
            .or_else(|| {
                // `text-top`/`text-bottom` равняют край куска по краю
                // ТЕКСТОВОЙ области родителя (CSS 2.1 §10.8.1). Разница
                // берётся из метрик обоих кеглей: подъём и спуск шрифта —
                // доли кегля, поэтому величина пропорциональна их разности.
                // Коробка куска — это глифы ПЛЮС полулидинг с каждой
                // стороны (CSS 2.1 §10.8.1), а у родителя берётся текстовая
                // область без лидинга. Подъём и спуск — доли кегля.
                const ASCENT: f32 = 0.8;
                const DESCENT: f32 = 0.2;
                let parent = style.vertical_align_base.unwrap_or(base_size);
                // Полулидинг БЫВАЕТ отрицательным: `L = line-height − AD`
                // (§10.8.1), и при `line-height` меньше кегля коробка куска
                // выше строки. Зажим нулём убивал ровно этот случай.
                let half = (line_px - size) / 2.0;
                style.vertical_align_text.map(|top| {
                    if top {
                        (ASCENT * size + half) - ASCENT * parent
                    } else {
                        DESCENT * parent - (DESCENT * size + half)
                    }
                })
            })
            // Запрет вне-поточной коробке — на ВСЮ цепочку, а не на её голову:
            // `(!out_of_flow).then(..)` гасил только сдвиг длиной в точках, а
            // `sub`/`super`, `em`, `ex`, процент и `text-top`/`text-bottom`
            // проходили дальше по `or_else`. Абсолютный кусок блокифицирован
            // (CSS 2.1 §9.7), `vertical-align` к нему не применяется вовсе.
            // `vertical-align-sub-001`: зелёный кусок уезжал вниз на 0.2em и
            // открывал красный. `super-001` был зелёным случайно: подъём
            // гасила верхняя надбавка строки (`line_padding`).
            .filter(|_| !out_of_flow);
        if let Some(v) = dy {
            out.push((at..end, gpui::px(v)));
        }
        at = end;
    }
    out
}

/// Высота строки ПО КУСКАМ: отрезок байт → своя `line-height` в точках.
///
/// §10.8: высоту строчной коробки задают куски, а не блок — у каждого своя
/// коробка отступа высотой `line-height`, и полулидинг считается от НЕЁ.
/// Отдаётся только кусок со СВОИМ значением: унаследованное блок уже учёл.
pub fn line_height_spans(
    pieces: &[Piece],
    inherited: &Computed,
    base_size: f32,
    normal: f32,
) -> Vec<(std::ops::Range<usize>, gpui::Pixels)> {
    let mut out = Vec::new();
    let mut at = 0usize;
    for p in pieces {
        let Piece::Text { text, style } = p else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        let end = at + text.len();
        if style.line_height != inherited.line_height
            && let Some(lh) = style.line_height
        {
            let size = match style.font_size {
                Some(Len::Px(v)) => v,
                _ => base_size,
            };
            let px_of = match lh {
                Len::Px(v) => Some(v),
                Len::Pct(k) => Some(k * size),
                Len::Em(k) => Some(k * size),
                _ => None,
            };
            match px_of {
                Some(v) => out.push((at..end, gpui::px(v))),
                None => out.push((at..end, gpui::px(normal * size))),
            }
        }
        at = end;
    }
    out
}

/// Трекинг ПО КУСКАМ: отрезок байт → своё значение `letter-spacing`.
pub fn letter_spans(
    pieces: &[Piece],
    base_size: f32,
) -> Vec<(std::ops::Range<usize>, gpui::Pixels)> {
    let mut out = Vec::new();
    let mut zero = Vec::new();
    let mut at = 0usize;
    for p in pieces {
        let Piece::Text { text, style } = p else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        let end = at + text.len();
        let size = match style.font_size {
            Some(Len::Px(v)) => v,
            _ => base_size,
        };
        let extra = style.letter_spacing.map(|len| {
            crate::metrics::spacing_px(
                Some(len),
                &style.font_family.clone().unwrap_or_default(),
                size,
            )
        });
        if let Some(v) = extra {
            // Знак нулевой ширины единицей письма не является, и трекинг за
            // ним не идёт (css-text-3 §8.2: интервал ставится МЕЖДУ
            // единицами). Пока шёл, строка с невидимыми знаками разъезжалась
            // на их число (`letter-spacing-control-chars-001`).
            //
            // Кусок из ОДНОГО такого знака — наша распорка (`spacer_style`):
            // в её трекинге лежит поле строчной коробки или зазор, и снимать
            // его нельзя.
            if text.chars().nth(1).is_some() {
                for (off, ch) in text.char_indices().filter(|(_, c)| zero_width_format(*c)) {
                    zero.push((at + off..at + off + ch.len_utf8(), gpui::px(0.)));
                }
            }
            out.push((at..end, gpui::px(v)));
        }
        // Зазор на ГРАНИЦЕ элементов: он принадлежит не куску, а ближайшему
        // общему предку обоих знаков (css-text-3 §8.2). Ставится на последний
        // знак куска и обязан перебить его собственный трекинг, поэтому идёт
        // впереди — поиск диапазона берёт первое попадание.
        if let Some(len) = style.letter_spacing_after
            && let Some(last) = text.char_indices().next_back()
        {
            let v = crate::metrics::spacing_px(
                Some(len),
                &style.font_family.clone().unwrap_or_default(),
                size,
            );
            zero.push((at + last.0..end, gpui::px(v)));
        }
        at = end;
    }
    // Нули идут ПЕРВЫМИ: поиск диапазона берёт первое попадание.
    zero.extend(out);
    zero
}

/// Знак нулевой ширины, управляющий набором, а не письмом: единицей письма он
/// не считается, и межбуквенный интервал вокруг него не ставится.
fn zero_width_format(ch: char) -> bool {
    matches!(
        ch as u32,
        // Нулевой пробел и соединители, знаки направления письма, встраивание
        // и отмена двунаправленности, невидимые знаки математики, устаревшее
        // управление формой арабской вязи, метка порядка байтов.
        0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x2069
            | 0x206A..=0x206F
            | 0xFEFF
    )
}

/// Куски со знаком акцента. Знак набирается в половину кегля своей базы
/// (css-text-decor-3 §5.3, как аннотация руби с `font-size: 50%`) и встаёт
/// на край строчной коробки куска (`line-height` куска, `normal` — доля
/// `normal` шрифта абзаца).
pub fn emphasis_spans(
    pieces: &[Piece],
    base_size: f32,
    normal: f32,
) -> Vec<crate::lines::EmphSpan> {
    let mut out = Vec::new();
    let mut at = 0usize;
    for p in pieces {
        let Piece::Text { text, style } = p else {
            continue;
        };
        let end = at + text.len();
        if let Some(mark) = style.text_emphasis.as_deref().filter(|m| !m.is_empty())
            && text.chars().any(|c| !c.is_whitespace() && c != '\u{feff}')
        {
            let size = match style.font_size {
                Some(Len::Px(v)) => v,
                _ => base_size,
            };
            let line_height = match style.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
                _ => normal * size,
            };
            out.push(crate::lines::EmphSpan {
                range: at..end,
                under: style.emphasis_under,
                size: size * 0.5,
                line_height,
                mark: mark.to_string(),
                color: style.emphasis_color.map(Color::to_hsla),
            });
        }
        at = end;
    }
    out
}

/// Межсловный интервал ПО КУСКАМ: отрезок байт → добавка к каждому пробелу.
///
/// `word-spacing` на вложенном `<span>` действует только на его пробелы. Пока
/// интервал брался один на абзац, заданный внутри пропадал целиком.
pub fn word_spans(pieces: &[Piece], base_size: f32) -> Vec<(std::ops::Range<usize>, gpui::Pixels)> {
    let mut out = Vec::new();
    let mut at = 0usize;
    for p in pieces {
        let Piece::Text { text, style } = p else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        let end = at + text.len();
        let size = match style.font_size {
            Some(Len::Px(v)) => v,
            _ => base_size,
        };
        let extra = style.word_spacing.map(|len| {
            crate::metrics::spacing_px(
                Some(len),
                &style.font_family.clone().unwrap_or_default(),
                size,
            )
        });
        if let Some(v) = extra {
            out.push((at..end, gpui::px(v)));
        }
        at = end;
    }
    out
}

/// Слой знака-распорки: ширину ему даёт трекинг на своём куске, а всё
/// остальное с него снимается — фон и замена пробелов принадлежат тексту.
///
/// Фон снимается ИМЕННО ЗДЕСЬ, а поле красится своей распоркой
/// (`margin_spacer_style`): под полем виден фон предка, под отступом — свой.
/// Первый заход на это без правки набора дал 0 и 0 на 3646 парах: распорка
/// фон получала, но полоса не рисовалась. Корень был в наборе — «default
/// ignorable» U+FEFF выбрасывается целиком, глифов у прогона не остаётся, и
/// цикл краски полосы не идёт ни разу. Чинится в `vendor/gpui`
/// (`line_layout.rs` — ширина по знакам, `line.rs` — квад безглифного
/// прогона), обе пометки «KaminIDE patch».
/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (две правки одной жилы, §8.4 и §10.8):
///
/// 1. Оставить фон НА РАСПОРКЕ отступа, чтобы область отступа строчной
///    коробки красилась её фоном. Полный свод CSS2: 5320 → 5319. Единственная
///    потеря — `word-spacing-characters-001` (0.00 → 5.91): у непустого куска
///    полосу с отступом уже рисует сам прогон (`inline_pad`), и фон распорки
///    ложится ВТОРЫМ слоем поверх той же области.
/// 2. То же, но ТОЛЬКО у пустой строчной коробки (у неё прогона нет вовсе), да
///    ещё со снятым боковым `inline_pad` (иначе полоса вдвое шире). Срез
///    linebox/css1/inline/bidi/word-spacing (1844 пары, 1699 зелёных): 1698.
///    Целевые пары не сдвинулись НИ НА СОТУЮ: `empty-inline-002/003` остались
///    «красное видно», `c42-ibx-pad-000` 2.19, `inlines-017` 3.00,
///    `border-padding-bleed-003` «красное видно». Та же
///    `word-spacing-characters-001` 0.00 → 4.63.
///
/// Почему целевые не двигаются: в них пустой `<span>` — ЕДИНСТВЕННОЕ
/// содержимое блока, а абзац без текста у нас не строится вовсе (проба:
/// `<div><span style="padding:100px;border:25px;background:green"></span></div>`
/// не рисует ничего, тогда как с текстом по бокам отступ красится). Значит
/// начинать надо с абзаца из одной пустой строчной коробки, а не с распорки.
///
/// Ещё одна замеренная и откаченная мелочь рядом: надбавка высоты строки от
/// ПУСТОЙ строчной коробки отрезком нулевой длины (`line_height_spans` +
/// `line_padding`) — срез linebox 613 без изменений, 0 и 0.
fn spacer_style(merged: &Computed, advance: f32) -> Computed {
    let mut style = merged.clone();
    style.letter_spacing = Some(Len::Px(advance));
    style.word_spacing = None;
    style.word_space_char = None;
    style.inline_bg = None;
    style.inline_border = None;
    style
}

/// Слой распорки ПОЛЯ строчной коробки: своего фона у поля нет, сквозь него
/// виден фон предка (§8.3 «margin properties … are always transparent»).
/// Рамку распорке поля не даём: полосу с рамкой уже мерили дважды, обе потери
/// в `bidi-*` (см. запись у `uniform_border`).
fn margin_spacer_style(merged: &Computed, inherited: &Computed, advance: f32) -> Computed {
    let mut style = spacer_style(merged, advance);
    style.inline_bg = inherited.inline_bg;
    style
}

/// Ровная рамка строчной коробки: одинаковые цвет и толщина у всех граней.
///
/// Только такую умеет нарисовать прогон текста. Разные грани оставляем
/// коробке в раскладке — там они честные, но текст в ней не переносится
/// вместе с абзацем.
/// Рамка с РАЗНЫМИ гранями, выражаемая прогоном: все заданные стороны в
/// точках и ЕДИНЫЙ цвет (или цвет текста). Возврат — [верх, право, низ,
/// лево]; незаданные стороны нулевые.
/// Толщина грани строчной рамки в точках.
///
/// Шрифтовые единицы разрешаются по СВОЕМУ кеглю — тем же `spacing_px`, каким
/// уже считаются боковые поля и отступы строчной коробки (`inline_sides`).
/// Прежде сюда пускались только `Len::Px`, и `border-left: 0.2em` — самая
/// обычная запись — уводила `<span>` из прогона текста в настоящую коробку:
/// она садится в строку атомом и растит строку на спуск шрифта. По
/// css-backgrounds-3 §4.1 `<line-width>` — это `<length [0,∞]>` любых единиц,
/// сужения до точек спека не даёт.
fn border_px(l: Option<Len>, c: &Computed, font_px: f32) -> Option<f32> {
    match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(v),
        Some(u @ (Len::Em(_) | Len::Ch(_) | Len::Ex(_))) => {
            // Кегль берётся у ВЫЗЫВАЮЩЕГО: сюда приходит собственный стиль
            // элемента, а `font-size` у `<span>` чаще всего не объявлен —
            // единицы считаются по слитому кеглю строки.
            let size = match c.font_size {
                Some(Len::Px(v)) => v,
                _ => font_px,
            };
            let family = c.font_family.clone().unwrap_or_default();
            Some(crate::metrics::spacing_px(Some(u), &family, size))
        }
        _ => None,
    }
}

pub fn sided_border(c: &Computed, font_px: f32) -> Option<(Color, [f32; 4])> {
    let w = c.borders();
    let px_of = |l: Option<Len>| border_px(l, c, font_px);
    let sides = [
        px_of(w.top)?,
        px_of(w.right)?,
        px_of(w.bottom)?,
        px_of(w.left)?,
    ];
    if !sides.iter().any(|v| *v > 0.0) {
        return None;
    }
    // Цвет сверяется только по ЗАДАННЫМ сторонам: у частичной рамки
    // остальных цветов просто нет.
    let mut color: Option<Color> = None;
    for i in 0..4 {
        if sides[i] <= 0.0 {
            continue;
        }
        // Цвет рамки по умолчанию — цвет текста, а он на строчном куске
        // часто не задан вовсе: без запасного чёрного прогон отказывался от
        // рамки, и кусок уходил в коробку, двигая текст на её ширину. То же
        // умолчание уже стоит на блочном пути (`apply.rs`).
        //
        // ЗАМЕРЕНО ОТДЕЛЬНО И ОТКАЧЕНО ДВАЖДЫ: включить в `vendor/gpui` полосу
        // прогона для рамки БЕЗ фона (сейчас квад заводится только при
        // заданной подсветке, и рамка строчной коробки не рисуется вовсе).
        // Первый замер 19/17, второй — вместе со сверкой `background_border`
        // при слиянии прогонов (`text_system.rs`) — 0 приобретено, 17
        // потеряно, все потери в семьях `bidi-*`. Разбор в
        // `target/scout-band.md`: полоса открывается и закрывается внутри
        // КАЖДОГО визуального прогона, поэтому на стыке двунаправленности и
        // на переносе рисуются обе боковые грани. Возвращать вместе с
        // признаками «полоса продолжается» у `pad_left`/`pad_right`.
        let side = c.border_colors[i]
            .or(c.border_color)
            .or(c.color)
            .unwrap_or(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            });
        match color {
            None => color = Some(side),
            Some(prev) if prev == side => {}
            _ => return None,
        }
    }
    Some((color?, sides))
}

pub fn uniform_border(c: &Computed, font_px: f32) -> Option<(Color, f32)> {
    let w = c.borders();
    // Здесь незаданная грань — НЕ ноль: ровной рамке нужны все четыре, и
    // `None` обязан рушить сведение (иначе `border-left` в одиночку сошёл бы
    // за ровную рамку по всем сторонам).
    let px_of = |l: Option<Len>| l.and_then(|v| border_px(Some(v), c, font_px));
    let (t, r, b, l) = (
        px_of(w.top)?,
        px_of(w.right)?,
        px_of(w.bottom)?,
        px_of(w.left)?,
    );
    if t <= 0.0 || t != r || t != b || t != l {
        return None;
    }
    let sides = &c.border_colors;
    let color = match (c.border_color, sides[0], sides[1], sides[2], sides[3]) {
        (_, Some(a), Some(b2), Some(c2), Some(d)) if a == b2 && a == c2 && a == d => a,
        (Some(one), None, None, None, None) => one,
        // Цвет не задан вовсе — рамка красится цветом текста, а без него
        // чёрным (то же умолчание, что у блочного пути).
        (None, None, None, None, None) => c.color.unwrap_or(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }),
        _ => return None,
    };
    Some((color, t))
}

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (08.09, v158→v162, `scout-trimedge-2026-09.md` Х1):
/// узкий гейт «атом обрывает ВЕДУЩИЙ срез, если за рядом пробелов текст»
/// (`trim_edge` на срезе + `text_after_spaces`). Обещание 0/0 с восьмёркой
/// контроля — восьмёрка устояла, но `baseline-inline-non-replaced-004`
/// 0.40 → «красное видно»; без гейта (v162) снова 0.40. Плюсов ноль. Пробел
/// за атомом остаётся прозрачным; эталоны `text-emphasis` по-прежнему ждут
/// другой раскладки — искать её в `as_wrapped_row`, а не в срезе краёв.
/// Пробелы по КРАЯМ строки, сквозь ПУСТУЮ строчную коробку.
///
/// По css-text-3 §4.1.3 схлопываемые пробелы в конце строки удаляются. Пустая
/// строчная коробка (`<span style="border-left:30px solid green"></span>`)
/// ряд пробелов не разрывает: пробелы по обе её стороны — один ряд, он
/// последний в строке и потому исчезает весь. Наш абзац к этому моменту уже
/// разложен на куски, и коробка стоит между ними отдельным куском — поэтому
/// проход идёт с конца и коробки пропускает
/// (`line-edge-white-space-collapse-001` и `-002`: иначе у рамки оставался
/// лишний пробел и из-под неё выглядывало красное).
pub fn trim_edge_spaces(pieces: &mut [Piece]) {
    trim_edge(pieces.iter_mut().rev(), false, false);
    trim_edge(pieces.iter_mut(), true, false);
}

/// То же для абзаца, чьи атомы встают В СТРОКУ (`Paragraph::atoms`): там атом
/// — содержимое строки (CSS 2.1 §9.2.2), и ряд пробелов ЗА ним уже не на краю.
/// Прозрачный атом срезал пробелы между атомами: `<img> <img>` слипались.
pub fn trim_edge_spaces_solid_atoms(pieces: &mut [Piece]) {
    trim_edge(pieces.iter_mut().rev(), false, true);
    trim_edge(pieces.iter_mut(), true, true);
}

/// Один край строки: куски идут от него внутрь, коробки пропускаются.
fn trim_edge<'a>(pieces: impl Iterator<Item = &'a mut Piece>, leading: bool, solid_atoms: bool) {
    for piece in pieces {
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (07.09, v155, `scout-emphasis-2026-09.md`):
    // отрисовка `text-emphasis` (11 хунков) вместе с правкой `trim_edge`
    // (`Piece::Atom` перестаёт быть прозрачным для среза краевого пробела).
    // Срез css-text-decor+css-pseudo+css-lists+css-counter-styles+css-ruby+
    // css-text+css-inline+CSS2 8145: +4/−8 у этой части —
    // `inline-block-baseline-015/016` 0.00 → 99.00,
    // `vertical-align-117a/118a` 0.11 → 6.84, `inline-formatting-context-013`,
    // `line-breaking-030/032`, `inline-block-replaced-width-003`.
    // Срез краевого пробела после атома трогает всю строчную раскладку —
    // мерить отдельно и сначала только его.
        match piece {
            Piece::Atom(_) if solid_atoms => return,
            // Коробка без текста для ряда пробелов прозрачна.
            Piece::Atom(_) | Piece::Overlay(..) => continue,
            // Распорка полей строчной коробки и метка атома прозрачны так же:
            // место они занимают, содержимым строки не являются, и пробел за
            // ними по-прежнему стоит на КРАЮ строки (css-text-3 §4.1.3).
            // Прежде первая же распорка обрывала проход, и ведущий пробел
            // после `<span style="padding-left:1em">` не срезался никогда.
            Piece::Text { text, .. } if text == SPACER || text == ZWSP => continue,
            Piece::Text { text, style } => {
                // `white-space: pre*` пробелы бережёт — там удалять нечего.
                if style.keep_spaces == Some(true) {
                    return;
                }
                // Пустой кусок ряда не обрывает: он и есть схлопнутый пробел,
                // а настоящий край строки лежит дальше внутрь.
                if text.is_empty() {
                    continue;
                }
                let trimmed = if leading {
                    text.trim_start_matches(' ')
                } else {
                    text.trim_end_matches(' ')
                };
                if trimmed.len() == text.len() {
                    // Кусок упирается не в пробел: ряд оборвался, дальше не идём.
                    return;
                }
                let rest = trimmed.to_string();
                let empty = rest.is_empty();
                *text = rest;
                if !empty {
                    return;
                }
                // Кусок был из одних пробелов — ряд продолжается левее.
            }
        }
    }
}

/// Зазоры `text-autospace` — диапазонами трекинга по тексту абзаца.
///
/// По css-text-4 §7 между иероглифом и соседней буквой (`ideograph-alpha`) или
/// цифрой (`ideograph-numeric`) стоит зазор в 1/8 кегля. Соседство считается
/// по ЗНАКАМ, а не по кускам: пара лежит и внутри одного текстового узла
/// (`国国XX国`), и по разные стороны границы (`<b>永</b>abc`).
///
/// Зазор — трекинг на знаке ПЕРЕД границей, и ставится он диапазоном, не
/// разрезая кусок. Два тупика, из которых это единственный выход:
///
/// * знаком-распоркой зазор сделать нельзя: любая распорка нулевой ширины
///   (U+FEFF, U+2060) имеет класс переноса WJ и запрещает разрыв по обе
///   стороны, а зазор на перенос влиять не должен;
/// * резать кусок ради своего трекинга тоже нельзя: соседние прогоны кладутся
///   с независимым округлением, и между половинками слова появлялся шов
///   в точку (`text-autospace-001`, две буквы `XX` расходились).
///
/// Трекинг вдобавок схлопывается на краю строки сам — как и требует
/// спецификация: строка рвётся по границе зазора, и на новой строке зазора
/// уже нет.
pub fn autospace_spans(
    pieces: &[Piece],
    base_size: f32,
) -> Vec<(std::ops::Range<usize>, gpui::Pixels)> {
    let mut out = Vec::new();
    // Знак перед курсором: его смещение, длина, стиль и кегль куска.
    let mut prev: Option<(usize, usize, char, f32, &Computed)> = None;
    let mut at = 0usize;
    for p in pieces {
        let Piece::Text { text, style } = p else {
            // Картинка иероглифом не бывает и соседство разрывает.
            prev = None;
            continue;
        };
        let size = match style.font_size {
            Some(Len::Px(v)) => v,
            _ => base_size,
        };
        for (off, ch) in text.char_indices() {
            // Соединительный знак (огласовка, диакритика) письменности не
            // меняет: он прозрачен, а класс пары берётся у БАЗОВОЙ буквы.
            // Пока знак считался обычным, арабское слово с огласовкой на
            // конце теряло зазор перед иероглифом (`text-autospace-mixed-001`,
            // `text-autospace-combining-marks`).
            if combining(ch) {
                if let Some(p) = prev.as_mut() {
                    // Место зазора — после ВСЕГО сочетания, поэтому отрезок
                    // переезжает на знак, а буква для решения прежняя.
                    p.0 = at + off;
                    p.1 = ch.len_utf8();
                }
                continue;
            }
            if let Some((p_at, p_len, p_ch, p_size, p_style)) = prev
                && autospace_between(p_ch, ch, style)
            {
                let family = p_style.font_family.clone().unwrap_or_default();
                let had = crate::metrics::spacing_px(p_style.letter_spacing, &family, p_size);
                // ЗАМЕРЕНО И ОТКАЧЕНО: считать зазор от `ic`, а не от кегля
                // (css-text-4 §7.1 «1/8 of the ideographic advance»). Полный
                // свод CSS3: приобретено 0, потеряно 1 —
                // `text-autospace-supplementary-ideograph` 0.08 -> 0.59.
                // Пары `text-autospace-elements-005/005b` (0.56) не сдвинулись.
                out.push((p_at..p_at + p_len, gpui::px(had + p_size / 8.0)));
            }
            prev = Some((at + off, ch.len_utf8(), ch, size, style));
        }
        at += text.len();
    }
    out
}

/// Нужен ли зазор между двумя соседними знаками.
fn autospace_between(left: char, right: char, style: &Computed) -> bool {
    let alpha = style.autospace_alpha.unwrap_or(false);
    let numeric = style.autospace_numeric.unwrap_or(false);
    let pair = |ideo: char, other: char| {
        ideographic(ideo)
            && ((alpha && other.is_alphabetic() && !ideographic(other))
                || (numeric && other.is_ascii_digit()))
    };
    pair(left, right) || pair(right, left)
}

/// Соединительный ли знак: своей ширины нет, письменность задаёт базовая
/// буква перед ним.
fn combining(ch: char) -> bool {
    matches!(
        unicode_linebreak::break_property(ch as u32),
        unicode_linebreak::BreakClass::CombiningMark
    )
}

/// Иероглиф ли знак — по классу переноса строк.
fn ideographic(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        Ideographic | ConditionalJapaneseStarter | NonStarter
    )
}

/// Пробелы куска набираются U+3000 (`text-transform: full-width`).
fn full_width_spaces(style: &Computed) -> bool {
    style.text_transform_flags & crate::computed::TT_FULL_WIDTH != 0
}

/// `word-space-transform` по КУСКАМ абзаца.
///
/// Соседи точки переноса сплошь и рядом лежат в других кусках: `あ<wbr>い` —
/// это три куска, а `<span>` с полем или рамкой делит абзац и вовсе на
/// отдельные элементы. Замена знак-в-знак: нулевой пробел и идеографический
/// занимают в UTF-8 одинаково, поэтому прогоны не съезжают.
pub fn space_transform_pieces(pieces: &mut [Piece]) {
    let mut seq: Vec<(usize, usize, char)> = vec![];
    for (i, piece) in pieces.iter().enumerate() {
        match piece {
            Piece::Text { text, style } => {
                // `text-transform: full-width` уже превратил пробел куска в
                // U+3000 (регистр меняется при сборе), а по порядку
                // css-text-3 §2.1 он идёт ПОСЛЕ обработки пробелов — для
                // схлопывания это всё ещё пробел (`word-space-transform-009`).
                let wide = full_width_spaces(style);
                for (at, ch) in text.char_indices() {
                    let ch = if wide && ch == '\u{3000}' { ' ' } else { ch };
                    seq.push((i, at, ch));
                }
            }
            // Атомарная коробка иероглифом не является и соседство разрывает.
            _ => seq.push((usize::MAX, 0, '\u{0}')),
        }
    }
    let mut edits: Vec<(usize, usize, char)> = vec![];
    for k in 0..seq.len() {
        let (piece, at, ch) = seq[k];
        if ch != '\u{200b}' || piece == usize::MAX {
            continue;
        }
        let Piece::Text { style, .. } = &pieces[piece] else {
            continue;
        };
        // Значение `space` подставляет ОБЫЧНЫЙ пробел, и оно тоже работает:
        // сравнение шло только с идеографическим, и половина свойства не
        // действовала вовсе.
        let Some(sep) = style.word_space_char.filter(|&c| c != '\0') else {
            continue;
        };
        // `space` — разделитель слов ЛЮБОЙ письменности (css-text-4
        // §word-space-transform: «word separators … are replaced with
        // U+0020»): латинское `aa<wbr>bb` тоже получает пробел
        // (`word-space-transform-014`). Соседи-иероглифы нужны только
        // идеографическому.
        let between = k > 0 && k + 1 < seq.len();
        if between && (sep == ' ' || (ideographic(seq[k - 1].2) && ideographic(seq[k + 1].2))) {
            edits.push((piece, at, sep));
            if sep == ' ' {
                seq[k].2 = sep;
            }
        }
    }
    // Подставленный U+0020 — обычный схлопываемый пробел: замена идёт ДО
    // обработки пробелов (css-text-4 §word-space-transform), и в
    // `i <wbr> &#x200B; j` от серии пробелов остаётся один
    // (`word-space-transform-007`). Схлопываем только серии, где есть
    // подставленный пробел: прочие уже свёрнуты сбором текста.
    let collapsible = |i: usize| {
        i != usize::MAX
            && matches!(&pieces[i], Piece::Text { style, .. } if style.keep_spaces != Some(true))
    };
    let mut k = 0;
    while k < seq.len() {
        if seq[k].2 != ' ' || !collapsible(seq[k].0) {
            k += 1;
            continue;
        }
        let mut end = k + 1;
        while end < seq.len() && seq[end].2 == ' ' && collapsible(seq[end].0) {
            end += 1;
        }
        let transformed = |j: usize| edits.iter().any(|e| e.0 == seq[j].0 && e.1 == seq[j].1);
        if (k..end).any(transformed) {
            for j in k + 1..end {
                let (piece, at, _) = seq[j];
                match edits.iter_mut().find(|e| e.0 == piece && e.1 == at) {
                    Some(e) => e.2 = '\0',
                    None => edits.push((piece, at, '\0')),
                }
            }
        }
        k = end;
    }
    // С конца: обычный пробел короче нулевого (1 байт против 3), и правка
    // впереди сдвигала бы смещения следующих правок того же куска.
    edits.sort_by(|a, b| (b.0, b.1).cmp(&(a.0, a.1)));
    for (piece, at, sep) in edits {
        if let Piece::Text { text, style } = &mut pieces[piece] {
            let Some(old) = text[at..].chars().next() else {
                continue;
            };
            let sep = if sep == ' ' && full_width_spaces(style) {
                '\u{3000}'
            } else {
                sep
            };
            let mut buf = [0u8; 4];
            let new = if sep == '\0' { "" } else { &*sep.encode_utf8(&mut buf) };
            text.replace_range(at..at + old.len_utf8(), new);
        }
    }
}

/// Титульный регистр знака — там, где он ОТЛИЧАЕТСЯ от прописного.
///
/// Таких мест в Юникоде немного: составные буквы, у которых прописной вариант
/// пишется двумя большими (`ǄǅǆЛЈ…`), и греческие с приданной йотой, где
/// полное прописное отображение даёт ДВА знака. `None` — отличий нет, годится
/// обычное `to_uppercase`.
fn titlecase(ch: char) -> Option<char> {
    let c = ch as u32;
    let title = match c {
        0x01C4..=0x01C6 => 0x01C5,
        0x01C7..=0x01C9 => 0x01C8,
        0x01CA..=0x01CC => 0x01CB,
        0x01F1..=0x01F3 => 0x01F2,
        // Приданная йота: заглавная форма стоит ровно на восемь позиций выше.
        0x1F80..=0x1F87 | 0x1F90..=0x1F97 | 0x1FA0..=0x1FA7 => c + 8,
        0x1FB3 => 0x1FBC,
        0x1FC3 => 0x1FCC,
        0x1FF3 => 0x1FFC,
        _ => return None,
    };
    char::from_u32(title)
}

/// `text-transform`: регистр меняется до шейпинга — шрифт про него не знает.
pub fn transform_case(text: &str, style: &Computed) -> String {
    let flags = style.text_transform_flags;
    let cased = transform_case_only(text, style);
    if flags == 0 {
        return cased;
    }
    // Порядок css-text-3 §2.1: регистр, затем `full-width`, затем
    // `full-size-kana`. `math-auto` — только у текста из ОДНОГО знака
    // (MathML Core §2.1.5 «If the text consists of a single character»).
    let single = {
        let mut it = cased.trim().chars();
        it.next().is_some() && it.next().is_none()
    };
    cased
        .chars()
        .map(|ch| {
            let mut ch = ch;
            if flags & crate::computed::TT_FULL_WIDTH != 0 {
                ch = full_width(ch);
            }
            if flags & crate::computed::TT_KANA != 0 {
                ch = full_size_kana(ch);
            }
            if flags & crate::computed::TT_MATH != 0 && single {
                ch = math_italic(ch);
            }
            ch
        })
        .collect()
}

/// Полноширинный двойник знака (css-text-3 §2.1 `full-width`: знаки,
/// у которых есть «fullwidth» форма по UAX #11, и полуширинные формы,
/// раскрытые обратно — как ICU `Halfwidth-Fullwidth`).
fn full_width(ch: char) -> char {
    let c = ch as u32;
    let m = match c {
        0x20 => 0x3000,
        0x21..=0x7E => c + 0xFEE0,
        0x2985 => 0xFF5F,
        0x2986 => 0xFF60,
        0xA2 => 0xFFE0,
        0xA3 => 0xFFE1,
        0xAC => 0xFFE2,
        0xAF => 0xFFE3,
        0xA6 => 0xFFE4,
        0xA5 => 0xFFE5,
        0x20A9 => 0xFFE6,
        0xFF61..=0xFF9F => HALF_KATAKANA[(c - 0xFF61) as usize] as u32,
        0xFFA0 => 0x3164,
        0xFFA1..=0xFFBE => c - 0xFFA1 + 0x3131,
        0xFFC2..=0xFFC7 => c - 0xFFC2 + 0x314F,
        0xFFCA..=0xFFCF => c - 0xFFCA + 0x3155,
        0xFFD2..=0xFFD7 => c - 0xFFD2 + 0x315B,
        0xFFDA..=0xFFDC => c - 0xFFDA + 0x3161,
        0xFFE8 => 0x2502,
        0xFFE9..=0xFFEC => c - 0xFFE9 + 0x2190,
        0xFFED => 0x25A0,
        0xFFEE => 0x25CB,
        _ => c,
    };
    char::from_u32(m).unwrap_or(ch)
}

/// Полуширинная катакана U+FF61..U+FF9F → полноширинная (UnicodeData,
/// разложение `<narrow>`).
const HALF_KATAKANA: [u16; 63] = [
    0x3002, 0x300C, 0x300D, 0x3001, 0x30FB, 0x30F2, 0x30A1, 0x30A3, 0x30A5, 0x30A7, 0x30A9,
    0x30E3, 0x30E5, 0x30E7, 0x30C3, 0x30FC, 0x30A2, 0x30A4, 0x30A6, 0x30A8, 0x30AA, 0x30AB,
    0x30AD, 0x30AF, 0x30B1, 0x30B3, 0x30B5, 0x30B7, 0x30B9, 0x30BB, 0x30BD, 0x30BF, 0x30C1,
    0x30C4, 0x30C6, 0x30C8, 0x30CA, 0x30CB, 0x30CC, 0x30CD, 0x30CE, 0x30CF, 0x30D2, 0x30D5,
    0x30D8, 0x30DB, 0x30DE, 0x30DF, 0x30E0, 0x30E1, 0x30E2, 0x30E4, 0x30E6, 0x30E8, 0x30E9,
    0x30EA, 0x30EB, 0x30EC, 0x30ED, 0x30EF, 0x30F3, 0x3099, 0x309A,
];

/// Малая кана → полноразмерная (css-text-3 §2.1 `full-size-kana`, таблица
/// «Full-Size Kana Mappings» приложения G).
fn full_size_kana(ch: char) -> char {
    match ch {
        'ぁ' => 'あ',
        'ぃ' => 'い',
        'ぅ' => 'う',
        'ぇ' => 'え',
        'ぉ' => 'お',
        'ゕ' => 'か',
        'ゖ' => 'け',
        'っ' => 'つ',
        'ゃ' => 'や',
        'ゅ' => 'ゆ',
        'ょ' => 'よ',
        'ゎ' => 'わ',
        'ァ' => 'ア',
        'ィ' => 'イ',
        'ゥ' => 'ウ',
        'ェ' => 'エ',
        'ォ' => 'オ',
        'ヵ' => 'カ',
        'ㇰ' => 'ク',
        'ヶ' => 'ケ',
        'ㇱ' => 'シ',
        'ㇲ' => 'ス',
        'ッ' => 'ツ',
        'ㇳ' => 'ト',
        'ㇴ' => 'ヌ',
        'ㇵ' => 'ハ',
        'ㇶ' => 'ヒ',
        'ㇷ' => 'フ',
        'ㇸ' => 'ヘ',
        'ㇹ' => 'ホ',
        'ㇺ' => 'ム',
        'ャ' => 'ヤ',
        'ュ' => 'ユ',
        'ョ' => 'ヨ',
        'ㇻ' => 'ラ',
        'ㇼ' => 'リ',
        'ㇽ' => 'ル',
        'ㇾ' => 'レ',
        'ㇿ' => 'ロ',
        'ヮ' => 'ワ',
        'ｧ' => 'ｱ',
        'ｨ' => 'ｲ',
        'ｩ' => 'ｳ',
        'ｪ' => 'ｴ',
        'ｫ' => 'ｵ',
        'ｯ' => 'ﾂ',
        'ｬ' => 'ﾔ',
        'ｭ' => 'ﾕ',
        'ｮ' => 'ﾖ',
        _ => ch,
    }
}

/// Курсивный математический двойник (MathML Core §2.1.5, «italic mappings»).
fn math_italic(ch: char) -> char {
    let c = ch as u32;
    let m = match c {
        0x68 => 0x210E,
        0x41..=0x5A => 0x1D434 + (c - 0x41),
        0x61..=0x7A => 0x1D44E + (c - 0x61),
        0x131 => 0x1D6A4,
        0x237 => 0x1D6A5,
        0x391..=0x3A1 => 0x1D6E2 + (c - 0x391),
        0x3F4 => 0x1D6F3,
        0x3A3..=0x3A9 => 0x1D6F4 + (c - 0x3A3),
        0x2207 => 0x1D6FB,
        0x3B1..=0x3C9 => 0x1D6FC + (c - 0x3B1),
        0x2202 => 0x1D715,
        0x3F5 => 0x1D716,
        0x3D1 => 0x1D717,
        0x3F0 => 0x1D718,
        0x3D5 => 0x1D719,
        0x3F1 => 0x1D71A,
        0x3D6 => 0x1D71B,
        _ => c,
    };
    char::from_u32(m).unwrap_or(ch)
}

fn transform_case_only(text: &str, style: &Computed) -> String {
    match style.text_transform {
        Some(TextTransform::Upper) => text.to_uppercase(),
        // Полноширинные двойники лежат ровно на 0xFEE0 выше своих знаков
        // ASCII; пробел заменяется отдельным знаком.
        Some(TextTransform::FullWidth) => text
            .chars()
            .map(|ch| match ch as u32 {
                0x20 => '\u{3000}',
                c @ 0x21..=0x7E => char::from_u32(c + 0xFEE0).unwrap_or(ch),
                _ => ch,
            })
            .collect(),
        Some(TextTransform::Lower) => text.to_lowercase(),
        Some(TextTransform::Capitalize) => {
            // Начало слова — первая БУКВА (css-text-3 §2.1: «first typographic
            // letter unit of each word»): открывающая скобка и прочая
            // пунктуация перед ней пропускаются (`(é` → `(É`). Границы слов —
            // по UAX #29: `.`, `'`, `:` между буквами слово НЕ рвут (WB6/WB7,
            // `x.x.` → `X.x.`), прочая пунктуация рвёт (`foo-bar` → `Foo-Bar`).
            // Прежде началом считался только знак после пробела.
            let mut out = String::with_capacity(text.len());
            let mut prev: Option<char> = None;
            let mut prev2: Option<char> = None;
            let mid = |c: char| matches!(c, '.' | '\'' | '\u{2019}' | ':' | '\u{b7}');
            for ch in text.chars() {
                let at_start = ch.is_alphabetic()
                    && match prev {
                        None => true,
                        Some(p) if p.is_alphanumeric() => false,
                        Some(p) if mid(p) => !prev2.is_some_and(char::is_alphanumeric),
                        Some(_) => true,
                    };
                if at_start {
                    // ТИТУЛЬНЫЙ регистр, а не прописной (css-text-3 §2.1).
                    // У диграфов и у греческого с приданной йотой это разные
                    // знаки: `ǆ` даёт `ǅ`, а не `Ǆ`; `ᾀ` даёт `ᾈ`, а не пару
                    // `ἈΙ` (`text-transform-capitalize-007` и `-016`).
                    match titlecase(ch) {
                        Some(title) => out.push(title),
                        None => out.extend(ch.to_uppercase()),
                    }
                } else {
                    out.push(ch);
                }
                prev2 = prev;
                prev = Some(ch);
            }
            out
        }
        _ => text.to_string(),
    }
}

/// Убрать ХВОСТОВОЙ пробельный кусок строки-ряда.
///
/// По CSS пробел в конце строки висит за краем и на раскладку не влияет. В
/// ряду он влияет: это отдельная коробка со своей высотой, и строка растёт на
/// его спуск — между двумя картинками 60×60 появлялась полоса в полкегля
/// (`line-breaking-030`). В сохранённых пробелах (`white-space: pre*`) кусок
/// значим и остаётся.
/// Открывающий знак, после которого перенос запрещён (UAX #14, класс OP;
/// CJK-набор). Такой знак клеится к СЛЕДУЮЩЕМУ содержимому.
fn opening_punct(c: char) -> bool {
    matches!(
        c,
        '「' | '『'
            | '（'
            | '〔'
            | '【'
            | '〈'
            | '《'
            | '〖'
            | '〘'
            | '〚'
            | '｛'
            | '［'
            | '｟'
            | '｢'
    )
}

/// Отрезать от текст-куска перед атомом хвост из открывающих знаков.
///
/// Перенос после открывающей скобки запрещён (UAX #14): когда за текстом
/// идёт строчный атом (`text-combine-upright`, картинка), скобка обязана
/// уйти на строку ВМЕСТЕ с ним. Внутри одного текст-куска это делает
/// перенос строк, но границу куска он не видит — скобка застревала
/// последней строкой текста, а атом падал на следующую
/// (text-combine-upright-line-breaking-rules-001).
fn split_glued_tail(pieces: Vec<Piece>) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::with_capacity(pieces.len());
    let mut it = pieces.into_iter().peekable();
    while let Some(p) = it.next() {
        match p {
            Piece::Text { text, style }
                if matches!(it.peek(), Some(Piece::Atom(_)))
                    && text.chars().next_back().is_some_and(opening_punct) =>
            {
                let head_len = text.trim_end_matches(opening_punct).len();
                let tail = text[head_len..].to_string();
                if head_len > 0 {
                    out.push(Piece::Text {
                        text: text[..head_len].to_string(),
                        style: style.clone(),
                    });
                }
                out.push(Piece::Text { text: tail, style });
            }
            p => out.push(p),
        }
    }
    out
}

fn drop_hanging_tail(mut pieces: Vec<Piece>) -> Vec<Piece> {
    while let Some(Piece::Text { text, style }) = pieces.last() {
        // Схлопываемый пробел по CSS — только `space`, `tab`, `CR`, `LF`.
        // Идеографический U+3000 и неразрывный U+00A0 значимы: `trim()` их
        // тоже снимает, и хвостовая строка из них пропадала целиком
        // (`trailing-ideographic-space-017`).
        let collapsible = |c: char| matches!(c, ' ' | '\t' | '\r' | '\n');
        if style.keep_spaces == Some(true) || !text.chars().all(collapsible) {
            break;
        }
        pieces.pop();
    }
    pieces
}

/// Разрыв строки в ряду из слов.
///
/// Ряд — гибкая строка с переносом, и перевод строки в нём не значит ничего:
/// `<br>` доезжал сюда куском текста `\n` и молча пропадал. Разрыв даёт
/// распорка во всю ширину — следующему ребёнку места в строке уже нет.
/// Разрыв, закрывающий ПУСТУЮ строку (в ней ни слова, ни атома): такая
/// строка не «zero-height» (CSS 2.1 §9.4.2 исключает только строки без
/// текста и без разрыва), её высоту даёт strut — `line-height` блока
/// (§10.8.1). Распорка `h_0` роняла строку `отступ + <br>` в ноль, и квадрат
/// вставал наверх вместо низа (`text-indent-on-blank-line-rtl-left-align`).
fn blank_line_break(style: &Computed) -> AnyElement {
    let size = match style.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    let lh = match style.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
        _ => size * 1.2,
    };
    gpui::div().w_full().h(gpui::px(lh)).flex_shrink_0().into_any_element()
}

fn line_break() -> AnyElement {
    gpui::div().w_full().h_0().into_any_element()
}

/// Схлопывание пробелов, как в HTML: переводы строк и повторы — один пробел.
fn normalize_spaces(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::with_capacity(raw.len());
    let mut at = 0usize;
    while at < chars.len() {
        if !is_collapsible(chars[at]) {
            out.push(chars[at]);
            at += 1;
            continue;
        }
        // Пробельный отрезок целиком: важно, был ли внутри перевод строки и
        // какие знаки стоят по краям.
        let start = at;
        while at < chars.len() && is_collapsible(chars[at]) {
            at += 1;
        }
        let had_break = chars[start..at].iter().any(|c| matches!(*c, '\n' | '\r'));
        // Соседи ряда ищутся СКВОЗЬ знаки управления двунаправленностью
        // (css-text-3 §4.1: «as if they were not there»): ряд по ту сторону
        // RLO/PDF — продолжение прежнего и удаляется целиком (CSS 2.1 §16.6.1
        // шаг 4), `x ␠RLO␠x` — один пробел (`white-space-collapsing-bidi-001`).
        let before = out.chars().rev().find(|c| !bidi_format(*c));
        if before == Some(' ') {
            continue;
        }
        let after = chars[at..].iter().copied().find(|c| !bidi_format(*c));
        // Преобразование перевода строки (CSS Text 3 §4.1.2): между двумя
        // ШИРОКИМИ знаками перевод УДАЛЯЕТСЯ, а не становится пробелом —
        // иначе японский текст, набранный в несколько строк, получает лишние
        // пробелы на каждом переводе.
        // Перевод строки рядом с нулевым пробелом удаляется, нулевой пробел
        // остаётся (css-text-4 §4.1.3 «If the character immediately before or
        // immediately after the segment break is the zero-width space
        // character (U+200B), then the break is removed»).
        // Широкий ЗНАК ПРЕПИНАНИЯ с любой стороны — тоже удаление (Gecko,
        // bug 1935148, `segment-break-transformation-punctuation-001`:
        // «場合、⏎Edge» и «ID⏎｢smith｣» без пробела).
        let drop = had_break
            && ((before.is_some_and(wide_cjk) && after.is_some_and(wide_cjk))
                || before.is_some_and(wide_punct)
                || after.is_some_and(wide_punct)
                || before == Some('\u{200b}')
                || after == Some('\u{200b}'));
        if !drop {
            out.push(' ');
        }
    }
    out
}

/// Широкий знак письма, между которыми перевод строки удаляется.
///
/// Хангыль сюда НЕ входит: по спецификации он пишется через пробелы, и
/// перевод между слогами обязан стать пробелом.
fn wide_cjk(ch: char) -> bool {
    let c = ch as u32;
    let hangul = (0x1100..=0x11FF).contains(&c)
        || (0x3130..=0x318F).contains(&c)
        || (0xA960..=0xA97F).contains(&c)
        || (0xAC00..=0xD7FF).contains(&c);
    if hangul {
        return false;
    }
    (0x2E80..=0x303E).contains(&c)
        || (0x3041..=0x33FF).contains(&c)
        || (0x3400..=0x4DBF).contains(&c)
        || (0x4E00..=0x9FFF).contains(&c)
        || (0xF900..=0xFAFF).contains(&c)
        || (0xFE30..=0xFE4F).contains(&c)
        || (0xFF01..=0xFF60).contains(&c)
        // Полуширинная кана и её знаки (East Asian Width H; полуширинный
        // хангыль U+FFA0.. — Hangul, в счёт не идёт).
        || (0xFF61..=0xFF9F).contains(&c)
        || (0xFFE0..=0xFFE6).contains(&c)
        || (0x20000..=0x3FFFD).contains(&c)
}

/// Широкий (F/W/H) знак препинания письма CJK.
fn wide_punct(ch: char) -> bool {
    let c = ch as u32;
    (0x3000..=0x303F).contains(&c)
        || c == 0x30A0
        || c == 0x30FB
        || (0xFE30..=0xFE4F).contains(&c)
        || (0xFF01..=0xFF0F).contains(&c)
        || (0xFF1A..=0xFF20).contains(&c)
        || (0xFF3B..=0xFF40).contains(&c)
        || (0xFF5B..=0xFF65).contains(&c)
}

/// Табуляция до ближайшей ПОЗИЦИИ табуляции, а не в `tab-size` пробелов.
///
/// По CSS `tab-size: 8` значит, что табуляция доводит строку до ближайшего
/// кратного восьми, то есть от третьего знака добирает пять пробелов, а не
/// восемь. Пока раскрывалось постоянным числом, отступ кода после любого

/// Схлопываемый пробел по CSS — ТОЛЬКО эти четыре знака.
///
/// Остальные пробельные символы юникода — обычные знаки со своей шириной:
/// идеографический пробел `U+3000` держит место целого иероглифа, неразрывный
/// `U+00A0` не даёт разорвать строку. Пока схлопывалось всё пробельное подряд,
/// такой пробел пропадал из текста вместе со своей шириной.
/// Знак управления двунаправленностью — встраивание, отмена, изоляция
/// (UAX #9: LRE/RLE/PDF/LRO/RLO, LRI/RLI/FSI/PDI). Для обработки пробелов
/// его нет вовсе: css-text-3 §4.1 — «ignoring bidi formatting characters as
/// if they were not there» (`white-space-collapsing-bidi-001/002`).
fn bidi_format(ch: char) -> bool {
    matches!(ch as u32, 0x202A..=0x202E | 0x2066..=0x2069)
}

fn is_collapsible(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r')
}

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

/// Знак-распорка: место под поля, рамки и отступы СТРОЧНОЙ коробки.
///
/// Своей коробки в раскладке у неё нет, поэтому ширину даёт трекинг на этом
/// знаке. Соединитель слов (U+FEFF) взят за то, что точкой переноса он не
/// является: строка не должна рваться по краю `<span>`. Но по UAX-14 его класс
/// (WJ) запрещает разрыв и ПЕРЕД собой — а значит, и по пробелу перед коробкой.
/// Поэтому точки переноса считаются по тексту БЕЗ распорок (`Paragraph`).
pub const SPACER: &str = "\u{feff}";

/// Метка атомарного куска: ширины не несёт, ряд пробелов не рвёт.
pub const ZWSP: &str = "\u{200b}";

/// Места распорок в тексте абзаца — байтовые смещения.
///
/// Считаются по тем же правилам, что и `text_and_runs`. Распорка — всегда
/// СВОЙ кусок ровно из одного знака: так она и отличается от того же знака,
/// пришедшего из документа.
pub fn spacers(pieces: &[Piece]) -> Vec<usize> {
    let mut at = 0usize;
    let mut out = Vec::new();
    for p in pieces {
        let Piece::Text { text, .. } = p else {
            continue;
        };
        if text == SPACER {
            out.push(at);
        }
        at += text.len();
    }
    out
}

/// Куски ВНЕ потока и их место в тексте абзаца — байтовое смещение.
///
/// Считается по тем же правилам, что и `text_and_runs`: смещение равно длине
/// текста, собранного до этого куска.
pub fn overlays(pieces: Vec<Piece>) -> Vec<(usize, AnyElement, OverlayAt)> {
    let mut at = 0usize;
    let mut out = Vec::new();
    for p in pieces {
        match p {
            Piece::Text { text, .. } => at += text.len(),
            Piece::Overlay(el, how) => out.push((at, el, how)),
            Piece::Atom(_) => {}
        }
    }
    out
}

/// Шрифт струта абзаца — тот же, каким набирался бы текст самого блока.
pub fn strut_font(style: &Computed, base: &TextStyle) -> gpui::Font {
    run_for("x", style, base).font
}

fn run_for(text: &str, style: &Computed, base: &TextStyle) -> TextRun {
    let mut font = base.font();
    if font.fallbacks.is_none() {
        font.fallbacks = crate::fonts::document_fallbacks();
    }
    // Названное семейство сильнее родового: подстановкой занимается система.
    // Пустое имя — «шрифт документа» (разбор `font-family`): база как есть.
    if let Some(family) = style.font_family.as_ref().filter(|f| !f.is_empty()) {
        // Имя из разметки может быть придуманным (`@font-face`) — система
        // шрифтов знает файл под его собственным именем. Лиц у имени бывает
        // несколько, и нужное выбирает ширина начертания (§font-matching).
        font.family = crate::fonts::alias_stretch(family, style.font_stretch)
            .unwrap_or_else(|| family.clone())
            .into();
    } else if style.monospace == Some(true) {
        font.family = crate::metrics::mono_family_for(style.lang.as_deref()).into();
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
        background_color: style.inline_bg.map(Color::to_hsla),
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
    align: Option<crate::computed::Align>,
    text_align: Option<crate::computed::TextAlign>,
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
    use crate::computed::{Align, TextAlign};
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
fn atom_glue(ch: char) -> bool {
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
fn glue_marker(p: &Piece) -> bool {
    matches!(p, Piece::Text { text, .. } if text == "\u{200b}" || text == SPACER)
}

/// Куски ряда, сгруппированные по правилу склейки с атомом (css-text-3 §5.1).
///
/// Клеится только ПРИЛЕГАЮЩЕЕ слово: точка переноса по пробелу перед ним
/// обязана остаться, иначе весь кусок текста стал бы неразрывным.
/// Две соседние коробки БЕЗ знака-склейки не склеиваются: между ними точка
/// переноса есть (`line-breaking-atomic-007`).
fn glue_atoms(pieces: Vec<Piece>) -> Vec<Vec<Piece>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::parse_decls;

    fn styled(css: &str) -> Computed {
        let mut c = Computed::default();
        c.apply_decls(&parse_decls(css));
        c
    }

    #[test]
    fn spaces_collapse_like_html() {
        assert_eq!(normalize_spaces("  два\n\tслова  "), " два слова ");
        assert_eq!(normalize_spaces("a\n\nb"), "a b");
        // Идеографический и неразрывный пробелы — знаки, а не пробелы CSS:
        // они остаются как есть и держат свою ширину.
        assert_eq!(normalize_spaces("あ\u{3000}あ"), "あ\u{3000}あ");
        assert_eq!(normalize_spaces("a\u{00a0}b"), "a\u{00a0}b");
    }

    #[test]
    fn logical_alignment_follows_the_writing_direction() {
        // Наследование хранит ЛОГИЧЕСКОЕ значение, разворот — на отрисовке.
        // Иначе левый край, посчитанный для тела страницы, доставался бы по
        // наследству вложенному блоку справа налево.
        let parent = Computed::default();
        let ltr = inherit(&parent, &styled("text-align: start"));
        assert_eq!(ltr.text_align, Some(TextAlign::Start));
        assert_eq!(crate::lines::align_for(&ltr), crate::lines::Align::Left);
        let rtl = inherit(&parent, &styled("text-align: start; direction: rtl"));
        assert_eq!(crate::lines::align_for(&rtl), crate::lines::Align::Right);
        let rtl_end = inherit(&parent, &styled("text-align: end; direction: rtl"));
        assert_eq!(crate::lines::align_for(&rtl_end), crate::lines::Align::Left);
        // Умолчание CSS — `start`: без выключки текст справа налево прижат
        // вправо, а не влево.
        let bare = inherit(&parent, &styled("direction: rtl"));
        assert_eq!(crate::lines::align_for(&bare), crate::lines::Align::Right);
        // Наследник блока справа налево берёт сторону письма у него.
        let child = inherit(&bare, &Computed::default());
        assert_eq!(crate::lines::align_for(&child), crate::lines::Align::Right);
    }

    #[test]
    fn breaking_rules_leave_the_text_alone_for_the_own_layout() {
        // Подсказки переносчику GPUI ставятся только там, где абзац рисует
        // сам движок. Своей раскладке они бы мешали: невидимый знак стал бы
        // лишней точкой разрыва (правила считает `lines::Paragraph`).
        for css in [
            "line-break: anywhere",
            "word-break: break-all",
            "word-break: keep-all",
            "white-space: break-spaces",
        ] {
            let style = styled(css);
            assert!(crate::lines::rules(&style).is_some(), "{css}");
            assert_eq!(breakable("a b", &style), "a b", "{css}");
        }
        // Своей раскладке мягкий перенос доезжает КАК ЕСТЬ: она знает его
        // точкой разрыва и рисует на его месте знак переноса.
        assert_eq!(breakable("a\u{ad}b", &Computed::default()), "a\u{ad}b");
        assert_eq!(breakable("a\u{ad}b", &styled("hyphens: none")), "ab");
    }

    #[test]
    fn plain_word_break_is_left_alone() {
        // Умолчание не трогается: там перенос между знаками разрешён самим
        // переносчиком, вставлять нечего.
        assert_eq!(breakable("中文", &styled("word-break: normal")), "中文");
    }

    #[test]
    fn same_size_pieces_go_into_one_block() {
        let pieces = vec![
            Piece::Text {
                text: "обычный ".into(),
                style: styled(""),
            },
            Piece::Text {
                text: "жирный".into(),
                style: styled("font-weight: 700"),
            },
        ];
        assert!(single_block(&pieces, 13.0), "вес не мешает единому блоку");
    }

    #[test]
    fn different_size_stays_in_one_block() {
        let pieces = vec![
            Piece::Text {
                text: "обычный ".into(),
                style: styled(""),
            },
            Piece::Text {
                text: "крупный".into(),
                style: styled("font-size: 24px"),
            },
        ];
        // Кегль едет в прогон (патч GPUI), поэтому разный размер больше не
        // выгоняет абзац в запасную ветку из отдельных слов: раньше там
        // строки наезжали друг на друга.
        assert!(
            single_block(&pieces, 13.0),
            "иной размер собирается единым блоком"
        );
        assert_eq!(
            max_font_size(&pieces, 13.0, 13.0),
            24.0,
            "строка растёт под кусок"
        );
    }

    #[test]
    fn inheritance_carries_text_not_box() {
        let parent = styled("color: #ff0000; padding: 10px; font-size: 20px");
        let child = styled("font-weight: 700");
        let merged = inherit(&parent, &child);
        assert_eq!(merged.color.map(|c| c.r), Some(1.0), "цвет наследуется");
        assert_eq!(merged.font_size, Some(Len::Px(20.0)), "размер наследуется");
        assert_eq!(merged.font_weight, Some(700), "свой вес сохранён");
        assert_eq!(merged.padding.top, None, "отступ родителя вниз не идёт");
    }

    #[test]
    fn own_style_wins_over_inherited() {
        let parent = styled("color: #ff0000");
        let child = styled("color: #0000ff");
        assert_eq!(inherit(&parent, &child).color.map(|c| c.b), Some(1.0));
    }
}

#[cfg(test)]
mod em_tests {
    use super::*;
    use crate::css::parse_decls;

    fn styled(css: &str) -> Computed {
        let mut c = Computed::default();
        c.apply_decls(&parse_decls(css));
        c
    }

    #[test]
    fn em_resolves_against_the_font_size() {
        // Родитель 12px, свой размер 2em = 24px, высота 1em = 24px.
        let parent = styled("font-size: 12px");
        let child = styled("font-size: 2em; height: 1em");
        let merged = inherit(&parent, &child);
        assert_eq!(
            merged.font_size,
            Some(Len::Px(24.0)),
            "свой кегль от родителя"
        );
        assert_eq!(merged.height, Some(Len::Px(24.0)), "высота от своего кегля");

        // Внук: 2em от 24 = 48, высота 1em = 48.
        let grand = styled("font-size: 2em; height: 1em");
        let merged2 = inherit(&merged, &grand);
        assert_eq!(merged2.font_size, Some(Len::Px(48.0)));
        assert_eq!(merged2.height, Some(Len::Px(48.0)));
    }

    #[test]
    fn em_without_own_font_size_uses_the_inherited_one() {
        let parent = styled("font-size: 20px");
        let child = styled("width: 10em");
        let merged = inherit(&parent, &child);
        assert_eq!(merged.width, Some(Len::Px(200.0)));
    }
}
