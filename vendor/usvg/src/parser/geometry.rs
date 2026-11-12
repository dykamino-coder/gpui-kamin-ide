//! Наблюдение канонической SVG-геометрии без данных в обычном дереве.

use crate::{Rect, Transform};

/// Optional source and layout trace. The ordinary parser uses the zero-sized `()`.
/// IDs identify typed parser nodes, not author attributes or generated SVG IDs.
pub trait GeometryTrace: Clone {
    /// Associates a source element byte offset with its typed parser node.
    fn parsed(&mut self, _node: usize, _parent: usize, _offset: usize) {}
    /// Enters an element coordinate system before converting its children.
    fn group_start(&mut self, _node: usize, _absolute: Transform, _local: Transform) {}
    /// Leaves that element coordinate system before applying paint effects.
    fn group_end(&mut self) {}
    /// Observes an object box before clipping, masking, filtering and stroke.
    fn object(&mut self, _node: usize, _rect: Rect, _transform: Transform) {}
    /// Starts a text layout; character offsets below are local to this element.
    fn text_start(&mut self, _node: usize, _transform: Transform) {}
    /// Associates a UTF-8 cluster offset with the source text-content element.
    fn text_char(&mut self, _node: usize, _chunk: usize, _byte: usize) {}
    /// Observes the full glyph cell after canonical shaping and positioning.
    fn text_cell(&mut self, _chunk: usize, _byte: usize, _rect: Rect, _transform: Transform) {}
    /// Finishes the current text layout, including a layout with no glyphs.
    fn text_end(&mut self) {}
}

impl GeometryTrace for () {}
