//! Строки абзацев блока, пока его записывают (`recorder`): блок, у
//! которого каждый абзац уложился в одну строку, раскладывается так же в
//! любом потоке, где эти строки не короче места (`width::Width::Stretch`).

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
}

thread_local! {
    static OPEN: RefCell<Vec<Rc<LineSink>>> = const { RefCell::new(Vec::new()) };
}

/// Выполнить prepaint `f` со строками абзацев, собранными в `sink`.
pub(super) fn with_sink<R>(sink: Rc<LineSink>, f: impl FnOnce() -> R) -> R {
    OPEN.with(|open| open.borrow_mut().push(sink));
    let result = f();
    OPEN.with(|open| open.borrow_mut().pop());
    result
}

/// Абзац ширины `width` уложен в строки шириной (с отступом) `lines`.
pub(crate) fn report(width: f32, vertical: bool, lines: impl ExactSizeIterator<Item = f32>) {
    OPEN.with(|open| {
        let open = open.borrow();
        if open.is_empty() {
            return;
        }
        let mut lines = lines;
        let one = (lines.len() == 1 && !vertical)
            .then(|| lines.next())
            .flatten();
        for sink in open.iter() {
            sink.paragraphs.set(sink.paragraphs.get() + 1);
            match one {
                Some(line) => sink.slack.set(sink.slack.get().min(width - line)),
                None => sink.broken.set(true),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_only_single_line_paragraph_gives_its_slack_and_a_wrap_breaks_it() {
        let sink = Rc::new(LineSink::default());
        assert_eq!(sink.slack(), None, "no paragraph reported");
        with_sink(sink.clone(), || report(300.0, false, [250.0].into_iter()));
        assert_eq!(sink.slack(), Some(50.0));
        report(300.0, false, [10.0, 20.0].into_iter());
        assert_eq!(sink.slack(), Some(50.0), "a closed sink hears nothing");
        let two = Rc::new(LineSink::default());
        with_sink(two.clone(), || {
            report(300.0, false, [120.0].into_iter());
            report(300.0, false, [250.0].into_iter());
        });
        assert_eq!(two.slack(), None, "one anonymous paragraph per text block");
        with_sink(sink.clone(), || {
            report(300.0, false, [10.0, 20.0].into_iter())
        });
        assert_eq!(sink.slack(), None);
        let vertical = Rc::new(LineSink::default());
        with_sink(vertical.clone(), || report(300.0, true, [10.0].into_iter()));
        assert_eq!(vertical.slack(), None);
    }
}
