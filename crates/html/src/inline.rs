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
    Text { text: String, style: Computed },
    Atom(AnyElement),
    /// Элемент ВНЕ потока строки: места не занимает, но рисуется там, где
    /// стоит в тексте (абсолютный элемент на статической позиции). В отличие
    /// от `Atom` не выводит абзац из текстового пути — иначе строка теряет
    /// пробелы и перенос по словам (`line-breaking-018`).
    Overlay(AnyElement),
}

/// Собрать инлайн-куски из детей узла.
pub fn collect(
    children: &[Node],
    inherited: &Computed,
    atom: &mut dyn FnMut(&Element) -> Option<Piece>,
) -> Vec<Piece> {
    let mut out = vec![];
    for child in children {
        match child {
            Node::Text(t) => {
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
                let text = breakable(&transform_case(&raw, inherited), inherited);
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
                    out.push(Piece::Text {
                        text: "\u{200b}".into(),
                        style: inherited.clone(),
                    });
                    continue;
                }
                if let Some(piece) = atom(e) {
                    out.push(piece);
                    continue;
                }
                let mut merged = inherit(inherited, &e.style);
                // Фон строчного бокса рисует прогон текста: коробки у него
                // нет, а фон обязан рваться на переносах вместе со строкой.
                if let Some(bg) = e.style.background {
                    merged.inline_bg = Some(bg);
                    let px_of = |l: Option<crate::value::Len>| match l {
                        Some(crate::value::Len::Px(v)) => v,
                        _ => 0.0,
                    };
                    merged.inline_pad = Some((
                        px_of(e.style.padding.left).max(px_of(e.style.padding.right)),
                        px_of(e.style.padding.top).max(px_of(e.style.padding.bottom)),
                    ));
                    merged.inline_radius = Some(px_of(e.style.radius.tl));
                }
                // Рамка строчной коробки рисуется прогоном — но только
                // РОВНАЯ: прогон рисует один прямоугольник, а разные грани
                // (`border-left: 30px`) ему не выразить, такому куску нужна
                // своя коробка (её заводит `render::has_own_box`).
                if let Some((color, width)) = uniform_border(&e.style) {
                    merged.inline_border = Some((color, width));
                }
                // Контур строчного куска рисует тот же прогон: коробки у
                // куска нет, а место контур и не занимает.
                if merged.inline_border.is_none()
                    && e.style.display.is_none()
                    && let Some(o) = e.style.outline
                    && let Some(Len::Px(width)) = o.width
                    && width > 0.0
                    && let Some(color) = o.color.or(e.style.color)
                {
                    merged.inline_border = Some((color, width));
                }
                // Атомарная строчная коробка — ГРАНИЦА переноса, даже когда
                // своей коробки в раскладке ей не завели (размер не задан, и
                // она осталась куском текста). По CSS строку можно рвать
                // между текстом и такой коробкой; у нас это выражается
                // нулевым пробелом по краям (`line-breaking-atomic-007`).
                let atomic = matches!(
                    e.style.display,
                    Some(crate::computed::Display::InlineBlock)
                        | Some(crate::computed::Display::InlineFlex)
                        | Some(crate::computed::Display::InlineGrid)
                );
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
                let (lead, trail) = inline_sides(e, &merged);
                if lead != 0.0 {
                    out.push(Piece::Text {
                        text: "\u{feff}".into(),
                        style: spacer_style(&merged, lead),
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
                out.extend(shift_overlays(collect(&e.children, &merged, atom), &e.style));
                if let Some(mark) = close {
                    out.push(Piece::Text {
                        text: mark.to_string(),
                        style: merged.clone(),
                    });
                }
                if trail != 0.0 {
                    out.push(Piece::Text {
                        text: "\u{feff}".into(),
                        style: spacer_style(&merged, trail),
                    });
                }
                if atomic {
                    out.push(Piece::Text {
                        text: "\u{200b}".into(),
                        style: marker,
                    });
                }
            }
        }
    }
    out
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

/// Сдвинуть куски вне потока на относительный сдвиг их строчного предка.
fn shift_overlays(pieces: Vec<Piece>, style: &Computed) -> Vec<Piece> {
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
            Piece::Overlay(el) => Piece::Overlay(
                gpui::div()
                    .pl(gpui::px(dx))
                    .pt(gpui::px(dy))
                    .child(el)
                    .into_any_element(),
            ),
            other => other,
        })
        .collect()
}

/// Отделить первую букву абзаца в свой кусок (`::first-letter`).
///
/// Пробелы и знаки перед буквой в CSS входят в буквицу вместе с ней, поэтому
/// разрез идёт по первому непробельному символу включительно.
pub fn split_first_letter(pieces: Vec<Piece>, style: &Computed) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::with_capacity(pieces.len() + 1);
    let mut done = false;
    for p in pieces {
        match p {
            Piece::Text { text, style: own } if !done => {
                let Some(pos) = text.char_indices().find(|(_, c)| !c.is_whitespace()) else {
                    out.push(Piece::Text { text, style: own });
                    continue;
                };
                let end = pos.0 + pos.1.len_utf8();
                let mut letter = own.clone();
                // Из слоя берутся ТОЛЬКО текстовые свойства: коробки у буквы
                // нет, и отступы абзаца ей не принадлежат.
                letter.font_size = style.font_size.or(own.font_size);
                letter.color = style.color.or(own.color);
                letter.font_weight = style.font_weight.or(own.font_weight);
                letter.italic = style.italic.or(own.italic);
                letter.font_family = style.font_family.clone().or(own.font_family.clone());
                out.push(Piece::Text {
                    text: text[..end].to_string(),
                    style: letter,
                });
                if end < text.len() {
                    out.push(Piece::Text {
                        text: text[end..].to_string(),
                        style: own,
                    });
                }
                done = true;
            }
            other => out.push(other),
        }
    }
    out
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
                    c.font_family = style.font_family.clone().or(base.font_family.clone());
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
pub fn inherit(parent: &Computed, own: &Computed) -> Computed {
    let mut c = own.clone();
    c.color = own.color.or(parent.color);
    c.font_size = own.font_size.or(parent.font_size);
    c.font_weight = own.font_weight.or(parent.font_weight);
    c.italic = own.italic.or(parent.italic);
    c.underline = own.underline.or(parent.underline);
    c.line_through = own.line_through.or(parent.line_through);
    c.line_height = own.line_height.or(parent.line_height);
    c.text_align = own.text_align.or(parent.text_align);
    c.text_align_last = own.text_align_last.or(parent.text_align_last);
    c.hanging = own.hanging.or(parent.hanging);
    c.monospace = own.monospace.or(parent.monospace);
    c.font_family = own.font_family.clone().or(parent.font_family.clone());
    c.nowrap = own.nowrap.or(parent.nowrap);
    // Направление письма наследуется: `writing-mode` ставят на `body`, а ось
    // потока обязана смениться у КАЖДОГО вложенного блока — иначе вертикально
    // становится только сам `body`, а его дети снова текут вниз.
    c.vertical = own.vertical.or(parent.vertical);
    c.vertical_rl = own.vertical_rl.or(parent.vertical_rl);
    c.ortho_limit = own.ortho_limit.or(parent.ortho_limit);
    c.wrap_anywhere = own.wrap_anywhere.or(parent.wrap_anywhere);
    c.word_space_char = own.word_space_char.or(parent.word_space_char);
    c.autospace_alpha = own.autospace_alpha.or(parent.autospace_alpha);
    c.autospace_numeric = own.autospace_numeric.or(parent.autospace_numeric);
    // Сдвиг НЕ наследуется: он принадлежит своему куску, иначе надстрочный
    // знак поднимал бы весь текст после себя.
    c.vertical_shift = own.vertical_shift;
    c.upright = own.upright.or(parent.upright);
    // Наследуемые текстовые свойства из второй волны разбора. Без них
    // `text-transform` на контейнере не доходил до вложенного текста —
    // а в разметке его ставят именно на контейнер.
    c.letter_spacing = own.letter_spacing.or(parent.letter_spacing);
    c.word_spacing = own.word_spacing.or(parent.word_spacing);
    c.text_transform = own.text_transform.or(parent.text_transform);
    c.text_indent = own.text_indent.or(parent.text_indent);
    c.break_anywhere = own.break_anywhere.or(parent.break_anywhere);
    c.break_word = own.break_word.or(parent.break_word);
    c.balance_lines = own.balance_lines.or(parent.balance_lines);
    c.break_anywhere_strict = own.break_anywhere_strict.or(parent.break_anywhere_strict);
    c.keep_all = own.keep_all.or(parent.keep_all);
    c.hyphens_auto = own.hyphens_auto.or(parent.hyphens_auto);
    c.lang = own.lang.clone().or(parent.lang.clone());
    c.break_after_spaces = own.break_after_spaces.or(parent.break_after_spaces);
    c.hyphenate = own.hyphenate.or(parent.hyphenate);
    c.tab_size = own.tab_size.or(parent.tab_size);
    c.marker = own.marker.or(parent.marker);
    c.vertical_align = own.vertical_align.or(parent.vertical_align);
    c.font_stretch = own.font_stretch.or(parent.font_stretch);
    c.no_select = own.no_select.or(parent.no_select);
    c.pointer_events_none = own.pointer_events_none.or(parent.pointer_events_none);
    c.line_clamp = own.line_clamp.or(parent.line_clamp);
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
    c.text_shadow = own.text_shadow.or(parent.text_shadow);
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
    if let Some(f) = c.filter.filter(|_| own.filter.is_none()) {
        c.background = c.background.map(|col| f.apply(col));
        c.color = c.color.map(|col| f.apply(col));
        c.border_color = c.border_color.map(|col| f.apply(col));
        for side in c.border_colors.iter_mut() {
            *side = side.map(|col| f.apply(col));
        }
        if let Some(g) = c.gradient.as_mut() {
            g.from = f.apply(g.from);
            g.to = f.apply(g.to);
            for stop in g.stops.iter_mut() {
                stop.0 = f.apply(stop.0);
            }
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
    c.border_spacing = own.border_spacing.or(parent.border_spacing);
    c.caret_color = own.caret_color.or(parent.caret_color);
    // `em` считается от размера шрифта — а он известен только здесь, когда
    // наследование уже произошло. Раньше длина переводилась в точки при
    // разборе, по постоянным 16 точкам, и вложенные кегли не перемножались.
    let parent_px = match parent.font_size {
        Some(Len::Px(px)) => px,
        _ => 16.0,
    };
    // Процент у размера шрифта — доля родительского кегля; в точках его надо
    // получить здесь, иначе абзац уходил в запасную ветку переноса (размер
    // «не такой, как у базового») и терял перенос по словам.
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
    if has_border
        && c.border_color.is_none()
        && c.border_colors.iter().all(Option::is_none)
    {
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
        None => parent.tab_size_len,
    };
    c
}

/// Знаки управления двунаправленностью вокруг куска.
///
/// Отмена (`bidi-override`, тег `<bdo>`) ставит знаки в заданную сторону как
/// есть; изоляция (`isolate`) прячет кусок от соседей; просто своя сторона —
/// встраивание. Все три случая Юникод выражает парой знаков, и разбор их
/// понимает — своего кода под них не нужно.
pub fn bidi_marks(own: &Computed, merged: &Computed) -> (Option<char>, Option<char>) {
    let rtl = merged.rtl == Some(true);
    if own.bidi_override == Some(true) {
        // Отмена: RLO/LRO … PDF.
        let open = if rtl { '\u{202e}' } else { '\u{202d}' };
        return (Some(open), Some('\u{202c}'));
    }
    if own.bidi_isolate == Some(true) {
        // Изоляция: RLI/LRI … PDI.
        let open = if rtl { '\u{2067}' } else { '\u{2066}' };
        return (Some(open), Some('\u{2069}'));
    }
    if own.rtl.is_some() {
        // Своя сторона письма: RLE/LRE … PDF.
        let open = if rtl { '\u{202b}' } else { '\u{202a}' };
        return (Some(open), Some('\u{202c}'));
    }
    (None, None)
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
            Piece::Overlay(_) => {}
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
    // запретить разрыв между соседями, а это решает своя раскладка по классу
    // знака. До шейпера он доезжать не должен — портит метрику последней
    // строки (`line-break-anywhere-overrides-uax-behavior-011`: коробка ниже
    // на шесть точек).
    let text = &text.replace('\u{034f}', "");
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

/// Сдвиг ПО КУСКАМ: отрезок байт → смещение базовой линии в точках.
///
/// `vertical-align: super`/`sub` поднимает и опускает кусок внутри строки.
/// Доля кегля взята браузерная: треть вверх и пятая часть вниз.
pub fn shift_spans(pieces: &[Piece], base_size: f32) -> Vec<(std::ops::Range<usize>, gpui::Pixels)> {
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
        let dy = style.vertical_shift.map(|k| k * size);
        if let Some(v) = dy {
            out.push((at..end, gpui::px(v)));
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
            out.push((at..end, gpui::px(v)));
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

/// Боковые поля, рамки и отступы строчной коробки в точках.
///
/// Доля тут не считается: она берётся от ширины контейнера, которая на сборке
/// кусков ещё не решена. Пропуск честнее приблизительной длины — её видно в
/// сравнении с браузером.
fn inline_sides(e: &Element, merged: &Computed) -> (f32, f32) {
    let size = match merged.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    let family = merged.font_family.clone().unwrap_or_default();
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(_)) | Some(Len::Em(_)) | Some(Len::Ch(_)) | Some(Len::Ex(_)) => {
            crate::metrics::spacing_px(l, &family, size)
        }
        _ => 0.0,
    };
    let border = e.style.borders();
    // Отступ вокруг ПОДСВЕТКИ держит сам прогон текста (`inline_pad`), поэтому
    // распорка его не повторяет — иначе он считался бы дважды и диакритика
    // уезжала из-под своей коробки (`shaping-arabic-diacritics-002`).
    let pad = |l: Option<Len>| {
        if e.style.background.is_some() {
            0.0
        } else {
            px_of(l)
        }
    };
    (
        px_of(e.style.margin.left) + px_of(border.left) + pad(e.style.padding.left),
        px_of(e.style.margin.right) + px_of(border.right) + pad(e.style.padding.right),
    )
}

/// Слой знака-распорки: ширину ему даёт трекинг на своём куске, а всё
/// остальное с него снимается — фон и замена пробелов принадлежат тексту.
fn spacer_style(merged: &Computed, advance: f32) -> Computed {
    let mut style = merged.clone();
    style.letter_spacing = Some(Len::Px(advance));
    style.word_spacing = None;
    style.word_space_char = None;
    style.inline_bg = None;
    style.inline_border = None;
    style
}

/// Ровная рамка строчной коробки: одинаковые цвет и толщина у всех граней.
///
/// Только такую умеет нарисовать прогон текста. Разные грани оставляем
/// коробке в раскладке — там они честные, но текст в ней не переносится
/// вместе с абзацем.
pub fn uniform_border(c: &Computed) -> Option<(Color, f32)> {
    let w = c.borders();
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    let (t, r, b, l) = (px_of(w.top)?, px_of(w.right)?, px_of(w.bottom)?, px_of(w.left)?);
    if t <= 0.0 || t != r || t != b || t != l {
        return None;
    }
    let sides = &c.border_colors;
    let color = match (c.border_color, sides[0], sides[1], sides[2], sides[3]) {
        (_, Some(a), Some(b2), Some(c2), Some(d)) if a == b2 && a == c2 && a == d => a,
        (Some(one), None, None, None, None) => one,
        // Цвет не задан вовсе — рамка красится цветом текста.
        (None, None, None, None, None) => c.color?,
        _ => return None,
    };
    Some((color, t))
}

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
    trim_edge(pieces.iter_mut().rev(), false);
    trim_edge(pieces.iter_mut(), true);
}

/// Один край строки: куски идут от него внутрь, коробки пропускаются.
fn trim_edge<'a>(pieces: impl Iterator<Item = &'a mut Piece>, leading: bool) {
    for piece in pieces {
        match piece {
            // Коробка без текста для ряда пробелов прозрачна.
            Piece::Atom(_) | Piece::Overlay(_) => continue,
            Piece::Text { text, style } => {
                // `white-space: pre*` пробелы бережёт — там удалять нечего.
                if style.keep_spaces == Some(true) || style.preserve_newlines == Some(true) {
                    return;
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
            Piece::Text { text, .. } => {
                for (at, ch) in text.char_indices() {
                    seq.push((i, at, ch));
                }
            }
            // Атомарная коробка иероглифом не является и соседство разрывает.
            _ => seq.push((usize::MAX, 0, '\u{0}')),
        }
    }
    let mut edits: Vec<(usize, usize)> = vec![];
    for k in 0..seq.len() {
        let (piece, at, ch) = seq[k];
        if ch != '\u{200b}' || piece == usize::MAX {
            continue;
        }
        let Piece::Text { style, .. } = &pieces[piece] else {
            continue;
        };
        if style.word_space_char != Some('\u{3000}') {
            continue;
        }
        if k > 0 && k + 1 < seq.len() && ideographic(seq[k - 1].2) && ideographic(seq[k + 1].2) {
            edits.push((piece, at));
        }
    }
    for (piece, at) in edits {
        if let Piece::Text { text, .. } = &mut pieces[piece] {
            text.replace_range(at..at + '\u{200b}'.len_utf8(), "\u{3000}");
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
            let mut out = String::with_capacity(text.len());
            let mut at_start = true;
            for ch in text.chars() {
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
                at_start = ch.is_whitespace();
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
fn line_break() -> AnyElement {
    gpui::div().w_full().h_0().into_any_element()
}

/// `word-spacing` и `text-indent`: строка из слов с зазором.
///
/// Обходной путь вместо правки шейпера: пробел остаётся в куске, а лишний
/// интервал даёт зазор гибкой строки — расстановка получается точной, а
/// перенос идёт по словам, как в браузере. Отступ первой строки — распорка
/// перед первым словом: при переносе она остаётся только на первой строке,
/// ровно как `text-indent`.
pub fn as_word_row(
    pieces: Vec<Piece>,
    word_spacing: f32,
    indent: f32,
    align: Option<crate::computed::Align>,
    text_align: Option<crate::computed::TextAlign>,
    render_text: &mut dyn FnMut(String, &Computed) -> AnyElement,
) -> AnyElement {
    use crate::computed::{Align, TextAlign};
    // Предел ширины — родитель: без него ряд считает себя по содержимому и
    // не переносит НИЧЕГО, сколько бы ни вылезал (`line-breaking-atomic-007`:
    // три знака по 50 в коробке шириной 40 стояли в строку).
    let mut row = gpui::div()
        .flex()
        .flex_wrap()
        .max_w_full()
        .gap_x(gpui::px(word_spacing));
    row = match align {
        Some(Align::Center) => row.items_center(),
        Some(Align::Start) => row.items_start(),
        Some(Align::End) => row.items_end(),
        _ => row.items_baseline(),
    };
    row = match text_align {
        Some(TextAlign::Center) => row.justify_center(),
        Some(TextAlign::Right) => row.justify_end(),
        Some(TextAlign::Left) => row.justify_start(),
        _ => row,
    };
    if indent != 0.0 {
        row = row.child(gpui::div().w(gpui::px(indent)).flex_shrink_0());
    }
    for p in drop_hanging_tail(pieces) {
        row = match p {
            Piece::Atom(el) => row.child(el),
            Piece::Overlay(el) => row.child(overlay_in_row(el)),
            Piece::Text { text, style } => {
                // Пробел остаётся при слове: зазор ДОБАВЛЯЕТСЯ к нему, а не
                // заменяет — иначе слова слипаются на нулевом `word-spacing`.
                for (n, part) in text.split('\n').enumerate() {
                    if n > 0 {
                        row = row.child(line_break());
                    }
                    for w in part.split_inclusive(' ') {
                        row = row.child(render_text(w.to_string(), &style));
                    }
                }
                row
            }
        };
    }
    row.into_any_element()
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
        let before = out.chars().next_back();
        let after = chars.get(at).copied();
        // Преобразование перевода строки (CSS Text 3 §4.1.2): между двумя
        // ШИРОКИМИ знаками перевод УДАЛЯЕТСЯ, а не становится пробелом —
        // иначе японский текст, набранный в несколько строк, получает лишние
        // пробелы на каждом переводе.
        let drop = had_break
            && before.is_some_and(wide_cjk)
            && after.is_some_and(wide_cjk);
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
        || (0xFFE0..=0xFFE6).contains(&c)
        || (0x20000..=0x3FFFD).contains(&c)
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
fn is_collapsible(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r')
}

/// Можно ли собрать всё в один текстовый блок: одинаковый размер шрифта и ни
/// одного не-текстового куска.
pub fn single_block(pieces: &[Piece], _base_size: f32) -> bool {
    // Кегль куску больше не мешает: он едет в прогон (патч GPUI). Мешает
    // только не-текстовый кусок — картинку в прогон не положить. Кусок ВНЕ
    // потока не мешает: место в строке он не занимает.
    pieces
        .iter()
        .all(|p| matches!(p, Piece::Text { .. } | Piece::Overlay(_)))
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
        Piece::Atom(_) | Piece::Overlay(_) => acc,
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
            Piece::Overlay(_) => {}
            Piece::Atom(_) => return None,
        }
    }
    (!text.is_empty()).then_some((text, runs))
}

/// Куски ВНЕ потока и их место в тексте абзаца — байтовое смещение.
///
/// Считается по тем же правилам, что и `text_and_runs`: смещение равно длине
/// текста, собранного до этого куска.
pub fn overlays(pieces: Vec<Piece>) -> Vec<(usize, AnyElement)> {
    let mut at = 0usize;
    let mut out = Vec::new();
    for p in pieces {
        match p {
            Piece::Text { text, .. } => at += text.len(),
            Piece::Overlay(el) => out.push((at, el)),
            Piece::Atom(_) => {}
        }
    }
    out
}


fn run_for(text: &str, style: &Computed, base: &TextStyle) -> TextRun {
    let mut font = base.font();
    // Названное семейство сильнее родового: подстановкой занимается система.
    if let Some(family) = &style.font_family {
        // Имя из разметки может быть придуманным (`@font-face`) — система
        // шрифтов знает файл под его собственным именем.
        font.family = crate::fonts::alias(family)
            .unwrap_or_else(|| family.clone())
            .into();
    } else if style.monospace == Some(true) {
        font.family = crate::metrics::mono_family().into();
    }
    if let Some(w) = style.font_weight {
        font.weight = FontWeight(w as f32);
    }
    if style.italic == Some(true) {
        font.style = FontStyle::Italic;
    }
    if let Some(pct) = style.font_stretch {
        font.stretch = gpui::FontStretch::from_percent(pct);
    }
    if !style.font_features.is_empty() {
        font.features = gpui::FontFeatures(std::sync::Arc::new(style.font_features.clone()));
    }
    let color = style.color.map(Color::to_hsla).unwrap_or(base.color);
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
            .map(|(c, w)| (c.to_hsla(), gpui::px(w))),
        background_pad: {
            let (x, y) = style.inline_pad.unwrap_or((0.0, 0.0));
            gpui::point(gpui::px(x), gpui::px(y))
        },
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
    render_text: &mut dyn FnMut(String, &Computed) -> AnyElement,
) -> AnyElement {
    use crate::computed::{Align, TextAlign};
    // `vertical-align` в строке: умолчание — базовая линия, но `middle`,
    // `top` и `bottom` встречаются и раньше не доезжали никуда, кроме
    // ячейки таблицы.
    let mut row = gpui::div().flex().flex_wrap().max_w_full();
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
    row = match text_align {
        Some(TextAlign::Center) => row.justify_center(),
        Some(TextAlign::Right) => row.justify_end(),
        Some(TextAlign::Left) => row.justify_start(),
        _ => row,
    };
    for p in drop_hanging_tail(pieces) {
        row = match p {
            Piece::Atom(el) => row.child(el),
            Piece::Overlay(el) => row.child(overlay_in_row(el)),
            Piece::Text { text, style } => {
                // Пробел остаётся при слове: без него слова слиплись бы.
                for (n, part) in text.split('\n').enumerate() {
                    if n > 0 {
                        row = row.child(line_break());
                    }
                    for w in part.split_inclusive(' ') {
                        row = row.child(render_text(w.to_string(), &style));
                    }
                }
                row
            }
        };
    }
    row.into_any_element()
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
