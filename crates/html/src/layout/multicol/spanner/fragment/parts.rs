//! Части многоколоночника между охватчиками (spanner_parts).

use super::super::{SpanPart, has_deep_spanner, passes_spanner, spanner_box, splits_for_spanner};
use super::{spanner_frag_visible, spanner_fragment, spanner_height_share};
use crate::dom::Node;
use crate::render::split_block_in_inline;
use crate::style::values::value::Len;

/// Разложить содержимое предка на чередование «кусок обычного потока» —
/// «спаннер», рекурсивно вынимая спаннеров из проходимых потомков. Список
/// всегда начинается и кончается куском потока (возможно пустым).
pub(crate) fn spanner_parts(kids: &[Node]) -> Vec<SpanPart> {
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
                let inner = spanner_parts(&c.children);
                let bodies: Vec<Vec<Node>> = inner
                    .iter()
                    .filter_map(|p| match p {
                        SpanPart::Body(b) => Some(b.clone()),
                        SpanPart::Span(_) => None,
                    })
                    .collect();
                let n_b = bodies.len();
                let shown = bodies
                    .iter()
                    .enumerate()
                    .filter(|(i, b)| spanner_frag_visible(c, b, *i == 0, *i + 1 == n_b))
                    .count();
                // Доли заданной высоты предка (`spanner_height_share`); без
                // них — прежнее правило `keep` ниже.
                let share = spanner_height_share(c, &bodies);
                let mut bi = 0usize;
                for p in inner {
                    match p {
                        SpanPart::Body(b) => {
                            let first = bi == 0;
                            let last = bi + 1 == n_b;
                            // Заданную высоту берёт только НЕ разошедшийся
                            // предок (см. `spanner_fragment`); пустой
                            // фрагмент без кромки и без такой высоты
                            // показывать нечем — он просто исчезает.
                            let own = share.as_ref().and_then(|s| s.get(bi).copied().flatten());
                            let keep = share.is_none() && first && shown <= 1;
                            if spanner_frag_visible(c, &b, first, last)
                                || own.is_some_and(|v| v > 0.0)
                                || (keep && c.style.height.is_some())
                                || (keep && c.style.min_height.is_some())
                            {
                                let mut f = spanner_fragment(c, b, first, last, keep, bi);
                                if let Some(v) = own {
                                    f.style.height = Some(Len::Px(v));
                                    f.style.min_height = None;
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
                            let s = match (c.style.opacity, &s) {
                                (Some(o), Node::Element(sp)) if o < 1.0 => {
                                    let mut sp = sp.clone();
                                    sp.style.opacity = Some(sp.style.opacity.unwrap_or(1.0) * o);
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
                                Node::Element(mut sp) if sp.attr("kamin-span-parent").is_none() => {
                                    sp.attrs.push((
                                        "kamin-span-parent".to_string(),
                                        c.node_id.to_string(),
                                    ));
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
