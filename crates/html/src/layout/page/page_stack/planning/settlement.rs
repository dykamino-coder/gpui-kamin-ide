//! Фиксированная точка измеренного плана; повтор геометрии — явный incomplete.
use super::super::PageGeom;
use super::super::continuation::CopyGeometry;
use crate::layout::fragment::types::Frag;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

#[derive(Debug, PartialEq)]
pub(crate) struct Incomplete;

// Ключ описывает наблюдаемый результат раскладки, не накопленные распорки.
// Повтор при всё ещё требуемой коррекции означает отсутствие прогресса.
// Число копий ограничено Space, узлов — деревом, координаты — битами f32:
// множество ключей конечно. Эвристического предела проходов здесь нет.
#[derive(Hash, PartialEq, Eq)]
pub(super) struct State {
    names: Vec<String>,
    areas: Vec<[u32; 2]>,
    fragments: Vec<(usize, usize, usize, [u32; 5])>,
    copies: Vec<CopyState>,
}

#[derive(Hash, PartialEq, Eq)]
struct CopyState {
    index: (usize, usize),
    area: [u32; 2],
    boxes: Vec<(u64, [u32; 2])>,
}

impl State {
    pub(super) fn new(
        names: &[String],
        geoms: &[PageGeom],
        plan: &[Frag],
        measured: &HashMap<(usize, usize), CopyGeometry>,
    ) -> Self {
        let mut copies: Vec<_> = measured
            .iter()
            .map(|(&index, m)| {
                let mut boxes: Vec<_> = m
                    .laid
                    .boxes
                    .iter()
                    .map(|(&id, &(a, b))| (id, [a.to_bits(), b.to_bits()]))
                    .collect();
                boxes.sort_unstable_by_key(|b| b.0);
                CopyState {
                    index,
                    area: [
                        m.inputs.constraints.area.0.to_bits(),
                        m.inputs.constraints.area.1.to_bits(),
                    ],
                    boxes,
                }
            })
            .collect();
        copies.sort_unstable_by_key(|c| c.index);
        Self {
            names: names.to_vec(),
            areas: geoms
                .iter()
                .map(|g| [g.area.0.to_bits(), g.area.1.to_bits()])
                .collect(),
            fragments: plan
                .iter()
                .map(|f| {
                    (
                        f.kid,
                        f.copy,
                        f.col,
                        [f.y, f.from, f.h, f.head, f.foot].map(f32::to_bits),
                    )
                })
                .collect(),
            copies,
        }
    }
}

pub(super) fn settle<S: Eq + Hash, T>(
    mut step: impl FnMut() -> (bool, S, T),
) -> Result<T, Incomplete> {
    let mut seen = HashSet::new();
    loop {
        let (stable, state, value) = step();
        if stable {
            return Ok(value);
        }
        if !seen.insert(state) {
            return Err(Incomplete);
        }
    }
}
