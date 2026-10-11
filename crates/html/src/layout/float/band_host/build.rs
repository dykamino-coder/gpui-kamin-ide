//! Построение хоста полос (band_host): края, кегль и поля кусков.

use super::{BandPiece, band_piece, px_margin_box};
use crate::dom::{Element, Node};
use crate::layout::positioned::static_position::at_static_position;
use crate::render::is_blank;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// Синтетический узел бандового хоста для пробега флоатов, начинающегося на
/// `i`; вернуть хост и номер узла ЗА хвостом.
///
/// Отличий от сегодняшнего пробега (`wrap_floats:1985-2038`) три:
///
/// 1. пробег НЕ обрывается ни на смене стороны, ни на `clear` — обе стороны
///    и очистка уезжают в один хост;
/// 2. `style.float` и `style.clear` с детей НЕ снимаются: их читает
///    `shape_flow`, который гасит их сам перед сборкой (`:5091`);
/// 3. флоатам не задаются ни `flex_shrink`, ни `fit-content` — ряда, ради
///    которого это делалось, здесь нет, а размер и так обязан быть в точках.
///
/// Гейт (все условия разом, иначе `None`):
///
/// * ширина содержащего блока известна ТОЧКАМИ и больше нуля — без неё
///   полосам негде поставить дальнюю стенку (`shape_flow` ставит
///   недостижимую `NO_WALL`, и сужения не будет вовсе);
/// * у каждого флоата пробега margin-box в точках;
/// * хвост непустой и состоит ТОЛЬКО из пустого текста и кусков `BandPiece`.
///
/// Последнее условие и держит радиус поражения: любой абзац, любой блок без
/// размеров, любой текст рядом с флоатом уводит на сегодняшний флекс-ряд.
pub(crate) fn band_host(
    nodes: &[Node],
    i: usize,
    cb_width: Option<Len>,
    lead: &[Node],
) -> Option<(Element, usize, Vec<Node>)> {
    let cb_w = match cb_width {
        Some(Len::Px(v)) if v > 0.0 => v,
        _ => return None,
    };
    let mut floaters: Vec<Element> = vec![];
    let mut j = i;
    while j < nodes.len() {
        if is_blank(&nodes[j]) {
            j += 1;
            continue;
        }
        let Node::Element(next) = &nodes[j] else {
            break;
        };
        if !next.style.float.is_some_and(|f| f != 0) {
            break;
        }
        // Размер флоата обязан быть в точках: полосы ничего не мерят, а
        // `width: auto` у флоата — это shrink-to-fit (§10.3.5), который
        // `px_margin_box` сложил бы как НОЛЬ и посадил флоат нулевой ширины.
        if !matches!(next.style.width, Some(Len::Px(_)))
            || !matches!(next.style.height, Some(Len::Px(_)))
        {
            return None;
        }
        px_margin_box(&next.style)?;
        floaters.push(next.clone());
        j += 1;
    }
    if floaters.is_empty() {
        return None;
    }
    let mut rest: Vec<Node> = vec![];
    let mut lifted: Vec<Node> = vec![];
    while j < nodes.len() {
        if let Node::Element(next) = &nodes[j]
            && (next.style.float.is_some_and(|f| f != 0) || next.style.clear.is_some())
        {
            break;
        }
        // Внепоточный сосед (absolute/fixed не на статической позиции) ни
        // строк, ни полос не занимает (CSS 2.1 §9.6): он уходит ЗА хост, как
        // и на пути `wrap_floats`. Прежде он отменял хост целиком, и пробег
        // правых флоатов падал во флекс-ряд, ставивший их бок о бок даже без
        // места (`shape-outside-circle-034-ref`: второй флоат 120 рядом с
        // первым в блоке 200 вместо места под ним, §9.5.1 п.3).
        if let Node::Element(next) = &nodes[j]
            && matches!(
                next.style.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            )
            && !at_static_position(&next.style)
        {
            lifted.push(nodes[j].clone());
            j += 1;
            continue;
        }
        rest.push(nodes[j].clone());
        j += 1;
    }
    if !lead.is_empty() {
        // Прогон отдаём полосам ЦЕЛИКОМ и только когда он вместе с флоатами
        // помещается в одну строку: разъехавшийся прогон — это перенос, а
        // его считает наборщик строк, не полосы.
        if !lead
            .iter()
            .all(|n| is_blank(n) || band_piece(n) == Some(BandPiece::Atom))
        {
            return None;
        }
        let mut row = 0.0f32;
        for n in lead {
            if let Node::Element(e) = n {
                row += px_margin_box(&e.style)?.0;
            }
        }
        for f in &floaters {
            row += px_margin_box(&f.style)?.0;
        }
        if row > cb_w + 0.01 {
            return None;
        }
        let mut all = lead.to_vec();
        all.extend(rest);
        rest = all;
    }
    // Пустой хвост хост НЕ отменяет, если флоатов НЕСКОЛЬКО: лесенку
    // (§9.5.1 п.5) и правило 3 флекс-ряд не выражает вовсе. Одинокий флоат с
    // пустым хвостом полосам не нужен — его кладёт ветка ниже, и хост ей
    // только мешал (замерено: с пустым хвостом при любом числе флоатов
    // приобретено 10, потеряно 9).
    // ЗАМЕРЕНО: отсекать здесь ещё и пробеги с `clear` — 5065 -> 5064.
    if (floaters.len() < 2 && !rest.iter().any(|n| !is_blank(n)))
        || !rest.iter().all(|n| is_blank(n) || band_piece(n).is_some())
    {
        return None;
    }
    let side = floaters[0].style.float.unwrap_or(-1);
    let mut host = Element {
        list_item: None,
        node_id: 0,
        anim: None,
        tag: "shape-flow".into(),
        style: Computed {
            // Ширина содержащего блока — дальняя стенка полос.
            width: cb_width,
            ..Computed::default()
        },
        hover: None,
        first_letter: None,
        first_line: None,
        children: Vec::new(),
        attrs: vec![
            // Сторона первого флоата — только запасной ответ для живого пути
            // `shape-outside`: у бандового хоста сторона лежит на КАЖДОМ
            // ребёнке, и `shape_flow` читает её оттуда.
            (
                "side".into(),
                if side < 0 {
                    "left".into()
                } else {
                    "right".into()
                },
            ),
            ("count".into(), floaters.len().to_string()),
            // Метка бандового хоста: включает блочную ветку `shape_flow`.
            ("bands".into(), "1".into()),
        ],
        inline: false,
    };
    host.children = floaters.into_iter().map(Node::Element).collect();
    host.children.extend(rest);
    Some((host, j, lifted))
}

