//! Главная копия даёт измеренную высоту и разрывы стопки; её final geometry определяет
//! распорки `push::Regrow`. Копии пересобираются до устойчивого плана роста этой стопки
//! (css-break-3 §box-splitting; Blink `FinishFragmentation`).

use crate::layout::fragment::types::BreaksAt;
use crate::layout::multicol::column_stack::ColumnStack;
use crate::tolerance::COORD_EPS;
use gpui::{App, AvailableSpace, Window, px, size};

mod copies;
mod extent;
mod items;
mod nest;
pub(crate) use nest::{first_row_overflow, fragmentainer_extent};
mod owners;
mod progress;
mod record;
mod trial;

impl ColumnStack {
    /// Раскладка главной копии ребёнка `i` в колонку до укладки.
    ///
    /// Копия может сама содержать многоколоночник, и его раскладка идёт внутри
    /// этой: кадр, живущий на время вложенной раскладки, держится малым, а
    /// разбор её итога — отдельным кадром (`record_main`) после возврата.
    pub(crate) fn lay_out_main(&mut self, i: usize, window: &mut Window, cx: &mut App) {
        let axis = self.axis;
        let c = &mut self.children[i];
        let (measured, w) = match (c.measure, c.breaks_w) {
            (Some(w), _) => (true, w),
            (None, Some(BreaksAt::Width(w))) => (false, w),
            _ => return,
        };
        let (sz, content) = c.el.layout_as_root_with_content(
            if axis.is_vertical() {
                size(AvailableSpace::MaxContent, AvailableSpace::Definite(px(w)))
            } else {
                size(AvailableSpace::Definite(px(w)), AvailableSpace::MaxContent)
            },
            window,
            cx,
        );
        self.record_main(i, measured, (sz, content), window);
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
            // Независимые потоки flex и сетки растут по одному фрагменту за проход.
            let flex = self.children[kid]
                .regrow
                .as_ref()
                .is_some_and(|r| !r.laid.flexes.is_empty() || !r.laid.placements.is_empty());
            let first = list.iter().map(|g| g.0).min();
            local_grows[kid].extend(
                list.into_iter()
                    .filter(|g| !flex || Some(g.0) == first)
                    .map(|g| g.1),
            );
        }
        for (kid, grows) in local_grows.iter_mut().enumerate() {
            grows.extend(self.item_growths(kid, &plan, &lines));
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
            copies::rebuild(&mut self.children[i]);
            self.lay_out_main(i, window, cx);
        }
        self.reserve_row_copies();
        self.reserve_overflow_copies();
        true
    }
}
