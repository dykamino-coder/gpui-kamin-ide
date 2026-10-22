//! Flex-элементы и реальные линии решателя: стиль задаёт запреты, Taffy — членство и геометрию.
use super::{BlockBox, BlockKid, Laid, Pushed, collect};
use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::content_monolith_blockified;
use crate::layout::fragment::flow::box_children::children;
use crate::style::computed::Display;
use crate::tolerance::COORD_EPS;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct FlexBox {
    pub(crate) items: Vec<BlockKid>,
}

/// Линии финальной раскладки; координаты вдоль блочной оси копии, до relative-сдвигов.
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

pub(super) fn collect_flex(e: &Element, cx: ShapeCx, vcx: ValueCx<'_>) -> FlexBox {
    FlexBox {
        items: children(&e.children)
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
                    inner: (!monolith).then(|| collect(k, true, cx, vcx)),
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
                if (pair[1].start - a).abs() < COORD_EPS {
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
            if frame.column && (top - a).abs() < COORD_EPS {
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
            .chain(self.table.iter().flat_map(|t| t.flows()))
        {
            inner.flexes(out);
        }
    }
}
