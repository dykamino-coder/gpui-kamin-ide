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
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};
mod breaks;
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
            && matches!(e.style.computed().display, Some(Display::None))
        {
            continue;
        }
        if let Node::Element(e) = n
            && out_of_flow(&e.style.computed())
        {
            *icb_reach = icb_reach.max(oof_reach(e, shape_cx, vcx));
        }
        let first_kid = first;
        let pad_top = if first { top } else { 0.0 };
        first = false;
        // Мера поддерева — те же точки разреза, что у колонок. Обёртка
        // первого ребёнка несёт отбивку корня сверху (`pad_top`): все
        // смещения меры сдвигаются на неё, а высота растёт.
        // Внепоточный корня места в стопке не занимает (CSS 2.1 §9.3.1):
        // его мера — обёртка (0); иначе абсолют `height: 4in` двигал соседей
        // на 384 (`monolithic-overflow-027`, абсолют `fixedpos-007` — 720).
        // Флоат: не влезший MARGIN box уходит на следующую страницу целиком
        // (`float-with-large-margin-bottom-cross-page-002`: эталон —
        // `break-before: page`), нижнее поле — часть его меры.
        // Флоат корня — ребёнок стопки СО СВОЕЙ мерой: css-break-4 §3.1
        // «User agents should also apply these properties to floated boxes
        // whose containing block is in the normal flow of the root fragmented
        // element». Гейт `out_of_flow` отнимал у него меру вместе с
        // абсолютами: таблица `float: left; break-inside: avoid` выше листа
        // резалась срезом по краю сквозь абзац вместо точки класса A между
        // абзацами ячейки (`float-page-break-inside-avoid-1-print` против
        // эталона с обычной таблицей), а ветка `h + mb` ниже была мёртвой.
        // The subtree measure is physical (heights): a vertical root has no
        // class-A cut list yet and its kids are sliced at the page edge.
        let (margins, shape) = match vertical {
            Some(_) => ((0.0, 0.0, 0.0), None),
            None => group_shape(root, shape_cx, group, n, pad_top, vcx),
        };
        let watched = breaks::recorder(group, n, vertical, shape.is_some(), shape_cx, vcx);
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
            // Writing Modes 4: vertical blocks advance along x. Repeat the
            // inline-start padding; slice block-start decoration on later pages
            // (CSS Break 3 section 5.4).
            let mut wrap = match vertical {
                Some(rl) => {
                    // Inline size: the page area's height (a block's `auto`
                    // inline size fills its containing block, CSS 2.1 §10.3.3).
                    let d = div().flex().h(px(ah)).pt(px(top));
                    match (rl, first_kid) {
                        (true, true) => d.flex_row_reverse().pr(px(right)),
                        (true, false) => d.flex_row_reverse(),
                        (false, true) => d.flex_row().pl(px(left)),
                        (false, false) => d.flex_row(),
                    }
                }
                None => div().pl(px(left)).pr(px(right)).pt(px(pad_top)),
            };
            if let [Node::Element(r)] = group.as_slice()
                && matches!(r.tag.as_str(), "html" | "body")
                && matches!(r.style.computed().height, Some(Len::Pct(_)))
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
        let el = build(true, false, &mut icb_copies[0], &mut fixed_copies[0]);
        let frags: Vec<AnyElement> = (1..PAGE_COPIES)
            .map(|i| build(false, false, &mut icb_copies[i], &mut fixed_copies[i]))
            .collect();
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
                if vertical.is_none() && visible_overflow(&e.style.computed()) =>
            {
                let cx = ShapeCx {
                    unclamped: true,
                    ..shape_cx
                };
                shape_full(e, 4, cx, vcx)
                    .map(|u| u.height + pad_top)
                    .filter(|&u| u > s.0 + COORD_EPS)
            }
            _ => None,
        };
        kids.push(crate::layout::page::page_stack::PageKid {
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
            watched: Some(watched),
            float_id: match n {
                Node::Element(e) if floated(n) => Some(e.node_id),
                _ => None,
            },
            reach,
            end: Default::default(),
            el,
            frags,
            monolith,
            force_before: fb || renamed,
            force_after: fa,
            shape,
            page: start_name,
            mt: margins.0,
            mb: margins.1,
            inner_top: margins.2,
        });
    }
}
