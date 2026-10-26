//! Части многоколоночника между охватчиками (spanner_parts).

use super::super::parent::ParentResume;
use super::super::{SpanPart, has_deep_spanner, passes_spanner, spanner_box, splits_for_spanner};
use super::{spanner_frag_visible, spanner_fragment};
use crate::dom::{Element, Node};
use crate::layout::block::inline_splits::split_block_in_inline;
use crate::layout::options::RenderOpts;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;

/// Разложить содержимое предка на чередование «кусок обычного потока» —
/// «спаннер», рекурсивно вынимая спаннеров из проходимых потомков. Список
/// всегда начинается и кончается куском потока (возможно пустым).
pub(crate) fn spanner_parts(
    kids: &[Node],
    parent: &BoxStyle,
    opts: &RenderOpts,
    width: f32,
    measure: &mut dyn FnMut(&Element, &BoxStyle, f32) -> super::super::SpannerMetrics,
) -> Vec<SpanPart> {
    // Спаннер под строчным предком: сперва разорвать строчные на анонимные
    // блоки (§9.2.1.1) — тогда спаннер виден на этом уровне и режет поток,
    // как прямой (`splits_for_spanner`).
    let split;
    let kids: &[Node] = if kids
        .iter()
        .any(|n| matches!(n, Node::Element(k) if splits_for_spanner(k) && has_deep_spanner(k)))
    {
        split = split_block_in_inline(kids);
        &split
    } else {
        kids
    };
    let mut out: Vec<SpanPart> = Vec::new();
    let mut body: Vec<Node> = Vec::new();
    for n in kids {
        match n {
            Node::Element(c) if spanner_box(c) => {
                out.push(SpanPart::Body(std::mem::take(&mut body)));
                out.push(SpanPart::Span(n.clone()));
            }
            Node::Element(c) if passes_spanner(c) && has_deep_spanner(c) => {
                let merged = box_style(parent, &c.style, opts.cx.vcx());
                let metrics = measure(c, parent, width);
                let child_width = metrics.content_width;
                let inner = spanner_parts(&c.children, &merged, opts, child_width, measure);
                let bodies: Vec<&Vec<Node>> = inner
                    .iter()
                    .filter_map(|p| match p {
                        SpanPart::Body(b) => Some(b),
                        SpanPart::Span(_) => None,
                    })
                    .collect();
                let n_b = bodies.len();
                let shown = bodies
                    .iter()
                    .enumerate()
                    .filter(|(i, b)| spanner_frag_visible(c, b, *i == 0, *i + 1 == n_b))
                    .count();
                let split_size = shown > 1
                    || bodies[..n_b - 1]
                        .iter()
                        .all(|b| b.iter().all(crate::dom::whitespace::is_blank));
                let mut continuation = ParentResume::new(metrics, c.style.used().bdb_clone);
                let mut bi = 0usize;
                for p in inner {
                    match p {
                        SpanPart::Body(b) => {
                            let first = bi == 0;
                            let last = bi + 1 == n_b;
                            let visible = spanner_frag_visible(c, &b, first, last);
                            let keep = !split_size && first && shown <= 1;
                            let mut f = spanner_fragment(c, b, first, last, keep, bi);
                            let own = if split_size {
                                let block_size = if !last && visible {
                                    measure(&f, parent, width).block_size
                                } else {
                                    0.
                                };
                                continuation.resume(block_size, first, last)
                            } else {
                                None
                            };
                            if visible
                                || own.is_some_and(|v| v > 0.)
                                || (keep
                                    && (c.style.used().height.is_some()
                                        || c.style.used().min_height.is_some()))
                            {
                                if let Some(v) = own {
                                    f.style.used_mut().height = Some(Len::Px(v));
                                }
                                body.push(Node::Element(f));
                            }
                            bi += 1;
                        }
                        SpanPart::Span(s) => {
                            // Спаннер потомка поднимается на НАШ уровень и
                            // режет уже наш поток: предки разрезаются вместе
                            // с ним (§column-span). Прозрачность предка при
                            // этом остаётся на спаннере: «Although the
                            // spanner is taken out-of-flow, this does not
                            // affect the painting order of the spanning
                            // element» (`Overview.bs:1471-1472`), а группа
                            // прозрачности — часть отрисовки
                            // (`spanner-in-opacity`). Трансформ и фильтр
                            // сюда не попадают вовсе: они барьер
                            // (`passes_spanner`).
                            let s = match (c.style.used().opacity, &s) {
                                (Some(o), Node::Element(sp)) if o < 1.0 => {
                                    let mut sp = sp.clone();
                                    sp.style.used_mut().opacity =
                                        Some(sp.style.used().opacity.unwrap_or(1.0) * o);
                                    Node::Element(sp)
                                }
                                _ => s,
                            };
                            // Родитель спаннера в ИСХОДНОМ дереве. После подъёма
                            // спаннеры разных предков стоят рядом, но между ними
                            // лежит граница предка (его пустой фрагмент в ряду
                            // колонок, §column-span `Overview.bs:1540`), и поля их
                            // НЕ схлопываются (`multicol-span-all-margin-nested-
                            // 001`: «the bottom margin of the first h4 element
                            // should not collapse with the top margin of
                            // div#child»). Метку ставит ближайший предок; глубже
                            // уже помеченные не трогаются. Читает её сегментный
                            // путь в `element()`.
                            let s = match s {
                                Node::Element(mut sp) if sp.marks.span_parent.is_none() => {
                                    sp.marks.span_parent = Some(c.node_id);
                                    Node::Element(sp)
                                }
                                other => other,
                            };
                            out.push(SpanPart::Body(std::mem::take(&mut body)));
                            out.push(SpanPart::Span(s));
                        }
                    }
                }
            }
            other => body.push(other.clone()),
        }
    }
    out.push(SpanPart::Body(body));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::multicol::spanner::SpannerMetrics;

    #[test]
    fn owned_parent_remainder_is_not_clamped_again_by_original_constraints() {
        fn host(nodes: &[Node]) -> Option<&Element> {
            nodes.iter().find_map(|node| match node {
                Node::Element(e) if e.tag == "div" => Some(e),
                Node::Element(e) => host(&e.children),
                _ => None,
            })
        }
        for clone in [false, true] {
            let tree = crate::dom::parse(
                &format!(
                    "<div style='height:100px; min-height:150px; max-height:180px; box-sizing:border-box; box-decoration-break:{}'><p>before</p><h2 style='column-span:all'>span</h2><p>after</p></div>",
                    if clone { "clone" } else { "slice" }
                ),
                "",
            );
            let e = host(&tree).expect("spanner parent");
            let opts = RenderOpts {
                text: Default::default(),
                viewport: (800., 600.),
                normal_line_height: 1.2,
                cx: Default::default(),
                frag: Default::default(),
            };
            let parts = spanner_parts(
                &[Node::Element(e.clone())],
                &BoxStyle::default(),
                &opts,
                200.,
                &mut |c, _, _| SpannerMetrics {
                    block_size: if has_deep_spanner(c) { 115. } else { 30. },
                    content_width: 200.,
                    content_size: Some(100.),
                    start_edge: 10.,
                    end_edge: 5.,
                },
            );
            let fragments: Vec<&Element> = parts
                .iter()
                .filter_map(|part| match part {
                    SpanPart::Body(nodes) => nodes.iter().find_map(|node| match node {
                        Node::Element(e) if e.tag == "div" => Some(e),
                        _ => None,
                    }),
                    SpanPart::Span(_) => None,
                })
                .collect();
            assert_eq!(fragments.len(), 2);
            assert_eq!(fragments[0].style.used().height, None);
            assert_eq!(
                fragments[1].style.used().height,
                Some(Len::Px(if clone { 85. } else { 80. }))
            );
            for f in fragments {
                assert_eq!(f.style.used().min_height, None);
                assert_eq!(f.style.used().max_height, None);
                assert_eq!(f.style.used().border_box, None);
            }
            assert_eq!(e.style.used().height, Some(Len::Px(100.)));
            assert_eq!(e.style.used().min_height, Some(Len::Px(150.)));
            assert_eq!(e.style.used().max_height, Some(Len::Px(180.)));
            assert_eq!(e.style.used().border_box, Some(true));
        }
    }
}
