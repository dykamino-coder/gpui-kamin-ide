//! Живая идентичность прохода и композиционных областей одного окна.

use super::*;

/// Identity of one window draw, shared by prepaint and paint.
#[derive(Clone, Default)]
pub struct PaintPassId(Rc<Cell<bool>>);

/// Identity of a live composition scope, independent of host paint policy.
#[derive(Clone, Default)]
pub struct PaintScopeId(Rc<Cell<bool>>);

/// A snapshot cannot be replayed outside its original live scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PaintError {
    /// The snapshot was captured by another window.
    #[error("paint snapshot belongs to another window")]
    WrongWindow,
    /// The captured draw completed or was aborted.
    #[error("paint snapshot pass has ended")]
    ExpiredPass,
    /// Another draw is currently active in this window.
    #[error("paint snapshot belongs to another pass")]
    WrongPass,
    /// The captured composition scope has closed.
    #[error("paint snapshot composition scope has closed")]
    ExpiredScope,
    /// Another composition scope is currently active.
    #[error("paint snapshot belongs to another composition scope")]
    WrongScope,
}

#[derive(Clone)]
pub(super) struct Identity {
    window: Rc<()>,
    pass: PaintPassId,
    scope: PaintScopeId,
}

impl Identity {
    pub(super) fn capture(window: &Window) -> Self {
        Self {
            window: window.paint_window.clone(),
            pass: window.paint_pass.clone(),
            scope: window.paint_scope.clone(),
        }
    }

    pub(super) fn check(&self, window: &Window) -> Result<(), PaintError> {
        self.check_pass(window)?;
        if !self.scope.0.get() {
            return Err(PaintError::ExpiredScope);
        }
        if !Rc::ptr_eq(&self.scope.0, &window.paint_scope.0) {
            return Err(PaintError::WrongScope);
        }
        Ok(())
    }

    // Commands retain draw resources, not permission to restore an old scope.
    pub(super) fn check_pass(&self, window: &Window) -> Result<(), PaintError> {
        if !Rc::ptr_eq(&self.window, &window.paint_window) {
            return Err(PaintError::WrongWindow);
        }
        if !self.pass.0.get() {
            return Err(PaintError::ExpiredPass);
        }
        if !Rc::ptr_eq(&self.pass.0, &window.paint_pass.0) {
            return Err(PaintError::WrongPass);
        }
        Ok(())
    }
}

impl PartialEq for PaintPassId {
    fn eq(&self, other: &Self) -> bool { Rc::ptr_eq(&self.0, &other.0) }
}
impl Eq for PaintPassId {}

impl PaintPassId {
    /// Whether the original draw is still open and has not unwound.
    pub fn is_live(&self) -> bool { self.0.get() }
    pub(super) fn invalidate(&self) { self.0.set(false); }
}

impl PartialEq for PaintScopeId {
    fn eq(&self, other: &Self) -> bool { Rc::ptr_eq(&self.0, &other.0) }
}
impl Eq for PaintScopeId {}

impl PaintScopeId {
    /// Whether the original composition scope remains open.
    pub fn is_live(&self) -> bool { self.0.get() }
}

impl Window {
    /// Identify the current draw without extending its validity.
    pub fn paint_pass_id(&self) -> PaintPassId { self.paint_pass.clone() }

    /// Identify the current composition scope without extending its validity.
    pub fn paint_scope_id(&self) -> PaintScopeId { self.paint_scope.clone() }

    pub(crate) fn with_paint_pass<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let pass = mem::replace(&mut self.paint_pass, PaintPassId(Rc::new(Cell::new(true))));
        let scope = mem::replace(&mut self.paint_scope, PaintScopeId(Rc::new(Cell::new(true))));
        let state = Box::new(self.snapshot_paint_state());
        let inputs = (self.layout_retry, self.layout_inputs.clone());
        self.with_restore((pass, scope, state, self.refreshing, inputs), |window, previous| {
            if std::thread::panicking() || !window.paint_pass.is_live() {
                previous.2.restore(window);
                window.next_frame.clear();
                if let Some(layout) = window.layout_engine.as_mut() { layout.clear(); }
                window.text_system().finish_frame();
                window.refreshing = previous.3;
                window.layout_retry = previous.4.0;
                mem::swap(&mut window.layout_inputs, &mut previous.4.1);
                window.invalidator.set_phase(DrawPhase::None);
            }
            window.paint_pass.0.set(false);
            window.paint_scope.0.set(false);
            mem::swap(&mut window.paint_pass, &mut previous.0);
            mem::swap(&mut window.paint_scope, &mut previous.1);
        }, f).0
    }

    pub(super) fn with_paint_scope<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let scope = mem::replace(&mut self.paint_scope, PaintScopeId(Rc::new(Cell::new(true))));
        self.with_restore(scope, |window, previous| {
            window.paint_scope.0.set(false);
            mem::swap(&mut window.paint_scope, previous);
        }, f).0
    }

    pub(super) fn with_isolated_scene<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> R,
    ) -> (R, Scene) {
        let mut scene = Scene::default();
        scene.in_group = true;
        mem::swap(&mut self.next_frame.scene, &mut scene);
        self.with_restore(scene, |window, scene| {
            mem::swap(&mut window.next_frame.scene, scene);
        }, |window| window.with_paint_scope(f))
    }
}
