//! Кусок корня `box-decoration-break: clone` в записи главной копии: главная копия —
//! непрерывная коробка, срез куска — содержимое, которое съел его фрагмент (как обёртка
//! `overflow-y: clip` копии `clone_fragment`), от начала содержимого коробки; у последнего
//! фрагмента — до конца видимого переполнения. Поперёк колонки срез не режет.

use crate::layout::fragment::types::Frag;
use crate::layout::multicol::column_stack::ColumnStack;
use crate::tolerance::COORD_EPS;
use gpui::{Bounds, Pixels, Point, point, px};

impl ColumnStack {
    /// Срез содержимого куска `f` в координатах источника (`source` — место коробки в
    /// главной копии); `None` — ребёнок не clone записи.
    pub(super) fn clone_slice(
        &self,
        f: &Frag,
        plan: &[Frag],
        source: Point<Pixels>,
        across: (Pixels, Pixels),
    ) -> Option<Bounds<Pixels>> {
        let k = &self.children[f.kid];
        let (dt, db) = k.clone_dec.filter(|_| k.clone_deco.is_some())?;
        let next = plan.iter().find(|g| g.kid == f.kid && g.copy == f.copy + 1);
        let end = if k.over > k.h + COORD_EPS {
            k.over + db
        } else {
            k.h
        };
        let eaten = next.map_or((end - dt - db - f.from).max(f.h - dt - db), |n| {
            n.from - f.from
        });
        let top = source.y + px(dt + f.from);
        Some(Bounds::from_corners(
            point(across.0, top),
            point(across.1, top + px(eaten.max(0.0))),
        ))
    }
}
