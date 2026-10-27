//! Сборка paint-ресурса группы: исходный порядок ICB/fixed и полосы таблицы.
use crate::dom::Node;
use crate::layout::fragment::paginate::watch::Watched;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, px};
use std::rc::Rc;

pub(super) struct Paint<'a> {
    pub(super) opts: &'a RenderOpts,
    pub(super) root: &'a BoxStyle,
    pub(super) group: &'a [Node],
    pub(super) watched: &'a Rc<Watched>,
    pub(super) padding: (f32, f32, f32),
    pub(super) height: f32,
    pub(super) first: bool,
    pub(super) vertical: Option<bool>,
    pub(super) layer: &'a dyn Fn(Vec<AnyElement>) -> Vec<AnyElement>,
}

impl Paint<'_> {
    pub(super) fn build(
        &self,
        main: bool,
        band: bool,
        slot: &mut Vec<AnyElement>,
        fixed_slot: &mut Vec<AnyElement>,
    ) -> AnyElement {
        let mut opts = self.opts.clone();
        opts.frag.watch = main.then(|| self.watched.clone());
        // The group's native source paints its absolute/fixed continuations;
        // the copies replay them in place (paint::continuation).
        opts.cx.position.continue_in_slots(self.native_oof());
        let icb_layer =
            crate::layout::positioned::layer_scope::LayerScope::icb(&opts.cx.position.state);
        opts.clear_paged_fixed();
        // Процентная обёртка html/body сохраняет исходный CB page area.
        let mut wrap = super::frame::wrap(self.padding, self.height, self.first, self.vertical);
        if let [Node::Element(r)] = self.group
            && matches!(r.tag.as_str(), "html" | "body")
            && matches!(r.style.used().height, Some(Len::Pct(_)))
        {
            wrap = wrap.h(px(self.height));
        }
        let band_group: Vec<Node> = if band {
            self.group
                .iter()
                .filter_map(|n| {
                    crate::layout::positioned::repeat::drop_uncontained_oof(n, false, false)
                })
                .collect()
        } else {
            Vec::new()
        };
        let source = if band {
            band_group.as_slice()
        } else {
            self.group
        };
        let el = wrap
            .children(opts.cx.contracts.build.blocks(source, self.root, &opts))
            .into_any_element();
        slot.extend((self.layer)(icb_layer.finish()));
        fixed_slot.extend((self.layer)(opts.take_paged_fixed()));
        el
    }
}
