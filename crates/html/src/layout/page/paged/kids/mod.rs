//! Дети листов из групп корневых узлов: копии ICB/fixed, имена страниц, формы.

use super::PAGE_COPIES;
use crate::dom::Node;
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::oof_reach;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::table_bands::repeat_candidates;
use crate::layout::options::RenderOpts;
use crate::layout::page::paged::visible_overflow;
use crate::style::computed::{BoxStyle, Display};
use crate::style::values::value::Len;
use crate::tolerance::COORD_EPS;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, px};
mod breaks;
mod flex;
mod frame;
mod laid;
mod repeat;
mod shape;
pub(super) use shape::group_shape;
mod names;
use names::group_break_names;
#[allow(clippy::too_many_arguments)]
pub(super) fn page_kids_from_groups(
    opts: &RenderOpts,
    root: &BoxStyle,
    left: f32,
    right: f32,
    top: f32,
    root_page: String,
    shape_cx: ShapeCx,
    icb_copies: &mut [Vec<AnyElement>],
    fixed_copies: &mut [Vec<AnyElement>],
    icb_reach: &mut f32,
    kids: &mut Vec<super::super::page_stack::PageKid>,
    mut prev_end: Option<String>,
    mut first: bool,
    aw: f32,
    ah: f32,
    layer: impl Fn(Vec<AnyElement>) -> Vec<AnyElement>,
    floated: impl Fn(&Node) -> bool,
    groups: Vec<Vec<Node>>,
    vertical: Option<bool>,
) {
    let vcx = opts.cx.vcx();
    for group in &groups {
        let n = &group[0];
        if let Node::Element(e) = n
            && matches!(e.style.used().display, Some(Display::None))
        {
            continue;
        }
        if let Node::Element(e) = n
            && out_of_flow(&e.style.used())
        {
            *icb_reach = icb_reach.max(oof_reach(e, shape_cx, vcx));
        }
        let first_kid = first;
        let pad_top = if first { top } else { 0.0 };
        first = false;
        let prepared = match vertical {
            Some(rl) => laid::vertical_group(
                group,
                root,
                ah - top,
                if first_kid {
                    if rl { right } else { left }
                } else {
                    0.0
                },
                rl,
                vcx,
            ),
            None => laid::group(group, root, aw - left - right, pad_top, vcx),
        };
        let group = prepared
            .as_ref()
            .map_or(group.as_slice(), |p| p.nodes.as_slice());
        let n = &group[0];
        let (margins, shape) = match (&prepared, vertical) {
            (Some(p), _) => (p.margins, None),
            (_, Some(_)) => ((0.0, 0.0, 0.0), None),
            _ => group_shape(root, shape_cx, group, n, pad_top, vcx),
        };
        let watched = breaks::recorder(
            group,
            n,
            vertical,
            shape.is_some(),
            prepared.as_ref().is_some_and(|p| p.blocks),
            shape_cx,
            vcx,
        );
        // Таблица, чьи ряды даёт раскладка копии: может ли повторяться её
        // шапка и подвал (`break-inside: avoid*` секции).
        let measured_table = watched
            .blocks()
            .and_then(|b| b.table.as_ref())
            .map(|t| (t.head.is_some_and(|s| s.2), t.foot.is_some_and(|s| s.2)));
        let build = |main: bool,
                     band: bool,
                     slot: &mut Vec<AnyElement>,
                     fixed_slot: &mut Vec<AnyElement>| {
            let mut opts = opts.clone();
            opts.frag.watch = main.then(|| watched.clone());
            let icb_layer =
                crate::layout::positioned::layer_scope::LayerScope::icb(&opts.cx.position.state);
            opts.clear_paged_fixed();
            // Оставленная обёртка (`html`/`body`) с долей высоты считает её от
            // page area — содержащего блока корня (css-page-3 §page-model).
            let mut wrap = frame::wrap((left, right, top), ah, first_kid, vertical);
            if let [Node::Element(r)] = group
                && matches!(r.tag.as_str(), "html" | "body")
                && matches!(r.style.used().height, Some(Len::Pct(_)))
            {
                wrap = wrap.h(px(ah));
            }
            let band_group: Vec<Node> = if band {
                group
                    .iter()
                    .filter_map(|n| {
                        crate::layout::positioned::repeat::drop_uncontained_oof(n, false, false)
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let source = if band { band_group.as_slice() } else { group };
            let el = wrap
                .children(opts.cx.contracts.build.blocks(source, root, &opts))
                .into_any_element();
            slot.extend(layer(icb_layer.finish()));
            fixed_slot.extend(layer(opts.take_paged_fixed()));
            el
        };
        let regrow = flex::regrow(
            opts,
            root,
            group,
            &watched,
            (left, right, top),
            first_kid,
            (aw, ah),
            vertical,
            icb_copies,
            fixed_copies,
        );
        let (el, frags) = match &regrow {
            Some(r) => r.rebuild(PAGE_COPIES - 1),
            None => (
                build(true, false, &mut icb_copies[0], &mut fixed_copies[0]),
                (1..PAGE_COPIES)
                    .map(|i| build(false, false, &mut icb_copies[i], &mut fixed_copies[i]))
                    .collect(),
            ),
        };
        // CSS Paged Media: anonymous runs inherit the page name; floats
        // and out-of-flow boxes do not pass a name to subsequent pages.
        let (fb, fa, monolith, renamed, start_name) =
            group_break_names(&root_page, &mut prev_end, &floated, group, n, vcx);
        // Монолит выше листа решается в `fill` (правило «сначала перенос,
        // потом разрыв внутри»): здесь мера считается и для него — точки
        // класса A нужны, когда он окажется с верха страницы.
        // Visible overflow past the box (unclamped flow measure) reserves
        // pages after it (css-break-3 §2.1 parallel overflow; Blink
        // `ReserveSpaceForMonolithicOverflow`, `monolithic-overflow-026`).
        let reach = match (n, &shape) {
            (Node::Element(e), Some(s))
                if vertical.is_none() && visible_overflow(&e.style.used()) =>
            {
                let cx = ShapeCx {
                    unclamped: true,
                    ..shape_cx
                };
                // TRANSITIONAL(F2-2): Reach ребёнка страницы оценивается как полная unclamped форма; удалить в P08 после child token и разделения normal size и OOF reach.
                shape_full(e, 4, cx, vcx)
                    .map(|u| u.height + pad_top)
                    .filter(|&u| u > s.0 + COORD_EPS)
            }
            _ => None,
        };
        kids.push(crate::layout::page::page_stack::PageKid {
            #[cfg(feature = "wpt-harness")]
            probe: opts.frag.probe.clone(),
            repeat: vertical
                .is_none()
                .then(|| {
                    let band = || build(false, true, &mut Vec::new(), &mut Vec::new());
                    match measured_table {
                        Some((head, foot)) => repeat_candidates(
                            head,
                            foot,
                            || (1..PAGE_COPIES).map(|_| band()).collect(),
                            || (0..PAGE_COPIES).map(|_| band()).collect(),
                        ),
                        None => {
                            repeat::sections(group, ah, shape_cx, pad_top, PAGE_COPIES, band, vcx)
                        }
                    }
                })
                .flatten(),
            regrow,
            watched: Some(watched),
            float_id: match n {
                Node::Element(e) if floated(n) => Some(e.node_id),
                _ => None,
            },
            float_extent: prepared.as_ref().is_some_and(|p| p.float_extent),
            reach,
            end: Default::default(),
            el,
            frags,
            monolith,
            force_before: fb || renamed,
            renamed,
            force_after: fa,
            shape,
            page: start_name,
            mt: margins.0,
            mb: margins.1,
            inner_top: margins.2,
        });
    }
}