/// Поле для измеряемого хоста: точки, доля ширины содержащего блока или
/// `em` по кеглю `em`; `auto` — ноль (у флоата так велит §10.3.5, у куска
/// хвоста — как `px_margin` статического хоста). Прочее (`calc`, `vw`…) —
/// `None`, хост отменяется.
pub(crate) fn band_edge(l: &Option<Len>, em: f32) -> Option<crate::layout::float::band_flow::Edge> {
    use crate::layout::float::band_flow::Edge;
    match l {
        None | Some(Len::Auto) => Some(Edge::Px(0.0)),
        Some(Len::Px(v)) => Some(Edge::Px(*v)),
        Some(Len::Pct(k)) => Some(Edge::Pct(*k)),
        Some(Len::Em(k)) => Some(Edge::Px(*k * em)),
        _ => None,
    }
}

/// Кегль коробки для `em` её полей; относительный `font-size` берёт
/// кегль родителя, в том числе проценты (CSS 2.1 §15.7).
pub(crate) fn band_em(c: &Computed, em: f32) -> Option<f32> {
    match c.font_size {
        None => Some(em),
        Some(Len::Px(v)) => Some(v),
        Some(Len::Em(k)) | Some(Len::Pct(k)) => Some(k * em),
        _ => None,
    }
}

/// Все четыре поля коробки разрешимы для измеряемого хоста.
pub(crate) fn band_margins(
    c: &Computed,
    em: f32,
) -> Option<[crate::layout::float::band_flow::Edge; 4]> {
    let em = band_em(c, em)?;
    Some([
        band_edge(&c.margin.top, em)?,
        band_edge(&c.margin.right, em)?,
        band_edge(&c.margin.bottom, em)?,
        band_edge(&c.margin.left, em)?,
    ])
}
