//! Элемент BandFlow: раскладка детей по полосам при request_layout/prepaint.

use super::flow_kid::intrinsic_width;
use super::kid::{Kid, Kind};
use super::{COORD_EPS, Plan, Probe, Slot, flatten, frame, plan};
use crate::hooks::Hooks;
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, IntoElement, LayoutId, Pixels, Window, div, point,
    prelude::*, px, size,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct BandFlow {
    pub(super) kids: Rc<Vec<Kid>>,
    pub(super) plan: Rc<RefCell<Option<Plan>>>,
    pub(super) built: Vec<AnyElement>,
    /// Письмо хоста: `Some(true)` — `vertical-rl`, `Some(false)` — `vertical-lr`.
    pub(super) vert: Option<bool>,
    pub(super) contain_floats: bool,
}

impl BandFlow {
    pub fn new(kids: Vec<Kid>, contain_floats: bool) -> Self {
        BandFlow {
            kids: Rc::new(kids),
            plan: Rc::new(RefCell::new(None)),
            built: Vec::new(),
            vert: None,
            contain_floats,
        }
    }

    /// Вертикальное письмо хоста: `rl` — `vertical-rl`, иначе `vertical-lr`.
    pub fn vertical(mut self, rl: bool) -> Self {
        self.vert = Some(rl);
        self
    }
}

impl Hooks for BandFlow {
    type Layout = LayoutId;
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, _cx: &mut App) -> (LayoutId, Self::Layout) {
        let kids = self.kids.clone();
        let cache = self.plan.clone();
        let mut style = gpui::Style::default();
        // Auto inline size stretches when layout supplies known.width; float,
        // inline-block and cell hosts use their children's shrink-to-fit sizes
        // (CSS 2.1 §10.3.5). A percentage width would instead fill all available
        // space even during intrinsic measurement: the floating .contain in
        // letter-spacing-206-ref must fit its widest floated paragraph.
        // Height follows the owner's float-containment policy independently
        // of this inline-size calculation (§§10.6.3, 10.6.7).
        style.flex_shrink = 0.0;
        let (vert, contain_floats) = (self.vert, self.contain_floats);
        let id = window.request_measured_layout(style, move |known, available, window, cx| {
            {
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
                    (None, AvailableSpace::Definite(w)) => {
                        let w = f32::from(w);
                        let (mn, mx) = window.with_nested_layout(|window| {
                            let p = &mut Probe {
                                window,
                                cx,
                                vert: v,
                            };
                            (
                                intrinsic_width(&kids, false, p),
                                intrinsic_width(&kids, true, p),
                            )
                        });
                        // §10.3.5: `min(max(min-content, available), max-content)`.
                        mx.max(mn).min(mn.max(w))
                    }
                    (None, a) => window.with_nested_layout(|window| {
                        let p = &mut Probe {
                            window,
                            cx,
                            vert: v,
                        };
                        intrinsic_width(&kids, matches!(a, AvailableSpace::MaxContent), p)
                    }),
                };
                if let Some(p) = cache.borrow().as_ref()
                    && (p.width - cb).abs() < COORD_EPS
                {
                    return phys(cb, p.height);
                }
                let p = window.with_nested_layout(|window| {
                    plan(
                        &kids,
                        cb,
                        contain_floats,
                        &mut Probe {
                            window,
                            cx,
                            vert: v,
                        },
                    )
                });
                let h = p.height;
                *cache.borrow_mut() = Some(p);
                phys(cb, h)
            }
        });
        (id, id)
    }

    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        let _ = bounds;
        // Отдельное дерево сохраняет точное начало хоста. Коробки и текст
        // округляются от общего абсолютного места, без переноса дробной
        // части начала в padding или позиции детей.
        let (vert, contain_floats) = (self.vert, self.contain_floats);
        let unr = window.layout_size_unrounded(*state);
        let cb = f32::from(if vert.is_some() {
            unr.height
        } else {
            unr.width
        });
        let origin = window.layout_origin_unrounded(*state);
        let cached = self
            .plan
            .borrow()
            .as_ref()
            .filter(|p| (p.width - cb).abs() < COORD_EPS)
            .cloned();
        let p = match cached {
            Some(p) => p,
            None => {
                let kids = self.kids.clone();
                window.with_nested_layout(|window| {
                    let p = &mut Probe {
                        window,
                        cx,
                        vert: vert.is_some(),
                    };
                    plan(&kids, cb, contain_floats, p)
                })
            }
        };
        self.built.clear();
        // Все дети — ОДНИМ корнем: относительный каркас и абсолютные
        // держатели по местам плана, как у статического хоста. Округление к
        // физической точке идёт на абсолютных краях внутри одного дерева
        // (`taffy.rs` `layout_bounds`), и соседние флоаты сходятся без щелей;
        // корень на каждого ребёнка округлял бы размер отдельно от места
        // (`units-005`: сто флоатов по `0.87em` с красными швами).
        // Порядок отрисовки — порядок детей: флоаты пробега, потом хвост.
        // Физический размер хоста: строчный `cb` и блочный `p.height`.
        let (pw, ph) = if vert.is_some() {
            (p.height, cb)
        } else {
            (cb, p.height)
        };
        let mut host = div().relative().w(px(pw)).h(px(ph));
        // CSS 2.1 прил. E: фоны блоков потока (шаг 4) — РАНЬШЕ флоатов
        // (шаг 5): флоат лежит поверх блока, под которым стоит
        // (`clear-004`). Строки рядом с флоатом его не перекрывают — их
        // порядок с флоатом не виден.
        let mut flat: Vec<(&Kid, &Slot)> = Vec::new();
        flatten(&self.kids, &p.slots, false, &mut flat);
        flatten(&self.kids, &p.slots, true, &mut flat);
        for (kid, s) in flat {
            if matches!(kid.kind, Kind::Strut(_)) {
                continue;
            }
            let tap = Rc::new(Cell::new(None));
            let el = frame(
                kid.kind,
                s.avail,
                (kid.build)(cb, s.avail, s.shapes.clone(), s.h),
                tap,
                vert.is_some(),
            );
            // Логическое место → физическое (Blink
            // `writing_mode_converter.cc:80-102`): строчный сдвиг — вниз,
            // блочный — от правого края у `vertical-rl`, от левого у `-lr`.
            let (left, top) = match vert {
                None => (s.x, s.y),
                Some(true) => (p.height - s.y - s.b, s.x),
                Some(false) => (s.y, s.x),
            };
            host = host.child(div().absolute().left(px(left)).top(px(top)).child(el));
        }
        let mut el = host.into_any_element();
        el.layout_as_root_at(
            origin,
            size(
                AvailableSpace::Definite(px(pw)),
                AvailableSpace::Definite(px(ph)),
            ),
            window,
            cx,
        );
        el.prepaint_at(point(px(0.0), px(0.0)), window, cx);
        self.built.push(el);
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        crate::paint::decorations::outline_phase::flow(window, cx, |window, cx| {
            for el in self.built.iter_mut() {
                el.paint(window, cx);
            }
        });
    }
}
