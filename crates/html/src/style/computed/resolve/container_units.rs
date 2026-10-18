//! База единиц контейнера `cq*` для потомков: содержимое контейнера запросов размера (css-conditional-5 §container-lengths).

use crate::style::computed::Computed;
use crate::style::values::value::{Len, px_strict};

impl Computed {
    /// Что унаследуют потомки в `cq_box`. Сам контейнер своих единиц от себя
    /// не берёт — только его потомки, поэтому база считается для детей.
    ///
    /// `container-type: size` даёт обе оси, `inline-size` — только строчную
    /// (по письму контейнера). Размер известен лишь заданный в точках; без
    /// него ось наследует базу внешнего контейнера, как будто этот не годится
    /// («eligible query container» для оси).
    pub fn container_box_for_children(&self) -> (Option<f32>, Option<f32>) {
        if !self.container_size_query {
            return self.cq_box;
        }
        let px = px_strict;
        let border_box = self.border_box == Some(true);
        let inner = |size: Option<Len>, a: Option<Len>, b: Option<Len>, c, d| {
            let v = px(size)?;
            if !border_box {
                return Some(v);
            }
            let edges = [a, b, c, d]
                .iter()
                .map(|l| px(*l).unwrap_or(0.0))
                .sum::<f32>();
            Some((v - edges).max(0.0))
        };
        let w = inner(
            self.width,
            self.padding.left,
            self.padding.right,
            self.border_width.left,
            self.border_width.right,
        );
        let h = inner(
            self.height,
            self.padding.top,
            self.padding.bottom,
            self.border_width.top,
            self.border_width.bottom,
        );
        // Полосы прокрутки съедают содержимое контейнера (css-overflow-3
        // §4.2): `100cqh` под горизонтальной полосой короче высоты.
        // Классические полосы съедают содержимое контейнера (css-overflow-3
        // §4.2). Модель полос (`scroll.used`) знает `overflow: scroll` и
        // полосы прошлого кадра; жёлоб `scrollbar-gutter: stable` раскладка
        // резервирует сверх неё, поэтому он добавляется, когда модель
        // строчную полосу не учла.
        let g = self.scroll.used;
        let stable = self.scroll.gutter.is_some_and(|v| v != 0)
            && !self.wm().vertical()
            && matches!(
                self.overflow_y,
                Some(crate::style::computed::Overflow::Scroll)
            )
            && g[0] + g[2] == 0.0;
        let sides = if self.scroll.gutter == Some(2) {
            2.0
        } else {
            1.0
        };
        let gutter = if stable {
            self.scroll.thickness() * sides
        } else {
            0.0
        };
        let w = w.map(|v| (v - g[0] - g[2] - gutter).max(0.0));
        let h = h.map(|v| (v - g[1] - g[3]).max(0.0));
        let both = self.contain_size == Some(true);
        let vertical = self.wm().vertical();
        let take_w = both || !vertical;
        let take_h = both || vertical;
        (
            if take_w {
                w.or(self.cq_box.0)
            } else {
                self.cq_box.0
            },
            if take_h {
                h.or(self.cq_box.1)
            } else {
                self.cq_box.1
            },
        )
    }
}
