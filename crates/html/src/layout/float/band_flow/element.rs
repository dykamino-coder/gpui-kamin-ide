//! Элемент BandFlow: раскладка детей по полосам при request_layout/prepaint.

use super::flow_kid::intrinsic_sizes;
use super::kid::Kid;
use super::{Probe, plan};
use crate::hooks::Hooks;
use crate::layout::build_contracts::ChildBuild;
use crate::layout::cx::LayoutCx;
use crate::layout::cx::deferred::Deferred;
use gpui::{AnyElement, App, AvailableSpace, Bounds, LayoutId, Pixels, Window, px, size};
use std::cell::RefCell;
use std::rc::Rc;

pub struct BandFlow {
    pub(super) paint: Rc<Deferred>,
    pub(super) build: ChildBuild,
    pub(super) kids: Rc<Vec<Kid>>,
    pub(super) plan: Rc<super::cache::Plans>,
    intrinsic: Rc<std::cell::Cell<Option<(f32, f32)>>>,
    pub(super) fill_inline: bool,
    pub(super) built: Vec<AnyElement>,
    /// Письмо хоста: `Some(true)` — `vertical-rl`, `Some(false)` — `vertical-lr`.
    pub(super) vert: Option<bool>,
    pub(super) inline_up: bool,
    pub(super) contain_floats: bool,
    pub(super) clamp: Option<(u64, crate::text::clamp::ClampMeasure)>,
    pub(super) fragment: Option<super::fragment::FragmentFlow>,
    pub(super) positioning: Option<Rc<crate::layout::positioned::containing_block::Positioning>>,
}

impl BandFlow {
    pub(crate) fn new(kids: Vec<Kid>, contain_floats: bool, cx: &LayoutCx) -> Self {
        BandFlow {
            paint: cx.deferred.paint.clone(),
            build: cx.contracts.build,
            kids: Rc::new(kids),
            plan: Default::default(),
            intrinsic: Default::default(),
            fill_inline: cx.constraints.avail_w.is_some(),
            built: Vec::new(),
            vert: None,
            inline_up: false,
            contain_floats,
            fragment: None,
            clamp: None,
            positioning: None,
        }
    }

    /// Host block direction and line-left edge (Writing Modes 4 §6.4).
    pub fn vertical(mut self, wm: crate::style::computed::writing_mode::WritingMode) -> Self {
        self.vert = Some(wm.block_rl());
        self.inline_up = wm.sideways_lr();
        self
    }
}

impl Hooks for BandFlow {
    type Layout = LayoutId;
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, _cx: &mut App) -> (LayoutId, Self::Layout) {
        let kids = self.kids.clone();
        let cache = self.plan.clone();
        let intrinsic = self.intrinsic.clone();
        let fill_inline = self.fill_inline;
        let fragment = self.fragment.clone();
        let paint = self.paint.clone();
        let scopes = self.probe_scopes();
        let build = self.build;
        let measured = Rc::new(RefCell::new(Default::default()));
        let output = measured.clone();
        let context = window.element_layout_context();
        let inline_up = self.inline_up;
        let style = gpui::Style {
            flex_shrink: 0.0,
            ..Default::default()
        };
        // BoxConstraints даёт definite stretch-fit только блочному потоку
        // (§10.3.3). Float/inline-block/abspos и flex/grid/table оставляют
        // размер неизвестным до своего алгоритма; их fit-content сохраняется.
        let (vert, contain_floats) = (self.vert, self.contain_floats);
        let measure = move |known: gpui::Size<Option<Pixels>>,
                            available: gpui::Size<AvailableSpace>,
                            geometry: gpui::MeasuredBox,
                            window: &mut Window,
                            cx: &mut App| {
            window.with_element_layout_context(&context, |window| {
                let v = vert.is_some();
                // Строчная ось — ширина в горизонтальном письме, высота в
                // вертикальном; блочный размер плана уходит в другую ось.
                let (known_inline, avail_inline) = if vert.is_some() {
                    (known.height, available.height)
                } else {
                    (known.width, available.width)
                };
                let phys = |inline: f32, block: f32| {
                    if vert.is_some() {
                        size(px(block), px(inline))
                    } else {
                        size(px(inline), px(block))
                    }
                };
                let cb = match (known_inline, avail_inline) {
                    (Some(w), _) => f32::from(w),
                    (None, AvailableSpace::Definite(w)) if fill_inline => f32::from(w),
                    (None, a) => {
                        let (min, max) = intrinsic.get().unwrap_or_else(|| {
                            window.with_nested_layout(|window| {
                                let p = &mut Probe::new(
                                    window,
                                    cx,
                                    v,
                                    fragment.as_ref(),
                                    paint.clone(),
                                    scopes.clone(),
                                    build,
                                );
                                let sizes = intrinsic_sizes(&kids, p);
                                intrinsic.set(Some(sizes));
                                sizes
                            })
                        });
                        match a {
                            AvailableSpace::Definite(w) => max.min(min.max(f32::from(w))),
                            AvailableSpace::MaxContent => max,
                            AvailableSpace::MinContent => min,
                        }
                    }
                };
                let axis = if v {
                    gpui::Axis::Vertical
                } else {
                    gpui::Axis::Horizontal
                };
                if geometry.intrinsic_axis == Some(axis) {
                    return phys(cb, 0.0);
                }
                if let Some(p) = cache.get(cb) {
                    if let Some(f) = &fragment {
                        f.publish(&p, &kids);
                    }
                    super::clamp::publish(&output, &p, vert, inline_up);
                    return phys(cb, p.height);
                }
                let mut engine = gpui::RetainedLayout::default();
                let retained = engine.clone();
                let p = window.with_retained_layout(&mut engine, |window| {
                    let mut p = Probe::new(
                        window,
                        cx,
                        v,
                        fragment.as_ref(),
                        paint.clone(),
                        scopes.clone(),
                        build,
                    );
                    p.engine = Some(retained);
                    plan(&kids, cb, contain_floats, &mut p)
                });
                if let Some(f) = &fragment {
                    f.publish(&p, &kids);
                }
                super::clamp::publish(&output, &p, vert, inline_up);
                let h = p.height;
                cache.insert(p);
                phys(cb, h)
            })
        };
        let id = window.request_measured_layout_with_box(style, measure);
        if let Some((_, clamp)) = &self.clamp {
            clamp.lift(id, measured);
        }
        if let Some(f) = &self.fragment {
            f.node.record(id, window);
        }
        (id, id)
    }

    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.prepaint_plan(bounds, state, window, cx);
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        // The host is not a stacking context (CSS 2.1 Appendix E.2): later
        // block backgrounds of the enclosing context paint before the
        // inline foreground of its runs. Paint groups keep their own
        // boundary (`holders`); the outline phase wraps the host (`Effect::OutlineFlow`).
        crate::hooks::paint_reopen(window, |window| {
            for el in self.built.iter_mut() {
                el.paint(window, cx);
            }
        });
    }
}
