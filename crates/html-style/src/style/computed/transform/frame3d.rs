//! The spatial source of an accumulated preserve-3d transform.

use std::{cell::RefCell, rc::Rc};

pub type FrameMatrices = ([[f32; 4]; 4], [[f32; 3]; 2]);

/// A context's matrices resolved at its own current spatial location.
pub trait Frame3dBuild: std::fmt::Debug {
    fn matrices(&self) -> FrameMatrices;
    fn outer(&self) -> gpui::TransformationMatrix;
}

/// Deferred children share the source, rather than a frozen paint-time matrix.
#[derive(Clone, Debug, Default)]
pub struct Frame3d(Rc<RefCell<Option<Rc<dyn Frame3dBuild>>>>);

impl PartialEq for Frame3d {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Frame3d {
    pub fn get(&self) -> Option<FrameMatrices> {
        self.source().map(|source| source.matrices())
    }

    pub fn source(&self) -> Option<Rc<dyn Frame3dBuild>> {
        self.0.borrow().clone()
    }

    pub fn set(&self, source: Rc<dyn Frame3dBuild>) {
        *self.0.borrow_mut() = Some(source);
    }
}
