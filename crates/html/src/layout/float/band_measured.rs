//! Замеренные полосы обтекания.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::block::margins::collapse_margins;
use crate::layout::block::struts::zero_len;
use crate::layout::float::band_clearance::supported as band_clear_supported;
use crate::layout::float::band_flow_host::{BAND_CBH, BAND_CBW, band_flow_block, band_flow_rest, band_flow_rest_lift, band_orthogonal, subtree_has_text};
use crate::layout::float::band_host::{BandPiece, band_em, band_margins, band_piece};
use crate::layout::float::band_nest::{band_nest_block, band_seq};
use crate::render::{block_level_in_flow, is_blank, out_of_flow, own_context, replaced_tag};
use crate::style::computed::{Computed, Display};

/// Кусок хвоста измеряемого хоста: `Some(true)` — коробка, флоаты не
/// перекрывающая (§9.5, последний абзац), `Some(false)` — распорка, `None` —
/// не годится (хост отменяется).
///
/// В отличие от `band_piece` размеры коробки своего контекста в стиле не
/// нужны: ширину окна и высоту из содержимого даёт пробная раскладка
/// (`band_flow::plan`). Атомы строки сюда НЕ пускаются: они делят строку, а
/// здесь каждый кусок берёт своё окно.
pub(super) fn band_piece_m(n: &Node, em: f32) -> Option<bool> {
    match band_piece(n) {
        Some(BandPiece::Strut) => return Some(false),
        Some(BandPiece::Atom) => return None,
        _ => {}
    }
    let Node::Element(c) = n else {
        return None;
    };
    if matches!(
        c.style.position,
        Some(crate::style::computed::Position::Absolute) | Some(crate::style::computed::Position::Fixed)
    ) || c.style.float.is_some_and(|f| f != 0)
        || (c.style.clear.is_some() && !band_clear_supported(c))
        || matches!(
            c.style.display,
            Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
                | Some(Display::InlineTable)
                | Some(Display::TableCell)
                | Some(Display::None)
        )
        // Строчный по природе тег строчен, только пока `display` не задан:
        // `<img style="display:block">` — блочного уровня.
        || (c.inline && c.style.display.is_none())
        || band_orthogonal(&c.style)
        // Таблица и замещаемый блочного уровня §9.5 названы прямо; тег
        // `<table>` вида в стиле не несёт — его строит сборщик таблиц по тегу
        // (`"table" =>` в `element`); `<img style="display:block">` своего
        // контекста не заводит, но флоаты не перекрывает (`floats-039`).
        || !(own_context(c) || c.tag == "table" || (block_level_in_flow(c) && replaced_tag(c)))
    {
        return None;
    }
    band_margins(&c.style, em)?;
    Some(true)
}

