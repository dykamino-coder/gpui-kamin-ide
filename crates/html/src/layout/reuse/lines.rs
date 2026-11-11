//! Строки абзацев блока, пока его записывают (`recorder`): блок, у
//! которого каждый абзац уложился в одну строку, раскладывается так же в
//! любом потоке, где эти строки не короче места (`width::Width::Stretch`).

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
    /// Приёмник внешней записи, открытой, когда открыли этот (`with_sink`):
    /// цепочка живёт в самих приёмниках, без узла на каждое открытие.
    outer: RefCell<Option<Rc<LineSink>>>,
}

impl Default for LineSink {
    fn default() -> Self {
        Self {
            broken: Cell::new(false),
            slack: Cell::new(f32::INFINITY),
            paragraphs: Cell::new(0),
            outer: RefCell::new(None),
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

    /// `note` в этот приёмник и во все внешние.
    fn note_chain(self: Rc<Self>, width: f32, one: Option<f32>) {
        let mut open = Some(self);
        while let Some(sink) = open {
            sink.note(width, one);
            open = sink.outer.borrow().clone();
        }
    }
}

/// Выполнить prepaint `f` со строками абзацев, собранными и в `sink`, и в
/// приёмники внешних записей. Приёмник открывают один раз.
pub(super) fn with_sink<R>(
    sink: Rc<LineSink>,
    window: &mut Window,
    f: impl FnOnce(&mut Window) -> R,
) -> R {
    let outer = window
        .render_context::<LineSink>()
        .filter(|outer| !Rc::ptr_eq(outer, &sink));
    sink.outer.replace(outer);
    window.with_render_context(Some(sink), f)
}

/// Абзац ширины `width` уложен в строки шириной (с отступом) `lines`.
pub(crate) fn report(
    window: &Window,
    width: f32,
    vertical: bool,
    mut lines: impl ExactSizeIterator<Item = f32>,
) {
    let Some(open) = window.render_context::<LineSink>() else {
        return;
    };
    let one = (lines.len() == 1 && !vertical)
        .then(|| lines.next())
        .flatten();
    open.note_chain(width, one);
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

    #[test]
    fn an_outer_sink_hears_the_paragraphs_of_an_inner_one() {
        let (outer, inner) = (Rc::new(LineSink::default()), Rc::new(LineSink::default()));
        inner.outer.replace(Some(outer.clone()));
        inner.clone().note_chain(300.0, Some(250.0));
        assert_eq!((inner.slack(), outer.slack()), (Some(50.0), Some(50.0)));
    }
}
