//! Возможности разрыва, экспортируемые измеренными контекстами форматирования.
//!
//! Координаты — в логической блочной оси владельца. Параллельные потоки дают
//! возможности вместе; разрез не может пересекать ни одну измеренную строку или
//! монолитный бокс (CSS Break 3, разделы 4.1 и 4.3). Деревья layout остаются
//! временными.

pub(crate) mod blocks;
pub(crate) mod box_children;
mod contexts;
mod continuation;
pub(crate) mod coordinates;
pub(crate) mod float_resume;
pub(crate) mod inline;
mod line_growth;
mod measurements;
mod normalize;
pub(crate) use continuation::ContentLine;
pub(crate) use line_growth::LinePush;
mod stack;
pub(crate) use measurements::{FlowMeasure, FlowNode};
pub(crate) use stack::fragment_leaf;

/// Строки блочного контейнера дают разрывы класса B (css-break-3 §4.3).
/// Подписи и ячейки таблицы сохраняют собственные неделимые строки внутри
/// границ таблицы; flex/grid и multicol регистрируют свои независимые потоки.
/// Монолитные и внепоточные коробки не передают строки родительскому потоку.
pub(crate) fn flow_lines(e: &crate::dom::Element) -> bool {
    flow_lines_in(e, None)
}

/// Собственный OOF-контекст принимает свои строки; родительский поток — нет.
pub(crate) fn flow_lines_in(e: &crate::dom::Element, owner: Option<u64>) -> bool {
    use crate::style::computed::Display;
    matches!(
        e.style.used().display,
        None | Some(Display::Block | Display::ListItem | Display::Contents | Display::Table)
    ) && e.style.used().webkit_box != Some(true)
        && !matches!(
            e.tag.as_str(),
            "caption" | "thead" | "tbody" | "tfoot" | "tr" | "td" | "th"
        )
        && !crate::layout::multicol::spanner::multicol_container(&e.style.used())
        && !crate::layout::fragment::clone::monolith_box(e)
        && (owner == Some(e.node_id) || !crate::layout::box_kind::out_of_flow(&e.style.used()))
}

#[derive(Clone, Debug, Default)]
pub(crate) struct FlowGeometry {
    pub(crate) line_pushes: Vec<LinePush>,
    /// Строка у начала float переносит сам float, а не добавляет место внутри.
    pub(crate) float_leads: Vec<(u64, LinePush)>,
    pub(crate) boxes: std::collections::HashMap<u64, (f32, f32)>,
    pub(crate) float_contexts: Vec<float_resume::Snapshot>,
    pub(crate) inline: Vec<inline::Inline>,
    pub(crate) cuts: Vec<(f32, f32)>,
    pub(crate) solid: Vec<(f32, f32)>,
    /// Original float identities and boxes, never temporary LayoutIds.
    pub(crate) floats: Vec<FloatGeometry>,
}

/// Float margins remain unbreakable and are preserved at fragmentainer starts.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FloatGeometry {
    pub(crate) id: u64,
    /// Начало margin-box в BFC; перенос геометрии родителю его не меняет.
    pub(crate) anchor: f32,
    pub(crate) top: f32,
    pub(crate) bottom: f32,
    pub(crate) margins: [f32; 2],
    /// First legal internal break of a non-monolithic float, in owner coordinates.
    pub(crate) first_break: Option<f32>,
}

impl FloatGeometry {
    /// Монолит сохраняет весь margin box; разрезаемый float задаёт свой первый кусок.
    pub(crate) fn balance_min(&self) -> f32 {
        let end = self.first_break.unwrap_or(self.bottom + self.margins[1]);
        (end - self.top + self.margins[0]).max(0.0)
    }
}

impl FlowGeometry {
    pub(crate) fn merge(self, cuts: &mut Vec<(f32, f32)>, solid: &mut Vec<(f32, f32)>) {
        if self.cuts.is_empty() {
            return;
        }
        let mut merged = self;
        merged.cuts.append(cuts);
        merged.solid.append(solid);
        merged.normalize();
        *cuts = merged.cuts;
        *solid = merged.solid;
    }

    pub(crate) fn append(&mut self, other: &Self, offset: f32) {
        self.float_leads
            .extend(other.float_leads.iter().map(|(id, p)| {
                (
                    *id,
                    LinePush {
                        at: p.at + offset,
                        ..p.clone()
                    },
                )
            }));
        self.boxes.extend(
            other
                .boxes
                .iter()
                .map(|(&id, &(a, b))| (id, (a + offset, b + offset))),
        );
        self.line_pushes
            .extend(other.line_pushes.iter().map(|p| LinePush {
                at: p.at + offset,
                ..p.clone()
            }));
        self.float_contexts
            .extend(other.float_contexts.iter().map(|g| g.translated(offset)));
        self.inline
            .extend(other.inline.iter().map(|flow| flow.translated(offset)));
        self.floats
            .extend(other.floats.iter().map(|g| FloatGeometry {
                top: g.top + offset,
                bottom: g.bottom + offset,
                first_break: g.first_break.map(|b| b + offset),
                ..*g
            }));
        self.cuts
            .extend(other.cuts.iter().map(|&(a, b)| (a + offset, b + offset)));
        self.solid
            .extend(other.solid.iter().map(|&(a, b)| (a + offset, b + offset)));
    }

    /// Keep the first `orphans` and the last `widows` line boxes of one inline
    /// formatting context together (css-break-3 §4.4; Blink
    /// `kBreakAppealViolatingOrphansAndWidows` in `block_layout_algorithm.cc`):
    /// the break opportunities between them are removed and their lines form
    /// one unbreakable range. `self` holds the lines' solids in order, one per
    /// line (`lines`); a break before the first or after the last line stays.
    pub(crate) fn keep_lines(&mut self, orphans: usize, widows: usize) {
        use crate::tolerance::COORD_EPS;
        let n = self.solid.len();
        if n < 2 {
            return;
        }
        // Break after line `k` (1-based) is legal when `k` lines stay before
        // it and `n - k` after it.
        let legal = |k: usize| k == 0 || k >= n || (k >= orphans.max(1) && n - k >= widows.max(1));
        let lines = std::mem::take(&mut self.solid);
        let mut groups: Vec<(f32, f32)> = Vec::new();
        for (i, &(a, b)) in lines.iter().enumerate() {
            match groups.last_mut() {
                Some(g) if !legal(i) => g.1 = b,
                _ => groups.push((a, b)),
            }
        }
        // A cut inside a group (between its lines, or in clearance before one
        // of them) breaks between kept lines.
        self.cuts.retain(|&(a, _)| {
            !groups
                .iter()
                .any(|&(s, e)| s + COORD_EPS < a && a < e - COORD_EPS)
        });
        self.solid = groups;
    }

    pub(crate) fn lines(heights: impl IntoIterator<Item = f32>) -> Self {
        let mut geometry = Self::default();
        let mut y = 0.0;
        for height in heights {
            let end = y + height;
            geometry.solid.push((y, end));
            geometry.cuts.push((end, end));
            y = end;
        }
        geometry
    }
}

#[cfg(test)]
mod tests;

mod logical;
pub(crate) use logical::transpose_tree;
mod independent;
pub(crate) use independent::{
    PagePercentages, Parallel, column_percentages, column_percentages_in, flow_end,
    independent_copy, logical_copy, page_copy, page_percentages, regrow_copy, vertical_copy,
};
mod copy_clearance;
pub(crate) use copy_clearance::copy_flow;
pub(crate) use copy_clearance::reaches as exports_floats;
mod empty_floats;
pub(crate) use empty_floats::empty_floats;