/// Измеряемый бандовый хост (шаги F2, F3, F5 `scout-float-bands-design.md`):
/// тот же пробег флоатов и хвост из кусков, что у `band_host`, но размеры,
/// которых нет в стиле, меряет раскладка (`band_flow::BandFlow`).
///
/// Гейт (все условия разом):
///
/// * у каждого флоата пробега поля разрешимы (`band_margins`); размеры любые
///   — shrink-to-fit §10.3.5 считает проба;
/// * хвост — только пустой текст и куски `band_piece_m`;
/// * одинокий флоат с пустым хвостом полосам не нужен (как у `band_host`).
///
/// Ширина содержащего блока не требуется вовсе: её отдаёт замер.
pub(super) fn band_host_m(
    nodes: &[Node],
    i: usize,
    em: f32,
    lead: &[Node],
) -> Option<(Element, usize, Vec<Node>)> {
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
        band_float_m(next, em)?;
        floaters.push(next.clone());
        j += 1;
    }
    if floaters.is_empty() {
        return None;
    }
    // Щупы ширины строчного содержимого перед флоатами (`Kid::lead`):
    // (номера флоатов, узлы перед ними в той же строке). Флоаты пробега
    // стоят после `lead` (`wrap_floats`).
    let mut probes: Vec<(usize, usize, Vec<Node>)> = vec![];
    if !lead.is_empty() {
        probes.push((0, floaters.len(), lead.to_vec()));
    }
    let mut rest: Vec<Node> = vec![];
    while j < nodes.len() {
        // С шагом F6 очищающая коробка остаётся в хосте: clearance считают
        // полосы (`band_flow::plan`), а не распорка флекс-ряда.
        if let Node::Element(next) = &nodes[j]
            && (next.style.float.is_some_and(|f| f != 0)
                || (next.style.clear.is_some() && !band_clear_supported(next)))
        {
            // Флоат дальше по той же строке прогона («BEF<float>Inner
            // <float>AFTER», `::after { float: right }` за текстом): пока
            // перед ним только строчное содержимое без разрывов, он встаёт
            // на ту же строку (правило 6 §9.5.1) — в этот же хост, со своим
            // щупом ширины набранного до него. Отдельный хост начинал бы
            // его с новой строки (`before-after-floated-001`).
            // Атомы в набранном — нет: щуп высоты их ряда (`FlowRow`) не
            // спускает строки под вырезы ранних флоатов, и второй флоат
            // вставал прямо под первым (`shape-outside-border-box-001-ref`:
            // 7.02 с атомами в щупе).
            let inline_only = rest.iter().all(|n| match n {
                Node::Text(_) => true,
                Node::Element(c) => {
                    (c.tag == "br" && c.style.clear.is_none())
                        || (!block_level_in_flow(c)
                            && !out_of_flow(&c.style)
                            && band_piece(n) != Some(BandPiece::Atom))
                }
            });
            if next.style.float.is_some_and(|f| f != 0)
                && inline_only
                && rest.iter().any(|n| !is_blank(n))
                && band_float_m(next, em).is_some()
            {
                let mut pre = lead.to_vec();
                pre.extend(rest.iter().cloned());
                probes.push((floaters.len(), floaters.len() + 1, pre));
                floaters.push(next.clone());
                j += 1;
                continue;
            }
            break;
        }
        rest.push(nodes[j].clone());
        j += 1;
    }
    band_host_m_tail(nodes, i, j, em, lead, floaters, rest, probes)
}

/// Годится ли флоат в измеряемый хост: `None` — хост отменяется.
pub(super) fn band_float_m(next: &Element, em: f32) -> Option<()> {
    {
        // Ортогональный флоат (своё письмо вертикально в горизонтальном
        // контейнере) С ТЕКСТОМ: строчный размер его строк (§7.3.1, от
        // начального содержащего блока) каркас пробы не считает, и ширина
        // (блочный размер) выходила по горизонтальной мере текста — на
        // прежний путь (`float-contiguous-vlr-011`: пять флоатов `abcde`).
        // Без текста блочный размер — сумма блочных размеров детей, и
        // проба считает его верно: такой флоат идёт в хост
        // (`float-shrink-to-fit-vrl-002…`, `contiguous-floated-table-v*`).
        if band_orthogonal(&next.style) && subtree_has_text(next) {
            return None;
        }
        // Флоат с трансформацией — на прежнем пути: трансформацию даёт
        // сборка узла `element`, а каркас флоата хоста её не несёт
        // (`transform-scale-test`). Буквица — своим исключением строки
        // (шаг F11, `Kind::Float { letter }`).
        if next.style.transform.is_some() {
            return None;
        }
        band_margins(&next.style, em)?;
    }
    Some(())
}

