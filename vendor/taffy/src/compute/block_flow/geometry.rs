//! Reversible physical/logical conversion for native block constraints and results.
use crate::tree::{AxisStaticEdge, AxisStaticPosition, OofCandidates, OofPositioningArea};
use crate::{
    Baselines, BlockFlow, Layout, LayoutInput, LayoutOutput, Line, Point, Rect, RequestedAxis, Size,
};

impl BlockFlow {
    pub(super) fn size<T>(self, value: Size<T>) -> Size<T> {
        if self.vertical {
            Size {
                width: value.height,
                height: value.width,
            }
        } else {
            value
        }
    }

    pub(super) fn point<T>(self, value: Point<T>) -> Point<T> {
        if self.vertical {
            Point {
                x: value.y,
                y: value.x,
            }
        } else {
            value
        }
    }

    pub(super) fn logical_edges<T>(self, v: Rect<T>) -> Rect<T> {
        if !self.vertical {
            return v;
        }
        if self.block_reverse {
            Rect {
                left: v.top,
                right: v.bottom,
                top: v.right,
                bottom: v.left,
            }
        } else {
            Rect {
                left: v.top,
                right: v.bottom,
                top: v.left,
                bottom: v.right,
            }
        }
    }

    pub(super) fn physical_edges<T>(self, v: Rect<T>) -> Rect<T> {
        if !self.vertical {
            return v;
        }
        if self.block_reverse {
            Rect {
                left: v.bottom,
                right: v.top,
                top: v.left,
                bottom: v.right,
            }
        } else {
            Rect {
                left: v.top,
                right: v.bottom,
                top: v.left,
                bottom: v.right,
            }
        }
    }

    pub(super) fn input(self, mut value: LayoutInput) -> LayoutInput {
        if !self.vertical {
            return value;
        }
        value.known_dimensions = self.size(value.known_dimensions);
        value.known_dimensions_are_definite = self.size(value.known_dimensions_are_definite);
        value.parent_size = self.size(value.parent_size);
        value.available_space = self.size(value.available_space);
        value.axis = match value.axis {
            RequestedAxis::Horizontal => RequestedAxis::Vertical,
            RequestedAxis::Vertical => RequestedAxis::Horizontal,
            RequestedAxis::Both => RequestedAxis::Both,
        };
        value
    }

    pub(super) fn output(self, mut value: LayoutOutput) -> LayoutOutput {
        if !self.vertical {
            return value;
        }
        // Out-of-flow candidates and the positioning area leave the logical block algorithm
        // here; the containing block's positioning pass (`compute_oof_layout`) is physical.
        let logical_size = value.size;
        self.physical_candidates(&mut value.oof_candidates, logical_size);
        value.oof_positioning_area = value.oof_positioning_area.map(|area| OofPositioningArea {
            size: self.size(area.size),
            offset: Point {
                x: if self.block_reverse {
                    logical_size.height - area.offset.y - area.size.height
                } else {
                    area.offset.y
                },
                y: area.offset.x,
            },
        });
        value.size = self.size(value.size);
        let x = value.baselines_x;
        value.baselines_x = reflect(value.baselines, value.size.width, self.block_reverse);
        value.baselines = x;
        // The inline-block channel is a horizontal-flow y offset only.
        value.inline_block_last_y = None;
        #[cfg(feature = "content_size")]
        {
            value.scrollable_overflow_rect = transpose_rect(value.scrollable_overflow_rect);
        }
        value
    }

    pub(super) fn logical_output(self, mut value: LayoutOutput) -> LayoutOutput {
        if !self.vertical {
            return value;
        }
        // Candidates bubbling out of a physical child become logical (inverse of `output`).
        let physical_size = value.size;
        for candidate in value.oof_candidates.as_mut_slice() {
            let sp = candidate.static_position;
            candidate.static_position = Point {
                x: sp.y,
                y: if self.block_reverse {
                    mirror(sp.x, physical_size.width)
                } else {
                    sp.x
                },
            };
        }
        let x = reflect(value.baselines_x, value.size.width, self.block_reverse);
        value.baselines_x = value.baselines;
        value.baselines = x;
        value.inline_block_last_y = None;
        value.size = self.size(value.size);
        #[cfg(feature = "content_size")]
        {
            value.scrollable_overflow_rect = transpose_rect(value.scrollable_overflow_rect);
        }
        value
    }

    pub(super) fn layout(self, mut value: Layout, container: Size<f32>) -> Layout {
        if !self.vertical {
            return value;
        }
        value.location = Point {
            x: if self.block_reverse {
                container.height - value.location.y - value.size.height
            } else {
                value.location.y
            },
            y: value.location.x,
        };
        value.size = self.size(value.size);
        value.scrollbar_size = self.size(value.scrollbar_size);
        value.border = self.physical_edges(value.border);
        value.padding = self.physical_edges(value.padding);
        value.margin = self.physical_edges(value.margin);
        #[cfg(feature = "content_size")]
        {
            value.scrollable_overflow_rect = transpose_rect(value.scrollable_overflow_rect);
        }
        value
    }
}

impl BlockFlow {
    /// Logical static positions (relative to the logical border box of `logical_size`) to
    /// physical ones, mirroring `layout`: logical x -> physical y, logical y -> physical x
    /// (reflected when the block axis runs right-to-left).
    fn physical_candidates(self, candidates: &mut OofCandidates, logical_size: Size<f32>) {
        for candidate in candidates.as_mut_slice() {
            let sp = candidate.static_position;
            candidate.static_position = Point {
                x: if self.block_reverse {
                    mirror(sp.y, logical_size.height)
                } else {
                    sp.y
                },
                y: sp.x,
            };
        }
    }
}

/// Reflect a static position within `[0, extent]`: the area flips and so do start/end edges.
fn mirror(value: AxisStaticPosition, extent: f32) -> AxisStaticPosition {
    fn flip(edge: AxisStaticEdge) -> AxisStaticEdge {
        match edge {
            AxisStaticEdge::Start => AxisStaticEdge::End,
            AxisStaticEdge::End => AxisStaticEdge::Start,
            AxisStaticEdge::Center => AxisStaticEdge::Center,
        }
    }
    let mut out = value;
    out.area = Line {
        start: extent - value.area.end,
        end: extent - value.area.start,
    };
    out.align.keyword = flip(value.align.keyword);
    out.align.fallback = flip(value.align.fallback);
    out
}

fn reflect(value: Baselines, extent: f32, reverse: bool) -> Baselines {
    if reverse {
        Baselines {
            first: value.first.map(|v| extent - v),
            last: value.last.map(|v| extent - v),
        }
    } else {
        value
    }
}

#[cfg(feature = "content_size")]
fn transpose_rect(v: Rect<f32>) -> Rect<f32> {
    Rect {
        left: v.top,
        right: v.bottom,
        top: v.left,
        bottom: v.right,
    }
}
