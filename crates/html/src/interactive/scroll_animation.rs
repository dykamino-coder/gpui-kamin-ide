//! One trajectory owner per scrollport: input sets goals, vsync moves the handle.

mod coverage;
mod driver;
mod motion;
use coverage::Coverage;
pub(crate) use coverage::Tracker;
use gpui::{App, Pixels, Point, ScrollHandle, Window, point, px};
use motion::Motion;
pub(crate) use motion::Source;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

/// CSSOM View behavior for scrollTo/scrollBy/scrollIntoView and native inputs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollBehavior {
    #[default]
    Auto,
    Instant,
    Smooth,
}

/// Persistent controller carried by the native scroll area's layout state.
#[derive(Clone)]
pub struct Scroll(Rc<State>);
struct State {
    handle: ScrollHandle,
    motion: RefCell<Option<Motion>>,
    smooth: Cell<bool>,
    pending: Cell<bool>,
    last: Cell<Point<Pixels>>,
    tracker: Rc<Tracker>,
    coverage: Coverage,
    waiting: Cell<Option<Duration>>,
}

impl Scroll {
    pub(crate) fn new(handle: ScrollHandle, tracker: Rc<Tracker>) -> Self {
        let scroll = Self(Rc::new(State {
            coverage: Coverage::new(handle.offset()),
            waiting: Cell::new(None),
            last: Cell::new(handle.offset()),
            handle,
            tracker,
            motion: RefCell::new(None),
            smooth: Cell::new(false),
            pending: Cell::new(false),
        }));
        scroll.0.tracker.register(&scroll.0);
        scroll
    }

    pub(crate) fn set_smooth(&self, smooth: bool) {
        self.0.smooth.set(smooth);
    }

    fn now(&self, cx: &App) -> Duration {
        self.0.tracker.now(cx)
    }

    pub(crate) fn target(&self) -> Point<Pixels> {
        self.0
            .motion
            .borrow()
            .as_ref()
            .map_or_else(|| self.0.handle.offset(), |m| pixels(m.target))
    }

    pub(crate) fn cancel(&self) {
        self.0.motion.replace(None);
        self.0.waiting.set(None);
    }

    pub(crate) fn recorded(&self, margin: Pixels) {
        self.0.coverage.recorded(self.0.handle.offset(), margin);
    }

    pub(crate) fn to(
        &self,
        target: Point<Pixels>,
        behavior: ScrollBehavior,
        source: Source,
        window: &mut Window,
        cx: &App,
    ) -> bool {
        let target = self.clamp(target);
        let changed = target != self.target();
        let animated = !cx.reduce_motion()
            && match behavior {
                ScrollBehavior::Instant => false,
                ScrollBehavior::Smooth => true,
                ScrollBehavior::Auto => self.0.smooth.get(),
            };
        if !animated {
            let moved = target != self.0.handle.offset();
            self.cancel();
            self.0.handle.set_offset(target);
            self.0.last.set(target);
            if moved {
                window.refresh();
            }
            return changed || moved;
        }
        if !changed {
            return false;
        }
        let now = self.0.waiting.get().unwrap_or_else(|| self.now(cx));
        let old = self.0.motion.borrow_mut().take();
        let sample = old
            .as_ref()
            .filter(|_| self.0.last.get() == self.0.handle.offset())
            .map(|m| m.sample(now));
        let (start, velocity) = sample.map_or((numbers(self.0.handle.offset()), [0.; 2]), |s| {
            (s.position, s.velocity)
        });
        let motion = Motion::new(start, velocity, numbers(target), now, source);
        self.0.last.set(self.0.handle.offset());
        let (low, high) = motion.extent();
        if !self.0.coverage.contains(low, high) {
            let at = numbers(self.0.handle.offset());
            let distance = (0..2)
                .flat_map(|a| [(low[a] - at[a]).abs(), (high[a] - at[a]).abs()])
                .fold(0., f64::max);
            self.0.tracker.reserve(distance as f32);
            self.0.waiting.set(Some(now));
            window.refresh();
        }
        self.0.motion.replace(Some(motion));
        self.schedule(window);
        true
    }

    fn clamp(&self, offset: Point<Pixels>) -> Point<Pixels> {
        let max = self.0.handle.max_offset();
        point(
            offset.x.clamp(-max.x, px(0.)),
            offset.y.clamp(-max.y, px(0.)),
        )
    }
}

fn numbers(p: Point<Pixels>) -> [f64; 2] {
    [f32::from(p.x) as f64, f32::from(p.y) as f64]
}
fn pixels(p: [f64; 2]) -> Point<Pixels> {
    point(px(p[0] as f32), px(p[1] as f32))
}
