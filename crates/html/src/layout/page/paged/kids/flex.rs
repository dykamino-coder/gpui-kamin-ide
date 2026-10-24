//! Пересборка независимой печатной копии с ICB/fixed-слоями и ограничениями роста F1-3.
use crate::dom::Node;
use crate::hooks::{Hooks, IntoHooked};
use crate::layout::fragment::paginate::watch::Watched;
use crate::layout::fragment::push::Regrow;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;
use gpui::{
    AnyElement, App, Bounds, IntoElement, LayoutId, ParentElement, Pixels, Styled, Window, div, px,
};
use std::cell::RefCell;
use std::rc::Rc;

/// Слот слоя остаётся в исходном порядке краски; пересборка меняет его содержимое.
struct Layer(Rc<RefCell<AnyElement>>);
impl Hooks for Layer {
    type Layout = ();
    type Prepaint = ();
    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        (self.0.borrow_mut().request_layout(window, cx), ())
    }
    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) {
        self.0.borrow_mut().prepaint(window, cx);
    }
    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0.borrow_mut().paint(window, cx);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn regrow(
    opts: &RenderOpts,
    root: &BoxStyle,
    group: &[Node],
    watched: &Rc<Watched>,
    padding: (f32, f32, f32),
    first: bool,
    (aw, ah): (f32, f32),
    vertical: Option<bool>,
    icb: &mut [Vec<AnyElement>],
    fixed: &mut [Vec<AnyElement>],
) -> Option<Regrow> {
    let [Node::Element(dom)] = group else {
        return None;
    };
    // Таблица листов строится своими полосами (повтор шапки/подвала,
    // `repeat::sections`); её рост — не по этой пересборке.
    let blocks = watched.blocks()?;
    let mut flexes = Vec::new();
    blocks.flexes(&mut flexes);
    let independent = match vertical {
        Some(rl) => crate::layout::fragment::flow::vertical_copy(dom, rl),
        None => crate::layout::fragment::flow::regrow_copy(dom),
    };
    if flexes.is_empty() && (blocks.table.is_some() || !independent) {
        return None;
    }
    let slots: Vec<_> = icb
        .iter_mut()
        .zip(fixed)
        .map(|(i, f)| {
            let a = Rc::new(RefCell::new(div().into_any_element()));
            let b = Rc::new(RefCell::new(div().into_any_element()));
            i.push(Layer(a.clone()).hooked());
            f.push(Layer(b.clone()).hooked());
            (a, b)
        })
        .collect();
    let (base, root, rec) = (opts.clone(), root.clone(), watched.clone());
    let build = move |dom: &crate::dom::Element, part: usize| {
        let mut opts = base.clone();
        let paged = crate::layout::page::paged::PagedBuild::begin();
        opts.frag.paged = Some(paged.clone());
        opts.frag.watch = Some(if part == 0 {
            rec.clone()
        } else {
            rec.copy_recorder(part)
        });
        let scope =
            crate::layout::positioned::layer_scope::LayerScope::icb(&opts.cx.position.state);
        let mut wrap = super::frame::wrap(padding, ah, first, vertical);
        if matches!(dom.tag.as_str(), "html" | "body")
            && matches!(dom.style.used().height, Some(Len::Pct(_)))
        {
            wrap = wrap.h(px(ah));
        }
        let el =
            wrap.children(opts.cx.contracts.build.blocks(
                &[Node::Element(dom.clone())],
                &root,
                &opts,
            ))
            .into_any_element();
        let layer = |els| {
            div()
                .relative()
                .w(px(aw))
                .h(px(ah))
                .children(els)
                .into_any_element()
        };
        *slots[part].0.borrow_mut() = layer(scope.finish());
        *slots[part].1.borrow_mut() = layer(opts.take_paged_fixed());
        paged.end();
        el
    };
    Some(Regrow {
        axis: match vertical {
            Some(true) => crate::layout::fragment::types::StackAxis::VerticalRl,
            Some(false) => crate::layout::fragment::types::StackAxis::VerticalLr,
            None => crate::layout::fragment::types::StackAxis::Horizontal,
        },
        dom: dom.clone(),
        source: dom.clone(),
        build: Rc::new(build),
        opts: opts.clone(),
        laid: Default::default(),
        space: None,
        laid_h: true,
        watched: watched.clone(),
    })
}
