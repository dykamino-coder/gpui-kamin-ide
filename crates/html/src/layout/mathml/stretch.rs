//! Intrinsic-измерение, затем построение оператора с ограничениями.
use super::{geometry::Metrics, parameters::Kind};
use gpui::{AnyElement, App, AvailableSpace, LayoutId, Window, size};
#[derive(Clone, Copy)]
pub(super) struct Target {
    pub length: f32,
    pub vertical: bool,
}
pub(super) type Factory = Box<dyn Fn(Target) -> AnyElement>;
pub(super) async fn measure(
    child: &mut AnyElement,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) -> (LayoutId, Metrics) {
    let available = size(AvailableSpace::MaxContent, AvailableSpace::MaxContent);
    let root = stack
        .run(|stack| child.request_layout_async(window, cx, stack))
        .await;
    stack
        .run(|stack| window.compute_layout_async(root, available, cx, stack))
        .await;
    let mut measurement = window.snapshot_layout_measurement(root);
    let (_, first, _) = stack
        .run(|stack| measurement.measure_async(available, window, cx, stack))
        .await;
    let (_, size) = window.layout_exact(root);
    let ascent = f32::from(first.unwrap_or(size.height));
    (
        root,
        Metrics {
            width: size.width.into(),
            ascent,
            descent: f32::from(size.height) - ascent,
        },
    )
}
pub(super) async fn constrain(
    kind: Kind,
    children: &mut [AnyElement],
    factories: &[Option<(u8, Factory)>],
    (metrics, roots): (&mut [Metrics], &mut [LayoutId]),
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) {
    let vertical = matches!(kind, Kind::Row | Kind::Padded(_));
    if !vertical && !matches!(kind, Kind::Limits(..)) {
        return;
    }
    let length = metrics
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            factories
                .get(*i)
                .and_then(Option::as_ref)
                .is_none_or(|(flags, _)| (*flags & 16 == 0) != vertical)
        })
        .map(|(_, m)| {
            if vertical {
                m.ascent + m.descent
            } else {
                m.width
            }
        })
        .fold(0.0, f32::max);
    for (i, factory) in factories.iter().enumerate() {
        let Some((flags, factory)) = factory else {
            continue;
        };
        if (*flags & 16 == 0) != vertical {
            continue;
        }
        let natural = if vertical {
            metrics[i].ascent + metrics[i].descent
        } else {
            metrics[i].width
        };
        if length <= natural {
            continue;
        }
        let mut child = factory(Target { length, vertical });
        let (root, m) = measure(&mut child, window, cx, stack).await;
        children[i] = child;
        roots[i] = root;
        metrics[i] = m;
    }
}