/// Хвост `band_host_m`: хост из собранных флоатов, хвоста и щупов.
#[allow(clippy::too_many_arguments)]
fn band_host_m_tail(
    nodes: &[Node],
    i: usize,
    j: usize,
    em: f32,
    lead: &[Node],
    floaters: Vec<Element>,
    rest: Vec<Node>,
    probes: Vec<(usize, usize, Vec<Node>)>,
) -> Option<(Element, usize, Vec<Node>)> {
    // Одинокий флоат без хвоста полосам не нужен — если перед ним в строке
    // ничего нет: флоат ПОСЛЕ текста («Inner<float>») встаёт на его строку
    // только в хосте.
    if floaters.len() < 2 && !rest.iter().any(|n| !is_blank(n)) && lead.is_empty() {
        return None;
    }
    // Хвост: куски своего контекста, блоки потока и строчные прогоны
    // (шаг F4: строки блоков потока режутся полосами, `band_flow::Kind::Flow`).
    let mut lifted: Vec<Node> = vec![];
    // Строчное содержимое перед флоатом в той же строке (`wrap_floats`):
    // оно — начало первого прогона хоста, флоаты встают на его строку.
    let lead_probe = (!lead.is_empty()).then_some(());
    let probe_nodes: Vec<Node> = probes
        .into_iter()
        .flat_map(|(a, b, pre)| {
            // Последний `<br>` верхнего уровня делит набранное: до него —
            // основание (`lead-base`), после — строка флоата (`lead-for`).
            let cut = pre
                .iter()
                .rposition(|n| matches!(n, Node::Element(c) if c.tag == "br"))
                .map(|p| p + 1);
            let (base, line) = match cut {
                Some(p) => (Some(pre[..p].to_vec()), pre[p..].to_vec()),
                None => (None, pre),
            };
            let probe = |children: Vec<Node>, key: &str| {
                Node::Element(Element {
                    list_item: None,
                    node_id: 0,
                    anim: None,
                    tag: "div".into(),
                    style: Computed::default(),
                    hover: None,
                    first_letter: None,
                    first_line: None,
                    children,
                    attrs: vec![
                        ("anon".into(), "1".into()),
                        ("lead-probe".into(), "1".into()),
                        (key.into(), format!("{a}-{b}")),
                    ],
                    inline: false,
                })
            };
            let mut v = vec![probe(line, "lead-for")];
            if let Some(base) = base {
                v.push(probe(base, "lead-base"));
            }
            v
        })
        .collect();
    let rest = if lead.is_empty() {
        rest
    } else {
        let mut r = lead.to_vec();
        r.extend(rest);
        r
    };
    let rest = band_flow_rest_lift(rest, em, Some(&mut lifted))?;
    // С содержимым перед флоатом первый кусок хвоста — его прогон: иначе
    // флоат не на той строке.
    if lead_probe.is_some()
        && !matches!(rest.first(), Some(Node::Element(c)) if c.attr("anon") == Some("1"))
    {
        return None;
    }
    // Блок потока или строчный прогон в хвосте (не кусок своего контекста).
    let flows = rest
        .iter()
        .any(|n| matches!(n, Node::Element(_)) && band_piece_m(n, em).is_none());
    // За хвостом — очищающая коробка с верхним полем: её поле и clearance
    // (§9.5.2) решаются в паре с высотой хвоста, а хост их не видит —
    // остаётся распорке флекс-ряда (`adjoining-float-nested-forced-clearance-003`).
    if flows
        && let Some(Node::Element(next)) = nodes[j..].iter().find(|n| !is_blank(n))
        && next.style.clear.is_some()
        && !band_clear_supported(next)
        && !zero_len(next.style.margin.top)
    {
        return None;
    }
    // Флоат с `shape-outside` рядом со СТРОКАМИ: строки обтекают ФОРМУ, а
    // полосы держат только прямоугольник margin-box (css-shapes-1 §1) —
    // такому пробегу место на живом пути форм `shape_flow`. Коробкам своего
    // контекста форма не важна: они обходят margin-box.
    if flows && host_floats_shaped(&nodes[i..j]) {
        return None;
    }
    let mut host = Element {
        list_item: None,
        node_id: 0,
        anim: None,
        tag: "shape-flow".into(),
        style: Computed::default(),
        hover: None,
        first_letter: None,
        first_line: None,
        children: Vec::new(),
        attrs: vec![
            ("count".into(), floaters.len().to_string()),
            ("em".into(), em.to_string()),
            // Высота содержащего блока (`BAND_CBH`): `shape-flow` своей нет.
            (
                "cbh".into(),
                BAND_CBH
                    .with(std::cell::Cell::get)
                    .map_or(String::new(), |v| v.to_string()),
            ),
            // И ширина (`BAND_CBW`) — блочный размер в вертикальном письме.
            (
                "cbw".into(),
                BAND_CBW
                    .with(std::cell::Cell::get)
                    .map_or(String::new(), |v| v.to_string()),
            ),
            // Метка измеряемого хоста: `shape_flow` отдаёт его `band_flow`.
            ("bands".into(), "m".into()),
        ],
        inline: false,
    };
    host.children = floaters.into_iter().map(Node::Element).collect();
    host.children.extend(rest);
    host.children.extend(probe_nodes);
    Some((host, j, lifted))
}

