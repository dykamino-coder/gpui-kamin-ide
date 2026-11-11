//! Строки абзацев блока, пока его записывают (`recorder`): блок, у
//! которого каждый абзац уложился в одну строку, раскладывается так же в
//! любом потоке, где эти строки не короче места (`width::Width::Stretch`).

use super::Record;
use gpui::Window;
use std::cell::Cell;
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

    /// Строки блока `inner`, записанного внутри этого: внешняя запись
    /// слышит их разом, когда внутренняя закрыта (`with_record`).
    fn absorb(&self, inner: &LineSink) {
        self.paragraphs
            .set(self.paragraphs.get() + inner.paragraphs.get());
        self.slack.set(self.slack.get().min(inner.slack.get()));
        self.broken.set(self.broken.get() || inner.broken.get());
    }
}

/// Выполнить prepaint `f` со строками абзацев, собранными в запись `record`
/// и, когда он закончен, во внешнюю открытую запись. Открытая запись — сама
/// запись блока в контексте окна: открытие ничего не выделяет.
pub(super) fn with_record<R>(
    record: &Rc<Record>,
    window: &mut Window,
    f: impl FnOnce(&mut Window) -> R,
) -> R {
    let outer = window
        .render_context::<Record>()
        .filter(|outer| !Rc::ptr_eq(outer, record));
    let result = window.with_render_context(Some(record.clone()), f);
    if let Some(outer) = outer {
        outer.making.lines().absorb(record.making.lines());
    }
    result
}

/// Абзац ширины `width` уложен в строки шириной (с отступом) `lines`.
pub(crate) fn report(
    window: &Window,
    width: f32,
    vertical: bool,
    mut lines: impl ExactSizeIterator<Item = f32>,
) {
    let Some(open) = window.render_context::<Record>() else {
        return;
    };
    let one = (lines.len() == 1 && !vertical)
        .then(|| lines.next())
        .flatten();
    open.making.lines().note(width, one);
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
        let (outer, inner) = (LineSink::default(), LineSink::default());
        inner.note(300.0, Some(250.0));
        outer.absorb(&inner);
        assert_eq!((inner.slack(), outer.slack()), (Some(50.0), Some(50.0)));
        let wrapped = LineSink::default();
        wrapped.note(300.0, None);
        outer.absorb(&wrapped);
        assert!(outer.broken() && outer.slack().is_none());
    }
}
