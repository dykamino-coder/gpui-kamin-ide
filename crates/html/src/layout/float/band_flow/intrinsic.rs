//! Парная min/max-content проба потомка на общем стеке обхода.
use super::kid::Kid;
use super::tap::Tap;
use super::{Build, Probe};
use crate::hooks::IntoHooked;
use gpui::{AvailableSpace, size};
use std::{cell::Cell, rc::Rc};

/// Внутренние ширины ребёнка (min-content, max-content) border-box.
pub(super) async fn intrinsic(
    kid: &Kid,
    p: &mut Probe<'_>,
    stack: &mut gpui::PaintStack,
) -> (f32, f32) {
    if let Some(size) = kid.intrinsic.get() {
        return size;
    }
    let old = p.fragment.as_ref().map(|f| f.probes.clone());
    if let (Some(fragment), Some(flow)) = (&mut p.fragment, &kid.flow) {
        flow.rewind();
        fragment.probes = flow.clone();
    }
    let size = intrinsic_of(&kid.build, p, stack).await;
    kid.intrinsic.set(Some(size));
    if let (Some(fragment), Some(old)) = (&mut p.fragment, old) {
        fragment.probes = old;
    }
    size
}

/// Внутренние ширины того, что строит `build` (ребёнок или голова прогона).
pub(super) async fn intrinsic_of(
    build: &Build,
    p: &mut Probe<'_>,
    stack: &mut gpui::PaintStack,
) -> (f32, f32) {
    p.temporary_layout(stack, async |p, stack| {
        stack.run(|stack| intrinsic_layout(build, p, stack)).await
    })
    .await
}

async fn intrinsic_layout(
    build: &Build,
    p: &mut Probe<'_>,
    stack: &mut gpui::PaintStack,
) -> (f32, f32) {
    crate::work::add!(BandIntrinsicCalls, 1);
    let positioning = p.positioning.clone();
    let _positioning = positioning.as_ref().map(|p| p.isolate());
    let _clamp_scope = p.clamp.as_ref().map(|(key, m)| m.enter(*key));
    let _flow_scope = p.fragment.as_ref().map(|f| f.probes.enter());
    let tap = Rc::new(Cell::new(None));
    let measure = p.vert.then(crate::text::vertical::InlineMeasure::default);
    crate::work::add!(FloatChildBuilds, 1);
    let inner = {
        crate::work::time!(_build, BandIntrinsicBuildNs);
        build(0.0, 0.0, None, None, measure.clone())
    };
    let mut el = Tap { inner, id: tap }.hooked();
    let root = stack
        .run(|stack| el.request_layout_async(p.window, p.cx, stack))
        .await;
    let vert = p.vert;
    let w = async |a: AvailableSpace, p: &mut Probe<'_>, stack: &mut gpui::PaintStack| {
        crate::work::add!(FloatIntrinsicQueries, 1);
        crate::work::time!(_layout, BandIntrinsicLayoutNs);
        if vert {
            // Повёрнутый текст свою длину высотой не заявляет (растягивается
            // окном) — берём её у самого текста через сборщик
            // intrinsic measurement; коробка без текста отвечает раскладкой.
            if let Some(measure) = &measure {
                measure.reset();
            }
            let size = p
                .window
                .measure_layout_size_async(
                    root,
                    size(AvailableSpace::MaxContent, a),
                    gpui::Axis::Vertical,
                    p.cx,
                    stack,
                )
                .await;
            let text = measure.as_ref().map_or(0.0, |measure| measure.extent());
            let laid = f32::from(size.height);
            if text > 0.0 { text.min(laid) } else { laid }
        } else {
            f32::from(
                p.window
                    .measure_layout_size_async(
                        root,
                        size(a, AvailableSpace::MaxContent),
                        gpui::Axis::Horizontal,
                        p.cx,
                        stack,
                    )
                    .await
                    .width,
            )
        }
    };
    let mn = w(AvailableSpace::MinContent, p, stack).await;
    let mx = w(AvailableSpace::MaxContent, p, stack).await;
    (mn, mx.max(mn))
}
