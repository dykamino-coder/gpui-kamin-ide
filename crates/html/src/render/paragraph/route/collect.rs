//! Сбор строчных кусков абзаца: атомы с выравниванием по строке, схлопывание и
//! срез пробелов, стили первой строки, отступ первой строки и распорки атомов,
//! поставленных в строку; затем выбор пути (`choose::route_pieces`).

use super::choose::route_pieces;
use super::styling::style_pieces;
use super::{LineAtoms, NativeRequest};
use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::render::paragraph::alignment::{atom_inline_size_fixed, atom_line_align};
use crate::render::paragraph::atom_piece::{AtomRuby, atom_piece};
use crate::render::paragraph::edges::has_flow_text;
use crate::render::paragraph::metrics::atoms_fit_line;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Len;
use crate::text::inline;
use crate::text::ruby::ruby_role;
use gpui::{AnyElement, IntoElement, div};

pub(crate) fn paragraph_pieces_routed(
    nodes: &[Node],
    inherited: &BoxStyle,
    opts: &RenderOpts,
    first_line_at: usize,
    first_line: &Computed,
    native_request: Option<&NativeRequest<'_>>,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    // Бюджет строк знака обрыва (авто-режим) забирается РАЗОМ, до сборки
    // кусков: куски строят вложенные абзацы (`inline-block`, `<svg>`), и
    // чужой бюджет им доставаться не должен.
    let clamp_tag = opts.cx.clamp.paragraph;
    let clamp_budget = clamp_tag.and_then(|(key, seq)| {
        opts.cx
            .clamp_plan(key)?
            .para
            .filter(|(s, _)| *s == seq)
            .map(|(_, k)| k)
    });
    let child_opts = opts.inner_if(clamp_tag.is_some(), |cx| {
        cx.clamp.paragraph = None;
    });
    let opts = &*child_opts;
    // The paragraph kept from the last frame, or the plan of this one's record.
    let plain = native_request.is_none() && clamp_tag.is_none() && first_line_at == 0;
    let keep = match crate::layout::reuse::paragraph::start(
        nodes,
        inherited,
        opts,
        plain,
        super::finish::atom_builder,
    ) {
        Ok(para) => return super::finish::finish(para, inherited, opts),
        Err(keep) => keep,
    };
    // Текстовый путь абзаца — единственное место, где место куска вне потока
    // считается ДВУНАПРАВЛЕННО: строку режет и переставляет разбор UAX#9
    // внутри `lines.rs`, а `point_of` берёт продвижение от начала СВОЕЙ
    // строки. Ряд из слов (`as_wrapped_row`) о направлении не
    // знает вовсе — куски идут в ЛОГИЧЕСКОМ порядке, строка прижимается
    // целиком, и щуп садится в НАЧАЛО rtl-строки.
    //
    // Абзац из одних внепоточных кусков (текста нет) сохраняет фантомный
    // `Paragraph`: он владеет статической точкой, не добавляя ни строки, ни
    // высоты. Поэтому внепоточные коробки горизонтального `direction: rtl`
    // остаются в общем конвейере абзаца, а не уходят в ряд, где кусок
    // теряет и `rtl`, и `next_line`.
    let flow_text = has_flow_text(nodes);
    let atom = |e: &Element,
                atom_cx: &inline::atom_parent::AtomCx,
                ruby: &AtomRuby|
     -> Option<inline::Piece> {
        atom_piece(e, inherited, opts, flow_text, atom_cx, ruby)
    };
    // Каждому атому — признак, можно ли поставить его В СТРОКУ абзаца
    // (`atom_line_align`): порядок записей совпадает с порядком `Piece::Atom`.
    // Руби несёт ещё и узлы своих аннотаций: по ним строка растёт
    // (`lines::ruby_extent`). Чужие узлы (руби вне строки внутри атома) атому
    // не достаются.
    let mut atom_aligns: Vec<
        Option<(
            crate::text::paragraph::AtomAlign,
            crate::text::paragraph::RubyExtents,
            bool,
        )>,
    > = Vec::new();
    let mut atom_noted =
        |e: &Element, atom_cx: &inline::atom_parent::AtomCx| -> Option<inline::Piece> {
            // Атом собирает узлы аннотаций руби, которые заведёт (`AtomRuby`).
            let ruby = AtomRuby::for_atom(e);
            let piece = atom(e, atom_cx, &ruby);
            let extents = ruby.take_extents();
            if matches!(piece, Some(inline::Piece::Atom(_))) {
                keep.iter().for_each(|keep| keep.note_atom(e, atom_cx));
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
                        e.style.used().reset_positioned.position,
                        Some(crate::style::computed::Position::Absolute)
                            | Some(crate::style::computed::Position::Fixed)
                    );
                atom_aligns.push(
                    (!lone_abs)
                        .then(|| atom_line_align(e, inherited, opts))
                        .flatten()
                        .map(|a| (a, extents, atom_inline_size_fixed(e))),
                );
            }
            piece
        };
    let mut pieces = inline::collect(nodes, inherited, &mut atom_noted, &opts.cx);
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
                    .any(|a| a.as_ref().is_some_and(|(_, ex, _)| !ex.is_empty())),
                &opts.cx,
            )
    };
    if atoms_in_line {
        inline::trim_edge_spaces_solid_atoms(&mut pieces);
    } else {
        inline::trim_edge_spaces(&mut pieces);
    }
    pieces = style_pieces(
        pieces,
        inherited,
        first_line_at,
        first_line,
        opts.cx.text_fonts(),
        opts.cx.vcx(),
    );
    // Межсловный интервал и отступ первой строки требуют строки из слов, а
    // единый текстовый блок их не умеет — поэтому решение принимается ДО
    // сборки блока, иначе оба свойства молча пропадали.
    // Отступ первой строки. Абсолютную часть разбор уже свёл к точкам, доля
    // же берётся от ширины содержащего блока и здесь ещё неизвестна — её
    // считает раскладка строк, когда ширина решена.
    // `calc(50% - 3px)` несёт обе части разом: `Paragraph` складывает
    // `px + pct × ширина строки` сам (`lines.rs`, css-text-3 §2.1).
    let mixed = match inherited.inh.text_flow.text_indent.clone() {
        Some(Len::Calc(i)) => crate::style::values::value::calc_get(i, vcx).pct_px(),
        _ => None,
    };
    let indent = crate::text::paragraph::Indent {
        px: match inherited.inh.text_flow.text_indent {
            Some(Len::Px(v)) => v,
            _ => mixed.map_or(0.0, |(_, px)| px),
        },
        pct: match inherited.inh.text_flow.text_indent {
            Some(Len::Pct(k)) => k,
            _ => mixed.map_or(0.0, |(pct, _)| pct),
        },
        each_line: inherited.inh.text_flow.text_indent_each_line == Some(true),
        hanging: inherited.inh.text_flow.text_indent_hanging == Some(true),
    };
    // Word spacing and the first-line indent (negative ones included)
    // are computed by the line layout itself (`lines::rules`): no row of words is made for them.
    // Атомы — В СТРОКУ абзаца (CSS 2.1 §9.2.2, §10.8): каждый становится
    // распоркой (U+FEFF), её продвижение — ширина атома, а сам элемент
    // раскладывает и ставит на базовую линию своей строки `Paragraph`. Для
    // переноса распорка атома читается как U+FFFC (`Paragraph::linebreaks`):
    // так атом кладёт и Blink (`inline_items_builder.cc`, знак-заместитель
    // объекта), а класс CB даёт разрыв до и после (UAX #14 LB20).
    // Прежде абзац с атомом уходил в гибкий ряд слов (`as_wrapped_row`): одна
    // высота строки на ряд, базовая линия текста taffy не видна, `top`/
    // `bottom`/`text-top` не выражались.
    let mut line_atoms: LineAtoms = Vec::new();
    let atom_count = pieces
        .iter()
        .filter(|p| matches!(p, inline::Piece::Atom(_)))
        .count();
    if atoms_in_line && atom_count == atom_aligns.len() {
        let mut aligns = atom_aligns.into_iter().flatten();
        let mut at = 0usize;
        let mut out = Vec::with_capacity(pieces.len() + 2 * atom_count);
        let mut mark = super::spacer::atom_spacer(inherited, opts);
        mark.inline_decoration.band_color = None;
        mark.inline_decoration.border_stroke = None;
        for p in pieces {
            match p {
                inline::Piece::Atom(el) => {
                    let (align, extents, fixed) = aligns.next().unwrap_or((
                        crate::text::paragraph::AtomAlign::Shift(0.0),
                        crate::text::paragraph::RubyExtents::default(),
                        false,
                    ));
                    line_atoms.push((at, el, align, extents, fixed));
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
    let keep = keep.and_then(|keep| keep.seal(nodes, flow_text));
    let doc = opts.cx.doc();
    let shared = doc.boxes.borrow().flow_text(nodes, inherited.self_node);
    route_pieces(
        pieces,
        inherited,
        opts,
        native_request,
        indent,
        clamp_budget,
        clamp_tag,
        line_atoms,
        keep,
        shared,
    )
}
