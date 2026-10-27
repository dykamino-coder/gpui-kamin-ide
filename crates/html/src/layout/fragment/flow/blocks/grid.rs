//! Сетка в блочной стопке копии: элементы сетки и их свойства, которые задаёт стиль
//! (принудительные разрывы и запреты на краях с переносом с крайних потомков, монолитность с
//! блокификацией, поля), и их собственные стопки. Дорожки рядов, положения элементов и их ряды —
//! из РАЗЛОЖЕННОЙ сетки (taffy `DetailedLayoutInfo::Grid` через рекордер: `Laid::rows`,
//! `Laid::item_rows`), а не из оценки по стилю: область элемента задают линии его финального
//! размещения (css-grid-2 §8), а не его поле-бокс. Ряды подсетки по блочной оси — дорожки
//! родителя (css-grid-2 §9): их перенос принадлежит родительской сетке.

use super::{BlockBox, Laid, collect};
use crate::document::state::ValueCx;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::monolith_blockified;
use crate::layout::fragment::flow::box_children::children;
use crate::style::computed::{Display, LenCx};
use crate::tolerance::COORD_EPS;

/// Поточные элементы сетки в порядке документа.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GridBox {
    pub(crate) items: Vec<GridItem>,
    /// Сетка — подсетка родителя по блочной оси с тем же направлением потока: её ряды —
    /// дорожки родителя в том же порядке.
    pub(crate) inherits_rows: bool,
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
    /// Своя блочная стопка или сетка элемента.
    pub(crate) inner: Option<BlockBox>,
}

impl GridItem {
    /// Сетка элемента, если он — подсетка родителя по блочной оси.
    fn subgrid(&self) -> Option<&GridBox> {
        self.inner
            .as_ref()?
            .grid
            .as_ref()
            .filter(|g| g.inherits_rows)
    }
}

impl BlockBox {
    /// Рост подсеток по рядам из роста их родителей, сверху вниз по стопке.
    pub(crate) fn subgrid_growth(&self, laid: &mut Laid) {
        if let Some(grid) = &self.grid
            && let Some(growth) = laid.row_growth.get(&self.id).cloned()
        {
            grid.subgrid_growth(&growth, laid);
        }
        let grid_items = self.grid.iter().flat_map(|g| &g.items).map(|i| &i.inner);
        let kids = self
            .kids
            .iter()
            .chain(self.flex.iter().flat_map(|f| &f.items))
            .map(|k| &k.inner);
        for inner in grid_items.chain(kids).flatten() {
            inner.subgrid_growth(laid);
        }
        for flow in self.table.iter().flat_map(|t| t.flows()) {
            flow.subgrid_growth(laid);
        }
    }
}

/// Контейнер сетки, чьи ряды дают точки разрыва его элементам.
pub(crate) fn grid_container(e: &Element) -> bool {
    matches!(
        e.style.used().display,
        Some(Display::Grid) | Some(Display::InlineGrid)
    )
}

/// Все элементы сетки `e`, записываемые native рекордером.
pub(super) fn collect_grid(e: &Element, cx: ShapeCx, vcx: ValueCx<'_>) -> GridBox {
    let mut out = GridBox::default();
    for k in children(&e.children).filter(|n| !is_blank(n)) {
        let Node::Element(k) = k else { continue };
        if out_of_flow(&k.style.used()) || matches!(k.style.used().display, Some(Display::None)) {
            continue;
        }
        let lens = LenCx::box_viewport(cx.viewport, &k.style.used());
        let px = |l| lens.px_lossy_zero(l, vcx);
        let monolith = monolith_blockified(k);
        let inner = (!monolith).then(|| {
            let mut inner = collect(k, true, cx, vcx);
            if let Some(g) = inner.grid.as_mut() {
                g.inherits_rows = inherits_rows(e, k);
            }
            inner
        });
        out.items.push(GridItem {
            id: k.node_id,
            mt: px(k.style.used().margin.top),
            mb: px(k.style.used().margin.bottom),
            force_before: edge_break(k, false, vcx),
            force_after: edge_break(k, true, vcx),
            avoid_before: edge_avoid(k, false, vcx),
            avoid_after: edge_avoid(k, true, vcx),
            monolith,
            inner,
        });
    }
    out
}

