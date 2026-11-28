//! Prepare the trajectory hull once, then vsync translates the retained record.

use gpui::{Pixels, Point, px};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

/// A document's prepared interest and displayed animation frames.
pub(crate) struct Tracker {
    frame: Cell<bool>,
    origin: Instant,
    elapsed: Cell<Option<Duration>>,
    margin: Cell<f32>,
    preparing: Cell<bool>,
    controllers: RefCell<Vec<Weak<super::State>>>,
}

impl Default for Tracker {
    fn default() -> Self {
        Self {
            frame: Cell::new(false),
            origin: Instant::now(),
            elapsed: Cell::new(None),
            margin: Cell::new(4000.),
            preparing: Cell::new(false),
            controllers: RefCell::new(Vec::new()),
        }
    }
}

impl Tracker {
    pub(super) fn register(&self, state: &Rc<super::State>) {
        let mut controllers = self.controllers.borrow_mut();
        controllers.retain(|weak| weak.strong_count() > 0);
        controllers.push(Rc::downgrade(state));
    }

    /// Geometry observes every ancestor/nested scrollport at one clock sample.
    pub(crate) fn synchronize(&self, window: &mut gpui::Window, cx: &gpui::App) {
        let live: Vec<_> = self
            .controllers
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        let now = self.now(cx);
        for state in live {
            super::Scroll(state).synchronize_at(now, window, cx);
        }
    }

    pub(crate) fn set_elapsed(&self, elapsed: Option<Duration>) {
        self.elapsed.set(elapsed);
    }
    pub(super) fn now(&self, cx: &gpui::App) -> Duration {
        self.elapsed
            .get()
            .or_else(|| cx.try_global::<gpui::AnimationElapsedTime>().map(|t| t.0))
            .unwrap_or_else(|| self.origin.elapsed())
    }
    pub(crate) fn margin(&self) -> Pixels {
        px(self.margin.get())
    }
    pub(crate) fn preparing(&self) -> bool {
        self.preparing.get()
    }
    pub(super) fn frame(&self) {
        self.frame.set(true);
    }

    pub(super) fn reserve(&self, distance: f32) {
        self.margin.set(self.margin.get().max(distance + 4000.));
        self.preparing.set(true);
    }

    pub(crate) fn painted(&self, built: bool) {
        if built {
            self.preparing.set(false);
            self.margin.set(4000.);
        }
        if self.frame.replace(false) {
            kamin_style::journal::scroll_animation_frame(built);
        }
    }
}

/// Coverage stays centred on the last fresh prepaint, also after many replays.
pub(super) struct Coverage(Cell<(Point<Pixels>, f32)>);
impl Coverage {
    pub fn new(offset: Point<Pixels>) -> Self {
        Self(Cell::new((offset, 0.)))
    }
    pub fn recorded(&self, offset: Point<Pixels>, margin: Pixels) {
        self.0.set((offset, margin.into()));
    }
    pub fn contains(&self, low: [f64; 2], high: [f64; 2]) -> bool {
        let (centre, radius) = self.0.get();
        [f32::from(centre.x), f32::from(centre.y)]
            .into_iter()
            .enumerate()
            .all(|(axis, at)| {
                low[axis] >= (at - radius) as f64 && high[axis] <= (at + radius) as f64
            })
    }
}
