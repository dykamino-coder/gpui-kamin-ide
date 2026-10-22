//! Сетка в блочной стопке копии: элементы сетки и их свойства, которые задаёт стиль
//! (принудительные разрывы и запреты на краях с переносом с крайних потомков, монолитность с
//! блокификацией, поля), и их собственные стопки. Дорожки рядов и положения элементов — из
//! РАЗЛОЖЕННОЙ сетки (taffy `DetailedLayoutInfo::Grid` через рекордер, `Laid::rows`), а не из
//! оценки по стилю. Ряды элемента выводятся из его разложенного поля-бокса: область элемента
//! (css-grid-1 §8) накрывает его поле-бокс, начало области — начало ряда.

use super::{BlockBox, Laid, collect};
use crate::document::state::ValueCx;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::monolith_blockified;
use crate::style::computed::{Display, LenCx};
use crate::tolerance::COORD_EPS;

/// Поточные элементы сетки в порядке документа.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GridBox {
    pub(crate) items: Vec<GridItem>,
}

/// Элемент сетки — параллельный поток своего ряда (css-break-3 §3; Blink
/// `PlaceGridItemsForFragmentation`: у каждого элемента свой break token).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GridItem {
    pub(crate) id: u64,
    /// Верхнее и нижнее поля в точках: поле-бокс лежит в области элемента.
    pub(crate) mt: f32,
    pub(crate) mb: f32,
    /// `break-before`/`break-after` элемента (с переносом с крайних
    /// потомков) переходят на границы его рядов (css-grid-1 §12).
    pub(crate) force_before: bool,
    pub(crate) force_after: bool,
    pub(crate) avoid_before: bool,
    pub(crate) avoid_after: bool,
    /// Монолит (css-break-4 §4.1) после блокификации элемента.
    pub(crate) monolith: bool,
    /// Охват рядов по размещению (css-grid-1 §8.3.1): явные линии, `span`,
    /// именованная область; авто-размещение охватывает один ряд. `None` —
    /// охват по стилю неизвестен (отрицательные линии).
    pub(crate) span: Option<usize>,
    /// Своя блочная стопка или сетка элемента.
    pub(crate) inner: Option<BlockBox>,
}

/// Ряды разложенного элемента: `(первый, последний)` по дорожкам `rows`
/// (координаты потока). Начало — последний ряд, начатый не ниже верха
/// поля-бокса (область накрывает поле-бокс); конец — по охвату `span`, а
/// без него — первый ряд, кончающийся не выше низа поля-бокса.
pub(crate) fn item_rows(
    rows: &[(f32, f32)],
    top: f32,
    bottom: f32,
    span: Option<usize>,
) -> (usize, usize) {
    let start = rows
        .iter()
        .rposition(|r| r.0 <= top + COORD_EPS)
        .unwrap_or(0);
    if let Some(n) = span {
        return (
            start,
            (start + n.max(1) - 1)
                .min(rows.len().saturating_sub(1))
                .max(start),
        );
    }
    let end = rows
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, r)| r.1 >= bottom - COORD_EPS)
        .map_or(rows.len().saturating_sub(1), |(i, _)| i);
    (start, end.max(start))
}

/// Контейнер сетки, чьи ряды дают точки разрыва его элементам.
pub(crate) fn grid_container(e: &Element) -> bool {
    matches!(
        e.style.computed().display,
        Some(Display::Grid) | Some(Display::InlineGrid)
    )
}

/// Элементы сетки `e` до глубины `depth`.
pub(super) fn collect_grid(e: &Element, depth: u8, cx: ShapeCx, vcx: ValueCx<'_>) -> GridBox {
    let mut out = GridBox::default();
    for k in e.children.iter().filter(|n| !is_blank(n)) {
        let Node::Element(k) = k else { continue };
        if out_of_flow(&k.style.computed())
            || matches!(k.style.computed().display, Some(Display::None))
        {
            continue;
        }
        let lens = LenCx::box_viewport(cx.viewport, &k.style.computed());
        let px = |l| lens.px_lossy_zero(l, vcx);
        let monolith = monolith_blockified(k);
        out.items.push(GridItem {
            id: k.node_id,
            mt: px(k.style.computed().margin.top),
            mb: px(k.style.computed().margin.bottom),
            force_before: edge_break(k, false, vcx),
            force_after: edge_break(k, true, vcx),
            avoid_before: edge_avoid(k, false, vcx),
            avoid_after: edge_avoid(k, true, vcx),
            monolith,
            span: row_span(e, k),
            inner: (!monolith).then(|| collect(k, depth, true, cx, vcx)),
        });
    }
    out
}

