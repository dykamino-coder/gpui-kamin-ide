//! Выпуск геометрии среза из неизменной позиции ребёнка в колонке.

use crate::layout::fragment::types::Frag;

pub(super) struct FragmentAt {
    kid: usize,
    copy: usize,
    col: usize,
    y: f32,
    from: f32,
    head: f32,
}

impl FragmentAt {
    pub(super) fn new(kid: usize, copy: usize, col: usize, y: f32, from: f32, head: f32) -> Self {
        Self {
            kid,
            copy,
            col,
            y,
            from,
            head,
        }
    }

    pub(super) fn emit(&self, out: &mut Vec<Frag>, h: f32, foot: f32) {
        out.push(Frag {
            kid: self.kid,
            copy: self.copy,
            col: self.col,
            y: self.y,
            from: self.from,
            h,
            head: self.head,
            foot,
        });
    }
}
