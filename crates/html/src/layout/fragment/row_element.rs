//! Реализация `gpui::Element` для построчного потока атомов `FlowRow` (`types`): замер —
//! раскладка пробы без детей по пределу инлайн-оси (в вертикальном письме — высота), на
//! `prepaint` дети ставятся по слотам в неокруглённых координатах, чтобы дробные начала
//! дожили до округления устройства; `vertical-rl`/`vertical-lr`/`sideways-lr` переводят
//! слоты в физические оси.

use crate::hooks::Hooks;
use crate::layout::fragment::types::{FlowChild, FlowRow};
use gpui::{App, Bounds, IntoElement, LayoutId, Pixels, Window, point, px, size};

impl Hooks for FlowRow {
    type Layout = LayoutId;
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, _cx: &mut App) -> (LayoutId, Self::Layout) {
        let sizes: Vec<(f32, f32)> = self.children.iter().map(|c| (c.w, c.h)).collect();
        let shapes = self.shapes.clone();
        let rtl = self.rtl;
        let vertical_rl = self.vertical_rl;
        let inline_limit = self.inline_limit;
        let id = window.request_measured_layout(
            gpui::Style::default(),
            move |known, available, _window, _cx| {
                // Предел инлайн-оси: в вертикальном письме это ВЫСОТА.
                let pick = |k: Option<gpui::Pixels>, a: gpui::AvailableSpace| {
                    k.map(f32::from).or(match a {
                        gpui::AvailableSpace::Definite(v) => Some(f32::from(v)),
                        _ => None,
                    })
                };
                // Известный инлайн-размер содержащего блока важнее замера:
                // автовысотный хост вертикального письма предела не даёт.
                let limit = if vertical_rl {
                    inline_limit.or(pick(known.height, available.height))
                } else {
                    pick(known.width, available.width)
                }
                .unwrap_or(0.0);
                // Тот же обход, что и в layout(): без детей-элементов.
                let probe = FlowRow {
                    children: sizes
                        .iter()
                        .map(|&(w, h)| FlowChild {
                            el: gpui::Empty.into_any_element(),
                            w,
                            h,
                        })
                        .collect(),
                    shapes: shapes.clone(),
                    rtl,
                    vertical_rl,
                    inline_limit,
                    block_lr: false,
                    inline_up: false,
                    slots: std::cell::RefCell::new(Vec::new()),
                };
                let (h, _) = probe.layout(limit);
                if vertical_rl {
                    // Блок-прогресс — ширина (колонки), инлайн — высота.
                    size(px(h), px(limit))
                } else {
                    size(px(limit), px(h))
                }
            },
        );
        (id, id)
    }

    fn prepaint(
        &mut self,
        _bounds: Bounds<Pixels>,
        state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        let bounds = Bounds {
            origin: window.layout_origin_unrounded(*state),
            size: window.layout_size_unrounded(*state),
        };
        // Раскладка и подготовка детей — здесь: замер поддеревьев в фазе
        // отрисовки запрещён самим окном.
        let limit = if self.vertical_rl {
            f32::from(bounds.size.height)
        } else {
            f32::from(bounds.size.width)
        };
        let bw = f32::from(bounds.size.width);
        let vertical_rl = self.vertical_rl;
        let block_lr = self.block_lr;
        let inline_up = self.inline_up;
        let (_, slots) = self.layout(limit);
        let slots: Vec<(f32, f32)> = self
            .children
            .iter()
            .zip(slots)
            .map(|(c, (sx, sy))| {
                // `sideways-lr`: инлайн-позиция отсчитывается от НИЗА.
                let sx = if vertical_rl && inline_up {
                    limit - sx - c.h
                } else {
                    sx
                };
                if vertical_rl && block_lr {
                    // `vertical-lr`: колонка sy идёт от ЛЕВОГО края.
                    (sy, sx)
                } else if vertical_rl {
                    // t-мир → физика: колонка sy идёт от ПРАВОГО края.
                    (bw - sy - c.w, sx)
                } else {
                    (sx, sy)
                }
            })
            .collect();
        for (c, (sx, sy)) in self.children.iter_mut().zip(slots.iter()) {
            let origin = point(bounds.origin.x + px(*sx), bounds.origin.y + px(*sy));
            c.el.layout_as_root_at(
                origin,
                size(
                    gpui::AvailableSpace::Definite(px(c.w)),
                    gpui::AvailableSpace::Definite(px(c.h)),
                ),
                window,
                cx,
            );
            c.el.prepaint_at(point(px(0.0), px(0.0)), window, cx);
        }
        *self.slots.borrow_mut() = slots;
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        for c in self.children.iter_mut() {
            c.el.paint(window, cx);
        }
    }
}
