//! Подъём хвоста потока в полосы: внутренний поток без полос, прижатые абсолюты.

use super::{band_flow_block, subtree_has_text};
use crate::dom::{Element, Node};
use crate::layout::float::band_host::{BandPiece, band_piece};
use crate::layout::float::band_measured::{band_nest_ok, band_piece_m, has_ruby};
use crate::render::{
    block_level_in_flow, inline_level, is_blank, out_of_flow, own_context, replaced_tag,
};
use crate::style::computed::Computed;
use crate::style::computed::writing_mode::WritingMode;
use crate::style::values::value::Len;

/// Внутри блока потока нет ни флоатов, ни блоков своего контекста — на
/// любой глубине обычного потока. Блок хоста раскладывается своим корнем, и
/// полосы доходят только до ЕГО строк (`flow_shapes`); вложенный флоат или
/// коробка своего контекста внешних флоатов не увидели бы вовсе — им нужен
/// один `FloatBands` на весь БФК (шаг F7: `floats-rule7-outside-left-001`,
/// `floats-wrap-bfc-with-margin-008`, `second-float-inside-empty-cleared-block`).
pub(super) fn flow_interior_plain(c: &Element) -> bool {
    c.children.iter().all(|n| match n {
        Node::Text(_) => true,
        Node::Element(k) => {
            if k.style.float.is_some_and(|f| f != 0) {
                return false;
            }
            // `<br style="clear">` под срезом конца строки по НЕ-текстовому
            // краю (`text-box-trim: trim-end` + `text-box-edge: … alphabetic`):
            // css-inline-3 §text-box-trim — срез не трогает clearance, конец
            // блока = max(срезанная строка, низ флоатов). Блок потока хоста
            // срезает строку, а clearance разрыва не видит — такой блок
            // уходит на прежний путь (`text-box-trim-float-clear-br-003`).
            // Срез по краю `text` (у Ahem он нулевой) хост держит верно:
            // гейт на любой `clear` отправлял и его на прежний путь, и
            // `text-box-trim-float-clear-br-001` терял 0.00 → 16.21.
            if trimmed_clear(c, k) {
                return false;
            }
            if block_level_in_flow(k) {
                // Вложенный `clear` тоже упирается во ВНЕШНИЕ флоаты
                // (`clear-applies-to-009`: `<div><span display:block;
                // clear:both>`).
                !own_context(k)
                    && k.tag != "table"
                    && k.style.clear.is_none()
                    && flow_interior_plain(k)
            } else {
                // Строчный: атом в строке (инлайн-блок) режется вырезом как
                // целое; флоат внутри строчного всплывает к блоку (§10.1).
                inline_level(k) || flow_interior_plain(k)
            }
        }
    })
}

/// A child with `clear` (a `<br clear>`) in a block whose last line is
/// trimmed to a non-text edge (css-inline-3 §text-box-trim): the trim must
/// not move the clearance, which the measured host's runs do not express.
pub(in crate::layout::float) fn trimmed_clear(c: &Element, k: &Element) -> bool {
    k.style.clear.is_some()
        && c.style.text_box_trim_end
        && c.style.text_box_under != crate::style::computed::TextEdge::Text
}

/// Абсолютная коробка с заданными вставками по обеим осям: её место от
/// статической позиции не зависит (CSS 2.1 §10.3.7/§10.6.4 — `auto` нет ни
/// у `left`/`right`, ни у `top`/`bottom` разом), и её можно вынести из
/// хоста в поток содержащего блока, ничего не сдвинув.
pub(super) fn abs_pinned(c: &Computed) -> bool {
    let set = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
    matches!(
        c.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    ) && (set(c.inset.top) || set(c.inset.bottom))
        && (set(c.inset.left) || set(c.inset.right))
}