/// Охват рядов элемента `k` сетки `grid` по размещению (css-grid-1 §8.3.1).
fn row_span(grid: &Element, k: &Element) -> Option<usize> {
    use crate::style::computed::Placement::{Auto, Line, Span};
    if let Some(name) = &k.style.computed().grid_area_name {
        let rows: Vec<usize> = grid
            .style
            .computed()
            .grid_areas
            .as_ref()?
            .iter()
            .enumerate()
            .filter(|(_, r)| r.iter().any(|x| x == name))
            .map(|(i, _)| i)
            .collect();
        return Some(rows.last()? - rows.first()? + 1);
    }
    match k.style.computed().grid_row.unwrap_or((Auto, Auto)) {
        (Line(a), Line(b)) if a == b => Some(1),
        (Line(a), Line(b)) if a != b && (a > 0) == (b > 0) => Some(a.abs_diff(b) as usize),
        (Line(_), Line(_)) => None,
        (Span(m), _) | (_, Span(m)) => Some(m.max(1) as usize),
        _ => Some(1),
    }
}

/// Перенос ряда сетки в следующий фрагментаинер (css-grid-1 §12.1: ряд,
/// начатый в следующем фрагментаинере, сдвигается вместе со всеми
/// последующими; Blink `ShiftBreakpointIntoNextFragmentainer`): растёт
/// граница ПЕРЕД рядом `row`. Элементы, кончающиеся в предыдущем ряду
/// (`items`), получают нижнее поле на величину роста — дорожка `auto`
/// растёт вместе с их вкладом, а растянутые элементы сохраняют размер
/// (Blink не растягивает элементы предыдущего ряда: сдвигаются смещения
/// рядов). Явная дорожка в точках растёт сама (`track`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RowPush {
    pub(crate) grid: u64,
    pub(crate) items: Vec<u64>,
    pub(crate) track: usize,
}

/// Куда встаёт распорка роста в разложенной стопке.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Pushed {
    /// Строка абзаца в независимом потоке flex-item.
    Line(super::super::LinePush),
    /// Коробка, начинающаяся в точке (распорка перед ней).
    Box(u64),
    /// Ряд сетки без зазора перед ним, начинающийся в точке.
    Row(RowPush),
    Flex(super::FlexPush),
}

impl GridBox {
    /// Распорка роста в точке `a` (координаты потока) сетки `grid`: начало
    /// ряда `i ≥ 1` без зазора перед ним — перенос ряда; точка внутри
    /// элемента — спуск в его стопку.
    pub(super) fn pushed_at(&self, grid: u64, a: f32, laid: &Laid) -> Option<Pushed> {
        let rows = laid.rows(grid);
        let placed: Vec<(&GridItem, f32, f32, usize, usize)> = self
            .items
            .iter()
            .filter_map(|it| {
                let (t, b) = laid.at(it.id)?;
                let (r0, r1) = rows
                    .as_deref()
                    .map_or((0, 0), |r| item_rows(r, t - it.mt, b + it.mb, it.span));
                Some((it, t, b, r0, r1))
            })
            .collect();
        if let Some(rows) = rows.as_deref() {
            for i in 1..rows.len() {
                // Разрыв в зазоре рядов — не перенос ряда: зазор у разрыва
                // исчезает (css-gaps-1 §fragmentation; Blink
                // `MaybeSuppressLastGap`), смещения рядов не растут.
                let at_row =
                    (rows[i].0 - a).abs() < COORD_EPS && rows[i].0 <= rows[i - 1].1 + COORD_EPS;
                if !at_row {
                    continue;
                }
                let items: Vec<u64> = placed
                    .iter()
                    .filter(|p| p.4 == i - 1)
                    .map(|p| p.0.id)
                    .collect();
                if items.is_empty() {
                    return None;
                }
                return Some(Pushed::Row(RowPush {
                    grid,
                    items,
                    track: i - 1,
                }));
            }
        }
        for (it, t, b, _, _) in &placed {
            if *t < a - COORD_EPS && a < b - COORD_EPS {
                return it.inner.as_ref()?.pushed_at(a, laid);
            }
        }
        None
    }

    pub(super) fn push_ids(&self, out: &mut Vec<u64>) {
        for it in &self.items {
            out.push(it.id);
            if let Some(inner) = &it.inner {
                inner.push_ids(out);
            }
        }
    }
}
