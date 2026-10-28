//! Итог раскладки главной копии: высота, поток, точки разрыва и разложенная геометрия
//! роста ребёнка стопки (вызывается после возврата вложенной раскладки).

use crate::layout::multicol::column_stack::ColumnStack;
use crate::tolerance::COORD_EPS;
use gpui::{Window, point, px};

impl ColumnStack {
    #[inline(never)]
    pub(super) fn record_main(
        &mut self,
        i: usize,
        measured: bool,
        (sz, content): (gpui::Size<gpui::Pixels>, gpui::Size<gpui::Pixels>),
        window: &mut Window,
    ) {
        let (axis, fixed) = (self.axis, self.fixed_height);
        // Колонки переполнения (css-multicol-1 §8.2) есть только у стопки без
        // балансировки: заданная высота колонок или ряды внешнего фрагментаинера.
        let fills = fixed.is_some() || self.rows.is_some_and(|r| !r.cap);
        let c = &mut self.children[i];
        let measured_size = sz;
        if measured {
            let h = f32::from(if axis.is_vertical() {
                sz.width
            } else {
                sz.height
            });
            c.h = h;
            super::extent::measure(c, h, content, sz, (axis, fills), window);
            if c.regrow.is_some() {
                c.cuts.clear();
                c.forced.clear();
                c.solid.clear();
                c.keep.clear();
                c.line_keep.clear();
            }
            let sections = c.measure_breaks(sz, axis, window);
            c.refresh_repeat(sections, fixed);
            // Целый вложенный многоколоночник выше колонки — не монолит:
            // его режет край внешней колонки (`StackChild::fit_whole`).
            if c.fit_whole {
                c.monolith = fixed.is_none_or(|f| h <= f + COORD_EPS);
            }
        } else {
            // Точки разрыва блочной стопки — по раскладке главной копии
            // в колонку (`StackChild::breaks_w`).
            #[cfg(feature = "wpt-harness")]
            if c.probe_pending && c.watched.is_some() {
                c.probe.height(
                    "columns.width",
                    c.h,
                    f32::from(if axis.is_vertical() {
                        sz.width
                    } else {
                        sz.height
                    }),
                );
            }
            // Использованный блочный размер — разложенный border box: в нём
            // уже `min-height`/`max-height` и распорки роста. Параллельный
            // поток (css-break-3 §3) остаётся протяжённостью меры.
            let table = c
                .watched
                .as_ref()
                .is_some_and(|w| w.blocks().is_some_and(|b| b.table.is_some()));
            let clipped = c
                .watched
                .as_ref()
                .is_some_and(|w| w.blocks().is_some_and(|b| b.clips_block));
            if clipped {
                // Shape-оценка может включать содержимое за clip box. Только
                // измеренный border box участвует в продолжении колонок.
                c.h = f32::from(if axis.is_vertical() {
                    sz.width
                } else {
                    sz.height
                });
                c.over = c.h;
            } else if table {
                // Таблица: высота — разложенные ряды, подписи и заданная
                // высота (css-tables-3 §computing-the-table-height), поток —
                // и видимое переполнение её содержимого (css-break-3 §3).
                let h = f32::from(if axis.is_vertical() {
                    sz.width
                } else {
                    sz.height
                });
                c.h = h;
                c.over = h.max(f32::from(if axis.is_vertical() {
                    content.width
                } else {
                    content.height
                }));
            } else if c.regrow.as_ref().is_some_and(|r| r.laid_h) {
                let h = f32::from(if axis.is_vertical() {
                    sz.width
                } else {
                    sz.height
                });
                c.over = if c.over > c.h + COORD_EPS {
                    c.over.max(h)
                } else {
                    h
                };
                // Видимое переполнение измеренной блочной копии продолжает
                // свой параллельный поток независимо от её border box; коробка,
                // обрезающая блочную ось (`overflow: clip/hidden`), потока не
                // имеет (css-break-3 §3, css-overflow-3).
                if c.regrow.as_ref().is_some_and(|r| {
                    crate::layout::page::paged::visible_overflow(&r.source.style.used())
                }) {
                    c.over = c.over.max(f32::from(content.height));
                }
                c.h = h;
            }
            let sections = c.replace_breaks(sz, axis, window);
            c.refresh_repeat(sections, fixed);
        }
        if let (Some(r), Some(w)) = (c.regrow.as_mut(), c.watched.as_ref()) {
            let origin = point(px(0.0), px(0.0));
            let flow = w.flow.at(origin, measured_size, axis, window);
            r.laid = w
                .laid_axis(origin, measured_size, axis, window)
                .with_flow(&flow);
            r.space = w.space(window);
            if !r.laid.flexes.is_empty()
                && let Some(blocks) = w.blocks()
            {
                let (before, after) = (blocks.edge(false, &r.laid), blocks.edge(true, &r.laid));
                c.force_before = before.0;
                c.force_after = after.0;
                c.avoid_before = before.1;
                c.avoid_after = after.1;
            }
        }
    }
}
