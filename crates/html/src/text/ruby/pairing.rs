//! Спаривание баз и аннотаций сегмента по колонкам (css-ruby-1 §2.3.2): пробельные единицы
//! внутри уровня спариваются отдельно от обычных — в каждом уровне на тех же границах стоят
//! пробелы или пустые анонимные единицы, нехватка обычных дополняется пустыми.

use super::{RubySegment, RubyUnit};
use crate::dom::Node;
use crate::dom::whitespace::blank_text;
use std::collections::{BTreeMap, BTreeSet};

fn gap(unit: &RubyUnit) -> bool {
    !unit.is_empty()
        && unit
            .iter()
            .all(|n| matches!(n, Node::Text(t) if blank_text(t)))
}

fn separate(units: Vec<RubyUnit>) -> (Vec<RubyUnit>, BTreeMap<usize, RubyUnit>) {
    let mut boxes = Vec::new();
    let mut gaps = BTreeMap::new();
    for unit in units {
        if gap(&unit) {
            gaps.entry(boxes.len())
                .or_insert_with(Vec::new)
                .extend(unit);
        } else {
            boxes.push(unit);
        }
    }
    (boxes, gaps)
}

pub(super) fn pair(segment: &mut RubySegment) {
    let (bases, base_gaps) = separate(std::mem::take(&mut segment.bases));
    let mut boundaries: BTreeSet<usize> = base_gaps.keys().copied().collect();
    let mut levels = Vec::new();
    let mut count = bases.len();
    for level in &mut segment.levels {
        if level.spanning {
            levels.push(None);
        } else {
            let (boxes, gaps) = separate(std::mem::take(&mut level.units));
            boundaries.extend(gaps.keys().copied());
            count = count.max(boxes.len());
            levels.push(Some((boxes, gaps)));
        }
    }
    let expand = |boxes: Vec<RubyUnit>, mut gaps: BTreeMap<usize, RubyUnit>| {
        let mut result = Vec::new();
        let mut boxes = boxes.into_iter();
        for i in 0..=count {
            if boundaries.contains(&i) {
                result.push(gaps.remove(&i).unwrap_or_default());
            }
            if i < count {
                result.push(boxes.next().unwrap_or_default());
            }
        }
        result
    };
    segment.bases = expand(bases, base_gaps);
    for (level, units) in segment.levels.iter_mut().zip(levels) {
        if let Some((boxes, gaps)) = units {
            level.units = expand(boxes, gaps);
        }
    }
}
