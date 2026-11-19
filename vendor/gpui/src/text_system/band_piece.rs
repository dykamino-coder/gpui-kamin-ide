//! KaminIDE patch: one piece of an inline box's band on a line, painted the
//! same way the first time (`paint_line_background`) and when the box's
//! paint is recorded again (`Window::rerecord_layer_band`).
//!
//! A piece is where an inline box meets one line: its place (the segment of
//! the line, `BandEdges`) is layout, its paint (`BandStyle`) is style. In a
//! scene layer every piece of a box with a band, or of a box that could take
//! one, is a run of that box (`Window::with_box_paint`), an empty one when it
//! paints nothing, so a band that appears, goes or changes its colours or
//! corners is painted again into those runs alone, in the order of the line's
//! stacking layer. Blink paints the whole block's fragment items again for
//! any change of one inline box (`InlineBoxFragmentPainter`, the client's
//! display items cached and copied); Gecko builds the inline frame's
//! `nsDisplayBackgroundColor` items again in a partial display list update.
//! Here only the box's own pieces are painted, and nothing else is copied.

use crate::{Bounds, Hsla, Pixels, Point, Window, size};

/// The paint of an inline box's band: its colour (`None`: no band), padding
/// (top, right, bottom, left), corner radius and border colour and widths.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BandStyle {
    /// The fill (`None`: no band).
    pub color: Option<Hsla>,
    /// Padding, physical sides.
    pub pad: [Pixels; 4],
    /// The corner radius.
    pub radius: Pixels,
    /// The border colour and widths, physical sides.
    pub border: Option<(Hsla, [Pixels; 4])>,
}

/// What a piece's place on its line adds to its segment (`BoxEnv::bounds`:
/// the segment's start, width and the line's height): the line's content
/// height as a band spans it, whether the box starts or ends here (only
/// those ends take its padding and side borders), and which sides of padding
/// (bits 0-3: top, right, bottom, left) and border (bits 4-7) the piece
/// paints: a piece cut where the box goes on in another piece (a wrap, a
/// visual run of a bidi line) paints no side there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BandEdges {
    pub(crate) content_height: Pixels,
    pub(crate) start: bool,
    pub(crate) end: bool,
    pub(crate) sides: u8,
}

impl BandEdges {
    /// The sides `style` paints.
    pub(crate) fn sides_of(style: &BandStyle) -> u8 {
        let mut sides = 0u8;
        for i in 0..4 {
            if style.pad[i] != Pixels::ZERO {
                sides |= 1 << i;
            }
            if style.border.is_some_and(|(_, w)| w[i] != Pixels::ZERO) {
                sides |= 1 << (4 + i);
            }
        }
        sides
    }

    /// `style` on this piece: its colours and corners, the padding and
    /// border widths only on the piece's sides.
    fn cut(&self, style: BandStyle) -> BandStyle {
        let side = |bit: usize, v: Pixels| if self.sides & (1 << bit) != 0 { v } else { Pixels::ZERO };
        BandStyle {
            pad: std::array::from_fn(|i| side(i, style.pad[i])),
            border: style.border.map(|(c, w)| (c, std::array::from_fn(|i| side(4 + i, w[i])))),
            ..style
        }
    }
}

/// The segment of a piece on its line and its edges.
pub(crate) struct Piece {
    pub(crate) origin: Point<Pixels>,
    pub(crate) width: Pixels,
    pub(crate) line_height: Pixels,
    pub(crate) edges: BandEdges,
}

impl Piece {
    fn segment(&self) -> Bounds<Pixels> {
        Bounds { origin: self.origin, size: size(self.width, self.line_height) }
    }
}

/// Paint `style` on the piece `segment`/`edges` of inline box `owner`'s band.
/// The band of a box without a border binds its fill as the box's background
/// (`LayerValue::Background`), so a new colour is written into every piece at
/// once (`LayerHandle::set_backgrounds`); with a border on any side fill and
/// border are one quad, which binds nothing.
pub(crate) fn paint_piece_style(segment: Bounds<Pixels>, edges: BandEdges, style: BandStyle, window: &mut Window) {
    let style = edges.cut(style);
    let Some(color) = style.color else { return };
    let quad = super::line::run_background_quad(
        segment.origin,
        segment.size.width,
        segment.size.height,
        edges.content_height,
        color,
        style.pad,
        style.radius,
        edges.start,
        edges.end,
        style.border,
    );
    let quad = super::line::snap_band(quad, window);
    if style.border.is_some() {
        window.paint_quad(quad);
    } else {
        window.with_paint_value(crate::LayerValue::Background, |w| w.paint_quad(quad));
    }
}

/// Paint one piece of the band of inline box `owner` (0: none). In a scene
/// layer the piece is the box's run, also when it paints nothing; such an
/// empty piece is kept only where its line shows, and pieces cut apart by
/// another painter (`Window::paint_split_bands`) are kept for their values
/// only: the box is then not recorded again (`LayerHandle::holds_band`).
pub(crate) fn paint_band(piece: Piece, owner: u32, style: BandStyle, window: &mut Window) {
    let segment = piece.segment();
    if owner == 0 || !window.records_layer() {
        if style.color.is_some() {
            window.with_paint_owner(owner, |w| paint_piece_style(segment, piece.edges, style, w));
        }
        return;
    }
    let shown = window.content_mask().bounds.intersects(&segment);
    if style.color.is_none() && (window.band_split || !shown) {
        // No run of the box here: it is recorded again nowhere.
        window.unhold_band(owner);
        return;
    }
    let edges = (!window.band_split).then_some(piece.edges);
    window.with_band_piece(owner, segment, edges, |w| paint_piece_style(segment, piece.edges, style, w));
}

impl Window {
    /// Paint `f`, whose bands another painter cuts into pieces this line
    /// painter does not own (justified words and their gap fills): the
    /// pieces keep their values, and their boxes are not recorded again.
    pub fn paint_split_bands<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let was = std::mem::replace(&mut self.band_split, true);
        let result = f(self);
        self.band_split = was;
        result
    }
}

