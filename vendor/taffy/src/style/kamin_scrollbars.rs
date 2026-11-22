//! KaminIDE patch: classic scrollbars that layout itself decides (CSS Overflow 3 §3,
//! `overflow: auto`). A box lays its content out with the bars it guesses (`overflow`), and
//! when the content's scrollable overflow asks for other bars it lays out again only itself,
//! as Blink `BlockNode::Layout` does when `ComputeScrollbars` changes and Gecko
//! `ScrollContainerFrame::ReflowContents` does with `TryLayout`.
use crate::geometry::{Point, Rect};

/// The layout-decided scrollbars of a box, and the boxes that follow the viewport's,
/// packed in two bytes: node styles keep their size.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ScrollbarLayout(u16);

impl ScrollbarLayout {
    /// No bar decided by layout, no root gutter followed.
    pub const NONE: Self = Self(0);
    /// Layout decides the horizontal bar.
    const AUTO_X: u16 = 1;
    /// Layout decides the vertical bar.
    const AUTO_Y: u16 = 1 << 1;
    /// The box is the viewport.
    const VIEWPORT: u16 = 1 << 2;
    /// The box is a scroll range proxy.
    const PROXY: u16 = 1 << 3;
    /// The box was built for a root gutter.
    const ROOT: u16 = 1 << 4;
    /// [RootGutter::grow].
    const GROW: u16 = 1 << 5;
    /// Shift of [RootGutter::assumed] in the bits.
    const SIDES: u32 = 8;

    /// - `auto`: axes (of `overflow`) whose bar the content decides: `Scroll` with a bar,
    ///   `Hidden` without; `overflow` holds the guess the box is laid out with first.
    /// - `viewport`: the box is the viewport, whose gutter `root_gutter` boxes follow.
    /// - `proxy`: the box adds nothing to the scrollable overflow that decides the parent's
    ///   bars: it sizes the parent's scroll range from a range measured before, or paints a
    ///   canvas under the bars.
    /// - `root_gutter`: an out-of-flow box built for this root gutter.
    pub const fn new(auto: Point<bool>, viewport: bool, proxy: bool, root_gutter: Option<RootGutter>) -> Self {
        const fn flag(on: bool, bit: u16) -> u16 {
            if on {
                bit
            } else {
                0
            }
        }
        let root = match root_gutter {
            Some(root) => Self::ROOT | flag(root.grow, Self::GROW) | (root.assumed as u16 & 0xf) << Self::SIDES,
            None => 0,
        };
        Self(
            flag(auto.x, Self::AUTO_X)
                | flag(auto.y, Self::AUTO_Y)
                | flag(viewport, Self::VIEWPORT)
                | flag(proxy, Self::PROXY)
                | root,
        )
    }

    /// The axes whose bar layout decides.
    #[inline(always)]
    pub fn auto(self) -> Point<bool> {
        Point { x: self.0 & Self::AUTO_X != 0, y: self.0 & Self::AUTO_Y != 0 }
    }

    /// Whether the box is the viewport.
    #[inline(always)]
    pub fn viewport(self) -> bool {
        self.0 & Self::VIEWPORT != 0
    }

    /// Whether the box is a scroll range proxy.
    #[inline(always)]
    pub fn proxy(self) -> bool {
        self.0 & Self::PROXY != 0
    }

    /// The root gutter an out-of-flow box was built for.
    #[inline(always)]
    pub fn root_gutter(self) -> Option<RootGutter> {
        (self.0 & Self::ROOT != 0)
            .then_some(RootGutter { assumed: (self.0 >> Self::SIDES) as u8 & 0xf, grow: self.0 & Self::GROW != 0 })
    }

    /// Whether layout decides any bar of the box or publishes the root gutter.
    #[inline(always)]
    pub fn decides(self) -> bool {
        self.0 & (Self::AUTO_X | Self::AUTO_Y | Self::VIEWPORT) != 0
    }
}
/// The sides of the root gutter an out-of-flow box was built with. Every side of the
/// gutter is one bar thick, so the sides are bits: no node style grows for them.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RootGutter {
    /// Sides its insets already account for: bit 0 left, 1 top, 2 right, 3 bottom.
    pub assumed: u8,
    /// The box covers the gutter (a canvas under the bars) instead of stopping at it.
    pub grow: bool,
}

impl RootGutter {
    /// The sides of a gutter as `assumed` bits.
    pub fn sides(gutter: Rect<f32>) -> u8 {
        [gutter.left, gutter.top, gutter.right, gutter.bottom]
            .iter()
            .enumerate()
            .fold(0, |bits, (side, &width)| if width > 0.0 { bits | 1 << side } else { bits })
    }

    /// How far the box's inset-resolution area moves in on each side when the viewport's
    /// gutter is `now`, its bars `bar` thick: zero when layout kept the bars the box was
    /// built with.
    pub fn shift(&self, now: Rect<f32>, bar: f32) -> Rect<f32> {
        let sign = if self.grow { -1.0 } else { 1.0 };
        let assumed = |side: u8| if self.assumed & 1 << side != 0 { bar } else { 0.0 };
        Rect {
            left: sign * (now.left - assumed(0)),
            top: sign * (now.top - assumed(1)),
            right: sign * (now.right - assumed(2)),
            bottom: sign * (now.bottom - assumed(3)),
        }
    }
}
