//! Элемент Sticky: липкий сдвиг по кадру прокрутки.

use crate::hooks::Wrap;
use gpui::{AnyElement, App, Bounds, Pixels, Window, px};

/// Что липкому элементу известно о родителе и о видимой части ленты.
///
/// Оба прямоугольника снимает распорка, которую сборщик дерева кладёт первым
/// ребёнком родителя: сам элемент к моменту своей отрисовки уже вынесен из
/// потока и обрезки ленты не видит.
#[derive(Clone, Copy, Default)]
pub struct StickyFrame {
    pub container: Option<Bounds<Pixels>>,
    pub viewport: Option<Bounds<Pixels>>,
}

pub type StickyCell = std::rc::Rc<std::cell::Cell<StickyFrame>>;

/// `position: sticky`: элемент едет с потоком, пока не упрётся в край видимой
/// части, и дальше стоит у края — но не выходит за пределы родителя.
///
/// Раньше липкий вёл себя как обычный: смещение ленты элементу было неоткуда
/// узнать. Теперь видимую часть даёт распорка родителя, а порядок отрисовки —
/// отложенный проход: иначе содержимое, идущее ниже по разметке, закрашивало
/// бы прилипший заголовок.
pub struct Sticky {
    pub(crate) child: AnyElement,
    /// Пороги прилипания в точках; `None` — сторона не задана.
    pub top: Option<f32>,
    pub bottom: Option<f32>,
    pub left: Option<f32>,
    pub right: Option<f32>,
    pub frame: StickyCell,
    /// The shift this box applies to its subtree (`ShiftLink`).
    pub(crate) link: crate::interactive::scroll_extent::StickyShift,
    /// Painted by GPUI's deferred pass, outside the scroller's clip.
    pub(crate) deferred: bool,
}

impl Sticky {
    pub(crate) fn new(
        child: AnyElement,
        frame: StickyCell,
        link: crate::interactive::scroll_extent::StickyShift,
    ) -> Self {
        Sticky {
            link,
            child,
            top: None,
            bottom: None,
            left: None,
            right: None,
            frame,
            deferred: false,
        }
    }

    fn edges(&self) -> Edges {
        Edges {
            top: self.top,
            bottom: self.bottom,
            left: self.left,
            right: self.right,
        }
    }
}

/// Sticky thresholds of a box: what its shift depends on besides the
/// geometry, kept by the follower node a scrolled replay moves it with.
#[derive(Clone, Copy)]
struct Edges {
    top: Option<f32>,
    bottom: Option<f32>,
    left: Option<f32>,
    right: Option<f32>,
}

impl Edges {
    /// Насколько сдвинуть элемент, чтобы он остался у края видимой части.
    fn shift(self, frame: StickyFrame, bounds: Bounds<Pixels>) -> gpui::Point<Pixels> {
        let Some(view) = frame.viewport else {
            return gpui::point(px(0.0), px(0.0));
        };
        let mut dx = px(0.0);
        let mut dy = px(0.0);
        if let Some(t) = self.top {
            let want = view.origin.y + px(t);
            if bounds.origin.y < want {
                dy = want - bounds.origin.y;
            }
        }
        if let Some(b) = self.bottom {
            let want = view.origin.y + view.size.height - px(b) - bounds.size.height;
            if bounds.origin.y > want {
                dy = want - bounds.origin.y;
            }
        }
        if let Some(l) = self.left {
            let want = view.origin.x + px(l);
            if bounds.origin.x < want {
                dx = want - bounds.origin.x;
            }
        }
        if let Some(r) = self.right {
            let want = view.origin.x + view.size.width - px(r) - bounds.size.width;
            if bounds.origin.x > want {
                dx = want - bounds.origin.x;
            }
        }
        // За пределы родителя липкий не выходит: у края родителя он снова
        // уезжает вместе с потоком.
        if let Some(c) = frame.container {
            let max_y = c.origin.y + c.size.height - bounds.size.height - bounds.origin.y;
            let min_y = c.origin.y - bounds.origin.y;
            dy = dy.clamp(min_y.min(px(0.0)), max_y.max(px(0.0)));
            let max_x = c.origin.x + c.size.width - bounds.size.width - bounds.origin.x;
            let min_x = c.origin.x - bounds.origin.x;
            dx = dx.clamp(min_x.min(px(0.0)), max_x.max(px(0.0)));
        }
        gpui::point(dx, dy)
    }
}

impl Wrap for Sticky {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let (edges, frame) = (self.edges(), self.frame.get());
        let shift = edges.shift(frame, bounds);
        // The shift follows the scroller's offset (css-position-3
        // §stickypos-scroll): a follower node moves the subtree when only
        // that offset changes, the box and its scrollport as recorded now.
        let moved = move |change: gpui::Point<Pixels>| {
            let (mut box_frame, mut at) = (frame, bounds);
            at.origin += change;
            if let Some(container) = &mut box_frame.container {
                container.origin += change;
            }
            edges.shift(box_frame, at) - shift
        };
        let id = gpui::ScrollNodeId::of_token(&self.frame);
        let child = &mut self.child;
        self.link.applied(shift, || {
            window.with_scroll_follower_prepaint(id, None, std::rc::Rc::new(moved), |window| {
                window.with_exact_element_offset(shift, |window| child.prepaint(window, cx))
            });
        });
    }

    fn paint(&mut self, _bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let id = gpui::ScrollNodeId::of_token(&self.frame);
        let child = &mut self.child;
        // A box deferred out of its scroller paints outside the scroller's
        // clip: put the clip back (`sticky_wrap`, positive `z-index`).
        let view = self.deferred.then(|| self.frame.get().viewport).flatten();
        window.with_scroll_node_paint(id, |window| match view {
            Some(view) => window
                .with_content_mask(Some(gpui::ContentMask { bounds: view }), |window| {
                    child.paint(window, cx)
                }),
            None => child.paint(window, cx),
        });
    }
}
