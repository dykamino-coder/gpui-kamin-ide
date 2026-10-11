//! Дети стопки листов (Kid) и предраскладка марджин-боксов.

use super::{PageGeom, PageStack};
use crate::layout::fragment::types::{Kid, Par, RepeatGeom};
use crate::layout::page::margin_boxes::layout_margin_boxes;
use gpui::{App, Bounds, Pixels, Window, point, px, size};

impl PageStack {
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
        aw: f32,
    ) -> (Vec<Kid>, usize) {
        let kids: Vec<Kid> = self
            .kids
            .iter_mut()
            .map(|k| {
                let sz = k.el.layout_as_root(
                    size(
                        gpui::AvailableSpace::Definite(px(aw)),
                        gpui::AvailableSpace::MaxContent,
                    ),
                    window,
                    cx,
                );
                // Мера поддерева даёт точки разреза внутри ребёнка; без неё
                // ребёнок — цельный кусок измеренной высоты.
                let (h, cuts, forced, solid) = match &k.shape {
                    Some((h, cuts, forced, solid)) => {
                        (*h, cuts.clone(), forced.clone(), solid.clone())
                    }
                    None => (f32::from(sz.height), Vec::new(), Vec::new(), Vec::new()),
                };
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
                    repeat: RepeatGeom::default(),
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