/// `band_flow_rest`, где абсолюты с заданными вставками (`abs_pinned`) не
/// отменяют хост, а уходят в `lift` — вызывающий кладёт их за хостом.
/// Свой содержащий блок они находят снаружи хоста: внутри его отдельного
/// дерева абсолют встал бы от держателя (`floats-placement-001`: зелёная
/// заплатка `left: 50px` у `position: relative` контейнера).
pub(crate) fn band_flow_rest_lift(
    rest: Vec<Node>,
    em: f32,
    mut lift: Option<&mut Vec<Node>>,
    wm: WritingMode,
) -> Option<Vec<Node>> {
    let mut out: Vec<Node> = vec![];
    let mut run: Vec<Node> = vec![];
    // A run of atoms of known size only is laid out by `FlowRow`, a run with
    // text is a paragraph; any other run (replaced elements of unknown size,
    // empty inline boxes) keeps the floats off the host.
    let mut bad = false;
    // Прогон после `<br>` — продолжение того же абзаца (`cont`): без отступа
    // первой строки (`band_kids`).
    let mut cont = false;
    // `clear` of the last `<br>`: the content after it starts below the
    // floats of that side (HTML `br[clear]`, CSS 2.1 §9.5.2 on the following
    // anonymous run); with nothing after it, an empty run still takes the
    // clearance so the host's flow ends below those floats.
    let mut pending_clear: Option<i8> = None;
    let clearing = |clear: Option<i8>| {
        Node::Element(Element::synthetic(
            "div",
            Computed {
                clear,
                ..Computed::default()
            },
            crate::dom::Marks {
                anon: true,
                ..Default::default()
            },
            Vec::new(),
        ))
    };
    let mut flush = |run: &mut Vec<Node>, out: &mut Vec<Node>, cont: bool, clear: Option<i8>| {
        if run.iter().any(|n| !is_blank(n)) {
            // `<br>` — строка, пусть и пустая: прогон `[<br>]` после разреза
            // по `<br>` — строчный, а не «пустой» (иначе хост отменялся).
            // Строчный элемент с текстом внутри (`<span>Inline box</span>`)
            // — тоже строки: прогон без ГОЛОГО текста отменял хост, и флоат
            // за таким прогоном уходил на следующую строку вместе с блоком
            // (`box-generation-002`: флоат обязан встать на строку прогона
            // слева от неё, §9.5.1 правило 6).
            let text = run.iter().any(|n| match n {
                Node::Text(t) => !t.trim().is_empty(),
                Node::Element(c) => {
                    inline_level(c)
                        && band_piece(n) != Some(BandPiece::Atom)
                        && !replaced_tag(c)
                        && subtree_has_text(c)
                }
            }) || run
                .iter()
                .all(|n| is_blank(n) || matches!(n, Node::Element(e) if e.tag == "br"));
            let atoms = !text
                && run
                    .iter()
                    .all(|n| is_blank(n) || band_piece(n) == Some(BandPiece::Atom));
            if !text && !atoms {
                bad = true;
            }
            if atoms && wm.vertical {
                bad = true;
            }
            // Вырезы строк (`lines.rs` `flow_cut`) считают строки равной
            // высоты `line_no × line-height`; руби поднимает строку на
            // аннотацию, и вырез уезжает с неё
            // (`initial-letter-block-position-raise-over-ruby-ref`).
            if run.iter().any(has_ruby) {
                bad = true;
            }
            out.push(Node::Element(Element::synthetic(
                "div",
                Computed {
                    clear,
                    ..Computed::default()
                },
                crate::dom::Marks {
                    anon: true,
                    atoms,
                    cont,
                    ..Default::default()
                },
                std::mem::take(run),
            )));
            true
        } else {
            run.clear();
            false
        }
    };
    for n in rest {
        match &n {
            Node::Text(_) => run.push(n),
            Node::Element(c) => {
                if out_of_flow(&c.style) {
                    if let Some(l) = lift.as_deref_mut()
                        && abs_pinned(&c.style)
                    {
                        l.push(n);
                        continue;
                    }
                    return None;
                }
                if !block_level_in_flow(c) {
                    // `<br clear>`: the break ends the run, the next run
                    // clears (`pending_clear`).
                    if c.style.clear.is_some() {
                        let mut br = c.clone();
                        br.style.clear = None;
                        run.push(Node::Element(br));
                        flush(&mut run, &mut out, cont, pending_clear.take());
                        pending_clear = c.style.clear;
                        cont = true;
                        continue;
                    }
                    // Прогон режется по `<br>` верхнего уровня: §9.5 сдвигает
                    // под флоат СТРОКУ, в которую ничего не влезло, а план
                    // умеет сдвигать только прогон целиком. Строка `<br>`
                    // остаётся рядом с флоатом, а слово за ним, не влезшее в
                    // окно, уходит под флоат своим прогоном
                    // (`float-no-content-beside-001-ref`: `<span float>` +
                    // `<br>` + длинное слово — прогон целиком съезжал под
                    // флоат вместе с пустой строкой `<br>`, на строку ниже).
                    // Вертикальный хост: прогон из одних `<br>` не отделяется от
                    // следующего содержимого. Вертикальные строки не режутся
                    // вырезами полос по одной, и пустой прогон перед текстом
                    // мерился блочным размером флоата: весь текст за ним
                    // вставал ПОСЛЕ буквицы вместо обтекания
                    // (`initial-letter-sunk-initial-vrl`: `3 2` даёт `<br>`
                    // осадки сразу за буквицей).
                    let only_breaks = wm.vertical
                        && run
                            .iter()
                            .all(|n| is_blank(n) || matches!(n, Node::Element(e) if e.tag == "br"));
                    let br = c.tag == "br";
                    run.push(n);
                    if br && !only_breaks && flush(&mut run, &mut out, cont, pending_clear) {
                        pending_clear = None;
                    }
                    if br && !only_breaks {
                        cont = true;
                    }
                    continue;
                }
                if !flush(&mut run, &mut out, cont, pending_clear) && pending_clear.is_some() {
                    out.push(clearing(pending_clear));
                }
                pending_clear = None;
                cont = false;
                if band_piece_m(&n, em, wm).is_some()
                    || band_flow_block(c, em)
                    || band_nest_ok(c, em, wm)
                {
                    out.push(n);
                } else {
                    return None;
                }
            }
        }
    }
    if !flush(&mut run, &mut out, cont, pending_clear) && pending_clear.is_some() {
        out.push(clearing(pending_clear));
    }
    (!bad).then_some(out)
}
