//! KaminIDE patch: transform pivots (`TransformPivot`) through the copies of a
//! finished scene.
//!
//! A paint record paints its transformed boxes once; a frame copies it where
//! it lands, maybe moved, under the node open there (`mark_copied`). The
//! copy's primitives keep the pivots of the record's commands: a moved copy
//! builds their matrices from the moved pivots (the bits a fresh paint there
//! builds), and its marks keep them, so a scrolled replay moves them again.

use super::*;
use std::rc::Rc;

/// The pivots of a scene being copied, by its command, and where it lands.
pub(crate) struct Copying {
    pivots: Vec<(u32, Rc<TransformPivot>)>,
    offset: Point<ScaledPixels>,
}

impl Scene {
    /// The pivots of this finished scene's commands, marked or not, sorted.
    pub(crate) fn command_pivots(&self) -> Vec<(u32, Rc<TransformPivot>)> {
        let marked = self.scroll.marks.iter().filter_map(|mark| Some((mark.op, mark.pivot.clone()?)));
        let mut pivots: Vec<_> = self.scroll.pivots.iter().cloned().chain(marked).collect();
        pivots.sort_by_key(|(op, _)| *op);
        pivots
    }

    /// Run `copy`, which copies `source`'s commands one per command, moved
    /// by `offset`, with their pivots at hand (`copied_pivot`).
    pub(crate) fn copying_pivots<R>(&mut self, source: &Scene, offset: Point<ScaledPixels>, copy: impl FnOnce(&mut Self) -> R) -> R {
        let pivots = source.command_pivots();
        let outer = match pivots.is_empty() {
            true => self.scroll.copying.take(),
            false => self.scroll.copying.replace(Box::new(Copying { pivots, offset })),
        };
        let result = copy(self);
        self.scroll.copying = outer;
        result
    }

    /// The pivot of `primitive`, the copy of source command `index`: its
    /// source's, moved where it lands, the primitive's matrix built from
    /// it (a moved copy then holds the bits a fresh paint there builds).
    /// Marked under the open node (`mark`), a matrix a shift cannot move
    /// exactly without one notes the node (`ScrollBook::unpivoted`).
    pub(in crate::scene) fn copied_pivot(&mut self, primitive: &mut Primitive, index: usize, mark: bool) -> Option<Rc<TransformPivot>> {
        if self.scroll.copying.is_none() && !mark {
            return None;
        }
        if primitive.transformation().is_none_or(TransformationMatrix::moves_exactly) {
            return None;
        }
        let pivot = self.scroll.copying.as_ref().and_then(|copying| {
            let at = copying.pivots.binary_search_by_key(&(index as u32), |(op, _)| *op).ok()?;
            let pivot = &copying.pivots[at].1;
            Some(match copying.offset == Point::default() {
                true => pivot.clone(),
                false => Rc::new(pivot.moved(copying.offset)),
            })
        });
        if let Some(pivot) = &pivot {
            set_matrix(primitive, pivot.matrix());
        }
        self.scroll.unpivoted |= mark && pivot.is_none();
        pivot
    }

    /// Keep the pivot of an unmarked primitive a paint record inserts now
    /// (painted under the nodes open before it began), as `pivot_of` keeps
    /// a marked one's.
    pub(in crate::scene) fn note_record_pivot(&mut self, primitive: &Primitive) {
        let Some(pivot) = &self.scroll.pivot else { return };
        if self.scroll.record_base.is_some() && primitive.transformation() == Some(&pivot.matrix()) {
            self.scroll.pivots.push((self.paint_operations.len() as u32, pivot.clone()));
        }
    }
}

/// Give `primitive` the matrix `matrix` (one it has a matrix to hold).
pub(in crate::scene) fn set_matrix(primitive: &mut Primitive, matrix: TransformationMatrix) {
    match primitive {
        Primitive::Quad(p) => p.transformation = matrix,
        Primitive::Gradient(p) => p.quad.transformation = matrix,
        Primitive::MonochromeSprite(p) => p.transformation = matrix,
        Primitive::SubpixelSprite(p) => p.transformation = matrix,
        Primitive::PolychromeSprite(p) => p.transformation = matrix,
        Primitive::Shadow(_) | Primitive::Path(_) | Primitive::Underline(_) | Primitive::Surface(_) => {}
    }
}
