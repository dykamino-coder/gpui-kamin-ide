//! Выключка строк: CSS абзаца и геометрия отдельного внешнего маркера.

use crate::layout::options::RenderOpts;
use crate::style::computed::{Computed, TextAlign};
use crate::text::paragraph::Align;

pub(super) fn marker(opts: &RenderOpts) -> Option<TextAlign> {
    // CSS Lists 3 §3.1: text-align наследуется текстом ::marker, но не
    // управляет его коробкой. Внешние строки заканчиваются у пункта даже
    // при противоположном direction самого ::marker.
    opts.cx.inline.outside_marker_rtl.map(|rtl| {
        if rtl {
            TextAlign::Left
        } else {
            TextAlign::Right
        }
    })
}

pub(super) fn first(style: &Computed, opts: &RenderOpts) -> Align {
    marker(opts).map_or_else(
        || crate::text::paragraph::align_for(style),
        crate::text::paragraph::align_of_value,
    )
}

pub(super) fn last(style: &Computed, opts: &RenderOpts) -> Option<Align> {
    marker(opts)
        .or_else(|| {
            style
                .inh
                .text_flow
                .text_align_last
                .flatten()
                .map(|a| a.physical(style.wm().rtl()))
        })
        .map(crate::text::paragraph::align_of_value)
        .map(|a| match a {
            // CSS Text 3 §7.3: text-justify:none запрещает растяжение последней строки.
            Align::Justify if style.inh.text_flow.no_justify == Some(true) => {
                if style.wm().rtl() {
                    Align::Right
                } else {
                    Align::Left
                }
            }
            other => other,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::cascade::inherit::inherit;
    use crate::style::computed::{BoxStyle, writing_mode::WritingModeKw};

    #[test]
    fn outside_marker_keeps_inherited_alignment_but_positions_every_line_at_item_edge() {
        let doc = crate::document::state::DocState::new(Default::default());
        let vcx = doc.values();
        for mode in [WritingModeKw::HorizontalTb, WritingModeKw::VerticalRl] {
            for item_rtl in [false, true] {
                for marker_rtl in [false, true] {
                    for align in [
                        TextAlign::Start,
                        TextAlign::End,
                        TextAlign::Center,
                        TextAlign::Justify,
                    ] {
                        let mut item = Computed::default();
                        item.inh.text_flow.text_align = Some(align);
                        item.inh.text_flow.text_align_last = Some(Some(align));
                        item.inh.writing.rtl = Some(item_rtl);
                        item.inh.writing.writing_mode = Some(mode);
                        let item = BoxStyle::new(item);
                        let mut own = Computed::default();
                        own.inh.writing.rtl = Some(marker_rtl);
                        let marker = inherit(&item, &own, vcx);
                        assert_eq!(
                            marker.inh.text_flow.text_align,
                            item.inh.text_flow.text_align
                        );
                        assert_eq!(
                            marker.inh.text_flow.text_align_last,
                            item.inh.text_flow.text_align_last
                        );
                        let before = (*marker).clone();
                        let mut opts = RenderOpts {
                            text: Default::default(),
                            viewport: (800., 600.),
                            normal_line_height: 1.2,
                            cx: Default::default(),
                            frag: Default::default(),
                        };
                        opts.cx.inline.outside_marker_rtl = Some(item_rtl);
                        let expected = if item_rtl { Align::Left } else { Align::Right };
                        assert_eq!(first(&marker, &opts), expected);
                        assert_eq!(last(&marker, &opts), Some(expected));
                        assert_eq!(*marker, before);
                        opts.cx.inline.outside_marker_rtl = None;
                        assert_eq!(
                            first(&marker, &opts),
                            crate::text::paragraph::align_for(&marker)
                        );
                    }
                }
            }
        }
    }
}
