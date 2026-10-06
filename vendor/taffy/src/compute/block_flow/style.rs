//! Project block styles into a logical coordinate frame while retaining native sizing contracts.
use crate::style::*;
use crate::{Point, Rect, Size};

pub(super) struct Logical<S> {
    pub style: S,
    pub flow: BlockFlow,
}

macro_rules! forward {
    ($($name:ident -> $result:ty),* $(,)?) => { $(
        fn $name(&self) -> $result { self.style.$name() }
    )* };
}

impl<S: CoreStyle> CoreStyle for Logical<S> {
    type CustomIdent = S::CustomIdent;
    forward!(
        box_generation_mode -> BoxGenerationMode,
        is_compressible_replaced -> bool,
        box_sizing -> BoxSizing,
        scrollbar_width -> f32,
        position -> Position,
        safe_alignment -> (bool, bool, bool, bool),
        contain -> Contain,
        grid_axis_reversed -> Option<Size<bool>>,
        baseline_x_hint -> Option<(f32, bool)>,
        baseline_x_flags -> u8,
    );
    fn is_block(&self) -> bool {
        self.style.is_block()
            && self.style.block_flow().map_or(!self.flow.vertical, |flow| {
                flow.vertical == self.flow.vertical && flow.block_reverse == self.flow.block_reverse
            })
    }
    fn direction(&self) -> Direction {
        if self.flow.inline_reverse {
            Direction::Rtl
        } else {
            Direction::Ltr
        }
    }
    fn overflow(&self) -> Point<Overflow> {
        self.flow.point(self.style.overflow())
    }
    fn size(&self) -> Size<Dimension> {
        self.flow.size(self.style.size())
    }
    fn min_size(&self) -> Size<LengthPercentageAuto> {
        self.flow.size(self.style.min_size())
    }
    fn max_size(&self) -> Size<LengthPercentageAuto> {
        self.flow.size(self.style.max_size())
    }
    fn aspect_ratio_preferred_size(&self) -> Size<Option<f32>> {
        self.flow.size(self.style.aspect_ratio_preferred_size())
    }
    fn aspect_ratio(&self) -> Option<f32> {
        self.style.aspect_ratio().map(|ratio| {
            if self.flow.vertical {
                ratio.recip()
            } else {
                ratio
            }
        })
    }
    fn inset(&self) -> Rect<LengthPercentageAuto> {
        self.flow.logical_edges(self.style.inset())
    }
    fn margin(&self) -> Rect<LengthPercentageAuto> {
        self.flow.logical_edges(self.style.margin())
    }
    fn padding(&self) -> Rect<LengthPercentage> {
        self.flow.logical_edges(self.style.padding())
    }
    fn border(&self) -> Rect<LengthPercentage> {
        self.flow.logical_edges(self.style.border())
    }
    fn calc_size(&self) -> [Option<(f32, f32, f32, f32)>; 4] {
        let [width, height, min_width, min_height] = self.style.calc_size();
        if self.flow.vertical {
            [height, width, min_height, min_width]
        } else {
            [width, height, min_width, min_height]
        }
    }
    fn margin_trim(&self) -> u8 {
        let value = self.style.margin_trim();
        if !self.flow.vertical {
            return value;
        }
        let mappings = if self.flow.block_reverse {
            [(1, 8), (2, 1), (4, 2), (8, 4)]
        } else {
            [(1, 8), (2, 4), (4, 2), (8, 1)]
        };
        mappings.into_iter().fold(0, |bits, (physical, logical)| {
            bits | if value & physical != 0 { logical } else { 0 }
        })
    }
}

impl<S: BlockContainerStyle> BlockContainerStyle for Logical<S> {
    forward!(text_align -> TextAlign, align_content -> Option<AlignContent>);
}

impl<S: BlockItemStyle> BlockItemStyle for Logical<S> {
    forward!(is_table -> bool);
    #[cfg(feature = "float_layout")]
    forward!(float -> Float, clear -> Clear);
}
