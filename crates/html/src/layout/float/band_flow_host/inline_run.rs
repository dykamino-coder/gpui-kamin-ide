//! Which inline runs between blocks the measured float host can lay out:
//! a run of atoms of known size (`FlowRow`), or a paragraph (text, `<br>`,
//! empty atomic boxes the paragraph sizes).

use super::subtree_has_text;
use crate::dom::Node;
use crate::layout::float::band_host::{BandPiece, band_piece};
use crate::render::{inline_level, is_blank, out_of_flow, replaced_tag};
use crate::style::computed::Display;

/// `(atoms, hostable)`: the run holds only atoms of known size, and the run
/// can be laid out by the host at all.
pub(super) fn classify(run: &[Node]) -> (bool, bool) {
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
            // CSS 2.1 sections 9.2.2, 10.3.9: atomic inline containers
            // are measured by the paragraph even without inline text.
            // Blink line_breaker.cc HandleAtomicInline lays them out first.
            // Ordinary inline fragments still need their own float lifting.
            let container = c.style.inline_display != Some(true)
                && matches!(
                    c.style.display,
                    Some(
                        Display::InlineBlock
                            | Display::InlineFlex
                            | Display::InlineGrid
                            | Display::InlineTable
                    )
                )
                && band_piece(n) != Some(BandPiece::Atom);
            out_of_flow(&c.style)
                || container
                || inline_level(c)
                && band_piece(n) != Some(BandPiece::Atom)
                // After inline floats are lifted, the empty in-flow fragment
                // still owns its margin and current line (CSS2 sections 9.2.2, 9.5).
                && (replaced_tag(c)
                    || subtree_has_text(c)
                    || c.children.iter().all(is_blank))
        }
    }) || run
        .iter()
        .all(|n| is_blank(n) || matches!(n, Node::Element(e) if e.tag == "br"));
    let atoms = !text
        && run
            .iter()
            .all(|n| is_blank(n) || band_piece(n) == Some(BandPiece::Atom));
    // Empty atomic inline boxes of unknown size (an empty `<button>`,
    // an empty inline-block) and `<br>`: the paragraph sizes them.
    let empty_boxes = run.iter().all(|n| match n {
        Node::Text(_) => is_blank(n),
        Node::Element(c) => {
            c.tag == "br"
                || band_piece(n) == Some(BandPiece::Atom)
                || (!replaced_tag(c)
                    && c.children.iter().all(is_blank)
                    && matches!(
                        c.style.display,
                        Some(Display::InlineBlock)
                            | Some(Display::InlineFlex)
                            | Some(Display::InlineGrid)
                    ))
        }
    });
    (atoms, text || atoms || empty_boxes)
}