/// Есть ли у блока флоат среди потомков обычного потока (сквозь блоки, не
/// заводящие своего контекста).
fn has_flow_float(c: &Element) -> bool {
    c.children.iter().any(|n| match n {
        Node::Text(_) => false,
        Node::Element(k) => {
            k.style.float.is_some_and(|f| f != 0)
                || (block_level_in_flow(k) && !own_context(k) && has_flow_float(k))
        }
    })
}

/// Измеряемый хост, начатый блоком потока с флоатами внутри (шаг F7): сам
/// блок — `Kind::Nest`, за ним — хвост как у `band_host_m` до флоата или
/// `clear`. Без братьев за блоком хост не нужен: флоаты внутри влияют
/// только на его собственное содержимое, и его раскладывает он сам.
pub(super) fn band_host_nested(
    nodes: &[Node],
    i: usize,
    em: f32,
    parent_bfc: bool,
) -> Option<(Element, usize)> {
    let Node::Element(c) = &nodes[i] else {
        return None;
    };
    if !has_flow_float(c) || !band_nest_ok(c, em) {
        return None;
    }
    let mut j = i + 1;
    let mut rest: Vec<Node> = vec![];
    while j < nodes.len() {
        if let Node::Element(next) = &nodes[j]
            && (next.style.float.is_some_and(|f| f != 0)
                || (next.style.clear.is_some() && !band_clear_supported(next)))
        {
            break;
        }
        rest.push(nodes[j].clone());
        j += 1;
    }
    // Без братьев за блоком хост нужен только корню БФК: §10.6.7 требует
    // охватить флоаты высотой (Blink `block_layout_algorithm.cc:1309-1315`,
    // гейт `IsNewFormattingContext`; Servo `BlockFormattingContext::layout`,
    // `flow/mod.rs:460-465`), а блок, обнулённый §10.6.3, их не держит
    // (`letter-spacing-206`). Обычному блоку флоаты внутри влияют только на
    // его собственное содержимое — его раскладывает он сам.
    if !rest.iter().any(|n| !is_blank(n)) && !parent_bfc {
        return None;
    }
    let rest = band_flow_rest(rest, em)?;
    if let Some(Node::Element(next)) = nodes[j..].iter().find(|n| !is_blank(n))
        && next.style.clear.is_some()
        && !band_clear_supported(next)
        && !zero_len(next.style.margin.top)
    {
        return None;
    }
    let mut host = Element {
        list_item: None,
        node_id: 0,
        anim: None,
        tag: "shape-flow".into(),
        style: Computed::default(),
        hover: None,
        first_letter: None,
        first_line: None,
        children: Vec::new(),
        attrs: vec![
            ("count".into(), "0".into()),
            ("em".into(), em.to_string()),
            ("bands".into(), "m".into()),
        ],
        inline: false,
    };
    host.children = vec![Node::Element(c.clone())];
    host.children.extend(rest);
    Some((host, j))
}

/// Блок с флоатами или коробками своего контекста внутри годится в
/// `Kind::Nest` целиком — со всеми потомками (шаг F7).
pub(super) fn band_nest_ok(c: &Element, em: f32) -> bool {
    if !band_nest_block(c, em) {
        return false;
    }
    let Some(inner_em) = band_em(&c.style, em) else {
        return false;
    };
    let Some(seq) = band_seq(collapse_margins(&c.children, false), inner_em) else {
        return false;
    };
    seq.iter().all(|n| match n {
        Node::Text(_) => true,
        Node::Element(k) => {
            k.style.float.is_some_and(|f| f != 0)
                || band_piece_m(n, inner_em).is_some()
                || k.attr("anon") == Some("1")
                || band_flow_block(k, inner_em)
                || band_nest_ok(k, inner_em)
        }
    })
}

/// Есть ли в поддереве руби (`<ruby>`, `<rt>`).
pub(super) fn has_ruby(n: &Node) -> bool {
    match n {
        Node::Text(_) => false,
        Node::Element(e) => {
            matches!(e.tag.as_str(), "ruby" | "rt" | "rtc" | "rb") || e.children.iter().any(has_ruby)
        }
    }
}

/// Есть ли среди флоатов пробега (`run` — узлы от первого флоата до конца
/// хвоста) флоат с `shape-outside`.
fn host_floats_shaped(run: &[Node]) -> bool {
    run.iter().any(|n| {
        matches!(n, Node::Element(c)
            if c.style.float.is_some_and(|f| f != 0) && c.style.shape_outside.is_some())
    })
}