/// Ряды элемента-сетки `k` — ряды сетки `grid` (`grid-template-rows: subgrid`) при параллельном
/// письме с тем же направлением блочной оси (css-grid-2 §9).
fn inherits_rows(grid: &Element, k: &Element) -> bool {
    let (parent, child) = (grid.style.used(), k.style.used());
    grid_container(k)
        && child.subgrid_rows
        && parent.wm().vertical() == child.wm().vertical()
        && parent.wm().block_rl() == child.wm().block_rl()
}

/// Перенос ряда сетки в следующий фрагментаинер (css-grid-2 §12: ряд, начатый в следующем
/// фрагментаинере, сдвигается вместе со всеми последующими; Blink
/// `ShiftBreakpointIntoNextFragmentainer`): решатель вставляет место после дорожки `track`
/// (индекс экспортированных дорожек `Laid::rows`), размеры дорожек и элементов прежних рядов
/// не меняются, охватывающие элементы растут со своей областью.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RowPush {
    pub(crate) grid: u64,
    pub(crate) track: usize,
}

/// Куда встаёт распорка роста в разложенной стопке.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Pushed {
    /// Независимое смещение float, не меняющее нормальную высоту родителя.
    Float(u64),
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
        if let Some(rows) = laid.rows(grid).filter(|_| !self.inherits_rows) {
            for i in 1..rows.len() {
                // Разрыв в зазоре рядов — не перенос ряда: зазор у разрыва
                // исчезает (css-gaps-1 §fragmentation; Blink
                // `MaybeSuppressLastGap`), смещения рядов не растут.
                let gap = rows[i - 1].1 + laid.growth_after(grid, i - 1);
                let at_row = (rows[i].0 - a).abs() < COORD_EPS && rows[i].0 <= gap + COORD_EPS;
                if !at_row {
                    continue;
                }
                // Ряд переносится, только если в предыдущем ряду кончается элемент:
                // иначе точка лежит внутри охватывающих элементов.
                if !self.ends_in(i - 1, 0, laid) {
                    return None;
                }
                return Some(Pushed::Row(RowPush { grid, track: i - 1 }));
            }
        }
        for it in &self.items {
            let Some((t, b)) = laid.at(it.id) else {
                continue;
            };
            if t < a - COORD_EPS && a < b - COORD_EPS {
                return it.inner.as_ref()?.pushed_at(a, laid);
            }
        }
        None
    }

    /// Кончается ли в ряду `row` сетки-владельца дорожек элемент этой сетки, чьи ряды
    /// начинаются с дорожки `offset` владельца (элементы подсеток — по её дорожкам).
    fn ends_in(&self, row: usize, offset: usize, laid: &Laid) -> bool {
        self.items.iter().any(|it| {
            let Some((first, last)) = laid.item_rows(it.id) else {
                return false;
            };
            match it.subgrid() {
                Some(sub) => sub.ends_in(row, offset + first, laid),
                None => offset + last == row,
            }
        })
    }

    /// Рост родителя `growth` внутри охвата подсеток по рядам — их собственный рост
    /// (решатель передаёт его подсетке так же, `fragmentation_growth`).
    fn subgrid_growth(&self, growth: &[f32], laid: &mut Laid) {
        for it in self.items.iter().filter(|it| it.subgrid().is_some()) {
            let Some((first, last)) = laid.item_rows(it.id) else {
                continue;
            };
            let inner: Vec<f32> = (first..last)
                .map(|track| growth.get(track).copied().unwrap_or(0.0))
                .collect();
            if inner.iter().any(|&g| g > 0.0) {
                laid.row_growth.insert(it.id, inner);
            }
        }
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
