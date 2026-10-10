//! Сборка строчных кусков абзаца (paragraph_pieces_routed).

mod styling;
pub(super) use styling::style_pieces;

mod routing;
pub(super) use routing::route_pieces;

mod native;
pub(super) use native::native_paragraph;

mod spans;
pub(super) use spans::paragraph_spans;

use crate::dom::{Element, Node};
use crate::render::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::inline;
use crate::text::ruby::ruby_role;
use gpui::{AnyElement, IntoElement, div};

pub(crate) fn paragraph_pieces_routed(
    nodes: &[Node],
    inherited: &Computed,
    opts: &RenderOpts,
    first_line_at: usize,
    first_line: &Computed,
    native_request: Option<&native_paragraph_route::Request<'_>>,
) -> AnyElement {
    // Бюджет строк знака обрыва (авто-режим) забирается РАЗОМ, до сборки
    // кусков: куски строят вложенные абзацы (`inline-block`, `<svg>`), и
    // чужой бюджет им доставаться не должен.
    let clamp_budget = crate::text::clamp::take_para_budget();
    let clamp_tag = crate::text::clamp::take_para_tag();
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: брать поперечное выравнивание ряда с самих
    // кусков, когда абзац своего не задал (`vertical-align: bottom` у
    // картинки). Ни это, ни `align-self` на самой картинке высоту строки не
    // меняют — строка всё равно выходит около 90 точек вместо 60, и картинка
    // просто прижимается к её низу. Дело не в выравнивании.
    // Текстовый путь абзаца — единственное место, где место куска вне потока
    // считается ДВУНАПРАВЛЕННО: строку режет и переставляет разбор UAX#9
    // внутри `lines.rs`, а `point_of` берёт продвижение от начала СВОЕЙ
    // строки. Ряд из слов (`inline.rs: as_wrapped_row`) о направлении не
    // знает вовсе — куски идут в ЛОГИЧЕСКОМ порядке, строка прижимается
    // целиком, и щуп садится в НАЧАЛО rtl-строки (замер: x = 328 при верных
    // 168, снимки `target/scoutrtlv/target/wpt-shots/`).
    //
    // Открыть текстовый путь при `direction: rtl` можно только абзацу, у
    // которого этот путь ДОСТУПЕН: на пустом тексте `inline::text_and_runs`
    // отдаёт `None` (`inline.rs:2543`), абзац всё равно сваливается в ряд, а
    // там кусок вне потока заворачивается в `inline.rs: overlay_in_row` со
    // СВОИМ, пустым `Spot` и теряет и `rtl`, и `next_line`. Ровно так устроена
    // семья `css-position/static-position/inline-level-absolute-in-block-
    // level-context-007..012` (rtl, абсолют `display: inline`, текста в абзаце
    // НЕТ, все шесть 0.00) — гейт оставляет её на прежнем пути.
    let flow_text = has_flow_text(nodes);
    let atom = |e: &Element| -> Option<inline::Piece> { atom_piece(e, inherited, opts, flow_text) };
    // Каждому атому — признак, можно ли поставить его В СТРОКУ абзаца
    // (`atom_line_align`): порядок записей совпадает с порядком `Piece::Atom`.
    // Руби несёт ещё и узлы своих аннотаций: по ним строка растёт
    // (`lines::ruby_extent`). Чужие узлы (руби вне строки внутри атома) атому
    // не достаются.
    let mut atom_aligns: Vec<
        Option<(
            crate::text::paragraph::AtomAlign,
            crate::text::paragraph::RubyExtents,
        )>,
    > = Vec::new();
    let mut atom_noted = |e: &Element| -> Option<inline::Piece> {
        let (piece, extents) = crate::text::paragraph::collect_ruby_extents(|| atom(e));
        if matches!(piece, Some(inline::Piece::Atom(_))) {
            let extents = if ruby_role(e) == Some(crate::style::computed::RubyRole::Container) {
                extents
            } else {
                crate::text::paragraph::RubyExtents::default()
            };
            // Абсолютная замещаемая — атом строки только РЯДОМ с текстом в
            // потоке: строка из одних внепоточных коробок нулевая (CSS 2.1
            // §9.4.2), а атом завёл бы ей струт (`clear-applies-to-001-ref`:
            // `<div>` с одной абсолютной картинкой вырастал на строку).
            let lone_abs = !flow_text
                && matches!(
                    e.style.position,
                    Some(crate::style::computed::Position::Absolute)
                        | Some(crate::style::computed::Position::Fixed)
                );
            atom_aligns.push(
                (!lone_abs)
                    .then(|| atom_line_align(e, inherited, opts))
                    .flatten()
                    .map(|a| (a, extents)),
            );
        }
        piece
    };
    let mut pieces = inline::collect(nodes, inherited, &mut atom_noted);
    if pieces.is_empty() {
        return div().into_any_element();
    }
    // Ряд пробелов через границу строчной коробки — один пробел (§16.6.1).
    // Проход идёт ПЕРВЫМ: всё дальше считает байтовые смещения по готовому
    // тексту кусков.
    inline::collapse_across_pieces(&mut pieces);
    // Точка переноса показывается знаком по СОСЕДЯМ, а они лежат в других
    // кусках — проход идёт по всему абзацу сразу.
    // Слогораздел идёт ПЕРВЫМ: он меняет сам текст кусков, а всё дальше
    // считает по готовому тексту байтовые смещения.
    inline::hyphenate_pieces(&mut pieces);
    inline::space_transform_pieces(&mut pieces);
    // Пойдут ли атомы В СТРОКУ (решение то же, что ниже у распорок атомов):
    // тогда атом — содержимое строки, и край для среза пробелов он обрывает.
    let atoms_in_line = {
        let count = pieces
            .iter()
            .filter(|p| matches!(p, inline::Piece::Atom(_)))
            .count();
        count > 0
            && count == atom_aligns.len()
            && atom_aligns.iter().all(Option::is_some)
            && atoms_fit_line(
                inherited,
                atom_aligns
                    .iter()
                    .any(|a| a.as_ref().is_some_and(|(_, ex)| !ex.is_empty())),
            )
    };
    if atoms_in_line {
        inline::trim_edge_spaces_solid_atoms(&mut pieces);
    } else {
        inline::trim_edge_spaces(&mut pieces);
    }
    pieces = style_pieces(pieces, inherited, first_line_at, first_line);
    // Межсловный интервал и отступ первой строки требуют строки из слов, а
    // единый текстовый блок их не умеет — поэтому решение принимается ДО
    // сборки блока, иначе оба свойства молча пропадали.
    // Отступ первой строки. Абсолютную часть разбор уже свёл к точкам, доля
    // же берётся от ширины содержащего блока и здесь ещё неизвестна — её
    // считает раскладка строк, когда ширина решена.
    // `calc(50% - 3px)` несёт обе части разом: `Paragraph` складывает
    // `px + pct × ширина строки` сам (`lines.rs`, css-text-3 §2.1).
    let mixed = match inherited.text_indent {
        Some(Len::Calc(i)) => crate::style::values::value::calc_get(i).pct_px(),
        _ => None,
    };
    let indent = crate::text::paragraph::Indent {
        px: match inherited.text_indent {
            Some(Len::Px(v)) => v,
            _ => mixed.map_or(0.0, |(_, px)| px),
        },
        pct: match inherited.text_indent {
            Some(Len::Pct(k)) => k,
            _ => mixed.map_or(0.0, |(pct, _)| pct),
        },
        each_line: inherited.text_indent_each_line == Some(true),
        hanging: inherited.text_indent_hanging == Some(true),
    };
    // Межсловный интервал считает своя раскладка строк: ряд из слов ломает
    // выключку, висящие пробелы и перенос. Ряд остаётся только под отступ
    // первой строки, который своей раскладке пока неизвестен.
    // Отступ первой строки умеет своя раскладка строк: она одна знает, где
    // строка кончается, и отрицательный отступ ей не помеха. Ряд из слов
    // остаётся запасным путём — на нём отступ становится распоркой, а она
    // отрицательной ширины не бывает.
    // Ряда из слов под отступ первой строки больше нет: `lines::rules` отдаёт
    // правила переноса ВСЕГДА, и своя раскладка строк умеет и отступ, и
    // отрицательный отступ.
    // Атомы — В СТРОКУ абзаца (CSS 2.1 §9.2.2, §10.8): каждый становится
    // распоркой (U+FEFF), её продвижение — ширина атома, а сам элемент
    // раскладывает и ставит на базовую линию своей строки `Paragraph`. Для
    // переноса распорка атома читается как U+FFFC (`Paragraph::linebreaks`):
    // так атом кладёт и Blink (`inline_items_builder.cc`, знак-заместитель
    // объекта), а класс CB даёт разрыв до и после (UAX #14 LB20).
    // Прежде абзац с атомом уходил в гибкий ряд слов (`as_wrapped_row`): одна
    // высота строки на ряд, базовая линия текста taffy не видна, `top`/
    // `bottom`/`text-top` не выражались.
    let mut line_atoms: Vec<(
        usize,
        AnyElement,
        crate::text::paragraph::AtomAlign,
        crate::text::paragraph::RubyExtents,
    )> = Vec::new();
    let atom_count = pieces
        .iter()
        .filter(|p| matches!(p, inline::Piece::Atom(_)))
        .count();
    if atoms_in_line && atom_count == atom_aligns.len() {
        let mut aligns = atom_aligns.into_iter().flatten();
        let mut at = 0usize;
        let mut out = Vec::with_capacity(pieces.len() + 2 * atom_count);
        let mut mark = inherited.clone();
        mark.word_space_char = None;
        mark.letter_spacing = Some(Len::Px(0.0));
        mark.inline_bg = None;
        mark.inline_border = None;
        for p in pieces {
            match p {
                inline::Piece::Atom(el) => {
                    let (align, extents) = aligns.next().unwrap_or((
                        crate::text::paragraph::AtomAlign::Shift(0.0),
                        crate::text::paragraph::RubyExtents::default(),
                    ));
                    line_atoms.push((at, el, align, extents));
                    out.push(inline::Piece::Text {
                        text: inline::SPACER.to_string(),
                        style: mark.clone(),
                    });
                    at += inline::SPACER.len();
                }
                inline::Piece::Text { text, style } => {
                    at += text.len();
                    out.push(inline::Piece::Text { text, style });
                }
                other => out.push(other),
            }
        }
        pieces = out;
    }
    route_pieces(
        pieces,
        inherited,
        opts,
        native_request,
        indent,
        clamp_budget,
        clamp_tag,
        line_atoms,
    )
}
