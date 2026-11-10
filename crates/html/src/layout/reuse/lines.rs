//! Строки абзацев блока, пока его записывают (`recorder`): блок, у
//! которого каждый абзац уложился в одну строку, раскладывается так же в
//! любом потоке, где эти строки не короче места (`width::Width::Stretch`).

use super::FlowReuse;
use gpui::Window;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Строки абзацев одного записываемого блока.
pub(super) struct LineSink {
    /// Хотя бы один абзац занял не одну строку или вертикален.
    broken: Cell<bool>,
    /// Наименьший запас строки до края её абзаца, в точках.
    slack: Cell<f32>,
    /// Абзацев (`text::paragraph`), разложенных в блоке.
    paragraphs: Cell<u32>,
}

impl Default for LineSink {
    fn default() -> Self {
        Self {
            broken: Cell::new(false),
            slack: Cell::new(f32::INFINITY),
            paragraphs: Cell::new(0),
        }
    }
}

impl LineSink {
    /// Запас строки единственного абзаца блока, если он — одна строка.
    /// Текст блока, набранный другим путём (рядом слов, сплошным текстом),
    /// сюда не сообщает, и абзацев тогда не один.
    pub(super) fn slack(&self) -> Option<f32> {
        (!self.broken.get() && self.paragraphs.get() == 1).then(|| self.slack.get())
    }

    /// A paragraph of the block took more than one line (or is vertical).
    pub(super) fn broken(&self) -> bool {
        self.broken.get()
    }

    /// Абзац ширины `width` из одной строки `one` (`None` — иначе).
    fn note(&self, width: f32, one: Option<f32>) {
        self.paragraphs.set(self.paragraphs.get() + 1);
        match one {
            Some(line) => self.slack.set(self.slack.get().min(width - line)),
            None => self.broken.set(true),
        }
    }
}

/// The pending record whose block is in prepaint (`with_sink`) and the flow
/// reuse of the document around this one, whose open record takes the lines
/// too. A flow wraps only its own children, never one inside another, so a
/// build has at most one open record: the scope lives in its `FlowReuse`, and
/// opening it allocates nothing (the render context is the reuse itself).
#[derive(Default)]
pub(super) struct Open {
    at: Cell<Option<usize>>,
    outer: RefCell<Option<Rc<FlowReuse>>>,
}

/// Выполнить prepaint `f` со строками абзацев, собранными и в ожидающей
/// записи `at` сборки `reuse`.
pub(super) fn with_sink<R>(
    (reuse, at): (Rc<FlowReuse>, usize),
    window: &mut Window,
    f: impl FnOnce(&mut Window) -> R,
) -> R {
    let outer = window
        .render_context::<FlowReuse>()
        .filter(|outer| !Rc::ptr_eq(outer, &reuse));
    let open = &reuse.open;
    let was = (open.at.replace(Some(at)), open.outer.replace(outer));
    debug_assert!(
        was.0.is_none(),
        "a wrapped block inside another of its flow"
    );
    let out = window.with_render_context(Some(reuse.clone()), f);
    reuse.open.at.set(was.0);
    reuse.open.outer.replace(was.1);
    out
}
/// Абзац ширины `width` уложен в строки шириной (с отступом) `lines`.
pub(crate) fn report(
    window: &Window,
    width: f32,
    vertical: bool,
    mut lines: impl ExactSizeIterator<Item = f32>,
) {
    let mut open = window.render_context::<FlowReuse>();
    if open.is_none() {
        return;
    }
    let one = (lines.len() == 1 && !vertical)
        .then(|| lines.next())
        .flatten();
    while let Some(reuse) = open {
        if let Some(at) = reuse.open.at.get()
            && let Some(pending) = reuse.pending.borrow().get(at)
        {
            pending.lines.note(width, one);
        }
        open = reuse.open.outer.borrow().clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_only_single_line_paragraph_gives_its_slack_and_a_wrap_breaks_it() {
        let sink = LineSink::default();
        assert_eq!(sink.slack(), None, "no paragraph reported");
        sink.note(300.0, Some(250.0));
        assert_eq!(sink.slack(), Some(50.0));
        let two = LineSink::default();
        two.note(300.0, Some(120.0));
        two.note(300.0, Some(250.0));
        assert_eq!(two.slack(), None, "one anonymous paragraph per text block");
        sink.note(300.0, None);
        assert_eq!(sink.slack(), None, "a wrapped or vertical paragraph");
    }
}
