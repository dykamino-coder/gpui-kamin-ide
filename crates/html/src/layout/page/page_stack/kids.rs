//! Дети стопки листов (Kid) и предраскладка марджин-боксов.

use super::{PageGeom, PageStack};
use crate::layout::fragment::types::{Kid, Par};
use crate::layout::page::margin_boxes::layout_margin_boxes;
use gpui::{App, Bounds, Pixels, Window, point, px, size};

impl PageStack {
    // Margin boxes use page geometry and the current GPUI layout states.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepaint_margins(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        geoms: &[PageGeom],
        names: Vec<String>,
        pages: usize,
        tail: String,
    ) {
        if let Some(mf) = self.margin_for.clone() {
            for (p, pg) in geoms.iter().enumerate() {
                let boxes = mf(
                    p,
                    &names.get(p).cloned().unwrap_or_else(|| tail.clone()),
                    pages,
                    pg,
                );
                let (sx, sy) = self.sheet_origin(p);
                for (rect, mut el) in layout_margin_boxes(boxes, pg, window, cx) {
                    let rect = round_rect(rect);
                    el.layout_as_root(
                        size(
                            gpui::AvailableSpace::Definite(px(rect.2.max(0.0))),
                            gpui::AvailableSpace::Definite(px(rect.3.max(0.0))),
                        ),
                        window,
                        cx,
                    );
                    el.prepaint_at(
                        point(
                            bounds.origin.x + px(sx + rect.0),
                            bounds.origin.y + px(sy + rect.1),
                        ),
                        window,
                        cx,
                    );
                    self.margin_els.push((p, el));
                }
            }
        }
    }

    pub(super) fn page_kids(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        (aw, ah): (f32, f32),
    ) -> (Vec<Kid>, usize) {
        let vertical = self.axis.is_vertical();
        let kids: Vec<Kid> = self
            .kids
            .iter_mut()
            .map(|k| {
                // `aw` is the INLINE size of the page area; a vertical root
                // measures its block size along x (`page_stack/axis.rs`).
                let (fixed, free) = (
                    gpui::AvailableSpace::Definite(px(aw)),
                    gpui::AvailableSpace::MaxContent,
                );
                let space = if vertical {
                    size(free, fixed)
                } else {
                    size(fixed, free)
                };
                let laid = k.el.layout_as_root(space, window, cx);
                let measured_flow = k
                    .watched
                    .as_ref()
                    .map(|w| w.flow.at(point(px(0.0), px(0.0)), laid, self.axis, window));
                let sz = laid;
                let sz = if vertical {
                    size(sz.height, sz.width)
                } else {
                    sz
                };
                // Мера поддерева даёт точки разреза внутри ребёнка; без неё
                // ребёнок — цельный кусок измеренной высоты.
                let (mut h, mut cuts, mut forced, mut solid) = match &k.shape {
                    Some((h, cuts, forced, solid)) => {
                        (*h, cuts.clone(), forced.clone(), solid.clone())
                    }
                    None => (f32::from(sz.height), Vec::new(), Vec::new(), Vec::new()),
                };
                if let Some(flow) = measured_flow {
                    // CSS Break 3 section 3.1: root floats participate in page
                    // fragmentation. Their anonymous normal-flow wrapper can
                    // have zero height, unlike the measured float border box.
                    if k.shape.is_none()
                        && let Some(id) = k.float_id
                    {
                        h = flow
                            .floats
                            .iter()
                            .filter(|g| g.id == id)
                            .map(|g| g.bottom)
                            .fold(h, f32::max);
                    }
                    match k.watched.clone().filter(|w| w.has_blocks()) {
                        // Точки разрыва блочной стопки — по раскладке копии, а
                        // не по оценке формы (`flow::blocks`). Мера формы
                        // отсчитывает смещения от border box коробки, копия
                        // поднята на `inner_top`.
                        Some(w) => {
                            let lift = k.inner_top;
                            let origin = point(px(0.0), px(lift));
                            // Таблица: высота — её разложенная обёртка (ряды,
                            // подписи, заданная высота), а не оценка.
                            if let Some(root) = w.blocks().filter(|b| b.table.is_some())
                                && k.float_id.is_none()
                                && let Some((_, bottom)) = w.laid(origin, window).at(root.id)
                            {
                                h = bottom;
                            }
                            let m = w.breaks(origin, laid, h + lift, self.axis, window);
                            (cuts, forced, solid) = (m.cuts, m.forced, m.solid);
                            k.refresh_repeat(m.sections, ah);
                        }
                        None => flow.merge(&mut cuts, &mut solid),
                    }
                }
                Kid {
                    h,
                    mt: k.mt,
                    mb: k.mb,
                    monolith: k.monolith,
                    cuts,
                    force_before: k.force_before,
                    force_after: k.force_after,
                    // Правило 1 §4.3 в ПЕЧАТИ пока не применяется (§2 отчёта):
                    // `PageKid` запретов не носит, а `false` в обоих полях
                    // включает быстрый выход `fill_avoiding` — путь страниц
                    // остаётся байт-в-байт прежним.
                    avoid_before: false,
                    avoid_after: false,
                    forced,
                    solid,
                    span: false,
                    // Страницы параллельный поток пока не берут: у них своё
                    // правило переполнения монолита (`fill_at`, ветка
                    // `paged && placed && cur > target`, crbug 1402540), и
                    // мешать их без отдельного замера печатного среза нельзя.
                    // `over == h` — поток выключен.
                    over: h,
                    // Страницы: `box-decoration-break` пока `slice` — весь
                    // кластер `clone` в колонках.
                    clone_dec: None,
                    // У страниц своё правило переполнения монолита (`fill_at`,
                    // ветка `paged && placed && cur > target`, crbug 1402540).
                    overflow_top: false,
                    repeat: k.repeat.as_ref().map_or_else(Default::default, |r| r.geom),
                    par: Par::default(),
                }
            })
            .collect();
        let limit = self
            .kids
            .iter()
            .map(|k| k.frags.len() + 1)
            .min()
            .unwrap_or(1);
        (kids, limit)
    }
}

/// Прямоугольник марджин-бокса, округлённый как раскладка потока (Taffy
/// `round_layout`: края — `round`, размер — разность округлённых краёв).
/// Центрированная коробка на 234.5 рисовалась с 234, а эталон, собранный
/// потоком (grid/flex), — с 235 (`margin-boxes/auto-margins-003`,
/// `overconstrained-001`).
fn round_rect((x, y, w, h): (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
    let (x0, y0) = (x.round(), y.round());
    (x0, y0, (x + w).round() - x0, (y + h).round() - y0)
}

#[cfg(test)]
mod tests {
    use super::round_rect;

    #[test]
    fn margin_box_rect_rounds_edges_like_taffy() {
        assert_eq!(
            round_rect((234.5, 69.5, 31.0, 31.0)),
            (235.0, 70.0, 31.0, 31.0)
        );
        // Размер — разность округлённых краёв, а не округлённая ширина.
        assert_eq!(round_rect((10.4, 0.0, 20.2, 5.0)), (10.0, 0.0, 21.0, 5.0));
    }
}
