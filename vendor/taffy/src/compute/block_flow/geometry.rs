//! Reversible physical/logical conversion for native block constraints and results.
use crate::{
    Baselines, BlockFlow, Layout, LayoutInput, LayoutOutput, Point, Rect, RequestedAxis, Size,
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
        value.size = self.size(value.size);
        let x = value.baselines_x;
        value.baselines_x = reflect(value.baselines, value.size.width, self.block_reverse);
        value.baselines = x;
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
        let x = reflect(value.baselines_x, value.size.width, self.block_reverse);
        value.baselines_x = value.baselines;
        value.baselines = x;
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
