//! Регистрация приостанавливаемой пробы полос с исходным окружением хоста.
use super::super::flow_kid::intrinsic_sizes;
use super::super::{Probe, plan};
use super::BandFlow;
use gpui::{App, AvailableSpace, LayoutId, Pixels, Window, px, size};
use std::{cell::RefCell, rc::Rc};
impl BandFlow {
    pub(super) fn request_plan(&self, window: &mut Window) -> LayoutId {
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
        let measure = async move |known: gpui::Size<Option<Pixels>>,
                                  available: gpui::Size<AvailableSpace>,
                                  geometry: gpui::MeasuredBox,
                                  window: &mut Window,
                                  cx: &mut App,
                                  stack: &mut gpui::PaintStack| {
            // A final layout pass whose parent already fixed the inline size
            // lays the children out at that size: shrink-to-fit would only
            // re-derive it from min/max-content probes.
            let fixed = geometry.fixed;
            gpui::MeasuredContent::new(
                window
                    .with_element_layout_context_async(&context, async |window| {
                        let v = vert.is_some();
                        let fixed_inline = if v { fixed.height } else { fixed.width };
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
                            (None, AvailableSpace::Definite(w)) if fill_inline || fixed_inline => {
                                f32::from(w)
                            }
                            (None, a) => {
                                // The min/max-content pair takes no measure input,
                                // so one pair per element is exact: `intrinsic_sizes`
                                // builds every child with no containing-block size
                                // (`build(0.0, 0.0, ..)`) and lays it out under an
                                // indefinite block size (`MaxContent`), never the
                                // `known`/`available` block size of this call; a
                                // kid's retained pair (`band_flow_host::kids`) is keyed
                                // by build inputs only. Blink
                                // keys its cache by `initial_block_size`
                                // (`layout_box.h`) because its algorithms may receive
                                // a definite one; these probes cannot.
                                let (min, max) = if let Some(sizes) = intrinsic.get() {
                                    sizes
                                } else {
                                    window
                                        .with_retained_layout_async(
                                            &mut gpui::RetainedLayout::default(),
                                            async |window| {
                                                let p = &mut Probe::new(
                                                    window,
                                                    cx,
                                                    v,
                                                    fragment.as_ref(),
                                                    paint.clone(),
                                                    scopes.clone(),
                                                    build,
                                                );
                                                let sizes = stack
                                                    .run(|stack| intrinsic_sizes(&kids, p, stack))
                                                    .await;
                                                intrinsic.set(Some(sizes));
                                                sizes
                                            },
                                        )
                                        .await
                                };
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
                            super::super::clamp::publish(&output, &p, vert, inline_up);
                            return phys(cb, p.height);
                        }
                        let mut engine = gpui::RetainedLayout::default();
                        let retained = engine.clone();
                        let p = window
                            .with_retained_layout_async(&mut engine, async |window| {
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
                                stack
                                    .run(|stack| plan(&kids, cb, contain_floats, &mut p, stack))
                                    .await
                            })
                            .await;
                        if let Some(f) = &fragment {
                            f.publish(&p, &kids);
                        }
                        super::super::clamp::publish(&output, &p, vert, inline_up);
                        let h = p.height;
                        cache.insert(p);
                        phys(cb, h)
                    })
                    .await,
            )
        };
        let id = window.request_measured_layout_with_box_async(style, measure);
        if let Some((_, clamp)) = &self.clamp {
            clamp.lift(id, measured);
        }
        if let Some(f) = &self.fragment {
            f.node.record(id, window);
        }
        id
    }
}
