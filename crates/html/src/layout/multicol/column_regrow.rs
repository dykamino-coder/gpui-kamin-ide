//! Главная копия даёт измеренную высоту и разрывы стопки; её final geometry определяет
//! распорки `push::Regrow`. Копии пересобираются до устойчивого плана роста этой стопки
//! (css-break-3 §box-splitting; Blink `FinishFragmentation`).

use crate::layout::fragment::types::BreaksAt;
use crate::layout::multicol::column_stack::ColumnStack;
use crate::tolerance::COORD_EPS;
use gpui::{App, AvailableSpace, Window, point, px, size};

mod copies;
mod extent;
mod owners;
mod progress;
mod trial;
use copies::rebuild;

impl ColumnStack {
    /// Раскладка главной копии ребёнка `i` в колонку до укладки.
    pub(crate) fn lay_out_main(&mut self, i: usize, window: &mut Window, cx: &mut App) {
        let (axis, fixed) = (self.axis, self.fixed_height);
        let c = &mut self.children[i];
        let measured_size;
        if let Some(w) = c.measure {
            let (sz, content) = c.el.layout_as_root_with_content(
                if axis.is_vertical() {
                    size(AvailableSpace::MaxContent, AvailableSpace::Definite(px(w)))
                } else {
                    size(AvailableSpace::Definite(px(w)), AvailableSpace::MaxContent)
                },
                window,
                cx,
            );
            measured_size = sz;
            let h = f32::from(if axis.is_vertical() {
                sz.width
            } else {
                sz.height
            });
            c.h = h;
            extent::measure(c, h, content, sz, axis, window);
            if c.regrow.is_some() {
                c.cuts.clear();
                c.forced.clear();
                c.solid.clear();
                c.keep.clear();
            }
            let sections = c.measure_breaks(sz, axis, window);
            c.refresh_repeat(sections, fixed);
            // Целый вложенный многоколоночник выше колонки — не монолит:
            // его режет край внешней колонки (`StackChild::fit_whole`).
            if c.fit_whole {
                c.monolith = fixed.is_none_or(|f| h <= f + COORD_EPS);
            }
        } else if let Some(BreaksAt::Width(w)) = c.breaks_w {
            // Точки разрыва блочной стопки — по раскладке главной копии
            // в колонку (`StackChild::breaks_w`).
            let (sz, content) = c.el.layout_as_root_with_content(
                if axis.is_vertical() {
                    size(AvailableSpace::MaxContent, AvailableSpace::Definite(px(w)))
                } else {
                    size(AvailableSpace::Definite(px(w)), AvailableSpace::MaxContent)
                },
                window,
                cx,
            );
            measured_size = sz;
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
            if table {
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
        } else {
            return;
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

    /// Один шаг роста при неизменной высоте пробного фрагментаинера.
    fn grow_once(&mut self, window: &mut Window, cx: &mut App) -> bool {
        let kids = self.kid_geoms();
        let mut grows = self.grow_points(&kids);
        // Внутри ребёнка — снизу вверх: правка ниже точки не сдвигает
        // коробки выше, и разложенные border box прохода остаются верны.
        grows.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
        let mut dirty = vec![false; self.children.len()];
        let mut local_grows = vec![Vec::new(); self.children.len()];
        let (_, lines, plan, _) = self.balance(&kids);
        // Рост у фрагмента сдвигает всё содержимое потока ниже: точки
        // следующих фрагментов, найденные по прежней раскладке, устарели.
        // Копия с flex растёт по одному фрагменту за проход — первому,
        // которому нужен рост (как последовательная раскладка Blink,
        // фрагмент за фрагментом); остальные — после переукладки.
        let mut tagged: Vec<Vec<(usize, crate::layout::fragment::push::Growth)>> =
            vec![Vec::new(); self.children.len()];
        for f in &plan {
            let c = &mut self.children[f.kid];
            let (Some(r), Some(w)) = (c.regrow.as_mut(), c.watched.as_ref()) else {
                continue;
            };
            let Some(blocks) = w.blocks() else { continue };
            // Колонки сверх последней линии переполняют её вбок (css-multicol-1 §8.2):
            // их высота — высота этой линии.
            let Some(&(_, height)) = lines.get(f.col / self.count).or(lines.last()) else {
                continue;
            };
            let edge = f.from + height - f.y;
            let local = r.fragment_growths(blocks, f.from, f.from + f.h, edge, height);
            tagged[f.kid].extend(local.into_iter().map(|g| (f.copy, g)));
        }
        for &(kid, at, grow, forced) in &grows {
            let c = &mut self.children[kid];
            let (Some(_r), Some(w)) = (c.regrow.as_mut(), c.watched.as_ref()) else {
                continue;
            };
            let copy = plan
                .iter()
                .filter(|f| {
                    f.kid == kid && f.from - COORD_EPS <= at && at <= f.from + f.h + COORD_EPS
                })
                .map(|f| f.copy)
                .min()
                .unwrap_or(usize::MAX);
            if let Some(blocks) = w.blocks() {
                tagged[kid].extend(
                    crate::layout::fragment::push::Growth::at(blocks, &_r.laid, at, grow, forced)
                        .map(|g| (copy, g)),
                );
            }
        }
        for (kid, list) in tagged.into_iter().enumerate() {
            let flex = self.children[kid]
                .regrow
                .as_ref()
                .is_some_and(|r| !r.laid.flexes.is_empty());
            let first = list.iter().map(|g| g.0).min();
            local_grows[kid].extend(
                list.into_iter()
                    .filter(|g| !flex || Some(g.0) == first)
                    .map(|g| g.1),
            );
        }
        for (index, grows) in local_grows.into_iter().enumerate() {
            if let Some(r) = self.children[index].regrow.as_mut() {
                dirty[index] = r.push_many(grows);
            }
        }
        if !dirty.contains(&true) {
            return false;
        }
        for (i, _) in dirty.iter().enumerate().filter(|d| *d.1) {
            rebuild(&mut self.children[i]);
            self.lay_out_main(i, window, cx);
        }
        self.reserve_row_copies();
        true
    }
}
