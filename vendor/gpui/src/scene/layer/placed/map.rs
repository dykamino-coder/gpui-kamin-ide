//! Maintain the per-command placement map.

use super::{MANY, NONE, Placed, PlacedMap, PlacedOp};

impl PlacedMap {
    /// A map for a replay (`at`) or for `len` commands of a draft (`None`).
    pub(crate) fn new(at: Option<Placed>, len: usize) -> Self {
        let none = PlacedOp {
            op: NONE,
            quad: NONE,
            placed: Placed::default(),
        };
        let ops = if at.is_some() {
            Vec::new()
        } else {
            vec![none; len]
        };
        Self { at, ops }
    }

    /// The next source command of a replay landed nowhere.
    pub(crate) fn skip(map: &mut Option<Self>) {
        if let Some(map) = map {
            let placed = map.at.unwrap_or_default();
            map.ops.push(PlacedOp {
                op: NONE,
                quad: NONE,
                placed,
            });
        }
    }

    /// The next source command of a replay landed as command `op`.
    pub(crate) fn land(map: &mut Option<Self>, op: usize, quad: Option<usize>) {
        if let Some(map) = map {
            let placed = map.at.unwrap_or_default();
            let quad = quad.map_or(NONE, |q| q as u32);
            map.ops.push(PlacedOp {
                op: op as u32,
                quad,
                placed,
            });
        }
    }

    /// Source command `source` of a draft landed as command `op`.
    pub(crate) fn set(
        map: &mut Option<Self>,
        source: usize,
        op: usize,
        quad: Option<usize>,
        placed: Placed,
    ) {
        let Some(entry) = map.as_mut().and_then(|m| m.ops.get_mut(source)) else {
            return;
        };
        *entry = if entry.op == NONE {
            PlacedOp {
                op: op as u32,
                quad: quad.map_or(NONE, |q| q as u32),
                placed,
            }
        } else {
            PlacedOp {
                op: MANY,
                quad: MANY,
                placed,
            }
        };
    }
}
