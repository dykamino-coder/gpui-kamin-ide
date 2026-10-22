//! Сборка каркаса BandFlow по измеренному плану в текущем кадре.
use super::element::BandFlow;
use super::{COORD_EPS, Probe, plan};
use gpui::{
    App, AvailableSpace, Bounds, IntoElement, LayoutId, Pixels, Window, div, point, prelude::*, px,
    size,
};
impl BandFlow {
    pub(super) fn prepaint_plan(
        &mut self,
        bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
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
                    let p = &mut Probe::new(
                        window,
                        cx,
                        vert.is_some(),
                        self.fragment.as_ref(),
                        self.paint.clone(),
                        self.probe_scopes(),
                        self.build,
                    );
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
        // CSS 2.1 прил. E: фоны блоков потока (шаг 4) — РАНЬШЕ флоатов
        // (шаг 5): флоат лежит поверх блока, под которым стоит
        // (`clear-004`); анонимные строчные прогоны (шаг 7) — поверх флоатов
        // (`shape-outside-content-box-001`: атомы шире выреза).
        let at = super::holders::Placing {
            paint: &self.paint,
            build: &self.build,
            cb,
            height: p.height,
            vert,
            inline_up: self.inline_up,
        };
        let host = div()
            .relative()
            .w(px(pw))
            .h(px(ph))
            .children(super::holders::holders(
                &self.kids,
                &p.slots,
                &at,
                (0.0, 0.0),
            ));
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
}
