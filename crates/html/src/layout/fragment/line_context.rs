//! Неизменяемые входы измерения строк, сохраняемые каждым путём фрагментации.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::fragment::line_shape::{items_kind, line_content_w};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;
use std::rc::Rc;

#[derive(Clone)]
pub(crate) struct LineFrame {
    pub(crate) inh: BoxStyle,
    pub(crate) w: Option<f32>,
    pub(crate) anon: bool,
    pub(crate) items: u8,
}

#[derive(Clone)]
pub(crate) struct LineCx {
    pub(crate) opts: RenderOpts,
    pub(crate) frames: Vec<LineFrame>,
}

pub(crate) fn with_lines<T>(
    base: &BoxStyle,
    w: Option<f32>,
    opts: &RenderOpts,
    f: impl FnOnce(&RenderOpts) -> T,
) -> T {
    let child = opts.inner(|cx| {
        cx.fragmentation.lines = Some(Rc::new(LineCx {
            // Retain the parent options, avoiding a cycle through this new owner.
            opts: opts.clone(),
            frames: vec![LineFrame {
                inh: base.clone(),
                w,
                anon: false,
                items: 0,
            }],
        }));
    });
    f(&child)
}

impl LineCx {
    pub(crate) fn child(c: &Element, vcx: ValueCx<'_>) -> Option<Rc<Self>> {
        let parent = vcx.lines?;
        let top = parent.frames.last()?;
        let inh = crate::style::cascade::materialize::box_style(&top.inh, &c.style, vcx);
        let w = top.w.and_then(|pw| match top.items {
            2 if !matches!(c.style.computed().width, Some(Len::Px(_))) => None,
            1 if c.style.computed().align_self.is_some()
                || c.style.computed().justify_self.is_some() =>
            {
                None
            }
            _ => line_content_w(c, pw),
        });
        let mut child = (**parent).clone();
        child.frames.push(LineFrame {
            inh,
            w,
            anon: c.tag == "anon-block",
            items: items_kind(c),
        });
        Some(Rc::new(child))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::Computed;
    #[test]
    fn retained_measurement_survives_nested_failure_without_parent_mutation() {
        let opts = RenderOpts {
            text: gpui::TextStyle::default(),
            viewport: (800.0, 600.0),
            normal_line_height: 1.2,
            cx: Default::default(),
            frag: Default::default(),
        };
        let base = Computed::default();
        let retained = with_lines(&BoxStyle::from(base.clone()), Some(120.0), &opts, |outer| {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_lines(&BoxStyle::from(base.clone()), Some(60.0), outer, |inner| {
                        assert_eq!(inner.cx.vcx().lines.unwrap().frames[0].w, Some(60.0));
                        panic!("measurement failed");
                    });
                }))
                .is_err()
            );
            assert_eq!(outer.cx.vcx().lines.unwrap().frames[0].w, Some(120.0));
            outer.cx.vcx().keep()
        });
        assert!(opts.cx.vcx().lines.is_none());
        assert_eq!(retained.vcx().lines.unwrap().frames[0].w, Some(120.0));
    }
}
