//! Метки исходных inline-коробок в канонической стопке стоячих глифов.

use super::Glyph;
use crate::{
    layout::{geometry::Collector, options::RenderOpts},
    style::computed::BoxStyle,
};
use gpui::{Bounds, Div, LayoutId, LayoutObserver, Window, point, px, size};
use std::ops::Range;

pub(super) trait Trace: Default {
    fn inline(&mut self, _: u64, _: &BoxStyle, _: Range<usize>, _: f32) {}
    fn observe(self, root: Div, _: &[Glyph], _: f32, _: &RenderOpts) -> Div {
        root
    }
}

impl Trace for () {}

struct Mark {
    node: u64,
    style: BoxStyle,
    range: Range<usize>,
    shift: f32,
}

#[derive(Default)]
pub(super) struct Capture(Vec<Mark>);

impl Trace for Capture {
    fn inline(&mut self, node: u64, style: &BoxStyle, range: Range<usize>, shift: f32) {
        self.0.push(Mark {
            node,
            style: style.clone(),
            range,
            shift,
        });
    }

    fn observe(self, root: Div, glyphs: &[Glyph], under: f32, opts: &RenderOpts) -> Div {
        let mut advances = vec![0.];
        for glyph in glyphs {
            advances.push(advances.last().unwrap() + glyph.size);
        }
        root.also_observe_layout(Observe {
            marks: self.0,
            advances,
            under,
            opts: opts.clone(),
            layout: None,
        })
    }
}

struct Observe {
    marks: Vec<Mark>,
    advances: Vec<f32>,
    under: f32,
    opts: RenderOpts,
    layout: Option<LayoutId>,
}

impl LayoutObserver for Observe {
    fn request_layout(&mut self, id: LayoutId, _: &Window) {
        self.layout = Some(id);
    }

    fn prepaint(&mut self, window: &mut Window) -> bool {
        let Some(layout) = self.layout else {
            return true;
        };
        let origin = window.layout_origin_unrounded(layout);
        for mark in &self.marks {
            let metrics = crate::text::inline::inline_box::font_metrics(
                &mark.style,
                &self.opts.text,
                self.opts.base_size(),
                self.opts.cx.vcx(),
                window,
            );
            let width = metrics.0 + metrics.1;
            let y = self.advances[mark.range.start];
            let height = self.advances[mark.range.end] - y;
            let bounds = Bounds::new(
                origin + point(px(self.under + mark.shift - width / 2.), px(y)),
                size(px(width), px(height)),
            );
            Collector::record(
                window,
                mark.node,
                [bounds; 3],
                crate::layout::geometry::font(
                    &mark.style,
                    self.opts.base_size(),
                    self.opts.cx.vcx(),
                ),
            );
        }
        true
    }
}
