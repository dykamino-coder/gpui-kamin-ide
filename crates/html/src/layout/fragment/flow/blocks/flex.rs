//! Flex-элементы и реальные линии решателя: стиль задаёт запреты, Taffy — членство и геометрию.
use super::{BlockBox, BlockKid, Laid, Pushed, collect};
use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::content_monolith_blockified;
use crate::style::computed::Display;
use crate::tolerance::COORD_EPS;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct FlexBox {
    pub(crate) items: Vec<BlockKid>,
    /// Промежуток главной оси колонки (`row-gap`), px.
    pub(crate) main_gap: f32,
}

/// Линии финальной раскладки; координаты физические, до relative-сдвигов.
#[derive(Clone, Debug, Default)]
pub(crate) struct FlexFrame {
    pub(crate) column: bool,
    pub(crate) lines: Vec<FlexLine>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct FlexLine {
    pub(crate) index: usize,
    pub(crate) start: f32,
    pub(crate) end: f32,
    pub(crate) items: Vec<u64>,
}

/// Рост перед линией: её предшественник сохраняет занятый фрагмент.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FlexPush {
    pub(crate) container: u64,
    pub(crate) line: usize,
}

pub(crate) fn flex_container(e: &Element) -> bool {
    matches!(e.style.display, Some(Display::Flex | Display::InlineFlex))
}

pub(super) fn collect_flex(e: &Element, depth: u8, cx: ShapeCx, vcx: ValueCx<'_>) -> FlexBox {
    let column = matches!(
        e.style.flex_dir,
        Some(crate::style::computed::FlexDir::Col | crate::style::computed::FlexDir::ColReverse)
    );
    let main_gap = match e.style.gap {
        Some((Some(crate::style::values::value::Len::Px(v)), _)) if column => v.max(0.0),
        _ => 0.0,
    };
    FlexBox {
        main_gap,
        items: e
            .children
            .iter()
            .filter_map(|n| {
                let Node::Element(k) = n else { return None };
                if out_of_flow(&k.style) || k.style.display == Some(Display::None) {
                    return None;
                }
                let monolith = content_monolith_blockified(k);
                Some(BlockKid {
                    id: k.node_id,
                    force_before: edge_break(k, false, vcx),
                    force_after: edge_break(k, true, vcx),
                    avoid_before: edge_avoid(k, false, vcx),
                    avoid_after: edge_avoid(k, true, vcx),
                    monolith,
                    inner: (!monolith).then(|| collect(k, depth, true, cx, vcx)),
                    ..Default::default()
                })
            })
            .collect(),
    }
}

impl FlexBox {
    pub(super) fn pushed_at(&self, id: u64, a: f32, laid: &Laid) -> Option<Pushed> {
        let frame = laid.flexes.get(&id)?;
        if !frame.column {
            for pair in frame.lines.windows(2) {
                // Разрыв перед линией режет и усечённые поля её элементов
                // (`FlexBox::breaks`): точка — начало линии или её содержимого.
                let top = super::flex_breaks::line_top(&pair[1], laid);
                if (pair[1].start - a).abs() < COORD_EPS || (top - a).abs() < COORD_EPS {
                    return Some(Pushed::Flex(FlexPush {
                        container: id,
                        line: pair[0].index,
                    }));
                }
            }
        }
        for item in &self.items {
            let Some((top, bottom)) = laid.at(item.id) else {
                continue;
            };
            // Элемент колонки сдвигается вместе со своим полем: граница
            // разрыва перед ним — начало его margin box.
            let start = laid.margin_box(item.id).map_or(top, |m| m.0);
            if frame.column && ((top - a).abs() < COORD_EPS || (start - a).abs() < COORD_EPS) {
                return Some(Pushed::Box(item.id));
            }
            if top <= a
                && a < bottom - COORD_EPS
                && let Some(found) = item.inner.as_ref().and_then(|b| b.pushed_at(a, laid))
            {
                return Some(found);
            }
        }
        None
    }
}

impl BlockBox {
    /// Есть ли в стопке flex-контейнер.
    pub(crate) fn has_flex(&self) -> bool {
        let mut out = Vec::new();
        self.flexes(&mut out);
        !out.is_empty()
    }

    /// Все flex-контейнеры, в том числе вложенные в элементы grid/flex.
    pub(crate) fn flexes(&self, out: &mut Vec<(u64, Vec<u64>)>) {
        if let Some(f) = &self.flex {
            out.push((self.id, f.items.iter().map(|i| i.id).collect()));
        }
        for inner in self
            .kids
            .iter()
            .chain(self.flex.iter().flat_map(|f| &f.items))
            .filter_map(|i| i.inner.as_ref())
            .chain(
                self.grid
                    .iter()
                    .flat_map(|g| &g.items)
                    .filter_map(|i| i.inner.as_ref()),
            )
        {
            inner.flexes(out);
        }
    }
}

impl BlockBox {
    /// Принудительные границы нулевых строк нельзя объединять по координате:
    /// каждая такая граница начинает отдельный фрагментаинер (css-break-3 §4.3).
    pub(crate) fn zero_forced_flex_lines(&self, laid: &Laid, out: &mut Vec<(FlexPush, f32)>) {
        if let Some(flex) = &self.flex
            && let Some(frame) = laid.flexes.get(&self.id)
            && !frame.column
        {
            for pair in frame.lines.windows(2) {
                let (previous, next) = (&pair[0], &pair[1]);
                if (previous.end - previous.start).abs() > COORD_EPS
                    || (next.start - previous.start).abs() > COORD_EPS
                {
                    continue;
                }
                let forced = flex.items.iter().any(|item| {
                    (previous.items.contains(&item.id) && item.edge(true, laid).0)
                        || (next.items.contains(&item.id) && item.edge(false, laid).0)
                });
                if forced {
                    out.push((
                        FlexPush {
                            container: self.id,
                            line: previous.index,
                        },
                        next.start,
                    ));
                }
            }
        }
        for inner in self
            .kids
            .iter()
            .chain(self.flex.iter().flat_map(|f| &f.items))
            .filter_map(|item| item.inner.as_ref())
            .chain(
                self.grid
                    .iter()
                    .flat_map(|g| &g.items)
                    .filter_map(|item| item.inner.as_ref()),
            )
        {
            inner.zero_forced_flex_lines(laid, out);
        }
    }
}
