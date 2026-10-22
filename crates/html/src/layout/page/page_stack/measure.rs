//! Мера одной печатной копии: её ширина, строки и реальные границы разрыва.
use super::PageKid;
use crate::layout::fragment::types::{Kid, Par, StackAxis};
use gpui::{App, Window, point, px, size};

pub(super) fn measure(
    k: &mut PageKid,
    axis: StackAxis,
    (aw, ah): (f32, f32),
    copy: usize,
    window: &mut Window,
    cx: &mut App,
) -> Kid {
    let vertical = axis.is_vertical();
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
    let laid = crate::layout::fragment::types::frag_copy(&mut k.el, &mut k.frags, copy)
        .map(|el| el.layout_as_root(space, window, cx))
        .unwrap_or_default();
    let measured_flow = k
        .watched
        .as_ref()
        .map(|w| w.flow.at(point(px(0.0), px(0.0)), laid, axis, window));
    let sz = laid;
    let sz = if vertical {
        size(sz.height, sz.width)
    } else {
        sz
    };
    #[cfg(feature = "wpt-harness")]
    if let Some((h, ..)) = &k.shape
        && k.watched.is_some()
    {
        k.probe.height("pages", *h, f32::from(sz.height));
    }
    // Мера поддерева даёт точки разреза внутри ребёнка; без неё
    // ребёнок — цельный кусок измеренной высоты.
    let mut avoid = Vec::new();
    let mut keep = Vec::new();
    let (mut h, mut cuts, mut forced, mut solid) = match &k.shape {
        Some((h, cuts, forced, solid)) => (*h, cuts.clone(), forced.clone(), solid.clone()),
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
                let geometry = w.laid(origin, window);
                let flex = !geometry.flexes.is_empty();
                if let Some(root) = w.blocks().filter(|b| b.table.is_some() || flex)
                    && k.float_id.is_none()
                    && let Some((_, bottom)) = geometry.at(root.id)
                {
                    h = bottom;
                }
                if let Some(r) = k.regrow.as_mut() {
                    r.laid = geometry;
                }
                let m = w.breaks(origin, laid, h + lift, axis, window);
                if flex {
                    avoid = m.avoid;
                    keep = m.keep;
                }
                #[cfg(feature = "wpt-harness")]
                if k.shape.is_some() {
                    k.probe.breaks("pages.breaks", &cuts, &m.cuts);
                }
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
        force_before: k.renamed || k.measured_edge(false).0,
        force_after: k.measured_edge(true).0,
        avoid_before: k.measured_edge(false).1,
        avoid_after: k.measured_edge(true).1,
        forced,
        solid,
        avoid,
        keep,
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
}
