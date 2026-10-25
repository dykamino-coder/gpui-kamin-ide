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
) -> (Kid, crate::layout::fragment::flow::inline::Geometry) {
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
    let (laid, content) = crate::layout::fragment::types::frag_copy(&mut k.el, &mut k.frags, copy)
        .map(|el| el.layout_as_root_with_content(space, window, cx))
        .unwrap_or_default();
    let measured_flow = k
        .watched
        .as_ref()
        .map(|w| w.flow.at(point(px(0.0), px(0.0)), laid, axis, window));
    let mut inline = crate::layout::fragment::flow::inline::Geometry::default();
    inline.paragraphs = measured_flow.as_ref().map_or_else(Vec::new, |flow| {
        flow.inline
            .iter()
            .map(|p| p.translated(-k.inner_top))
            .collect()
    });
    inline.floats = measured_flow.as_ref().map_or_else(Vec::new, |flow| {
        flow.float_contexts
            .iter()
            .map(|f| f.translated(-k.inner_top))
            .collect()
    });
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
    let mut over = h;
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
                .map(|g| g.bottom + g.margins[1])
                .fold(h, f32::max);
        }
        match k.watched.clone().filter(|w| w.has_blocks()) {
            // Точки разрыва блочной стопки — по раскладке копии, а
            // не по оценке формы (`flow::blocks`). Мера формы
            // отсчитывает смещения от border box коробки, копия
            // поднята на `inner_top`.
            Some(w) => {
                let lift = k.inner_top;
                let (origin, frame) = match axis {
                    StackAxis::Horizontal => (point(px(0.0), px(lift)), laid),
                    StackAxis::VerticalLr => (point(px(lift), px(0.0)), laid),
                    StackAxis::VerticalRl => (
                        point(px(0.0), px(0.0)),
                        size(laid.width - px(lift), laid.height),
                    ),
                };
                // Таблица: высота — её разложенная обёртка (ряды,
                // подписи, заданная высота), а не оценка.
                let geometry = w.laid_axis(origin, frame, axis, window);
                let flex = !geometry.flexes.is_empty();
                let regrown = k.regrow.as_ref().is_some_and(|r| r.laid_h);
                if let Some(root) = w
                    .blocks()
                    .filter(|b| k.shape.is_none() || b.table.is_some() || flex || regrown)
                    && k.float_id.is_none()
                    && let Some((_, bottom)) = geometry.at(root.id)
                {
                    h = bottom;
                }
                let visible = w.blocks().is_some_and(|b| !b.clips_block);
                if visible && let Some(root) = w.blocks() {
                    over = crate::layout::fragment::flow::flow_end(root, &geometry).unwrap_or(h);
                    over = flow.solid.iter().map(|p| p.1 - lift).fold(over, f32::max);
                }
                if visible && k.float_extent {
                    over = float_extent(&flow, lift, over);
                }
                if k.shape.is_none() && k.regrow.is_some() {
                    // Реальное переполнение копии резервирует последующие листы.
                    let reach = f32::from(if vertical {
                        content.width
                    } else {
                        content.height
                    }) - lift;
                    k.reach = (reach > h + 0.01).then_some(reach);
                }
                if let Some(r) = k.regrow.as_mut() {
                    r.laid = geometry;
                    r.space = w.space(window);
                }
                let m = w.breaks(origin, frame, h.max(over) + lift, axis, window);
                inline.barriers = m.barriers;
                inline.keep = m.avoid_inside;
                if flex || k.shape.is_none() {
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
            None => {
                // Анонимная группа сохраняет общий IFC и обтекание, но её
                // нормальная высота не включает нижние края флоатов.
                if k.float_extent {
                    h = float_extent(&flow, k.inner_top, h);
                }
                anonymous_breaks(flow, &mut cuts, &mut solid, &mut inline);
            }
        }
    }
    (
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
            over: h.max(over),
            // Страницы: `box-decoration-break` пока `slice` — весь
            // кластер `clone` в колонках.
            clone_dec: None,
            // У страниц своё правило переполнения монолита (`fill_at`,
            // ветка `paged && placed && cur > target`, crbug 1402540).
            overflow_top: false,
            repeat: k.repeat.as_ref().map_or_else(Default::default, |r| r.geom),
            par: Par::default(),
        },
        inline,
    )
}

fn float_extent(flow: &crate::layout::fragment::flow::FlowGeometry, lift: f32, h: f32) -> f32 {
    flow.floats
        .iter()
        .map(|g| g.bottom + g.margins[1] - lift)
        .fold(h, f32::max)
}

fn anonymous_breaks(
    flow: crate::layout::fragment::flow::FlowGeometry,
    cuts: &mut Vec<(f32, f32)>,
    solid: &mut Vec<(f32, f32)>,
    inline: &mut crate::layout::fragment::flow::inline::Geometry,
) {
    inline.barriers.clone_from(&flow.solid);
    flow.merge(cuts, solid);
}

#[cfg(test)]
mod tests;
